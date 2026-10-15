#!/usr/bin/env python3
"""从青简全量译词表裁出英文释义子集，给水杉 Android 随包带。

    用法（在 engine/ 目录）：
        python -X utf8 tools/build_glossary_subset.py --out <目标 tsv> [--top-n N]
                                                       [--append 源件 | --no-append]

**已知活雷**：随包的 `glossary-en.tsv` = 本脚本裁出的子集 **加上 A3 手写整句条目**
（`engine/tools/glossary-sentences-zh-en.tsv`）。`--append` 默认就指向那个源件，
所以不带参数重跑得到的是完整的随包表；**加 `--no-append` 重跑会把那些整句条目冲掉**，
而体积只小几 KB、测试也不会红——没人会发现。A3 条目在上游全量表里不存在，
只能靠那个源件带回来，别把它删了或跳过。
（这里刻意不写行数：那张表还在随 A3 长，写死的数字一定再烂一次。）

**顺带产出资源锁**：写完表之后会把 `--lock` 指的那份 `desktop-dictionary.lock.json` 重写成
`{"artifacts":[{"name":…,"bytes":…}]}`，字节数按**磁盘上实际文件**量。`refreshHostRaw`
把这份锁烘进 `.so` 比长度，所以**改词库必须重跑本脚本 + 重编 `.so`**（见 README 构建步骤）；
锁与实际文件脱节时 `engine/src/resources_lock.rs` 的单测会红。

`--top-n` 不给时取**整个词库**（= 词库 ∩ 全量译词表，能达到的覆盖上限）。
2026-10-13 A1 起这是默认值：随包表 88,349 行 / 2,116,088 B。
**以前这里是 80_000，是个活雷**——谁不带参数重跑一次，覆盖就无声退回 76,273 行、
不报错也不掉体积，只会让真机少一批译词，所以改成从词库行数动态推导上限。

历史上 80k 的来由（2026-10-12 在 host 三元组上用真词库 + 真译词表实测的
「候选里有多少条能查到释义」，全量译词表 6,788,267 B / 232,213 行，词库 93,236 条带词频）：

    shi                      415 / 500 = 83.0%
    nihao                     30 /  44 = 68.2%
    zhonghuarenmingongheguo  474 / 503 = 94.2%
    woxiangquningbo          203 / 269 = 75.5%

同一组输入在 30k 时是 78.2% / 43.2% / — / 41.6%，30k→80k 这一段涨得最陡，80k 之后趋平。
这四个数字是**一次采样的命中率**，不是产品指标，别拿去做宣传。
A1 之后重算的三个口径（词库侧 / 词频加权 / 候选侧）见 `README.md`「当前状态」的 A1 那条，
差别很大：**词频加权只涨 0.026 个百分点**，涨的主要是单音节翻页。

只依赖标准库。
"""
import argparse
import json
import sys
from pathlib import Path

DEFAULT_DICT = Path("../../qingjian/assets/lexicon/dict.tsv")
DEFAULT_GLOSSARY = Path("../../qingjian/assets/glossary/glossary-en.tsv")
# 随包资源锁。宿主 `Bootstrap.packagedArtifacts` 只读 `name`，多出来的 `bytes`
# 由我们的 `refreshHostRaw` 用；加字段不改一行 Java（理由见 README「升级重解包」那条）。
# 相对 engine/ 目录，所以是一层 `..`（`DEFAULT_DICT` 那两条才是跨到同级 qingjian）。
DEFAULT_LOCK = Path("../host/assets/desktop-dictionary.lock.json")
# A3 手写整句条目的源件。默认追加，因为随包表就是「子集 + 这份」；
# 不追加会静默丢掉 A3 那 165 行（体积只小几 KB，没人会发现），所以留了 --no-append
# 显式开关，而不是把它做成默认行为。
DEFAULT_APPEND = Path(__file__).resolve().parent / "glossary-sentences-zh-en.tsv"

HEADER = (
    "# 英文释义子集，由 qingshan/engine/tools/build_glossary_subset.py 从青简全量译词表裁出。\n"
    "# 取词库按词频降序的前 {top_n} 个词与全量译词表的交集，格式与上游一致：词\\t[词性. ]译词\\t…\n"
    "# 载入侧按 `Glossary::parse` 逐行解析，`#` 开头与空行跳过"
    "（crates/qingjian-translate/src/glossary/mod.rs:46）。\n"
)


def words_with_frequency(path):
    """dict.tsv 的 (词, 词频) 列表。列序是 词\\t音节\\t词频，`#` 行与字段不足的行跳过。"""
    rows = []
    skipped = 0
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) < 3:
            skipped += 1
            continue
        try:
            frequency = int(fields[2].strip())
        except ValueError:
            skipped += 1
            continue
        rows.append((fields[0].strip(), frequency))
    return rows, skipped


def write_lock(lock_path, assets_dir, table_name):
    """把资源锁重写成 `name` + `bytes`，字节数一律按磁盘上的实际文件量。

    文件名沿用锁里已有的那份清单（锁是「随包带哪些文件」的出处），本函数只补尺寸。
    任何一项对不上就**宁可不写**并返回错误串：一份与实际不符的锁会让每次启动都误判
    过期、每次重解包 4.2 MB。
    """
    if not lock_path.is_file():
        return f"资源锁不存在：{lock_path}"
    try:
        current = json.loads(lock_path.read_text(encoding="utf-8"))
    except ValueError as error:
        return f"资源锁读不出：{error}"
    names = [item.get("name") for item in current.get("artifacts", [])]
    if not names or any(not name for name in names):
        return f"资源锁里没有合法文件名：{lock_path}"
    if table_name not in names:
        return f"生成的表 {table_name} 不在锁的清单里：{names}"
    artifacts = []
    for name in names:
        path = assets_dir / name
        if not path.is_file():
            return f"锁里的 {name} 在 {assets_dir} 里不存在，锁不重写"
        artifacts.append({"name": name, "bytes": path.stat().st_size})
    lock_path.write_text(
        json.dumps({"artifacts": artifacts}, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    return None


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--dict", type=Path, default=DEFAULT_DICT)
    parser.add_argument("--glossary", type=Path, default=DEFAULT_GLOSSARY)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--top-n", type=int, default=None,
                        help="取词库按词频降序的前 N 个词；不给 = 整个词库（覆盖上限）")
    parser.add_argument("--append", type=Path, default=DEFAULT_APPEND,
                        help="裁完再追加这份手写条目（默认指 glossary-sentences-zh-en.tsv）")
    parser.add_argument("--no-append", action="store_true",
                        help="只出子集，不追加手写整句——会冲掉 A3 条目，慎用")
    parser.add_argument("--lock", type=Path, default=DEFAULT_LOCK,
                        help="写完表后把随包资源锁重写成 name+bytes（refreshHostRaw 烘进 .so）")
    args = parser.parse_args(argv)

    for path in (args.dict, args.glossary):
        if not path.is_file():
            print(f"输入不存在：{path}", file=sys.stderr)
            return 1
    if args.no_append:
        args.append = None
    elif not args.append.is_file():
        print(f"追加源件不存在：{args.append}", file=sys.stderr)
        return 1

    rows, skipped = words_with_frequency(args.dict)
    if args.top_n is None:
        args.top_n = len(rows)
    if args.top_n <= 0 or args.top_n > len(rows):
        print(f"--top-n 必须在 1..{len(rows)}，收到 {args.top_n}", file=sys.stderr)
        return 1
    # 词频相同的并列词按词本身排序，保证同一输入两次生成得到同一个集合。
    rows.sort(key=lambda row: (-row[1], row[0]))
    wanted = {word for word, _ in rows[: args.top_n]}
    cutoff = rows[args.top_n - 1][1]

    kept = []
    total_lines = 0
    for raw in args.glossary.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        total_lines += 1
        if line.split("\t", 1)[0].strip() in wanted:
            kept.append(line)

    body = HEADER.format(top_n=args.top_n) + "\n".join(kept) + "\n"
    extra_count = 0
    if args.append is not None:
        seen = {line.split("\t", 1)[0].strip() for line in kept}
        for raw in args.append.read_text(encoding="utf-8").splitlines():
            line = raw.strip()
            if not line:
                continue
            if line.startswith("#") or line.split("\t", 1)[0].strip() not in seen:
                body += line + "\n"
                if not line.startswith("#"):
                    extra_count += 1
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(body, encoding="utf-8")

    lock_error = write_lock(args.lock, args.out.parent, args.out.name)
    if lock_error is not None:
        print(f"资源锁没写：{lock_error}", file=sys.stderr)
        return 1

    print(f"词库带词频的行 {len(rows)}（跳过 {skipped}），取前 {args.top_n}，"
          f"第 {args.top_n} 名的词频 = {cutoff}")
    print(f"译词表 {total_lines} 行，命中 {len(kept)} 行")
    if args.append is None:
        print("追加：--no-append，手写整句条目未写入（A3 条目会丢）")
    else:
        print(f"追加 {args.append.name}：{extra_count} 行手写条目")
    print(f"写出 {args.out}，{args.out.stat().st_size:,} B")
    for item in json.loads(args.lock.read_text(encoding="utf-8"))["artifacts"]:
        print(f"资源锁 {args.lock}：{item['name']} = {item['bytes']:,} B")
    return 0


if __name__ == "__main__":
    sys.exit(main())
