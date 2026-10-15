#!/usr/bin/env python3
"""核对 jniLibs 里每个 .so 的 JNI 导出与 NativeClient.java 的 native 声明是否一一对应。

少一个符号 = Java 侧 UnsatisfiedLinkError。多出来的符号无害，但仍列出来。

用法（在 engine/ 目录）：
    python -X utf8 tools/verify_symbols.py "<llvm-nm 绝对路径>"
llvm-nm 路径用 find 从 NDK 里取，不要手敲。
"""
import pathlib
import re
import subprocess
import sys

JAVA = pathlib.Path(__file__).resolve().parents[2] / "host" / "java" / "app" / "msime" / "android" / "core" / "NativeClient.java"
JNI_LIBS = pathlib.Path(__file__).resolve().parents[1] / "jniLibs"
PREFIX = "Java_app_msime_android_NativeClient_"


def expected():
    text = JAVA.read_text(encoding="utf-8")
    names = re.findall(r"private static native\s+[A-Za-z0-9_\[\]]+\s+([A-Za-z0-9_]+)\s*\(", text)
    assert names, "NativeClient.java 里一个 native 都没解析到，正则或路径错了"
    dupes = {n for n in names if names.count(n) > 1}
    assert not dupes, "Java 声明里有重名方法: %s" % sorted(dupes)
    return {"%s%s" % (PREFIX, n) for n in names}


def exported(nm, so):
    proc = subprocess.run([nm, "--dynamic", "--defined-only", str(so)], capture_output=True, text=True, encoding="utf-8")
    if proc.returncode != 0:
        raise SystemExit("llvm-nm 退出码 %d: %s" % (proc.returncode, proc.stderr.strip()))
    symbols = set()
    for line in proc.stdout.splitlines():
        parts = line.split()
        if len(parts) >= 3:
            symbols.add(parts[-1])
    assert symbols, "%s 的动态导出为空——llvm-nm 没跑对，不是产物没问题" % so
    return symbols


def main():
    if len(sys.argv) != 2:
        raise SystemExit("用法: verify_symbols.py <llvm-nm 路径>")
    nm = sys.argv[1]
    want = expected()
    print("NativeClient.java native 声明 = %d" % len(want))
    libs = sorted(JNI_LIBS.glob("*/*.so"))
    if not libs:
        raise SystemExit("jniLibs 下没有 .so，先跑 cargo ndk build")
    rc = 0
    for so in libs:
        have = exported(nm, so)
        jni = {s for s in have if s.startswith(PREFIX)}
        extra = sorted(jni - want)
        missing = sorted(want - have)
        size = so.stat().st_size
        print("%-28s %10d B  导出 JNI=%d  缺失=%d  多余=%d" % (so.parent.name + "/" + so.name, size, len(jni), len(missing), len(extra)))
        if missing:
            rc = 1
            for m in missing:
                print("   缺失: %s" % m[len(PREFIX):])
        if extra:
            rc = 1
            for e in extra:
                print("   多余: %s" % e)
        if not missing and not extra:
            print("   双向差集均为空 ✔")
    sys.exit(rc)


if __name__ == "__main__":
    main()
