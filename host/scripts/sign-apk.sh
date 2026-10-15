#!/usr/bin/env bash
# 青杉 Android 发布包的事后签名：对齐 → 签名 → 验签 → 复核对齐。
#   用法：bash host/scripts/sign-apk.sh <未签名 APK> <输出 APK>
#   密钥只在仓库外，两条取一：
#     QINGSHAN_RELEASE_KEYSTORE        密钥库路径（PKCS12）
#     QINGSHAN_RELEASE_KEYSTORE_PASSWORD  口令；不给则读 <密钥库>.password 的首行
# 缺任一条就直接失败，不退到调试密钥：Android 只允许同证书覆盖安装，悄悄换一把
# 等于让已装机的人收不到更新（同上游 platforms/android/scripts/signing.sh 的裁定）。
set -euo pipefail

unsigned=${1:-}
output=${2:-}
[[ -n "$unsigned" && -n "$output" ]] || { echo "usage: sign-apk.sh <unsigned-apk> <output-apk>" >&2; exit 2; }
[[ -f "$unsigned" ]] || { echo "no such unsigned APK: $unsigned" >&2; exit 2; }

keystore=${QINGSHAN_RELEASE_KEYSTORE:-$HOME/.android/qingshan-release.p12}
[[ -f "$keystore" ]] || { echo "keystore not found: $keystore (set QINGSHAN_RELEASE_KEYSTORE)" >&2; exit 1; }
if [[ -z "${QINGSHAN_RELEASE_KEYSTORE_PASSWORD:-}" ]]; then
  [[ -f "$keystore.password" ]] || { echo "$keystore needs its password in QINGSHAN_RELEASE_KEYSTORE_PASSWORD or in $keystore.password" >&2; exit 1; }
  QINGSHAN_RELEASE_KEYSTORE_PASSWORD=$(head -n 1 "$keystore.password")
fi
export QINGSHAN_RELEASE_KEYSTORE_PASSWORD

build_tools=${QINGSHAN_ANDROID_BUILD_TOOLS:-${ANDROID_SDK_ROOT:-/d/toolchains/android-sdk}/build-tools/35.0.0}
zipalign=$build_tools/zipalign.exe
apksigner=$build_tools/apksigner.bat
[[ -x "$apksigner" || -f "$apksigner" ]] || { echo "apksigner not found under $build_tools" >&2; exit 1; }
[[ -f "$zipalign" ]] || { echo "zipalign not found under $build_tools" >&2; exit 1; }

# JVM 认不出 /d/... 这种 MSYS 路径，参数要先换成 Windows 形（脚本自身两边都用换算后的值）。
winpath() { if command -v cygpath >/dev/null; then cygpath -w "$1"; else echo "$1"; fi; }

aligned=$(dirname "$output")/$(basename "$output").aligned
# 顺序不能反：对已签名的包再 zipalign 会剥掉 v2/v3 块，apksigner verify 直接 DOES NOT VERIFY
# （PIN 环境陷阱第 40 条，本轮实测）。签名后的 zipalign 只准带 -c 做核对。
"$zipalign" -f -P 16 4 "$(winpath "$unsigned")" "$(winpath "$aligned")"
"$apksigner" sign --ks "$(winpath "$keystore")" --ks-key-alias "${QINGSHAN_RELEASE_KEY_ALIAS:-qingshan-release}" \
  --ks-pass env:QINGSHAN_RELEASE_KEYSTORE_PASSWORD \
  --key-pass env:QINGSHAN_RELEASE_KEYSTORE_PASSWORD \
  --out "$(winpath "$output")" "$(winpath "$aligned")"
"$apksigner" verify --verbose --print-certs "$(winpath "$output")"
"$zipalign" -c -P 16 4 "$(winpath "$output")"
rm -f "$aligned"
# apksigner 会在输出旁边留一个同名 .idsig（v4 增量安装用），它不是发布产物，别跟着上传。
rm -f "$(winpath "$output").idsig" 2>/dev/null || true
ls -l "$(winpath "$output")"
echo "signed: $output"
