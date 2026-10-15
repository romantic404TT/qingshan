#!/usr/bin/env python3
"""扫宿主对 native 桥的调用点，按「catch 体做什么」判定会不会杀会话。

背景见 `third-party/PIN.md` 的「启动路径的击穿判定」：判定一条调用点是否击穿，
不能只看有没有 `catch`，要看 `catch` 体里调的是 `fail()`/`stop()`（会话照死）
还是只写一条提示（真降级）。阶段二每加一个真实现都要复跑本脚本，确认没有把
`fail()` 组的站点数改大。

用法（在 `engine/` 目录，也可从仓库根跑）：
    python -X utf8 tools/scan_bridges.py              # 全表 + 汇总
    python -X utf8 tools/scan_bridges.py --summary    # 只要计数
    python -X utf8 tools/scan_bridges.py --json       # 机器可读，供未来接 CI
    python -X utf8 tools/scan_bridges.py --java <路径> --lib <路径>   # 覆盖默认文件

分类口径（`判定` 列）：
    real        该方法我们已真实现，不属降级面
    杀会话      桩，且 catch 体调 `fail()`/`stop()` ⇒ 触发即 `session = 0`
    降级        桩，catch 体只写提示/弹诊断/忽略 ⇒ 会话活着
    不解析      桩，返回值根本没被解析（如裸调丢弃）⇒ 安全
    需人工定性  桩，本方法内无 catch，异常会冒到调用方 —— 得逐个查调用方

已知局限（读输出前先看这几条）：
1. **依赖 `MSIMEInputService.java` 的结构稳定性**：脚本靠花括号配对还原 `try` 边界，
   用正则认 `catch`。水杉上游若重构该文件（挪方法、改 catch 结构、把 `fail()` 包进
   别的辅助函数），解析逻辑要同步更新，否则会把「杀会话」误报成「降级」。
2. 「解析方式」列是尽力判定，**已知两处错标**（都不影响判定列，因为那两处本来就在
   `fail()` 组里）：`:6034`/`:6035` 写成「不解析」，实际是 `String result = switch (...) { … }`
   之后才解析；`:3273` 写成「硬解析(跨行)」，实际外层是 `reopenPairedQuote(..)`。
   同一行找不到 `value(`/`apply(` 时只回看前 3 行；lambda（`() -> NativeClient.x(..)`）
   与 `XxxModel.decode(..)` 都算「间接」。**要信的是判定列，不是解析方式列。**
3. **扫描范围只覆盖 `MSIMEInputService.java`**，其余调用 `NativeClient` 的文件不在口径内。
   这里原先写着「它们不持会话，最坏是各自界面失败」——**这句已被证伪**。已知反例：
   `ImeCandidates.java:210-213`（展开候选面板点候选 → `NativeClient.selectAnyCandidate`
   是桩 → catch 体 `s.fail()`），那是货真价实的杀会话站点，只是不计在本脚本的口径数里
   （2026-10-13 现值 5，阶段一 12）。该站点在阶段二 A 步已随 `selectAnyCandidateRaw` 转
   real，但它证明的「口径外」这件事仍然成立。
   同类站点可能还在其余文件里，**要逐个手工排查**（`grep -n "NativeClient\\." <文件>`
   再看外层 catch 体）。所以本脚本的计数是「`MSIMEInputService.java` 内的口径数」，
   不是全局数。其余文件各有自己的软失败 helper（如 `home/HostStore.java:251-269` 的
   `value()`/`call()` 返回 null），多数确实不杀会话——但那是逐个查出来的，不是推出来的。
"""
import json
import re
import sys
from pathlib import Path

BS = chr(92)
PREFIX = "Java_app_msime_android_NativeClient_"
HARD = ("value(", "apply(", "new JSONObject(", "decode(")


def default_paths():
    root = Path(__file__).resolve().parents[2]
    return (root / "host/java/app/msime/android/core/MSIMEInputService.java",
            root / "engine/src/lib.rs")


def strip_noise(text):
    """抹掉字符串/字符字面量与注释，保留换行与长度，使行号与花括号深度不失真。"""
    out, i, n = [], 0, len(text)
    while i < n:
        c = text[i]
        if c == '"' or c == "'":
            quote, j = c, i + 1
            while j < n:
                if text[j] == BS:
                    j += 2
                    continue
                if text[j] == quote:
                    j += 1
                    break
                j += 1
            seg = text[i:j]
            out.append(seg[0] + "".join(ch if ch == "\n" else " " for ch in seg[1:-1])
                       + (seg[-1] if seg.endswith(quote) else ""))
            i = j
        elif text.startswith("//", i):
            j = text.find("\n", i)
            j = n if j < 0 else j
            out.append(" " * (j - i))
            i = j
        elif text.startswith("/*", i):
            j = text.find("*/", i)
            j = n if j < 0 else j + 2
            out.append("".join(ch if ch == "\n" else " " for ch in text[i:j]))
            i = j
        else:
            out.append(c)
            i += 1
    return "".join(out)


def match_braces(lines):
    pairs, stack = {}, []
    for idx, line in enumerate(lines, 1):
        for ch in line:
            if ch == "{":
                stack.append(idx)
            elif ch == "}" and stack:
                pairs[stack.pop()] = idx
    return pairs


CATCH = re.compile(r"\bcatch\s*\(([^)]*)\)\s*")


def collect_tries(clean, lines, pairs):
    """返回 [(起始行, 结束行, [catch 体文本, ...])]。"""
    tries = []
    for m in re.finditer(r"\btry\b\s*\{", clean):
        start = clean.count("\n", 0, m.end() - 1) + 1
        if start not in pairs:
            continue
        end = pairs[start]
        seg = "\n".join(lines[end - 1:])
        bodies, k = [], 0
        while True:
            mm = CATCH.search(seg, k)
            if not mm:
                break
            brace = seg.find("{", mm.end())
            if brace < 0:
                break
            depth, p = 0, brace
            while p < len(seg):
                if seg[p] == "{":
                    depth += 1
                elif seg[p] == "}":
                    depth -= 1
                    if depth == 0:
                        break
                p += 1
            bodies.append(seg[brace:p + 1])
            k = p + 1
            if not re.match(r"\s*(catch|finally)\b", seg[k:k + 40]):
                break
        tries.append((start, end, bodies))
    return tries


def parse_form(raw_lines, idx, call_start):
    """判定解析方式：同一行的包裹词优先，否则回看前 3 行。"""
    head = raw_lines[idx - 1][:call_start]
    for token in HARD:
        if token in head:
            return "硬解析" + token[:-1]
    for back in range(1, 4):
        prev = raw_lines[idx - 1 - back] if idx - 1 - back >= 0 else ""
        if prev.rstrip().endswith("(") or any(t in prev for t in ("value(", "apply(", "decode(")):
            return "硬解析(跨行)"
    return "不解析"


def main():
    argv = sys.argv[1:]
    java, lib = default_paths()
    for flag, key in (("--java", "java"), ("--lib", "lib")):
        if flag in argv:
            i = argv.index(flag)
            if i + 1 >= len(argv):
                raise SystemExit("%s 后面要跟路径" % flag)
            if key == "java":
                java = Path(argv[i + 1])
            else:
                lib = Path(argv[i + 1])
            del argv[i:i + 2]
    summary_only = "--summary" in argv
    as_json = "--json" in argv
    for flag in ("--summary", "--json"):
        if flag in argv:
            argv.remove(flag)
    if argv:
        raise SystemExit("不认识的参数: %s（用法见文件头）" % " ".join(argv))
    for p, what in ((java, "Java 宿主"), (lib, "Rust 导出")):
        if not p.is_file():
            raise SystemExit("%s 文件不存在: %s" % (what, p))

    real = {m[:-3] for m in re.findall(PREFIX + r"(\w+Raw)", lib.read_text(encoding="utf-8"))}
    if not real:
        raise SystemExit("在 %s 里一个真导出都没解析到——正则或路径错了，不是产物没问题" % lib)

    src = java.read_text(encoding="utf-8")
    raw_lines = src.split("\n")
    clean = strip_noise(src)
    lines = clean.split("\n")
    tries = collect_tries(clean, lines, match_braces(lines))

    rows = []
    for idx, line in enumerate(lines, 1):
        m = re.search(r"NativeClient\.(\w+)\(", line)
        if not m or "private static native" in raw_lines[idx - 1]:
            continue
        name = m.group(1)
        covering = [t for t in tries if t[0] <= idx <= t[1]]
        bodies = covering[-1][2] if covering else []
        kills = bool(re.search(r"\b(fail|stop)\s*\(", " ".join(bodies)))
        form = parse_form(raw_lines, idx, m.start())
        if name in real:
            verdict = "real"
        elif kills:
            verdict = "杀会话"
        elif not covering:
            verdict = "不解析" if form == "不解析" else "需人工定性"
        else:
            verdict = "降级"
        rows.append({"line": idx, "method": name, "form": form,
                     "catch": "有" if covering else "无", "verdict": verdict})

    counts = {}
    for r in rows:
        counts[r["verdict"]] = counts.get(r["verdict"], 0) + 1
    total_stub = sum(v for k, v in counts.items() if k != "real")

    if as_json:
        print(json.dumps({"file": str(java), "total": len(rows),
                          "real": counts.get("real", 0), "stub": total_stub,
                          "counts": counts, "sites": rows}, ensure_ascii=False, indent=1))
        return
    print("扫描 %s" % java)
    print("调用点 %d = real %d + 桩 %d" % (len(rows), counts.get("real", 0), total_stub))
    if not summary_only:
        print("%-6s %-38s %-14s %-5s %s" % ("行", "native 方法", "解析方式", "catch", "判定"))
        for r in rows:
            mark = "★" if r["verdict"] == "杀会话" else ("?" if r["verdict"] == "需人工定性" else " ")
            print("%-6d %-38s %-14s %-5s %s %s"
                  % (r["line"], r["method"], r["form"], r["catch"], mark, r["verdict"]))
        print()
    for key in ("杀会话", "降级", "不解析", "需人工定性", "real"):
        if key in counts:
            print("  %-8s %d" % (key, counts[key]))
    print("\n基线（2026-10-13 收口后实测）：杀会话 5 / 降级 32 / 不解析 1 / 需人工定性 1 / real 23。"
          "\n历史：阶段一收尾实测 12 / 降级 34 / real 14；B1 后 8、B2 后 6 是当时写下的期望值，"
          "没有单独实测过，别当数据引用。转 real 的 9 个站点：B1 的 `:6034`/`:6035`/`:6037`/`:6233`、"
          "B2 的 `:5968`/`:6039`、A 的 `:6334` 与另两个 allCandidates 站点 `:2586`/`:2662`"
          "（前 7 个原属杀会话，后 2 个原属降级）。"
          "\n口径外已知站点 1 个：ImeCandidates.java:210-213（selectAnyCandidate），见局限第 3 条。"
          "\n阶段二每加一个真实现，杀会话数应当只降不升。")


if __name__ == "__main__":
    main()
