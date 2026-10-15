#!/usr/bin/env python3
"""静态校验 debug APK：ABI 目录、每个 .so 的 JNI 导出、assets 词库、清单里的安装标识。

不装机，所以只证明「包内容对」，不证明「键盘起得来」。设备相关的验收项见 README 的
「未在设备上验证」清单。

用法（在 engine/ 目录）：
    python -X utf8 tools/verify_apk.py <apk 路径> <llvm-nm 路径> <aapt 路径>
三个路径都用 find 取或从 README 抄，不要凭记忆敲。任一项不符即退出码 1。
"""
import pathlib
import re
import subprocess
import sys
import tempfile
import zipfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import verify_symbols  # noqa: E402  复用 Java 侧 102 个声明的解析

LIB = "libmsime_android.so"
ABIS = ("arm64-v8a", "x86_64")
DICT_BYTES = 2_053_612


def expected_version():
    """版本号只认 host/version.txt 这一处，与 app/build.gradle.kts 的公式同形。

    写死 0.1.0/1000 的版本每发一次包就会误报一次清单不符。
    """
    text = (pathlib.Path(__file__).resolve().parents[2] / "host" / "version.txt").read_text().strip()
    major, minor, patch = (int(part) for part in text.split("."))
    return str(major * 1_000_000 + minor * 1_000 + patch), text


EXPECT_CODE, EXPECT_NAME = expected_version()
# aapt dump badging 的真实字段名：minSdk 打的是 sdkVersion:，不是 minSdkVersion。
# 每条 = (说明, 整行正则, 捕获组 1 的期望值)
BADGES = (
    ("包名", r"package: name='([^']+)'", "app.qingshan.ime"),
    ("minSdk", r"^sdkVersion:'([^']+)'", "28"),
    ("targetSdk", r"^targetSdkVersion:'([^']+)'", "35"),
    ("versionCode", r"package: name='[^']*' versionCode='([^']*)'", EXPECT_CODE),
    ("versionName", r"package: name='[^']*' versionCode='[^']*' versionName='([^']*)'", EXPECT_NAME),
)
# 清单里必须出现的 IME 入口要素：服务类、系统权限、Intent 动作、独立的 :ime 进程。
MANIFEST_NEEDLES = (
    "app.msime.android.MSIMEInputService",
    "android.permission.BIND_INPUT_METHOD",
    "android.view.InputMethod",
    "android:process",
)


def check_apk(archive):
    names = set(archive.namelist())
    ok = True
    for abi in ABIS:
        entry = "lib/%s/%s" % (abi, LIB)
        info = archive.getinfo(entry) if entry in names else None
        if info is None:
            print("  ✗ 缺 %s" % entry)
            ok = False
            continue
        print("  %-14s 打包 %9d B" % (abi, info.file_size))
    for entry, size in (("assets/dictionary/dict.tsv", DICT_BYTES), ("assets/desktop-dictionary.lock.json", None)):
        info = archive.getinfo(entry) if entry in names else None
        if info is None:
            print("  ✗ 缺 %s" % entry)
            ok = False
        elif size is not None and info.file_size != size:
            print("  ✗ %s = %d B，期望 %d B" % (entry, info.file_size, size))
            ok = False
        else:
            print("  %-40s %9d B" % (entry, info.file_size))
    return ok


def check_so(nm, archive, abi, tmpdir, want):
    entry = "lib/%s/%s" % (abi, LIB)
    raw = archive.read(entry)
    so = tmpdir / ("%s_%s" % (abi, LIB))
    so.write_bytes(raw)
    have = verify_symbols.exported(nm, so)
    jni = {s for s in have if s.startswith(verify_symbols.PREFIX)}
    missing = sorted(want - have)
    extra = sorted(jni - want)
    note = "导出 JNI=%d 缺失=%d 多余=%d" % (len(jni), len(missing), len(extra))
    print("  %-14s %10d B（解压后）  %s" % (abi, len(raw), note))
    for m in missing:
        print("     缺失: %s" % m[len(verify_symbols.PREFIX):])
    for e in extra:
        print("     多余: %s" % e)
    if "JNI_OnLoad" in have:
        print("     ✗ 出现了 JNI_OnLoad：上游靠命名约定查找，不该有它")
        return False
    return not missing and not extra


def check_manifest(aapt, apk):
    proc = subprocess.run([aapt, "dump", "badging", str(apk)], capture_output=True, text=True, encoding="utf-8")
    if proc.returncode != 0:
        raise SystemExit("aapt dump badging 退出码 %d: %s" % (proc.returncode, proc.stderr.strip()))
    text = proc.stdout
    ok = True
    for label, pattern, value in BADGES:
        match = re.search(pattern, text, re.MULTILINE)
        found = match.group(1) if match else None
        if found != value:
            print("  ✗ %s 期望 %s，APK 里是 %r" % (label, value, found))
            ok = False
        else:
            print("  ✓ %s = %s" % (label, value))
    xml = subprocess.run([aapt, "dump", "xmltree", str(apk), "AndroidManifest.xml"], capture_output=True, text=True, encoding="utf-8")
    if xml.returncode != 0:
        raise SystemExit("aapt dump xmltree 退出码 %d: %s" % (xml.returncode, xml.stderr.strip()))
    for needle in MANIFEST_NEEDLES:
        if needle in xml.stdout:
            print("  ✓ 清单含 %s" % needle)
        else:
            print("  ✗ 清单缺 %s" % needle)
            ok = False
    return ok


def main():
    if len(sys.argv) != 4:
        raise SystemExit(__doc__)
    apk, nm, aapt = map(pathlib.Path, sys.argv[1:4])
    if not apk.is_file():
        raise SystemExit("APK 不存在: %s" % apk)
    want = verify_symbols.expected()
    print("NativeClient.java native 声明 = %d" % len(want))
    print("APK = %s (%d B)" % (apk, apk.stat().st_size))
    with tempfile.TemporaryDirectory() as td:
        tmpdir = pathlib.Path(td)
        with zipfile.ZipFile(apk) as archive:
            print("\n[包内容]")
            content_ok = check_apk(archive)
            print("\n[每个 ABI 的 JNI 导出]")
            so_ok = all(
                check_so(nm, archive, abi, tmpdir, want) for abi in ABIS
                if "lib/%s/%s" % (abi, LIB) in set(archive.namelist())
            )
        print("\n[清单]")
        manifest_ok = check_manifest(aapt, apk)
    print("\n结论: 包内容=%s 符号=%s 清单=%s" % (content_ok, so_ok, manifest_ok))
    sys.exit(0 if (content_ok and so_ok and manifest_ok) else 1)


if __name__ == "__main__":
    main()
