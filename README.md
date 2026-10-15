# 青杉输入法（Qingshan IME）

水杉输入法（MSIME）的 Android 原生宿主 + 青简输入法（Qingjian）的 Rust Core。
以 GPL-3.0-or-later 分发，两个上游的许可证原文分别是 `LICENSE`（水杉，GPL-3.0-only，
其 `README.md:136` 明确带 `-only`）与 `LICENSE-QINGJIAN`（青简，GPL-3.0-or-later），
来源固定依据见 `third-party/PIN.md`，分发声明见 `NOTICE.md`。

## 架构：假适配器

水杉的 Java 宿主**基本不改**：394 个 `.java` 里只有 `MSIMEInputService.java` 一个文件不同，
共 **4 行**（2 行品牌字提示 + 2026-10-15 起新增的 2 行「把包名写进建会话选项」，见
`NOTICE.md` §5a 第 1 条）。做法是新建一个 Rust cdylib，直接以 JNI 命名约定导出
水杉原本由 C++ 胶水层（`platforms/android/native/client_jni.cpp`）提供的那 102 个
`Java_app_msime_android_NativeClient_*` 符号，并冒充水杉的 `libmsime_android.so`：

```
MSIMEInputService.java (8146 行；上游 8144 行 + 本项目 4 行改动中的 2 行新增)
  → NativeClient.java (102 个 native 方法，未改动)
    → System.loadLibrary("msime_android")   ← 这一层由 Rust cdylib 顶替
      → engine/ (crate qingshan-engine，lib name = msime_android)
        → qingjian-core 的 Engine::push / query / commit
```

水杉的 `input-runtime`、`engine-bridge`、C++ `MSIME-Engine` 全部不参与。真实现的是 17 个
（实测：`engine/tools/gen_stub.py:19` 的 `REAL` 集合 17 条，`engine/src/stub.rs` 里
`Java_app_msime_android_NativeClient_` 出现 85 次，17 + 85 = 102；本节此前写「16 个」是
`refreshHostRaw` 转真实现后没跟着改的旧数，2026-10-15 按实测改正。输入路径 8 个，加
`setEnglishModeRaw`——宿主的启动路径会把它的返回值当 View 硬解析，
留桩就会让会话建不起来，见 `third-party/PIN.md` 的「启动路径的击穿判定」；候选长按四件
`pinCandidate` / `fixCandidatePosition` / `clearCandidatePosition` / `selectEdge` 回
「原样 View + `handled:false`」止血；`removeCandidate` 接到 Core 的 `Engine::forget`；
展开面板 `allCandidates` 与它唯一的写入口 `selectAnyCandidate` 成对实现；`refreshHostRaw`
比随包锁里那几个文件的长度，让升级安装包能重新解包），其余 85 个符号
返回 `{"ok":false,"error":"not_implemented"}`，靠水杉自身的降级分支
（`fail()` → 「输入连接失败：仅直接输入」）保住键盘可用。

两条硬约束来自水杉 Java 侧的严格 JSON 解析，改动适配器时必须满足：

- `apply()` 要求信封 `{"ok":true,"value":{...}}`，`value.view` 必须存在，
  `view.editing_text` 用 `getString` 硬取，`handled` 必须是真正的 JSON boolean。
- 同类方法的 payload 形状不一致：`create`/`setEnglishMode` 等返回的 `value` 直接是
  `View`，而 `character`/`command`/`focus`/`select` 返回的是
  `Transition{handled,commit,view}`。逐方法按调用点确定，见 `engine/src/wire.rs`。

`prepareHost` 必须真实现：`Bootstrap.java:48` 拿到 `ok!=true` 就抛异常，
`FirstRunPreparation.java:69` 会把状态置为 FAILED，于是永远不生成
`runtime-options.json`，`session` 恒为 0，键盘退化成纯直接输入。

## 构建前提

**先取源码，再取数据。** 本仓库（GPL-3.0-or-later，含 `engine/` 适配层、`host/` 宿主树与 Gradle 工程、
两份 `LICENSE`、`NOTICE.md`）的公开地址是
**⚠ 待填：`https://github.com/<待填>/<仓库名>.git`**（2026-10-15 用户拍板"稍后自己填"，
push 之前这一行不算一个能被第三方取到的位置，见 `NOTICE.md` §5 与 PIN 待办第 19 条）。
两个上游要单独 clone 到与本目录平级的位置，构建与词库重建都按路径引用它们：

```bash
git clone https://github.com/romantic404TT/qingjian.git ../qingjian
git -C ../qingjian checkout 4a3e963dfe299126574bfe3c370ff70a3f85ee77   # 本项目固定的 HEAD
# 水杉宿主：https://github.com/metasequoiaime/msime 的 develop 分支
# 本地这份是 zip 归档（无 .git、无 commit 号），固定依据 = MD5 cfdab2624a8e6469b588984477f1ed65 / 29,461,249 B
```

**两份随包数据不入库**（`.gitignore` 的 `host/assets/dictionary/*.tsv`），clone 后要自己生成，
否则 `Bootstrap` 解包与 `refreshHostRaw` 的锁校验都会失败：

```bash
cp ../qingjian/assets/lexicon/dict.tsv host/assets/dictionary/dict.tsv   # 实测与随包那份逐字节相同
cd engine && python -X utf8 tools/build_glossary_subset.py --out ../host/assets/dictionary/glossary-en.tsv
# 上表顺手重写 host/assets/desktop-dictionary.lock.json 的 bytes；锁被 include_str! 烘进 .so，
# 所以这一步之后必须重跑下面第 2 步（cargo ndk），否则二进制里的锁是旧的。
```

依据是许可证文本 `LICENSE:59`「可自动重建的不必随源码给」；两条重建路径的**逐字节 `cmp` 证据**
与完整说明在 `NOTICE.md` §5「不随源码分发的那两份数据」。**本仓库不含签名私钥，也不该含**
（`*.p12`/`*.keystore`/`*.jks`/`*.password` 全在 `.gitignore` 里）。

工具链全部放在 `D:\toolchains`，由 `D:\toolchains\env.sh` 汇总。

**必须用 Git Bash，并且先 source 再跑 gradle/cargo**：

```bash
. /d/toolchains/env.sh
```

原因有两个：系统里的 JDK 是 25，而 Gradle 8.14.3 不支持运行在其上，`env.sh` 把
`JAVA_HOME` 指到 Temurin 17；另外 Git Bash 自带 `/usr/bin/link.exe`（coreutils 的 `link`
命令）会抢占 MSVC 链接器，`env.sh` 会把 MSVC 的 `Hostx64\x64` 排到 PATH 最前面。
不 source 就会看到 `link: extra operand` 这类误导性报错。

已核实版本：Rust 1.96.0（+ `aarch64-linux-android`、`x86_64-linux-android` 两个 Android target）、
cargo-ndk 4.1.2、MSVC 14.44.35207、NDK 28.2.13676358（钉死，AGP 的 llvm-strip 也用它）、
compileSdk 36 / build-tools 35.0.0 / minSdk 28 / targetSdk 35。

`env.sh` 里设了 `RUSTUP_AUTO_INSTALL=0`：水杉仓的 `rust-toolchain.toml` 钉的是 1.97.1，
误在其目录里跑 `rustc` 会让 rustup 静默下载整套工具链（实测 +700 MB），关掉后变成响亮报错。

## 构建步骤

```bash
. /d/toolchains/env.sh

# 1) 宿主侧测试（host 三元组，不是设备）
cd engine && cargo test

# 2) 交叉编译两条 ABI → engine/jniLibs/<abi>/libmsime_android.so
#    **改词库必须重跑这一步**：随包锁 `host/assets/desktop-dictionary.lock.json` 是用
#    `include_str!` 烘进 `.so` 的（`engine/src/resources_lock.rs`），只重打 APK 不重编 `.so`
#    的话，二进制里的锁还是旧的 —— 2026-10-13 真机实测过这个后果：升级后既不重解包、
#    也不报错，静默留着旧表。
cargo ndk -t arm64-v8a -t x86_64 -o jniLibs build --release

# 3) 核对 102 个 JNI 导出（Java 声明 ↔ 每个 .so 的双向差集）
python -X utf8 tools/verify_symbols.py "<llvm-nm 绝对路径>"

# 4) 出 debug APK。用 clean：增量重打包的体积不可信（见下表）
cd ../host/gradle-app && ./gradlew clean assembleDebug

# 5) APK 静态校验：包内 .so 与符号 / 词库大小 / 清单标识
cd ../../engine
python -X utf8 tools/verify_apk.py "<app-debug.apk>" "<llvm-nm>" "<aapt>"

# 6) 注释与文档里的 `文件:行号` 引用审计（从 projects/ 跑，才同时命中水杉与青简的真身）
cd ../.. && python -X utf8 qingshan/engine/tools/audit_citations.py

# 7) 改动 native 导出后：看会不会新增「杀会话」的调用点（在 engine/ 里跑）
cd qingshan/engine && python -X utf8 tools/scan_bridges.py --summary

# 8) 真机装机后：日志桥的三条自证行 + 耗时（改了 engine/src/logcat.rs 必须走这步，
#    cargo test 编不到 cfg(target_os = "android") 那半边）
adb logcat -s qingshan

# 9) 发版才做：出未签名 release 包，再用仓外密钥事后签名
#    assembleRelease 实测会跑 lintVitalRelease（NewApi 门禁在发布路径上同样生效），
#    产物名 app-release-unsigned.apk（release 块里没有 signingConfig，签名不进 Gradle）。
#    密钥与口令只在仓库外：QINGSHAN_RELEASE_KEYSTORE / QINGSHAN_RELEASE_KEYSTORE_PASSWORD
#    （或 <密钥库>.password 首行），缺任何一条脚本直接失败，不退到调试密钥。
cd ../host/gradle-app && ./gradlew assembleRelease
bash ../scripts/sign-apk.sh app/build/outputs/apk/release/app-release-unsigned.apk <输出>.apk
#    次序是 zipalign → apksigner sign → apksigner verify → zipalign -c：
#    align 放签名之后会剥掉 v2/v3 块（实测 DOES NOT VERIFY），脚本末尾会删掉 apksigner
#    顺手写的 .idsig。**release 包不是 debuggable，`run-as` 全部失效**，
#    读解包件 / 学习表的判据只能在 debug 包上做（见 PIN「签名链落地」一节）。
```

词库侧只有一条正规生成路径：`python -X utf8 tools/build_glossary_subset.py --out ../host/assets/dictionary/glossary-en.tsv`
**一次同时写出随包表和 `desktop-dictionary.lock.json` 的 `bytes`**（锁里列的文件字节数一律按磁盘实际值量，
对不上脚本就报错不写）。手写表或手改锁都不算数——`resources_lock.rs` 的
`packaged_lock_matches_shipped_assets` 单测会在两者脱节时立刻红。

`llvm-nm` 在 `$MSIME_ANDROID_NDK/toolchains/llvm/prebuilt/windows-x86_64/bin/`，
`aapt` 在 `$ANDROID_SDK_ROOT/build-tools/35.0.0/`；两者都用 `find` 取全路径再传进脚本，
不要凭记忆敲（PIN「环境陷阱」第 7 条就是这么栽的）。`apksigner` 是 `.bat`，直接
`java -jar build-tools/35.0.0/lib/apksigner.jar verify <apk>`，但要先 `. /d/toolchains/env.sh`，
否则 `$JAVA_HOME` 是空的，命令塌成 `/bin/java`。

Gradle 从 `host/assets` 取数据、从 `engine/jniLibs` 取 `.so`。
`host/assets/dictionary/dict.tsv`（2,053,612 字节，来自青简 `assets/lexicon/`）与
`host/assets/dictionary/glossary-en.tsv`（2,167,528 字节 / 89,828 数据行，**现值 2026-10-14 18:10：
2,189,354 字节 / 90,317 数据行**，A3 第 5 批三合一落表后；由
`engine/tools/build_glossary_subset.py --top-n 93236` 从青简 `assets/glossary/glossary-en.tsv`
按「词库全部 93,236 行 ∩ 全量译词表」裁出，
再追加 `engine/tools/glossary-sentences-zh-en.tsv` 的 1,479 条 A3 整句 ⇒ 89,828 行（**现值 1,968 条 ⇒ 90,317 行**）；2026-10-13 A1 之前是「按词频前 80k」的
1,815,026 字节 / 76,273 行。命令里的 93236 就是词库带词频的行数，改词库要跟着改）
与 `host/assets/desktop-dictionary.lock.json`（167 字节，`name` + `bytes`）一起满足水杉 `Bootstrap` 的解包校验，
这样 Java 侧无需任何改动。**随包资源必须放在 `host/assets/dictionary/` 下**：
`Bootstrap.java:259` 把资产路径硬拼成 `"dictionary/" + name`，而 `:278` 的
`[A-Za-z0-9_.-]+` 又不允许锁里的名字带目录——换个目录名就会 `FileNotFoundException`，
整个 Bootstrap 报准备失败（连词库一起解不出来）。
**这份译词已接进 `Engine` 并在真机显示**：`assemble.rs` 的 `create_engine` 用
`Glossary::from_path` 装 `with_translator`，`session.rs` 的 `requery()` 在 `refresh` 之前
`annotate`，`view.rs` 的 `translation_to_string` 把释义拼成单行送进 `Candidate.translation`。
打 `nihao` 时首页 10 格里 5 格带英文释义（`你好` → `int. hello · int. hi`）。
**但只对新装或清数据的设备生效**，见「已知失效项」第 8 条。

## 产物实测（2026-10-13 重测，host x86_64 上交叉编译，非设备数据）

| 项 | 实测值 |
| --- | --- |
| `x86_64-linux-android` 标准库 | 145,284,914 B（rustup 组件目录，与 arm64 那份 134,033,019 B 同量级） |
| `cargo ndk` 两 ABI release | 首次 28.95 s（arm64 命中缓存），0 条 warning |
| `jniLibs/arm64-v8a/libmsime_android.so` | 4,341,128 B，ELF64 / DYN / AArch64（日志桥之前 4,254,440 B ⇒ **+86,688 B**；其中 C 第 4 项把 `qingjian-translate` 真链进来就占 **+38,000 B**，见下方「加了依赖 ≠ 依赖被链接进来」） |
| `jniLibs/x86_64/libmsime_android.so` | 4,583,272 B，ELF64 / DYN / X86-64（之前 4,494,504 B ⇒ **+88,768 B**，C 第 4 项单步 **+41,632 B**） |
| 动态符号 | 两个 `.so` 各 102 个 `Java_app_msime_android_NativeClient_*`，与 Java 声明双向差集为空；`JNI_OnLoad` 0 个 |
| 多出来的导出 | `opencc_convert/create/destroy/free_string` 4 个（青简依赖带进来的，JNI 按 `Java_` 前缀查找，不受影响） |
| `NEEDED` | `libdl.so` / `libm.so` / `libc.so` + **`liblog.so`**（日志桥直连 `__android_log_write` 带进来的，唯一新增依赖），无 SONAME |
| 首次 `assembleDebug`（拉依赖） | BUILD SUCCESSFUL in 3m 33s，37 个 task，`caches/modules-2` 落盘 144,305,065 B |
| `clean assembleDebug` | BUILD SUCCESSFUL in 52s，38 个 task |
| `app-debug.apk` | **49,897,529 B**（clean；**A3 第 7 批落表 63 条 +1,460 B**：表 2,189,354 → 2,192,288 B、`.so` 磁盘大小不变（两版锁同为 7 位十进制，所以烘进二进制的锁换了但长度没变）；上一版 **49,896,069 B** = 方案甲-2 落地后；再上一版 49,846,673 B = **方案 A 那轮 +280 B**：只有 `.so` 里的 Rust 代码变了 （`keys.rs` 命令 0 + 两条单测），词库与锁都没动；`.so` 磁盘大小 arm64 4,351,520 → 4,351,792 B、 x86_64 4,597,720 → 4,598,000 B。上一轮 **flush_learning +2,488 B**：只有 `.so` 里的 Rust 代码变了，词库与锁都没动。上一轮 **A3 第 4 批 +8,152 B**：只有随包表变了， 2,150,622 → 2,167,528 B，包内 deflate 961,150 → 969,300 B（+8,150 B），`.so` 与锁字节数没变 （锁内容变了但长度只差 1 B，`.so` 里烘的仍是 4,349,376 / 4,595,232 B）。 上一轮 **refreshHostRaw +28,372 B**：表未变，涨在 代码与锁——`.so` 磁盘大小 arm64 4,341,128 → 4,349,376（+8,248 B）、x86_64 4,583,272 → 4,595,232（+11,960 B），锁 87 → 167 B（+80 B）；三项合计 20,288 B，余下约 8 KB 是 zip 对齐与其它条目，**没逐项拆**。全部同口径 `ls -l` 相减。A1 之前是 49,659,177 B，译词表补到词库∩全量 88,349 行 ⇒ **+130,640 B**；A3 第 1 批 165 条 ⇒ +3,244 B、第 2 批 303 条 ⇒ +5,348 B、第 3 批 491 条 ⇒ +8,972 B，三批合计 +148,204 B。全部来自 `glossary-en.tsv` 的 deflate 后增量：812,946 → 943,589 → 946,832 → 952,181 → 961,150 B。再往前：只加 80k 译词子集时 49,584,033 B；C 第 4 项把译词表真链进镜像再 **+74,400 B**。未带译词前 48,770,926 B ⇒ 合计 **+1,036,455 B**）。曾观测到不带 clean 的增量重打包给 53,171,668 B，与上一次从零打包的 48,674,350 B 条目集合一模一样却多约 4.5 MB（2026-10-13 再撞一次：临时探针包不带 clean 打出 53,318,428 B，其 `.so` 4,303,672 B 只比 clean 版大 1,112 B；还原后 `clean assembleDebug` 逐字节回到 48,770,926 B。序列见 `third-party/PIN.md` 环境陷阱第 10 条）—— 体积一律以 clean 为准 |
| 包内 `.so` | 4,341,120 / 4,583,264 B（`:app:stripDebugDebugSymbols` 各再削 8 B），`compress_type=0` 未压缩存储 |
| **加了依赖 ≠ 依赖被链接进来** | `qingjian-translate` 早就写在 `engine/Cargo.toml:38`，但在 C 第 4 项调用 `Glossary::from_path` 之前，它一个符号都没被引用，链接器整块丢掉——所以「加依赖会不会涨体积」的答案是 0，而「第一次真调用」的答案是 arm64 **+38,000 B** / x86_64 **+41,632 B**。以后估体积增量要按「有没有真符号被引用」判，不按 Cargo 行数判 |
| 包内词库 | `assets/dictionary/dict.tsv` 2,053,612 B（deflate 后 964,523 B）、`assets/dictionary/glossary-en.tsv` 2,167,528 B（deflate 后 969,300 B；**现值 2,189,354 B / deflate 后 980,442 B**，A3 第 5 批落表后，与 APK 增量的 11,140 B 对得上；A1 前 1,815,026 / 812,946 B；**再现值 2026-10-15 18:03：2,192,288 B / 90,380 数据行 / debug 包内 deflate 后 981,868 B**，A3 第 7 批落表 63 条 ⇒ +2,934 B）、`desktop-dictionary.lock.json` 167 B（**现在含 `bytes`**：宿主仍只读 `name`，多出来的长度由我们的 `refreshHostRaw` 用 ⇒ **换表必须重生成锁并重编 `.so`**，见「构建步骤」第 2 步） |
| 别的 ABI 也被打进来了吗 | 没有。`merged_native_libs` 里 ML Kit 有 4 个 ABI（`armeabi-v7a`/`x86`/`arm64-v8a`/`x86_64`），APK 里只有后两个，`abiFilters` 生效 |
| `zipalign -c -v 4` | Verification succesful（退出码 0） |
| `apksigner verify` | 退出码 0，签名者 `CN=Android Debug`。**（现值 2026-10-15 18:20：发布包另有自己的签名者，见下面两行）** |
| 发布包（第一个 release，2026-10-15 18:19–18:20 实测） | 未签名 `app-release-unsigned.apk` **45,966,895 B** → 签名后 **46,070,982 B**（+104,087 B）；`apksigner verify` 退出码 0，**v3 true / v1、v2、v4 false**（minSdk 28 下 apksigner 按 min/max SDK 自动选方案），签名者 `CN=Qingshan IME Release, O=Qingshan`，证书 SHA-256 `8eb90f8dc8c4efc561c3d1743c53b0b2818444bba78d3bbc635f2d77b7627132`，RSA 3072、有效期 10,000 天。比同日 debug 包（49,897,529 B）**小 3,826,547 B**，差额几乎全来自资源压缩率：同一份 `dict.tsv`（CRC 相同）debug 里 deflate 后 964,523 B、release 里 859,681 B |
| 发布包上 `run-as` | **不可用**：`run-as: package not debuggable: app.qingshan.ime`，`ls /data/data/app.qingshan.ime` → `Permission denied`（本机未 root）。`logcat -s qingshan` 仍读得到（会话建立/挂起那几行照旧）。⇒ **凡是要读应用私有文件的判据（解包件字节、`files/qingshan/user*.tsv` 学习表、flush 落盘）只能在 debug 包上做**；release 侧只能用日志口 + UI 口。给 release 加 `debuggable=true` 这条路不走：那是把应用门户敞开换几条日志 |
| 清单（`aapt dump badging`） | `app.qingshan.ime` / minSdk 28 / targetSdk 35 / versionCode 1000 / versionName 0.1.0。**（现值 2026-10-15 18:19：两个包都是 versionCode 2000 / versionName 0.2.0；release 清单里 `TestPasswordActivity` 出现 0 次，debug 清单里有）** |

宿主侧（同一台机器，`cargo test`）：32 个测试全过；词库 2,053,612 B 解析并查询通过，
`nihao` → 候选含「你好」→ `commit` 回「你好」。这些跑的是 host `x86_64-pc-windows-msvc`，
**不等于设备上的 ARM 行为**。

耗时（`cargo test --release -- --nocapture`，修复后重测 4 次采样，host x86_64 的 SSD 与 CPU）：
`Dictionary::from_path` 2,053,612 B 用 **50.01–61.50 ms**，`query("nihao")`
**8.05–11.55 ms** 出 44 个候选。

设备侧同一件事现在能实测了（OPPO PEGT10 / Android 13，`adb logcat -s qingshan`，
三次冷启动各一条）：词库载入 **55 / 58 / 69 ms**（2,053,612 B，与包内字节数一致），
会话建立 **56 / 58 / 72 ms**（含载入）。也就是说宿主那组数字确实是设备量级，
之前挂在未验证清单上的「真机词库首次加载耗时」已划掉。

## 日志：`adb logcat -s qingshan`

Rust 侧自己装了一个 `tracing` subscriber（`engine/src/logcat.rs`，零新依赖：手写
`Subscriber` 的 7 个方法 + `#[link(name = "log")]` 直连 `__android_log_write`），
门限 INFO，所以 `ERROR`/`WARN`/`INFO` 出得来、青简 Core 那 30 处 `debug!` 出不来。
装好时先自证一行，看到它就说明桥在工作：

```
I qingshan: [logcat] 日志桥已装好（级别门限 INFO）
I qingshan: [msime_android::assemble] 词库载入 58 ms / 2053612 B
I qingshan: [msime_android::session] 会话建立 58 ms（含词库载入）
W qingshan: [msime_android::wire] 回 ok:false: create_failed: 词库加载失败: ...
```

桥只挂在 `prepareHostRaw` 与 `createRaw` 两个入口（其余导出必须先有会话）。
宿主 `cargo test` 默认一行都看不到：单元用例不走那两个入口，`init()` 没跑，
`tracing` 仍无 subscriber —— 要看得临时在测试里调一次 `logcat::init()`。

## 四个校验脚本

| 脚本 | 查什么 | 现在的答案 |
| --- | --- | --- |
| `engine/tools/verify_symbols.py` | `NativeClient.java` 的 native 声明 ↔ 每个 `.so` 的动态导出，双向差集 | 102 对 102，两边都空 |
| `engine/tools/verify_apk.py` | APK 里的 `.so`/词库/清单，符号从包里解压出来再数（不是查 `jniLibs`） | 全过，退出码 0 |
| `engine/tools/scan_bridges.py` | 宿主每个 `NativeClient.` 调用点会不会**杀会话**（看 catch 体调不调 `fail()`/`stop()`）。`--summary` 只看计数，`--json` 供未来接 CI | 62 个调用点 = real 23 + 桩 39；桩里 **杀会话 5 / 降级 32 / 不解析 1 / 需人工定性 1**。阶段二每加一个真实现，杀会话数应当只降不升（阶段一收尾实测 12，2026-10-13 实测 5；中间各步的期望值写在脚本末尾的基线段里） |
| `engine/tools/audit_citations.py` | 注释与文档里每条 `文件:行号` 是否还能落到真实行 | 375 条：OK 146 / AMBIG 122 / MISSING-PATH 94 / **LINE-OOB 0** / NOT-FOUND 13（2026-10-15 21:58 实测；本轮 373 → 375 的 +2 **又全部落在 AMBIG**（OK / MISSING-PATH / NOT-FOUND 一条没动）——新引的是 `UpdateJobService.java`、`AboutPage.java`、`ImeLetterRows.java` 这类上游有逐字孪生的宿主文件与 `LICENSE` 的条款行，属既有口径；**LINE-OOB 仍 0**。上一轮 373 条：OK 146 / AMBIG 120 / MISSING-PATH 94 / **LINE-OOB 0** / NOT-FOUND 13（21:05 实测；本轮 368 → 373 的 +5 **全部落在 AMBIG**（OK / MISSING-PATH / NOT-FOUND 三项一条没动）——新引的宿主 Java 与 `LICENSE` 条款行里，那些在上游有逐字孪生的文件脚本只能报"存在多份"，属既有口径；**LINE-OOB 仍 0**。上一轮 368 条：OK 146 / AMBIG 115 / MISSING-PATH 94 / **LINE-OOB 0** / NOT-FOUND 13（19:47 实测，19:48 复跑同数；本轮 366 → 368 的 +2 落在 OK/AMBIG 各一条，**MISSING-PATH 与 NOT-FOUND 的明细一条没动**（与上一轮逐行比对：只有 `qingshan/host/java/app/msime/android/account/UpdateApi.java` 的 45 行、`.../home/AboutPage.java` 的 52 行、`qingshan/host/gradle-app/app/build.gradle.kts` 的 48 行 三条的**出处列表**多了 README，脚本按引用串去重所以计数不动）。新引的行号来自「密码框路径 C 真机验证」「A3 第 7 批落表」「签名链落地」三新节与 README 两条新 bullet。上一轮 366 条：OK 145 / AMBIG 114 / MISSING-PATH 94 / **LINE-OOB 0** / NOT-FOUND 13（2026-10-15 03:35 实测；本轮 354 → 366 的 +12 全来自「A3 第 7 批起草」「签名链与发布准备评估」两新节与 README 三条新 bullet 新引的上游/宿主行号，**OK 与 AMBIG 一条没动**，+12 全落 MISSING-PATH（83 → 94）：这批引用是 `msime-develop/platforms/android/scripts/signing.sh`、`msime-develop/.github/workflows/release-android.yml`、`qingshan/host/java/app/msime/android/account/UpdateApi.java` 这类带目录前缀的完整写法，脚本认的六个前缀里没有 `msime-develop/` 与 `qingshan/`，**属既有口径不是新问题**；NOT-FOUND 12 → 13 的那 1 条是 `env.sh:25`——`/d/toolchains/env.sh` 在扫描根之外，永远认不到，与 `jni-0.22.4`/`prd.md`/`AlertDialog.java` 那 12 条同类。**本轮一度把 LINE-OOB 引出 3 条**：新节里把 `NOTICE.md` 的 24 行、26–28 行与 `build.gradle.kts` 的 48 行**裸写**（不带目录前缀），被对到 msime-develop 的孪生文件（那儿那个 NOTICE.md 只有 5 行）——**本行末尾那条既有教训我又犯了一次**（那条教训本身写着"我又犯了一次"），补齐 `qingshan/` 前缀后当场归零；顺带把 `UpdateApi.java`、`AboutPage.java` 这类在上游有**逐字孪生**的引用改成完整路径（孪生行号目前恰好一致，但那是巧合不是保证，裸写等于赌两份文件永不分叉）。**补一条自指的坑**：本行刚写下"裸写 `NOTICE.md` 24 行会被对到孪生文件"这句话时，那句话里**照抄下来的三个"文件名+冒号+行号"裸引用自己被审计脚本当真了**，复跑一次立刻变 369 条 / LINE-OOB 3——改成"`NOTICE.md` 的 24 行"这种不被解析的写法才归零。**描述错行的句子本身不能沿用错行的写法。**上一轮 354 条：OK 145 / AMBIG 114 / MISSING-PATH 83 / **LINE-OOB 0** / NOT-FOUND 12（2026-10-15 02:40 实测；本轮 352 → 354 的 +2 来自「A3 第 6 批起草」新节与 README 那条新 bullet；起草用的离线判据脚本在 `/d/toolchains/tmp/` 下、不在扫描根里，故不产引用（临时脚本不进仓是刻意的）。上一轮 352 条：OK 144 / AMBIG 113 / MISSING-PATH 83 / LINE-OOB 0 / NOT-FOUND 12，2026-10-15 01:30 实测；本轮 338 → 352 的 +14 全来自「方案甲-2 落地」新节与 README 两条新 bullet 新引的宿主/Core 行号，**LINE-OOB 仍 0、NOT-FOUND 明细与前两轮逐行一致**（12 条全是 `jni-0.22.4/src/env.rs`、`prd.md`、`AlertDialog.java`、`usr/include/android/log.h` 这些扫描根之外的既有项）。上一轮末 338 条：OK 139 / AMBIG 106 / MISSING-PATH 81 / LINE-OOB 0 / NOT-FOUND 12，2026-10-14 23:45 实测；本轮 322 → 338 的 +16 条来自「算不拢测清 + 方案甲卡在『输入框身份』上」新节与 README 两条新 bullet 新引的宿主/Core 行号，NOT-FOUND 明细与上一轮**逐行一致**、LINE-OOB 归零。**那一轮 LINE-OOB 又被引出来过 1 条**：新节里裸写本仓 `engine` 下的那个 `lib.rs` 行号，被对到 msime-develop 里只有 101 行的同名文件——**正是本行末尾那条既有教训，我又犯了一次**，改成本仓全路径后当场归零（代价是落进 MISSING-PATH）。再上一轮 322 条：OK 133 / AMBIG 102 / MISSING-PATH 75 / LINE-OOB 0 / NOT-FOUND 12，2026-10-14 21:10 实测；**这轮从 290 涨到 322 全是 D3 那一节新引的 Core 行号**——`correction/costs.rs:45`、`correction/mod.rs:26/48`、`fuzzy/rules.rs:38`、`typo/kind.rs:7`、`typo/mod.rs:17`、`ranking/mod.rs:33/64`、`sentence/viterbi.rs:319`、`commit/last.rs:5`、`learning/mod.rs:194`、`platform/config/mod.rs:73` 这类**短格式**落进 MISSING-PATH（脚本认的六个前缀里没有 `qingjian/crates/…` 的 crate 根写法），**LINE-OOB 仍 0、NOT-FOUND 明细与前几轮逐行一致**；18:15 那次是 290 / 114 / 101 / 63 / 0 / 12；14:41 那次是 288 / 113 / 101 / 62 / 0 / 12，17:00 是 289 / 113 / 101 / 62→63 / 0 / 12（+1 MISSING-PATH 是修正表测试注释里那条 Core 短格式引用，脚本认的六个前缀里没有它，**属既有口径而非新问题**），最后这次 +1 OK 是本节新写的两条 Core 挂点引用被正常解析到；03:29 是 282 / 110 / 101 / 59 / 0 / 12，13:31 复跑同一组，本轮 C1 那一节与待办新增 6 条引用 ⇒ 计数到 288，MISSING-PATH 59→62 的三条是本仓短格式引青简生成器的那几个文件（`phrases/mod.rs:231`、`fuzzy/rules.rs:106`、`tools/dict-convert/src/lexicon/readings.rs:138`，脚本认的六个前缀里没有 `qingjian/tools/…`），**LINE-OOB 仍 0、NOT-FOUND 明细与上两轮逐行一致**；当年 03:29 那轮「已知失效项」第 9 条新写的几处宿主行号（`apply()` 里那个 handled 判定、硬件键映射里 DEL 归零那条、试用行与九键面板的删字行、上游 macOS 壳的 controller 段）都与文档里已有引用**同串**，脚本按引用串去重所以计数不动，但**逐条 sed 取行核对过**；比本行此前记录的 277 条多 5 条，来自方案 A 那一节新引的宿主与 Core 行号，**LINE-OOB 仍为 0**。**AMBIG 81→99→101 不是回归**：多出来的是宿主 Java 文件（候选菜单、输入服务那两个）与 Core 里重名的 `mod.rs` 这类，它们在 msime-develop 里有同名孪生，脚本只能报"存在多份"（判定口径见本节上方第 1 条局限）。MISSING-PATH 50→59 = 本仓自引加了 `qingshan/` 前缀（脚本认的六个前缀里没有它）+ 青简 Core 的 crate 根短格式。**上一轮 LINE-OOB 真被引出来过 1 条**：裸写 `engine` 下的那个 lib.rs 会被对到 msime-develop 里只有 101 行的同名文件，改成本仓全路径后归零，代价是落进 MISSING-PATH——**引本仓文件一律带 `qingshan/` 前缀**） |

审计脚本按**文件名**锚定，六类情况它自己判不了：写成 `crate/src/x.rs` 这类 cargo 相对前缀的引用
（真身还要再加 `crates/`）、装在 `$CARGO_HOME` 里根本不在扫描根的 `jni-0.22.4/src/env.rs`
与 `tracing-core-0.1.36/src/metadata.rs`、装在 NDK 里的 `usr/include/android/log.h`、
位于扫描根**之上**的 `prd.md`（它在 `qing/`，脚本从 `qing/projects/` 跑）、同名文件，
以及 Android 框架自己的文件（`AlertDialog.java:1137`，来自「已知失效项」第 7 条那段崩溃日志）。
同名文件那一类原先有 6 条被点红，逐条人工定性后补上了可锚定的完整路径，
**LINE-OOB 现在 = 0**。三条上游引用（`crates/host-api/src/ffi/input.rs:184`、
`crates/engine/src/session/input.rs:863` / `:867`）改成以脚本认的 `crates/` 前缀书写后已进 OK；
本仓那条 `pinCandidateRaw` 只能落到 MISSING-PATH（脚本的 `KNOWN_PREFIXES` 不含 `qingshan/`，
而裸 `engine/src/lib.rs` 在 msime-develop 里有同名孪生，写哪种都到不了 OK），
它解析出的文件是对的、行号也在范围内。定性的结论留在这里备查：

- `keys.rs` 里那条 184 行的引用指 `msime-develop/crates/host-api/src/ffi/input.rs`
  （750 行；`:157` 是 `msime_client_command` 签名、`:184` 是 `_ => … unknown input command`，
  内容核过是对的），脚本此前对到只有 127 行的 `crates/engine/src/backend/input.rs`。
- `keys.rs` 里那两条 863 / 867 行的引用指 `msime-develop/crates/engine/src/session/input.rs`
  （1800 行；`:858-863` 是 `Command::Cancel → reset_composition`，`:865-867` 是
  `ConvertHanja / ConversionLeft / ConversionRight → unhandled`，两条都核过是对的）。
  （本行原文把第二条写成 `:869`，与它自己正文里的 `:865-867` 和脚本实测都不符，
  当时按 `grep -n` 与上游文件对盘后改成 `:867`。）
- 「当前状态」B1 那条指我们自己的 `qingshan/engine/src/lib.rs`（不是
  `msime-develop/crates/engine/src/lib.rs`，那份只有 101 行）。同段另三个行号写成裸
  `:NNN`，脚本的正则要求文件名，它们**根本不成条**——所以那一段里只有带文件名的那条被点红，
  不代表其余三行更可信。
- 「未验证清单」第 6 条那两处原先不带目录前缀（裸文件名 + 行号），被脚本对到 27 行的
  `qingjian/apps/windows/server/src/dispatch/status/view.rs` 而报 LINE-OOB；2026-10-13 21:00
  已补上 `engine/src/` 前缀（本行是当时留下的核验记录）：`:154` = `pub const SCHEME_QUANPIN: u8 = 0;`、
  `:194`/`:330` = `local_mode: "none".into(),`，三条都 `sed -n 'Np'` 逐行回读对过。

所以这脚本的用法是「拿它找可疑项，再人工定性」，不是拿来清零的。


## 版本号

从 `0.1.0` 起，对齐青杉自己的发布节奏，不继承水杉的 `0.3.1`。唯一来源是 `host/version.txt`，
`versionCode = 主*1_000_000 + 次*1_000 + 补丁`，因此 0.1.0 → **1000**。水杉的版本号只保留在
`third-party/PIN.md` 里作溯源。构建脚本的内部属性名沿用上游 `-PmsimeVersion` 未改名，
目的是最小化与上游的 diff。

**现值 `0.2.0` → versionCode `2000`**（2026-10-15 用户裁定第一个发布包用这个号；`aapt dump badging`
在 debug 与 release 两个包上都实测到 `versionCode='2000' versionName='0.2.0'`）。
两条同时定下的规矩：**debug 与 release 共用这一份 `host/version.txt`**（不分两套号，
所以 `engine/tools/verify_apk.py` 的期望值改成从这里读，不再写死 0.1.0——写死的话每发一版
就会误报一次「清单不符」）；**`versionCode` 只增不减**，Android 只在版本号更大且证书相同
时才允许覆盖安装。渠道差异只进 tag / 发布名，`versionName` 保持纯 `X.Y.Z`（抄上游的做法）。

## 已知失效项

以下是路线 B（假适配器）的必然代价与刻意取舍，不是待修 bug——除非产品要求恢复，否则不去实现：

1. **其余 85 个非输入 native 方法返回 `{"ok":false,"error":"not_implemented"}`**（102 个总
   数减去真实现的 17 个；计数出处 = `engine/tools/gen_stub.py` 的 `REAL` 集合）：
   主题目录与解析、偏好读写与 revision CAS、云词库、NDJSON 快照、剪贴板共享存储、
   豆包帧编解码、打字统计、离线释义、英文补全、资源包下载安装、诊断包等。它们走水杉自身的
   降级分支失效（`fail()` → 「输入连接失败：仅直接输入」），不崩进程，但相应入口不可用。
   **注意其中 5 个调用点（4 个方法）不是"只是不可用"**：它们的 `catch` 体里调的是 `fail()`，触发即整个会话停掉
   ——`glide`（`MSIMEInputService.java:3145`）、`setNineKeyFilter`（`:3158`）、
   `punctuationWithContext` 两处（`:3250`、`:3273`）、`resetCache`（`:5959`）。
   （阶段一曾真机复现的「点候选条右侧的展开箭头 → `allCandidates` 桩 → 会话死、键盘退回英文」
   已在阶段二 A 步转 real，不在这 5 个里。）
   逐条可达性与优先级见 `third-party/PIN.md`「会话建立后的 stub 降级面」。
2. **Apple 登录必定失败**：`java/app/msime/android/account/AppleWebSignIn.java:31-33` 写死了
   applicationId 白名单（`app.msime.android` 及其 5 个版本后缀），青杉的 `app.qingshan.ime`
   不在其中。要恢复须把新包名加进那份列表。
3. **account / voice / handwriting 三个模块的运行时行为待观察**：它们依赖上面第 1 条里失效的
   native 入口和外部 SDK。ML Kit 手写的 `libdigitalink.so` 确实随依赖进了包
   （arm64-v8a 6,891,488 B / x86_64 8,771,480 B，占 APK 的近三分之一），而语音要的
   `libsherpa-onnx-c-api.so`、`libonnxruntime.so` 本项目根本不打包（最终 APK 里只有 4 个
   `.so`：两个 ABI × `libmsime_android.so` + `libdigitalink.so`）。
   键盘主路径不经过它们，但设置页里的对应入口点了会走失败分支。真实表现未在设备上验证。
4. **界面残留「水杉」品牌字**：Java 源码里有 35 处用户可见的「水杉」硬编码（主题名、云剪贴板、
   账号页、备份页、社区页等），非键盘主路径。原因是路线 B 承诺「Java 零改动」，只对键盘主路径
   的 2 处破例（`MSIMEInputService.java:5242/5249` 的启动失败提示）。将来同步水杉上游时
   需留意这 2 处冲突。
   **2026-10-15 起 Java 侧共有三处破例、共 4 行**：除上面 2 行品牌字外，`：1697-1698` 新增
   1 行注释 + 1 行代码，把已经算好的包名写进建会话选项，作为跨会话撤销的挂起键
   （**不是**品牌字破例，是功能破例；逐行理由见 `NOTICE.md` §5a 改动声明第 1 条）。
   破例数从 2 到 3 的代价：将来同步上游时这一处也要手工重放，且它改的是**建会话请求的内容**，
   比提示文案更容易和上游改动撞车。
5. **宿主树未取上游的两个 Kotlin 插件文件**（`java/app/msime/android/account/AccountPlugin.kt`、
   `java/app/msime/android/voice/VoicePlugin.kt`）：两者仅被
   `java/app/msime/android/policy/AccountSessionRoutingPolicy.java:45` 的 javadoc
   `{@code AccountPlugin}` 提及，不是 `{@link}`、不是代码引用、不构成编译依赖；上游与本项目的
   `plugins` 块都只有 `id("com.android.application")`，没有 Kotlin 插件。
   **3.6 之后 diff 上游树时不要以为这是漏拷**：`java`+`res` 两侧文件数 539（本项目）对
   541（上游），差的正是这两个 `.kt`。

6. **「删除词条」与一切学习数据都只在内存里**：B2 把 `removeCandidateRaw` 接到了青简的
   `Engine::forget`（`crates/qingjian-core/src/engine/learning/mod.rs:152`），但落盘要
   `Engine::flush_learning`，而我们**从没调过它**（`engine/src/` 里 `flush` 零命中；
   接在哪一层的评估见 `third-party/PIN.md`「那 `flush_learning` 该接在哪里」）。
   后果比"进程被杀才丢"更近一步：会话在**离开输入框**时就销毁（`onFinishInput` →
   `stop(true)` → `MSIMEInputService.java:1598` 的 `NativeClient.destroy`），Engine 随之 drop，
   所以删掉的词只在这一次停留在输入框期间有效，切走再回来就复活。
   真机旁证：`files/qingshan/` 从装机到现在是空的，即学习数据一条都没落过盘。
   **本条已于 2026-10-14 00:54 关闭**（上面那段是关闭前的记录，保留原文）：`flush_learning`
   已接成「会话销毁必落 + 上屏满 60 秒节流落」（`engine/src/session.rs` 的
   `flush_at_destroy` / `note_commit`，常量 `LEARNING_FLUSH_INTERVAL = 60 s` 抄上游 Linux 壳）。
   真机证据：三次上屏后目录**仍空**（节流生效），回桌面触发 `onFinishInput` → `destroy` 后
   出现 `user.tsv` 50 B（`你好\t3`）、`user-choices.tsv` 71 B、`user-ngram.tsv` 133 B；
   `am force-stop` 冷启动后再上屏一次并销毁 → `你好` 从 3 涨到 **4**（读回 + 累加都通）；
   节流那条路也在真机自然触发过（`00:51:41 学习数据落盘 0 ms（上屏满 60 秒）`）。
   写盘耗时实测 **0 / 3 / 7 / 9 ms**，crash `FATAL` 0 ⇒ 没有可感知卡顿。
   **仍然存在的缺口**：上屏后立刻 `am force-stop`（不走 Java 回调）会丢这 ≤ 60 秒的数据，
   实测过：那次 `你好` 停在 4 没涨。这是方案 2 的固有边界，不是新 bug。
   **另需说明**：「学习是否改变排序」本轮**没测出来**——把落盘目录挪开做 A/B，
   `nihaonihao` 的候选条两张截图 **sha256 相同、像素差异包围盒 None**，
   个人计数（你好 4 / 你好好 3 / 你好像 2）压不过基础词频（你好 71,960、你好好 4,974、
   你好像 4,541）。数据链路三段（写盘 / 读回 / 累加）已证，排序影响要更大样本或
   先读 Core 的权重公式再设计决定性实验。
   **（现值 2026-10-14 01:56：决定性实验已做，**学习已证改变排序**——同音对 `suoyi`
   所以 155,225 / 蓑衣 48，**1 次上屏**就把蓑衣顶到第 1 格，冷启动后仍是第 1 格；
   负对照（两张学习表一起挪开）会退回所以在前。上面那段「没测出来」的原因是测错了对象：
   `nihaonihao` 的次序由排序键前 4 级（是否打全 / 覆盖字母数 / 简拼数 / 末音节）先决定，
   个人计数所在的第 5、6 级没轮到比。证据与公式见「当前状态」的
   「学习改变排序决定性实验」条与 PIN 同名一节。）**
   （菜单路径 `:6039` 真机不可验，见第 7 条；`:5968` 需硬件组合键——
   `MSIMEInputService.java:3955-3956` 的注释写明两条维护组合键都是 Ctrl+Shift+Alt，
   触屏没有入口。B2 的验证面为静态 + 单测。）
7. 长按候选 → 「删除词条…」会触发宿主 `MSIMEInputService.confirmCandidateRemoval`
   （`:6058`）用 `InputMethodService` 的 window token 调 `AlertDialog.show()`，
   在 ColorOS/Android 13 上抛 `WindowManager$BadTokenException` 崩掉 `:ime` 进程
   （`ImeCandidates.java:148-150` → `:6054-6056`）。**这不是我们的缺陷**，
   阶段一/B1/B2 三版表现一致；`Java 零改动`约束下无法修复。
   **2026-10-13 真机重新确认，并补齐了触发条件**（设备 `e1cdb946`，装机
   `lastUpdateTime=11:31:10`，`:ime` pid 17874）：候选条第一格长按
   `input swipe 104 1701 104 1701 1500` → 菜单五项弹出（`int. hello · int. hi` /
   优先显示 / 固定到首位 / 取消固定 / 删除词条…）→ 点「删除词条…」（行心 y=2278）即崩：

   ```
   11:47:42.560 D StrictMode: at android.app.AlertDialog$Builder.show(AlertDialog.java:1137)
   11:47:42.560 D StrictMode: at app.msime.android.MSIMEInputService.confirmCandidateRemoval(MSIMEInputService.java:6058)
   11:47:42.571 E AndroidRuntime: FATAL EXCEPTION: main
   11:47:42.571 E AndroidRuntime: android.view.WindowManager$BadTokenException: Unable to add window -- token null is not valid; is your activity running?
   ```

   进程随后重启（pid 17874 之前是 8187）。**两个入口共用同一个菜单**
   （`ImeCandidates.java:111-113` 与 `:116` 两个重载，候选条 `:169`、面板格 `:217`），
   所以候选条与面板格长按「删除词条…」都走这条路。
   菜单五项的**精确边界取自 `uiautomator dump`**（不是像素猜的）：`ListView`
   `[170,1728][670,2285]`，五行依次 `[211,1763][629,1814]`（第一行是候选的英文释义标题，
   不是动作项）/ `[211,1885][629,1936]` 优先显示 / `[211,2007][629,2058]` 固定到首位 /
   `[211,2129][629,2180]` 取消固定 / `[211,2251][629,2285]` 删除词条…。同日 12:00 在同一
   装机上复测一次（长按第二格 `input swipe 267 1700 267 1700 1500`，dump 出的边界逐项一致），
   并把「删除词条…」以外的三件各点一次：`:ime` pid 全程 17874 不变、时间窗内
   `FATAL EXCEPTION` 0 条。
   **2026-10-14 评估完成，方案待拍板**（见 PIN「『错选纠正入口』评估」一节）：崩溃链是
   `ImeCandidates.java:147-149` → `MSIMEInputService.java:6049-6059`（全仓唯一一处
   `AlertDialog.Builder`），**要拦这一项必须改 Java**；但纠错本身有**Java 零改动**的走法——
   Core 的 `Engine::note_backspace` + `apply_retraction` 会在「删掉刚上屏的词、重打同一段拼音、
   选别的词」时把那次学习整笔退回，上游 macOS 壳就是这么接的，而**我们从来没调过
   `note_backspace`**（`grep -rn note_backspace engine/src/` = 0），且 ⌫ 一定会送到 native
   （`HardwareKeyPolicy.java:12` → `command(0)`，`keys.rs:65-67` 把返回值丢了）
   ⇒ 补一行 `if !engine.backspace() { engine.note_backspace(); }` 即可激活。
   另：`Engine::forget` 的语义是**清掉个人学习**（词库词只是掉回静态次序，不是永久屏蔽），
   且**整句候选上屏根本不记学习**（实测：连点 6 次非最优整句，三张表一个没生成；
   Core 行为，是否有意未核实 ⇒ 本项目只长期标注，不改 Core、不反馈上游，见 PIN 同名节）。
   **2026-10-14 03:25 方案 A 已实现并真机验收**：本条那个崩溃**本身仍未修**（Java 零改动下修不了，
   点「删除词条…」照旧崩），但**纠错不再依赖这个入口** —— `keys.rs` 命令 0 改成
   「组句非空 ⇒ `Consumed`；组句为空 ⇒ 记 `note_backspace` 并回 `Passthrough`」，
   于是「误选 → ⌫ 删掉 → 重打同一段拼音选对的」会由 Core 把上一次的学习整笔退回。
   真机实测（`suoyi`：误选 蓑衣 词频 48 → 顶到 @0 → 两下 ⌫ 删掉 → 重打选 所以）：
   三张表里 `蓑衣` 的词频、`suoyi 蓑衣` 选择、`<s> 蓑衣` 转移**全部消失**，冷启动后次序回到
   `所以@0 蓑衣@1`；`FATAL` 0。**顺带修掉一个本轮量出来的旧缺陷**：改之前空组句下的 ⌫
   回 `handled:true`，宿主因此不删字 ⇒ **⌫ 在没组句时是死的**（旧版实测：上屏 `你好` 后连按两下
   ⌫，输入框仍是 `你好`）；改后同一动作删字正常。全过程与两例对照见 PIN
   「方案 A 实现与真机验收」一节。

8. **升级安装包不会重新解包随包资源**：`Bootstrap.java:37-41` 只在没有
   `runtime-options.json` 时走 `extractDictionary`；已有配置走
   `refreshExistingConfiguration`（`:372-379`），它要 `refreshHost` 回
   `dictionary_outdated` 才重解包，而 `refreshHostRaw` 属 86 个桩、报不出这个串。
   真机两侧都验过：`install -r` 后 `bootstrap/resources/` 仍只有 `dict.tsv`；
   `pm clear` 后 `glossary-en.tsv`（1,815,026 B；现值 2,167,528 B）才出现。后果是 C 的译词对
   「装过旧包再升级」的设备不可见，除非清数据。
   **修法已立项**（只 `stat` 比长度、零新依赖、不动 Java），排在下一轮独立做，
   见 `third-party/PIN.md`「refreshHostRaw：方案① 立项，排下一轮」。
   **本条已于 2026-10-13 20:44 真机关闭**（`refreshHostRaw` 转真实现，只比随包锁里那几个
   文件的长度）：装「表更长」的包，`install -r` 且**不 `pm clear`**，解包件从 2,150,622 B
   变到 **2,150,650 B**（20:38）；再装回原包，变回 **2,150,622 B**（20:44）——两个方向都通。
   每次升级只刷失败一次，日志成对：`[msime_android::wire] 回 ok:false: dictionary_outdated:
   glossary-en.tsv:length-2150650-want-2150622` 紧跟 `MSIMEBootstrap: Runtime options refresh
   failed: dictionary_outdated`；**随后再冷启动一次，这两条都是 0 行**（不误报、不重复解包）。
   四种分档（全等 / 长度不符 / 文件缺失 / 配置读不出不得报过期）与实现细节见
   PIN「refreshHostRaw ① 落地」。

9. **「空组句下的 ⌫ 是死键」——已于 2026-10-14 03:25 修复**（单独立一条是为了将来 grep
   「⌫ / 退格 / backspace」能找到，它埋在方案 A 里面不显眼；**第 7 条原文没动**）：
   - **缺陷**：没有组句时按 ⌫，应用里刚上屏的字删不掉。旧版真机实测——上屏 `你好` 后连按两下
     ⌫，输入框仍是 `你好`（同一串观测在第 7 条末尾与本节「方案 A 实现与真机验收」都记过；
     它是方案 A 之前就存在的老问题，不是方案 A 引入的）。
   - **成因**：`engine/src/keys.rs` 命令 0（⌫）原来无条件回 `CommandOutcome::Consumed`
     → `handled:true` → 宿主 `MSIMEInputService.apply()` 用
     `strictBoolean(result, "handled")`（`MSIMEInputService.java:2424`）判"已被输入法消费"，
     于是**不再自己删字**。而 ⌫ 一定会送到 native（`HardwareKeyPolicy.java:12` DEL→0、
     `MSIMEInputService.java:4006-4007`），所以宿主没有兜底机会。
   - **修法**（就是方案 A 那一行改动）：命令 0 改成「`engine.backspace()` 为真 ⇒ `Consumed`；
     为假（组句为空）⇒ 先 `engine.note_backspace()` 记撤销账，再回 `Passthrough`」——
     交给宿主自己删那个字，与上游 macOS 壳同法
     （`apps/macos/src/imk/controller/command.rs:10-19`）。
   - **证据**：单测 `empty_composition_backspace_passes_through_and_retracts_the_mis_pick`
     与 `backspace_inside_a_composition_is_consumed_and_leaves_the_erase_ledger_alone`
     （`engine/src/keys.rs`，2 条都过）；真机 03:21 最小序列——上屏 `蓑衣` → 1 下 ⌫ 得 `蓑` →
     2 下 ⌫ 回占位符 `打字试试`，且同一段拼音重打时 `蓑衣` 仍在 @0（撤销账本生效）；
     03:23 两例对照（clean / retap）`FATAL EXCEPTION` 0 条。
   - **仍未验**：本轮删字走的是 `input keyevent 67`（=硬件 DEL → `command(0)`）这条路，
     **屏幕删除键的点击**（`ImeLayoutRows.java:388/469/565`、`ImeNineKeyPanel.java:294`）
     与九键/手写面板下的同一动作**未单独复测**（按上面的 Java 调用链应当同路，属推断）。
10. **跨会话撤销的窗口 = 进程存活期**：`recent_commits` 只活在内存里（方案甲-2 按包名挂起
   `EngineSession`，见「当前状态」那条），进程被杀 / 冷启动后账本丢失，**没有撤销入口**。
   2026-10-15 用户裁定接受这个边界（跨冷启动要改 Core，已排除）。
11. **「检查更新」入口已关掉（2026-10-15 21:47 方案 b 落地，宿主 Java 树的第 3 处破例）**：
   改动在 `qingshan/host/java/app/msime/android/home/UpdateJobService.java`——加一个命名开关
   `IN_APP_UPDATES_DISABLED = true`，让 `installedFromPlay()` 走
   `IN_APP_UPDATES_DISABLED || PLAY_STORE.equals(installer(context))`。借 Play 那道现成的门
   一次带走三处消费：关于页的「检查更新」药丸（`AboutPage.java:190`/`:219`）、
   「自动更新」与「更新通道」两行（`:122-126`）、每天一次的后台任务（`UpdateJobService.java:51`）。
   **没删 Play 来源判定、没动 `UpdateApi.java` 的三个地址常量**：将来接青杉自己的发布物时
   把开关翻成 `false` 就恢复原行为（用开关而不是直接 `return true`，是为了不把 `installer()`
   变成死代码、也让回退是一行）。编译实测 `./gradlew compileDebugJavaWithJavac` 退出码 0；
   **本轮没重打 APK**（APK mtime 仍 18:09:20）⇒「药丸确实不显示」这条**尚未真机验**，下一轮出包时看。
   下面是现象与两次裁定的完整历史（原文保留）：宿主那三条地址是上游写死的：
   `qingshan/host/java/app/msime/android/account/UpdateApi.java:45` 查
   `https://msime.app/api/releases?platform=android`、`:47` 从
   `https://github.com/metasequoiaime/msime/releases/download/` 取包、`:49` 先试镜像
   `https://dl.msime.app/gh/`，edition 固定 `full`（`qingshan/host/gradle-app/app/build.gradle.kts:48`），
   而 `qingshan/host/java/app/msime/android/home/AboutPage.java:52` 说明这三项只在**非 Play 安装**时显示
   ⇒ 我们侧载的包会显示「检查更新」。真实后果是**「提示有新版 → 下几十 MB → 拒绝安装」的死循环**，
   不是装错包：`UpdateApi.java:404` 安装前核对「包名与本应用相同 + 版本号更高 + 签名证书与当前一致」，
   `app.msime.android ≠ app.qingshan.ime` 第一道就挡掉。为什么不改：用户量小、更新机制不急，
   而破例面越大越难同步上游（同一份 Java 树只改了 4 行，见第 4 条）。
   **未验**：真机没点过这个按钮（本轮装完 release 包后设备无外网，点了也测不出下载行为）。
   将来要做更新机制，先决定三选一：指向青杉自己的仓库 / 关掉这三项入口 / 只发不经应用内更新的包。
   **（现值 2026-10-15 20:50：三选一的破例面已读码量完，见 PIN「「检查更新」三选一的破例面」一节。**
   推荐**关掉入口**，代价实测 **1 个文件 1 行**（`UpdateJobService.java:123-125` 那个谓词恒真，
   一次带走「检查更新」药丸 + 「自动更新」「更新通道」两行 + 每天一次的后台任务）；
   指向青杉要动 5–6 处常量，还要一个等价的 releases 列表端点（`msime.app/api/releases` 是水杉自建镜像）
   与发布侧上传 `.sha256`。**两条修正**：① 危害比本节原话轻——`UpdateJobService.java:92-94` 显示
   自动更新**默认关**，所以只有用户主动点一下才会白下一趟，不是每天自己发生；
   ② 关入口**不能走 `local_mode`**——它是键盘视图状态（7 处读它，含 `ImeLetterRows.java:210`），
   走它关不掉关于页反而会改键盘行为。**
   **（现值 2026-10-15 21:47：按 b 落地，见本条开头。**上面那段「1 个文件 1 行」的说法
   实现时改成 2 行（开关常量 + 短路），理由如上。**）

## 构建环境的已知限制

**依赖源未配国内镜像**：`host/gradle-app/settings.gradle.kts` 用 `google()` + `mavenCentral()`
直连。两个源本机实测可达（Google Maven 就是下 SDK 用的那个），但没走镜像。首次 `assembleDebug`
实测耗时 3m 33s，`GRADLE_USER_HOME/caches/modules-2` 落盘 144,305,065 B，整个
`GRADLE_USER_HOME` 到 1,008,259,744 B（其中 Gradle 分发自身 289,496,395 B、
`caches/8.14.3` 573,600,003 B）。Gradle 分发已改走腾讯镜像，且 `distributionSha256Sum`
与官方 pin 一致，内容等价。

**`cargo fmt --check` 常红**：`engine/src/lib.rs` 的 JNI 签名为对齐
`host/java/app/msime/android/core/NativeClient.java:594-704` 刻意保持紧凑单行形态，这些
hunk 是有意保留、不是失误，手改前先读本节。（曾另计 `engine/src/view.rs` 的 6 个测试排版
hunk，合计 14；2026-10-12 02:33 一次误跑 `cargo fmt` 写模式把它们规范化了，原始排版无备份、
不可逐字节复原。现值以 `cargo fmt --check` 实测为准 = **17 hunk，全在 `lib.rs`**；
计数史：阶段一 9 → B1 后 13 → A 后 16 → 乙-2 后仍 16 → **refreshHostRaw 后 17**，
增量来自各步新增真实现的紧凑签名（第 17 个就是 `refreshHostRaw` 沿用同一写法，
**没有**顺手格式化前面 16 个）。`resources_lock.rs` 按 rustfmt 结果写，0 hunk。）

构建前还须读 `third-party/PIN.md` 的「环境陷阱」一节（全部条目，其中 `python3` 占位符与
ANSI 假零最值得先读）。

## 当前状态

**PRD 阶段一（最小闭环）四行验收标准全部在真机通过**（OPPO PEGT10，Android 13 / SDK 33，
`primaryCpuAbi=arm64-v8a`，装机件 `versionCode=1000 / versionName=0.1.0`）：

| PRD 阶段一 | 真机结果 |
|:---|:---|
| 交叉编译出 arm64 `.so` | ✓（另出 x86_64，两 ABI 各 102 个 JNI 导出） |
| 最小 JNI 适配层，返回候选 | ✓ `nihao` → 候选条 `你好 你好好 你好像 你好久 你好看 你哈 …`，读音行显示 `nihao`，页码 `1/5` |
| 可安装 APK，系统识别、键盘弹出 | ✓ `ime list -s` 里有我们、`mCurMethodId=app.qingshan.ime/…MSIMEInputService`、`mInputShown=true` |
| `nihao` → 「你好」上屏 | ✓ 按空格后输入框内容为 `你好`（`uiautomator` 节点文字回读） |

过程中真机第一轮暴露两个启动路径击穿点，都已修（见 `third-party/PIN.md`「启动路径的击穿判定」）：
`requery()` 把「读不出拼音」当故障（单按一次 `i` 就杀会话）、`setEnglishModeRaw` 还是桩却被硬解析。
修复后键盘右上角那条「共享运行时未就绪：仅直接输入」红幅消失，中/英切换键也可用
（按「中」→ 键位标签 `全拼`→`space`、`，`→`,`，再按字母直出英文且无候选条）。

- 引擎侧：**51** 个宿主测试全过（32 个原有 + `resources_lock.rs` 7 个 + `session.rs` 落盘 4 个
  + `keys.rs` 方案 A 2 个 + `assemble.rs` C1 方案 C 新增 6 个；**2026-10-14 16:55 复跑 `cargo test`：
  51 passed / 0 failed**、0 warning。上一轮记的 45 是含方案 A、不含修正表的实数，保留原文），
  `cargo fmt --check` 是刻意保留的 **17 个 `lib.rs` hunk**（各步新增真实现沿用同一
  紧凑签名写法；同日 16:55 复跑仍是 17 处，**`assemble.rs` 新代码手工排成 rustfmt 的形状，没多出 hunk**）。
- 产物侧：`app-debug.apk` 49,659,177 B（clean；现值 **49,897,529 B** = A3 第 7 批落表 63 条后（表 +2,934 B、`.so` 磁盘大小不变）；上一版 **49,896,069 B** = 方案甲-2 落地后（`.so` 4,371,376 / 4,619,872 B）；上一版 49,861,845 B = A3 第 5 批落表后；再上一版 49,850,705 B = 方案 C 落地后 +4,032 B；49,846,673 B 见 flush_learning 与观测口那条），
  `verify_apk.py` + `zipalign` + `apksigner`
  退出码全 0；整轮真机操作期间 `logcat` **无 FATAL / AndroidRuntime / ANR**。
  （该"整轮"只覆盖那一轮。同日长按复测那轮**有** FATAL，且是宿主自身的
  `BadTokenException`，见「已知失效项」第 7 条。）
- 译词侧（阶段二 C）：真机 `nihao` 首页 10 格 **5 格带英文释义**，`你好` → `int. hello · int. hi`、
  `你好像` → `phr. you seem`，与子集文件逐格一致；`会话建立 119 / 137 / 150 ms`
  （词库 54–72 ms + 译词表 63–72 ms），比不带译词的 54–55 ms 约翻一倍。
  `译词全未命中` 这条 warn 在正常路径上 **0 条**，把表从 `bootstrap/resources/` 移走后
  每次按键各一条——门控是双向验过的。
- 译词表 A1（2026-10-13 13:45，只动数据、代码零改动）：子集从「词频前 80k」的 76,273 行
  换成「词库 ∩ 全量」的 **88,349 行**（+12,076 行），文件 1,815,026 → **2,116,088 B**，
  APK 49,659,177 → **49,789,817 B（+130,640 B，+0.26%）**。真机 `pm clear` 重解包后
  `run-as` 看到 `files/bootstrap/resources/glossary-en.tsv` = 2,116,088 B，
  `译词表载入 62 / 78 ms`、`会话建立 143 / 144 ms`（A1 前 55–59 ms / 104–108 ms，
  在噪声内，未做统计）。`cargo test --release` 仍 **32 passed**，其中
  `shipped_glossary_subset_parses` 就是用真加载器 `Glossary::from_path` 过新表的那条。
  **收益要分三个口径看，别只看一个**：
  ① 词库侧（唯一词头有译词的比例）**82.6% → 95.7%**——这是我上轮拿去拍板的那个数字；
  ② **词频加权**（按词频算「敲出来的词有译词」的比例）**93.438% → 93.463%，只涨
  0.026 个百分点**——新增 12,076 词的词频合计 70,916 / 全库 275,814,283，
  其中词频最高的是「调演」（15），**词频 ≥100 的一条都没有**；
  ③ **候选侧**（同一个音节串的全部候选里带译词的比例，按 `dict.tsv` 音节索引算）：
  `ni` 27 条候选 66.7% → **81.5%**（+14.8 pt）、`shi` 82 条 84.1% → **95.1%**（+11.0 pt），
  而 `jin tian` / `xie xie` / `de` / `ni hao` 都是 **100% → 100%（+0 pt）**。
  ⇒ A1 的实际作用面是**单音节、同音词很多的那些输入的翻页**，常用多字词本来就已满覆盖。
  真机对照：`nihao` 首页与 A1 前**逐格相同**（`你好`/`你好像` 有译词，
  `你好好`/`你好久`/`你好看`/`你哈` 仍无）——**这四个词不是覆盖率问题，它们在全量译词表里
  也不存在**，任何子集裁法都救不回来（我上轮把它们说成「子集没裁到」是错的）。
  **2026-10-13 14:30 用户裁定：A1 保留，不回滚。** 理由记在案：+130,640 B 占 APK 的
  0.26%，用户不可感知；候选侧单音节翻页 `ni` 66.7→81.5%、`shi` 84.1→95.1% 是真实可感的
  提升；设备已经为 A1 做过一次 `pm clear`，回滚要再清一次数据、再走一遍引导页，
  代价大于收益。**教训也已登记**：我上轮报的 +8.5 KB 是错数（把 APK 内 stored 与我方
  zlib level 9 估算相减），正确口径见 PIN 环境陷阱第 25 条。
- 句子译词 A3（2026-10-13 14:40 第 1 批 197 条 → **落表 165 条**；15:20 第 2 批 291 条等审）：清单来自
  「能被 `dict.tsv` 词头 100% 最长正向匹配切分」的手机常用语，逐条自审后分三堆——
  **33 条其实已经在表里**（`早上好`/`晚上好`/`晚安`/`再见`/`不客气`/`没问题`/`不好意思`/
  `没关系`/`辛苦了`/`一路平安`/`生日快乐`/`新年快乐`/`节日快乐`/`恭喜恭喜`/`保重`/
  `多少钱`/`太贵了`/`太晚了`/`你在哪里`/`好多了`/`多喝水`/`都可以`/`好久不见`/`还可以`/
  `不太好`/`非常好`/`就是这样`/`还没有`/`慢慢来`/`加油啊`/`我帮你`/`我请你`/`不用了`），
  不重复追加；**14 条我有疑问**（14:40 提出，15:03 你逐条裁定全部采纳我的建议）。
  那 14 条落定时两条按你的授权展开：第 2 条取「我同意」（弃「这是我的意见」）、
  第 6 条拆成「公交车来了」+「出租车来了」两行 ⇒ **14 条 = 15 行**。
  合计入表 150 + 15 = **165 行**。表 88,349 → **88,514 行**（唯一词头 88,514、重复 0），
  文件 2,116,088 → **2,121,929 B**，APK 49,789,817 → **49,793,061 B（+3,244 B）**。
  （第一版我按行号挑疑问项时错把没问题的 53「我不太饿」当疑问、漏了真正可疑的 32
  「我同意我的意见」，发现后**回滚到 A3 追加前重做了一遍**，所以最终是 150/14/33 而不是 149/15/33。）
  源件 `engine/tools/glossary-sentences-zh-en.tsv`（165 行）留在仓库里，表尾两行 `#` 注释
  指回它。**15:06 已按批准给 `build_glossary_subset.py` 加了 `--append`**（默认就指向那个
  源件，所以不带参数重跑得到的是完整随包表；另有 `--no-append` 显式只出子集）。
  双向逐字节验证过：`--no-append` 的输出与 A3 之前的快照 `cmp` **逐字节相同**；
  默认输出与显式 `--append` 指到源件的输出也**逐字节相同**。
  **A3 不改整句候选的排序——真机已验**：`womenhuijiaba` 首页第 1 格仍是「我们回家啊」
  （无译词），第 2 格「我们回家吧」下面出现 `let us go home`。
  **真机验证（15:09 装机 + `pm clear`，走完引导页四步）**：
  `run-as` 看到 `files/bootstrap/resources/glossary-en.tsv` = 2,121,929 B；
  三条整句的第二行译词都出来了——`womenhuijiaba` 第 2 格 `let us go home`、
  `shoujimeidianle` 第 1 格 `the phone is dead`、`jintiantianqihenhao` 第 1 格
  `the weather is nice today`（展开面板同样显示，且不截断）；
  对照 `nihao` 首页与 A1 包**逐格一致**（`你好`/`你好像` 有译词，`你好好`/`你好久`/
  `你好看` 仍无）⇒ A3 不影响词区。`:ime` pid 5637、crash 缓冲 `FATAL` 0、
  两条上屏路径复验通过（点候选条 → `你好`；空格 → `你好你好好`）。
  **顺带修一个我上量错的数据**：候选条第一格（选中态、带底色的那颗）实测放得下
  25 个拉丁字符（`the weather is nice today` 完整显示），不是我上轮按窄格量出的
  「14–20 字符」；面板格同样不截断。⇒ B 的「22 字符预算」是按窄格定的，
  **落地前要按格宽分别重量**（见 PIN 的 B 降级记录）。
- A3 第 2 批（2026-10-13 15:40，**已落表、未装机**）：316 条草稿 → 与第 1 批重复 3 条剔除、
  已在译词表 22 条不重复追加 → 291 条；再补 12 条凑满 **500 条整句语料**（197 + 303）。
  14 条疑问项按用户裁定全部采纳建议改法；自审又抓到 **17 处我自己的译文错误**（含一处人称错：
  `老板找我` 被我译成 `the boss is looking for you`），已改后入表，逐条列在
  `third-party/PIN.md`「A3 第 2 批：我自改的 17 条」，用户不认同哪条就改回哪条。
  表 88,514 → **88,817 行**（唯一词头 88,817、重复 0），文件 2,132,624 B，
  APK clean **49,798,409 B**（本批 +5,348 B）。源件 `glossary-sentences-zh-en.tsv` 现 468 行，
  本批是**走 `build_glossary_subset.py --append` 的正规生成路径**写回随包表的。
  `cargo test --release` 32 passed、`cargo fmt --check` 16 hunk、审计 187 条。
  **本批没装机、没 `pm clear`**：真机验证并入下一次装机窗口（等 A3 扩到 1,000 或别的改动一起装），
  避免每落一批就清一次数据。装机后要看的仍是：这批整句的第二行译词有没有出。
- A3 第 3 批 + 三批合并装机验证（2026-10-13 16:40，**已落表、已装机**）：518 条草稿 →
  可用 514（3 条含拉丁字母切不出、1 条与既有语料重）→ 取 496 条凑满
  **整句语料 1,000 条**（504 + 496），其中 5 条已在译词表不重复追加 ⇒ 源件 468 → **959 行**，
  表 88,817 → **89,308 行**，文件 2,150,622 B，APK clean **49,807,381 B**（本批 +8,972 B）。
  自审又改了自己 **28 处**（拼写 `rince`→`rinse`、不合语法 `do not wait dinner for me`→
  `do not wait up for me`、疑问句误作陈述 `听懂了吗` `got it`→`did you get it`、
  多音字与口径不一致若干），逐条见 PIN。`cargo test --release` 32 passed、fmt 16 hunk、审计 196。
  **装机**：`install -r` + `pm clear` + 走完引导页四步，`run-as` 看到
  `files/bootstrap/resources/glossary-en.tsv` = 2,150,622 B，`译词表载入 66 / 68 ms`，
  `:ime` pid 28055，crash 缓冲 `FATAL` 0，`default_input_method` 仍是我们的。
  **三批抽样结果——这里有一个改变判断的发现**：
  第 1 批 `womenhuijiaba` 第 2 格 我们回家吧 → `let us go home` ✓（且仍排在
  「我们回家啊」后面，**A3 不改排序**再验一次）；第 2 批 `shoujimeidianle` 第 1 格
  手机没电了 → `the phone is dead` ✓；对照 `nihao` 首页与 A1 包**逐格一致** ✓；
  但**第 3 批抽 5 条只有 2 条亮**：`mashanghuilai` → 马上回来 `back soon` ✓、
  `jidesuomen` → 记得锁门 `remember to lock up` ✓，而 `wobadetuole` 打不出「我把地拖了」
  （首页是 我吧的脱了/我吧德国了）、`xueyapiangao` 打不出「血压偏高」（首页是 血压偏好/学鸦片高）、
  `xiyijizaixi` 打不出「洗衣机在洗」（首页是 洗衣机再次/在此/仔细）。
  ⇒ **「整句能被词库 100% 切分」只是必要条件，不是充分条件**：引擎给不给这个整句候选、
  排不排进首页，由词图与打分决定（我们没接语言模型，末字是同音多写的高频字时尤其容易被顶掉）。
  所以 A3 落表的条目里有一部分是**永远显示不出来的死行**（无害，但无收益）。
  这条写进 PIN，并且是**下一轮先测可达率、再决定要不要继续扩量**的依据。
- **可达率测量与 A3 扩量决策**（2026-10-13 18:07；观测口 = ① 临时探针，测完已逐字节还原）：
  固定 **23 条整句**（2/3/4/5 段、虚词、上轮对照组与失败样本各一组，全部来自已落表的 1,000 条语料；
  每条拼音先过 `dict.tsv` 切分核验，再过「应为 vs 实敲」机器比对——这道比对抓出我自己敲错的 3 条）。
  结果：**首页前 3 格 / 首页全部 / 展开面板三档同为 18/23 = 78.3%**。三档同值不是巧合：
  命中的 18 条位次只有 0 或 1，「在首页但不在前 3」这一档在本样本里是**空集**。
  失败 5 条（`先这样吧` `完成了` `我把地拖了` `血压偏高` `洗衣机在洗`）**全部是「整句不在候选表」，
  「候选出现但无译词」0 条**；这 5 条的表大小 87–283 行、远没到实测上限 ~500
  ⇒ 是**词图与打分没产出这条路径**，不是排序靠后，**加译词条目救不了这一类**。
  探针的 `G` 位与屏幕第二行做过逐格目视对照（两例共 10 格全对），所以这不是"注入结果代表真实触摸"。
  **建议（等用户拍板；本轮没有扩量、没有动词库）**：78.3% 落在「≥ 50%」档 ⇒ **A3 值得继续扩**，
  下一批 **+500 条**（凑 1,500），按第 3 批实测同比例（+491 行 ⇒ APK +8,972 B）预算约 **+9 KB**，
  预期新增可达整句 ≈ 500 × 78.3% ≈ **390 条**，其余 ~110 条是无害死行。
  口径提醒：78.3% 是**整句样本内的可达率**，不等于用户可见收益——A1 那次的词频加权只涨
  **0.026 个百分点**（见上面 A1 条目），所以扩量值不值还要按「新增可达的高频整句条数」算。
  **干净版复验**（探针逐字节还原后）：`cargo test --release` 32 passed、`cargo fmt --check` 16 hunk、
  `grep -rn PROBE engine/src/` 0 条（正样例 `translation_to_string` 9 条）、两个 `.so` 里
  `grep -a -c PROBE` = 0（正样例 `译词表载入` = 1）、APK 回到 **49,807,381 B**（与 A3 第 3 批 clean
  同值）；装机后点第 1 格 → 输入框 `你好`，再打 `womenhuijiaba` 按空格 → `你好我们回家啊`，
  `:ime` pid 22162，crash 缓冲 `FATAL` 0。同一台设备、探针版与干净版各打 `jintianhenre` 与
  `wanchangle`，候选条**前 5 格文字与第二行译词逐格一致**（2 例 × 5 格）⇒ 探针没有扰动排序。
- **refreshHostRaw ① 落地**（2026-10-13 20:44，关掉「已知失效项」第 8 条）：随包锁
  `desktop-dictionary.lock.json` 扩成 `name` + `bytes`（由 `build_glossary_subset.py` 一次写出），
  `engine/src/resources_lock.rs` 用 `include_str!` 烘进 `.so`，`refreshHostRaw` 只 `stat`
  锁里那几个文件比长度。**零 Java 改动、零新依赖**。真机两侧都验过（全程不 `pm clear`）：
  装更长的包 ⇒ 解包件 2,150,622 → **2,150,650 B**；装回原包 ⇒ **2,150,622 B**；
  再冷启动一次 ⇒ `dictionary_outdated` 与 `ok:false` **各 0 行**（不误报、不重复解包）。
  单测 7 条覆盖四种分档（配置读不出**不**报过期，口径同 `crates/host-api/src/lib.rs:1629-1640`）。
  计数：**17 real + 85 stub = 102**（`verify_symbols.py` 两 ABI 缺失/多余均 0）、
  `cargo test --release` **39 passed**、`cargo fmt --check` **17 hunk**（16 旧的 + 新的那件同风格）、
  `scan_bridges.py --summary` 与基线逐项相同。装机后复验：点第 1 格 → `你好`，`nihao` + 空格 →
  `你好你好`，`:ime` 与主进程都在、crash `FATAL` 0。
  **过程中撞了两个坑**（细节在 PIN「refreshHostRaw ① 落地」与环境陷阱第 28 条）：
  ① 只重打 APK 没重编 `.so` ⇒ 烘的是旧锁，升级既不重解包也不报错——新纪律的实测后果；
  ② `refreshHostRaw` 可能就是本进程第一个桥调用 ⇒ 日志桥得在这里也装一次，否则我们自己的
  原因行是静默 no-op（补 `logcat::init()` 后才第一次在 logcat 看见那行原因）。
- **A3 第 4 批（+500）预检异常，本轮没有落表**（2026-10-13 22:10）：起草 573 条 → 派生可切 522 →
  自审改 61 条译文、删 2 条不自然说法 → **520 条待落**；拼音全部由 `dict.tsv` 反推（不手敲），
  自审补丁逐条 `assert` 原文。落表前系统抽样 20 条打真机候选条：**前 5 格命中 11/20 = 55%**
  （命中的全在第 1-2 格），9 条未命中里 2 条换自然读音复测仍未命中 ⇒ 是同音高频组合压过目标，
  不是拼音错。归因很直白：**命中组单字段占比均值 0.288、未命中组 0.75**，全批均值 0.424，
  按「单字段 < 1/3」过滤只剩 242 条。**口径限制**：本轮不改代码就没有观测口，只能测「前 5 格可见」，
  测不到「在不在候选表里」，所以与上轮 78.3%（前 3 格且带译词、样本是已落表的老语料）不是同一个测量。
  按停手规则交回用户三个选项，细节与证据在 PIN「A3 第 4 批：预检异常」与
  `D:\toolchains\tmp\qingshan-snap\a3b4\`。**随包表与 A3 源件本轮逐字节未变**（`cmp` 对过）。
  落表通道已经就绪：`build_glossary_subset.py` 一次写表与锁 → 重编 `.so` → `install -r`
  **不再需要 `pm clear`**（上一轮 refreshHostRaw 换来的）。
  （本条末尾那条「口径限制」已在 22:58 用观测口测掉，见下一条。）
- **观测口复测：真实可达率与过滤器（2026-10-13 22:58，探针已还原、代码零净改动）**：
  33 条样本（上轮那 20 条 + 新抽 10 条 + 3 条对照），帧完整 33/33。
  **关键档「整句在候选表里」= 新批 16/30 = 53.3%**；「前 3 格且带译词」只有 2/33，
  且那 2 条全是对照——**新批没落表就不可能有译词，6.1% 不是回退**。
  对照 3 条复现了上一轮的数据（`你好` 位次 0 带译词、`手机没电了` 位次 0 带译词、
  `洗衣机在洗` 不在表）。**最值钱的发现是这台引擎的行为是二值的**：命中的 16 条位次
  全是 0 或 1，没有「在表里但埋得深」这种情况 ⇒ 上轮「前 5 格 55%」与本轮「在表里 55%」
  是同一个数，**前 5 格代理指标有效，以后不必每批开观测口**。
  **过滤器实测**：单字段占比 ≤1/3 的 17 条里 14 条在表（**82%**）、1/2~2/3 的 7 条里 2 条（29%）、
  ≥3/4 的 6 条**全部不在表（0%）**。全批 520 条分桶 A 242 / B 206 / C 72，外推
  **全落 ≈ 258 条会露出**、**只落 A 桶 ≈ 198 条**。建议 **② 全落 520**（多约 60 条可见，
  代价是约 262 条无害死行与约 4.7 KB），等用户拍板。
  24 条疑问裁定已全部落地（16 改 8 留，逐条 `assert` 现值；与随包表同译文 0 条、批内重复 0 条），
  裁定后的 520 条存在 `qingshan-snap/a3b4/derived.ruled.tsv`。
  还原证据：`view.rs` 与探针前快照 `cmp` 逐字节相同、`grep -rn PROBE engine/src/` = 0
  （正样例 `no_op` 6）、两个 `.so` `grep -a -c PROBE` = 0（正样例 `译词表载入` = 1）、
  APK 回到 **49,835,753 B**、`cargo test --release` 39 passed、`fmt --check` 17 hunk；
  装机复验点第 1 格 → `你好`、`nihao`+空格 → `你好你好`、crash `FATAL` 0、设备侧 PROBE 0。
- **A3 第 4 批落表（2026-10-13 23:37，用户选 ② 全落）**：源件 `glossary-sentences-zh-en.tsv`
  959 → **1,479 行**（追加 `derived.ruled.tsv` 的 520 条，24 条疑问裁定已内含；追加前断言
  批内 0 重复、与源件 0 冲突），表 89,308 → **89,828 行**（重复词头 **0**），文件
  2,150,622 → **2,167,528 B**（+16,906 B），APK clean **49,843,905 B**（+8,152 B，
  与包内 deflate 增量 +8,150 B 同量级）。**`.so` 已重编**——实测包内烘的是新锁
  （`grep -a -c 2167528` = 1、`2150622` = 0），这是新纪律第一次真正走完整条链。
  **装机验证不再 `pm clear`**：`install -r` + 冷启动后解包件从 2,150,622 B 自动变到
  **2,167,528 B**（23:37），日志恰好一次 `dictionary_outdated`（我们的原因行
  `glossary-en.tsv:length-2150622-want-2167528` + 宿主 `MSIMEBootstrap` 行，wire 层
  `ok:false` 计数 = 1 ⇒ 重解包后的第二次刷成功），`译词表载入 73 ms / 2167528 B`，
  `:ime` pid 1354、主进程 32215、crash `FATAL` 0、`default_input_method` 仍是我们的。
  **抽样 8 条新句（覆盖出行/吃饭/购物/工作/身体/家庭）7 条露出**，位次全是 0 或 1：
  `我打车走`②`i will leave by taxi`、`出口在右边`①`the exit is on the right`、
  `我早饭没吃`①`no breakfast today`、`运费到付`②`pay on delivery`、`刷卡失败`①`card declined`、
  `投影仪没信号`①`the projector has no signal`、`我要验血`②`i need a blood test`；
  未露出的 `我家里有事` 与观测口实测的「不在表」判定一致（**死行无害：不报错、不影响别的条目**）。
  对照 `nihao` 首页与 A1 包**逐格一致**（`你好`=`int. hello · int. hi`），
  `womenhuijiaba` 第 1 格仍是 `我们回家啊`、第 2 格才是 `我们回家吧`=`let us go home`
  ⇒ **A3 不改排序再验一次**。点第 1 格 → 输入框 `你好` ✓。
  **引擎二值行为已确认（用户 2026-10-13 拍板）**：整句要么排在位次 0/1，要么根本不产出，
  所以**以后每批 A3 用「候选条前 5 格」代理指标即可，不必每批开观测口**；
  若某批出现位次 ≥ 2，停下重新评估。
- **flush_learning 落盘（2026-10-14 00:54，方案 2 = 会话销毁必落 + 上屏满 60 秒节流）**：
  全在 Rust 侧（`session.rs` 加 `LEARNING_FLUSH_INTERVAL` / `note_commit` / `flush_at_destroy`，
  `lib.rs::run_transition()` 在 `requery()` **之后**才落盘，`destroy_session()` 取出后在锁外 flush），
  **`host/java` 零改动**。动手前按 PIN 的要求先核「一次 flush 五项」的边界：除 learner 外四项
  （`NoInputLogger` / `NoUsageMeter` / `NoVocabularyTracker` / `NoTranslator`）都走 trait 的
  **空默认 `fn flush(&mut self) {}`**，所以不会顺手打开四个没计划的落盘面。
  真机（不 `pm clear`）：三次上屏后 `files/qingshan/` **仍空**（节流生效）→ 回桌面触发
  `onFinishInput` → `destroy` 后出现 `user.tsv` 50 B / `user-choices.tsv` 71 B /
  `user-ngram.tsv` 133 B → `am force-stop` 冷启动后再上屏一次，`你好` 从 3 涨到 **4**（读回 + 累加）。
  写盘耗时实测 **0 / 3 / 7 / 9 ms**、crash `FATAL` 0 ⇒ 无可感知卡顿；节流那条路也自然触发过一次
  （`00:51:41 学习数据落盘 0 ms（上屏满 60 秒）`）。
  **两个边界如实记着**：① 上屏后立刻 `force-stop`（没有 Java 回调）会丢这 ≤ 60 秒的数据，实测过；
  ② **「学习改变排序」本轮没测出来**——把落盘目录挪开做 A/B，同一个 `nihaonihao` 两张截图
  **sha256 相同、像素差异包围盒 None**，个人计数（你好 4）压不过基础词频（你好 71,960）。
  数据链路三段（写盘 / 读回 / 累加）已证，排序影响要另设决定性实验。
  **（现值 2026-10-14 01:56：排序影响已证——见下面「学习改变排序决定性实验」那条。
  这次没测出来的原因是测错了地方：`nihaonihao` 的前 4 级结构键先把整句候选排在词级候选之前，
  个人计数所在的第 5/6 级根本没轮到比。上面两句是当时的记录，保留原文。）**
  `cargo test --release` **43 passed**、`fmt --check` **17 hunk**、APK **49,846,393 B**（+2,488 B）。
  这两个数首测在 00:16–00:17，而最后一次改代码是 00:30（销毁路径日志从 `debug!` 换成 `info!`），
  所以 01:07–01:12 收尾重跑一遍：**43 passed / 17 hunk（全在 engine/src/lib.rs）/ 本地 APK 与
  设备 base.apk 都是 49,846,393 B**，数字没动。「拿改动之前的输出当最终值」这坑本轮真踩到了。
  过程中烧了两轮次：第一次跑批进的是字母不是汉字，真因是**设备被转成横屏**（截图 2400×1080）
  把坐标全打偏 ⇒ 跑真机交互前先 `screencap` 看尺寸；第二是我把销毁路径的日志定成 `debug!`，
  而日志桥按 level 挡 `debug!`，设备上看不见落盘，改成「本会话攒过上屏才打 `info!`」才拿到这张表。
  「已知失效项」第 6 条已关闭；未验证清单第 4 条半关闭（Core 侧 `info!` 实测过桥，`warn!` 仍未直接观测）。
  **收尾复验（01:14 实测）**：`run-as` 看落盘目录，三个表仍在（`user.tsv` 74 B、n-gram 表 337 B、
  choices 表 107 B，mtime 都是 00:54），距最后一次写已过 20 分钟、期间进程死过好几次——
  持久性这一格算是钉住了。比第一次销毁时的 50 / 133 / 71 B 大，是因为后来又上屏了几条。
- **学习改变排序决定性实验（2026-10-14 01:32–01:56，只读 + 真机，代码净改动 0）**：
  **学习确实改变排序，1 次上屏就翻**。观测口换成 `uiautomator dump --windows`——候选条每格的
  文字与 bounds 都能 dump 出来，按 x 排序就是位次，不必读像素、不必开探针。
  两组同音对（都满足排序键前 4 级完全并列）：**B 组 `suoyi` 所以 155,225 / 蓑衣 48（词频比 3,168）**
  基线 `蓑衣@1` ⇒ 第 1 次上屏后 `蓑衣@0`，12 次一直在 @0，`am force-stop` 冷启动后仍在 @0；
  **A 组 `shenban` 申办 592 / 身板 592** 基线 `身板@1` ⇒ 1 次有效上屏后 `@0`，冷启动后仍在 @0。
  机制归因做了三段：只挪开 `user-choices.tsv` 仍翻、只挪开 `user-ngram.tsv` 仍翻
  （⇒ 排序键第 5 级「同输入串选择次数」与第 6 级「个人 n-gram 插值」**各自单独都够**），
  **两张一起挪开则回到 `所以@0`**（只剩 `user.tsv` 的 12 次，加分 1.279 < 需要的 8.061）
  ⇒ 负对照证明翻转来自学习数据，且与 Core 的 `WEIGHT_BONUS=0.5`/`WEIGHT_CAP=20` 上限吻合。
  公式、量纲与逐轮位次见 `third-party/PIN.md`「『学习改变排序』决定性实验」一节。
  crash `FATAL` 0。**设备已复位**（02:09 实测）：三张学习表先归档再 `rm`，清空后两对都退回
  实验前的静态次序（`所以@0`、`申办@0`）⇒ 这是「翻转来自学习数据」的第三次反证；
  归档在 `qingshan-snap/learnrank/`。
  **上一轮那个「两张截图 sha256 相同」的零结果不是学习无效**，是测错了对象：
  `nihaonihao` 前 4 级结构键先决定了次序，个人计数所在的第 5/6 级没轮到比。
- **方案 A：退格即撤销（2026-10-14 03:06–03:25，已实现 + 真机验收）**：误选一次的代价变大之后
  （上一轮证明 1 次上屏就能顶到第 1 格），纠错入口从"锦上添花"变成必需，而长按菜单那条路
  在 ColorOS 上必崩（第 7 条）。**用户拍板只做 A、不做 B/C/D/E。**
  改动只在 `engine/src/keys.rs` 命令 0：组句非空 ⇒ `Consumed`；组句为空 ⇒ `note_backspace()` +
  `Passthrough`（与上游 macOS 壳同形）。**Java 零改动**，`host/java` 一个文件都没碰。
  新增 2 条单测（组句内退格不动撤销账本 / 空组句退格 + 重选 ⇒ 首格回到静态次序），
  `cargo test --release` **45 passed / 0 failed**、`fmt --check` **17 hunk（全在 lib.rs）**、
  APK **49,846,673 B**（+280 B）。真机验收：`suoyi` 误选 蓑衣（词频 48）顶到 @0 →
  两下 ⌫ 删掉 → 重打选 所以 → **三张表里 蓑衣 的词频 / `suoyi 蓑衣` 选择 / `<s> 蓑衣` 转移全部消失**、
  冷启动后回到 `所以@0 蓑衣@1`、`FATAL` 0。
  **顺带修掉一个旧缺陷**：改之前空组句下的 ⌫ 回 `handled:true` ⇒ 宿主不删字，**⌫ 在没组句时是死的**
  （旧版实测：上屏 `你好` 后连按两下 ⌫，输入框仍是 `你好`）；改后正常删字。
  **一条边界**：若中间由应用侧改文本（试用页那个「清空」按钮），撤销账本会作废——
  v1 就是这么失败、v2/v3 两例对照（含"重新点一次输入框"）证明焦点重进不影响。
  全过程见 PIN「方案 A 实现与真机验收」一节。
- **A3 第 5 批观测口 + C1 词典错读音评估 + D/E/B 裁定（2026-10-14 13:47–14:41，代码净改动 = 探针往返后 0，数据 0 落表）**：
  ① **观测口按用户选的 ③ 开完并还原**——探针仍在 `engine/src/view.rs` 的 `build_view()`，形状与
  2026-10-13 那轮逐字相同；四档实测：**新批 40 条「整句在表里」27/40 = 67.5%**（位次 0:23 / 1:2 / **2:2**），
  已落表对照 12 条 ①②③ 三档同为 **7/12 = 58.3%** 且位次全是 0，56 条样本 BAD 帧 0。
  **位次 ≥2 本轮 4 条** ⇒ 「整句要么 0/1 要么不出」这条二值前提**被推翻**，但 35 条在表样本
  **没有一条埋在首页之外** ⇒ 「在表」≈「首页前 3 格」仍成立，代理指标够用，口径改成「位次 ∈ {0,1,2} 或不在表」。
  还原证据：`cmp` 与 `qingshan-snap/b5probe/view.rs.before-probe` 逐字节相同、
  `grep -rn PROBE engine/src/` = 0、两个 `.so` `grep -a -c PROBE` = 0，`.so` 回到
  4,351,792 / 4,598,000 B，`clean assembleDebug` 后 APK 回到 **49,846,673 B（= 本轮改前值）**，
  `install -r` 后点选 `你好好` 与 `women`+空格上屏都通，`FATAL` 0。
  ② **C1 词典错读音根因定位到行**：随包 `dict.tsv` 与青简 `assets/lexicon/dict.tsv` `cmp` 逐字节相同；
  `长时间/太长/很长` 不在上游常用词表里，是**短语层生成器**按 `phrases/mod.rs:231`「同词多读音取词频最高」
  拼出来的（`:104-114` 取音、`:247-253` 选择），叠加单字 `长 zhang 130,456 > chang 81,856`。
  机械风险集 2,476 行是**上界**，逐字判定后**确证真错 10 行**（九个 `*长=zhang` + `睡不着=shui bu zhe`）
  ⇒ C3 的 277 条不是错数。**四个方案已列、推荐 C**（`assemble.rs` 里
  `engine.set_extra_dictionaries(...)`，挂点 `engine/setup.rs:495`/`:505` 读码确认，性能未测），
  本轮**没动 `dict.tsv`、没动 Core、没为 C1 重编 `.so`**，等拍板。全过程见 PIN「C1 词典自带错读音」一节。
  ③ **D/E/B 裁定已在批数据上执行**：D2/D5/D6 定为流程关卡，D1「≤1/3」降级；
  E2 去 `i will` 198 处（⚠ 已落表语料里还有 **169 条**同款，风格会分两代，等裁）、E3 只补 1 条、
  **E4「缩短长句」被本轮 A/B 实测推翻并全部还原**（同 12 条：原句 8/12 可达 vs 缩短版 4/12）；
  B 14 条逐条判 ⇒ 4 条改写后可达（`报名已经截止`/`带上雨伞了`/`师傅在路口等我`/`正文别太多`）、
  9 条弃、`话筒没有电` 保留并标注位次 2。**批从 500 变 491 行**，`poly.py` 复算 491/491 mismatch=0，
  裁定后代理预检 **24/31 = 77%、位次 ≥2 = 1**。仍未落表。
- **C1 方案 C 落地（2026-10-14 16:00–16:58，代码改动 = `engine/src/assemble.rs` + 新增 `engine/src/reading-fixes.tsv`）**：
  读音修正表做成**独立 TSV 烘进 `.so`**（12 条：10 条错读音 `长时间/太长/很长/好长/不长/这么长/那么长/头发长/多长/睡不着`
  + `长的`/`不着` 两条歧义按上游政策出**双行**），在 `create_engine` 里一句
  `engine.set_extra_dictionaries(vec![fixes])` 挂上（`engine/setup.rs:495`，查词并集 `:505`），
  **只加行不删行**：随包 `dict.tsv` 复测 **`cmp` 仍与青简 `assets/lexicon/dict.tsv` 逐字节相同**，
  表/锁/A3 源件本轮一字未动，`host/java` 零改动。
  **驱动用例是被自己的假前提逼出来的**：我原以为"敲 `chang` 打不出长时间"，第一条测试一跑就绿
  ——真缺陷是 `taichang`/`henchang` **整表 380/442 格里根本没有「太长」「很长」**（合词路被
  「提倡」「合唱」这类同音高频压死），改成这两条后才当场红。
  单测 +6（共 51 passed / 0 failed，0 warning；`fmt --check` 仍 17 hunk 全在 `lib.rs`）：
  除两条驱动与两条回归守护，另两条把**数据完整性**（parse / 12 行 / 每个音节 ∈ 该字在 `dict.tsv` 的读音集 /
  词频 == 词典那条）与**排序不受影响**（挂与不挂，5 串无关输入的首页前 10 格逐格相同）钉住。
  两次证伪确认防线有牙：`chang`→`chajg` 红、词频 3234→3240 红；再临时断掉那一句接线，驱动用例当场回红。
  真机复测 8 条（`e1cdb946`，`install -r` 不 `pm clear`）：`changshijian`/`taichang`/`henchang`/`shuibuzhao`
  四条目标词**全部第 0 格**（其中「太长」「很长」是改前整表不出的那两条），
  `zhangshijian`/`taizhang` 旧路仍在第 0 格，`shuijiao` 睡觉 @0、`zhaojia` 着急 @3（正对照）；`FATAL` 0。
  设备日志给出规模与代价实测：**12 条 / 词频合计 94,886，占主词库 275,814,283 的 0.0344%**。
  `.so` 4,355,360 / 4,602,032 B（+3,568 / +4,032），APK（clean）**49,850,705 B**。
  过程中一次"空格上屏坏了"的假警报靠**断线对照**排除：挂与不挂表现相同，
  真因是我自创的"MOVE_END + 12 次 ⌫"清空序列（PIN 第 34 条②）；按上一轮原序列复跑，
  点选 `你好好` 与 `women`+空格 → `你好好我们` 都正常。设备已复位（四张学习表归档后删除，`total 0`，
  自动旋转已恢复）。全过程见 PIN「C1 方案 C 落地与真机复测」一节。
- **A3 第 5 批落表三合一（2026-10-14 17:30–18:10，数据与产物改动，代码零改动）**：
  按裁定**全落 491 条**，并把 **E2 的 169 条**（连已落表一起改，语料不留两代风格）与
  **B14 的 9 条**（第 4 批已落表的同音压过句）并进同一次落表。
  先确认了机制：`build_glossary_subset.py` **每次从「全量子集 + 源件」重建整张表**，
  所以改源件里的已落表行会照进新表，不需要额外的 in-place 更新。
  **源件 1,479 → 1,968 行；表 89,828 → 90,317 行 / 2,167,528 → 2,189,354 B（deflate +11,142 B）；
  锁同步重写；`.so` 尺寸不变（4,355,360 / 4,602,032 B，锁只是数值变了）；APK clean 49,861,845 B**。
  关卡与判据：**D2** `poly.py` 复算 **491/491 mismatch=0**；**D6** 落表前 40 条代理预检
  **前 5 格命中 25/40 = 62.5%、位次 ≥2 = 2 条**（阈值 5，未触发停手；与上轮 77% 是不同样本不同步长，不可比）。
  **E2 实到 166 + 3 = 169**（其中 3 条恰好也是 B14 整行改写的行，`assert n_e2 == 169` 第一版当场红，
  查清后改成两者之和）；落表后随包表 `i will` 残留 **0 条**。
  **B14 全部按实测处置**：9 条原句 9/9 位次 −1；改写句 6 条可达 + 二次尝试再救 1 条
  （`我下楼取餐` → `外卖到了`@0）⇒ **7 改写 + 2 删行**（`汤是凉的` 两轮改写都不出；
  `地址填错了` 与表内 `地址写错了`@0 同义）。
  **逐行对盘 `tablecheck.py`**：差异归类 新增 498／消失 9／同头改英文 166，**意外 0**（对不上就 exit 1）。
  `install -r` 不 `pm clear`：解包件 `glossary-en.tsv` **2,167,528 → 2,189,354 B**（refreshHostRaw ① 再验一次）。
  真机复验 9 条新批整句：**全部露出且第二行都带上了译词**（7 条位次 0、2 条位次 1）；
  对照 `nihao` 首页与方案 C 包逐格一致（`你好`·[`int. hello · int. hi`]、`你好像`·[`phr. you seem`]），
  `womenhuijiaba` 仍是 `我们回家啊`@0 / `我们回家吧`@1 ⇒ **A3 不改排序**；
  `default_input_method` 未变、`FATAL` 0、`dictionary_outdated` 稳定态 0 次、
  `会话建立 119 ms`；`cargo test` **51 passed**、`fmt --check` **17 hunk 全在 `lib.rs`**、
  `verify_symbols`/`verify_apk` 全绿。设备复位：学习表逐个显式名删（`total 0`）、自动旋转恢复。
  **一条预判被推翻**：「修正表救不了整句路」——`这里不能长时间停` 从"整表没有"变成**位次 2**
  （加词给组句图加了边），`安检队伍非常长` 仍 −1；由此产生两件待裁事项，见 PIN 待办第 14 条。
- **D3 读清 + 跨会话撤销评估（2026-10-14 20:16–20:50，只读 + 真机判别，代码净改动 0）**：
  **一句话结论：`si` 出 `zi` 不是模糊音，是键盘相邻的"敲错边"。** 青简把两套东西分开——
  `fuzzy/` 的 9 条读音混淆规则（`z_zh c_ch s_sh n_l f_h l_r an_ang en_eng in_ing`）**缺省全关**
  （`fuzzy/rules.rs:8-38`，`#[derive(Default)]` + `#[serde(default)]`，可经 `[fuzzy]` 配置开，
  **我们没接**：`grep -rn set_fuzzy engine/src/` = 0），
  `correction/` 的敲错边**默认参与**：`TypoCosts::DEFAULT`（`correction/costs.rs:37-45`）
  换位/替换 5.0、多敲/少敲 5.5、整段纠正罚 5.0、个人敲错表最多折 3.0。
  词级边的门槛"≥4 字母 + 切分完整 + 非双拼 + 非末尾不带简拼"（`engine/query/lookup.rs:18-26`）
  ⇒ **正确输入也会生成替代边**；整段纠错只在切分不干净时触发（`correction/mod.rs:39-48,69-73`）、
  只允许一处编辑、不严格更好就放弃（`engine/correcting.rs:69-70,90`）。
  真机判别 5 条（先按 `typo/mod.rs:17,46-66` 的键盘表算预测再去证）：
  `mihao`→**你好@0**（m↔n 相邻）、`lannvde`→**男女不出**（l/n 不相邻且 n_l 关）、
  `zhongwne`→**中文@0**（整段换位救活）、`xueyapiangao`→**血压偏好@0**（`偏高` 在 `dict.tsv` 里根本不是词）、
  `sijizailukoudengwo`→**自己@0**（复现）⇒ 天花板负面侧的主因是"替代边 + 词频"，不是模糊音。
  硬上限一并记：整句候选 **3 条**（`engine/mod.rs:372`）、重排窗口 6、词级 500、快照给壳 32
  ⇒ "整句不在表里"分两种成因（图上没边 / 进了图挤不进前 3）。
  **附加词库确实改变组句边**（`converting.rs:198` → `sentence/viterbi.rs:317-319` 对
  `all_dictionaries()` 逐词库查 span，边权 `:323` 直接取该词在自己词库里的 `frequency`）
  ⇒ C1 修正行沿用原词频是有意的，动它等于动组句权重。
  **留一个算不拢**：`ln(454875/14613) ≈ 3.44 < 5.0` 按公式该是"司机在路口等我"赢，实测"自己"占第 0 格
  ⇒ 下一轮第一件事是开观测口测"全表里到底有没有司机那条路径"（PIN 待办第 15 条），它卡住 A3 第 6 批的挑句判据。
  **（现值 2026-10-14 23:35：已用观测口测清——11 条输入的全表里 `Sentence` 一律恰好 3 条、固定在 0/1/2 格，
  `n=503` 出现两次 = 500 词 + 3 句；`司机在路口等我` **不在 54 行全表里** ⇒
  **格子剪枝这条解释排除，成立的是 k=3 的束宽剪枝**（`converting.rs:87-92` 把
  `wanted=SENTENCE_CANDIDATES` 直接当 k 交给路径生成器，`converting.rs:200-204` 无 scorer 时 `k = want`）。
  8 条已知 −1 离线判过可达性：**"图上没边" 0 条、"有边但挤不进前 3" 8 条**
  （见证切分 `司机/在/路口/等我` 等，见 PIN「算不拢测清…」一节 ③）⇒ A3 第 6 批的挑句判据只该防束宽，
  补词边救不了。**但"为什么 `司机` 那条路径连前三都进不去"仍未解释**：按
  `sentence/viterbi.rs:323` 的边权 `司机 14,613 × 1` 对 `自己 454,875 × e^-5 ≈ 3,065` 是司机赢，
  这一刀要看路径分数本身，候选表这个仪器看不到。**）**
  **跨会话撤销**：账本 `Engine::recent_commits: Vec<LastCommit>`（`engine/mod.rs:169`，容量
  `RECENT_COMMITS=4`）在 Core 里**既不序列化也读不到**（`commit/last.rs:5` 无 `Serialize`；
  `flush_learning`（`learning/mod.rs:188-194`）只 flush learner/logger/meter/vocabulary/translator
  五样，不含它；也没有 getter/setter）⇒ **不改 Core 就无法把账本恢复进引擎**。
  但 Core 已有 `Engine::swap_session(&mut EngineSession)`（`engine/session.rs:57-86`，
  `:63` 连 `recent_commits` 一起换）——**我们没用它**（我们是 `onFinishInput → destroy` 销毁制）。
  四个候选方案与代价见 PIN「跨会话撤销评估」一节；我倾向**甲：挂起而非销毁**
  （零 Java、零 Core、不新写隐私文本到盘），**等拍板，本轮一行代码没动**。
  另记两项裁定：**14① `这里不能长时间停` 不加回**；**A3 冻结在 1,968 条**，
  第 6 批必须"先定挑句规则再起草"（PIN 待办第 16 条）。
- **方案甲（跨会话挂起）实现轮：撞在"没有输入框身份"上，代码净改动 0**（2026-10-14 22:57–23:35）。
  native 侧收得到的只有 `resources` 与 `state_root`（`qingshan/engine/src/lib.rs:430-441` 只解这两个键，
  `core/Bootstrap.java:43-44` 写出的请求本身就只有这两个，`core/MSIMEInputService.java:1696` 建会话前
  只多塞一个 `phrase_preedit`）；包名 `currentEditorPackage`（`:1283-1284`）只被用来算 `dedicatedEnglish`，
  落到 native 的是两个**布尔**（`:1709`、`:1911`），不是身份。更硬的一条：**宿主自己也没有跨
  "离开再回来"的输入框标识**——`:1233-1235` 的 `newDocument = !restarting || id == 0`
  让回到同一个框也换新号，`:1238` 的 `onStartInput` 无条件 `stop(false)`，`stop()` 在 `:1595-1600`
  无条件 `destroy` ⇒ 「按输入框缓存 `EngineSession`」**没有可用的键**。
  三条出路（单槽启发式 = 能验①不能验②、按包名 = 要 Java 破例 1 行、延长会话生命周期 = 动不了），
  本轮**没有按启发式硬上**，等裁。细节与出处见 PIN「算不拢测清 + 方案甲卡在『输入框身份』上」一节 ⑥。
  **（现值 2026-10-15 01:26：用户裁**甲-2 按包名**并已落地，见下一条。）**
- **方案甲-2 落地：跨会话撤销按包名挂起输入上下文（2026-10-15 00:05–01:26）**。
  **Java 侧破例 1 行代码 + 1 行注释**（`core/MSIMEInputService.java:1697-1698`，
  把宿主已经算好的 `currentEditorPackage` 写进建会话选项的 `application` 键）；
  符号数仍 **102**（`verify_symbols.py` 两 ABI 双向差集为空），不改任何交互、不动 `stop()`/`destroy` 时机。
  与上游的总 diff 现为**两个文件**：本文件 4 行（两处）+ `UpdateJobService.java` 11 增 1 删（2026-10-15 关掉更新入口，其中 9 行是注释），逐行声明见 `NOTICE.md` §5a 第 1 条与第 7 条。
  Rust 侧：`lib.rs` 多解 `application` 与 `preferences.learning` 两个键（后者是 Java 在
  `learningSuppressed()` 为真时写的，`MSIMEInputService.java:1298`、`:1905-1907`）；
  `session.rs` 用 Core 现成的 `Engine::swap_session`（`engine/session.rs:57-86`）
  在建会话时换回、销毁时换出，**换出前先 `Engine::clear()`**（否则半截拼音会跟着回来），
  挂起表按包名、上限 8 份、按最久没再挂淘汰、**只在进程内存、不落盘**。
  两道闸：不许学习/隐私模式的框**既不换回也作废已有挂起项**（Core `engine/session.rs:90` 的边界）；
  包名为空 ⇒ 不挂不换（行为与引入前逐字相同）。
  **测试 51 → 59 passed**（6 条挂起用例 + 2 条身份解析用例；账本没有 getter，
  改用 `Engine::history()`（`engine/setup.rs:352`）当"上下文真回来了"的可见证据，
  它与 `recent_commits` 是同一次 `swap_session` 换进换出的两样）。
  过程中被测试抓住两个真问题：`derive(Default)` 会把 `learning_allowed` 弄成 `false`
  （等于把所有无身份会话当私密框）；挂起表是进程级 static ⇒ 断言条数的用例并发跑会互相污染，
  已加串行锁。`cargo fmt --check` 一度涨到 26 hunk，逐处手改成 rustfmt 形状后**回到 17 hunk、全在 `lib.rs`**。
  真机（最终包 APK 49,896,069 B）：空格上屏 `你好` ✓、⌫ 退字 ✓、
  HOME 离开 → `会话挂起 键="app.qingshan.ime"，表内 2 份` → 回来 → `换回挂起上下文=true` ✓、
  恢复后的会话再上屏/⌫ 均正常、切输入法回来 `换回=true` ✓、换到 `com.android.settings` 时
  **挂起键随包名变** ✓、FATAL 0、`outdated` 0、`我们回家吧` 仍在 `我们回家啊` 之后（位次 1 / 0）。
  **已知粒度限制（不是缺陷）**：键只有包名 ⇒ 同一个 app 里两个输入框共用一份挂起上下文。
  **路径 C（密码框）已于 2026-10-15 17:50 真机验过**——上一轮的洞是"本机没有可安全到达的
  `textPassword` 框"（`WifiAddNetworkActivity` 不存在、`SEARCH_SETTINGS`/`vpn.SETUP` 两个 intent 都 `unable to resolve`、
  我们自己的 `res`+`java` 里 `textPassword` 0 命中），本轮按用户裁定**自建 debug 专属测试页**补上：
  `host/gradle-app/app/src/debug/java/app/msime/android/TestPasswordActivity.java` +
  `src/debug/AndroidManifest.xml`（都在 Gradle 的 debug 源集里，release 清单实测 0 命中，见 `NOTICE.md` §5a 第 6 条）。
  实测：点密码框打 `nihao` ⇒ `会话建立` 计数**零增量**（只多一行 `会话挂起 键="app.qingshan.ime"，表内 1 份`）、
  无候选格；点回普通框 ⇒ 计数 +1 且 `换回挂起上下文=true`。与代码门逐字对得上：
  `policy/EditorPolicy.java:9-15` 的 `useEngine` 把三种 password variation 与非 `TYPE_CLASS_TEXT`
  全排除，`MSIMEInputService.java:1269-1270` 的 `engineWanted` 用它 ⇒ 密码框根本不建会话，
  挂起代码不参与；最接近的真机替身（不许学习的框）另有单测锁住。
  **两条别当证据的观测**：密码框那一步 `mInputShown=false` 是**系统对第三方输入法的策略**
  （换成出厂搜狗同框同结果，A/B 证过），不是青杉不弹键盘；dump 里那个节点的 `password="false"` 也不可信。
  另记一条观测口教训：`adb input keyevent 66` 在这台机器上**不是文本空格**，
  它会激活有焦点的工具栏视图并打开剪贴板面板（改前对照包行为逐字相同 ⇒ 不是本轮回归），
  上屏要点屏幕空格键 —— PIN 环境陷阱第 38 条。
- **MIN_LETTERS=4 小实验（只观测）**：`correction/mod.rs:26` 的 4 在两处生效
  （`lookup.rs:24` 词级敲错边、`correction/mod.rs:33` 整段纠正的长度窗）。
  判别式两例：`xan`（3 字母，s↔x 相邻）**不出**`三`、`toan`（4 字母）**出**`天@0`
  ⇒ 3 字母没有任何纠正路。**顺带抓到观测口自己的一个假阳性口径**：`fem` 的 `分@4`、`hoi` 的 `后@0`
  都是**简拼/前缀读法**给的（`ho` 是 `hou/hong` 的前缀），与纠错无关——
  用首屏格子判"有没有纠错"不可判别，要看候选的 `syllables` 或把错放在**首键**
  （记进 PIN 环境陷阱第 37 条）。
- **A3 第 6 批已起草、未落表（2026-10-15 01:50–02:35）**：草稿 **294 条**
  （`/d/toolchains/tmp/qingshan-snap/a3b6/batch6-draft4-clean.tsv`），来源是第 5 批剩下的
  316 条备用池（去重 0、多音字关卡剔除 6），派生拼音全部沿用脚本、未手敲。
  40 条真机代理预检的**露出率只有 5/40**（位次 0 四条、位次 2 一条、位次 −1 三十五条）
  ⇒ **这批落上去约 87% 是死行**。按新判据写的离线 k-best 复刻（音节格对齐 + 每词
  `-ln(total)-4` 常数项）经实测**只能当粗筛**：离线名次 1 的 10 条真机露出 5 条，
  名次 2 及更差的 30 条露出 0 条。疑问清单 33 条待裁（含"要不要改用新写的一批"）。
  本轮**表 / 锁 / `dict.tsv` / `.so` / APK 逐字节未动**。
  **（现值 2026-10-15 03:20：用户裁定第 6 批那 294 条不落表，离线判据正式降级为粗筛；
  理由与后续见 PIN 待办第 16 条的现值块与「A3 第 7 批起草」一节。）**
- **A3 第 7 批已起草、未落表，露出率 35/40（2026-10-15 02:51–03:15）**：起草阶段就按第 6 批
  露出那 5 条的形状写（高频双字词、5–8 字、虚词不密集、九档且**补齐情绪客套**），
  187 条 → `derive5.py` 派生 181 → `poly.py` **mismatch=0** → `path.py` 离线名次 1 **69 条**
  → 真机等距抽 40 条预检 **露出 35/40 = 87.5%**（位次 0 = 33、1 = 2、−1 = 5，`FATAL` 0 条），
  比第 6 批同口径的名次 1 那一层（5/10 = 50%）高一截，但**"形状控制"与"只留名次 1"两个变量没分开**。
  沉下去的 5 条全部是被近音/纠错读法抢（`司机`→`自己`、`能`→`梦` 首键相邻），不是图上没边。
  新查两个冗余口径：与现有 1,968 条源件**英文逐字撞车 4 条**、**中文头互为包含 6 对**。
  疑问清单 15 条（含"要不要再跑一组名次 2–6 的对照"）。本轮同样**表 / 锁 / `dict.tsv` / `.so` / APK 未动**。
  **（现值 2026-10-15 18:45：这批已落表，见下一条。）**
- **A3 第 7 批落表 63 条，真机可见（2026-10-15 17:55–18:45）**：用户裁定"直接落名次 1 的那批、
  不扩写到 200"。落表前先按两条**新加的机械冗余检查**裁了一遍与现有 1,968 条源件的重叠：
  **互为包含的 6 条删**（`司机在路口等我`、`买单可以刷卡吗`、`我要复查一次`、`风太大了关门`、
  `家里网络断了`、`酒店能开发票吗`）、**改写英文 14 条**（3 条消逐字撞车 + 11 条自审修正）⇒ 69 → **63**。
  关卡复跑：`poly.py` **63/63 mismatch=0**、同英文 0 / 互为包含 0 / 同头 0（`assert` 兜底）。
  **改写译词会引入新撞车**：`我马上到` 改成 `i am on my way` 正撞上已有的 `我在路上了`，
  再改 `i will be there shortly` 才干净 ⇒ 改完必须重跑查重。
  落表前再抽 40 条确认：**露出 35/40 = 87.5%**（位次 0=33、1=2、−1=5，`FATAL` 0）。
  正规通道一次走完：源件 1,968 → **2,031 行** → `build_glossary_subset.py` 一次写表与锁
  （随包表 90,317 → **90,380 数据行**、2,189,354 → **2,192,288 B**）→ `cargo ndk` 重编两 ABI `.so`
  （锁烘进二进制，实测新值命中 1、旧值 0；体积不变是因为两版字节数同为 7 位）→
  `verify_symbols.py` 仍 **102/缺失 0/多余 0** → `clean assembleDebug` **49,897,529 B**（+1,460 B）→
  `install -r` 不 `pm clear`。真机：解包件 `glossary-en.tsv` 跟上 **2,192,288 B**、
  `dictionary_outdated` 首启 2 行再启 0 行；试用框打 `wozaizhantaidengni` → 第 1 格
  **`我在站台等你` + 第二行 `i am waiting on the platform`**（位次 0）。`cargo test --release` **59 passed**。
  细节见 PIN「A3 第 7 批落表 63 条」。
- **签名链落地：第一个 release 包 0.2.0 / versionCode 2000（2026-10-15 18:17–18:50）**。
  自造青杉发布密钥（RSA 3072、10,000 天、`CN=Qingshan IME Release`、证书 SHA-256
  `8eb90f8dc8c4efc561c3d1743c53b0b2818444bba78d3bbc635f2d77b7627132`），**只在仓库外**
  `D:\toolchains\keys\`，`.gitignore` 挡住 `*.p12`/`*.keystore`/`*.jks`/`*.password`，
  口令不进命令行也不进 Gradle。新脚本 `host/scripts/sign-apk.sh` 做四步事后签名
  （zipalign → apksigner sign → verify → `zipalign -c`），缺密钥/缺口令直接失败、不退调试密钥。
  `assembleRelease` 实测出 `app-release-unsigned.apk` **45,966,895 B** → 签名 **46,070,982 B**、
  **v3 true / v1、v2、v4 false**，且 **`lintVitalRelease` 确实在发布路径上跑**（NewApi 门禁没被绕过）。
  **`run-as` 在 release 上失效**（`run-as: package not debuggable`，`/data/data` 也 `Permission denied`）
  ⇒ 读私有文件的判据只能在 debug 包上做，release 侧用 `logcat` + `uiautomator` + `dumpsys`（实测都活着）。
  跨证书覆盖安装必失败（`INSTALL_FAILED_UPDATE_INCOMPATIBLE`），本机已卸 debug 装 release 并验完金路径：
  点候选 → `我在站台等你`、`nihao` + 点空格 → `你好`、`FATAL` 0、日志 `译词表载入 68 ms / 2192288 B`
  ⇒ **发布包用的就是刚落表的新表**。当前装机是 **release 0.2.0**。细节见 PIN「签名链落地」一节。
  **（现值 2026-10-15 19:00：装机已切回 debug 0.2.0，为的是保住 `run-as` 那批文件类判据；
  切回 release 一条 `adb install` + 点两次弹窗即可，产物在 `a3bj/dist/qingshan-android-0.2.0.apk`。）**
- **GPL 源码分发完整性已评完，等你拍方案（2026-10-15 20:25–20:55，本轮未改 `.gitignore` 与 `NOTICE.md`）**。
  实测：被忽略的 11,124 个文件 / 2,581,685,903 B 里，**只有 2 个是"被忽略且随包分发"的数据**
  （`dict.tsv` 2,053,612 B + `glossary-en.tsv` 2,192,288 B = 4,245,900 B），其余是构建产物与 `.so`；
  仓内密钥文件 0 命中。**你简报里点名的 `reading-fixes.tsv` 与 A3 源件其实没被忽略**（在 `engine/` 下）。
  可复现性两条 `cmp` 都过：`glossary-en.tsv` 用 `build_glossary_subset.py` 在仓库外重建**逐字节相同**、
  `dict.tsv` 与青简 `assets/lexicon/dict.tsv` **逐字节相同**（青简那份**有 `.git`**，HEAD `4a3e963d…`）。
  ⇒ 依据 `LICENSE:59`「可自动重建的不必给」+ `:107`（§6d 要在分发处写清源码在哪），
  **真正的阻塞是没有一个第三方取得到的公开仓库地址，不是 `.gitignore`**。
  三方案（B 只写清重建路径 = 推荐 / A 只把 `dict.tsv` 进仓 / C LFS 不推荐）与代价见
  PIN「GPL 源码分发完整性评估」。**顺手量到 `NOTICE.md` 三处过期数字**（词库 1,815,026 B/76,273 行、
  锁 52 B、86 个桩 ⇒ 现值 2,192,288 B/90,380 行、167 B、85 个），按"本轮不动 NOTICE"留着等拍板一起修。
- **A3 分离变量实验做完了，判据可以定死（2026-10-15 20:40–20:52，数据零改动）**：
  另抽 40 条**形状合格但离线名次 2–6**（49 条过闸 44 条，等距抽 40）上真机 ⇒
  **露出 8/40 = 20.0%，且没有一条进第 1 格**（位次 1 有 6、2 有 2、−1 有 32）。
  对照名次 1 那批的 87.5%（33/40 在第 1 格）与第 6 批无名次筛选时的 0/20：
  **形状闸有效但弱（0% → 20%），`离线名次 = 1` 才是决定性的** ⇒ 第 8 批判据定成两道都要。
  两条混杂写在 PIN 同节（本轮跑在已落表 63 条之后的包上）。第一次跑废了：手机中途转横屏
  ⇒ 候选条整条不显示、40 条全记 −1，看着像"这批全沉"（环境陷阱第 47 条；作废日志留档）。
- **README 表格断行已修**（本轮任务）：`产物实测` 表 `app-debug.apk` 那一行此前被拆成
  162–171 共 10 行（快照证明是既有问题，不是本轮造成），已合回单行并断言竖线 = 3（该表是两列）；
  同一行开头的体积从 49,846,673 B 更新到现值 **49,897,529 B**（并补上甲-2 与第 7 批两级历史值）。
  审计复跑 **368 条与上一轮终值逐行完全一致**（`diff` 空输出）⇒ 合行没动任何引用。
- **本地建仓 + 首次提交完成，GPL 方案 B 落地（2026-10-15 21:27–21:55）**：
  `git init -b main` → 首次提交 `db5ad5a`（**579 文件 / 92,024 行**，作者用的是这台机器已有的
  全局 config，**没改任何 git 配置**）。提交前五条安全核查全过（仓内密钥 0 命中、暂存清单里
  构建产物/APK/快照 0 命中、秘密扫描只命中两处文档占位示例），过程中抓到并修掉两件事：
  `git add -A` 把 `engine/tools/__pycache__/*.pyc` 收了进来（`.gitignore` 补 `__pycache__/`+`*.pyc`，
  重暂存后 580 → 579）；这台机器 `core.autocrlf=true` 会把 `host/scripts/sign-apk.sh` 检出成 CRLF
  让 Git Bash 跑不动（新增 `.gitattributes` 钉 `*.sh eol=lf`，`git check-attr` 复验）。
  **§5 现在写的是"从哪取 + 怎么重建"**：两个上游地址都是查出来的（青简 = 本地那份的
  `git remote -v`，HEAD `4a3e963d…`；水杉 = 其 `README.md:8` 的仓库链接，本地那份仍靠 zip MD5 固定），
  两份 TSV 的重建路径有逐字节 `cmp` 依据，三处过期数字（词库 / 锁 / 桩数）已修。
  **⚠ 还差一步**：青杉自己的公开地址在 NOTICE 与「构建前提」里都是「⚠ 待填」占位，
  push 由用户执行（命令见 PIN「GPL 方案 B 落地」一节末尾），agent 不接触凭据。
- **「检查更新」入口已按方案 b 关掉（2026-10-15 21:47，第二个提交）**：宿主 Java 树**第 3 处破例**，
  `UpdateJobService.java` 11 增 1 删（命名开关 + 注释块，不是直接 `return true`）。
  编译退出码 0；**本轮没重打 APK ⇒ 关于页那三样是否真的消失，尚未真机验**。详见已知失效项第 11 条。
- **横屏下候选条整条不显示 = 新发现、未定位**（2026-10-15 20:35 撞见，本轮按裁定**只记不测**）：
  `mCurrentOrientation=90` 时键位正常、会话活着、候选面板一个节点都没有，锁回竖屏立刻恢复。
  已记 PIN 待办第 21 条与环境陷阱第 47 条；下一轮先确认"是不是只有试用页这样"。
- 候选长按管理菜单（B1/B2 真机口径，2026-10-13 12:00 复测）：**B1 三件从「静态 + 单测」
  升级为真机入口可达**——「优先显示」`pinCandidateRaw`（`qingshan/engine/src/lib.rs:286`）、
  「固定到首位」`fixCandidatePositionRaw`（`:293`）、「取消固定」`clearCandidatePositionRaw`（`:301`）
  各点一次，`:ime` pid 全程 17874 不变、时间窗内 `FATAL EXCEPTION` 0 条，菜单关掉后
  点候选条 → 输入框 `nihao 你好hao`、按空格 → `nihao 你好好`，会话活着。
  **验的是"入口可达 + 不杀会话"，不是"功能生效"**：三件都走 `no_op`（`:277`，
  回原样 View + `handled:false`），候选顺序不变正是设计结果。
  （这四个行号 2026-10-14 00:2x 又复量过一次：B1 记录当时是 `:278`/`:285`/`:293`/`:269`，
  加 `mod resources_lock;` 与两行模块文档后下移 2 行 = `:280`/`:287`/`:295`/`:271`，
  `run_transition()` 里的学习落盘钩子再加 6 行 ⇒ 现值 `:286`/`:293`/`:301`/`:277`。
  **`lib.rs` 一动手就得复量这四个引用**，审计脚本抓不到（它锚到 msime-develop 的同名孪生）。）
  **B2（删除词条）不升级**，仍是静态 + 单测：它在到达任何 native 调用之前就把 `:ime` 崩掉
  （宿主 `MSIMEInputService.java:6051-6058` 的确认框），真机拿不到可判定的结果。
  以上全部是 `adb input` 注入，**未做真人点击**，注入与真实触摸的等价性按 PIN 第 23 条 ④ 保留。
- 已确认的降级面（预期内，不是新 bug）：键盘右上显示「设置读取或应用失败，保留当前设置」
  （偏好读写属桩），日志里 `MSIMETelemetry`/`MSIMEResourcePacks`/`MSIMEBootstrap` 各条
  `not_implemented`，主页各分组行显示「读取失败」。

**清数据后的首次运行已单独验过**（`pm clear app.qingshan.ime` 之后）：引导页重新出现且两步
都 ✓，`files/runtime-options.json` 重新写出且三个路径正确，`bootstrap/resources/dict.tsv`
重新解出 2,053,612 B，进试用键盘打 `nihao` 照样出候选条（`你好 你好好 你好像 …`，页码 1/5）。
日志里 `refreshHostRaw` 桩会留一条 `Runtime options refresh failed: error`，被
`Bootstrap.java:380` 吞掉，不影响首启。

**仍未验证的**：

1. Android 15+ 的 16 KB page size 内核：本机是 13，跑不了真机。**归档层已证**——
   `zipalign -c -P 16 -v 4` 退出码 0，两个 `.so` 都是 `(OK)`；没证的只是设备加载。
2. 「已知失效项」第 3 条里 account / voice / handwriting 三个入口的实际界面表现。
3. 长时间使用与多次切换输入法/编辑器的稳定性（本轮共约 40 次按键与 3 次面板开合，无崩溃）。
4. 青简 Core 自己的 `warn!` 能不能落到 logcat。桥按 level 过滤、不看 target，理论上一定落。
   **2026-10-14 00:38 起本项半关闭**：`flush_learning` 接上之后，Core 侧
   `qingjian_learning::frequency_learner::learner_impl` 的 `info!`「用户词频已保存」
   在设备 logcat 里实测出现（00:38 / 00:41 / 00:42 三次），**证明 Core 的 target 照样过桥**；
   原文里「构造不出触发条件」与「`files/qingshan/` 是空的」两个前提都已不成立（现在有三张表）。
   仍缺的是一条**真实的 `warn!`**（例如 `FrequencyLearner 构造失败` 或保存失败）——
   它与 `info!` 走同一条 level 过滤，但没实测过，所以这一半留着。见
   `third-party/PIN.md`「flush_learning 落盘」一节。
5. panic 的两条日志（panic hook 一条 + `wire.rs` FFI 边界一条）没实测过。造它得往代码里塞
   临时 `panic!`，成本大于收益，按裁定记「未实测」。两条的文本互相指认，理由见
   `third-party/PIN.md`「日志桥」一节的第 3 条风险点。
6. **候选条点选——本项已关闭（原记「不进 JNI」被本轮推翻）**。
   原记录（2026-10-13 02:2x）：`adb input tap` 点候选条、`selectRaw` 入口的临时探针 0 条
   PROBE，据此挂起本项，并列出两种可能：① adb 注入手势与真实触摸不等价；② 宿主候选条
   触摸处理另有缺口。
   本轮重做（同一台 `e1cdb946`，探针重新加在 `selectRaw` 与 `selectAnyCandidateRaw` 入口）：
   `input tap 104 1701` → `PROBE selectRaw enter s=1 g=9 i=0` + `hit=true cur_g=9 表长=44`，
   输入框回读 `text="你好"`；`input swipe 104 1701 104 1701 50` 同样触发；面板格
   `input tap 609 2082` → `PROBE selectAnyCandidateRaw … i=10 hit=true`，输入框 `尼黑ao`。
   **两个候选解释都不成立——上一轮的前提本身是错的。** 已确认的干扰源两条：
   ① `input motionevent DOWN` 与 `UP` 分两次命令发时不构成一次连贯手势，0 PROBE 属注入方式
   的假阴性；② 候选格中心随布局漂移（候选加上英文释义第二行后，第 1 格中心 x 从 63 变 104），
   复用上一轮坐标会落到格间空白。上一轮 0 PROBE 的当时状态**未复现**：那几次点击时键盘下方
   显示的是剪贴板页而非键区，覆盖面板是否吃掉候选条触摸**未核实**。
   干净版复验（探针已删、`logcat -c` 清缓冲后）：点第 1 格 → 输入框 `你好`，PROBE 0 条。
   （2026-10-13 02:24 那条「长按面板格弹菜单 / 候选条同坐标长按弹剪贴板页」的补注保持原文；
   本轮另记：候选条格长按**不弹菜单**，与 `ImeCandidates.java:167` 的
   `candidateManagementEnabled()` 门控一致——键盘右上一直挂着「设置读取或应用失败，
   保留当前设置」，偏好读不到。该门控的实际开关值**未核实**。）
   **另：紧接上面括号里那条「候选条格长按不弹菜单，疑为 `candidateManagementEnabled()`
   门控关闭」已被同日稍后的复测推翻**——长按 `input swipe … 1500` 菜单五项正常弹出，
   900ms 才不弹；且那两个门控（`MSIMEInputService.java:5979` 与 `:6069`）**不读任何偏好**，
   只看我们自己 View 的 `local_mode`（我们发 `"none"`，`engine/src/view.rs:194`/`:330`）与 `scheme`
   （我们发 `SCHEME_QUANPIN = 0`，`engine/src/view.rs:154`），而 `:5979` 拒绝的是 scheme 2/3/4/5/6/7/8/9，
   0 通过 ⇒ 恒为 true。所以「偏好是桩 ⇒ 菜单不可达」这条推断链根本不成立。
   详见「已知失效项」第 7 条的复测记录与 PIN 环境陷阱第 23 条的 ④。
   **2026-10-13 13:40 本项关闭。关闭依据是用户手动验证，不是 event1 硬件级直证。**
   用户亲手操作设备后确认：候选条点选可上屏、打字正常、候选旁译词正常显示。
   据此，「adb 注入 ≡ 真实触摸」这条等价性**以用户的口头确认为准，不再追硬件级证据**，
   也不再要求用户复现。当天为取证做过的事记在这里备查：`/dev/input/event1`
   （name `touchpanel`）在 shell 权限下可读、且 `sendevent` 被 SELinux 拒写，
   所以该节点的事件无法由 adb 伪造；捕获器抓到过**一次真手指**
   （`BTN_TOUCH=1` + `ABS_MT_POSITION_X/Y` = 3601/9186，÷4 → 屏幕 (900, 2297)，
   实测该处键块为 x 883-1064 的右下角橙色键，即无组字时的 `↵`、有组字时的「确认」），
   但**候选条格那一下没抓到**——捕获窗口 13:27 到点自动退出，用户 13:29:59 才进试用页
   （`logcat` 两条 `会话建立`）。三个捕获器缺陷已记 PIN 第 24 条 ①②③。
   **JNI 级直证按推断记**：候选格点击 → `NativeClient.select` → `selectRaw` →
   `Engine::commit` 是把汉字送进输入框的唯一路径（候选条格绑 `selectVisibleCandidate`
   在 `ImeCandidates.java:163`，它调 `NativeClient.select` 在
   `MSIMEInputService.java:6264-6265`），所以「输入框出现 `你好`」蕴含 `selectRaw` 被调用过；
   **这是推断，本轮没有 `PROBE` 行做直接观测**。
7. **可达率 78.3% 的口径边界（2026-10-13 18:07）**：① 23 条样本是我按「2/3/4/5 段 + 虚词 +
   上轮对照组与失败样本」挑的，**不是从 1,000 条语料里随机抽**，外推到全语料只是估计；
   ② 「展开面板」这一档我用的是**引擎全表**（实测 `n` 最大 503），没验过人开面板能不能滚到
   第 300 行，所以三档同值只说明「命中的都排在表首」，不代表面板的真实可及范围；
   ③ 打字是 `adb input keyevent` 注入，但读的是引擎自己吐的候选表，与第 6 条那个
   「注入 ≡ 真实触摸」问题无关；④ 探针版与干净版的候选渲染做过逐格对盘（见上面测量条目），
   所以「探针扰动排序」这条**已排除**，不再挂未验证。

签名链还没接，但**流程已经评完**（2026-10-15 03:20，见 PIN「签名链与发布准备评估」一节）：
debug APK 用的是自动生成的 `CN=Android Debug` 证书（实测 v2 true / v1、v3 false，
库文件 `D:\toolchains\android-sdk\.android\debug.keystore` 是 **PKCS12** 不是 JKS）。
release **不沿用上游 `platforms/android/scripts/signing.sh`**——那把 `msime-release` 密钥属
水杉的 `app.msime.android`、不在我们手上，且脚本依赖 macOS `security` 与无扩展名的 `apksigner`，
本机两条都不成立。定下来的做法是：新造一把青杉自己的发布密钥（发出第一个包之后就换不掉了）、
另写 `host/scripts/sign-apk.sh`（该目录现在是空的）按 zipalign → apksigner → verify → `zipalign -c`
四步事后签名，不把 `signingConfig` 写进 Gradle。**本轮按约束没出过 release 包**，
所以 release 产物名与 `assembleRelease` 上的 lint 门禁都还是未测项。
