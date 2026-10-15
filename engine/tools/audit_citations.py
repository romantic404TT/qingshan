#!/usr/bin/env python3
"""审计文档与源码里的 `文件:行号` 引用，逐条对盘核验。

对每条引用判四件事：这个文件到底存在吗、路径能不能在仓库里锚定、行号是否越界、
同名文件是否有多个。用来拦截「二手引用」这一类错误：写错的前缀（`core/X.java` 而真
身在 `candidate/`）、缺前缀（`native/client_jni.cpp` 实为
`platforms/android/native/client_jni.cpp`）、根本不存在的文件（曾写过
`scripts/build-native-rust.sh`）。

用法（`--root` 默认当前目录；文档路径会依次试 `<root>/` 与 `<root>/qingshan/`，
所以从 `qingshan/` 或 `projects/` 跑都行，从 `engine/` 跑要 `--root ..`）：
    # 跨仓索引（推荐：能同时命中 msime-develop 与 qingjian 的真身）
    cd ../../../                        # 即 projects/
    python -X utf8 qingshan/engine/tools/audit_citations.py

    # 只审本项目
    cd qingshan
    python -X utf8 engine/tools/audit_citations.py

    # 3.6：把交叉编译产物等额外目录纳入扫描
    python -X utf8 engine/tools/audit_citations.py --root . --doc engine/jniLibs

退出码：0 = 全部 OK；1 = 有非 OK 条目；2 = 没找到任何引用（多半是 --doc/--root 给错，
刻意报错而不是静默返回空表 —— 空结果必须先证伪）。

已知局限（读结果前必看）：
  1. **同名孪生文件不能判定"指的是哪一份"。** `msime-develop/` 与 `qingshan/host/`
     各有一份 `Bootstrap.java`、`MSIMEInputService.java` 等，索引按目录遍历顺序取第一个
     命中，所以 `AMBIG` 只说明"存在多份"，不说明引用写错；同理它给的 resolved 路径可能
     是另一仓的同名文件。要定论得人工看引用所在小节的基准声明。
  2. 只核存在性与行号范围，**不核该行内容是否真支持该结论**（比如行号在范围内但指向
     了别的方法），那部分仍要人工回读。
  3. 短引用（如 `core/ResourcePacks.java:18`，其基准目录由所在小节声明）会被标成
     `MISSING-PATH`，这是刻意的严格化；靠 detail 里的候选数区分：有候选=只是没锚定，
     零候选=前缀写错。
"""
import argparse
import os
import re
import sys
from collections import Counter

EXTS = (".rs", ".java", ".kt", ".cpp", ".h", ".sh", ".md", ".toml", ".kts", ".json", ".py")
SKIP_DIRS = {"target", ".git", "node_modules", ".gradle", "build", "out", ".idea", "__pycache__"}
# 仓库根一级的合法前缀：以此开头的引用才算"锚定"，否则一律 MISSING-PATH
KNOWN_PREFIXES = ("crates", "platforms", "engine", "host", "apps", "shared")
DEFAULT_DOCS = ["README.md", "NOTICE.md", "third-party/PIN.md", "engine/src", "engine/tools", ".gitignore"]

CITE_RE = re.compile(
    r"([A-Za-z0-9_./-]*[A-Za-z0-9_-])\.(rs|java|kt|cpp|h|sh|md|toml|kts|json|py):(\d+)(?:-(\d+))?"
)


def build_index(root):
    """basename -> 相对 root 的路径列表。"""
    index = {}
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
        for name in filenames:
            if name.endswith(EXTS):
                rel = os.path.relpath(os.path.join(dirpath, name), root).replace(os.sep, "/")
                index.setdefault(name, []).append(rel)
    return index


def doc_files(root, docs):
    """解析 --doc：先按 root 相对，再试 root/qingshan 相对。

    这样同一个 root 既能从 qingshan/ 里跑，也能从 projects/ 跑（跨仓索引模式）。
    """
    out = []
    for spec in docs:
        for candidate in (os.path.join(root, spec), os.path.join(root, "qingshan", spec)):
            if os.path.isfile(candidate):
                out.append(candidate)
                break
            if os.path.isdir(candidate):
                for dirpath, dirnames, filenames in os.walk(candidate):
                    dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
                    out += [os.path.join(dirpath, f) for f in filenames if f.endswith(EXTS)]
                break
        else:
            print("警告：--doc 路径不存在，已跳过：%s" % spec, file=sys.stderr)
    return out


def read(path):
    with open(path, encoding="utf-8", errors="replace") as fh:
        return fh.read()


def nlines(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as fh:
            return sum(1 for _ in fh)
    except OSError:
        return -1


def classify(cited, line_hi, index, root, linecache):
    """返回 (状态, resolved, 说明)。

    规则：带斜杠的引用声称了一个路径，必须以仓库根一级前缀开头才算锚定；
    不带斜杠的裸文件名不声称前缀，只按同名文件多少判 OK / AMBIG。
    """
    base = cited.split("/")[-1]
    cands = index.get(base, [])
    if not cands:
        return "NOT-FOUND", "", "没有任何同名文件（路径多半不存在）"
    tail = [c for c in cands if c == cited or c.endswith("/" + cited)]
    if "/" not in cited:
        status, rel = ("AMBIG" if len(cands) > 1 else "OK"), cands[0]
    elif not (cited.split("/")[0] in KNOWN_PREFIXES):
        hint = "候选：%s" % "|".join(sorted(tail)[:3]) if tail else "前缀写错（无路径以此为尾）"
        return "MISSING-PATH", (tail[0] if tail else ""), hint
    elif not tail:
        return "MISSING-PATH", "", "锚定前缀对但路径对不上；同名候选：%s" % "|".join(sorted(cands)[:3])
    else:
        status, rel = ("AMBIG" if len(tail) > 1 else "OK"), tail[0]
    if rel not in linecache:
        linecache[rel] = nlines(os.path.join(root, rel))
    n = linecache[rel]
    if n < 0:
        return "UNREADABLE", rel, "读不到该文件"
    if line_hi > n:
        return "LINE-OOB", rel, "文件只有 %d 行，引用到 %d" % (n, line_hi)
    return status, rel, ""


def main():
    ap = argparse.ArgumentParser(description="审计 `文件:行号` 引用")
    ap.add_argument("--root", default=".", help="索引与文档的根（默认当前目录；跨仓核验请在 projects/ 下跑）")
    ap.add_argument("--doc", action="append", default=None, help="要扫描的文件或目录，可重复；默认 %s" % DEFAULT_DOCS)
    args = ap.parse_args()

    root = os.path.abspath(args.root)
    docs = doc_files(root, args.doc or DEFAULT_DOCS)
    if not docs:
        print("错误：没有任何可扫描的文档（--root/--doc 给错？），拒绝给出空结论。", file=sys.stderr)
        return 2

    index = build_index(root)
    linecache = {}
    found = {}
    for path in docs:
        for m in CITE_RE.finditer(read(path)):
            cited = "%s.%s" % (m.group(1), m.group(2))
            a, b = int(m.group(3)), m.group(4)
            hi = int(b) if b else a
            rel = os.path.relpath(path, root).replace(os.sep, "/")
            found.setdefault((cited, hi), set()).add(rel)

    if not found:
        print("错误：扫了 %d 个文件却没找到任何引用，命令本身可能没在干活。" % len(docs), file=sys.stderr)
        return 2

    rows = []
    for (cited, hi), docs_seen in found.items():
        status, resolved, hint = classify(cited, hi, index, root, linecache)
        rows.append((status, cited, hi, sorted(docs_seen), resolved, hint))

    counts = Counter(r[0] for r in rows)
    print("引用条数：%d（来自 %d 个文件，root=%s）" % (len(rows), len(docs), root))
    for key in ("OK", "AMBIG", "MISSING-PATH", "LINE-OOB", "NOT-FOUND", "UNREADABLE"):
        if counts.get(key):
            print("  %-13s %d" % (key, counts[key]))
    print()
    for row in sorted(rows):
        if row[0] not in ("OK", "AMBIG"):
            print("%-12s %s:%d | 出处：%s | %s | %s" % (row[0], row[1], row[2], ",".join(row[3]), row[4], row[5]))
    if counts.get("AMBIG"):
        print("\n注：AMBIG 只说明存在多份同名文件，不说明引用写错（见头部「已知局限」1）。")
    return 0 if not (counts.get("MISSING-PATH") or counts.get("LINE-OOB") or counts.get("NOT-FOUND") or counts.get("UNREADABLE")) else 1


if __name__ == "__main__":
    sys.exit(main())
