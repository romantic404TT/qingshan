# 上游固定依据

融合项目取两个上游的本地拷贝，不重新 clone（本机 GitHub 直连不稳）。

## 青简 Qingjian（Rust Core）

- 路径：`projects/qingjian`
- HEAD：`4a3e963dfe299126574bfe3c370ff70a3f85ee77`
- 提交：`chore(release): 开发版 0.2.1-dev`，2026-10-10 14:16:07 +0800
- remote：`https://github.com/romantic404TT/qingjian`
- 许可证：GPL-3.0-or-later（原文见 `../LICENSE-QINGJIAN`）
- 本项目只读引用其 `crates/qingjian-{core,dictionary,format,learning,lm,translate}`，**不修改青简仓库任何文件**。
- 注意：`projects/qingjian/apps/android/` 是本地未跟踪的骨架，不在上述 HEAD 里，本项目不依赖它。

## 水杉 MSIME（Android 宿主）

- 路径：`projects/msime-develop`
- **无 git 元数据**：该目录由 `projects/msime-develop.zip` 解包而来，`.git` 不存在，因此给不出 commit 号。
- 固定依据改为归档校验值：
  - `msime-develop.zip` MD5 `cfdab2624a8e6469b588984477f1ed65`，29,461,249 字节
  - `platforms/android/version.txt` = `0.3.1`
- 许可证：**GPL-3.0-only**（`projects/msime-develop/README.md:136`「源码为 **GPL-3.0-only**」，
  `:3` 的 badge 同样是 `GPL-3.0-only`；`LICENSE` 正文是未带 “or later” 的 GPL-3 全文）。
  本项目 `LICENSE` 是这份的逐字拷贝。
- 本项目复制 `platforms/android/{java,res,AndroidManifest.xml,version.txt,gradle-app}` 与 `apps/desktop/src-tauri/gen/android` 的 Gradle wrapper 三件套，**不修改水杉仓库任何文件**。

## 构建脚本里刻意保留的上游命名

- `-PmsimeVersion`（读 `host/version.txt` 之外的覆盖入口）属性名**未改**成青杉风格，
  目的是最小化与上游 `gradle-app/app/build.gradle.kts` 的 diff。它只是构建脚本内部属性，
  不出现在装机标识或界面文案里。
- `namespace` 保持 `app.msime.android`（394 个 `.java` 的包名 + `AppEdition` 反射的
  `app.msime.android.BuildConfig` 类名都依赖它）；只有 `applicationId` 改成 `app.qingshan.ime`。
- 水杉自带的版本号 `0.3.1` 在这里留作溯源；本项目版本号从 `host/version.txt` = `0.1.0` 起。

## AGP 版本锁定的实据

AGP 固定 8.13.2，不可顺手升级。上游水杉曾在 2026-10-02 由 Dependabot 升到 9.4.1，
导致 release-android 编不出 APK。本项目沿用同一条约束（Gradle wrapper 8.14.3，
AGP 9.x 要求 Gradle 9.6+），升 AGP 会直接破坏构建。

> 这条历史证据的来源是上游 `platforms/android/gradle-app/build.gradle.kts` 的注释原文
> （2026-10-02 Dependabot 升到 9.4.1 后 release-android 编不出 APK）。它是水杉自己的
> Dependabot/CI 事件，不是本项目发生过的事故，所以记在这里作溯源，不写进代码注释——
> 代码注释只描述本项目的成因（见 `host/gradle-app/build.gradle.kts`）。

## 环境陷阱（构建前必读，全部实测）

1. **`python3` 是商店占位符，不可用于验证。**
   `command -v python3` → `C:\Users\Lenovo\AppData\Local\Microsoft\WindowsApps\python3`，实体是
   `AppInstallerPythonRedirector.exe`：执行后**不打印任何东西，退出码 49**。用它跑校验脚本，
   "空输出"会被误读成"没命中"，从而得出假的干净结论。
   本机真实解释器是 `python`（`C:\Program Files\python`，3.8.6）和 `D:\Anacnada\python.exe`。
   `env.sh` 已把 WindowsApps 从 PATH 摘掉，让 `python3` 变成响亮的"命令未找到"；
   副作用是该 shell 里 `winget` 也不可用。
   另：WindowsApps 在 PATH 第 64/71 段，排在 `/usr/bin`（第 23 段）**之后**，所以它不抢
   grep/sed/curl，只影响 coreutils 里没有对应物的命令名。
2. **`link.exe` 被 coreutils 抢占。** Git Bash 自带 `/usr/bin/link.exe`（`ln` 的同族命令），
   排在 MSVC 之前时，cargo 会报 `link: extra operand ...`，看起来像 MSVC 装坏了。
   `env.sh` 的 MSVC 前置段把 `VC\Tools\MSVC\*\bin\Hostx64\x64` 放到 PATH 首位；
   自检命令：`command -v link.exe` 必须指向 `D:\toolchains\vs-buildtools\...`。
3. **`source env.sh` 后面不能接管道。** `. /d/toolchains/env.sh | tail -1` 会让整个 env.sh
   跑在子 shell 里，所有 export 丢失，表现为 `cargo: command not found` 且 `link.exe`
   又退回 `/usr/bin/link.exe`。正确写法：`. /d/toolchains/env.sh > /dev/null 2>&1`。
4. **取退出码不要接管道。** `cmd | head -2; echo $?` 读到的是 `head` 的状态码。
   要 `cmd > /tmp/o 2>/tmp/e; echo $?`。这条和第 1 条叠在一起时，静默失败会显示成 exit=0。
5. **验证纪律**：用任何工具去证明一个结论之前，先 `command -v <工具>` + 跑一个最小正样例
   确认它真在干活；得出"零命中/全部一致"这类否定结论时，换第二种独立方法复核一次。
6. **rustfmt 在重定向到文件时仍输出 ANSI 颜色码**（减行是 `\x1b[31m-`，加行是 `\x1b[32m+`），
   所以 `grep -cE '^[+-]'` 会静默返回 0。统计 rustfmt diff 前先剥色
   （`sed -E 's/\x1b\[[0-9;]*m//g'`），或改用 `cut -c1` 看首字节分布（`D` 前缀 = hunk 头，可靠）。
7. **空结果必须先证伪，且工具/文件路径不要重打。** 三个实例：
   ① `llvm-nm` 上一轮用 `find` 取到正确路径并跑通 `--version`，这一轮手敲时漏掉
   `prebuilt/windows-x86_64` 一段，命令不存在却被 `2>/dev/null` 吞掉，管道输出空集，
   `wc -l` 报 0，看起来像「产物里没有符号」，据此生成的比对行全部无效；
   ② `wc -l ../../msime-develop/...` 多退一级目录，同样被 `2>/dev/null` 吞掉，
   误读成「上游没这个文件」；
   ③ `find -newermt` 凭记忆估时间窗，三次得到假的「无文件改动」，其中一次窗口设在了
   **未来**（`date` 显示 00:54，却用 01:00）。
   **对策**：校验类命令不接 `2>/dev/null`；路径一律复用 `find` 结果或重新 `find`；
   得到 0/空之前，先用一个已知非空的样例证明命令本身在干活；时间窗先 `date` 再取；
   **探针一律先 `cd` 到中性目录**（`$LOCALAPPDATA/Temp`）——Bash 的 cwd 在命令之间是保留的，
   在水杉仓里跑 `rustc` 会命中它的 `rust-toolchain.toml`（钉 1.97.1），rustup 于是**静默安装整套**
   （实测 `.rustup` 719M → 1.4G）。`env.sh` 已加 `export RUSTUP_AUTO_INSTALL=0` 拦住，
   负样例：`rustup run 9.9.9-… rustc --version` 现在报 `toolchain … is not installed` 且**不下载**。
8. **`page_size` 固定 10；`page_count` 的 `div_ceil` 有两个坑。**
   上游算法是 `input-runtime/src/runtime.rs:754`
   的 `page_count: self.cached.candidates.len().div_ceil(self.page_size)`，配 `:701`
   的 `page = highlighted / page_size`。① **除数为 0 直接 panic**，所以本项目不开入参，
   `page_size` 恒为 `MAX_CANDIDATE_PAGE_SIZE = 10`（水杉 `client-core/src/preferences.rs:3373`，
   类型是 `u8`，用 `as usize` 转）。上游确实把它当可配值——同文件 `:2608` 的
   `if !(1..=MAX_CANDIDATE_PAGE_SIZE).contains(&self.candidate_page_size)` 就是那个下限；
   我们固定 10 **不是图省事，而是收不到这个偏好**：读写偏好的 native 入口属于 85 个（当时 86 个）
   `not_implemented` 桩。② **空候选表时上游给 `page=0, page_count=0`，不是 1**
   （`0usize.div_ceil(10) = 0`，实测边界 `1→1 / 10→1 / 11→2 / 25→3`），本项目照抄。
9. **`cargo fmt` 写模式会顺手改写整个 crate。** 只想规范化新写的 `session.rs` 时跑了
   `cargo fmt`，结果 `lib.rs` 的 8 个刻意紧凑的 JNI 签名与 `view.rs` 的 6 处测试排版一起被展开
   （lib.rs 88 → 110 行、view.rs 278 → 367 行）。`lib.rs` 用「按 `pub extern "system" fn X(`
   收参数行、合并回一行」的脚本还原，校验目标是**行数回到 88、`cargo fmt --check` 回到 8 hunk**
   （现值：`lib.rs` 437 行 / 16 hunk，2026-10-13 17:05 复测：`wc -l` 与 `awk END{print NR}` 均 437，
   末字节是 `\n`。**关于此前那句「现值 436 行」的裁定（2026-10-13 18:10）**：最早可用的 lib.rs 快照
   `qingshan-snap/lib.rs.before-strip-probe`（11:20 存）**与当前逐字节相同**（`diff` 输出 0 行）、
   都是 437 行 ⇒ 11:20 之后 lib.rs 内容没再变过（mtime 11:29 那次写入是内容中性的）；
   而「436」在最早可用的 PIN 快照 `PIN.md.before-item20`（10:41）里就已经存在。
   10:41→11:20 之间 C 批往 lib.rs 里加了 32 行（现 `lib.rs:345-376`，实测跨度），
   所以**若 436 是 10:41 当时量的，之后应当涨到 ~468 而不是停在 437** ⇒ **436 至少从 11:20 起就是错的**。
   它在更早（10:41 之前、没有任何 lib.rs 快照的那段）是否对过：**无法判定**，不猜。
   教训同第 27 条：「现值」这类活断言要么当场带时间戳实测，要么别写），
   并逐字比对还原后的参数行与还原前 diff 里的 `-` 行一致；`view.rs` 的原始排版没有备份
   （本工程不是 git 仓库），**不可逐字节复原**。
   **对策**：本项目只用 `cargo fmt --check`；要规范化单个新文件用
   `rustfmt --edition 2024 src/<file>.rs`，或先 `cp` 快照再跑写模式并当场 diff。
10. **`packageDebug` 重跑时，增量 `assembleDebug` 的 APK 体积虚高约 4.5 MB。** 实测序列
    （同一台机器，相邻两次构建之间只换 `.so`）：
    ① 首次构建（等价于从零打包）= 48,674,350 B；② 换 `.so` 后**不带 clean** 重打包 =
    **53,171,668 B**；③ 紧接着 `clean assembleDebug` = 48,674,462 B；④ 再换 `.so` + clean =
    48,674,758 B；⑤ 本轮修复后 clean = 48,675,382 B。
    ② 与 ① 的条目名集合完全相同（1202 条，双向差集为空），dex 与各 `.so` 的 `header_offset`
    逐个对得上，`lib/*` 四个 `.so` 两边都在——**多出的约 4.5 MB 不在任何条目里**，
    没继续追它落在哪一段。复现条件也量过：37 个 task 全 up-to-date 的那一次增量构建尺寸没变，
    所以只有 `packageDebug` 真的重跑才虚高。**结论：体积数字必须来自 clean 构建。**
11. **`apksigner` 是 `.bat`，`$JAVA_HOME` 没 source 时报的是「文件不存在」。** 未 source
    `env.sh` 的 shell 里 `"$JAVA_HOME/bin/java" -jar …/apksigner.jar` 展开成 `/bin/java`，
    退出码 127，看起来像 apksigner 装坏了。另外 `verify --verbose 1 <apk>` 会把 `1` 当成
    APK 路径，报 `Unexpected parameter(s) after APK`；`--verbose` 不吃参数，
    用 `verify --print-certs <apk>` 就够了。
12. **真机操作的四个坑（本轮全部踩过一次）。**
    ① **`am force-stop` 与 `pm clear` 都会让系统把默认输入法回退掉**：两次实测
    `settings get secure default_input_method` 从青杉变回 `com.sohu.inputmethod.sogouoem/.SogouIME`。
    所以"默认输入法不是我们"不等于"我们崩了被系统踢"——先查 `logcat` 有没有 FATAL，
    再回想自己是不是刚 force-stop 过。恢复用 `adb shell ime set app.qingshan.ime/app.msime.android.MSIMEInputService`。
    ② **`uiautomator dump` 默认只 dump 应用窗口**（21-25 个节点，全是 Activity 的），要加
    `--windows` 才看得见 IME 窗口的控件（加上后 110 节点 / 3 窗口，工具栏「收起」拿到
    `[925,1624][1072,1735]`）。键位行仍是自绘、无节点，得靠截图。
    ③ **`screencap` 的 PNG 与设备像素 1:1**（`wm size` = 1080x2400，PNG 也是 1080x2400），
    但**肉眼看被缩放过的图会偏 100-200 px**——我两次把 ⌄ 点到面板里就是这个原因。
    坐标一律从 `uiautomator --windows` 的 bounds 或 `dumpsys window` 的窗口帧推，不要目测。
    ④ **"设备已解锁"要先 `dumpsys power | grep mWakefulness=` 证**：本轮实际是 `Asleep`，
    `wm dismiss-keyguard` 与 `keyevent 82` 白跑，正解是 `keyevent 224` 唤醒。
    另外 Git Bash 下 `export MSYS_NO_PATHCONV=1` 之后，`adb install <本机路径>` 反而会因为
    `/d/...` 不再被换算而报 `failed to stat`——**设备侧路径**要 `MSYS_NO_PATHCONV=1`，
    **传给 adb 的 Windows 文件路径**要写成 `D:\\...`。
13. **`android/log.h` 的优先级枚举记反了，只有真机能发现。** 日志桥第一版按
    「ERROR 3 / WARN 4 / INFO 6 / DEBUG 7」写映射，NDK sysroot 的实际枚举是
    `VERBOSE 2 / DEBUG 3 / INFO 4 / WARN 5 / ERROR 6`（`usr/include/android/log.h:76-90`）——
    正好反了，真机上三条 `info!` 全被打成 `E`。宿主侧走 `eprintln!`，一行都看不出来。
    同一个坑的第二半：`__android_log_write(int, const char*, const char*)` 在 bionic 上
    `c_char` 是 `u8`（host x86_64 是 `i8`），extern 签名写 `*const i8` 直接 E0308。
    两处都只在 `cfg(target_os = "android")` 里，**`cargo test` 永远编不到它们**——
    见下面「日志桥」一节的验证口径。
14. **`export MSYS_NO_PATHCONV=1` 会连带关掉环境变量里路径的换算** ⇒
    `ANDROID_HOME=/d/toolchains/android-sdk` 原样交给 java ⇒ Gradle 报
    `SDK location not found`。**gradle 与 adb 不共用一条命令；链中每步查退出码**。
    （更糟的是用 `;` 串命令时，失败后 `verify_apk`/`zipalign`/`install` 会跑在上一版
    APK 上——`delta = 0` 才暴露。）
15. **`mWakefulness=Awake` ≠ 已解锁**；判据是 `mDreamingLockscreen`。睡眠屏那一半已
    在第 12 条④记过，这次补上另一半：**醒屏后仍可能锁屏**。
16. **坐标不能跨轮复用**：同一按钮在键盘弹出/收起时 bounds 会漂（`keyboard_try`
    在 `[82,957]` 与 `[82,978]` 之间）。**每轮重新 `uiautomator dump`**，不复用上轮坐标。
17. **`svc power stayon true` 会阻止屏保**，调试期间开，测完关回 `false`——否则下次
    接手的调试者可能不知道设备为什么没睡。
18. **清理声明也须复核**：一句「已删」不等于命令里真有 `rm`。写「已删」前用
    `ls <路径>` 或 `grep -c` 复核；同理「已改」前用 `diff` 复核（本项目不是 git 仓，
    没有 `git diff` 可用，只能拿快照 `diff`）。
    （2026-10-13：报告里两条「已删」实际未删——`/tmp/PIN.before-note.md` 与
    `/tmp/PIN.before-p0.md`，那两条命令里没有 `rm`，查 `ls /tmp` 才发现。）
19. **状态声明必须经命令复核**：报告里的「已改」、「已删」、「已更新」必须附 `diff` /
    `ls` / `git status` 的输出，不能把「打算做」当成「已做」。2026-10-13 一次报告里
    写了「锁已更新，审计从 158 推到 160」，实际锁编辑没发生、数字凭空写。
    发现方式：事后 `ls` 快照目录 + 重跑审计。
    （与第 18 条同族：18 是「命令跑了但没含那个动作」，19 是「命令根本没跑」。
    19 更危险，因为叙述自洽、数字看着合理，只有回命令复核才戳得穿。）
20. **`/tmp` 里的临时快照不能当长期证据**：2026-10-13 一次跨时段复核发现
    `/tmp/doc-snap/` 里 03:38 之前存的 before-snapshot 全部不见了，09:34 之后存的 4 份还在。
    **归因未查明**——「Windows 定期清理 Temp」这个说法当场被证伪：同一个 Temp 目录下
    `HeadlessEdge*`（10-11）等 56 个早于当天 09:00 的条目都还在，清理不是按年龄全局发生的。
    机制不知道，但**「快照会消失」这件事本身是真的**，所以按最坏情况处理：
    ① 跨时段工作的 before-snapshot 落在项目外、且不是 Temp 的目录（本机用
    `/d/toolchains/tmp/qingshan-snap/`）；② **结论只依赖改动当场打出的 `diff` / `grep` 输出**，
    不依赖「事后还能回看快照」。第 19 条要求命令输出，这条要求那份输出别存在会消失的地方。
21. **`am force-stop` 我们自己的 IME 会把系统默认输入法切走**：2026-10-13 第一次做「移走
    译词表看 warn」的真机反证，全程废掉——force-stop 之后 `input text nihao` 打进了
    **搜狗**的编辑面板（截图是「选择/全选/复制/粘贴」那个圆盘），`adb logcat -s qingshan`
    **0 条**，而我一度判成「新代码没生效」。判据：`settings get secure default_input_method`；
    恢复：`adb shell ime set app.qingshan.ime/app.msime.android.MSIMEInputService`。
    **凡是 force-stop 之后的真机步骤，先复核这一条。**
22. **译词表只在 `createRaw` 读一次，「收起键盘再点开」不重建会话**：把
    `bootstrap/resources/glossary-en.tsv` 移走后按「收起键盘」再点输入框，warn 继续响——
    因为日志里最后一次 `会话建立` 还是故障那次的，引擎实例根本没换。要验证「表回来了」
    必须让会话真销毁（本机用 force-stop + 第 21 条的 `ime set` 复位）。
    同族事实：`assemble.rs` 的 `译词表载入 {} ms / {} B` 在表缺失时报 `0 ms / 0 B`，
    这个 0 就是「没读到文件」的指纹，比看 warn 有无更定量。
23. **注入式点击的「0 结果」不能当结论**：2026-10-13 一轮把「`adb input tap` 点候选条 ⇒
    `selectRaw` 探针 0 条 PROBE」当成了缺陷证据，隔天同一台设备同一动作重做，直接上屏
    （`hit=true cur_g=9 表长=44`，输入框 `你好`）。三个假阴性来源：
    ① `input motionevent DOWN` 与 `UP` 分两条命令发，不构成一次连贯手势；
    ② 坐标跨布局漂移——候选条加上英文释义第二行后，第 1 格中心 x 从 63 变 104。
    第 16 条讲的是按钮 bounds 会漂，这里漂的是**同一按钮内部文字块变宽**，重新 `dump` 也救不了，
    必须重新量像素；
    ③ 被测态不对——那几次点击时键盘下方是剪贴板覆盖页，不是键区。
    配套纪律：**探针日志按 pid 过滤再计数**。`logcat -c` 之前起的旧进程，它的探针行会留在
    缓冲区里，`grep -c PROBE` 的总数会把上一版镜像算进来（本轮实测：清缓冲后 0 条，
    未清时旧 pid 的 2 条还在）。
    ④ **长按注入有阈值**：`input swipe x y x y 900` 在候选条上不触发长按（据此曾误判
    「菜单不可达」），`1500` 才触发。长按类断言至少试两个时长，且用**截图**而不是日志判定
    （弹菜单这条路不经过 native，日志本来就该是空的）。
    判「会话死没死」的定量办法：比较 `ps -A -o PID,NAME | grep qingshan.ime:ime` 的 pid——
    宿主 `fail()` 或崩溃都会让它换号（本轮实测：B1 的「固定到首位」后 pid 不变、FATAL 0；
    「删除词条…」后 pid 8187 → 17874）。
    ⑤ **宿主弹出来的菜单可以直接 `uiautomator dump` 拿文字和边界，不必猜像素**：
    `PopupWindow` 里五行 `TextView` 全在 dump 里（第一行是候选的英文释义标题，不是动作项）。
    本轮先按像素量了一轮，噪声给出 `[1834, 1985, 2129, 2148, 2272, 2288]` 六个假行心，
    换成 dump 一次就拿到精确边界。两个 Git Bash 坑：
    `adb shell uiautomator dump /sdcard/x.xml` 的 `/sdcard/…` 会被 MSYS 改写成
    `C:/Program Files/Git/sdcard/…`（设备端输出行里就能看到），要加 `MSYS_NO_PATHCONV=1`；
    读回时路径要整体进引号（`exec-out "cat /sdcard/x.xml"`），裸写实测只拿到 71 B。
    dump 文件是 UTF-8，Windows 控制台按 GBK 打印成乱码，判读设 `PYTHONIOENCODING=utf-8`。
24. **真机 `logcat` main 环只有 256 KiB，本机约 2 分钟就滚完 ⇒ 事后取证基本拿不到**：
    2026-10-13 13:02:32 读 `logcat -d -s qingshan` 还有 3 条 `会话建立`（13:00:05/06/15），
    到 13:05:10 同一命令返回 **0 行**；`logcat -d -b main` 现存最早一行是 13:01:20，
    `logcat -g` 显示 `main: ring buffer is 256 KiB (249 KiB consumed)`。
    所以「用户刚才那一下发生了什么」这类问题**只能当场抓，不能回头查**。
    两条配套办法：① 起流落盘 `adb logcat -v time -s qingshan … > file &`（注意用
    `run_in_background` 直接跑 logcat，别在命令里再套 `&`——套了会被判为任务已完成、
    子进程随之退出，实测拿到 0 B 空文件）；② 判「是不是真手指」不必靠日志：
    `getevent` 在这台机器上**可读**（shell 在 `input` 组里，节点 `crw-rw---- root input`），
    触摸屏是 `/dev/input/event1`（name `touchpanel`，有 `ABS_MT_POSITION_X` max 4319
    与 `BTN_TOUCH`；4319/1080 ≈ 4 ⇒ 原始坐标除以 4 就是屏幕像素，能反推点了哪一格）。
    `adb input tap` 是在 InputManager 层注入，**不产生 `/dev/input` 事件**，
    所以 event1 上有没有行 = 真手指与否的硬判据。
    但这条判据有三个坑，本轮踩到两个：
    ① **`adb shell "cmd"` 在非交互下不分配 PTY**（`adb shell tty` → `not a tty`），
    于是 `getevent` 的 stdout 是管道 ⇒ **stdio 全缓冲**，触摸不密集时文件可以长时间 0 字节。
    本轮差点据此把「用户没操作」当成结论——**0 字节 ≠ 没触摸**。修法：
    **`adb shell -t -t "timeout 1800 getevent -lt /dev/input/event1" > 本地文件`**
    （实测 `tty` → `/dev/pts/0`，且事件在进程存活期间逐行到盘）。
    ② **别用 `while :; do getevent -c 6 …; done` 当修法**——本轮试过，`-c N` 每收满
    N 个事件就退出重启，而**一次触摸就有 6~10 个事件**（TRACKING_ID、BTN_TOUCH、X、Y、SYN、
    若干 move、BTN_TOUCH=0、SYN），重启间隙还**不会补投**旧事件，结果把整条手势截断：
    实测只抓到 `BTN_TOUCH=1` + 一个 move，丢掉抬起，也就分不清「按的是候选条格还是确认键」。
    长跑单进程 + `-t -t` 才是对的；`-t`（时间戳）也要带上，否则没法和 logcat 对齐。
    ③ **`sendevent` 在这台机器上不能用**：shell 虽然在 `input` 组、节点是
    `crw-rw---- root input`，但 `sendevent /dev/input/event1 0 0 0` 仍
    `Permission denied`（SELinux `u:r:shell:s0` 拒写）。这条**反而是好事**：
    adb 侧伪造不出 `/dev/input` 事件，所以 event1 有行只能是真手指。
25. **估 APK 体积只能同口径相减**：2026-10-13 我把 **APK 里实测的 stored 大小**
    （`glossary-en.tsv` = 812,946 B）去减**我自己用 `zlib.compress(level=9)` 估的**
    新表大小（821,473 B），得出「+8,527 B」并拿这个数去请示批准。实际 A1 落地是
    **+130,640 B，超批 15 倍**。同口径（都 zlib9）应是 708,040 → 821,473 = +113,433 B，
    与 AGP deflate 实测的 +130,640 B 同量级——差的那 17 KB 是 zlib9 与 AGP deflate
    压缩率不同（实测 AGP 比率 0.446，zlib9 给 0.388）。
    **规矩**：要报 APK 增量，就用 `zipfile.ZipFile(apk).infolist()` 读**包内 stored**
    前后两次相减；`zlib` 估算只能用于「同一估算方法内部」的比较，不能和包内实测混着减。
    同理，`du -sh` 会四舍五入吞掉百 KB 级增量，体积一律量到文件级字节数。

26. **真机取进程状态：应用包名 ≠ Java 包名，且 `pidof` 只给主进程**：2026-10-13 收尾核验时我
    按类前缀写命令，连着两个**假零**——`adb shell pidof app.msime.android` 返回空，
    `run-as app.msime.android` 报 `run-as: unknown package: app.msime.android`。
    实测 applicationId 是 `app.qingshan.ime`（出处：`settings get secure default_input_method`
    = `app.qingshan.ime/app.msime.android.MSIMEInputService`）；`app.msime.android` 只是
    Java 包名。改成 `run-as app.qingshan.ime` 后立刻拿到
    `/data/data/app.qingshan.ime/files/bootstrap/resources/glossary-en.tsv` = 2,150,622 B。
    **另外 `pidof app.qingshan.ime` 只返回主进程 24758，而引擎在 `:ime` 子进程 28055**
    （`ps -A | grep qingshan` 同刻列出两条），所以「IME 进程 pid」不能拿 `pidof` 的单个值。
    **规矩**：进程存在性一律 `ps -A | grep <关键词>` 按进程名列全；`logcat` 取证必须带 `-d`
    （本轮 `logcat -T 1` 漏了 `-d` 变成实时流，把整条命令挂到超时）。

27. **跑批量测的三个假数据源（2026-10-13 可达率轮实测）**：
    ① **`input keyevent` 会偶发丢事件**——发 5 个 DEL 只落 4 个（清 `nihao` 之后日志尾帧还是
    `edit=n`）。对策：删空缓冲**多发 8 个**，并且解析端**断言尾帧的 `edit=` 等于我敲的串**、
    断言「分段齐 + 行数 == n」，不满足就整条作废重跑，绝不拿半帧算命中。
    ② **`adb exec-out screencap -p /sdcard/x.png > 本地.png` 得到 0 字节**——exec-out 的 stdout
    才是图像，一旦带设备路径参数，它写的是设备上的文件、不吐 stdout。正确写法
    `adb exec-out screencap -p > local.png`（同一条命令改完实测 162,213 B）。
    ③ **脚本末尾一条 `grep -c` 命中 0 会让整条命令 exit 1**——本轮验收脚本全部通过却报
    "Exit code 1"，就是最后那句 `grep -c 'FATAL EXCEPTION'` 返回 0 造成的。判成败要看**每一步输出**，
    不能只看末尾退出码（与第 24 条的 `sendevent` SELinux 拒绝同类：那次也是靠逐条输出才定位）。
    另：`am force-stop` 之后默认输入法会被切走（第 21 条），本轮装机后又踩一次，靠
    `ime set` 复位——**每次 install 之后先复核 `default_input_method` 再动手**。

28. **烘进二进制的内容改了必须重编；批量改文件必须保留行尾**（2026-10-13 两条都是踩出来的）：
    ① 只重打 APK、不重编 `.so` ⇒ 二进制里烘的随包锁还是旧的，升级既不重解包也不报错，
    静默留着旧表。一眼看穿的办法：`grep -a -c <新表字节数> libmsime_android.so`（本轮实测
    旧锁 1 命中、新锁 0 命中，才定位到是我自己的构建顺序错了）。
    ② 用 python 批量改注释时以通用换行模式读、再写回 ⇒ **CRLF 被改成 LF**，整文件变成 diff。
    本仓 `.rs` 行尾本来就是混的（实测 CR 计数：`keys.rs`/`logcat.rs`/`view.rs` 带 CR，
    `assemble.rs`/`session.rs`/`stub.rs`/`lib.rs` 不带），不能假设。要么用 Edit 工具，
    要么读写两端都 `newline=""`；**改注释之前同样要先存快照**，否则连「我只改了两行」都
    证明不了——本轮 `view.rs` 靠快照还原成 CRLF（diff 只剩 2 行注释），`wire.rs` 没快照，
    **原始行尾已无法判定**，只能记在这儿。
    **2026-10-13 用户裁定：`wire.rs` 行尾的原始状态记录在案、不再追**——继续查是沉没成本，
    防线是这条前半段的规矩（Edit 工具或 `newline=""` + 改前先存快照），本轮起照办。

29. **收尾复量时踩到的四个「假数据」**（2026-10-14 flush_learning 收尾轮，每个都差点写成结论）：
    ① **Bash 工具每次调用都是新 shell**（只有工作目录留着）。有一条命令忘了在同一次调用里
    `source` 环境脚本，`cargo fmt -- --check` 根本没跑，rustup 的「没有默认 toolchain」报错被
    重定向进文件，`grep -c "Diff in"` 得 0、退出码 1 —— 看起来像「17 个 hunk 突然清零」。
    真信号是那个文件只有 247 B，读一眼就知道。**判据：负结果之前先看输出文件的字节数。**
    ② `adb shell pm list packages 2>&1 | grep -i qing` 把 adb 自己的报错一起喂给 grep 吃掉了，
    于是显示「设备上没这个包」。两个错叠在一起：包名猜错（装机标识是 `app.qingshan.ime`，
    `app.msime.android` 只是 namespace / Java 包名），以及在设备侧写 `*` 通配再指望它展开。
    正确口径：先用装机包名跑 `pm path` 拿全路径，把全路径原样交给下一条命令。
    ③ **审计脚本必须在 `projects/` 下跑**（跨仓口径，能同时命中 msime-develop 与 qingjian）。
    在 `qingshan/` 里跑，所有青简 Core 的引用会整批掉进 NOT-FOUND：本轮实测同一份文档，
    正确 root = 224 条 / NOT-FOUND 12，错 root = 225 条 / NOT-FOUND 110。
    跟历史数字对比之前，先确认 root 是同一个。
    ④ 把「审计结果」写进文档时，**说明文字里的 `文件:行号` 本身也是一条引用**——写完要复量，
    数字可能对不上。本轮补上说明后重新实测仍是 224 条（那两条引用早已在集合里，脚本按
    「路径+行号」去重），算是收敛；但下一次不保证，别把这条当省事。
    **2026-10-14 又中一次，这次是我自己举例举的**：为了说明「完整路径能锚定」，我在审计行里
    写了一个「从 `ranking/` 那一级起头 + 行号」的短格式当反例 ⇒ 它真的成了一条新引用，
    MISSING-PATH 50→51、总数 241→242，行里写的数字当场变成假的。
    反例要写成不带「文件.扩展名:行号」形状的说法。

30. **位次可以程序化读出来，不必读像素（2026-10-14 学习排序实验发现，值得长期复用）**：
    `uiautomator dump --windows` 会把**候选条每一格**的文字与 bounds 一起吐出来
    （例：`text="所以&#10;conj. so" bounds="[0,1648][131,1723]"`，`&#10;` 后面是释义第二行），
    **按 x 升序排就是名次** ⇒ 逐次位次、点哪一格的中心坐标都能现取，不用截图、不用开探针、不用改代码。
    三条配套注意：① 第 12 条②说的「自绘无节点」只对**键区**成立，候选条是 `android.widget.Button`；
    ② 格子宽度随文字变（`suoyi` 那轮第 2 格中心 244，`shenban` 那轮是 204）⇒ **每轮重新 dump 取中心**，
    与第 16、23 条同源；③ 抓设备侧文件要用 `MSYS_NO_PATHCONV=1 adb … shell cat /sdcard/x.xml`，
    不加就是 0 字节（`/sdcard/…` 被 MSYS 换算成 `C:/Program Files/Git/sdcard/…`），
    本轮第一次 dump 就中了这个假零。
    **同族两条实验纪律**：④ 上一节留下的**未提交组合码会污染下一节**——`input text shenban`
    接在还挂着 `suoyi` 的组合码后面，组合变成 `suoyishenban`，候选自然没有目标词
    （表现为解析出的中心 = −1）。每轮 `input text` 前先点「清空」，别假设输入区是干净的。
    ⑤ **单表归因用 `run-as … mv 同目录改名` 就够了**：`FrequencyLearner::from_path` 对缺失的兄弟表
    容错（`crates/qingjian-core/src/storage/mod.rs:79` 把 `NotFound` 映射成 `Ok(None)`，
    `crates/qingjian-learning/src/frequency_learner/mod.rs:124-153` 逐张 `if let Some(...)` 跳过），
    所以挪走一张表不会连带让构造
    失败退回空学习器；测完 `mv` 回来即可。做之前先把三张表 `cat` 出来留底，防止中途有一次落盘把空表写回去。

31. **真机实验的三条新坑（2026-10-14 错选纠正评估轮，全部现场踩过）**：
    ① **bash 里裸单引号会吞掉后面十几行**：`type_ suo'yi` 的 `'` 开了一个引号，直到下一行才闭合，
    中间所有 `echo`/`cat` 变成参数没执行 ⇒ 日志"少了一段"，看着像脚本漏打印。特殊字符一律走
    `input keyevent`（撇号 = 75；字母 a=29…z=54），或在调用处写成带双引号的参数。
    ② **按 `class=` 取第一个节点会拿错控件**：`class:android.widget.EditText` 给到 y=369 的另一个框，
    试用框实际在 1482 一带 ⇒ 要么按 `text` 精确匹配，要么沿用已验证过的坐标。
    ③ **要落盘就按 HOME，别敲死"收起键盘"的坐标**：那个按钮的位置会随页面布局变（本轮点空 ⇒
    会话没销毁 ⇒ 没落盘 ⇒ 我差点把"提交没记学习"写成结论）。
    `input keyevent 3` → `onFinishInput` → `destroy_session` → 落盘，这条本轮实测稳定
    （`02:54:59 学习数据落盘 6 ms（会话销毁）` + 三张表齐）。
    与第 30 条同族：**凡是"点某个控件"的动作，坐标都从当次 dump 现取；取不到（−1）就停下，别照 tap。**

32. **A3 第 5 批预检轮的四条新坑（2026-10-14，全部现场踩过或现场抓出）**：
    ① **派生拼音的"同段数取词频最大"会给裸单音节多音字选错音**：本轮 816 行里 9 行错
    （`长`→zhang、`调`→diao、`还`→hai、`重`→zhong、`弹`→dan，全是词频大但语境错的那个）。
    ⇒ **凡是拿派生拼音当真机输入，先跑一遍"裸单音节多音段"清单逐字判定**（`poly.py`，
    它先用同一套 DP 重算全表并 `assert mismatch==0`，复现不了就说明脚本和 `derive5.py` 不同、
    结论不可用）。
    ② **`dict.tsv` 自己就教错读音**，不是 DP 的错：`长时间 = zhang shi jian`、`太长 = tai zhang`、
    `很长 = hen zhang`（词频 7,429 / 3,729 / 5,128，本轮 `dictlook.py` 实测）。
    ⇒ "照词典"与"照用户会敲的音"是两种口径，**测出来的可达率不是同一个数**，报告里必须写清用的哪个。
    ③ **代理预检要先断言候选条真的出现，再看位次**：本轮前 4 次预检全无效——屏幕睡着且焦点在
    `NotificationShade`、行检测器选中了 QWERTY 键行（"中文最密的一行"会选中键位行，要按
    **CJK 密度**判）、`input keyevent` 打字母落成品文字而没有组句（改用 `input text`）、
    `am start KeyboardTryoutActivity` 未导出 ⇒ 焦点还在桌面。现在脚本开头固定做
    `input text nihao` + 断言格子 ≥ 3，不过就 `ABORT`。
    ④ **Windows 版 Python 看不见字符串字面量里的 `/d/...`**（MSYS 只转换 argv，不转换代码里的串）
    ⇒ `open(r'/d/toolchains/x')` 报 `FileNotFoundError` 而 `open(sys.argv[1])` 正常；
    **路径一律走 argv 传**，别写进代码。

33. **A3 第 5 批裁定轮的四条新坑（2026-10-14，全部现场踩过）**：
    ① **改前/改后配对不能按行序 `zip`**：批里删过 1 行，`zip` 之后整体错位，
    我一次造出 **242 条"假改写"**（真数 24）。⇒ 比对一律按词头建表再配，配不上的当场 assert 失败。
    ② **含中文的 python 用 heredoc 喂 stdin 会报 `Non-UTF-8 code starting with '\xe9'`**
    （同样的 `-X utf8` 有时够有时不够，取决于内容）⇒ 带中文的脚本一律用 Write 落成 `.py` 再跑。
    ③ **长预检别用前景 `timeout 340` 挡**：我 32 条的预检被截在第 31 条。
    ⇒ 要么后台跑要么放宽；报告按实跑条数写（31），别写"32 条跑完"。
    ④ **带探针的 APK 用 `assembleDebug`（非 clean）打出来是 54,460,291 B，比干净包大 4.6 MB**——
    这不是探针的代价（`.so` 只 +3,600 / +4,480 B），是增量构建的噪声。
    ⇒ **体积对比必须同一套命令**：还原后走 `clean assembleDebug` 才回到 49,846,673 B 对上数。

34. **C1 方案 C 落地轮的四条新坑（2026-10-14，全部现场踩过）**：
    ① **`run-as` 里不能靠通配符删设备文件**：`adb shell "run-as pkg rm -f /data/user/0/pkg/files/…/*.tsv"`
    的 `*` 是由 **adb shell 那侧**（uid shell，stat 不到那个目录）展开的，展不开就把字面量传给 `rm`，
    **静默不删**。上一轮我据此写的"已清空"其实没清（本轮 `ls` 复验还剩 4 张表）。
    ⇒ 逐个写死文件名 `rm -f …/user.tsv`，或 `run-as pkg sh -c "rm -f *.tsv"`；删完必须 `ls` 复验。
    ② **别在冒烟序列里自创"清空"步骤**：我加了 `MOVE_END + 12 次 ⌫`，之后 `input text nihao` + 空格
    只剩字母，看着像方案 C 打坏了空格上屏。**对照实验**（临时断掉 `set_extra_dictionaries`、重编、重装）
    显示挂与不挂**表现相同** ⇒ 是我操作序列的伪象。⇒ 冒烟固定用同一串步骤，与历史轮次可比；
    出现"疑似回归"先做**断线对照**再下结论。
    ③ **注释里的数字必须来自实测输出**：我在测试注释里写了"12 条合计 106,541 / 占 0.039%"，
    实际是 **94,886 / 0.0344%**（设备日志给的真值）——我没把 12 个数加起来就写了。
    ④ **`qingjian_core::Engine` 不是 `Clone`**：要"挂表 vs 不挂表"对比只能各建一台（每台读 2 MB 词库约 150 ms，
    五串对照的那条单测因此跑 5.9 s）。别指望 `.clone()` 省钱。

35. **「Edit 锚点吞掉紧跟其后的标题」我连犯两次，防线改成机器查（2026-10-14）**：
    第 33 条已经记过一次（`### refreshHostRaw：方案① 立项，排下一轮` 被我当锚点替换掉、正文留下空行），
    本轮往同一位置插新章节时**又犯了一次**——同一轮内靠 grep 发现并复原。
    ⇒ 以后凡是"在某标题前插入整节"的编辑：① `old_string` 用该标题**下面**的正文首行，
    ② `new_string` 末尾**把那个标题原样带上**，③ 改完必须跑这一条机器检查（丢失标题数必须为 0）：
    ```
    comm -23 <(grep -E '^#{2,4} ' 改前快照) <(grep -E '^#{2,4} ' 现文件) | wc -l
    ```
    只靠"我记得不会删掉"已经证明没用：两次都是在插入新内容的那一步发生的，不是删内容的时候。
36. **探针把两个字段直接串成一个 token，解析端差点把结论读反（2026-10-14 23:10）**：
    `probe_table` 写的是 `write!(rows, " |{}:{}{} {:?} [{}]", index, text, gloss, kind, syl)`——
    `{}` `{}` 中间没空格，`gloss` 的 `G`/`-` 直接粘在候选文本尾巴上。解析正则用 `(\S+)` 取文本，
    于是每个候选都成了 `司机-`、`你好G`，`exact == target` **全部落空**，第一批报告里
    11 条输入"整表位次 = −1"（包括明明在第 1 格的 `客户反馈已经收到`）。
    发现得早是因为 `子串位次` 那一列还留了活口（`子串=[1]` 与"格子行里明明有它"自相矛盾）。
    **教训**：① 探针里**每个字段之间都要有分隔符**，别靠"文本不会有空格"这种假设；
    ② 解析端与生成端的字段边界要各写一次断言，宁可当场炸；
    ③ 修的是**解析脚本**不是探针——logcat 文件留存了（`/d/toolchains/tmp/probe15/lc-*.txt`），
    重跑解析就够，**没有为此多开一次真机**。
37. **「首屏格子里有没有那个词」判不了"有没有纠错"（2026-10-14 23:30，MIN_LETTERS 实验踩的）**：
    青简对不完整输入本来就按**前缀**与**简拼**查（`lookup.rs:8` 的注释就是这个意思），
    所以 `fem` 的 `分`（简拼 `f`）、`hoi` 的 `后`（`ho` 是 `hou/hong` 的前缀）都是**假阳性**——
    看着像"3 字母也纠出来了"，其实与纠错边无关。要判别只有两条路：
    ① 看候选的 `syllables`（`[fen]` 才是纠错、`[f]` 是简拼），也就是得开探针；
    ② 选那种**简拼根本到不了目标**的敲错位置（把错放在**首键**：`xan`→`三` 缺、`cian`→`先` 缺，
       因为 `三`/`先` 的声母不是 `x`/`c`）。本轮用 ② 得到结论，①没做。
38. **`adb input keyevent 66` 不是"文本空格"，它会去激活有焦点的工具栏视图（2026-10-15 00:38 踩，
    误判了一整轮验收）**：试用框里 `input text nihao` 出候选后发 `keyevent 66`，结果不是上屏，
    而是**把剪贴板面板（本机/云端/刷新）打开**，且面板一直留着遮住键位 ⇒ "空格上屏坏了"看起来千真万确。
    **A/B 证伪**：把改前三份文件（`session.rs`/`lib.rs`/`MSIMEInputService.java` 的开局快照）重编成
    对照包（APK 49,861,845 B）跑同一条序列，行为**逐字相同** ⇒ 与本轮改动无关，是注入口径的问题。
    正确口径：上屏要**点屏幕上的空格键**（本机 1080×2400、键位页可见时约 `(400,2300)`），
    点之前必须 `screencap` 确认键位页真露出来了（面板开着时那个坐标落在面板上）；
    复位 = 先 ⌫ 清掉组句让工具栏行回来 → 点工具栏那个高亮的剪贴板图标（`pick.py 剪贴板` 取到 539,1680）
    → 面板关掉、键位回来。`keyevent 67`（⌫）不受这个坑影响，仍可用作删字。
    同轮另两条：① **`pidof app.qingshan.ime` 返回的是 UI 进程**，`:ime` 是另一个 pid，
    报"IME 进程号"要用 `ps -A -o PID,NAME | grep qingshan`；
    ② **`logcat -c` 只清缓冲区、不清进程状态** ⇒ "这条 create 之前没挂起过"不能靠日志窗口反推；
    本轮靠**补一条 destroy 侧日志**（`会话挂起 键=… 表内 N 份`）才把
    「设置第一次建会话却 `换回=true`」查清（更早那次 settings 会话的建立行落在被清掉的范围里）。
39. **PKCS12 会静默吞掉 `-keypass`，代价要到 `apksigner` 那步才爆（2026-10-15 03:08 探针实测）**：
    `keytool -genkeypair -storetype PKCS12 -storepass A -keypass B` 只出一行
    `警告: PKCS12 密钥库不支持其他存储和密钥口令。正在忽略用户指定的-keypass值。`
    然后 **exit 0、密钥正常生成**（脚本根本看不出异常）。真正失败在后面：
    `apksigner sign … --key-pass pass:B` ⇒ **exit 2** +
    `Failed to load signer "signer #1"` / `java.io.IOException: Failed to obtain key with alias "probe" from probe.p12. Wrong password?`。
    顺带两条同族事实：本机 `D:\toolchains\android-sdk\.android\debug.keystore` 的魔数是 `30 82 0a 36`
    ⇒ **AGP 生成的 debug 库也是 PKCS12 而不是 JKS**（JKS 应是 `fe ed fe ed`），
    所以"库口令 = 密钥口令"这条约束对我们现有的调试包同样成立；
    判类型别用 `keytool -list | grep`（中文 Windows 上它会输出 GBK，grep 直接判成
    `Binary file (standard input) matches`），`od -An -tx1 -N4 <文件>` 更稳。
40. **`zipalign` 只能放在签名之前（2026-10-15 03:09 探针实测）**：对已签名的包再跑一次
    `zipalign -P 16 4` ⇒ `apksigner verify` **exit 1**，原文
    `DOES NOT VERIFY` + `ERROR: JAR signer PROBE.RSA: JAR signature META-INF/PROBE.SF indicates the APK is signed using APK Signature Scheme v2 but no such signature was found. Signature stripped?`。
    坑在"反着来看不出来"：Gradle 产物本来就是对齐的，所以**签名前不 align 也能 verify 通过**，
    只有手搓/重打包时才会撞上。次序抄 `msime-develop/platforms/android/build-apk.sh:180-185`：
    zipalign → apksigner sign → apksigner verify → `zipalign -c`（**只检查，不改文件**，
    在已签名包上实测 exit 0）。另一条残留：`apksigner --out X.apk` 会在旁边写一个 `X.apk.idsig`
    （探针实测 398,691 B），上游靠 `release-android.yml:93` 只 `cp` 正式名规避，我们自己传产物要记得排除。
41. **上游那份 `signing.sh` 直接搬到本机跑不起来，三条都不报"哪条不成立"**：
    ① 无扩展名调用 `"$tools_dir/apksigner"` ⇒ `No such file or directory`、**exit 127**
    （第 11 条讲的是 JAVA_HOME 没 source，那是另一条）；② 钥匙串分支依赖 macOS 的 `security`，
    本机 `command -v security` 为空 ⇒ 那段代码永远静默跳过，口令只能从环境变量给；
    ③ 本机是 Git Bash，`/d/...` 形式的路径参数传给 `.bat`/JVM 不生效，要么 `cd` 进去用相对名，
    要么给 `D:/...` 全形。**Python 同理**：字符串字面量里的 `/d/...` 不会被换算，只有 argv 会——
    本轮又踩一次（heredoc 里写死路径报 `FileNotFoundError`），改成正样例从 argv 传。
42. **`am force-stop app.qingshan.ime` 之后系统会把"当前输入法"绑走，`default_input_method` 看不出来（2026-10-15 03:27 踩，差点误报"干净版候选条点不选"）**：收尾做金路径抽查时，试用框 `input text nihao` 直接落**原样字母**、`strip.py` 格子为空——第一反应是功能坏了。真因是 `dumpsys input_method` 里 `mCurMethodId=com.sohu.inputmethod.sogouoem/.SogouIME`（出厂搜狗），而 `settings get secure default_input_method` 依旧回 `app.qingshan.ime/...`。⇒ **判"现在是谁在服务"只能看 `mCurMethodId`**；恢复用 `ime set app.qingshan.ime/app.msime.android.MSIMEInputService`，之后格子立刻回来（`你好/你好好/你好像/你好久/你好看/你哈`）。
    同轮另一条：空格键坐标**别用记忆值**，本轮从 dump 现取是 `(450,2251)`（`text="空格"` 节点），第 38 条写的"约 (400,2300)"是另一屏状态下的近似值。
43. **`env MSYS_NO_PATHCONV=1 adb shell uiautomator dump --windows /sdcard/x.xml` 会变成静默 no-op（2026-10-15 17:36 踩，差点把"没有候选"当成结论）**：
    同一条命令 03:04 还写得出 42,964 B 的 dump，本轮起 **rc=0、stdout 空、设备上一个字节都不写**——
    而 `/sdcard/` 里留着上一轮同名文件，`cat` 出来就是**旧数据**（本轮 `e.xml` 的 mtime 是 01:19，一眼假）。
    证伪过程：换 `MSYS_NO_PATHCONV=1 adb ...`（变量前置）、换不带 `--windows`、换引号整串，
    最后只有 **`adb -s $S shell "uiautomator dump --windows /sdcard/x.xml"`（引号整串、不加 env 前缀）** 正常，
    且必须**先 `rm -f` 目标文件**再 dump。
    ⇒ 机械防御（第 18 条那一类）：**dump 之后断言本地文件字节数 > 1000，否则记 `DUMP_FAILED` 并停下**，
    绝不让 `cells()` 的空输出参与"有没有候选"的判断。本轮 40 条预检与路径 C 验证都带了这个断言（`DUMP_FAILED 0`）。
44. **`exported=false` 的 Activity 连 debuggable 包也拉不起来**（2026-10-15 17:33 实测）：
    `adb shell am start -n app.qingshan.ime/app.msime.android.TestPasswordActivity` →
    `java.lang.SecurityException: Permission Denial: starting Intent { ... } from null (pid=..., uid=2000) not exported from uid 10691`，
    换 exit 255、屏幕上什么都没有。⇒ 自建测试页的清单条目必须 `exported="true"`；
    它没有 intent-filter，只能被显式组件名拉起，且只存在于 debug 变体（release 清单里 0 次，实测）。
45. **这台 OPPO 系设备 adb 装包要人工点确认**（2026-10-15 18:30 踩）：`adb install` 停在
    `Performing Streamed Install` 不返回，`timeout` 到点 rc=124；屏幕上其实是
    `com.oplus.appdetail/...InstallGuideActivity`（「来自"电脑端未知来源"」「涉及敏感权限」「继续安装 / 取消安装」），
    点完第一轮还会进「正在扫描：未成年人安全风险」再要一次确认。
    ⇒ 装 release 包的正确做法：后台起 `adb install`，另开循环 dump 找 `继续安装` 文本节点并 tap，
    直到 stdout 出现 `Success`。debug 包此前能直装是因为它是同一把调试证书覆盖安装、不走这个引导页。
46. **release 包上 `run-as` 全废，且 debug↔release 来回切换每次都要卸载重装**（2026-10-15 18:25 实测）：
    `run-as app.qingshan.ime ls files` → `run-as: package not debuggable: app.qingshan.ime`；
    `ls /data/data/app.qingshan.ime` → `Permission denied`；`allowBackup="false"`
    （`qingshan/host/AndroidManifest.xml:19`）连 `adb backup` 这条路也堵死。
    跨证书覆盖安装必失败：`INSTALL_FAILED_UPDATE_INCOMPATIBLE: Existing package app.qingshan.ime
    signatures do not match newer version; ignoring!`。
    ⇒ 三条后果写进流程：**读私有文件的判据（解包件字节、`files/qingshan/user*.tsv`、flush 落盘）只在 debug 包上做**；
    **release 侧只用 `logcat -s qingshan` + `uiautomator` + `dumpsys`**（这三样实测在 release 上都活着）；
    装机状态在报告里必须写清楚当前是哪一把证书，否则下一轮拿不到 `run-as` 会误判成"功能坏了"。
47. **预检脚本必须自己锁竖屏：中途转横屏会让候选条整条消失，40 条全记 −1，看着像"这批全沉"（2026-10-15 20:35 踩）**：
    上一轮收尾把 `accelerometer_rotation` 恢复成 1（自动旋转），本轮的分离变量预检没重新锁屏，
    跑到第一条就整段空——`strip.py` 格子为空、`第1格=` 为空，40 条全 `位次=-1`，
    而 `FATAL=0`、开局自检 `nihao` 正常，**看起来完全像"这批一句都不露"**。
    定位：`dumpsys display` 的 `mCurrentOrientation=0` 才对，当时是 `ROTATION_90`（横屏 2400×1080），
    横屏下键位在、会话活着、**候选条就是不出现**（现场手打 `nihao` 复现，锁回竖屏立刻恢复
    `你好/你好好/你好像/…`）。⇒ ① 预检脚本开头必须
    `settings put system accelerometer_rotation 0` + `user_rotation 0`（第 38 条那版脚本本来就有，
    我这次抄漏了）；② 循环里每 5 行记一次 `mCurrentOrientation`，事后能证明这段数据是不是干净的；
    ③ **横屏下候选条不显示**这件事本身没有定位，属新发现，见待办第 21 条。
    **（现值 2026-10-16 00:14：本条第 15 行那句「横屏下键位在、会话活着、候选条就是不出现」和
    ③ 那条"新发现"，本轮**复现不出来**，先降级为"未定位"，别当已证事实引用。**
    横屏 + 键位点击、横屏 + `input text`、竖屏两种探针，四格候选条全部正常（10 / 10 / 6 / 10 条，
    见「横屏候选条调查」一节）。**注意：本轮中途我曾用一支写坏的计数器（`cls=='Button'` 等值比较，
    见第 48 条）"证明"过这是 `input text` 不建组合造成的假象——那个解释是假的，已撤回。**
    剩下两个没排除的差别：当时是真实传感器旋转、且预检脚本按竖屏坐标表点横屏键位（点空就根本不建组合）。
    本条 ① ② 两条防御**照旧要做**，但理由要换一个：不是"横屏候选条会消失"，
    而是"横屏键位坐标对不上竖屏那套表"。**）
48. **自己写的计数器必须先跑正样例，否则它会"完美印证"你的假设**（2026-10-16 00:14 踩，本轮最贵的一条）：
    本轮我为「横屏候选条」写了个候选计数：`[n for n in ns if n['cls']=='Button' and '你' in n['text']]`。
    `uiautomator` 的 `class` 是**全限定名** `android.widget.Button`，等值比较 `'Button'` **恒假**
    ⇒ 这一支对任何 dump 都回 0。我拿它跑了"竖屏 `input text`"与"横屏 `input text`"两格，
    两格都是 0，于是当场宣布证伪了待办第 21 条、并把「`input text` 不建组合」写进了文档。
    实际改成 `n['cls'].endswith('Button')` 重跑，**同一批操作竖屏出 6 条、横屏出 10 条候选**，
    前面那条"决定性对照"整段作废。
    ⇒ 三条硬规矩：① **计数器要先在已知有候选的 dump 上数出非零**（本轮的正样例是 `L4.xml`，
    截图 `L5.png` 肉眼可见 10 条，脚本却数出 0 —— 那一刻就该停）；② **一个"0 结果"若刚好
    印证了待验证的假设，它首先是可疑的**，要用第二种口径交叉（截图 / `grep -c` 原文 / 换属性名）；
    ③ `cls` 一律 `endswith`，别等值比较。第 23 条讲的是"注入式点击的 0 不能当结论"，
    这条讲的是**读侧的 0 同样不能**——两侧都会造出假 0。
49. **这台 OPPO 上 `settings put system user_rotation` 单独写不转屏，要用 `cmd window` 两条**（2026-10-15 23:30–23:40 实测）：
    `settings put system user_rotation 1` 之后 `settings get` 回 1、`dumpsys window` 里
    `mUserRotation=ROTATION_90` 也确实变了，但 `mFullConfiguration` 仍是 `ROTATION_0 … port`，
    屏幕**不转**——因为 `mFixedToUserRotation=false`（ColorOS 默认不让用户旋转覆盖应用请求）。
    真正生效的是这两条：`cmd window user-rotation lock 1` + `cmd window fixed-to-user-rotation -d 0 enabled`，
    之后 `dumpsys display` 的 `mCurrentOrientation=1`、`screencap` 出 2400×1080。
    ⇒ 复位也是两条一起：`fixed-to-user-rotation -d 0 default` + `user-rotation lock 0` +
    把 `accelerometer_rotation` 写回原值。**注意 `cmd window user-rotation free` 会顺手把
    `accelerometer_rotation` 改成 1**（实测：执行后 accel 从 0 变 1），照原值恢复要在它之后做。

## 方案 A 实现与真机验收（2026-10-14 03:06–03:25）

用户拍板：**只做 A（退格即撤销），B/C/D/E 全不做。**

### 一、实现（`qingshan/engine/src/keys.rs` 命令 0）

改前三处引用：`keys.rs:65-69` 是无条件 `session.engine.backspace(); CommandOutcome::Consumed`。
改成：

```rust
0 => {
    if session.engine.backspace() {
        CommandOutcome::Consumed
    } else {
        session.engine.note_backspace();
        CommandOutcome::Passthrough
    }
}
```

**比拍板的"一行"多做了半行，理由是先量到一个缺陷**：装旧版实测（03:10，`deltest.sh`）
——上屏 `你好` 之后连按两下 ⌫，输入框**一直是 `你好`**，一个字都没删。原因就在我们这边：
`apply()` 把响应的 `handled` 原样返回给宿主（`MSIMEInputService.java:2424`
`return strictBoolean(result, "handled")`），而 `:6602` 与 `:4007` 都是
「`command(0)` 返回 false 才让编辑器自己删」。我们以前对空组句的那一下回 `Consumed`
（`handled:true`）⇒ **宿主永远不删字，⌫ 在没组句时是死的**。这属于「击穿判定」里
"声称消费了却没做事"那一类，且是本轮之前就在的缺陷。
补 `Passthrough` 之后与上游 macOS 壳完全同形（`command.rs:11-24` 就是
`if !composing { note_backspace(); return false; }`）。**验收步骤 3 直接量到修复生效**（见下）。

### 二、单测（`keys.rs` 测试模块，+2 条，共 45 passed）

夹具是文件里已有的 `SAMPLE`（你好 9000 / 拟好 3000 / 泥壕 800，同一段 `ni hao`），
新加 `learning_session()`（挂 `FrequencyLearner::default()`，内存版，不写盘）。

- `backspace_inside_a_composition_is_consumed_and_leaves_the_erase_ledger_alone`：
  组句非空 ⇒ `Consumed`、`composition().text()` 从 `nihao` 变 `niha`，且先前学到的
  拟好@0 **不许**被退回。
- `empty_composition_backspace_passes_through_and_retracts_the_mis_pick`：
  选一次 拟好 ⇒ 拟好@0；空组句连两下 ⌫ ⇒ 两次都 `Passthrough`；重打同一段选 你好 ⇒
  首格回到 你好。这条就是方案 A 的端到端锁。
- 写测试时自己错了两处，都被机械检查抓回：`first_cell` 读完不清组句（第二次打接在残串后面，
  组成 9 个字母而不是 5 个）、把 `nihao` 删一个字母的期望值写成 `nihad`（正解 `niha`）。
- `cargo test --release` = **45 passed / 0 failed**；`cargo fmt -- --check` 一度涨到 18 hunk
  （keys.rs 新增 1 条，超 100 列），按 rustfmt 的输出手改后**回到 17 hunk，全在 lib.rs**；
  `lib.rs` 没动 ⇒ B1 那四个引用 `:277`/`:286`/`:293`/`:301` 逐行 `sed -n` 复量仍在原位。

### 三、构建与装机

`cargo ndk -t arm64-v8a -t x86_64 -o jniLibs build --release` ⇒ `.so` 4,351,520 → **4,351,792 B**（+272）、
4,597,720 → **4,598,000 B**（+280）；`./gradlew clean assembleDebug` ⇒ APK 49,846,393 → **49,846,673 B**（+280）。
`verify_symbols.py`：102 对 102、两 ABI 缺失/多余 0；`verify_apk.py`：包内容/符号/清单全 True。
`install -r`（不 `pm clear`）后设备 `base.apk` = **49,846,673 B**，与构建产物同尺寸。

### 四、真机验收（`e1cdb946`，竖屏 1080×2400，观测口 = `uiautomator dump --windows`）

**v1（03:17，`planA.sh`）没撤销**：序列里我在两下 ⌫ 之后点了应用自带的「清空」按钮，
结果三张表里 `蓑衣` 与 `所以` 各留 1 条 ⇒ 撤销没发生。当时没下结论，改成最小序列复跑：

**v2（03:21，`planA2.sh`）通过**，逐步实测：

| 步骤 | 实测 |
|---|---|
| 基线（学习目录空） | `所以@0 蓑衣@1` |
| 点第 2 格 蓑衣 | 输入框 `[蓑衣]` |
| **⌫ 一下 / 两下** | `[蓑]` → `[打字试试]`（= 空框的提示字）⇒ **⌫ 真的删字了，旧版做不到** |
| 重打 `suoyi` | `蓑衣@0 所以@1` ⇒ 误选的加成还在生效 |
| 点 所以 | 输入框 `[所以]` ⇒ `apply_retraction` 该在此刻触发 |
| 按 HOME 落盘 | `03:22:42 … 用户词频已保存 entries=1 / 个人 n-gram transitions=1 / 输入串选择 entries=1 / 学习数据落盘 7 ms（会话销毁）` |
| **三张表** | `user.tsv` 只剩 `所以 1`；`user-choices.tsv` 只剩 `suoyi 所以 1`；`user-ngram.tsv` 只剩 `<s> 所以 2` ⇒ **蓑衣 的词频、选择、n-gram 三处全被退回** |
| 冷启动后 | `所以@0 蓑衣@1` ⇒ 回到静态次序 |
| crash `FATAL` | 0 |

**v3（03:23，`planA3.sh`）两例对照定位 v1 的因**：`clean`（⌫ 后直接重打）与
`retap`（⌫⌫ 后多点一次输入框）**都成功**（三张表都只剩 所以）⇒ 焦点重进不是原因，
**v1 丢账本的是那个「清空」按钮**。⇒ 记一条边界：
**「误选 → ⌫ 删掉 → 重打选对的」这条撤销路径实测有效；但中间若由应用侧改文本（本例是
试用页的「清空」），撤销账本就作废。** 机制没往 Core 里追（要追得读 `onStartInput`/
`restartInput` 那条链），真实用户流不会去点应用的清空按钮，先按边界记录。

### 五、B/C/D/E 的裁定（用户拍板，记在案）

**都不做。** B（把 `CandidateManagementAction.java:8` 的确认位改成 false）会让误点即删且无撤销，
比现状更糟；且按 Core 语义 `Engine::forget` 只能清偏好、不能屏蔽词库词
（`crates/qingjian-core/src/engine/learning/forgotten.rs:3-15`：只有用户自造词才「本地不再出」），
"永久不想看到某词"这个语义 Core 不支持，要做得改 Core，超出本项目范围。C 要新增 Java 文件、
D 连带改五处键盘行为，E 是维持现状——A 落地后现状已经不再是"没有纠错入口"。

### 六、「点非最优整句不产生学习」的处理（用户拍板：只标注，不反馈上游）

Core 行为：`Sentence` 分支不 `record`/`record_choice`（`commit/mod.rs:127-130`），逐词记转移要
`sentence_words = Some(..)`，而 `sentence_words()` 在 `:420` 要求**重算出的最优整句 == 被点的那条**
才给词序列 ⇒ 点非最优整句走 `:243` 的 `chain.reset()`，一笔不记（本轮实测：连点 6 次第 2 格，
三张表一个都没生成）。Core 测试里没有对应断言 ⇒ **是否有意未核实**。
**决定：本项目文档长期标注，不改 Core、不反馈青简上游**（理由：可能是有意设计；反馈不一定被接受，
还会引入外部依赖）。

## Java 侧的 Bootstrap 契约

## 数量断言的纠正记录


**水杉 Android 的 native 方法是 102 个，不是 110 个。**
110 这个数字来自第 1 步调研 agent 的二手报告，被一路抄进设计文档、README 和多轮对话。
本仓实测四个独立来源一致：

| 来源 | 计数 |
|---|---|
| `NativeClient.java` 逐行 `grep -c 'private static native'` | 102 |
| `awk RS=";"` 解析（能正确处理换行续写的声明） | 102 |
| 上游 `platforms/android/native/client_jni.cpp` 里 `Java_app_msime_android_NativeClient_*` 去重 | 102 |
| 上游 `platforms/android/verify-native.sh:49` 逐个点名的方法清单 | 102 |

**四来源交叉验到集合级**：Java 声明 / 上游 C++ 导出 / `msime_android.dll.lib` 导出 /
`platforms/android/verify-native.sh:49` 清单，各 **102** 条；**6 对两两双向差集全部为 0**，
并集 = 交集 = 102，即四个集合逐名相同。`.dll.lib` 那份用 NDK 的 `llvm-nm` 取符号表后比对，
不是只比计数。

推导出的下游数字：**符号总数 102 一个不能少**。真实现最初 8 个（`prepareHostRaw createRaw
destroyRaw focusRaw characterRaw commandRaw selectRaw viewRaw`），2026-10-12 真机第一轮后
加到 **9 个**（多一个 `setEnglishModeRaw`，原因见「启动路径的击穿判定」），阶段二再加 7 个
——B1 四件（`pinCandidateRaw` / `fixCandidatePositionRaw` / `clearCandidatePositionRaw` /
`selectEdgeRaw`）、B2 一件（`removeCandidateRaw`）、A 两件（`allCandidatesRaw` /
`selectAnyCandidateRaw`）⇒ 16 real + 86 stub = 102；2026-10-13 20:44 再加 1 件
（`refreshHostRaw`，见「refreshHostRaw ① 落地」）⇒ **现值 17 real + 85 stub = 102**
（实测：`gen_stub.py` 打印 `REAL=17 / 生成 stub=85 / 合计 102`，`verify_symbols.py` 两 ABI
均「导出 JNI=102 缺失=0 多余=0」）。
`gen_stub.py` 的 `REAL` 集合是唯一出处，改它再重生成 `stub.rs`，两处计数由脚本打印。

**教训：跨仓、跨阶段的二手数量断言，使用前必须在本仓重新实测。** 数量类结论一旦写进
设计文档，后续所有"约 110 个""其余 102 个"都会跟着错，而且错得很自洽，不容易发现。

## JNI 0.22 API 事实

与上面「环境陷阱」不同类：那节是 Windows 层面的坑，这节是 `jni` crate 0.22.4 的硬约束。
每条都核到 crate 源码位置（`$CARGO_HOME/registry/src/rsproxy.cn-*/jni-0.22.4/`）。

1. **`jni::JNIEnv` 在 0.22 只是 `EnvUnowned` 的别名**（`src/lib.rs:383`，结构体定义在
   `src/env.rs:4768`）。完整的 JNI API 挂在 `Env` 上，所以在 native 方法里直接写
   `env.byte_array_from_slice(...)` 会报 `no method found`。正规写法是
   `unowned.with_env(|env| …)` 升级后再调用（crate 文档 `src/lib.rs:140-195` 有完整样例）。
   继续用 `jni::JNIEnv` 这个名字会吃一批 deprecated 警告（3.3 实测 17 条），签名里直接写
   `EnvUnowned<'_>` 即可。
2. **`with_env` 要求 `&mut self`**（`src/env.rs:4793`），所以导出函数的 env 参数必须是
   `mut env: EnvUnowned<'_>`。
3. **`#[unsafe(no_mangle)]`**：edition 2024 下裸 `#[no_mangle]` 直接报错
   「unsafe attribute used without unsafe」（3.3 实测 8 处）。
4. **`macro_rules!` 的 follow-set 不允许 `$t:ty` 后接 `$n:ident`**。生成器要「类型 + 参数名」
   成对时，类型位改用 `$t:tt`。另外展开位（右侧）的 `$( … );*` 会被解析成展开结束并报
   「macro expansion ignores ';'」，分隔符只该出现在匹配位（左侧），右侧写 `$( … )*`。
5. **`Outcome` 从 crate 根拿**：`use jni::Outcome;`。枚举定义在 `src/env.rs:4587`，但
   `env` 模块是私有的，写 `jni::env::Outcome` 会 E0603；`src/lib.rs:346` 的
   `pub use env::*;` 才是可见性来源。
6. **`with_env` 内部自带 `catch_unwind`**（`src/env.rs:4801`），panic 以
   `Outcome::Panic` 交回。所以 PRD 风险表要求的「FFI 边界用 catch_unwind 把 panic 转成
   错误码」由 crate 兜住，每个导出不必再包一层。
   **但不要用 `resolve::<jni::errors::ThrowRuntimeExAndDefault>()` 收场**：那条路会往
   `:ime` 进程抛 Java 异常，而水杉 Java 侧的降级分支吃的是 `{"ok":false,…}` 信封
   （`MSIMEInputService.value():844`），抛异常可能击穿没 catch `LinkageError` 的调用点。
   本项目因此自己 match `Outcome` 三分支，失败时回 null 并 `tracing::error!`（见
   `engine/src/wire.rs`）。

## 平台宿主的 character 路径形态

`apps/linux/server/` 与 `apps/windows/server/` 的 dispatcher 接收的**已是成型的 `c: char`**
（`apps/linux/server/src/dispatch/key/input.rs:180/203/219/262` 四个 `fn …(c: char, …)`），
码点校验不在 Core 侧，也不在这两个 dispatcher 里。linux 的转换点上移到事件层：该 server 的
Cargo 依赖里没有 X11/XKB/Wayland 客户端库（只有 `qingjian-*`、serde、tracing、`libc`），
字符来自 `qingjian-platform`
（`crates/qingjian-platform/src/protocol/key/event.rs:7-16`，`KeyEvent.character: Option<char>`，
其注释写明是 `ToUnicode` 的结果），我没有继续往里追。Windows 侧看得见：
`apps/windows/tsf/src/com/key/layout.rs:18-26` 用 `ToUnicodeEx` 取单个 UTF-16 单元后
`char::from_u32(...)`。**不要**把这两个壳当成「Core 该做码点校验」的先例。

Core 自己唯一的码点门槛在 `crates/qingjian-core/src/shortcut/mod.rs:45-47`：
`char::from_u32(value)?` 之后 `(!c.is_control()).then(...)`。本项目 `char_for` 沿用
`!is_control()`，**不用** `is_ascii_graphic()`——后者全仓 9 处，集中在 raw/直输分支
（`apps/linux/server/src/dispatch/key/input.rs:279`、
`apps/windows/server/src/dispatch/key/input.rs:289`），是「英文直输段」的局部规则，
且只认 ASCII，会把 Java 送来的 >127 合法字符挡回去。

对照上游水杉自己的门槛：`platforms/android/native/client_jni.cpp:533-539` 对 `characterRaw`
判 `ascii < 0 || ascii > 127` 就 `ThrowNew(IllegalArgumentException)`，而 Java 侧唯一的 catch
（`MSIMEInputService.java:3125`）是 `JSONException | LinkageError`，接不住它。本项目裁定放宽
到 `char::from_u32 + !is_control()`，理由与逐条调用点记在 `engine/src/keys.rs` 的 `char_for` 注释里。

## 上游性能与体积事实（引用必须带条件）

出处全部是青简 `docs/notes/performance.md`，第五轮之前的容器化改造记录（`:128-141`）。

### 1. 「启动 50 ms / TSV 930 ms」不能搬到本项目

上游原表：
```
| | 改前 | 改后 |
| 词库加载 | 240 ms | 12 ms |      | 语言模型加载 | 620 ms | 9 ms |
| Engine 就绪 | 930 ms | 50 ms |   | 文件体积 | dict.tsv 26 MB + lm TSV 48 MB | dict.qj 46 MB + lm.qj 46 MB |
```
**改前的 930 ms 是「26 MB 词库 + 48 MB 语言模型」全量 TSV 的总就绪时间**，改后的 50 ms 是同一份全量数据换成 `.qj` 之后的结果。

本项目阶段一的数据是 `dict.tsv` **2,053,612 B（实测）**，且**不带语言模型**、不带释义表。
数据规模与上游那张表差一个多量级，所以 **930 与 50 两个数字都不适用**，任何按它们外推的说法都算未验证。
真实数字等 3.4.4 的 host 端到端测试用 `Instant` 实测后写回这里，届时区分「实测」与「上游记载」。

### 2. `.qj` 不一定比 TSV 小，对词库反而更大

同表 `文件体积` 行：`dict.tsv 26 MB + lm TSV 48 MB` → `dict.qj 46 MB + lm.qj 46 MB`。
上游自己的解释（`performance.md:137-138`）：TSV 里词频是十进制文本，容器里是定长 `u32`，
再加上哈希表约一半是空槽，**容器反而涨体积**；要减体积只能改变长编码，而那会破坏零拷贝。

⇒ 因此 PRD 里「用 `.qj` 压缩容器控制 APK 体积」这条缓解措施**对词库不成立**，
`.qj` 的收益是加载耗时（240→12 ms）而不是体积。阶段一直接随包带 TSV 的决策不受影响。

### 3. TSV 是一等路径，不是调试通道

`performance.md:128`：「TSV 解析路径也改成产出同样的结构，查询代码只有一份」。
`qingjian-dictionary/src/dictionary/mod.rs:199-207` 的 `from_path` 按 `Container::is_qj` 分流，
TSV 分支用 `Text::Owned` / `Table::Owned`（同文件 `:189-195`），**不依赖 mmap**。
⇒ Android 上带 TSV 不需要额外处理，也不存在"生产环境其实只测过 .qj"的风险。

### 4. 学习数据的接法（本项目采用）

`FrequencyLearner::from_path` 收的是**词频文件**路径而非目录
（`qingjian-learning/src/frequency_learner/mod.rs:121`），五张兄弟表由它按同目录自动派生
（同文件 `:128-151`，常量 `:18-33`）。`user.tsv` 这个名字**不是库里的常量**，是各壳自选的落盘文件名
（`apps/linux/server/src/assembly/mod.rs:100-101`）。
本项目定为 `<filesDir>/qingshan/user.tsv`（与 `<filesDir>/bootstrap/state/` 平级的兄弟目录，不嵌在其下），与水杉自己的资源校验目录分开，避免抢路径。

> 这一节说的是**读**的接法，已经接了。**写**没接：`Engine::flush_learning` 从未被调用，
> 学习数据只在内存里 —— 见「阶段二排期」的 P1（学习数据落盘）。
> （现值：写已于 2026-10-14 00:54 接上，见下面「flush_learning 落盘」一节；这段是接之前的记录，保留原文。）

#### 那 `flush_learning` 该接在哪里（评估，未实现 ⇒ 现值：2026-10-14 00:54 已按方案 2 落地，见下一节）

上游三个壳的口径（行号本轮逐个核过）：

- **热路径节流**：`apps/linux/server/src/dispatch/mod.rs:93` 的 `handle()` 每条客户端消息后判一次
  `if self.last_flush.elapsed() >= LEARNING_FLUSH_INTERVAL`，常量在 `:24` = **60 秒**；
  同一判断在主循环 `tick()`（`:105`）里再来一次。Windows 壳同构（`apps/windows/server/src/dispatch/mod.rs:42` 的 60 秒常量、`:204` 的 flush）。
- **会话关闭**：`close_session`（`apps/linux/server/src/dispatch/session/mod.rs:89`）末尾 `:105` flush。
- **`Drop`**：`impl Drop for Router`（`apps/linux/server/src/dispatch/mod.rs:121`）。
- **停用/切输入源**：macOS 在 deactivate 路径 flush（`apps/macos/src/imk/controller/mod.rs:145`），
  善后路径再补一次（`:155`）。

Android 侧的等价物本轮实测清楚了：`NativeClient.destroy` 全树**只有一个调用点**
（`MSIMEInputService.java:1598`），它在 `stop(boolean finish)` 的 `if (session != 0)` 块里
（`:1595-1600`，紧跟着 `session = 0`），而 `stop()` 由 `onFinishInput()`（`:1407`）、
`onDestroy()`（`:1484`）、`fail()`（`:3116`）、`startEngineSession` 失败（`:1734`）等路径到达。
我们的 `destroyRaw` **已经是真实现**（`engine/src/lib.rs:96`，调 `destroy_session`）。
⇒ 挂在 `destroy_session` 里 flush，等于覆盖「离开输入框 / 键盘销毁 / 会话被杀」三个时机；
盖不住的是 `am force-stop` 与低内存回收 —— 那两个没有 Java 回调，正是上游要加 60 秒节流的原因。

三个候选，按代价排：

1. **只在 `destroy_session` 里 flush**：改动最小、零热路径代价；进程被杀仍然丢。
2. **1 + 上屏路径 60 秒节流**（抄 `handle()`）：耐久度接近上游。代价是写盘落在用户能感知的路径上
   （`write_atomic` 是落盘级），**要先在真机实测有没有偶发卡顿**再决定。
3. **1 + 自建定时线程**：最像 macOS 的 `watch`，但要给会话表加跨线程访问，复杂度跳一档，现阶段不建议。

**动手前必须先核的一件事**：`Engine::flush_learning` 一次 flush **五项**
（`learner`/`logger`/`meter`/`vocabulary`/`translator`，`crates/qingjian-core/src/engine/learning/mod.rs:188-193`），
而我们只 `with_learner(...)`。其余四项在没配路径时各自往哪儿写，得先读清楚 ——
否则接一个 flush 会顺手打开四个没计划的落盘面。

### flush_learning 落盘（2026-10-14 00:54，方案 2 已落地）

**动手前那项必核已核**（上一节末尾列的那件事）：`Engine::flush_learning` 一次 flush 五项
（`crates/qingjian-core/src/engine/learning/mod.rs:188-193`），除 learner 外四项
`NoInputLogger` / `NoUsageMeter` / `NoVocabularyTracker` / `NoTranslator` **全部走 trait 的空默认**
（`input_log/logger.rs:9`、`statistics/meter.rs:9`、`vocabulary/tracker.rs:16`、`translator.rs:15`，
且四个 `impl ... for No*` 块里 `grep -c flush` = 0，没有覆写）⇒ 接 flush **不会**打开四个没计划的落盘面。
只有 `MutedLearner::flush()`（`learning/muted.rs:159`）转发到 `assemble.rs:126` 装的
`FrequencyLearner`，落 `filesDir/qingshan/user.tsv` 与它的兄弟表。

**实现**（全在 Rust 侧，`host/java` 零改动）：
- `session.rs` 新增 `LEARNING_FLUSH_INTERVAL = 60 s`（抄上游 `apps/linux/server/src/dispatch/mod.rs:24`）、
  `Session.last_flush` / `Session.dirty`、`note_commit()`、`flush_at_destroy()`。
- `lib.rs::run_transition()`：`state.requery()` **之后**才 `if committed { state.note_commit() }`
  ——先把用户要的候选算出来，再碰磁盘。
- `destroy_session()` 改成「先从表里取出、**在锁外** flush、再返回」。为什么销毁路径不看我们的
  `dirty`：`Engine::forget`（删词）会让 learner 变脏但不会走 `note_commit`；learner 自己有
  `if self.dirty` 门（`learner_impl.rs:209`），没变化就不写，所以无代价。

**真机验证（`e1cdb946`，全程不 `pm clear`）**：

| 步骤 | 观察 |
|:---|:---|
| 三次上屏（输入框 `你好你好你好`） | `files/qingshan/` **仍空** ⇒ 节流生效 |
| 回桌面 → `onFinishInput` → destroy | 出现 `user.tsv` 50 B（`你好\t3`）、`user-choices.tsv` 71 B（`nihao\t你好\t3`）、`user-ngram.tsv` 133 B（`<s>\t你好\t2`、`你好\t你好\t4`）；日志 `用户词频已保存` + `学习数据落盘 3 ms（会话销毁…）` |
| `am force-stop` 冷启动 → 再上屏一次 → 销毁 | `你好` 3 → **4** ⇒ 读回 + 累加都通 |
| 上屏后**立刻** force-stop（不走 destroy、不到 60 s） | `你好` 停在 4 没涨 ⇒ **这 ≤ 60 秒的数据丢了**，方案 2 的固有边界 |
| 自然触发节流 | `00:51:41 学习数据落盘 0 ms（上屏满 60 秒…）` |
| 写盘耗时 | 实测 **0 / 3 / 7 / 9 ms**，crash `FATAL` 0 ⇒ 无可感知卡顿 |

**A/B：学习有没有改排序？本轮没测出来。** 把 `files/qingshan` 挪开（`mv` 到 `-off`，
跑完再换回来，`user.tsv` 内容已核对还原）做对照，同一个输入 `nihaonihao`：
两张截图 **sha256 相同**、`ImageChops.difference` 包围盒 **None** ⇒ 排序没变。
原因是量级：个人计数 你好 4 / 你好好 3 / 你好像 2，基础词频 你好 71,960 / 你好好 4,974 /
你好像 4,541 —— 个人次数压不过基础词频。**结论要分清**：数据链路三段（写盘 / 读回 / 累加）已证，
「学习让排序变好」这句**没有证**，要证得先读 Core 的权重公式或造基础词频接近的最小对照。

**本轮两个自己造的测量故障（都不是实现问题，但都烧了轮次）**：
① 第一次跑批三次「上屏」进的是字母 `nihao` 而不是汉字，我差点据此判实现无效——
真因是**设备被转成横屏**（截图 2400×1080），候选格坐标全打偏。
`settings put system user_rotation 0` + 关自动旋转后立刻正常。**跑真机交互前先 `screencap` 看尺寸**，
这一条比任何断言都便宜。收尾时已把 `accelerometer_rotation` 恢复成 1。
② 第一版把销毁路径的耗时日志定成 `debug!`，而日志桥按 level 挡 `debug!`（`logcat.rs:44`）
⇒ 设备上完全看不见销毁有没有落盘，我因此无法区分「没写」与「写了但没日志」。
改成「本会话确实攒过上屏才打 `info!`」后才拿到上面那张表。**给新代码加观测点时，
先确认那个 level 能穿过我们自己的过滤器。**

**顺带**：`lib.rs` 因为落盘钩子又下移 6 行，B1 那四个引用按同法复量成
`:286`/`:293`/`:301`/`:277`（README 已同步，并写了「动 `lib.rs` 必复量」的提醒）。

## 「学习改变排序」决定性实验（2026-10-14 01:32–01:56，只读 + 真机，代码净改动 0）

上一轮留的话是「要证『学习让排序变好』，得先读 Core 的权重公式或造基础词频接近的最小对照」。
本轮两件都做了，结论是**学习有效**，而且比预想的灵敏得多。

### 一、Core 的词级排序公式（读码，每条引用本轮 `sed -n` 逐行回读过）

下面路径以青简仓根为基准（`projects/qingjian/`）。排序键是一个九元组，**元组顺序即规则**
（`crates/qingjian-core/src/ranking/scored.rs:37-49` 的 `SortKey`，规则文档在
`crates/qingjian-core/src/ranking/mod.rs:3-15`）：

1. `Reverse(exact)` 打全的在前 → 2. `Reverse(coverage)` 覆盖字母多在前 →
3. `abbreviated` 简拼少在前 → 4. `Reverse(full_last)` 末音节完整在前 →
5. **`Reverse(choice)` 同一输入串下用户选过的次数** →
6. **`Reverse(score × 1000)`** → 7. `altered` 原样命中在前 → 8. 字数少在前 → 9. 文本（稳定 tie-break）

第 6 级 `score = log_prob + weight_bonus(weight) − penalty`
（`crates/qingjian-core/src/ranking/mod.rs:64`）：

- `weight_bonus(n) = WEIGHT_BONUS × ln(1 + min(n, WEIGHT_CAP))`，`WEIGHT_BONUS = 0.5`、
  `WEIGHT_CAP = 20`（`crates/qingjian-core/src/ranking/mod.rs:27`、`:30`、`:36`）
  ⇒ **n=1 给 0.347，封顶 1.536**（源码注释写明这是故意的：让上下文仍能压过它）。
- `log_prob = transition_log_prob(model, personal, context, word, fallback)`
  = `model.log_prob(prev, word).unwrap_or(fallback)` 再与个人 n-gram 插值
  （`crates/qingjian-core/src/sentence/mod.rs:86-95`）。
- **我们没接语言模型**：`Engine::new` 缺省装的是
  `Box::new(NoLanguageModel)`（`crates/qingjian-core/src/engine/mod.rs:410`，它的 `log_prob`
  恒 `None`，`crates/qingjian-core/src/sentence/language_model.rs:13-15`）⇒ 走兜底
  `fallback_log_prob = ln(f + 1) − ln(total_frequency) + FALLBACK_PENALTY`，`FALLBACK_PENALTY = −4.0`
  （`crates/qingjian-core/src/sentence/mod.rs:76-84`）。
  **基础词频就是从这一项进来的**，两个候选之差 = `ln(f₁+1) − ln(f₂+1)`，与 `total` 无关。
- 个人 n-gram 插值（`crates/qingjian-core/src/sentence/user_ngram.rs:225-262`）：
  `μ = min(c(v)/(c(v) + K), 封顶)`，`K = CONFIDENCE_K = 8.0`、封顶 `MAX_CONFIDENCE = 0.5`、
  `λ = USER_LAMBDA = 0.8`、三元绝对折扣 `TRIGRAM_DISCOUNT = 0.75`
  （四个常数在 `crates/qingjian-core/src/sentence/mod.rs:44`、`:48`、`:51`、`:55`）；
  `P = (1−μ)·P_base + μ·P_personal`，`P_personal = λ·c(v,w)/c(v) + (1−λ)·c(w)/N`；
  **`c(v)` 这个上文从没见过就直接返回 `base`**
  （`crates/qingjian-core/src/sentence/user_ngram.rs:233-238`）；
  上限：转移条目数超 `MAX_USER_TRANSITIONS = 200_000` 时**所有计数减半**
  （`crates/qingjian-core/src/sentence/mod.rs:58`、
  `crates/qingjian-core/src/sentence/user_ngram.rs:70-72`）。
- 一次「点候选上屏」同时动三处，**按什么计数**要看清：
  `user.tsv` = 词 → 次数（喂给 `weight`，按**词**、与拼音串无关）；
  `user-choices.tsv` = 输入串 → 词 → 次数（喂给第 5 级 `choice`，键是**敲的那串字母**，
  `choice_key` 只滤掉 `'`，`crates/qingjian-core/src/engine/mod.rs:555-560`）；
  `user-ngram.tsv` = 前词 → 后词 → 次数（句首 `<s>`），**用户自己点选的记双份**
  （`crates/qingjian-core/src/sentence/user_ngram.rs:38` 的 `record_times`；实测 12 次上屏 ⇒ `<s> 蓑衣 24`）。
- 词库静态词频本身**只用于预选和兜底**，不直接进第 6 级（`crates/qingjian-core/src/ranking/mod.rs:15`）。

### 二、由公式直接算出的预期

`total_frequency` 本轮实测（`awk` 求和 `dict.tsv` 第三列）= **275,814,283**。

- **B 组 `suoyi`**：所以 155,225 / 蓑衣 48 ⇒ Δ = ln(155226/49) = **8.061** ≫ 加分封顶 1.536
  ⇒ 只靠 `user.tsv` 的词频加分**永远翻不动**（能翻的最大词频比 = e^1.536 = **4.65**）。
  但第 5 级 `choice` 排在第 6 级**之前** ⇒ **1 次上屏就该翻**；个人 n-gram 那边 1 次上屏记 2 份，
  μ = 2/(2+8) = 0.2、`P_personal` ≈ 1 ⇒ 第 6 级也满足。**两条路各自都够。**
- **A 组 `shenban`**：申办 592 / 身板 592 ⇒ Δ = 0 ⇒ 任何一次上屏（0.347）都翻。
- 预期翻转次数：**两组都是 1**。

### 三、实测（真机 `e1cdb946`，竖屏 1080×2400，装机版 49,846,393 B 本轮没动过）

**观测口换了，而且比截图强**：`uiautomator dump --windows` 能把候选条每一格的文字和 bounds
全吐出来（`所以&#10;conj. so` 的 bounds 是 `[0,1648][131,1723]`），**按 x 排序就是位次**。
⇒ 不改代码、不开探针、不读像素就能拿到逐次位次。键位行仍是自绘无节点（第 12 条②只说对了键区）。

| 步骤 | 实测结果 |
|---|---|
| 基线（0 次上屏） | `所以/蓑衣/索引/缩影/缩印/所以我` ⇒ 蓑衣 @1 |
| **第 1 次上屏蓑衣后** | `蓑衣/所以/...` ⇒ **蓑衣 @0，翻转点 = 1** |
| 第 2–12 次 | 一直 @0；三张表：`suoyi 蓑衣 12`、`user.tsv 蓑衣 12`、`<s> 蓑衣 24` |
| `am force-stop` + `ime set` 冷启动 | 仍 `蓑衣 @0` ⇒ 数据从磁盘读回 |
| 只挪开 `user-choices.tsv` | 仍 `蓑衣 @0` ⇒ 个人 n-gram 单独就够 |
| 只挪开 `user-ngram.tsv` | 仍 `蓑衣 @0` ⇒ 第 5 级 `choice` 键单独就够 |
| **两张一起挪开（负对照）** | **回到 `所以 @0`** ⇒ 翻转确由学习数据造成；此时只剩 `user.tsv 蓑衣 12`，加分 1.279 < 需要的 8.061，**与公式一致** |
| 装回两张 | 又翻回 `蓑衣 @0` |
| A 组基线 | `申办/身板/身板儿/身边/申报/深表` ⇒ 身板 @1 |
| A 组 1 次有效上屏后 | `身板/申办/...` ⇒ @0；冷启动后仍 @0（`shenban 身板 2`） |
| crash `FATAL` | 0（三次检查均为 0） |

**结论：学习有效，1 次上屏就翻，词频差 3,168 倍也翻。**
上一轮的零结果不是「学习无效」，是**测错了地方**：`nihaonihao` 是四音节串，第 1–4 级结构键
（exact / coverage / abbreviated / full_last）先把整句候选压在词级候选之前，个人计数所在的
第 5/6 级**根本没轮到比**。⇒ 规矩：**在给「学习无效」下结论之前，先确认被测那一对候选
在第 1–4 级是平的**（同串、同音节数、同字数、都打全）。

### 四、本轮自己造的测量事故一条

A 组第 1 轮 `LO_CENTER=-1`（低频词不在候选条上）：上一节做完负对照后，输入法里还留着**没提交的
`suoyi` 组合码**，我又直接 `input text shenban` ⇒ 组合变成 `suoyishenban`，候选自然是整句。
那一次「上屏」根本没发生（`user-choices.tsv` 里 `shenban` 只有 2 条 = 第 2、3 轮）。
所以 A 组的翻转点要写成「**1 次有效上屏**」而不是「第 1 轮」。
⇒ 每轮 `input text` 之前必须先点 `清空`，别假设组合码是空的。

### 五、还没证的

- 只测了「词级、第 1–4 级并列」这一类。**整句路径**（`convert_paths`）与**换一种写法**
  （如带 `'` 的 `suo'yi`）会不会同样翻，本轮没测。
- μ 封顶 0.5 与 `MAX_USER_TRANSITIONS` 减半只在读码层面确认，没灌到 200,000 条实测。
- 上面 μ / P 的算式是**按公式手算的量级**，设备侧只直接观测了位次与三张表的计数。
- **顺带发现（本轮没查）**：候选条上方工具栏一直显示
  「设置读取或应用失败，保留当前设置」，两次截图都在。与本实验无关，未定性。

### 六、实验后的设备复位（02:09 实测）

实验在设备上留下的是**学习数据**（`suoyi → 蓑衣 12`、`shenban → 身板 2`），留着会静默污染以后
任何看位次的实验（`suoyi` 的蓑衣会一直钉在第 1 格）。做法：三张表先 `cat` 出来归档
（`/d/toolchains/tmp/qingshan-snap/learnrank/final-*.tsv`），再 `run-as … rm -f` 掉三张表，
冷启动后重打同一个拼音 ⇒ **`所以/蓑衣/...` 与 `申办/身板/...` 都退回实验前的静态次序**
（与本轮基线逐格一致）。这既是复位，也是「翻转来自学习数据」的第三次反证。
收尾状态：`default_input_method` 仍是我们的、crash `FATAL` 0、`accelerometer_rotation` 恢复 1。

**这一段又踩了两次，都记下来**：
① 第一版复位脚本把 `cat > 快照目录/文件` 写在 `mkdir -p` 之前 ⇒ 重定向失败、**归档没写成而设备已删**
（报错确实打了，但在前 10 行、被我自己的 `tail -25` 截掉，第一眼没看见）。本机侥幸：01:56 那一轮
已经把三张表 `cat` 到 `/d/toolchains/tmp/final-*.tsv`，字节还在。
⇒ **往目录里重定向之前先 `mkdir -p`；删之前先 `ls` 归档目标，确认字节真在里面。**
② 上一节收尾时把 `accelerometer_rotation` 恢复成 1，之后又碰了一次设备 ⇒ 设备侧躺着，**立刻变回横屏**，
那一轮 `CELLS=0`、`mInputShown=false`，`input text` 把 `suoyi` 当英文直接落进输入框。
`user_rotation=0` 在自动旋转开着时**不管用**。⇒ **要么把剩下的设备步骤做完再恢复自动旋转，
要么恢复之后就别再碰设备**；每次重新碰设备都按第 30 条先 `screencap` 核尺寸。

## 「错选纠正入口」评估（2026-10-14 02:25–03:05，只读 + 真机，代码净改动 0）

动机：上一轮证明 1 次上屏就能把词频 48 的词顶到第 1 格 ⇒ 误选代价变大，而唯一的纠错入口
（长按「删除词条…」）在 ColorOS 13 上必崩（「已知失效项」第 7 条）。

### 一、崩溃链与「能不能不改 Java」的结论

链路（本轮逐行读过，行号现量）：

1. 候选格长按：`ImeCandidates.java:164-170` `setOnLongClickListener` → `showCandidateMenu`。
2. 菜单项来自硬编码枚举：`ImeCandidates.java:128-132` 把 `CandidateManagementAction.values()`
   全加进 `PopupMenu`；点击回调在 `:141-152`：**`action.confirmationRequired()` 为真就走
   `s.confirmCandidateRemoval(id, text)`（`:147-149`），否则直接 `s.editCandidate(id, action)`（`:151`）**。
3. `confirmationRequired` 只有 REMOVE 是 `true`：`candidate/CandidateManagementAction.java:8`
   `REMOVE("删除词条…", "已删除词条", true)`。
4. 崩点：`core/MSIMEInputService.java:6049-6059` —— `new AlertDialog.Builder(this) … .show()`，
   `this` 是 `InputMethodService` 的 context，token 不可用 ⇒ `BadTokenException`。
   **这是整个 host/java 里唯一一处 `AlertDialog.Builder`**（`grep -rn "new AlertDialog.Builder(" → 1 处）。
5. 确认后才会走到 native：正按钮 lambda → `editCandidate(id, REMOVE)` → `NativeClient.java:518`
   → `removeCandidateRaw` → 我们的 `qingshan/engine/src/lib.rs:340` → `remove_candidate()`（`:323-337`）
   → `Engine::forget`。**所以 `removeCandidateRaw` 在 AlertDialog 之后，不在之前。**

**可绕点判定：要「拦截这一项的行为」= 不可绕，必须动 Java。** 依据：
- 菜单项的构造与分派全是硬编码，宿主没有留 listener / 策略接口 / 可覆盖的工厂；
- 唯一由我们决定的开关是 `candidateManagementEnabled()`（`MSIMEInputService.java:5979-5987`），
  它读的是 **我们 payload 里的 `local_mode` 与 `scheme`**（`view` 就是 `NativeClient.create`/按键结果
  的 JSON，见 `:1698`、`:2352`）——但它是**整块菜单的总开关**，`local_mode != "none"` 还会连带改掉
  滑行输入、字母行、九宫格面板、面板页等五处行为（`ImeGlideTyping.java:94`、`ImeLetterRows.java:210`/`:494`、
  `ImeNineKeyPanel.java:139`、`ImePanels.java:47`）。用它去藏一项 = 把整个键盘换一种模式，不可用。

### 二、本轮真正的发现：纠错入口不必经过这个菜单

`Engine::note_backspace()`（`crates/qingjian-core/src/engine/composing.rs:135-151`）是**壳契约**：
「不在组句时按了退格，删的是应用里刚上屏的文字」。它喂给 `apply_retraction()`
（`crates/qingjian-core/src/engine/commit/mod.rs:328-377`，在每次上屏开头被调 `:161`）：
**最近被删干净的那次上屏与这次是同一段拼音（`last.rs:57-59` 的 `same_input`，含前缀），
而这次选了别的词 ⇒ 把上次记的学习整笔退回**（`unrecord` + `unrecord_choice` + `unrecord_transition`
+ `unrecord_typo`，`commit/mod.rs:360-376`）。判据是 `is_erased()`：`erased == chars`（`last.rs:52-54`），
所以要用户把那个词整个删掉。

- **上游 macOS 壳就是这么接的**：`apps/macos/src/imk/controller/command.rs:10-19`
  —— 不在组句且 `deleteBackward:` 就 `engine.note_backspace()`。
- **我们从来没调过**：`grep -rn note_backspace engine/src/` → **0 命中**（本轮实测）。
- **而 ⌫ 一定会送到 native**：`policy/HardwareKeyPolicy.java:12` 把 `KEYCODE_DEL` 映射成命令 0，
  `MSIMEInputService.java:4006-4007` 无条件 `command(0)`；屏幕上的删除键走
  `MSIMEInputService.java:6602`（`!command(0)` 才自己删），另五处键位布局同构
  （`ImeLayoutRows.java:388`、`:469`、`:565`、`:833`、`:902`、`ImeNineKeyPanel.java:294`）。
- 我们接命令 0 的地方是 `qingshan/engine/src/keys.rs:65-67`：`session.engine.backspace()`，
  **返回值被丢掉**。而 `Engine::backspace()`（`composing.rs:195-206`）在组句为空时返回 `false`。

⇒ **候选方案 A 的实现量是 Rust 侧一行**：`if !engine.backspace() { engine.note_backspace(); }`。
Java 零改动、无新 UI、不改用户习惯（用户本来就会「删掉重选」）。
**效果本轮没测**（不改代码），下一轮实现后用位次观测口验：`suoyi` 选 蓑衣 → 删掉 蓑衣 两个字 →
重打 `suoyi` → 选 所以 → 看 蓑衣 是否退回 @1、`user-choices.tsv` 的 `suoyi 蓑衣` 是否消失。

### 三、候选方案清单（不选，等拍板）

| 方案 | 做法 | Java 零改动？ | 实现范围 | 与 PRD 承诺 | 上游同步代价 | 真机可验证性 |
|---|---|---|---|---|---|---|
| **A 退格即撤销** | `keys.rs` 里 `backspace()` 返回 false 时补调 `note_backspace()` | **是** | Rust 1 行 + 单测 | 不破例 | 无（不碰 Java，也不碰 Core） | 可，用本轮位次口 |
| B 去掉确认框 | `CandidateManagementAction.java:8` 末尾那个 `true` 改 `false` | 否（改 1 行 Java） | Java 1 字符 | **破例** | 每次同步上游都要重打这一行；该枚举是上游高频改动面（四个动作都在里面） | 可；但误点即删、无撤销 |
| C 自己的纠错 UI | 新增 View/窗口 + 自己的确认框，走 `removeCandidateRaw` | 否（要新增 Java 文件才能建窗口） | 新 Java 类 + 接线 | 破例（新增也是改 host/java 树） | 最大：新文件与上游目录混在一起，且要自己管 window token（正是崩点的同一类问题） | 可，但工作量最高 |
| D 只隐藏坏入口 | `candidateManagementEnabled()` 走 `local_mode` | 名义上是 | Rust 改 payload | 不破例 | 无 | **不可取**：连带改掉五处键盘行为（见上） |
| E 不做 | 记为已知缺陷 | 是 | 0 | 不破例 | 0 | —— |

**A 与 B/C 不是互斥的**：A 解决「我刚点错了」这个高频场景（也是本轮动机），B/C 解决
「这个词我永远不想再看到」。按 Core 语义（下一节）**这两件事本来就不是一个操作**。

### 四、产品语义：Core 能做什么、做不到什么

- `Engine::forget`（`crates/qingjian-core/src/engine/learning/mod.rs:152-172`）分候选种类：
  只有 `Chinese | Code | Cloud` 会走 `learner.forget(text)`；`English` 走 `forget_english`；
  **`Sentence | Shortcut | Custom | Emoji | Generated` 直接返回 `Forgotten::default()`**（`:168-172`）。
- `FrequencyLearner::forget`（`crates/qingjian-learning/src/frequency_learner/learner_impl.rs:168-198`）
  清四样：用户词表（`words`）、`user.tsv` 词频（`counts`）、**所有输入串下**的 `choices`、
  个人 n-gram 里与该词有关的转移（`ngram.forget_word`）。
  ⇒ **语义 = 彻底清掉"我学到的"，不是降权**；词库里的词照旧会出（`dict.tsv` 不是用户数据）。
  `Forgotten.user_word` 注释写明「它是用户词…本地不再出」（`learning/forgotten.rs:3-15`）——
  即**只有用户自造词才会"永远不再出现"**，普通同音词只是掉回静态次序。
- 我们的 B2 已按这个语义实现：`lib.rs:330-336` —— `is_nothing()` 为真时回 `handled:false`，
  宿主弹「当前候选不支持此操作」（`MSIMEInputService.java:6042`），不假装删掉了。
- **整句误选学不到东西**（本轮实测 + 读码定位）：
  ① `Sentence` 分支不 `record`、不 `record_choice`（`commit/mod.rs:127-130`）；
  ② 逐词记转移要 `sentence_words = Some(..)`（`:217-241`），而 `sentence_words()` 在 `:420`
  要求**重算出来的最优整句文本 == 被点的那条**才给词序列；点的是非最优整句 ⇒ `None` ⇒
  走 `:243` 的 `chain.reset()`，**一笔都不记**。
  实测：`nihaonihao` 连点 6 次第 2 格 你好你要，三张学习表**一个都没生成**，只有
  `user-typos.tsv`（敲错表）被写；次序 6 次完全不变。
  **这是 Core 的行为，不是我们接错了**；是否有意，`grep sentence_words` 在 Core 测试里没有对应断言 ⇒ **未核实**。
- 因此「误选纠正」的产品语义建议：
  - **误选词** → 首选 A（删掉重选即自动撤销），「删除词条」只作为"清掉我对这个词的偏好"的第二入口；
  - **误选整句** → `forget` 帮不上（返回 default），A 也帮不上（没记就不必退）⇒ 无需入口，但要知道
    「反复选同一个非最优整句不会学起来」这个既有限制；
  - **想永久屏蔽某个词** → Core 没有这个能力（只有用户词才"不再出"）⇒ 要做得改 Core，超出本项目范围。

### 五、学习线三条便宜实验的结果

- **(a) 整句路径**：见上一节 —— **点非最优整句不产生任何个人学习**（实测 + `commit/mod.rs:420` 的等值判断）。
  整句路径受个人数据影响这条路只能靠**词级上屏积累的 n-gram**间接生效，本轮没测那条间接路径 ⇒ **未证**。
- **(b) 撇号输入串**：`input keyevent 47 49 43 75 53 37`（`suo'yi`）上屏 蓑衣 后，
  `user-choices.tsv` 存的键是 **`suoyi`**（撇号被 `choice_key` 滤掉，`engine/mod.rs:555-560`）。
  再把 `user-ngram.tsv` 挪开（只剩 choice 键）冷启动：打 `suoyi` 与打 `suo'yi` **都是 蓑衣@0**
  ⇒ **两种写法共用同一个 choice 键**，且这次也第三次独立证明了「choice 键单独就能翻 3,168 倍的词频差」。
- **(c) μ 封顶 0.5**：**设备侧测不了**。μ = c(v)/(c(v)+8) 封顶 0.5 要 c(v) ≥ 8 才过拐点、要顶到封顶
  得几十次；而 `MAX_USER_TRANSITIONS = 200_000` 的减半要灌 20 万条转移。⇒ 只能靠 Core 侧单测
  （`blend_favors_seen_continuations_but_never_kills_unseen_ones` 那类），**不在本项目改 Core 的范围里**。
  本轮把 (c) 从"待做实验"改成"设备侧不可测，需要单测"，别再排期。

### 六、本轮测量故障（三条，都是自己的）

① **硬编码坐标不可信**：`收起键盘` 我按上一轮 dump 的 `[461,1372][614,1464]` 敲死 (537,1418)，
本轮该页布局变了 ⇒ 点空 ⇒ **会话没销毁 ⇒ 没落盘 ⇒ 我一度判成"提交没记学习"**。
`清空 @-1,-1` 也是同一原因（pick 不到就 tap -1）。
② **`class:android.widget.EditText` 的首个节点不是我要的输入框**（它给到 y=369，试用框实际在 1482 一带）
⇒ 按 class 取第一个节点会拿错；要么按 text 精确匹配，要么沿用已验证过的坐标。
③ **bash 里 `type_ suo'yi` 的裸单引号把后面十几行吞成一个字符串**（引号直到下一行才闭合）⇒ (b) 第一次跑
只输出了末尾两行，看着像"脚本漏打印"。特殊字符一律走 `input keyevent`（撇号 = 75）。
④ 另外补一条真机制：**`am force-stop` 会丢掉 ≤60 秒未落盘的学习**（第 6 条关闭时记过的固有边界），
本轮 step 0 的那次 蓑衣 提交就是这么丢的，不是 bug。⇒ **实验里要落盘就按 HOME 触发 `onFinishInput`→destroy**
（本轮 (b) 用的就是这条，`02:54:59 学习数据落盘 6 ms（会话销毁）` + 三张表齐）。

## Java 侧的 Bootstrap 契约

3.4.7 接 JNI 时必读。行号来自 `host/java/app/msime/android/`，全部本轮实测。

### 请求与回显

- `core/Bootstrap.java:43-44` — Java 向 native 发请求：`{"resources": "<filesDir>/bootstrap/resources", "state_root": "<filesDir>/bootstrap/state"}`。
- `core/Bootstrap.java:57` — native 回的 `value` 原样写进 `runtime-options.json`，之后全 App 从那份文件读。

### 两条硬约束（不符会静默失效）

1. **`resources` 必须回显请求原值。** `core/Bootstrap.java:376-377` 把配置里记录的 `resources` 与 Java 自己的 `bootstrap/resources` 做 canonical 比对，不等则 APK 升级后**永不重解词库**。
2. **`preferences_directory` 必须是 `state_root`。** `core/ResourcePacks.java:18` 与 `core/MSIMEInputService.java:217` 都把 `filesDir/bootstrap/state` 当成 `preferences_directory`。该键在全 host 出现 **13 处 = 8 处读出 + 5 处回填进别的 native 请求**。读出：`MSIMEInputService.java:1275`（统计偏好）、`:1293`（外观偏好）、`:1668`、`:1705`（会话 stateRoot）、`:1718` 共 5 处 `textOr`，加 `core/Telemetry.java:256`、`dictionary/CommonPhrasesStore.java:424`、`home/HostStore.java:47` 共 3 处 `readOption`/`runtimeOption`。回填：`core/Telemetry.java:258`、`home/CloudSync.java:363,640`、`home/LocalBackup.java:98,452` 共 5 处 `put`。那些 native 入口在阶段一属于 93 个 `not_implemented` 桩（现值：86 个，2026-10-13），回错目录不会立刻显形，但语义上仍然错。`candidate/CandidateTranslationPolicy.java:94` 还按它去找 `resource-packs/offline-glosses`。

### 词库落盘布局

`core/Bootstrap.java:259-260` 把 `assets/dictionary/<name>` **拍平**复制到 `resources/<name>`，所以设备上是 `resources/dict.tsv`，没有 `dictionary/` 子目录。`assemble.rs` 的 `dict_path` 必须按这个布局构造；用仓库路径会同时错过文件与 `:376-377` 的比对。

### 硬编码与锁文件的绑定

`dict.tsv` 硬编码在 `prepare_host` 中，与 `host/assets/desktop-dictionary.lock.json` 的 `artifacts[].name` 是绑定关系（锁当前只有一项；`host/assets/dictionary/dict.tsv` = 2,053,612 B）。未来锁里加第二个 artifact 时，`prepare_host` 的硬编码名不会自动跟着走——它会以 `DataFileMissing` 明确失败，不是静默错。可接受。

### `state_root` 的往返

`state_root` 只在 Java 侧发出（`core/Bootstrap.java:44`、`core/ImeKeyFeedback.java:203`、`core/NativeClient.java:198/351/357/370`、`core/ResourcePacks.java:66`），没有任何地方从 `value` 回读。`HostResources` 仍带上它，只为让 `runtime-options.json` 的结构完整。

## View 的候选窗口

上游把候选发给宿主前按页切过一次，`crates/input-runtime/src/runtime.rs:700-717`（`pub fn view`）。
`:701-702` 两行原样是：

```rust
let page = self.highlighted / self.page_size;
let start = page * self.page_size;
```

`:703-708` 算 `page_len = self.cached.candidates.len().saturating_sub(start).min(self.page_size)`
（上游把这串跨了 6 行排版），`:710-717` 取切片：
`.iter().enumerate().skip(start).take(page_len).map(|(index, text)| self.candidate(index, text))`
（同上，上游是一行一个方法调用）。后面两处为了可读性压成单行，token 与顺序未改。

两个要点：① `View.candidates` 是**当页**那 ≤ `page_size` 个，不是整表；② `id.index` 取
`.enumerate()` 的**整表下标**（`skip` 在 `enumerate` 之后）。本项目 `engine/src/view.rs`
的 `page_candidates()` 逐字对齐这两点，测试是 `only_the_current_page_reaches_the_host`。
写第一版时把整表 44 个候选全发出去了，`page` / `page_count` 就对不上，宿主第一页会画出全部候选——
这条是 3.6 静态校验阶段发现的，不是靠读上游代码提前避开的。

同一次改动里还有一个 Rust 侧的形状陷阱：`transition()` 拿到的 `built` 是
`Option<Option<Transition>>`（外层 = 会话是否存在，内层 = `requery()` 是否成功），
第一个匹配臂写成 `Some(transition)` 会把内层 `Option` 整个吞掉，`None` 一路走到
`serde_json::to_value` 变成 `{"ok":true,"value":null}`——`ok:true` 却给不出 `value` 对象，
Java 的 `value()` 解析抛 JSONException，最终照样降级，只是错误信封里的 `query_failed` 丢了。
编译器用 `unreachable pattern` 报出来的。**教训：`ok:false` 与 `ok:true + value:null`
在宿主眼里是同一种失败，但只有前者带着能查的 reason。**

## 启动路径的击穿判定

宿主对返回信封是硬解析：`value()`（`MSIMEInputService.java:844-848`）见 `ok != true` 就抛
`JSONException`。抛点若落在启动路径 `startEngineSession`（`:1691-1733`）的大 try 里，
后果是 `stop(false)` → `session = 0`（`:1597`）→ 键盘退化成纯直接输入，横幅
「共享运行时未就绪：仅直接输入」（`:1735`）。**在启动路径上「有 catch」不等于安全，等于会话死亡。**

判定一条调用点会不会击穿，最初用三条规则：**是桩** + **返回值被 `value()`/`apply()` 硬解析** +
**没有局部 catch**。按这三条扫完 62 个调用点，结论是「击穿点 1 处，在 `:1709` 的
`setEnglishModeRaw`」。真机第一轮证明这个结论不完整——见下。

### 击穿判定的第四类：real 方法在语义边界回 ok:false

C 的三条规则（stub + 硬解析 + 无局部 catch）覆盖不到：**real 方法**在
某些输入下自己回 `{"ok":false}`，被 `value()` 抛 `JSONException` 后击穿。
本次实例：`focusRaw` → `transition()` → `Session::requery()` 在空组字时
把 `ParseError::Empty` 误判为故障 → 回 `query_failed` → 启动路径死在这里
（`MSIMEInputService.java:1707`），与 `:1709` 的桩无关。
修法：`requery()` 不把解析结果当故障（上游 `runtime.rs:469-473` 在 `editing_text`
为空时照样 `refresh()` 并期望 Ok）。

**教训**：C 的机械判定只能找「桩」和「catch 覆盖」两类，找不出「real 方法
在边界条件下的错误返回值」。后者需要**探针实测**或**逐条 real 方法的边界
语义审计**，不是静态扫描能覆盖的。

### 修法的两次收窄与一次扩大（实测驱动）

第一版按上面那句修，只把 `ParseError::Empty` 当空候选。探针随即量出这不够：

```
PROBE 样本 36 个，Ok 33 个，Err 3 个："i"=NoSegmentation "i"=NoSegmentation "im"=NoSegmentation
```

即**单按一次 `i` 键**就给 `NoSegmentation`，只挖 `Empty` 仍会当场杀会话。而
`ParseError` 的三个变体（`qingjian-core/src/parser/error.rs:2-16`：`Empty` /
`InvalidChar` / `NoSegmentation`）**全部是「这串读不出拼音」**，没有一个是运行时故障；
`engine/query/phonetic.rs:40-46` 那段 Err 分支的语义也是「拼音没戏、英文/快捷也补不出来
⇒ 无候选」。⇒ 最终形态是 `requery()` **不返回 `bool`**：任何 `Err` 都落成空候选，
`transition()` 与 `setEnglishModeRaw` 里的 `Some(None) => query_failed` 臂随之删除，
`built` 回到单层 `Option`（两层 `Option` 正是本仓上一轮 `Some(transition)` 吞内层 `None`
那个坑的载体）。测试是 `unreadable_input_yields_an_empty_page` 与
`set_english_mode_advances_generation_and_flips_the_view`。

修完复扫：`:1698 create` / `:1707 focus` / `:1709 setEnglishMode` / `:3750 setEnglishMode`
**全部是 real**，启动路径上再无「桩 + 硬解析」组合，符号仍是 9 real + 93 stub = 102
（这是当时的值；2026-10-13 20:44 现值 17 real + 85 stub）。

### 击穿判定的第五类：catch 体调 `fail()`/`stop()` 的「局部 catch」

C 的三条规则和第四类都没覆盖：**有局部 catch 不等于安全**——要看 catch 体做什么。
按 catch 体重判 **48 个「桩 + 硬解析」站点**（`MSIMEInputService.java` 里 62 个 `NativeClient.`
调用点，其中 14 个是 real 方法；`setEnglishMode` 的两处 `:1709`/`:3750` 是本轮从桩转 real 的）：

| catch 体动作 | 站点数 | 后果 |
|:---|---:|:---|
| 调 `fail()` / `stop()` | **12** | 触发即会话死（不崩进程而已） |
| 软失败（写 `preferencesNotice` / 弹诊断 / 忽略） | 34 | 真降级 |
| 无 catch 但不解析返回值 | 2 | `:1911` 裸调；`:2258` 的 7 个调用方 catch 体内均无 `fail()` |

**当场复现**：`nihao` 出候选后点候选条右侧 ⌄（`allCandidates`，`:6334`，catch 在 `:6346`
是 `{ fail(); }`）⇒ 候选条与组字一起消失，横幅变「输入连接失败：仅直接输入」。
触发条件 `page_count > 1`（我们 44 候选 = 5 页）⇒ **主路径、一点就死**。

12 个站点覆盖 **10 个方法**（`punctuationWithContext` 与 `removeCandidate` 各有两处）：
`glide` `:3145`、`setNineKeyFilter` `:3158`、`punctuationWithContext` `:3250`
与 `:3273`、`resetCache` `:5959`、`removeCandidate` `:5968` 与 `:6039`、`pinCandidate` `:6034`、
`fixCandidatePosition` `:6035`、`clearCandidatePosition` `:6037`、`selectEdge` `:6233`、
`allCandidates` `:6334`。可达性与排期见下一节。

**2026-10-13 复扫现值**（`scan_bridges.py` 实测，上面那张表是阶段一的记录，不覆写）：
62 个调用点 = **real 23 + 桩 39**；桩里 **杀会话 5 / 降级 32 / 不解析 1 / 需人工定性 1**。
杀会话从 12 降到 5，是因为阶段二把 7 个站点转成了 real：B1 的 `:6034` / `:6035` / `:6037` /
`:6233` 四件止血、B2 的 `:5968` / `:6039` 接 `Engine::forget`、A 的 `:6334` 出整表。
**剩下的 5 个**：`glide` `:3145`、`setNineKeyFilter` `:3158`、`punctuationWithContext`
`:3250` 与 `:3273`、`resetCache` `:5959` —— 全部不在键盘主路径上（滑动输入、九键筛选、
标点上下文、缓存重置），所以「触发即会话死」的面比阶段一小了一大截，但**可达性仍未逐条
真机验过**：`glide` 要滑动输入、`setNineKeyFilter` 要九键、`punctuationWithContext` 要标点
联想、`resetCache` 要设置页入口。降级面从 34 少到 32 的那 2 处是 `allCandidates` 的另外两个
调用点 `:2586` 与 `:2662`（阶段一属降级，A 步转 real；上面那份 12 站点清单只记了 `:6334`
一处，因为它按 catch 体动作分类，另外两处当时不杀会话）。站点总数 62 不变，`real` 与
「杀会话/降级」互斥。

## 日志桥（`engine/src/logcat.rs`）实测与三个风险点

手写 `tracing::Subscriber`（7 个方法）+ `#[link(name = "log")]` 直连 `__android_log_write`，
**零新依赖**（`Cargo.toml` 一字未改）。装 subscriber 而不是换成自家宏，是因为青简 Core 自己
用了 `tracing`（实测 23 处 `warn!` + 6 处 `info!` + 30 处 `debug!`），一并接出来且不改 Core。
门限 INFO：`tracing-core-0.1.36/src/metadata.rs:945-950` 的 `Ord` 是反着比的
（`(other.0).cmp(&self.0)`）⇒ 越啰嗦数值越大 ⇒ 留 ERROR/WARN/INFO 的写法是 `<= Level::INFO`。

**真机实测（OPPO PEGT10）**：`I qingshan:` 三条（自证行 + 词库载入 55/58/69 ms + 会话建立
56/58/72 ms），`W qingshan:` 一条 —— 把 `files/bootstrap/resources/dict.tsv` 临时改名造出
`回 ok:false: create_failed: 词库加载失败: failed to read dictionary: No such file or directory (os error 2)`，
随后改回并复验 `nihao` → 「你好」上屏。**改名不会触发重解包**（Bootstrap 没有自愈）。

**体积**：arm64 **+34,400 B** / x86_64 **+31,224 B**（方案里预测 6–30 KB，**超出上界**）。
拆开：`Backtrace::capture()` 一条占 6,576 / 6,528 B（临时探针实测后删探针、还原源码、
复验体积回到原值）；剩下约 27.8 KB 是 subscriber + 字段 visitor + panic hook + `format!`/`CString`。
`NEEDED` 只多 `liblog.so`，102 个导出与 `JNI_OnLoad = 0` 不变。

**三个风险点（批准方案时点名要入档的）**：

1. `set_global_default` 失败（别处已装过 subscriber）⇒ 退到 `eprintln!`，不 panic。
   但设备上那行提示不带我们的 tag，等于看不到，只能靠「自证行没出现」反推桥没装上。
2. panic hook 里 `format!` 与 `Backtrace::capture()` 都要分配内存：堆已经坏了的情况下
   在 panic 路径上再 panic ⇒ double panic ⇒ abort。`release` 是 `strip = true`，
   强抓栈也拿不到符号，所以刻意用尊重 `RUST_BACKTRACE` 的 `capture()` 而不是 `force_capture()`。
3. 一次 FFI 路径的 panic 出**两条**日志（hook 一条、`wire.rs` 边界一条），文本互相指认。
   不是冗余：青简后台线程（`qingjian-rescore`）上的 panic 不经过 `with_env`，只有 hook 那条。

**验证口径**：`cargo test` 编不到 `cfg(target_os = "android")` 那半边（环境陷阱第 13 条就是这么栽的），
所以动这个文件必须走完 `cargo ndk … build --release` + 真机 `adb logcat -s qingshan` 两步才算验过。

**未实测的两条**（别当成已验）：

1. **青简 Core 自己的 `warn!` 能不能落 logcat**。桥按 level 过滤、不看 target，理论上一定落，
   但现在构造不出触发条件（`flush_learning` 没接，见上节。现值：2026-10-14 已接上，但成功路径走
   的是 `info!`，`warn!` 那条要写盘失败才会响，仍然构造不出 ⇒ 本条状态不变，仍未实测）。
   顺带纠正我原方案里的一个错：探针目标写成 `files/qingshan/user.tsv` 是**名字错了**——
   `user.tsv` 只是我们自选的词频文件名，
   五张兄弟表里那张叫 `user-words.tsv`（`qingjian-learning/src/frequency_learner/mod.rs:18`）。
2. **panic 的双条日志**（hook 一条 + `wire.rs` 边界一条）。造它要临时往代码里塞 `panic!`，
   成本大于收益，按裁定记「未实测」。

**`Backtrace::capture()` 那 6.5 KB 值不值（评估，未改代码）**：`[profile.release]` 是
`strip = true`，符号表被削，所以即使设备上开了 `RUST_BACKTRACE=1`，多半只剩裸地址与 `<unknown>`
（**这条也没实测**，设备上从没 panic 过）。真正有用的是 hook 里 `info.location()` 给的
`文件:行号`——它来自 panic 宏内联的 `Location`，与 strip 无关、不要钱。
⇒ 这 6,576 B（arm64，`.so` 的 0.15%）买到的只是「调用链的地址」。
结论：**先留**。真要设备栈符号，该做的是留一份未 strip 的构建产物，那是比这一句大得多的口子，
届时与 `strip` 一起议，而不是现在为省 6.5 KB 删掉。

## 会话建立后的 stub 降级面（阶段二排期）

分类口径、站点计数与复现证据都在上一节「击穿判定的第五类」：**48 个桩站点里 12 个的
`catch` 体调 `fail()`/`stop()` ⇒ 触发即会话死**，34 个软失败，2 个不解析返回值。
这里只记**可达性**与排期（阶段一固定全拼、26 键、无九键/注音）：

- **可达、已真机复现**：`allCandidates`（`:6334` 候选面板展开，catch 在 `:6346` 调 `fail()`）。
  点候选条右侧「收起/展开」那个 ⌄ 就死：候选条与组字一起消失，横幅变
  「输入连接失败：仅直接输入 · 设置读取或应用失败，保留当前设置」，键位退回半角。
  触发条件 `page_count > 1`，而 `nihao` 有 44 个候选、5 页 ⇒ 属主路径。
- **可达**：`selectEdge`（`:6233`）、`pinCandidate`/`fixCandidatePosition`/`clearCandidatePosition`/
  `removeCandidate`（`:6034-6039`，候选长按操作）、`resetCache`（`:5959`，设置页清缓存）、
  `glide`（`:3145`，滑行输入手势）、`setNineKeyFilter`（`:3158`，九键筛选——阶段一给不出九键，
  实际不可达，但入口在）。
- **不可达（被方案门挡住）**：`punctuationWithContext`（`:3250`/`:3273`）——`:3248` 的
  `if (zhuyinSchemeActive())` 才走，而阶段一固定全拼（`scheme = 0`）且偏好读写属桩 ⇒ 注音开不了。
  它仍是阶段二要做的，但优先级低于上面那些。

按「离主路径的距离 + 是否杀会话」重排：

- **P0（点一下就死会话）**：`allCandidates`（`:6334`）、`removeCandidate`（`:5968`/`:6039`）、
  `resetCache`（`:5959`）。候选长按另外四件 —— `pinCandidate`（`:6034`）、
  `fixCandidatePosition`（`:6035`）、`clearCandidatePosition`（`:6037`）、`selectEdge`（`:6233`）
  —— 已在 B1 转为良性空操作（`ok:true` + 原样 View + `handled:false`），
  `removeCandidate` 在 B2 转真实现。

  量体积时记一条：`pinCandidateRaw`/`clearCandidatePositionRaw` 与
  `fixCandidatePositionRaw`/`selectEdgeRaw` 当前共享实体（`llvm-nm --print-size` 同地址），
  是链接器对函数体相同者的合并。将来若其中一个转真实现，另一个会分离——地址不再相同不是回归。
- **P0（核心卖点，不杀会话）**：译词显示 —— Core 侧
  `qingjian-core/src/engine/commit/mod.rs:28 pub fn annotate(&self, list: &mut CandidateList) -> AnnotationReport`
  与 `engine/setup.rs:375 pub fn with_translator(mut self, translator: Box<dyn Translator>) -> Self`，
  候选字段是 `candidate/mod.rs:42 pub translation: Option<Translation>`（**是结构体，不是 String**，
  `candidate/translation.rs:6-13`：`language` + 最多 `MAX_SENSES = 2` 条 `senses`）
- **P1（输入体验）**：`glide`（`:3145`）、`setNineKeyFilter`（`:3158`）、`shuangpinKeyHints`（`:3660`）、
  `emojiCatalog`（`:4686`）、`punctuationWithContext`（`:3250`，注音方案解锁后才可达）
- **P1（消掉那条横幅）**：偏好读写 `loadPreferences`/`savePreferences`/`updatePreferences`
  —— 现在键盘右上常驻「设置读取或应用失败，保留当前设置」就是它
- **P1（学习数据落盘）—— 已落地（2026-10-14 00:54，见「flush_learning 落盘」一节）**：
  原来只接了读（`FrequencyLearner::from_path`）、**从没调过 `Engine::flush_learning`**
  （`crates/qingjian-core/src/engine/learning/mod.rs:188`）⇒ 词频/用户词/n-gram 全在内存里。
  现在接成「会话销毁必落 + 上屏满 60 秒节流落」，真机 `files/qingshan/` 已出现
  `user.tsv` / `user-choices.tsv` / `user-ngram.tsv` 并跨冷启动累加。
  连带后果也消了：Core 那批「用户词频已保存」`info!` 现在能构造出触发条件，
  实测落进 logcat ⇒ 日志桥那个验证盲区补上了一半（`warn!` 仍未直接观测到）。
- **P2（辅助功能）**：翻译/释义（`applyTranslations` `:2365` 等）、皮肤（`resolveTheme` `:4400`、
  `themeCatalog` `:4414`）
- **P3（远端/可选）**：云/AI 路径、语音

行号出处均为 `host/java/app/msime/android/core/MSIMEInputService.java`。

## 句子级译词的取证（2026-10-13 评估轮：只出报告，未立项）

**一句话结论**：整句候选没译词不是 annotate 的缺陷，也不是数据裁得不够——`annotate` 只做
**整串精确查表**（`crates/qingjian-core/src/engine/commit/mod.rs:46`，miss 时没有任何
子串/前缀回退），而整句候选的文字**既不在 `dict.tsv` 也不在任何一张译词表里**（它是
`CandidateKind::Sentence`，由 `sentence/viterbi.rs` 现拼），所以按定义必然 miss。

同一处还有第二条规则要记：`CandidateKind::Custom(_) => None`（`commit/mod.rs:40`）——
**自动造出来的用户词永远不标译**，所以「同一个句子打多了变成用户词」也不会自己长出译词。

### 表的粒度（实测，脚本已删，数字可复算）

| | 随包子集 | 上游全量 |
|:---|:---|:---|
| 数据行 | 76,273 | 232,213 |
| 词头含空格 | **0** | **0** |
| 最长词头 | 8 字 | 8 字（`一人之下万人之上`） |
| 词头字数 1/2/3/4/5/6/7+ | 7.9 / 66.9 / 16.0 / 8.6 / 0.45 / 0.14 / 0.09 % | 3.2 / 34.1 / 25.3 / 21.3 / 8.6 / 4.4 / 3.0 % |

- 表是**词与固定短语**级：成语、谚语、诗句条目有（6+ 字 17,351 条），但没有自由句。
- 覆盖天花板：`dict.tsv` 92,330 个词头里，只有 **88,349** 个能在全量表查到；子集已收 76,273，
  即**再补 12,076 条（其中 4 字及以上 1,706 条）就到顶**，之后每加一条都是白加体积。
- 真机观察到的四个整句（`今天天气很好`/`我们是中国人`/`谢谢你的帮助` 及其变体）
  在**词库、子集、全量三处都不存在**；它们的组成词（`今天天气`/`今天`/`天气`/`很好`/
  `我们是`/`中国`/`人`/`谢谢`/`你`/`的`/`帮助`）**三处都在**。

### 宿主其实有第二条释义通道（本轮新发现，先前所有文档都没记）

`scheduleCandidateGlosses()`（`:2568`）→ `allCandidates` → `NativeClient.candidateGlosses`
→ `NativeClient.applyTranslations`（`:2653`）→ `render()`。要点：

- **画不画由 `candidateEnglishGloss || candidateTranslationsEnabled` 决定**
  （`candidateAnnotation()` `:6147-6150`）。而 `candidateTranslationsEnabled` 在
  `preferences == null` 时**恒 true**（`:1986-1987`）——我们现在看到的译词就是从这条
  门控过的 `translation` 字段画出来的。
- **请求侧的门 `candidateEnglishGloss` 在 `preferences == null` 时是 false**（`:1972-1973`），
  我们偏好读失败 ⇒ 通道整条关着。旁证：`candidateGlossesRaw`/`applyTranslationsRaw`
  两个桩（`engine/src/stub.rs:75`、`:77`）真机 logcat **0 次调用**（缓冲区里同期有 5 条
  `会话建立`，标签是通的，所以 0 不是 grep 假零）。**该标志在真机上的实际值仍未直接核实**，
  这是按代码推的。
- 这条通道**失败全静默、不杀会话**（`:2630`、`:2670` 两处注释都写明是可选显示态），
  和「击穿判定」第五类那些 `fail()` 站点性质不同——接它没有击穿风险。
- 宿主把 `translation` 按**换行**切成多语言并列行（`CandidateTranslationPolicy.java:40-52`
  `insertionGlosses`，`:43` 是 `split("\\R")`；每行 ≤4096 B、最多 `MAX_TARGETS` 行）。
  这正是 PRD 3.2.2 禁止我们采用的结构；我们的 `translation_to_string` 刻意单行，
  接这条通道时必须继续只发一行，否则破约束。
- 体积上限：请求 `GLOSS_REQUEST_LIMIT = 262_144`、应答 `GLOSS_RESPONSE_LIMIT = 1_048_576`
  （`NativeClient.java:23`、`:25`），超限直接 `IllegalArgumentException`。

### 排版预算（真机像素量，`e1cdb946` 1080×2400）

- 候选条第二行可用宽实测 **130–155 px ≈ 14–20 个拉丁字符**，且**裁切不加省略号**：
  `n. today's weather`（18 字符）在候选条上被切成 `n. today's wea`。
- 展开面板格约 200 px，`n. today's weather`、`adj. near · adj. close`、
  `v. exhaust · v. use up` 都放得下——**面板比候选条宽裕**。

### 体积与耗时（实测；APK 侧按 DEFLATE 实算）

| 方案 | 文件 raw | APK 内 stored | 相对现包 |
|:---|:---|:---|:---|
| 现子集 76,273 行 | 1,815,026 B | 812,946 B | — |
| 词库∩全量 88,349 行 | 2,115,787 B | 821,473 B | **+8,527 B** |
| 上游全量 232,213 行 | 6,788,267 B | 2,574,546 B | **+1,761,600 B** |

（stored 列是 zlib level 9 实测，APK 里 `assets/dictionary/*` 走 DEFLATE，两个 `.so` 走 STORE；
现 APK 49,659,177 B。）载入耗时实测：`词库载入 48–53 ms`、`译词表载入 55–59 ms / 1,815,026 B`，
即约 **31 µs/KB**；按此线性外推 88,349 行版约 +9 ms、全量版约 +153 ms——**外推值，未实测**。

### Core 的切词能力：有，但我们现在用不了

`qingjian_core::sentence::segment_text(text, &dyn LanguageModel)`
（`crates/qingjian-core/src/sentence/text_segment.rs:17`，`MAX_TEXT_WORD_CHARS = 6`）
确实是对汉字串做 Viterbi 切词的公开函数，`Engine::language_model()` 也拿得到模型
（`engine/setup.rs:348`）。**但**：它的「是不是词」判据是**语言模型词表**（`lm-unigram.tsv`），
不是译词表也不是 `dict.tsv`；而我们从没调 `with_language_model`，缺省是 `NoLanguageModel`
（`engine/mod.rs:410`，其 `log_prob` 恒 `None`，`sentence/language_model.rs:12-16`）
⇒ `segment_text` 直接返回 `None`。且 `data/generated/lm-*.tsv` 在本仓**根本不存在**
（`ls data` → No such file），要自己跑 `tools/dict-convert` 从维基+LCCC 语料生成。
另记：`Dictionary` 侧**没有**按汉字文本查「这是不是词」的公开 API（只有按音节
`lookup`/`lookup_exact_alt` 与全量 `entries()` 扫描，`crates/qingjian-dictionary/src/dictionary/mod.rs:360`、`:393`、`:333`）。

### 裁定（2026-10-13 13:40，用户按我给的默认推荐逐条确认）

1. 形态：**a + b 分两步**——先 A3（手工句子条目，零代码见效），再 B（拆词拼接，句子级正式方案）。
2. 体积上限：批的是 **+8.5 KB**；**A1 实际落地 +130,640 B，超批 15 倍**。
   错因是我把两个不同压缩器的数相减：拿 APK 里实测的 stored 812,946 B 去减我自己
   zlib level 9 估的 821,473 B。同口径（都 zlib9）应是 708,040 → 821,473 = **+113,433 B**，
   与 AGP deflate 实测的 +130,640 B 同量级。**保留还是回滚等用户复核**。
3. 句子译词**只在展开面板显示**（候选条第二行 130–155 px 装不下）。
4. 拼接样式：**去词性前缀**，分隔符用 `, `（` · ` 已是我们多条释义的分隔符，不复用）。
5. **不接受**联网/账号做句子级翻译。
6. A3 句子清单：**agent 从词库高频组合挑 200–1,000 条给用户审**，审过才入表。
7. 路线 D：**挂到「偏好读写」P1 之后**（它的请求门 `candidateEnglishGloss` 在
   `preferences == null` 时是 false，见上面那条通道小节）。

### A1 落地后留下的两处代码措辞陈旧（各一行改动，等批准，本轮按「只动数据」没碰）

- `engine/tools/build_glossary_subset.py`：`DEFAULT_TOP_N = 80_000` 与 docstring 的
  「为什么是 80k」。现随包表是用 `--top-n 93236` 生成的；**默认值不改的话，下次谁不带参数
  跑一次就把覆盖退回 76,273 行**，而且不会报错，只会让表变小。
- `engine/src/assemble.rs` 的 `shipped_glossary_subset_parses` panic 文案「随包的 80k 子集…」，
  现值 88,349 行 / 全词库交集。

### A1 的三个覆盖口径（拍板时只看第一个会误判）

| 口径 | A1 前 | A1 后 | 说明 |
|:---|:---|:---|:---|
| 词库侧（唯一词头有译词） | 82.6% | **95.7%** | 我拿去拍板的数，**最会骗人** |
| 词频加权（敲到的词有译词） | 93.438% | **93.463%** | 只涨 **0.026 pt**；新增 12,076 词词频合计 70,916 / 全库 275,814,283，最高频的是「调演」15，**≥100 的一条没有** |
| 候选侧（同音节串全部候选带译词比例） | — | — | `ni` 66.7%→**81.5%**、`shi` 84.1%→**95.1%**；`jin tian`/`xie xie`/`de`/`ni hao` 都 **100%→100%** |

⇒ A1 的实际作用面只有**单音节、同音词多**的输入翻页。真机 `nihao` 首页与 A1 前逐格相同；
`你好好`/`你好久`/`你好看`/`你哈` 仍无译词，**因为它们在全量表里也不存在**，
不是子集没裁到（我上轮这么说过，是错的）。

### A3 第 1 批：197 条，前提已在真机验过

判据 = 整句能被 `dict.tsv` 词头做 **100% 最长正向匹配**切分（切不出来说明引擎拼不出这个
整句候选，加了也永远不命中）。197 条全过，最长 8 字、平均 2.2 段、要敲的拼音平均 6.1 个字母、
最长 15 个；其中 146/197 条的**每个分段在现译词表里都已有译词**（剩下 51 条有段查不到，
是路线 B 拼接时的兜底问题）。清单在 `/d/toolchains/tmp/a3-sentence-review.md`（**没进仓库**，
等审过再落文件、再入表）。

前提验证（真机 `e1cdb946`，14:14，装机 `lastUpdateTime` 已随 A1 更新）：

| 敲的拼音 | 页码 | 目标整句在候选里吗 | 有第二行译词吗 |
|:---|:---|:---|:---|
| `womenhuijiaba` | 1/28 | **第 2 格 = 我们回家吧** ✓（第 1 格是 我们回家啊） | 无 |
| `shoujimeidianle` | 1/51 | **第 1 格 = 手机没电了** ✓ | 无 |
| `jintiantianqihenhao` | 1/21 | 第 1 格 = 今天天气很好 ✓（上一轮已测） | 无 |

⇒ 「整句确实是引擎给出的候选，只是表里没这一行」这条前提成立，A3 加行就会亮。
**但注意**：整句候选的**排序**我们没接语言模型（缺省 `NoLanguageModel`，见上面
「Core 的切词能力」），整句名次由一元词频与个人 n-gram 决定，所以「我们回家吧」排在
「我们回家啊」后面——A3 只补译词，不改排序。

### A3 疑问清单（14 条，等你裁；其余 150 条已入表）

| 中文 | 我的英文 | 疑问点 | 建议改法 |
|:---|:---|:---|:---|
| 你今天怎么样 | `how is it going today` | 漏译「你」 | `how are you today` |
| 我同意我的意见 | `i agree` | **中文本身冗余**（像「我同意」和「这是我的意见」串了） | 换成 `我同意` → `i agree`，或 `这是我的意见` → `this is my opinion` |
| 麻烦你了 | `sorry for the trouble` | 递东西/拜托时更偏**致谢**，我按致歉译了 | `thanks for your trouble`（致谢）或标两可 |
| 多关照 | `please look after me` | 中文常说**多多关照**，「多关照」略不自然 | 换成 `多多关照` |
| 给我一张发票 | `give me a receipt` | 中国语境的**发票**是税务发票，不是收银小票 | `an invoice, please` 或 `the fapiao (official receipt)` |
| 车来了 | `the bus is here` | 我把「车」具体化成 bus，原文未指明 | 定一个：bus / taxi / your ride，或保留歧义 |
| 我早上八点起床 | `i get up at eight` | 漏译「早上」 | `i get up at eight in the morning` |
| 发个微信给我 | `send me a message` | **专有名词「微信」被泛化**（113「无线」→ `wifi` 那个是对的） | `send me a WeChat message` |
| 我需要吃药 | `i need medicine` | 漏译「吃」这个动作 | `i need to take medicine` |
| 这个多少钱一斤 | `how much per jin` | 量词 `jin` 未译，英文读者不懂 | `how much per 500 g` 或 `per jin (500 g)` |
| 来两斤 | `two jin please` | 同上 | `two jin (1 kg), please` |
| 没有事的 | `everything is fine` | 原文是**安慰**语气（=别担心），我译成陈述状态 | `it is okay, do not worry` |
| 说话算话 | `keep your word` | 我给了祈使；单说这句多为**承诺或提醒**，语气不同 | 承诺 `i keep my word` / 提醒 `keep your word`，请定 |
| 真的不用 | `really no need` | 机翻腔 | `really, do not bother` |

**全局样式一条**（影响所有条目，我按「与现表一致」先定了默认）：现表 88,349 行**全小写**
（`int. hello · int. hi`），所以我写的整句也全小写（`i am home`、`good morning`）。
整句用小写读起来确实像机翻，但改成首字母大写 + `I` 大写会让整句区与词区两种风格并存。
**默认保留小写**；要改就是一次机械重写（150 行 + 追加 14 行），我这边几分钟。

### A3 落表时踩到的一次自纠

第一版我按**行号**挑疑问项，把没问题的 53「我不太饿」当成疑问、漏了真正可疑的
32「我同意我的意见」。发现后**回滚到 A3 追加前的快照重做**，最终 150 落表 / 14 待裁 /
33 条其实已在表里。教训：**按内容核对，别按编号记**。

### A3 落地后新出现的一个流程缺口（要改脚本逻辑，等批准）

`build_glossary_subset.py` 只生成「词库 ∩ 全量」那部分，**不知道 A3 追加的存在**：
谁重跑一次脚本，150 条整句就被冲掉，体积只小 2.9 KB，不会有人发现。
本轮的处理是：A3 条目单独存成 `engine/tools/glossary-sentences-zh-en.tsv`（源件），
生成出的表尾部追加，并插两行 `#` 注释指回源件（`Glossary::parse` 跳过 `#` 行，安全）。
**更稳的做法**是给脚本加 `--append`（默认指向那个源件），生成时一并写入，约 6 行：
读文件、按词头去重、追加。要不要做等你说。

### 裁定补充（2026-10-13 15:03 / 15:20）

**14 条疑问全部采纳我的建议**；其中两条按用户授权展开：第 2 条取「我同意」（弃
「这是我的意见」），第 6 条拆成「公交车来了」+「出租车来了」⇒ **14 条 = 15 行**，
源件从 150 行变 **165 行**，表 88,499 → **88,514 行**，APK 49,792,717 → **49,793,061 B**。

**全局小写：保留。** 现表 88,349 行全小写，整句区 165 行也全小写，**这是刻意与词区风格
一致，不是遗漏**。将来若改首字母大写 + `I` 大写，是一次机械重写（165 行 + 第 2 批），
不是必须项。

**`--append` 已加并双向逐字节验证**：默认指向 `glossary-sentences-zh-en.tsv`，
`--no-append` 显式只出子集。实测 `--no-append` 输出与 A3 之前快照 `cmp` 相同、
默认与显式 `--append` 输出相同。docstring 里写明了「不 `--append` 重跑会冲掉 A3 条目」
这条活雷。

**我上轮量错的一个数**：候选条第二行「14–20 字符」是按**窄格**量的；真机 A3 落地后看到
**选中态第一格放得下 25 个拉丁字符**（`the weather is nice today` 完整显示），
面板格也不截断。⇒ 下面 B 的「22 字符预算」只适用于窄格，**落地前必须按格宽分别重量**。

### B 设计稿的降级裁定（2026-10-13 15:20，暂缓实现）

**用户裁定：B 降级为「只拼 2 段」的最小版本，且暂缓实现。** 理由记在案：
A3 已覆盖 70/197 的高质量场景；B 的产出质量在 3 段以上崩
（`谢谢你的帮助` 拼出来是 `thanks, you, of, help`，**语义已经错了，不是"不够好"**）；
长尾整句里能切成 2 段的比例未知，边际价值不确定；先扩 A3 到 500 条再看长尾剩多少。

四个待裁点按降级口径定档：

| 待裁点 | 裁定 |
|:---|:---|
| 虚词跳过集 | `的 了 吗 吧 呢 啊 着 过 么`（保持；都是词库有、译词表无的单字，实测） |
| 可读性上限 | **只拼 2 段**（收紧：段数 ≠ 2 直接不出译词） |
| 面板 22 字符预算 | **暂不放宽**（但要按格宽重量，见上面那条纠错） |
| B 做不做 | **暂缓**，等 A3 扩到 500 条后评估 |

架构结论仍留档备用（若将来重启 B）：`Translator::translate` 看不到 `CandidateKind`，
所以用 `Arc<Glossary>`（`.tsv` 走 `Storage::Owned(HashMap)`，`Send + Sync` 成立）
一份塞进 wrapper translator、一份留在 session，`requery()` 里按 kind 后处理；
候选条与面板共用 `candidate_row`，"只在面板显示"要加 `for_panel` 参数，
会**故意让两条路径内容分叉**（形状不分叉），理由必须写进注释否则后人当 bug 修。
另记一条小事实：随包 A3 条目里已有 4 行英文含 `, `（`an invoice, please` 等），
与 B 的分隔符 `, ` 同形——不会算错（两者不会出现在同一行），但人读会分不清是拼的还是手写的。

### A3 第 2 批：291 条草稿 → 补 12 条 → **303 条已落表**（15:40）

校验判据与第 1 批同：整句能被 `dict.tsv` 词头 100% 最长正向匹配切分。
316 条草稿 → 与第 1 批重复 3 条（`我请你吃饭`/`我先走了`/`我无所谓`）剔除 →
已在译词表 22 条不重复追加 → 291 条；再补 12 条凑满 **500 条整句语料**（197 + 303）。
0 条切不出；段数 1/2/3/4/5 = 24/121/125/38/5。最长英文 31 字符（`别晒着了`）。
含 `, ` 的整句条目 4 条（`别担心是小问题`/`我保证不再`/`别急慢慢说`/`我累了想躺会儿`），
第 1 批另有 4 条 ⇒ 全表 8 条。**与 B 的分隔符 `, ` 同形**：不会算错（两者不会出现在同一行），
但人读会分不清是拼的还是手写的。

**14 条疑问按用户裁定全部采纳建议改法**（`i will take the bullet train`、
`the ac is not cooling`、`take this 3 times a day`、`i am five minutes late`、
`i cannot hear your voice`、`i have a cold and a fever`、`we have not talked in a while`、
`this major leads to good jobs`、`eat small frequent meals`、`is cash on delivery ok`、
`do not worry, it is minor`、`i am tired, let me lie down`、`how do you pronounce this`、
`do not stay in the sun too long`）。

### A3 第 2 批：我自改的 17 条（不认同哪条就说，我改回哪条）

| 中文 | 我原来写的 | 改成 | 为什么 |
|:---|:---|:---|:---|
| 老板找我 | `the boss is looking for you` | `the boss is looking for me` | **人称错**，找的是我 |
| 我要考研 | `going to grad school` | `i am taking the postgrad exam` | 考研是入学考试，不是上研 |
| 我保证不再 | `i promise not again` | `i promise, never again` | 原英文不成句 |
| 再说就晚了 | `too late to say more` | `it will be too late if we wait` | 语义偏了（原文=再拖就来不及） |
| 别玩手机了 | `stop playing on the phone` | `get off your phone` | 英语搭配是 play with / get off |
| 我买件衣服 | `buy some clothes` | `i will buy some clothes` | 丢主语，读起来像祈使 |
| 我拍照给你 | `take a photo for you` | `i will take a photo for you` | 丢主语 |
| 放大一点 | `a bit bigger` | `zoom in a bit` | 图像语境「放大」= zoom |
| 头发长了 | `hair got long` | `my hair is long` | 缺所有格 |
| 请教一下 | `may i ask for guidance` | `may i ask you something` | 生硬 |
| 加班到很晚 | `working late overtime` | `working late` | overtime 冗余 |
| 再给我一次机会 | `one more chance` | `give me one more chance` | 丢了「给我」 |
| 这道题不会 | `do not know this problem` | `i cannot do this problem` | 中式英语 |
| 我要换个灯泡 | `change the light bulb` | `i will change the bulb` | 丢了「我要」 |
| 别急慢慢说 | `no rush take your time` | `no rush, take your time` | 两分句间缺标点 |
| 我住几楼 | `what floor do i live on` | 头词改 **你住几楼** + `what floor do you live on` | 真实场景多半问对方 |
| 堵车的 | `traffic is heavy` | 头词改 **堵车了**（英文不变） | 「堵车的」不成句 |

### 电报体：保留（2026-10-13 15:40 裁定）

第 2 批及后续英文沿用**电报体**（省主语/所有格：`paid the water bill`、`account locked`、
`cannot hear you`），与已落表的第 1 批一致。**这是刻意的**：短、不容易被候选条与面板格截断。
将来若改成完整句式，是一次批量机械重写，不是必须项。
（上面那 17 条里有 4 条我把主语**补回去**了——那不是改风格，是因为省了主语后英文被读成祈使句、
语义就变了。风格与语义错是两回事。）

### `refreshHostRaw` 的契约事实（2026-10-13 评估轮，只查不做）

**调用链**：`HomeActivity.java:155` → `FirstRunPreparation.startIfNeeded` → 工作线程上
`Bootstrap.prepare(context)`（`Bootstrap.java:23`，持 `bootstrap.lock` 文件锁）→
已有配置时 `refreshExistingConfiguration`（`:372-384`）→ `refreshHost(configuration)`
（`:387-395`）→ `NativeClient.refreshHost(绝对路径)` → `refreshHostRaw(jbyteArray)`（桩在
`engine/src/stub.rs:114`）。

**应答契约**（`Bootstrap.java:388-394` 逐字）：`{ok:true}` ⇒ 成功，返回 null；
否则取 `error` 字符串，**只有 `error.startsWith("dictionary_outdated")` 才继续**，
且还要满足「配置里记录的 `resources` canonical 路径 == 本宿主的 `bootstrap/resources`」
（`:376-377`），才 `extractDictionary` 重新解包、再刷一次。其他任何 error 一律直接 return。

**上游怎么做**（`msime-develop`，只读参考）：`crates/host-api/src/lib.rs:1626`
`DICTIONARY_OUTDATED_PREFIX = "dictionary_outdated:"`；`:1629-1639` 的 `outdated_resources`
**只把 `ResourceError::Integrity`（长度/摘要不符）和 `ExistingGeneration`（目录里文件对不上）
映射成 outdated**，`InvalidManifest`/Io 不算。锁是**编译期烘进二进制的**
（`crates/client-core/src/resources.rs:959` `serde_json::from_str(include_str!(
"../../../resources/desktop-dictionary.lock.json"))`），结构是
`ResourceSet { source_commit, artifacts: Vec<Artifact> }`（`resources.rs:30-34`）。
我们那份 `host/assets/desktop-dictionary.lock.json` 是手写的极简版，**只有文件名、
没有长度和摘要**（87 B），所以上游这套比对我们目前无条件可用。

**关键实测**（支撑「用摘要判定」可行）：设备上解包出来的
`files/bootstrap/resources/glossary-en.tsv` 的 sha256 = `48763920713991d7…619db34e`，
与本地待解包快照 `sha256sum` **逐位相同** ⇒ 解包是字节忠实的，摘要可比。
`dict.tsv` 也一样（`1407e986779d6f40…`）。

**代价实测**：`run-as … sha256sum` 两个文件（2,053,612 + 2,121,929 B）单次采样
**73.7 ms**（含进程启动）。第一次我量出 0 ms 是假零（命令没按预期跑，`date +%s%3N`
那条写法把 stderr 也吞了）——按 PIN 第 7 条重测才拿到真数。
`extractDictionary` 走 `copyAsset`（`Bootstrap.java:421-431`）：临时同级文件 + `ATOMIC_MOVE`，
不会读到半截文件，正被 mmap 的旧 inode 也不受影响。

**风险面**：`refreshExistingConfiguration` 自己 `catch (Exception | LinkageError)`
（`:380-383`），`FirstRunPreparation` 外层也 catch 并把状态置 `FAILED`（`:66-74`）。
所以这条路上**没有 `fail()`/`stop()`，不会杀会话**——不属「击穿判定」第五类。
最坏情况是首页显示准备失败 + 一个重试按钮（`KeyboardFragment.java:108`），键盘照常能用；
`MSIMEInputService` 根本不调 `Bootstrap.prepare`。

**推荐方案两条（代价从低到高），细节见本轮报告**：
① 无状态长度比对：把 `(name, length)` 烘进 `.so`，`refreshHostRaw` 只 `stat` 资源目录里那两个文件，
长度不符就回 `dictionary_outdated:`。零哈希、零新依赖。
② 长度 + sha256 两级比对：长度全等时再比摘要（多 ~74 ms/启动，可只在长度相等时算）。
两条都不动 Java。

### 17 条自改的裁定（2026-10-13 16:23）

**全部采纳，一条不驳回。** 用户原话理由：「都是译文质量问题，不是产品决策，你的改法每条都成立」。

**把「老板找我」这条留作自审有效性的证据**：我第一版把它译成 `the boss is looking for you`——
**人称硬错**，找的是「我」不是「你」。这类错不会让任何测试变红（表能解析、词头唯一、
真机也有第二行显示），只有**逐条读英文**才能抓到。所以「落表前自审英文列」这一步
不是仪式，它是目前唯一能拦住语义错的关卡。

两条「只报告不改」也按报告处置：

- `内存满了` → `storage is full` **保持**。本产品面向手机，手机语境「内存」就是存储；
  PC 语境才该是 RAM。不改。
- 全表 **8 条整句英文含 `, `**（第 1 批 4 条 + 第 2 批 4 条），与 B 的分隔符 `, ` 同形。
  **B 已暂缓，本轮不处理**；等 B 重启时一并解决（要么给 B 换分隔符，要么把这些条目
  的逗号改分号/句号），现在改反而会让 B 的口径没有对照。

### A3 第 3 批与「可达率」这个新发现的门槛（2026-10-13 16:55）

第 3 批：518 草稿 → 514 可用（3 条含拉丁字母切不出、1 条与既有语料重）→ 取 496 凑满
**整句语料 1,000 条**。自审再改自己 **28 处**，典型几类：

- **拼写错**：`过下凉水` → 我写的 `rince with cold water` → `rinse`。
- **不合语法**：`晚饭别等我了` → `do not wait dinner for me` → `do not wait up for me`。
- **疑问句译成陈述**：`听懂了吗` → `got it` → `did you get it`（与 `没听懂` 撞了口径）。
- **口径不一致**：`发票` 在第 2 批已裁定 `invoice`，第 3 批两条我又写了 `receipt`；
  `来半斤肉`/`来头蒜` 补 `please` 与「来两斤」对齐；`这个多少钱一条` 对齐 `per piece`。
- **多音字导致语义错**：`车停地下了` 我译 `downstairs`，实际是地下车库；
  `我借个充电宝` 译成 `i need`，原文是"借"；`加九十二` 是 92 号汽油不是"九十二块"。
- **主语缺失到改变句类**：`我在线` `我在家等` `我挂个专家号` `我换个锁芯` 等 6 条补回主语，
  否则英文读起来像祈使或状态描写。

**装机抽样暴露的门槛（比上面这些都重要）**：

| 输入 | 目标整句 | 结果 |
|:---|:---|:---|
| `womenhuijiaba` | 我们回家吧（第 1 批） | ✓ 第 2 格 `let us go home`；仍排在 我们回家啊 后面 |
| `shoujimeidianle` | 手机没电了（第 2 批） | ✓ 第 1 格 `the phone is dead` |
| `mashanghuilai` | 马上回来（第 3 批） | ✓ 第 1 格 `back soon` |
| `jidesuomen` | 记得锁门（第 3 批） | ✓ 第 1 格 `remember to lock up` |
| `wobadetuole` | 我把地拖了（第 3 批） | ✗ 首页是 我吧的脱了 / 我吧德国了 |
| `xueyapiangao` | 血压偏高（第 3 批） | ✗ 首页是 血压偏好 / 学鸦片高 |
| `xiyijizaixi` | 洗衣机在洗（第 3 批） | ✗ 首页是 洗衣机再次 / 在此 / 仔细 |
| `nihao` | 对照组 | ✓ 与 A1 包逐格一致 |

⇒ **第 3 批可达 2/5，前两批 2/2。** 「整句能被 `dict.tsv` 100% 最长正向匹配切分」
只是**必要条件**：引擎给不给这个整句候选、排不排进首页，由词图与打分决定。
我们没接语言模型（缺省 `NoLanguageModel`），整句名次由一元词频 + 个人 n-gram 定，
**末字是同音多写的高频字时最容易被顶掉**（了/高/洗 都有更高频的同音字）。
所以 A3 落表的条目里有一部分是**永远显示不出来的死行**——无害（不影响别的条目、
不报错、就是查不到），但也没有收益。

**下一轮该做的不是继续扩量，而是先量可达率**：要一个能列出「这一页候选的中文与有没有
译词」的观测口。两条路：① 临时探针在 `requery()` 里把候选文本 + `translation.is_some()`
打进 logcat（改代码，用完还原，`grep -c PROBE` 复验）；② 走已实现的 `allCandidatesRaw`
把整表文本打日志（同样要改代码）。**没有观测口之前，任何"扩到 N 条"的收益都是估的。**

**另一个自己踩的坑（记下来防再犯）**：清单里那列「要敲的拼音」是按**每个词头词频最高的
读音**拼的，不是句内实际读音 ⇒ 多音字（地 de/di、把 ba/bo、好 hao/hào）会拼错，
我因此先拿一个错拼音去真机验证、又手抄错过一次，白跑两轮。**验证用的拼音要从清单文件里
读，不要手抄；多音字句要逐个读音试或改用无歧义的句子。**

### 可达率测量（2026-10-13 18:07，观测口 = ① 临时探针）

**选型核实：② 先看后弃。**
- ② 的应答**结构上确实带译词**：`engine/src/view.rs:222-228` 的 `AllCandidates.candidates` 走
  `all_candidates` → `candidate_row`（`engine/src/view.rs:290-310`），第 308 行就是
  `translation: translation_to_string(&candidate.translation)`；`engine/src/view.rs:73-74` 是
  `skip_serializing_if = "Option::is_none"`。
- 但**没有任何现成出口能读到那份 JSON**：自动调用点 `scheduleCandidateGlosses()`
  （`MSIMEInputService.java:2568-2569`）第一道闸就是 `!candidateEnglishGloss` 直接 return，
  而它取自 `preferences != null && optBoolean("candidate_english_gloss", true)`（`:1972-1973`）；
  另一条是展开面板 `:6334`，要人手点箭头。两条路径**都不打日志**——宿主侧 `Log` 命中 0，
  Rust 侧日志点只有 `assemble.rs:98/120`、`session.rs:122/285`、`wire.rs:63`。
- ⇒ 走 ①，但探针不放在 `requery()` 而放在 `view::build_view()`（`engine/src/view.rs:177`）：它是每次按键的
  唯一漏斗（`lib.rs:163/184/241/271` 四个调用点都经过它），而 `annotate` 已在 `session.rs:120`
  把译词灌进候选、`:158` 存回 `state.candidates`，所以一处就能看到**全表 + 页游标**，
  三档数据一次拿全。
- 探针形状：`PROBE gen= edit= page=/ sz= n= p=k/m` + `位次:文本{G|-}`，每 12 条一行
  （避开 logd 单条截断），`p=k/m` 让缺段可被发现；解析端**断言「分段齐 + 行数 == n」才采信**。

**结果（固定 23 条，覆盖 2/3/4/5 段与虚词的、全部来自已落表的 1,000 条语料）**

| 档 | 命中 | 百分比 |
|:---|:---:|:---:|
| 首页前 3 格且带译词 | 18/23 | **78.3%** |
| 首页全部（页宽 10）且带译词 | 18/23 | 78.3% |
| 展开面板（全表 32–503 行）且带译词 | 18/23 | 78.3% |

三档同值不是巧合：**命中的 18 条位次只有 0 或 1**（`我们吃饭吧`、`我们回家吧` 在第 2 格，
其余全在第 1 格），没有一条落在 2 ≤ idx < 10，也没有一条被挤出首页。⇒ 可达与否基本是
**二值**的：整句要么在表首，要么整表都没有。中间档（首页内但不在前 3）在本样本里是空集。

失败 5 条：`先这样吧` `完成了` `我把地拖了` `血压偏高` `洗衣机在洗`——**全部是「整句不在候选表」，
「候选出现但无译词」0 条**。这 5 条的表大小 87–283 行，远没到观察到的 ~500 上限
⇒ **不是被截断，是词图与打分根本没产出这条路径**。加译词条目救不了这一类。

**目视对照（证明探针 ≡ 屏幕渲染）**：`今天很热` 屏幕 1–5 格 =
今天很热(第二行 `it is hot today`) / 今天真的 / 今天很的 / 今天好 / 金田宏(`n. Kaneda Hiroshi`)，
与日志 `0:今天很热G 1:今天真的- 2:今天很的- 3:今天好- 4:金田宏G` 逐格一致；`完成了` 屏幕 =
玩唱了 / 晚场(`n. evening show`) / 完成(`v. complete`) / 万春 / 晚春，与日志
`0:玩唱了- 1:晚场G 2:完成G` 一致。`G` 就是屏幕上那行第二排小字。证据在
`D:\toolchains\tmp\qingshan-snap\reach-logs\`（23 份原始日志）与 `reach.vis-*.png`（两张候选条截图）。

**自审：23 条里有 3 条拼音是我自己敲错的。** 用 `pincheck.py` 把「`segcheck.py` 逐字选定并验过能切进
`dict.tsv` 的读音」与「`run.sh` 实际敲出去的串」机器比对才抓出来：
`xiexieniudebangzhu`（`你的` 多打了个 u）、`nishenmeshouyokong`（漏 `hi`）、
`wanmeichangle`（把「完成」敲成「完美唱」）。前两条**用错拼音照样命中**（引擎容错），
第三条是**假失败**，已用正确串复跑并覆盖其日志。
⇒ 上一轮我写的教训是「拼音要从清单读、不要手抄」，这轮照样抄错，说明**光靠提醒自己没用，
防线必须是机器比对**：期望值与实际值各存一份、跑批前 `assert` 相等。

**一个新事实：引擎会给「纠错候选」。** `血压偏高` 我敲的是 `xueyapiangao`，表里第 1 格却是
`血压偏好`——`偏好` 在 `dict.tsv` 只有 `pian hao` 一个读音，`偏高` 根本不是词条；能把 `gao`
给成 `好` 只能是编辑距离类纠错（`crates/qingjian-core/src/correction/` 确有
`costs.rs` `edit.rs` `typo/`）。**代价表本轮没读，机制算未核实**；但它解释了「末字/关键字同音」
的句子为什么会被顶掉，也说明可达率的天花板由引擎决定，不由译词表决定。

**与上一轮「2/5」的关系**：上轮是「截图看候选条前 5 格」的 5 条第 3 批抽样，本轮是「全表日志」的
23 条混合样本，**不是同一个测量**，不能直接比大小。本轮完整复现了上轮那 3 条失败（同样整表没有），
并新增 2 条失败。

### A3 第 4 批：预检异常，按停手规则**没有落表**（2026-10-13 22:10）

**做了什么**：起草 573 条新整句（出行/吃饭/购物快递/工作电脑/身体医院/天气时间/情绪客套/
家庭孩子/旅行酒店九档）→ 派生脚本判定 522 条可切 → 自审补丁改 61 条译文、删 2 条不自然说法
（`随便吃点是` `大了太大`）→ **520 条待落**。随包表与 A3 源件本轮**逐字节未变**
（`cmp` 对过 `glossary-en.tsv.before-refreshHost` 与 `glossary-sentences.before-upgrade-test`）。

**拼音一律不手敲**：`derive.py` 用 `dict.tsv` 做 DP（段数最少优先、同段数取词频最大），
把「该敲什么」反推出来；`patch.py` 对 61 处自审改动逐条 `assert` 原文匹配，不匹配就退出。
上一轮那 3 处手敲错就是这么防住的——本轮 0 处拼音错（只有 2 条我自己写进草稿的
`AA 吧`、`我换个会Rooms` 混了拉丁字母，被 DP 判切不通，正好当反向样例）。

**预检结果（落表前，系统抽样）**：步长 26 取 20 条，逐条 `input keyevent` 后截候选条。
**前 5 格命中 11/20 = 55%**，且命中的全在第 1-2 格 ⇒ 前 3 格也是 55%。
9 条未命中：`我下楼取餐` `汤是凉的` `蜡烛插上` `商家不回复` `地址填错了` `我看得见`
`我挂个号` `我补个觉` `地还没拖`。
其中 2 条我换了自然读音复测（`地` di、`觉` jiao，派生脚本给的是 `de`/`jue`），
**仍未命中** ⇒ 不是读音造成的，是同音高频组合压过目标：躺尸/汤是、上架/商家、
迪拜/地还、我不/我补、的得/我看的见、差上/插上、天/填。

**口径限制（必须写清）**：本轮约束是「不改代码」，所以**没有观测口**，只能测
「整句有没有出现在候选条可见的前 5 格」，**测不到「整句在不在引擎候选表里」**
（表有 270-410 条，面板要滚到底才看得完）。上一轮的 78.3% 是「前 3 格且带译词」，
样本是**已落表的老语料**里我挑的 23 条——两个数不是同一个测量，不能直接说跌了 23 个点。

**能确定的归因**：命中的 11 条单字段占比均值 **0.288**，未命中的 9 条 **0.75**
（按 DP 切分段统计）。也就是说**这一批的句式风格本身就更不可达**：口语短句里大量
单字（取/餐、插/上、挂/个/号、补/个、填、还/没/拖），而同音的高频多字词会把它们压掉。
全批 520 条的单字占比均值 0.424；若按「单字段占比 < 1/3」过滤，只剩 **242 条（47%）**。

**决定**：按用户给的停手规则（「不在候选表的比例明显上升 > 30% 就停下汇报，不要盲扩」）
**本轮不落表**，把数据与三个选项交回用户：
① 按单字占比过滤后落 ~242 条（预计可达率回到上轮量级，代价是覆盖少一半）；
② 全 520 条照落（明知其中相当一部分要翻页或永远不露，但无害）；
③ 先加回观测口（一次代码改动 + 用完还原）把「在不在表里」测准，再决定 ①/②。
证据文件：`qingshan-snap/a3b4/`（520 条派生表、20 条抽样表、4 张候选条拼图、3 个脚本）。

### 观测口复测：四档真实可达率（2026-10-13 22:58，探针已还原）

**为什么测**：上一轮只能看候选条前 5 格，测不到「整句在不在引擎候选表里」，所以 55% 那个数
不能直接触发停手规则。用户选 ③→①：先把观测口加回来测准，再决定落多少。

**探针**：位置与形状与上一轮**完全相同**（`engine/src/view.rs:177 build_view()`、每 12 条一行、
`p=k/m` 分段自校验），这样两轮的数才可比。装机后 `grep -a -c PROBE` 在两个 `.so` 里各 1 命中
（证明确实是探针版在跑），测完 `cmp` 还原、`.so` 回到 4,349,376 / 4,595,232 B、
APK 回到 **49,835,753 B**（与本轮改前同值）。另外把 logcat main 环放大到 2 MiB
（`logcat -G 2M`，实测生效）——表长最大 503 条 = 42 段，256 KiB 环有被自己冲掉的风险。

**样本 33 条**：上一轮那 20 条（同一批、同一拼音，保证可比）+ 新抽 10 条（步长系统抽）
+ 3 条对照（`你好` / `手机没电了` / `洗衣机在洗`，用来验探针复现旧数据）。
**33/33 全部帧完整**（分段齐、行数 == n、尾帧 `edit=` 等于所敲串），0 条作废。

| 档 | 命中 | 说明 |
|:---|:---:|:---|
| ① 首页前 3 格且带译词 | 2/33 | **只有 2 条对照**——新批 30 条**还没落表**，结构上不可能有译词 |
| ② 首页全部且带译词 | 2/33 | 同上 |
| ③ 展开面板(全表)且带译词 | 2/33 | 同上 |
| ④ **整句在候选表里**（本轮新增） | **18/33** | 新批 **16/30 = 53.3%**；对照 2/3（`洗衣机在洗` 仍不在表，与上一轮一致） |

⇒ **①②③ 那三个 6.1% 不是回退**，是"没落表就没译词"的必然结果；对照 3 条把探针与上一轮
的数据接上了（`你好` 位次 0 带译词 ✓、`手机没电了` 位次 0 带译词 ✓、`洗衣机在洗` 不在表 ✓）。

**本轮最值钱的一条结构性发现**：命中的 16 条位次**全部是 0 或 1**，没有一条是「在表里但埋得深」。
也就是说这台引擎的行为是二值的——**要么把整句顶到最前，要么根本不产出**。
所以上一轮「前 5 格可见 55%」与本轮「在表里 55%（同样那 20 条）」是同一个数，
**前 5 格代理指标被证明是准的**，以后不必每批都开观测口。

**过滤器有效性（按 DP 切分的单字段占比分桶，只算新批 30 条）**：

| 单字段占比 | 样本 | 在表 | 率 |
|:---|:---:|:---:|:---:|
| ≤ 1/3 | 17 | 14 | **82%** |
| 1/2 ~ 2/3 | 7 | 2 | 29% |
| ≥ 3/4 | 6 | 0 | **0%** |

全批 520 条按同一规则分桶：A(≤1/3) **242** / B **206** / C **72**。外推：
**全落 520 条 ≈ 258 条会露出**；**只落 A 桶 242 条 ≈ 198 条露出**。

**决策算术（交用户拍板，本轮没落表）**：全落比只落 A 桶**多约 60 条可见整句**，
代价是多约 262 条永不露出的死行（无害：不影响别的条目、不报错）与约 4.7 KB。
我的建议是 **② 全落 520**——目标既然是让用户看到更多整句，密度不是稀缺资源；
但若你要控制表规模/让「表里每条都有意义」，就走 ①（242 条，命中密度 82%）。

**24 条疑问项裁定已应用**：`ruling.py` 逐条 `assert` 现值匹配（16 改、8 留，全部对上），
改完做冲突检查：**与随包表 89,308 条同译文的 0 条、批内新译文重复 0 条**。
裁定后的 520 条在 `qingshan-snap/a3b4/derived.ruled.tsv`，落表时用它，不再重挑。

**二值行为已由用户确认并定成规矩（2026-10-13 23:37 落表后复验）**：见下一节
「A3 第 4 批落表与『引擎二值行为』确认」——以后每批 A3 用「前 5 格」代理指标，
不必每批开观测口；某批出现位次 ≥ 2 即推翻，停下重评。

### A3 第 4 批落表与「引擎二值行为」确认（2026-10-13 23:37）

**用户裁定**：选 **② 全落 520 条**，不选 ① 过滤。理由（记在这里，下次别再问）：目标是让用户
看到更多整句，密度不是稀缺资源；② 比 ① 多约 60 条可见整句，代价是约 262 条死行 + 约 4.7 KB，
而死行已证无害（不影响别的条目、不报错、就是查不到）。

**落表实测**：源件 `glossary-sentences-zh-en.tsv` 959 → **1,479 行**（追加前断言：批内 0 重复、
与源件 0 冲突，脚本 `assert` 不过就退出）；表 89,308 → **89,828 行**（重复词头 0）；
文件 2,150,622 → **2,167,528 B**；锁由 `build_glossary_subset.py` 同一次运行重写
（`dict.tsv 2,053,612 / glossary-en.tsv 2,167,528`）；`.so` **已重编**，实测包内烘的是新锁
（`grep -a -c 2167528` = 1、`2150622` = 0）；APK clean **49,843,905 B**（+8,152 B，
与包内 deflate +8,150 B 同量级）；`cargo test --release` 39 passed、`fmt --check` 17 hunk、
`verify_symbols.py` 两 ABI 102/缺失 0/多余 0、audit 213 条 LINE-OOB 0。

**install -r 不 pm clear（这是 refreshHostRaw 落地后第一次真正的升级重解包）**：
解包件 2,150,622 → **2,167,528 B**（23:37），日志恰好**一次** `dictionary_outdated`
（`[msime_android::wire] 回 ok:false: dictionary_outdated: glossary-en.tsv:length-2150622-want-2167528`
+ `MSIMEBootstrap: Runtime options refresh failed: dictionary_outdated`，wire 层 `ok:false` 计数 = 1
⇒ 重解包后的第二次刷成功），`译词表载入 73 ms / 2167528 B`，`:ime` pid 1354、crash `FATAL` 0。
**注意口径**：`grep -c dictionary_outdated` 会给 **2**，因为一次事件有两行日志（我们的 + 宿主的）；
判"几次事件"要看 wire 层 `ok:false` 的行数。

**抽样 8 条新句（截图口径）7 条露出，位次全是 0 或 1**：`我打车走`②、`出口在右边`①、
`我早饭没吃`①、`运费到付`②、`刷卡失败`①、`投影仪没信号`①、`我要验血`②；
未露出的 `我家里有事` 与观测口「不在表」的判定一致。对照 `nihao` 首页与 A1 包逐格一致，
`womenhuijiaba` 第 1 格仍是 `我们回家啊`、第 2 格才是 `我们回家吧` ⇒ **A3 不改排序**。

**⇒ 引擎二值行为确认（用户 2026-10-13 拍板写进规矩）**：整句要么排在位次 0/1，要么根本不产出。
所以**以后每批 A3 扩量用「候选条前 5 格」这个代理指标就够，不必每批开观测口**；
**若某批抽样出现位次 ≥ 2（第 3 格及以后），本结论被推翻，停下重新评估**。

### A3 第 5 批：起草 816 → 选 500 → 预检**触发停手判据，未落表**（2026-10-14 13:25）

**做了什么**：起草 → `keep.tsv` 533 条（第一批，机械校验过）+ `batchB-clean.tsv` 286 条
= `draft-all.tsv` 819 条 → `derive5.py` 反推拼音并算单字段占比 → 去重/切不通后
**`derived-all.tsv` 816 条可用** → 按单字段占比从低到高**取 500 条**（`set.tsv` /
`set-with-share.tsv`），剩 **316 条留作备用**（`reserve.tsv`）。随包表、A3 源件、`.so`、锁
**本轮逐字节未变**（没跑 `--append`，没重编）。

**起草这一半不体面，照实记**：第一轮草稿 `draftA.tsv` 写崩了（2,105 行里 597 行字段数不对、
406 行是同一个「我把这份…」模板），截断到前 533 行才用；第二轮（batchB）才回到正常密度。
教训：一次别超过 ~300 条，且落笔前先 `awk -F'\t' 'NF!=2'` 数一遍。

**预检（步长 25 抽 20 条，`input text` 注拼音 → `uiautomator dump --windows` 截候选条）**：
前 5 格逐字命中 **10/20**，未露出 10，**位次 ≥ 2 出现 1 条**（`话筒没有电` @ 第 3 格，
格子 = 话筒没有点/话筒没有店/**话筒没有电**/话筒/话）⇒ **停手判据触发，按规矩没往下落表**。
`FATAL` 0。这一条 13:23 单独复测**完全复现**（同格序、同位次）⇒ 不是抖动。

**没落表不是因为可达率低，是因为"二值行为"这条前提被推翻**：上轮立的规矩是
「整句要么 0/1 要么根本不产出」，这次实测到位次 2 ⇒ **要么规矩错，要么代理口径错**，
必须先弄清楚再扩量，所以停下。

**派生拼音的多音字审计（本轮新增，防我自己手敲出错）**：`poly.py` 用同一套 DP 重算 816 行，
**逐行断言与 `set.tsv`/`reserve.tsv` 里的拼音列一致（mismatch=0 / 816）**，再列出所有
「裸单音节多音段」涉及的 44 个字。逐字判定后，**9 行选错**（DP 的规则是"同段数取词频最大"，
而词频大的那个读音在这里恰好是错的）：

| 字 | 词典里两个读音的词频 | 选了什么 | 该选 | 行数 |
| --- | --- | --- | --- | --- |
| 长 | chang 81,856 / zhang 130,456 | zhang | chang | 1（`安检队伍非常长`） |
| 调 | diao 11,247 / tiao 6,143 | diao | tiao | 3（`我把音量调大` 等） |
| 还 | hai 997,169 / huan 17,426 | hai | huan | 3（`还车时检查外观` 等） |
| 重 | zhong 34,690 / chong 6,260 | zhong | chong | 1（`重填一张要时间`） |
| 弹 | dan 10,023 / tan 1,601 | dan | tan | 1（`弹窗太多了`） |

另有**词典自带的错读音词条**（不是 DP 的错，是 `dict.tsv` 教的）：
`长时间 = zhang shi jian`（f=7,429）、`太长 = tai zhang`（3,729）、`很长 = hen zhang`（5,128）
——这三条影响的是**真机用户**：打 `changshijian`/`taichang` 出不来这些词，不是我们本轮的事，
但它决定了下面这 2 条整句能不能落。

`fixpoly.py` 把 11 行（set 3 / reserve 8）的拼音列改成正确读音，**每个替换读音都 assert 存在于
`dict.tsv` 该字的读音集合里**（我打错字脚本就退出），改完 `diff` 只对得上这 11 行、行数与其余
列不变（`before-fixpoly/` 存了改前四份）。

**修正后这 3 条 set 内的句子复测，结论更硬**：`安检队伍非常长`/`这里不能长时间停`/`正文别太长`
按正确拼音（`…feichangchang` / `…changshijian` / `…taichang`）注进去，前 5 格分别是
`案件队伍非常长`/`这里不能尝试舰艇`/`正文别提倡` ⇒ **三条都不可达**（首格被同音高频词占死，
或整词不在词典）。按词典教的 `zhang` 它们能出，但那是错读音 ⇒ **这 3 条要删或改写，不能落表**。

**上一轮那条挑选规则降级**：本轮 20 条样本里，命中 10 条的单字段占比均值 **0.158**、
未露出 10 条 **0.213**，分离度很弱；且 5 条命中的与 3 条未露出的都是 0.000（全多字段切分）。
⇒ 上轮那个「已在表的 82% 是 ≤1/3，全批只有 53%」**只能说明相关性存在，不能当预测器用**。
500 条里 ≤1/3 的只有 407 条（199 条 ≤0.2 + 208 条 0.2–1/3），93 条 >1/3。

**交回用户的三个选项**（本轮按停手规则只做数据，不做决定）：
① **A3 冻结**，等"位次≥2 到底是引擎行为还是代理口径"弄清楚再动；
② 只落**复测位次 0/1 的子集**——要先跑一轮更大抽样（20 条 ≈ 3 分钟，100 条 ≈ 15 分钟）；
③ **再开一次观测口**（一次代码改动 + 用完还原）把"在不在候选表里"和"排第几"分开测。

脚本与数据归档在 `/d/toolchains/tmp/qingshan-snap/a3b5/`：**44 个文件**（`derive5.py`
`poly.py` `poly2.py` `fixpoly.py` `dictlook.py` `review5/6.py` `strip.py` `bigfield.py` `pick.py`
`precheck5–9.sh` `precheck8/9.log` 与全部 tsv），外加 `before-fixpoly/` 里改拼音之前的四份
（`set.tsv` `set-with-share.tsv` `reserve.tsv` `derived-all.tsv`）与本轮的两份文档快照。
`md5sum` 抽对过 `set.tsv` 归档件与工作件一致。

### C1 词典自带错读音：根因、规模、方案（2026-10-14 13:47–14:41，**只评估，未动任何数据**）

**根因已定位到具体那几行代码**（每条都本轮 `sed -n` 逐行回读过；先派了个子代理查，
它报的数字我全部复测过才写下来）：

- 随包 `host/assets/dictionary/dict.tsv` 与 `qingjian/assets/lexicon/dict.tsv`
  **`cmp` 逐字节相同**（md5 `425050c1b26f4395b3526ce3afc091a2`，2,053,612 B）⇒ 不是我们裁过的副本。
- **上游数据里没有这三个词**：`02_common/modern_chinese_common_words.tsv` grep `长时间|太长|很长` **0 命中**；
  该表里 `长` 有两行（`chang2` f=1368、`zhang3` f=1533，`:1370`/`:1535`）——**zhang 本来就比 chang 频高**。
- 错读音出现在**短语层**：`assets/lexicon/phrases.tsv:1167` `长时间 7429 zhang shi jian`、
  `:1831` `很长 5128 hen zhang`、`:2702` `太长 3729 tai zhang`，且 `dict.tsv:86482/35374/25840` 与之一致、
  **没有 chang 版**。`phrases.tsv` 的表头自己写着「读音由成分词拼出」。
- 拼读音的规则在 `tools/dict-convert/src/phrases/mod.rs`：文档注释 `:231`
  「词 → 读音（同一个词多个读音**取词频最高的那条**）」，实现 `:247-253`
  （`Some(entry) if entry.0 >= frequency => {}`），调用点 `:104-114`（逐个成分 `readings.get(*p)` 取音、
  `syllables.join(" ")`）。单字那侧的频次来自 `tools/dict-convert/src/lexicon/readings.rs:112-138
  weighted()`（kHanyuPinlu 分摊）。
  ⇒ **判定：不是教育部/上游词表转录错，是青简短语生成器的 tie-break 选错了成分读音。**
- 对照正例（同一张表里由 02_common 直给读音的词）：`银行 yin hang`、`音乐 yin yue`、`重新 chong xin`、
  `重复 chong fu`、`睡觉 shui jiao`、`着急 zhao ji` **全部正确** ⇒ 错误面只在"短语层拼出来的词"。

**规模（`c1scale.py` 实测，脚本已归档）**：dict.tsv 数据行 93,236 / 词头 92,330 /
一个词头有多行不同读音的 **871** 个 / 单字多读音的字 **601** 个 / 短语层词头 5,398 个。
「短语层 + 只有一行 + 成分字另有读音且用了高频那个」的**机械风险集 = 2,476 行**
（词频 ≥5,000 的 769 行）。**但风险集是上界，绝大部分是对的**：按字分布的前几名
`了 521`、`的 457`、`么 203`、`这 199`、`说 166`、`没 158`、`还 126`、`会 122`、`那 102`、`都 98`
逐字判定后**取的都是正确的白读**（`真的 zhen de`、`都是 dou shi`、`还在 hai zai`…）。
逐条判完可疑字后，**确证真错只有 10 行**，除 `睡不着` 外全是「长」该读 chang 而表里只有 zhang：

| 词频 | 词头 | 表里 | 应为 |
|---|---|---|---|
| 7,429 | 长时间 | `zhang shi jian` | `chang shi jian` |
| 5,128 | 很长 | `hen zhang` | `hen chang` |
| 5,120 | 好长 | `hao zhang` | `hao chang` |
| 4,232 | 不长 | `bu zhang` | `bu chang` |
| 4,074 | 这么长 | `zhe me zhang` | `zhe me chang` |
| 3,729 | 太长 | `tai zhang` | `tai chang` |
| 3,234 | 头发长 | `tou fa zhang` | `tou fa chang` |
| 3,020 | 多长 | `duo zhang` | `duo chang` |
| 2,498 | 那么长 | `na me zhang` | `na me chang` |
| 343 | 睡不着 | `shui bu zhe` | `shui bu zhao` |

`长的 22,404`、`不着 33,675` 属"两个读音都可能出现"的歧义类，按水杉的规矩应**双行**而非改音。
⇒ **C3 的答案：真错 ≪ 50 条（10–12 条），不是 277 条**（277 是"含多音字的多字段"上界，不是错）。
`c1-risk.txt`（2,524 行 + 按字分布）已归档，可复算。

**第二方判据（顺手挖到的既有惯例，很重要）**：`msime-develop/resources/dictionary-sources/` 里有
`pinyin-reading-corrections.txt`（**218 条**有效行，`词<TAB>错拼音<TAB>正确拼音`）与
`pinyin-overrides.txt`（60 条短语读音）。它的表头写明做法：**构建期**跑一个 reading-corrections 阶段，
删掉错读音行，且「一条修正若没删掉任何东西就失败」「修正后若该词没有正确读音行也失败」，
政策是「只收词义可证的读音；一词两读的（重载、调配）两行都留；查不全的名字不动」。
拿它对我们的 dict.tsv：**217/218 词头我们根本没有，0 条带着它的错读音** ⇒ 不能当规模判据，
但**它是同项目家族里既有的处理惯例**，比我自己发明的任何做法都有说服力。

**四个方案与代价**（本轮一条都没执行）：

| 方案 | 做法 | 代价 | 我的评估 |
|---|---|---|---|
| A 手工改 dict.tsv | 直接改这 10 行 + 把 277 个多字段逐个判完 | 随包词库与青简上游**逐字节分叉**（现在 `cmp` 相同这条事实会失效）；锁重生成 + `.so` 重编；277 条判定是纯人工工时 | 可行，但把分叉做在数据文件里，将来同步上游要人肉并 |
| B 改生成器 tie-break | 让 `phrases/mod.rs` 在成分字多读音时输出多行 | **要改青简 Core 工具**，越出「Core 只读、不反馈上游」的边界 | 排除（用户已两次说不碰青简） |
| C engine 侧读音别名 | 在 `assemble.rs::create_engine` 里 `engine.set_extra_dictionaries(vec![修正表])`，修正表烘进 `.so` | 只改我们自己的适配层（约 30 行 + 1 条单测）；**dict.tsv 保持与上游逐字节相同**；锁不用动；`.so` 要重编；挂点可达性已读码确认（`engine/setup.rs:7 impl Engine`、`:495 set_extra_dictionaries`、`:505 all_dictionaries` 把附加词库并进查词面），但**性能与"错行仍在"的行为未实测** | **推荐**：加行不删行，用户打 `chang` 出得来、打 `zhang` 照旧出得来，不动上游数据 |
| D 只修高频 3 条 | 只处理 长时间/太长/很长，其余排后 | 与 A/C 同构，只是范围小；A3 侧能立刻让 B 里那两条 `长` 句子可达 | 可作为 C 的第一批内容（10 条一起做，成本一样） |

**推荐 = C（范围按 D 一次做满 10 条）**，理由：不动上游数据、不改 Core、
分叉面变成"我们适配层里的一张可审计清单"，与水杉构建期清单同构，且加行不删行不破坏任何现有通路。
等用户拍板；拍板前 A3 里那两条 `长` 句子（`这里不能长时间停` `正文别太长`）本轮已按实测**作弃处理**，不靠 C 也能收口。

### C1 方案 C 落地与真机复测（2026-10-14 16:00–16:58，用户拍板 C，范围做满 12 条）

**用户拍板的理由（记在案）**：不动上游 `dict.tsv`（`cmp 相同` 这条事实要保住）、不破「青简只读」边界、
只改适配层约 30 行 + 单测、与上游水杉的构建期修正清单同族。

**先说一处我自己的错**：动手前我以为「敲 `changshijian` 打不出长时间」，因为上一轮真机里
`这里不能长时间停` 整句不可达。**第一条按 TDD 写的用例一跑就绿** ⇒ 前提错了。
补了一个临时诊断（跑完即删）才看清真实缺陷：

| 输入 | 挂修正表前 | 说明 |
|---|---|---|
| `changshijian` | 长时间 **第 0 格** | 走得通：单字 `长 chang`（dict.tsv 里本有 81,856 那条）+ `时间` 的合词路 |
| `shuibuzhao` | 睡不着 第 **1** 格（首格「说不着」） | 走得通，同上 |
| `taichang` | **整表 380 格里没有「太长」**，首格「提倡」 | 真缺陷 |
| `henchang` | **整表 442 格里没有「很长」**，首格「合唱」 | 真缺陷（`hen chang` 本就是「合唱」的读音，压得更死） |

⇒ 驱动用例改成 `taichang`/`henchang` 两条（它们当场红，报错就是"380 格里没有 太长"），
`changshijian`/`zhangshijian` 那两条降级成**回归守护**（改前就绿，注释里写清了它们不是驱动用例）。

**实现**（`engine/src/assemble.rs` + `engine/src/reading-fixes.tsv`，共 12 条）：
表是**独立 TSV**、`include_str!` 烘进 `.so`，不是 Rust 常量——理由是它能 diff、能逐行溯源，
列格式与 `dict.tsv` 一致（`词头<TAB>音节<TAB>词频`），所以 `Dictionary::parse`
（`crates/qingjian-dictionary/src/dictionary/mod.rs:88`）直接吃得，注释行由 `:94` 跳过。
接线一句：`engine.set_extra_dictionaries(vec![fixes])`（挂点 `engine/setup.rs:495`，
查词并集 `:505 all_dictionaries`，分母 `:516 total_frequency` 是**各词库之和**）。
**只加行不删行**，所以词典里原有的 `zhang shi jian` 那条路照旧通（守护用例 `the_wrong_reading_row_still_works`）。
解析失败走 `?` 而不是像 Glossary 那样降级：表是编译期常量，坏了就是有人改错，属于"就该红"，
与水杉清单"对不上就失败"同政策。

**词频沿用 `dict.tsv` 同一词头那条错读音行的频次**（不新造数字）。这条由
`reading_fixes_match_the_shipped_dictionary` 四段断言把住：能 parse、行数 12、
每个音节 ∈ 该字在 `dict.tsv` 的单字读音集、词频 == `dict.tsv` 里那个词头的频次。
**两次现场证伪（证明这防线有牙）**：把 `头发长` 的 `chang` 改成 `chajg` ⇒ 红；
改回读音、把词频 3234 改成 3240 ⇒ 也红（报 `修正表用的词频 3240 与 dict.tsv 里的 [3234] 不一致`）。
再证一次接线本身有牙：临时注释掉 `set_extra_dictionaries` 那一句 ⇒ `typing_taichang_reaches_tai_chang`
当场回红（380 格里没有 太长），然后恢复。

**规模与代价（设备日志实测，不是我算的）**：
`读音修正 12 条 / 词频合计 94,886，主词库词频 275814283（修正表占 0.0344%）`
——我先前在注释里写"106,541 / 0.039%"是**没加就写**（真值 94,886 / 0.0344%），已改正。
分母只涨 0.034% 且是**对所有候选同减**的平移 ⇒ 不动相对次序；这一条写成了实测守护
`the_extra_dictionary_does_not_reshuffle_unrelated_inputs`（`shuijiao`/`zhaojia`/`nihao`/`women`/`bushicongming`
五串，挂与不挂首页前 10 格逐格相同；Engine 不是 Clone，所以每串新建两台引擎）。

**体积**：`.so` arm64 4,355,360（+3,568 B）/ x86_64 4,602,032（+4,032 B），
`clean assembleDebug` APK **49,850,705 B（+4,032 B）**；两次分别构建的字节数一致（可重复）。
随包 `dict.tsv` 复测 **`cmp` 仍与青简 `assets/lexicon/dict.tsv` 逐字节相同**（md5 `425050c1…`）
⇒ 方案 C 的核心卖点守住了。表/锁/A3 源件 mtime 全是 10-13 23:33/23:34，本轮未动。

**真机复测 8 条**（`e1cdb946`，装机 16:32:30，`uiautomator dump --windows` 读首页；竖屏已核 1080×2400）：

| 输入 | 目标 | 位次 | 首页前几格 |
|---|---|---|---|
| changshijian | 长时间 | **0** | 长时间/尝试/常识/常侍/常事 |
| taichang | 太长 | **0**（改前整表不出） | 太长/太差/太丑/台城/台秤 |
| henchang | 很长 | **0**（改前整表不出） | 很长/很丑/很/和你/河南 |
| shuibuzhao | 睡不着 | **0** | 睡不着/水埗/睡/谁/水 |
| zhangshijian | 长时间 | 0（旧路仍在） | 长时间/张氏/章事/仗势/涨势 |
| taizhang | 太长 | 0（旧路仍在） | 太长/台中/太重/台州/台柱 |
| shuijiao | 睡觉 | 0（正对照） | 睡觉/水饺/睡觉了/睡觉的 |
| zhaojia | 着急 | **3**（正对照） | 招架/召见/招架不住/着急/召集 |

`位次=-1` 0 条，`FATAL` 0，pid 10858。`着急` 那条**没有改前基线可比**，只能证"没消失"，
不能证"位次没动"（它不在修正表里，且分母平移不动次序）——按推断记。
`长的`/`不着` 两条双行按上游政策加了，但**没在真机单测**（它们本来就在某条读音下可达）。

**中途一次假警报（值得记）**：冒烟时我用了一段"MOVE_END + 12 次 ⌫"的清空序列，之后
`nihao` + 空格只留下字母 `nihao`，看着像方案 C 把空格上屏打坏了。对照实验：
**临时把 `set_extra_dictionaries` 断掉、重编、重装**——空格表现**一模一样**（还是不提交），
⇒ 不是修正表的回归，是我那个清空序列的伪象（上一轮冒烟没有清空这一步）。
按上一轮的原序列复跑（16:57:47 装的这台）：点「你好好」→ 输入框 `你好好` ✓，
`women` + 空格 → `你好好我们` ✓，`FATAL` 0、PROBE 0。**设备已复位**：四张学习表
（`user.tsv` 53 B、`user-choices.tsv` 74 B、`user-ngram.tsv` 165 B、`user-typos.tsv` 57 B）
先归档到 `qingshan-snap/c1fix/` 再逐个显式文件名 `rm`（上一轮我用 `*.tsv` 通配符没删动，见第 34 条），
复位后 `total 0`，`accelerometer_rotation=1` 已恢复。

**测试与静态检查（16:55 实测）**：`cargo test` **51 passed / 0 failed**（45 → 51，本轮 +6 条：
`typing_taichang_reaches_tai_chang`、`typing_henchang_reaches_hen_chang`、
`typing_changshijian_reaches_chang_shi_jian`、`the_wrong_reading_row_still_works`、
`reading_fixes_match_the_shipped_dictionary`、`the_extra_dictionary_does_not_reshuffle_unrelated_inputs`），
0 warning；`cargo fmt --check` 仍 **17 个 hunk、全在 `lib.rs`**（新代码手工排成 rustfmt 的形状，
没让 `assemble.rs` 多出 hunk）；审计 290 条：OK 114 / AMBIG 101 / MISSING-PATH 63 / **LINE-OOB 0** / NOT-FOUND 12（17:06 稳定值；中途 288→289→290 的两次上涨都来自本轮新写的引用本身）。

**仍未验的**：① 修正表对"整句"路径的帮助（`这里不能长时间停` 仍不可达，本轮没重测整句）；
② `着急` 的位次没有改前基线；③ 分母平移不动次序只在 5 串上实测，未穷举；
④ 双行条目（`长的`/`不着`）未做真机单测。


### 观测口第 2 轮与 E4 推翻：四档 + A/B（探针 13:54–14:08，A/B 与裁定 14:15–14:34，探针已还原）

**探针**：位置与形状与 2026-10-13 那轮**逐字相同**（`engine/src/view.rs` 的 `build_view()`、每 12 条一行、
`PROBE gen= edit= page=/ sz= n= p=k/m` + `位次:文本{G|-}`，解析端断言"分段齐 + 行数 == n"）。
带探针的 `.so` = 4,355,392 / 4,602,480 B（+3,600 / +4,480 B），装机实测出帧正常，
**56 条样本 BAD 帧 = 0**（40 + 12 + 4，全部帧自校验通过）。
还原后：`cmp` 与 `b5probe/view.rs.before-probe` 逐字节相同、`grep -rn PROBE engine/src/` = 0、
两个 `.so` `grep -a -c PROBE` = 0 与 0、`.so` 回到 **4,351,792 / 4,598,000 B**、
APK `clean assembleDebug` 回到 **49,846,673 B（= 本轮改前值）**；干净版 `install -r` 后冒烟：
`nihao` 点第 2 格 → 输入框 `你好好` ✓，`women` + 空格 → 输入框 `你好好我们` ✓，
`FATAL` 0、logcat 里 PROBE 0 条。（注：候选条检测器在空格上屏那一步抓到的是工具行而不是候选条，
不影响点选/上屏两件事的判定，但说明 strip 检测器还要再收一格判据。）

**四档（本轮实测）**

| 样本 | n | ①首页前 3 格且带译词 | ②首页全部且带译词 | ③全表且带译词 | ④**整句在表里** | 位次分布 |
|---|---|---|---|---|---|---|
| 新批（未落表）40 条 | 40 | **0**（结构上不可能） | 0 | 0 | **27 = 67.5%** | 0:23 / 1:2 / **2:2** |
| 重点复测 4 条 | 4 | 0 | 0 | 0 | 1 = 25% | **2:1**（`话筒没有电`） |
| 已落表对照 12 条 | 12 | **7 = 58.3%** | 7 | 7 | 7 | 0:7（全在第 1 格） |

- ①②③ 三档同值、且对照 12 条位次全是 0 ⇒ **复现上一轮的二值行为**；58.3% 与上轮 78.3% 的差
  是样本不同（12 vs 23 条），**不是回退**。
- **位次 ≥ 2 本轮共 4 条**（`话筒没有电` 三次独立测量同位次、`车辆保养已经到期`、`我预约了明早年检`、
  `我在站台上看着车来`）⇒ 二值前提**确实被推翻**，但推翻的形状只是"多出一个第 3 格"：
  **35 条在表样本里没有一条埋在首页之外**（位次 ≥10 = 0）⇒ 「在表」与「首页前 3 格」仍近乎等价，
  前 5 格代理指标**够用**，只是别把"二值"当公理，改成「位次 ∈ {0,1,2} 或不在表」。
- 新批 40 条未露的 13 条全部是"整表根本没有这条路径"，**加译词条目救不了**（与上一轮同结论）。

**落表分档建议（按用户给的阈值，数据说话，等拍板）**：「在表」67.5% 落在 40%–70% 档 ⇒
按字面该走"只落位次 ≤1 的子集"；位次 ≥2 = 2/40 = 5%，正好卡在 5% 线上。
但**裁定后的批**（491 条）代理预检 **24/31 = 77%**、位次分布 0:20 / 1:3 / 2:1 ⇒
按更贴近最终形态的那个数，**我倾向全落 491 条**，代价是明知约 23% 永不露出（无害死行）。

### A3 第 5 批裁定执行结果（D/E/B 批量，2026-10-14 14:15–14:34，仍未落表）

**流程裁定入档**：D1「≤1/3」**降级为相关指标，不再当硬门槛**（命中 0.158 vs 未露 0.213，两侧都有 0.000）；
D2 `poly.py`（复算全表 + `assert mismatch==0` + 裸多音段清单）**定为每批落表前关卡**；
D3 模糊音/纠错代价表**下一轮读清**（与跨会话撤销同一轮）；D4 `reserve.tsv` 316 条**保留**；
D5 每批 ≤300 条 + 落笔前 `awk -F'\t' 'NF!=2'`；D6 预检抽样 20 → **40 条**（本轮已按 40 跑）。

**E2**（去 `i will`）：`set.tsv` 62 条 + `reserve.tsv` 136 条 = **198 处**改为无时态电报体，
改后断言"残留 0"。⚠ **一致性代价（实测）**：已落表的 1,500 条语料里 `i will` 开头有 **169 条**
（A3 源件与随包表两侧同数），只改新批会让语料风格分两代。要么下批连那 169 条一起改
（要重生成表 + 重编 `.so`），要么回滚 E2。**这一条等用户裁，我没动随包表。**

**E3**（≤2 词英文 17 条逐条判）：只 1 条值得补——`保险单已带上` 的 `policy brought`
（policy 歧义：政策/保单）改成 `insurance policy brought`；其余 16 条是「名词+过去分词」电报体，
与已落表语料里 **201 条**同形态（实测计数），**保留**。

**E4（≥9 字整句缩短）——被本轮自己的 A/B 推翻，已还原**：先按裁定把 12 条缩到 ≤8 字，
再对同一批 12 条做 A/B：原句 **8/12 进前 5 格**（`服务器需要重启维护` `短信验证码已经输入`
`下次保养是五千公里` `停车场入口在前面右转` `需求评审安排在周四` `我在无线网络下更新` 位次 0，
`前面事故占用两车道` 位次 1，`我在站台上看着车来` 位次 2），缩短版只有 **4/12**
⇒ 缩短把可达率腰斩，"长句不可达"这个前提不成立；12 条全部还原原句（英文仍去 `i will`）。
**教训（用户 2026-10-14 定）：改动前先做 A/B 对照，不要凭直觉缩句**——"看起来太长所以打不出"
是我脑子里的因果，实测方向正好相反（长句 8/12、短句 4/12）。
顺带纠正我自己一处口径错误：第一次配对用 `zip(改前, 改后)`，因批里删过 1 行导致整体错位，
**造出 242 条"假改写"**；改成按词头配对后实数是 24 条（记进环境陷阱第 33 条）。

**B 14 条逐条裁定**（标准：派生拼音错 ⇒ 改音；拼音对但同音高频压过 ⇒ 弃；句子不自然 ⇒ 弃）：

| 原句 | 实测首格/位次 | 处置 |
|---|---|---|
| 截止时间已经过了 | 截至时间…（-1） | **改写** `报名已经截止` @1 ✓ |
| 雨具已经带上 | 语句已经带上（-1） | **改写** `带上雨伞了` @0 ✓ |
| 司机在路口等我 | 自己在路口等我（-1，si→zi 模糊） | **改写** `师傅在路口等我` @0 ✓ |
| 正文别太长 | 正文别提倡（-1，`太长` 是词典错音） | **改写** `正文别太多` @0 ✓ |
| 窗户密封不好 | 窗户蜜蜂不好 | **弃**（改后 `窗缝漏风` 仍 -1） |
| 下午茶时间到了 | 下午差时间到了 | **弃**（`该喝下午茶了` 仍 -1） |
| 我先去检查一下 | 我想去检查一下 | **弃**（`我检查完再说` 仍 -1） |
| 我只能坐公交 | 我只能做公交 | **弃**（`我改乘地铁` 仍 -1） |
| 这趟车不停这一站 | 这趟车不听着一站 | **弃**（`这站不停` 仍 -1） |
| 我把数据核对一遍 | 我吧数据核对一遍 | **弃**（`数据我核过了` 仍 -1） |
| 这里不能长时间停 | 这里不能尝试舰艇（词典错音 + 全表无此路径） | **弃**（`这里不能久停` 仍 -1） |
| 安检队伍非常长 | 案件队伍非常长 | **弃**（`安检排队很久` 仍 -1） |
| 我用牙签清一下 | — | **弃**（句子本身不自然） |
| 话筒没有电 | 位次 2（三次独立测量同） | **保留并标注**（在表、可见，只是第 3 格） |

**B14（第 4 批已落表里那 9 条同类）本轮只出清单、没动表**：`我下楼取餐` `汤是凉的` `蜡烛插上`
`商家不回复` `地址填错了` `我看得见` `我挂个号` `我补个觉` `地还没拖`——
改/删它们等于动随包表（锁 + `.so` + APK），与本轮「不落表」冲突，
列为下批 `--append` 时的**随行修订清单**（同标准：同音高频压过 ⇒ 改写或删行）。

**裁定后的批**：`set.tsv` / `set-with-share.tsv` = **491 行**（500 − 9 弃），
`reserve.tsv` = 316 行（只吃了 E2/E3），`poly.py` 复算 **491/491 mismatch=0**、
批内裸多音段种类 44 → **25**；裁定后代理预检（步长 15 抽 32，实际跑完 31 条，
最后一条被我的 `timeout 340` 截掉）**24/31 = 77%、位次 ≥2 = 1**，FATAL 0。

**本轮归档**：`/d/toolchains/tmp/qingshan-snap/a3b5b/` = **80 个文件**（`c1scale.py` `c1oracle.py`
`derive_plain.py` `rule5.py` `revert_e4.py` `make_e4ab.py` `pparse.py` `probemeasure.sh`
`precheck10/11/12.sh` + 三份 `precheck*.log`、四份 `probe-*.log`、`c1-risk.txt`（2,524 行）、
裁定前后的 `set.tsv` `reserve.tsv`、`poly-*-after.txt`，以及 `cargo test` / `fmt --check` / 三轮 audit 的输出）；
探针前的源码与两份文档快照在 `qingshan-snap/b5probe/`。关键件都做了 md5 对盘（工作件 ≡ 归档件）。
`/d/toolchains/tmp/a3b5` 那个空目录本轮再试两次仍 `Device or resource busy`：现场有 3 个 `bash.exe`，
**推断**其中一个的 cwd 卡在那里（我先前的命令 `cd` 进去过、那个 shell 没退出）——
读别的进程的 cwd 要句柄工具，本机没有，所以这条按推断记，**没有杀进程**；目录是空的，等 shell 退出即可删。

### A3 第 5 批落表三合一（2026-10-14 17:30–18:10，表 **89,828 → 90,317 行**，已装机）

**用户裁定**：全落 491 条（不选"只落位次 ≤1"），并把 E2 的 169 条与 B14 的 9 条并到同一轮，
一次 `--append` 写表 + 重生成锁 + 重编 `.so` + `install -r`。理由记在案：裁定后批实测
24/31 = 77%、位次 ≥2 只 1 条；全落比只落 ≤1 多约 50 条可见整句，代价是约 23% 无害死行。

**关键机制先确认了一遍**（决定 ②③ 能不能用 `--append` 带出来）：
`engine/tools/build_glossary_subset.py` **每次都从「青简全量子集 + 源件」重建整张表**
（`:162-174`），源件就是随包表的手写条目出处 ⇒ 改源件里的**已落表行**（E2、B14）会照进新表，
不需要另外"in-place 更新"。源件 1,479 行 → **1,968 行**（+491 −2 删行）。

**落表前两道关卡**：
- **D2**：`poly.py` 复算 **491/491 mismatch=0**（第一次作为硬关卡跑；两份镜像 `set.tsv`
  与 `set-with-share.tsv` 前三列 0 差异）。
- **D6**：40 条代理预检（步长 12）：**前 5 格命中 25/40 = 62.5%，位次 ≥2 = 2 条**
  （`我打过一次没人接`、`加湿器要加水`）⇒ **未超用户给的 5 条阈值，可以继续**。
  ⚠ 62.5% 与上轮那个 77% **不是同一个测量**（样本 31 vs 40、步长 15 vs 12、
  且批内容经过 E4 还原与 B 改写），不能拿来比大小。

**② E2 的一致性处理（这里差点写错断言）**：源件里 `i will` 开头 **169 条**，
但其中 3 条（`我下楼取餐`/`我挂个号`/`我补个觉`）恰好也是 B14 要整行改写的行 ⇒
它们走改写分支、不计入"剥离"分支。`land5.py` 第一版写 `assert n_e2 == 169` **当场红**
（实改 166），查出来才是 166 + 3 = 169，断言改成两者之和。
落表后随包表里 `i will` 残留 **0 条**（`grep -cP '^\S+\ti will '` = 0）。

**③ B14 的 9 条全部按实测处置**（判据来自 `precheck-b14.log` 17 行 + `precheck-b14b.log` 2 行）：

| 原句 | 原句位次 | 改写 | 改写位次 | 处置 |
|---|---|---|---|---|
| 我下楼取餐 | -1 | `我去拿外卖` -1 → 二次 `外卖到了` | **0** | **改写**（两轮才找到能出的写法） |
| 蜡烛插上 | -1 | 我点了蜡烛 | 0 | 改写 |
| 商家不回复 | -1 | 店家没回我 | 0 | 改写 |
| 我看得见 | -1 | 我能看见 | 0 | 改写 |
| 我挂个号 | -1 | 我要挂号 | 0 | 改写 |
| 我补个觉 | -1 | 我要补觉 | 1 | 改写 |
| 地还没拖 | -1 | 我要拖地 | 0 | 改写 |
| 汤是凉的 | -1 | `汤已经凉了` -1、`汤凉了` -1 | — | **删行**（两轮改写都不出） |
| 地址填错了 | -1 | 不试 | — | **删行**（同义条目 `地址写错了` 已在表内且实测位次 0） |

⇒ 7 改写 + 2 删行；`地`/`觉` 那类裸字读音坑（`地还没拖` 派生成 `de...`）这次是通过改写绕开的，
没有再靠手敲拼音。

**逐行对盘（`tablecheck.py`，这是"动已落表内容"必须做的一步）**：旧表 89,828 → 新表 90,317，
差异归类 **新增 498（491 批 + 7 改写新头）／消失 9（7 旧头 + 2 删行）／同头改英文 166（E2）**，
**意外改动 0**；脚本任何一项对不上就 `exit 1`。表体积 2,167,528 → **2,189,354 B**（+21,826 B），
锁同步重写（`glossary-en.tsv = 2,189,354 B`）。

**重编与装机**：`.so` 4,355,360 / 4,602,032 B（**与上一版逐字节同尺寸**——锁只是数值变了、
长度没变）；`clean assembleDebug` APK **49,861,845 B**（+11,140 B）。
`install -r`、**不 `pm clear`**：解包件 `bootstrap/resources/glossary-en.tsv`
**2,167,528 B（10-13 23:37）→ 2,189,354 B（18:00）**，`dict.tsv` 仍是 2,053,612 B
⇒ refreshHostRaw ① 的"升级即重解包"在真长表的场景下再次成立。
`dictionary_outdated` 计数 **0**（口径注意：这条 0 是 18:04 之后窗口的稳定态——
`logcat -c` 在 18:04 跑过，18:00 那次重解包的事件行没被抓到，
所以"触发过"的正证是上面那对字节数，不是日志）。

**落表后真机验证（`land-verify.log`，`e1cdb946`，18:04–18:08）**：
- **新批 9 条全部露出且全部带译词**：`上门时间已经约定`@0 `home visit time is set`、
  `备份要测试恢复`@1 `test the restore of backups`、`屏幕时间要控制`@0、`我确认一下含义`@0、
  `昼夜温差比较大`@0、`师傅在路口等我`@0 `the driver waits at the crossing`、
  `报名已经截止`@1 `registration has closed`、B14 改写行 `我要拖地`@0 `need to mop the floor`、
  `外卖到了`@0 `the delivery has arrived`。
  （上一轮同一批未落表时这些格子的第二行是空的 ⇒ 这就是 A3 的效果，位次没变。）
- **对照 1**：`nihao` 第 0 格 `你好`·[`int. hello · int. hi`]、第 2 格 `你好像`·[`phr. you seem`]
  ⇒ 与方案 C 包逐格一致，落表没动词区。
- **对照 2（A3 不改排序）**：`womenhuijiaba` → `我们回家啊`@0、**`我们回家吧`@1**（带 `let us go home`）
  ⇒ 顺序仍是"啊"在前、"吧"在后，与 A1 那轮的实测一致。
- 环境：`default_input_method` 仍是我们的服务；`:ime` pid 14624→17762（会话重建）；
  `FATAL` 0；`词库载入 51 ms / 译词表载入 68 ms / 会话建立 119 ms`（与 1,000 条时代的 119/137/150 ms 同量级）；
  `读音修正 12 条` 那行仍在。
- 复位：学习目录三张表逐个显式名删除（PIN 第 34 条①），`total 0`；`accelerometer_rotation=1`。
- 静态：`cargo test` **51 passed / 0 failed**、`cargo fmt --check` **17 hunk 全在 `lib.rs`**、
  `verify_symbols.py` 两 ABI「导出 JNI=102 缺失=0 多余=0」、`verify_apk.py` **结论 True/True/True**。

**冒烟**：点选与空格上屏沿用上一轮的原序列复跑（不带那个清空步骤）⇒ 见下一节。

### 「修正表救不了整句路」——这条预判被实测推翻了一半（2026-10-14 18:06）

本轮要确认的是：方案 C 只加了词级读音，整句路是不是依旧不动。**答案是"不完全是"**：

| 输入 | 上轮（方案 C 之前）实测 | 本轮（方案 C 之后）实测 |
|---|---|---|
| `zhelibunengchangshijianting` | 首格 `这里不能尝试舰艇`，**整表没有**目标句（位次 -1） | 首页三格 = `这里不能长时间听` / `这里不能长时间挺` / **`这里不能长时间停`@2** |
| `anjianduiwufeichangchang` | 首格 `案件队伍非常长`，位次 -1 | 仍然 -1（`案件队伍非常长/…场/…唱`） |

机制归因（读码级，未做单测证）：修正表把 `长时间` 变成了**一个词**（key = `chang shi jian`），
于是组句图上多了 `这里|不|能|长时间|停` 这条边 ⇒ 整句**从"根本不产出"变成"第 3 格"**；
而 `非常长` 本身不是词、也没进修正表，所以 `安检队伍非常长` 照旧出不来。

**两个连带后果，都要用户裁定，我没擅自动表**：
1. `这里不能长时间停` 上轮是按"整表不可达"被删掉的（B11）。现在它**位次 2 可达**——
   但"能出"不等于"露得好"（前 2 格被 `听/挺` 占），而且它是本轮唯一一条实测到 2 的旧句 ⇒
   **要不要把它加回源件并重落一次**（再加一次表改动 = 再一次重编 `.so`）。我建议**不加**：
   第 3 格 + 没有语义优势的整句，收益低于再走一遍落表的风险。
2. 这直接改写了 D3 的前提：**修正表会通过"加词"影响组句图**，所以读清
   `correction/`（`costs.rs`/`edit.rs`/`typo/`）与 `fuzzy/rules.rs:106 expand()` 时，
   必须把"附加词库带来的边变化"一起算进去；可达率天花板不止由模糊音决定，也由词表决定。

`位次 ≥2` 累计（本轮全部实测）：落表前 40 条预检 2 条（`我打过一次没人接`、`加湿器要加水`）、
B 改写句 0 条、落表后 9 条抽样 0 条、整句路确认 1 条（`这里不能长时间停`）。
⇒ 二值行为的修正式仍是「位次 ∈ {0,1,2} 或不在表」，本轮没有一条埋到首页之外。

### D3 读清：模糊音、敲错代价表、组句边（2026-10-14 20:16–20:50，只读 + 真机判别，代码净改动 0）

**先给结论**：`si` 能出 `zi` **不是模糊音**，是**键盘相邻的"敲错"边**。青简把两件事分开：
`fuzzy/`（读音混淆，**缺省全关**）与 `correction/`（敲错，**默认参与**）。

**① 模糊音规则**（`crates/qingjian-core/src/fuzzy/rules.rs`）：`:11-38` 一共 9 条——
`z_zh` `c_ch` `s_sh` `n_l` `f_h` `l_r` `an_ang` `en_eng` `in_ing`；
`:8` 文档注释「缺省全关」，`:9` `#[derive(... Default ...)]` + `:10` `#[serde(default)]` ⇒ **默认全 false**；
`:42 ALL` 是全开（测试/CLI 用），`:59 NAMES` 是配置键名。
⇒ **表里没有 `s↔z` 这条规则**，所以 `si→zi` 不可能是模糊音。
可配置：`FuzzyRules::enable(&mut self, name:&str)->bool`(`:64`)、`set(:69)`、`is_on(:80)`；
Engine 侧 `with_fuzzy(:229)` / `set_fuzzy(:235)` / `fuzzy(:242)`；配置文件 `[fuzzy]` 分节
（`platform/config/mod.rs:73 pub fuzzy: FuzzyRules`），各上游壳都是 `engine.set_fuzzy(config.fuzzy)`
（`apps/cli/src/main.rs:306`、linux `:79`、windows `:216`）。**我们没接这个配置**（`grep -rn set_fuzzy engine/src/` = 0，本轮实测）。

**② 敲错代价表**（`correction/costs.rs:37-45`，`TypoCosts::DEFAULT`，全项目唯一一组）：

| 字段 | 值 | 含义（`:22-32` 的注释） |
|---|---|---|
| `transpose` | 5.0 | 相邻两键敲反（`shou`→`shuo`） |
| `substitute` | 5.0 | 敲到隔壁键（`ni`→`mi`）；**注释明写"不相邻的键不算敲错（那是读音问题，模糊音管）"**（`typo/kind.rs:7`） |
| `extra` | 5.5 | 多敲一键（`gang`→`gan`） |
| `missing` | 5.5 | 少敲一键（`gan`→`guan`） |
| `correction_penalty` | 5.0 | 整段纠错的惩罚 |
| `correction_transpose_discount` | 1.0 | 整段里换位更便宜 |
| `discount_cap` | 3.0 | 个人敲错表最多折掉多少 |

高低：`extra`/`missing`(5.5) > `transpose`/`substitute`/`correction_penalty`(5.0)，换位再减 1.0。
**没有声母/韵母/调号类别**（那些归 fuzzy）。`TypoCosts` **不对用户开放**，只有回放调参的
`set_typo_costs`（`engine/setup.rs:306`），引擎固定用 `DEFAULT`（`engine/mod.rs:418`）。

**③ 词级"敲错边"的门槛**（`engine/query/lookup.rs:9-34`，函数 `expand_positions(patterns, typos)`）：
`:14` 先 `self.fuzzy.expand(patterns)`，然后 `:24-26` 三种情况**不加**敲错边——双拼、
总字母数 < `MIN_LETTERS=4`（`correction/mod.rs:26`）、**非末尾位置带简拼/残缺音节**；
`:27-34` 对每个完整音节 `typo::variants()` 生成变体，`push_alternative(index, text, typo_cost(kind, accepted))`，
`accepted = learner.typo_count(...)` 是**个人敲错表**给的折扣（越常敲错越便宜）。
代价进排序：`ranking/mod.rs:64` `score = log_prob + weight_bonus − penalty`，模糊音固定扣
`FUZZY_PENALTY = ln 2 ≈ 0.693`（`ranking/mod.rs:33`，注释「词频减半」），敲错扣 5.0/5.5。

**④ 整段纠错的门槛**（`engine/query/phonetic.rs:68-78` → `engine/correcting.rs`）：
只有 `unlikely_pinyin`（尾巴有落不下的字母，或**非末尾**有残缺音节，`correction/mod.rs:39-48`）
或 `trailing_single_letter`（末音节是单字母，`:69-73`）才触发 ⇒ **切分完全合法的输入不会走整段纠错**。
且：只试**一处**编辑（`correcting.rs:69-70`「纠正后的拼音上不再猜第二处敲错」）、
"删掉最后敲的那个字母"不算纠正（`:64-67`）、**纠正后得分不严格高于原样就放弃**（`:90`）。

**⑤ `si→zi` 的判定与真机判别（本轮实测 5 条，`d3undo/adj3.log`）**
键盘表 `typo/mod.rs:17 ROWS = ["qwertyuiop","asdfghjkl","zxcvbnm"]`，
相邻判定 `:46-66`：同排 `|Δcol|=1`，跨排按"下排整体右错半个键"（`lower==upper || lower+1==upper`）。
`s` = 排1列1，`z` = 排2列0 ⇒ `lower+1==upper` ⇒ **相邻** ⇒ `si` 一处 Substitute 得合法音节 `zi`，代价 5.0。
我按这段代码写了个 `adjscan.py` 复算相邻关系，然后拿预测去真机证：

| 输入 | 预测（只从代码推） | 实测 |
|---|---|---|
| `mihao` | m↔n 同排相邻 ⇒ 该出「你好」 | **你好 @0**（格子：你好/迷糊/迷惑/迷幻）✓ |
| `lannvde` | l(排1列8) 与 n(排2列5) 不相邻，n_l 又默认关 ⇒ 不该出「男女」 | **男女不在**（格子全是 兰/蓝/懒/烂/岚/拦…）✓ |
| `zhongwne` | 切不干净 ⇒ 整段换位纠正 | **中文 @0** ✓ |
| `xueyapiangao` | g↔h 同排相邻 ⇒ 会给出「血压偏好」 | **血压偏好 @0**；且 `grep -P '^偏高\t' dict.tsv` **零命中**（偏高根本不是词）✓ |
| `sijizailukoudengwo` | s↔z 相邻替换 | **自己在路口等我 @0**，`司机` 只在第 4 格 ✓（与前两轮一致） |

**⇒ 机制确认：正确输入被"相邻键替代 + 词频"挤掉，不是模糊音。**

**⑥ 影响面（可达率天花板）**
- 正面：真敲错时能救（`zhongwne`→中文 @0 就是救活的例子），且个人敲错表会让常用错法越用越便宜。
- 负面：正确输入也会生成替代边（词级边**无条件**加，只要 ≥4 字母且切分完整），
  于是 A3 那批"末字/关键字同音"的整句被挤掉——`血压偏高`、`司机在路口等我` 是同一件事的两个样本。
- 硬上限：整句候选最多 **3 条**（`engine/mod.rs:372 SENTENCE_CANDIDATES`，用在 `converting.rs:88`），
  神经重排窗口 6（`:369 RESCORE_PATHS`），词级候选 500（`:366 MAX_CANDIDATES`），
  快照给壳的是 32（`engine/query/snapshot.rs:22`）。
  ⇒ 所以"整句不在表里"有**两种**成因：图上没这条边，或**进了图但挤不进前 3 条路径**。
  上轮探针只能证"表里没有"，分不开这两种。

**⑦ 附加词库确实改变组句边（PIN 第 14 条②要的那条）**
`converting.rs:198` `let dictionaries = self.all_dictionaries();` → `:205-206`
`sentence::convert_paths(&dictionaries, ...)` → `sentence/viterbi.rs:317-319`
`dictionaries.iter().flat_map(|d| d.lookup_exact_alt(span))` ⇒ 主词库、**附加词库**、用户词
**一起当格子**；边权 `:323` `score = frequency × (1 + seen) × exp(−penalty)` 里的 `frequency`
就是该词在**自己那份词库**里的词频 ⇒ C1 给修正行沿用原词频（`长时间` 7,429）是有意的，
改这个数就等于改组句权重。三者叠加：**词表（A3/修正表）决定有没有边与边权，模糊音决定有没有替代写法（默认无），敲错边决定替代写法扣多少**。

**⑧ 一个算不拢的地方（未决，下轮用观测口一次测清）**
按 `fallback_log_prob = ln(f+1) − ln(total) − 4` 与 `score = log_prob − penalty`：
`自己 454,875` vs `司机 14,613` ⇒ 词频优势 `ln(454875/14613) ≈ 3.44`，**小于**替代边的 5.0
⇒ 公式上"司机在路口等我"这条路径该赢，实测却是"自己…"占第 0 格。
两个候选解释：(a) `span_candidates` 每格只留"前几个"（`:294-296` 注释），格子层面的折扣排序可能已经把
`司机` 挤到别的位置；(b) 前 3 条路径的束宽剪枝。
**本轮没定论**；判据是"敲 `sijizailukoudengwo` 时全表里到底有没有 `司机在路口等我`"——
开一次观测口就能分。这件事直接卡住 A3 第 6 批的挑句规则，所以列为下一轮第一件事。

### 跨会话撤销评估（2026-10-14 20:50，只评估，代码净改动 0；等用户拍板）

**a. 账本在哪、什么形态、能不能持久化**：撤销账本是 `Engine::recent_commits: Vec<LastCommit>`
（`engine/mod.rs:169`，初始化 `:422`，容量 `RECENT_COMMITS = 4`（`engine/mod.rs:347`，
满了在 `composing.rs:158` 丢最旧）。`LastCommit` 字段（`engine/commit/last.rs:6-33`）：
`text`/`chars`/`input`/`chosen`/`transitions`/`typos`/`erased`/`log_id`/`phrase`。
**三条硬事实**：① `LastCommit` 只 derive 了 `Debug, Clone, PartialEq, Eq`（`:5`）**没有 Serialize**；
② `flush_learning`（`engine/learning/mod.rs:188-194`）只 flush `learner/logger/meter/vocabulary/translator`
**五样，不含 recent_commits**；③ **没有任何公开 API 能读回或写入 `recent_commits`**
（`grep "pub fn .*commit"` 只有 `commit`、`commit_translation`、`note_backspace`）。
⇒ **不改 Core 就无法把账本恢复进引擎**。

**b. 撤销窗口**：Core 自己给的窗口是**条数**不是时间——最多 4 条最近上屏（`RECENT_COMMITS=4`），
且 `is_erased()`（`last.rs:52-54`）要求"退格数 == 字符数"、`same_input()`（`:57-59`）要求重打的串是
原输入的前缀。跨会话（切走再回来）在 Core 里**已有机制**：`Engine::swap_session(&mut EngineSession)`
（`engine/session.rs:57-86`）会把 `recent_commits` 一起换进换出（`:63`），
`EngineSession` 就是"可挂起的输入上下文"。**我们没用它**（`grep -rn swap_session engine/src/` = 0，
只在 `keys.rs:89` 的注释里提过一次）——我们的会话是 `onFinishInput → destroy → Engine drop`。

**c. 落盘代价与共存**：现成的学习目录已有 6 张表（`user.tsv`、`user-words.tsv`、`user-ngram.tsv`、
`user-choices.tsv`、`user-english.tsv`、`user-typos.tsv`，文件名常量在
`qingjian-learning/src/frequency_learner/mod.rs:18-33`），实测落盘 7–9 ms、上限
`MAX_CHOICE_ENTRIES = 50_000`（`:36`）。加一张 `undo.tsv`（≤4 条）体积可忽略。
**但隐私边界是 Core 明确设计的**：`EngineSession::discard_input` 的注释
（`engine/session.rs:90`）写「不写日志、不学习，**也不把私密文本带到下一次普通输入**」，
而 `LastCommit.text` 正是用户刚打的文本 ⇒ 把它落盘等于把上屏文本写进一个跨会话文件，
需要产品明确同意（这是本项目第一次要往盘上写"文本"而不是"计数"）。

**d. 入口**：只有 ⌫（现状）⇒ 免费但要求用户"先删掉再重打"，跨会话时删的是应用里早已改掉的内容；
显式"撤销上屏"键 ⇒ 要 Java 破例（键盘上多一个键位 + `NativeClient` 新方法 = 第 102→103 个符号，
破例范围比方案 A 大得多，且要重打签名链之外的构建步骤）。

**候选方案与代价（不选）**：

| 方案 | 做法 | 代价 | 边界 |
|---|---|---|---|
| 甲 挂起而非销毁 | 我们侧改用 Core 的 `Engine::swap_session` + 按输入框/`application` 缓存 `EngineSession`；`onFinishInput` 不再 `destroy` | 只改我们 `engine/src/session.rs`（约 30–60 行 + 2 单测）；**零 Java、零 Core**；窗口 = 进程存活期（跨 app、跨输入框都保住） | 内存驻留变长；会话生命周期是击穿判定第五类的敏感面，改动要重跑一遍启动/切输入法/密码框三条路径 |
| 乙 我们自己的账本 + 近似撤销 | 我们落一张 `undo.tsv`，恢复时用 `Engine::forget` 或"重放一次错误学习再抵消" | 我们侧一张表 + 读写；**语义不等价**（`forget` 是清掉整个词的个人学习，不是退回那一笔；整句候选本来就不记学习，无笔可退） | 隐私面（写文本到盘）；`user.tsv` 等表的既有行为要重测 |
| 丙 请 Core 加 API | `Serialize` + `restore_recent_commits()` | **越界**（用户两次说不碰青简、不反馈上游） | 排除 |
| 丁 不做 | 保持方案 A 的会话内撤销 | 0 | 冷启动后无入口，已实测确认 |

**我的倾向**：甲。它不需要新持久化、不碰隐私边界、不动 Java 与 Core，就把"切走再回来"这个
最常见的"过一会儿才想起点错了"场景接住；真正的冷启动撤销（丁→乙）代价与风险都高一个量级。
**等用户拍板，本轮一行代码没动。**

### 算不拢测清 + 方案甲卡在「输入框身份」上（2026-10-14 22:57–23:35，探针轮，**代码净改动 0**）

**① 探针口径**：`engine/src/view.rs:177 build_view()` 里加一次 `probe_table()`，把
**整张表**连 `kind` 与 `syllables` 打进 logcat（`edit` 长度 ≥10 才打，免刷屏），量完删。
带探针的构建实测：`.so` 4,359,952 / 4,606,320 B，APK 49,882,517 B。
**还原证据**：`cmp` 四个 `.rs` 与本轮开局快照全部一致、`grep -rn PROBE engine/src/` = 0、
两个 `.so` 里 `grep -a -c PROBE` = 0、`.so` 回到 4,355,360 / 4,602,032 B、
`clean assembleDebug` 的 APK 回到 **49,861,845 B**、`cargo test` 51 passed、`cargo fmt --check` 17 hunk、
`audit_citations.py` 仍 322 条（OK 133 / AMBIG 102 / MISSING-PATH 75 / NOT-FOUND 12）。
真机：FATAL 0、`dictionary_outdated` 0、`default_input_method` 未变、学习目录 `total 0`（本轮没上屏过任何字）。

**② 结论：格子剪枝排除，束宽剪枝成立（PIN 第 15 条要的那一刀）**
11 条输入的**全表**里，`Sentence` 一律**恰好 3 条、且固定在 0/1/2 格**；`page_size` 是 10，
所以首页 = 3 句 + 7 个词。`sijizailukoudengwo` 的全表只有 54 行，
**`司机在路口等我` 不在其中**（第 0 格是 `自己在路口等我`，差 2 个字，它的音节是
`[zi,ji,zai,lu,kou,deng,wo]`——首音节被读成了 `zi`；`司机` 本身在整表 0 基位次 3，即首页第 4 格，
与前两轮那条"只在第 4 格"是同一个位置）。
两条对上 `MAX_CANDIDATES=500` 与 `SENTENCE_CANDIDATES=3` 的算术：`n=503` 出现了两次 = **500 词 + 3 句**。
⇒ 上一轮留的两个解释里，**(a)「格子层面的折扣排序把 `司机` 挤掉」死了**；
成立的是 **(b) 前 3 条路径的束宽剪枝**——`converting.rs:87-92` 把 `wanted = SENTENCE_CANDIDATES`
当参数交给 `sentence_paths(...)`，而 `converting.rs:200-204` 在没有神经重排器时 `k = want`
（我们这版没有 scorer，`rescoring/mod.rs:32-35` 的两个条件都不成立）
⇒ **k-best 枚举从一开始就只向路径生成器要 3 条完整路径**。

**③ 整句 −1 的两种成因占比（本轮问的 e）**：离线拿随包词库跑了一遍可达性
（`edges.py`，词边 93,248 条 = 主词库 93,236 + 修正表 12，音节串 63,172 种），
11 条**全部「有路」**，见证切分分别是
`司机/在/路口/等我`、`账号/密码/已经/重置`、`我在/高速/上/信号/不好`、`我/改成/标准/格式`、
`牙科/预约/在/周五`、`行李/限额/已/确认`、`内/循环/换/一下/空气`、`我/把/退票/手续/办了`
（对照三条命中的：`加湿器/要加/水`、`下一个/出口/更/绕`、`客户/反馈/已经/收到`）。
⇒ **「图上没边」0/8，「有边但挤不进前 3 条路径」8/8**。
A3 第 6 批的挑句判据因此**只该防一件事**：目标句要能挤进 k=3 的束，
而不是去补词边——补词边救不了它（本轮这 8 条本来边就齐）。
另有一类不在两种成因之内、但同样表现为"位次 −1"的**异体字**：`账号密码已经重置` 的第 1 格是
`帐号密码已经重置`（差 1 个字），`我改成标准格式` 的第 0 格是 `我改称标准格式`——
**引擎把整句打对了、只是某处选了另一个词**，这类靠 A3 表加条目**永远治不好**。

**④ 还没拢的地方（缩小了，但没关闭）**：束宽剪枝解释了"为什么全表没有"，
没解释"为什么 `司机` 那条路径连前三都进不去"。按 `viterbi.rs:323`
`score = frequency × (1 + seen) × exp(−penalty)`：`司机 14,613 × 1` 对 `自己 454,875 × e^-5 ≈ 3,065`，
词级分数是**司机赢 4.8 倍**，实测却是三条路径全以 `自己` 开头。
要判这一刀得看**路径分数本身**（`convert_paths` 内部逐格累计值），候选表这个仪器看不到它——
探针能证的已经证完了，剩下的不是"再 dump 一次"能解决的。

**⑤ MIN_LETTERS=4 小实验（只观测，没改码）**：`correction/mod.rs:26 MIN_LETTERS = 4`，
两处用它——`lookup.rs:24`（`letters < MIN_LETTERS` 不给词级敲错边）与
`correction/mod.rs:33`（整段纠正要求 4..=MAX_LETTERS）。真机三批：

| 输入 | 长度 | 目标 | 位次 | 说明 |
|---|---|---|---|---|
| `san` | 3 | 三 | **0** | 对照 |
| `sam` | 3 | 三 | **−1** | n↔m 相邻；首屏全是 `萨摩/萨姆/萨默`（`sa`+简拼），没有纠正痕迹 |
| `xan` | 3 | 三 | **−1** | **判别式**：s↔x 相邻，而简拼 `x` 出不来 `三`，缺就是被挡住 |
| `fen` | 3 | 分 | 0 | 对照 |
| `fem` | 3 | 分 | 4 | **假阳性样本**：`份额没/份额/飞蛾/发/分/放` 全是简拼读法，`分` 是被 `f` 简拼捞出来的，不是纠错 |
| `hoi` | 3 | 后 | 0 | **第二个假阳性**：`ho` 是 `hou/hong` 的合法**前缀**，前缀读法给的，与纠错无关 |
| `xian` | 4 | 先 | 1 | 对照（第 0 格是 `县`） |
| `toan` | 4 | 天 | **0** | 首屏 `天/甜/田/舔/填/添/恬` 全是 `tian` ⇒ 4 字母档纠正通了 |
| `cian` | 4 | 先 | **−1** | x↔c 相邻却**没纠**：`cian` 能切成 `ci`+`an`（合法），整段纠正按 `unlikely_pinyin` 跳过，而词级边不能把两个格子并成一个音节 `xian` |

⇒ 与代码一致：**3 字母没有任何纠正路**（`xan`/`sam` 两例），4 字母才通（`toan`）。
**但"用首屏格子判断有没有纠错"这个口径本身不可判别**——简拼与前缀读法会在 3 字母档
造出同字形候选（`fem`、`hoi` 两条就是证据）。要判别得看候选的 `syllables`
（`[fen]` 是纠错、`[f]` 是简拼），也就是得再开一次探针；本轮没有为这一条重开。

**⑥ 方案甲的实现前提不成立：native 侧没有任何"输入框身份"**（本轮唯一的拦路结论，代码一行没写）
- `qingshan/engine/src/lib.rs:430-441` `host_request()` 只取两个键：`resources` 与 `state_root`；
  `createRaw`（`:75-99`）拿到的 `options` 只有这两样 + 原样回显的偏好。
- `core/Bootstrap.java:43-44` 写进 `runtime-options.json` 的请求本身就只有
  `{"resources", "state_root"}`（非 full 版多一个 `edition`）。
- `core/MSIMEInputService.java:1696` 建会话前只补 `phrase_preedit`，`:1698` 就调 `create`。
- Java 手里**有**身份：`:1226 onStartInput(EditorInfo info, boolean restarting)`、
  `:1283-1284` 用 `currentEditorPackage` 算 `dedicatedEnglish`——但唯一落到 native 的是两个**布尔**：
  `:1709 setEnglishMode(session, dedicatedEnglish)` 与 `:1911 setPrivateSession(session, learningSuppressed())`，
  而后者在我们侧还是桩（`engine/src/stub.rs:126`）。
- 更硬的一条：**宿主自己也没有跨"离开再回来"的输入框身份**——`:1233-1235`
  `newDocument = !restarting || currentDocumentIdentifier == 0`，回到同一个框时 `restarting=false`，
  照样换新号；`:1238 onStartInput` 里无条件 `stop(false)`，`stop()` 在 `:1595-1600` 无条件 `destroy`。
- Core 那边留了槽位：`engine/session.rs:39 application: Option<String>`、
  `engine/composing.rs:74-75 pub fn set_application(...)`，**我们从来没喂过它**
  （`grep -rn "set_application\|swap_session" engine/src/` = 0 命中）。

⇒ 「按输入框缓存 `EngineSession`」**没有可用的键**。三条出路，请裁一条：
- **甲-1 单槽挂起（零 Java）**：`destroy` 时把 `EngineSession` 挂进一个 ≤2 槽的内存缓存，
  下一次 `create` 无条件下回装。验收①「同框返回能撤销」✅；
  验收②「不同框不串」**做不到**——换 app、换一个输入框都会把上一处最多 4 条上屏账装进新会话，
  而 `composing.rs:136-150 note_backspace` + `commit/mod.rs:328-349 apply_retraction`
  会照着它撤销那一笔学习。文本本身不会显示到别处（`LastCommit` 只进日志与学习），
  但这是**跨输入框的行为污染**，且与 `engine/session.rs:90`「不把私密文本带到下一次普通输入」的设计相反。
- **甲-2 按包名缓存（要 Java 破例 1 处）**：`startEngineSession` 往 `options` 里多写一个
  `application`（值就是已经算好的 `currentEditorPackage`），我们侧 `lib.rs:430` 多解一个键、
  建会话时 `set_application`，缓存键用它。**符号数不变（102 不动）、Java 只加 1 行、不改任何交互**，
  但破的是"host/java 零改动"这条红线；粒度是 **app 级**，同 app 内两个输入框仍会串。
- **甲-3 不挂起，改成把会话生命周期接长**：`onFinishInput` 已经无条件 `destroy`，
  我们侧动不了（动它就等于动 Java），排除。

本轮**没有按启发式硬上**：验收①②是互相排斥的，选哪个都要先改掉另一条。

### 方案甲-2 落地：按包名挂起输入上下文（2026-10-15 00:05–01:26，Java 破例 1 行 + Rust 挂起表）

**① 用户裁定**：选 **甲-2**（按包名缓存）。理由记录在案：甲-1 把上屏账带进别的输入框，
与 Core `engine/session.rs:90`「不把私密文本带到下一次普通输入」和 PRD 1.3「隐私优先」正面冲突；
甲-3 动不了（`destroy` 由 Java 无条件发，`MSIMEInputService.java:1595-1600`）；
「Java 零改动」是承诺不是法律，隐私优先 > 流程纯洁。

**② Java 侧 = 1 行代码 + 1 行注释**（`core/MSIMEInputService.java:1697-1698`，紧挨 `phrase_preedit` 那行）：

```java
if (!currentEditorPackage.isEmpty()) options.put("application", currentEditorPackage);
```

值就是宿主已经算好的 `currentEditorPackage`（`:446` 初值空串、`:1259` 每次 `onStartInput` 赋值）。
**与上游的总 diff 现在是 6 行改动内容**（2 行品牌字提示 + 本轮 2 行新增，`diff -u` 实测），
逐行声明见 `NOTICE.md` §5a 第 1 条。**符号数不变**：`verify_symbols.py` 两 ABI 各
「导出 JNI=102 缺失=0 多余=0，双向差集均为空」。

**③ Rust 侧**（只动 `qingshan/engine/src/session.rs` 与 `lib.rs`）：
- `SessionIdentity { application: Option<String>, learning_allowed: bool }`，
  `lib.rs:430-441` 旁边新增 `session_identity()` 解这两个键（`application` + `preferences.learning`，
  后者由 Java 在 `learningSuppressed()`（`:1905-1907`）为真时于 `:1298` 写进同一份 JSON）。
- `create_session` 里 `take_parked(key)` → `Engine::swap_session(&mut context)` 换回；
  `destroy_session` 里 **先 `Engine::clear()` 再 `swap_session` 换出** → `park(key, context)`。
  `clear()`（`composing.rs:208-218`）不碰 `recent_commits`、不碰链上前两个词，
  所以账本照旧跟着走；不清的话**半截拼音会跟着挂起项回来**（这条有专门的测试，见 ⑤）。
- 挂起表 `PARKED: HashMap<String, (EngineSession, u64)>`，上限 `PARKED_SESSIONS = 8`，
  按「最久没再挂」淘汰；**只在进程存活期内，不落盘**（`LastCommit` 没有 `Serialize`、
  `flush_learning` 也不含它，跨冷启动只能改 Core，已排除）。
- 两道隐私/身份闸：`learning_allowed == false` ⇒ 既不换回也作废已有挂起项；
  包名为空/缺失 ⇒ `cache_key()` 返回 `None`，既不挂也不换（行为与引入本特性前逐字相同）。
- 新增两条生产日志：`会话建立 … 挂起键=… 换回挂起上下文=true|false`、
  `会话挂起 键=… 表内 N 份`（真机上就是靠这两条把生命周期看清的，见 ⑥）。

**④ 一个被测试抓住的真 bug**：`SessionIdentity` 一开始用 `#[derive(Default)]`，
`bool` 的默认值是 `false` ⇒ `learning_allowed` 变成"不许学习"，
**所有没带身份的会话都会被当私密框**，方向正好反了。
`identity_without_a_package_never_becomes_a_cache_key` 这条测试（断言"没写 learning 时按允许处理"）
当场把它点红 ⇒ 改成手写 `impl Default`（注释里写明为什么不能用 derive）。

**⑤ 测试**：`cargo test --release` **59 passed**（本轮 51 → 59，+8）。
新增 6 条挂起用例（`session.rs`）+ 2 条身份解析用例（`lib.rs` 的第一个测试模块）。
证据口径：账本本身没有 getter，但 `Engine::history()`（`engine/setup.rs:352`）是公开的，
而 `history` 与 `recent_commits` 是同一次 `swap_session` 换进换出的两样
（`engine/session.rs:81` 与 `:63`）⇒ **用 history 当"上下文真回来了"的可见证据**。
六条的形状：主功能（同包名换回）、副作用锁（半截拼音不回来）、键锁（两个 app 各拿各的，
**单槽启发式甲-1 这条必红**）、隐私锁（私密框不继承不往外挂）、容量锁（第 9 份淘汰最老的）、
空包名锁（无身份会话不进挂起表）。
**三次自我证伪**：① 把 `session_identity()` 改成直接回 `Default` ⇒ 解析用例点红
（但"无包名"那条**不会红**，说明它只锁契约不锁逻辑 ⇒ 补了行为级的 `empty_package_name_is_never_a_cache_key`，
再把 `cache_key()` 的 `filter(!is_empty)` 去掉 ⇒ 该条点红 ⇒ 复原，`cmp` 证过）；
② 挂起表是进程级 static ⇒ 断言条数的用例**并发跑会互相污染**（实测单独跑绿、整组跑红），
加 `park_serial()` 串行 + 每条开头清空表；③ 见 ④ 的 derive bug。
`cargo fmt --check` 一度涨到 26 hunk（新增代码没按 rustfmt 形状写），
**逐处手改成 rustfmt 要的形状后回到 17 hunk、且全在 `lib.rs`**（没有跑写模式）。

**⑥ 真机验收（最终包 APK 49,896,069 B，`.so` 4,371,376 / 4,619,872 B）**
- 空格上屏：试用框打 `nihao` → 点**屏幕上的空格键** → 框 `[你好]`，再 ⌫ → 框 `[你]` ✓
- 跨会话：上屏 `我们` → HOME（`会话挂起 键="app.qingshan.ime"，表内 2 份`）
  → 回来自建会话 `挂起键=Some("app.qingshan.ime")，换回挂起上下文=true` ✓
  → 恢复后的会话里再上屏 `我们我们` ✓、⌫ → `我们我` ✓、FATAL 0
- 路径 A 冷启动：候选 6 格与上一轮逐字一致、`换回=false`（本进程第一次）✓
- 路径 B 切输入法：搜狗下我们不参与（格子数=0），切回后候选 ✓、`换回=true`
- 验收 b 换 app：`挂起键=Some("com.android.settings")`（**包名真的传进来了**）；
  第一次读数出现"设置第一次建会话却 `换回=true`"的疑点，**加挂起日志后查清不是 bug**：
  本进程更早有一次设置会话（它的 `会话建立` 行落在 `logcat -c` 之前被清掉），
  切走时打了 `会话挂起 键="com.android.settings"，表内 1 份`（01:17:20，早于 01:17:26 那次建立）
  ⇒ 隔离行为正确，**"看不见 create 行"≠"没发生过"**
- 验收 c 粒度限制（已知、不算缺陷）：同一个 app 里换到新的框仍 `换回=true`，
  因为键只有包名 ⇒ **同 app 内两个框共用一份上下文**
- 对照 d/e：`nihao` 首页 6 格不变；`我们回家啊` 位次 0、`我们回家吧` 位次 1（与 A3 落表后一致）
- 体检：`default_input_method` 未变、FATAL 0、`outdated` 0、学习目录已按显式文件名清成 `total 0`、
  自动旋转恢复 1、剪贴板面板已关、设置搜索框回到提示词
- **`pidof app.qingshan.ime` 返回的是 UI 进程**（本轮 20951），`:ime` 是另一个（24193）；
  要看 IME 进程得 `ps -A -o PID,NAME | grep qingshan`

**⑦ 路径 C（密码框）本轮仍未真机验证**——不是偷懒，是本机没有可安全到达的 `textPassword` 框，
四条取证：`am start -n com.android.settings/.Settings$WifiAddNetworkActivity` → `Error type 3 不存在`；
`-a android.settings.SEARCH_SETTINGS` 与 `-a android.net.vpn.SETUP` → `unable to resolve Intent`；
`com.android.settings` 首页 dump 里 `EditText=0`（点「搜索设置项」后才有 `AutoCompleteTextView`，那是普通文本框）；
我们自己 `host/res` + `host/java` 里 `textPassword`/`numberPassword` **0 命中**
（正样例：`EditText` 14 个文件、`inputType` 8 个文件、唯一值是 `inputType="text"`）。
WLAN 列表要点用户的真实网络、账号页要碰登录，**都不动**。
该路径由代码门决定：`policy/EditorPolicy.java:9-15` 的 `useEngine` 把三种 password variation
与非 `TYPE_CLASS_TEXT` 全部排除，`MSIMEInputService.java:1269-1270` 的 `engineWanted` 用它
⇒ 密码框**根本不建会话**，挂起代码不参与；最接近的真机替身是"不许学习的框"，
它由单测 `private_field_neither_restores_nor_parks` 锁住。
**（现值 2026-10-15 03:20：本轮仍未真机验。** 上一轮留给你的那问没答
（"要不要临时装一个带 `textPassword` 的测试页来验，还是就按上面这条代码门结案"），
按你给的分支 (c)"用户未提 = 跳过并保持等回复"处理：本轮**没装任何东西、没改任何东西**，
路径 C 继续保持"代码已判、真机未验"。**）
**（现值 2026-10-15 17:50：用户改裁"自建测试页验"⇒ 路径 C 已真机验过，本条洞已闭合。**
新文件 `qingshan/host/gradle-app/app/src/debug/java/app/msime/android/TestPasswordActivity.java`
与 `qingshan/host/gradle-app/app/src/debug/AndroidManifest.xml`（debug 源集，release 清单 0 命中）。
实测：密码框那一步 `会话建立` 计数**零增量**、只多一行 `会话挂起 键="app.qingshan.ime"，表内 1 份`；
回到普通框计数 +1 且 `换回挂起上下文=true`。上面那四条"到不了密码框"的取证**保留原文不动**，
它们解释的是为什么要自建页面。详见「密码框路径 C 真机验证」一节（含两个假象：
密码框上 `mInputShown=false` 是系统对第三方输入法的策略、dump 里 `password="false"` 不可信）。**）

### A3 第 6 批起草：备用池露出率 5/40，离线判据被降级（2026-10-15 01:50–02:35，**未落表**）

**做了什么**：备用池 `reserve.tsv` 316 条 → 与已落源件撞词头 **0** 条、池内重复 **0** 条 →
`poly.py` 多音字关卡复现不一致 **6** 条剔除 → **草稿 294 条**
（`/d/toolchains/tmp/qingshan-snap/a3b6/batch6-draft4-clean.tsv`，四列）。
派生拼音全部沿用第 5 批脚本产物，**本轮没有手敲过一个拼音**。
疑问清单 33 条：`a3b6/questions-b6.md`。

**① 新判据的实现与它的真实成绩**。写了一个按**音节格**对齐的离线 k-best 复刻（`a3b6/path.py`）：
合法音节集取自词库单字读音，词边来自 `dict.tsv` + C1 修正表，
分数 = Σ(ln(词频+1) − ln(总词频) − 4)（常数项抄 `ranking/mod.rs:64` 的 `fallback_log_prob`）。
两个必须记下来的弯路：
- 第一版把拼音当**字母串**做跨度匹配，给「我开窗透透气」排出第 1 名的是
  「我开出昂头头其」（`wo|kai chu|ang tou|tou|qi`，字母拼回去完全相等）——
  Core 的图是**先切音节再展开**（`lookup.rs:9-34` 按 pattern 下标加边），这种假路径在 Core 里不存在。
- 第二版漏了**每词常数项**，结果"切成更多单字"被系统性奖励，316 条里 0 条排进前 6。
  补上 `-ln(total)-4 ≈ 23.44` 之后名次才像话（总词频实测 275,909,169）。

**② 40 条代理预检（真机、截图口径，按名次分层各抽 10 条）**：
位次 0 = **4**、位次 2 = **1**、位次 −1 = **35** ⇒ **露出率 5/40 = 12.5%**。
按离线名次分档：名次 1 → 露出 5/10（其中 4 条位次 0、1 条位次 2）；
名次 2–6 → **0/20**；前 6 名之外 → **0/10**。
⇒ 判据**方向正确但远不够用**：`我散会后找他` 离线第 1 名就是它，真机第 1 格是「我三会有找他」；
`我拆成两个阶段`（离线名次 1）真机第 1 格「我吃撑两个阶段」。
**结论：离线名次只作粗筛，落表与否仍以真机预检为准**（这条已写进疑问清单 A3 请用户确认）。

**③ 露出与未露出的形状差异**（用来指导"要不要新写一批"）：露出的 5 条
（`换玻璃要预约`、`文档要更新版本`、`走快速路省十分钟`、`这样很危险快走`、`窗帘轨道卡住`）
共同点是**全句由高频双字词拼成、5–8 字、虚词不密集**；未露出 35 条平均长度差不多（6.5 vs 6.8 字），
差别在**词级边的密度**而不是长度。

**④ 顺手实测到的两条数据事实**：`卡住` 在随包词库里**没有词条**
（`grep -P '^卡住\t' dict.tsv` 零命中，正样例 `轨道` 命中）⇒「窗帘轨道卡住」只能逐字走，
实测位次 2；`poly.py` 在 294 条里挑出 **33 个裸单音节多音字**，最密的三个是
「已」26 条（另读 `si`）、「了」23 条（`liao`）、「车」16 条（`ju`）。

**⑤ 本轮没做的事**：不落表、不 `--append`、不改锁、不重编 `.so`/APK、不碰 `dict.tsv`
（mtime 全部未变，见本轮验收）；英文自审只手工过了 30/294 条（挑出 9 条要改的，见清单 F 组），
**没跑全批人工过**——这一条在清单 G3 里请用户定。

**⑥ 预检脚本自己踩的坑（值得进环境陷阱）**：`while read` 循环体里调 `adb`，
**每条 adb 都必须 `</dev/null`**——`uiautomator dump` 会把循环的 stdin（那份 TSV）吃掉，
结果 40 条只跑了 1 条就"正常结束"，日志看着像"这批全沉"。第一版还误判成 `read -u 9` 的问题。
另：清缓冲区用 `input keyevent 67 67 67…` 一次发一串，逐条发 40×18 次要白等多分钟。

### A3 第 7 批起草：按形状新写 + 只留离线名次 1，露出率 35/40（2026-10-15 02:51–03:15，**未落表**）

**判据从哪来**：第 6 批露出的那 5 条共同形状是「全句由高频双字词拼成、5–8 字、虚词不密集」
（见上一节 ③）。本轮**起草阶段就按这个形状写**，再用 `path.py` 的离线名次当验收，
与第 6 批"先写完再筛"的顺序相反。

**链条与每一关的数字**（全部机械，本轮同样没手敲过一个拼音）：

| 关 | 结果 | 产物 |
|:---|:---|:---|
| 起草（九档，**情绪客套补齐**——第 6 批那档是 0 条） | 187 条 | `a3b7/draft7-raw.tsv` |
| `derive5.py` 派生 + 去重 | 181 条（切不通 1 = `CT报告还没出`、与源件重复 5、池内重复 0、**已在随包表 0**） | `a3b7/derived7.tsv` |
| `poly.py` 关卡 | **复现一致 181/181，mismatch=0**；裸单音节多音段 18 个字（最密「了」18 条） | `a3b7/poly-batch7.txt` |
| `path.py` 离线名次 | **名次 1 = 69**、2/3/4/5/6 = 22/11/9/6/1、前 6 之外 63（存活率 38.1%） | `a3b7/rank7.tsv`、`batch7-rank1.tsv` |
| 真机 40 条代理预检（名次 1 里等距抽 40，覆盖 69 条中的 58%） | **露出 35/40 = 87.5%**：位次 0 = 33、位次 1 = 2、位次 −1 = 5；`FATAL EXCEPTION` 0 条 | `a3b7/precheck.log` |

**① 与第 6 批的公平对照（混杂变量必须写清）**：第 6 批的 40 条是**按名次分层**抽的
（名次 1 十条 + 名次 2–6 二十条 + 名次外十条），第 7 批的 40 条**全是名次 1**。
同口径只能比第 6 批的名次 1 那一层：**5/10 = 50% → 35/40 = 87.5%**。
「形状控制」与「只留名次 1」各自的贡献**分不开**——要分开得再跑一组
"形状好但离线名次 2–6"的对照（约 7 分钟真机），本轮没跑（不在授权范围）。
**停手判据（< 30%）没触发**，所以按你给的规则继续起草、不落表。

**② 沉下去的 5 条 + 挤到第 2 格的 2 条，形状全部是"被近音/纠错读法抢"，不是"图上没边"**
（与第 15 条的结论同向）：`司机在路口等我`→第 1 格 `自己在路口等我`（老问题，`path.py` 文件头记的就是这条）；
`快递今天能到吗`→`梦到`（`neng`→`meng`，**首键 n↔m 相邻**）；`我发烧到三十八`→`找到`；
`作业还没写完`→`好美写完`；`房间少一床被`→`一场被`；位次 1 的两条是
`这次是我疏忽`→`属于`、`我丢了一张票`→`都可以`。
**这 7 条一条都不该靠改 `dict.tsv` 救**（你裁过"别为 5 条改表"）。

**③ 本轮新查的两个冗余口径**（第 6 批没查过）：把 69 条与现有 1,968 条源件比对，
**英文逐字撞车 4 条**（`买单可以刷卡吗`/`可以刷卡吗` = `can i pay by card`；
`这个要退货`/`我要退货`；`抱歉我来晚了`/`对不起我迟到了`；`门票已经买好`/`景点门票已经购买`）、
**中文头互为包含 6 对**（`司机在路口等我`⊃`在路口等我` 等）。
去重脚本只看中文头，这 10 条**不会被现有流程挡掉**。

**④ 口径确认**：源件是**两列** `中文<TAB>英文`（1,968 行；英文首字母大写 **0** 行、
含句末标点 **1** 行、含逗号 **11** 行）⇒ 草稿侧的拼音列/占比列不进表，本轮起草全部按小写、
几乎不加句末标点，与源件同形。

**⑤ 本轮没做**：不落表、不 `--append`、不改锁、不碰 `dict.tsv`/`.so`/APK（mtime 见本轮验收）；
英文自审只做到"逐条看我这 69 条 + 两条机械比对"，标出 **12 条建议改**，其中
`我马上到`→`sorry, i am on my way`、`孩子睡着了`→`quiet, the kid is asleep`
**英文多了中文没有的内容**（这两条是硬伤，其余是语域/时态）。
疑问清单 15 条：`/d/toolchains/tmp/qingshan-snap/a3b7/questions-b7.md`。

### 密码框路径 C 真机验证：自建 debug 测试页，通过了（2026-10-15 17:29–17:50）

用户裁定：**不接受"未真机验证"留洞**，自建一个极简测试页。做完的结论先说：
**代码门成立——密码框不建会话、不出候选；普通框建会话、出候选**，路径 C 从"代码已判、真机未验"
升级为"真机验过"。

**页面放在哪、为什么不是 `host/java`**：两个新文件都进 Gradle 的 **debug 源集**——
`qingshan/host/gradle-app/app/src/debug/java/app/msime/android/TestPasswordActivity.java` 与
`qingshan/host/gradle-app/app/src/debug/AndroidManifest.xml`。这样 release 变体**既不编译也不合并清单**，
实测 `aapt dump xmltree` 在发布包清单里搜 `TestPasswordActivity` **0 次**。
放 `host/java`（main 源集）的话类会进发布包的 dex，而清单没有它 = 一段永远走不到的死代码随包分发。
代价：`app/build.gradle.kts` 一行没改（源集是 AGP 默认位置），但文件路径与用户原话"在 host/java 新增"
不同，**这是有意的偏离**，已记进 NOTICE §5a 第 6 条。

**判据与实测**（`e1cdb946`，`a3bj/verify-c.log`）：进页面时普通框自动聚焦 ⇒ `会话建立` 计数 1；
点密码框打 `nihao` ⇒ 计数**仍是 1**（新增 0 条），日志只多了一行
`会话挂起 键="app.qingshan.ime"，表内 1 份`；再点回普通框 ⇒ 计数 2，且
`会话建立 121 ms（含词库载入），挂起键=Some("app.qingshan.ime")，换回挂起上下文=true`。
**顺带白捡一条甲-2 的跨输入框证据**：同一 app 内换框，挂起→换回真的发生了。

**一个必须写下来的假象**：密码框那一步 `mInputShown=false`（键盘窗口没显示）。第一反应是
"青杉在密码框里不弹键盘 = 用户没法输密码"，**A/B 证伪**：把默认输入法换成出厂搜狗，
同一页同一密码框同样 `mInputShown=false`、同一页普通框 `true` ⇒ 是**系统对第三方输入法在
密码框上的策略**，不是我们的行为。所以"候选条不出现"这一条在密码框上是**被系统盖住的**，
真正有判别力的是**会话建立计数 0 增量**。判"现在是谁在服务"仍只看 `mCurMethodId`（第 42 条）。

**另一条别踩**：dump 里密码框节点 `password="false"`、文本原样露出 `nihao`——
**不要拿 `password` 属性当"这是密码框"的证据**，本轮它是假的（行为侧两处都证明 inputType 带了
password variation）。

### A3 第 7 批落表 63 条（2026-10-15 17:55–18:45，表 / 锁 / `.so` / APK 全部重出）

用户裁定：**直接落名次 1 的那批，不扩写到 200**；C 组冗余按标准处理。

**先裁冗余再落**（`a3bj/`）：69 条名次 1 里，与现有 1,968 条源件**互为包含的 6 条全删**
（`司机在路口等我`、`买单可以刷卡吗`、`我要复查一次`、`风太大了关门`、`家里网络断了`、
`酒店能开发票吗`——其中只有 `司机在路口等我` 预检未露出，其余 5 条位次 0，
但"0 包含冲突"这条验收是机械的，不留例外），**改写英文 14 条**（3 条是为消掉逐字撞车、
11 条是疑问清单 F 组的自审修正）。**落表 63 条**。
复跑关卡：`poly.py` **63/63 一致、mismatch=0**；机械冗余检查
**英文逐字撞车 0 / 互为包含 0 / 同头 0**（`assert` 兜底，不是眼看）。
**改写英文时新撞出来一条**：`我马上到` 改成 `i am on my way` 正好等于已有条目
`我在路上了` 的译词——**改译词之后必须重跑查重**，不然修一个洞挖一个洞。改成
`i will be there shortly` 后才 0。

**落表前 40 条确认预检**（最终 63 条里等距抽 40，同一份装机包）：**露出 35/40 = 87.5%**，
位次 0 = 33、1 = 2、−1 = 5，`FATAL` 0，`DUMP_FAILED` 0 次 ⇒ 与 03:04 那轮同分布，
判据没有因为抽样子集变化而漂移。

**正规通道一次走完**：`engine/tools/glossary-sentences-zh-en.tsv` 追加 63 行（1,968 → **2,031**，
唯一头、LF、无 CR）→ `build_glossary_subset.py --out ../host/assets/dictionary/glossary-en.tsv`
一次写表与锁（**90,317 → 90,380 数据行**、2,189,354 → **2,192,288 B**，+63 行/+2,934 B 对得上）
→ `cargo ndk` 重编两 ABI `.so`（体积没变：**新旧锁的字节数都是 7 位十进制**，`include_str!`
长度一样，所以只能靠内容判定——实测 `grep -c 2192288` 命中 1、旧值 `2189354` 命中 0）
→ `verify_symbols.py` 两 ABI 各 **102 导出、缺失 0 多余 0** → `clean assembleDebug`
（**49,897,529 B**，比上一版干净包 +1,460 B；顺带再证一次"增量重打包体积不可信"：
同一份代码增量构建出 51,502,819 B，虚高 1.6 MB）→ `install -r` 不 `pm clear`。

**真机验收**：冷启动后 `run-as … ls files/bootstrap/resources` 的 `glossary-en.tsv` =
**2,192,288 B**（跟上了新表），`dictionary_outdated` 首启 2 行、**再启一次 0 行**（不误报不重复解包）；
试用框打 `wozaizhantaidengni` → 第 1 格文本 = **`我在站台等你⏎i am waiting on the platform`**
（位次 0，译词同行第二行）⇒ 新落的整句与它的英文释义真机可见。
`cargo test --release` **59 passed**（含 `packaged_lock_matches_shipped_assets`，锁与表脱节会红）。

### 签名链落地：第一个 release 包（2026-10-15 18:17–18:50，用户批准方案后执行）

**密钥**（仓外，永不进仓）：`D:\toolchains\keys\qingshan-release.p12`，
alias `qingshan-release`，RSA **3072**、有效期 **10,000 天**（≈27 年）、
`CN=Qingshan IME Release, O=Qingshan`，**证书 SHA-256 `8eb90f8dc8c4efc561c3d1743c53b0b2818444bba78d3bbc635f2d77b7627132`**。
口令 28 位随机、只写在 `qingshan-release.p12.password`（本轮生成后没有打印过），
`.gitignore` 加了 `*.p12` / `*.keystore` / `*.jks` / `*.password` 四条。
**这把钥匙发出第一个包之后就换不掉了**（换 = 现有装机用户收不到更新，同上游
`release-android.yml:25` 的理由）。**待办：用户自己再做一份加密离线备份**——现在这台机器是唯一副本。

**脚本**：`host/scripts/sign-apk.sh`（`host/scripts/` 之前是空目录）。四步次序抄上游
`build-apk.sh:180-185`：`zipalign -f -P 16 4` → `apksigner sign --out` → `apksigner verify --verbose --print-certs`
→ `zipalign -c -P 16 4`，末尾删掉 apksigner 顺手写的 `.idsig`。三条刻意的取舍：
① 缺密钥或缺口令**直接 exit 1**，不退到调试密钥；② 口令只从
`QINGSHAN_RELEASE_KEYSTORE_PASSWORD` 或 `<密钥库>.password` 首行取，不进命令行、不进 Gradle；
③ Windows 上 JVM 不认 `/d/...`，脚本用 `cygpath -w` 自己换算参数。

**三件必须实测的事，结果**：
1. **产物名** = `app/build/outputs/apk/release/app-release-unsigned.apk`（release 块没有
   `signingConfig` 时 AGP 8.13.2 就出 `-unsigned`，实测）。
2. **lint 门禁在发布路径上生效**：`assembleRelease` 跑了
   `lintVitalAnalyzeRelease` / `lintVitalReportRelease` / **`lintVitalRelease`**，BUILD SUCCESSFUL
   ⇒ `checkOnly += "NewApi"` 那道门不会被发布构建绕过（此前只是推断）。
3. **`run-as` 在 release 上失效**（这条最要紧，早测是对的）：
   `run-as: package not debuggable: app.qingshan.ime`，`ls /data/data/app.qingshan.ime` →
   `Permission denied`（本机未 root）。**受影响的是所有"读应用私有文件"的判据**：
   解包件字节数、`files/qingshan/user*.tsv` 学习表、`flush_learning` 落盘、
   refreshHostRaw 的两侧对盘。**不受影响**：`logcat -s qingshan`（会话建立/挂起、词库载入字节数
   都在，release 上实测照打）、`uiautomator` 的 UI 口、`dumpsys`。
   ⇒ 流程定成：**文件类判据只在 debug 包上做，release 包只做日志口 + UI 口的验收**；
   不给 release 加 `debuggable=true`（拿整个应用门户换几条日志）。
   另有一条没走的路：`allowBackup="false"`（`host/AndroidManifest.xml:19`）⇒ `adb backup` 也拿不到。

**发布包真机验收**：`install -r` 跨证书必失败，原文
`INSTALL_FAILED_UPDATE_INCOMPATIBLE: Existing package app.qingshan.ime signatures do not match newer version; ignoring!`
⇒ 卸载 debug 再装 release（丢本地词库与设置，本机可接受）。装完金路径全过：
试用框 `wozaizhantaidengni` 第 1 格带译词、点候选 → `我在站台等你`、清空后 `nihao` + 点空格
（本机现取 `(450,2251)`）→ `你好`，`FATAL` 0。日志显示 `译词表载入 68 ms / 2192288 B` ⇒
发布包用的是**刚落表的新表**。体积：未签名 45,966,895 → 签名 46,070,982 B（+104,087），
比同日 debug 包小 3,826,547 B，差额几乎全是压缩率（同一 CRC 的 `dict.tsv`：debug 内 964,523 B、
release 内 859,681 B）。**当前装机状态：release 0.2.0**（要回到可观测状态需再卸载装 debug，
切换一次就要重装一次，见环境陷阱第 46 条）。

**顺手发现的两条**：① `verify_apk.py` 把期望版本写死成 `0.1.0`/`1000`，版本一 bump 就误报
"清单不符"——改成从 `host/version.txt` 读（与 Gradle 同一个唯一来源），两个包现在都是
`包内容=True 符号=True 清单=True`；② 这台 OPPO 系设备**adb 装包会弹人工确认**
（`com.oplus.appdetail` 的安装引导页，要点两次「继续安装」，中间还有一步"正在扫描"），
`adb install` 会挂在那儿不返回，本轮 timeout 124 就是这么来的。

### GPL 源码分发完整性评估（2026-10-15 20:25–20:55，**只评估，未改 `.gitignore` 与 `NOTICE.md`**）

**先把"到底缺什么"量出来**。用 python 按 `.gitignore` 的每条模式扫全仓，实测分类：

| 被忽略的东西 | 文件数 | 字节 | 判定 |
|:---|---:|---:|:---|
| 构建产物 / 缓存（`app/build`、`.gradle`、`engine/target`、`.cxx`） | 11,122 | 2,568,448,755 | 正确忽略 |
| 交叉编译产物 `engine/jniLibs/**/*.so` | 2 | 8,991,248 | 正确忽略（`cargo ndk` 可重建，README 构建步骤第 2 步） |
| **随包数据 `host/assets/dictionary/*.tsv`** | **2** | **4,245,900** | **本轮要判的就是这两个** |
| 密钥与口令（`*.p12`/`*.keystore`/`*.jks`/`*.password`） | 0 | 0 | 仓内零命中（`find` 无输出；这四条模式确实在 `.gitignore` 里） |

`host/gradle-app/local.properties` 这条模式当前没有对应文件（本机没生成过）。

**纠正本轮简报的一处前提**：`engine/src/reading-fixes.tsv`（1,734 B）与
`engine/tools/glossary-sentences-zh-en.tsv`（74,167 B，A3 源件）**不在忽略列表里**，
`.gitignore` 只挡 `host/assets/dictionary/` 下的 `*.tsv`/`*.qj`。所以"数据源件被忽略"这个担心
对这两个文件不成立；真正被挡的只有 `dict.tsv`（2,053,612 B）与 `glossary-en.tsv`（2,192,288 B）。

**可复现性实测（判合规的关键证据，两条都是 `cmp` 逐字节比）**：

1. `glossary-en.tsv`：把 `build_glossary_subset.py` 的 `--out`/`--lock` 指到仓库外的 tmp 目录重跑一次
   （随包表与锁因此**没被碰**，mtime 仍是 18:03:30），产物与随包那份 **逐字节相同**（2,192,288 B）。
   它的全部输入 = 青简 `assets/lexicon/dict.tsv` + 青简 `assets/glossary/glossary-en.tsv`
   + 我们的源件 + 我们的脚本 ⇒ **纯派生物**。
2. `dict.tsv`：与 `projects/qingjian/assets/lexicon/dict.tsv` **逐字节相同** ⇒ 它是上游文件的原样副本，
   不是我们的产物。`projects/qingjian` **有 `.git`**（HEAD `4a3e963d…`，本文件第 8 行），
   `msime-develop` 没有 `.git`、靠 zip 归档 MD5 `cfdab2624a8e6469b588984477f1ed65` 固定（`:20`）。

**条款原文**（逐字抄本仓 `qingshan/LICENSE`，行号是那份文件的行号）：

- `:51` 「The "source code" for a work means the preferred form of the work for making
  modifications to it.」
- `:57` 「The "Corresponding Source" for a work in object code form means all the source code
  needed to generate, install, and (for an executable work) run the object code and to modify
  the work, **including scripts to control those activities**.」
- `:59` 「**The Corresponding Source need not include anything that users can regenerate
  automatically from other parts of the Corresponding Source.**」
- `:87`（§5a）「The work must carry prominent notices stating that you modified it, and giving a
  relevant date.」——这条 `NOTICE.md` §5 已满足（Java 4 行 + 未取的两个 `.kt` + 新测试页）。
- `:107`（§6d）「Convey the object code by offering access from a designated place (gratis or for
  a charge), and **offer equivalent access to the Corresponding Source in the same way through the
  same place** at no further charge. … If the place to copy the object code is a network server,
  the Corresponding Source may be on a different server … provided you **maintain clear directions
  next to the object code saying where to find the Corresponding Source**.」

**据此的判断**：`:59` 直接覆盖 `glossary-en.tsv`（可自动重建 ⇒ 不必随包给）；`dict.tsv` 是上游原样副本，
"上游仓库 @ 固定 HEAD 就是它的源码"也成立，但**必须在分发处写清去哪取**。
⇒ **真正的阻塞不是 `.gitignore`，是没有一个第三方能取到的公开位置**：没有公开仓库，
`:107` 那句 "clear directions" 就无处可写，`:59` 里"从 Corresponding Source 的其余部分重建"的
"其余部分"（我们的 `engine/` + `host/` + 源件 + 脚本）也拿不到。
**改 `.gitignore` 不解决这个；公开地址才解决。**

**三条处置方案与代价**：

- **方案 B（推荐）：数据不进仓，只把"从哪取、怎么重建"写全。** 代码 0 行、`.gitignore` 不动，
  改的是 `NOTICE.md` §5 与 `README.md` 构建前提，要补四项：
  ① 青杉仓库的公开地址（**唯一真正要做的动作**）；② 青简的公开地址 + 固定 HEAD `4a3e963d…`；
  ③ 水杉那份无 `.git` 的拷贝要给"别人从哪下到这个 zip"的出处（现在只有 MD5）；
  ④ 一条 `build_glossary_subset.py` 的重建命令（`README.md` 已有，要从 NOTICE 指过去）。
  依据就是 `LICENSE:59` + `:107`。缺点：第三方要多走一步"自己取上游 + 跑脚本"。
- **方案 A：只把 `dict.tsv` 移出忽略列表进仓**（2,053,612 B，clone 即得，防上游改名/删除）。
  **不建议 `glossary-en.tsv` 也进仓**：它是纯派生物，进仓就多出一个"表 ≠ 源件 + 脚本"的会腐烂的副本
  （现在这个不一致由锁 + `packaged_lock_matches_shipped_assets` 单测挡着）。
- **方案 C：Git LFS / release 附件。** 只有"希望一次下载就拿到随包数据"才需要；
  代价是 LFS 配额 + clone 后还要 `lfs pull`，而存的是可重建的冗余 ⇒ 性价比最低，不推荐。

**顺手量到、按"本轮不动 NOTICE"没改的三处过期数字**（拍板时一起修）：
`NOTICE.md` §随包数据 写 `glossary-en.tsv`「1,815,026 B，76,273 行」与「前 80,000 个词」
⇒ 现值 **2,192,288 B / 90,380 行**，裁剪口径自 A1 起已是"词库全量 93,236 ∩ 全量译词表"；
锁「52 B」⇒ 现值 **167 B**；「86 个 `not_implemented` 桩」⇒ 现值 **85**（REAL 17 + 桩 85 = 102）。

### 「检查更新」三选一的破例面（2026-10-15 20:50 读码，**未改 Java**）

**先纠正一个前提：`local_mode` 不是"关掉网络功能"的开关。** 它是键盘视图状态，7 处读它：
`ImeGlideTyping.java:94`、`ImeLetterRows.java:210` 与 `:494`、`ImeNineKeyPanel.java:139`、
`ImePanels.java:47`、`MSIMEInputService.java:759` 与 `:899-900`，控制滑行输入、键位排布与面板渲染。
拿它关更新入口既关不掉（这几处不看关于页），又会改键盘行为——正是 D 方案被排除的同一类理由。

**真正的开关是一个谓词**：`UpdateJobService.java:123-125` 的 `installedFromPlay()`。三处消费它：
`AboutPage.java:119` 取到 `play` ⇒ `:122-126` 决定「自动更新」「更新通道」两行建不建，
`:190`/`:219` 决定「检查更新」药丸显不显示；`UpdateJobService.java:51` 决定每天一次的后台任务跑不跑。

| 方案 | Java 破例面 | 附带前提 | 残留危害 |
|:---|:---|:---|:---|
| **b 关掉入口（推荐）** | **1 个文件 1 行**（让 `installedFromPlay()` 恒真 + 一行注释） | 无 | 无（入口与后台任务一起没了） |
| c 保持现状 | 0 行 | 文档已记 `README.md` 已知失效项第 11 条 | 用户主动点「检查更新」仍会白下一趟 |
| a 指向青杉仓库 | 1 个文件 **5–6 处**（`:45` RELEASES_URL、`:47` DOWNLOAD_PREFIX、`:49` MIRROR_PREFIX、`:51-52` HOSTS 白名单、`:138` 资产名 `msime-android`） | 要有等价的 releases 列表端点（`msime.app/api/releases` 是水杉自建镜像；GitHub 原生 API 有 60 次/小时未鉴权限流——那正是镜像存在的全部理由）；发布侧还要上传 `<apk>.sha256`（`:36`「没有校验文件的发布一律不装」） | 无，但依赖公开仓库先落地（与 GPL 方案 B 同一前置） |

**一条降低 c 方案严重性的实测**：`UpdateJobService.java:92-94` 的 `autoUpdate()` 默认值是
`getBoolean(KEY_AUTO, false)` ⇒ **自动更新默认关**，后台不会自己下载；只有用户手动点
「检查更新」或自己打开开关才有那趟白下载。所以 `README.md` 第 11 条那句"死循环"要读作
"手动触发一次、白下一趟、装不上"，不是每天自己发生。

**推荐**：先做 **b**（1 行把危害清零，且不动上游那 5 处地址常量）；等真要发对外版本、
公开仓库有了，再按 **a** 接回青杉自己的发布物（那时 `.sha256` 与 release 命名一起定）。

### A3 分离变量实验：形状 vs 离线名次（2026-10-15 20:40–20:52，真机 40 条，数据零改动）

**设计**：从第 7 批那 181 条里取**离线名次 2–6** 的 49 条，用与第 7 批同一道形状闸
（5–8 字 且 单字段占比 ≤ 1/2）筛出 **44 条**，等距抽 **40 条**（名次 2/3/4/5 = 19/9/7/5）。
这 40 条**全部不在随包表里**（落表的是名次 1 的 63 条），所以判据仍是"未落表状态下能不能露出"。

**结果**：**露出 8/40 = 20.0%**；位次分布 =1 有 6 条、=2 有 2 条、−1 有 32 条；`FATAL` 0、`DUMP_FAILED` 0。
**最强的一条**：**名次 2–6 没有一条进第 1 格（0/40）**，而名次 1 那 40 条里 33 条在第 1 格。

**结论（第 8 批判据据此定）**：
1. **形状闸有效但弱**：同为名次 2–6，第 6 批（无形状闸）0/20 ⇒ 本轮（有形状闸）**8/40 = 20%**；
2. **`离线名次 = 1` 才是决定性的**：20% → 87.5%，且只有名次 1 能进第 1 格；
3. ⇒ 第 8 批**两道都要**：先按形状闸起草，再只留离线名次 1；名次 2–6 不进批。

**两条混杂必须写清**：① 本轮跑在**已落表 63 条之后**的包上，词图比测名次 1 那批时多了 63 条整句边
（方向上只会让对照组的 20% 偏乐观）；② 名次 1 的 87.5% 是 18:0x 在**旧表**上测的，两组表状态不同。
所以"87.5% vs 20%"是跨状态比较；而"0/40 进第 1 格 vs 33/40"这组不受影响——多几条边不会把对照组
推进第 1 格，真推进了日志里会显出位次 0。

### GPL 方案 B 落地：本地建仓 + 首次提交 + 更新入口关掉（2026-10-15 21:27–21:55）

**公开地址仍是占位**：用户选了"稍后自己填"，所以 `NOTICE.md` §5 与 `README.md`「构建前提」里
青杉自己的地址写的是 **`https://github.com/<待填>/<仓库名>.git`，并标了「⚠ 待填」**——
这不是一个值，push 之前 §5 那套"从哪取"的承诺都不生效。**两个上游的地址是查出来的、不是猜的**：
青简 = 本地那份 `git remote -v` 的 origin `https://github.com/romantic404TT/qingjian.git`
（HEAD `4a3e963d…`）；水杉 = 其 `README.md:8` 的「GitHub 仓库」链接
`https://github.com/metasequoiaime/msime`（本地那份无 `.git`，固定依据仍是 zip 的
MD5 `cfdab2624a8e6469b588984477f1ed65` / 29,461,249 B）。

**提交前的安全核查（五条，逐条有输出）**：① `.gitignore` 的 14 条模式逐条 `grep` 到；
② 仓内 `find *.p12|*.keystore|*.jks|*.password` **0 命中**；③ 暂存清单里
`build/`、`.gradle/`、`engine/target`、`jniLibs/*.so`、`*.apk`、`*.idsig`、快照目录 **0 命中**
（`grep_rc=1`）；④ 秘密扫描（`storepass <值>` / `pass:<6+位>` / `PRIVATE KEY` / `ghp_` / `xox…`）
只命中两处**文档里的占位示例**（`PIN.md` 的 `-storepass A -keypass B` 与 `-storepass <口令>`）；
⑤ `host/scripts/sign-apk.sh` 不含口令，只从 `QINGSHAN_RELEASE_KEYSTORE_PASSWORD`
或 `<密钥库>.password` 首行读。

**抓到两个必须写下来的东西**：
1. **`engine/tools/__pycache__/verify_symbols.cpython-38.pyc` 被 `git add -A` 收了进来**——
   `.gitignore` 里没有 `__pycache__/`。已补两条模式（`__pycache__/`、`*.pyc`）并
   `git rm -r --cached` 重暂存，复查 `grep -c "pycache\|\.pyc"` = **0**，暂存数 580 → **579**。
   下次加新工具的缓存目录（`.pytest_cache/`、`.mypy_cache/`）要一起补。
2. **这台机器 `core.autocrlf=true`**，会把 `host/scripts/sign-apk.sh` 检出成 CRLF ⇒
   Git Bash 里 `bash sign-apk.sh` 因 `\r` 报错。新增 `.gitattributes`（`* text=auto` +
   `*.sh eol=lf` + `*.bat eol=crlf` + 按类型钉 LF + png/jar/so 标 binary），
   `git check-attr eol -- host/scripts/sign-apk.sh` 现回 **`lf`**。
   顺带量了工作树里真带 CRLF 的文本文件只有 **5 个**（`LICENSE-QINGJIAN`、
   `engine/src/keys.rs`/`logcat.rs`/`view.rs`、`host/assets/desktop-dictionary.lock.json`），
   **`host/java` 一个都没有** ⇒ 「Java 树逐字未改」这条不受归一化影响。
   但锁文件是 python `write_text` 在 Windows 上生成的，实测 167 B 里含 **12 个 CR** ⇒
   **LF 形式是 155 B**，所以文档里"锁 167 B"是**本机（Windows）形式**，
   Linux 上重建会得到 155 B、`.so` 也随之差 12 B。这条以前没人记过。

**两个提交**：
- `db5ad5a` `Initial commit: Qingshan IME (阶段一 + 阶段二 A/B/C + 纠错闭环 + 签名链)`
  ⇒ **579 files changed, 92,024 insertions(+)**；作者
  `romantic404TT <2721890995@qq.com>` —— 用的是这台机器**已有的全局 config，我没改任何 git 配置**。
  分布：`host/java` 394、`host/res` 145、`host/gradle-app` 10、`engine/src` 10、`engine/tools` 8、
  文档与许可 5、`host/scripts` 1、`host/assets` 1（只有锁）。
- 第二个提交：`UpdateJobService.java` 的更新入口开关（**11 增 1 删，宿主 Java 树第 3 处破例**）。
  形式上没照原话写 `return true`，而是 `IN_APP_UPDATES_DISABLED || 原判定` + 8 行注释块：
  直接 `return true` 会让 `installer()` 与 `PLAY_STORE` 变成死代码，而翻开关是一行回退
  （按"可逆优先于删除"那条既有裁定）。编译实测 `compileDebugJavaWithJavac` 退出码 0；
  **本轮没重打 APK**（APK mtime 仍 18:09:20）⇒ "药丸确实不显示"**未真机验**。

**push 由用户执行**（agent 不接触任何凭据）：
`git remote add origin <URL>` + `git push -u origin main`，URL 填好后把
`NOTICE.md` §5 与 `README.md`「构建前提」里那两处「⚠ 待填」一起换掉。

### 签名链与发布准备评估（2026-10-15 02:51–03:20，本轮主任务，**只评估与设计，没出 release 包**）

对应待办第 17 条的 ①②③④ 四件，逐件给结论 + 证据。本轮**没生成过 release 变体的 APK**、
没跑过 `assembleRelease`（Gradle 一次都没启动），签名探针只用**临时自造密钥重签现有 debug 包**，
探针与密钥同轮删除（见"探针"）。

**① 上游 signing.sh 的流程（读全，逐行）**：`msime-develop/platforms/android/scripts/signing.sh`
是一份被 source 的片段，只做两件事——设 `android_signing` 数组与 `android_signed_with` 标签。
取密钥顺序：`MSIME_ANDROID_RELEASE_KEYSTORE` + `MSIME_ANDROID_RELEASE_KEYSTORE_PASSWORD`
→ `~/.android/msime-release.p12`（口令可用 macOS 钥匙串条目 `msime-android-release-keystore`）
→ **两处都没有才退回 `dev-keystore.sh` 的开发密钥并打警告**。硬失败条件：有密钥文件却拿不到口令
（原话「有密钥文件却拿不到口令时直接失败，不悄悄换成别的证书」）。
正式版/beta/本地构建**共用同一把** `msime-release`（证书 `3a889e43…`），理由写在文件头：
Android 只允许同证书覆盖安装，任一渠道换证书 = 装过它的设备只能卸载重装、丢本地词库和设置。
CI 侧 `msime-develop/.github/workflows/release-android.yml:79-86` 把 secrets
`ANDROID_RELEASE_KEYSTORE_BASE64`/`ANDROID_RELEASE_KEYSTORE_PASSWORD` 解到 `$RUNNER_TEMP`，
`:85` 有 `trap 'rm -f "$MSIME_ANDROID_RELEASE_KEYSTORE"' EXIT`；
`:25` 解释了为什么这把钥匙不能有第三份：**APK 经 GitHub release 分发、靠应用内更新，
没有 Play App Signing 托底，泄露后无法更换，换了现有用户就收不到更新**。
`build-native.sh` 与签名无关（`grep -n "apksigner\|zipalign\|signing.sh\|keystore"` 零命中，
同一条 grep 对存在的文件跑正样例确认过命令本身有效）。
上游出包次序在 `msime-develop/platforms/android/build-apk.sh:176-185`：
`assembleRelease` → 取 `app-$edition-release-unsigned.apk` → `zipalign -P 16 4` →
`apksigner sign --out` → `apksigner verify --verbose --print-certs` → `zipalign -c -P 16 4`。

**② 要不要 keystore、走上游还是自己接**：

- **上游那把不能用**。它是 `app.msime.android` 的发布密钥，我们装机的是 `app.qingshan.ime`
  （`aapt dump badging` 实测 `package: name='app.qingshan.ime' versionCode='1000' versionName='0.1.0'`），
  而且密钥本身不在我们手上。**结论：青杉必须新造一把自己的发布密钥**，
  并按 `signing.sh` 文件头那条理由**从此固定**——一旦发出第一个 release 包就再也换不掉。
- **现状实测**：当前装机包是 `CN=Android Debug`，`apksigner verify` 退出码 0，
  **v1 false / v2 true / v3 false**，证书 SHA-256 `0686eb3acfd13e15783e6b7dbaee7767d751a92c10f88769c66376866ade493a`。
  密钥文件 = `D:\toolchains\android-sdk\.android\debug.keystore`（`env.sh:25` 把
  `ANDROID_SDK_HOME` 指到 SDK 根，所以它不在 `~/.android`），
  魔数实测 `30 82 0a 36` ⇒ **PKCS12，不是 JKS**（JKS 是 `fe ed fe ed`）。
- **走上游脚本 = 不做**。上游那份依赖 macOS `security`（本机 `command -v security` 无）、
  依赖无扩展名的 `"$tools_dir/apksigner"`（本机实测 `No such file or directory`，exit 127），
  还带着 6 个 edition 与钥匙串的逻辑，我们没有一样用得上。
  **建议新写一份本项目的 `host/scripts/sign-apk.sh`**（`host/scripts/` 现在是空目录，实测 `ls` 零项），
  只做上游那四步次序，密钥只从环境变量取、**永不进仓**。
- **不建议把 `signingConfig` 写进 Gradle**（`qingshan/host/gradle-app/app/build.gradle.kts:67-73` 的
  release 块现在只有 `isMinifyEnabled = false`，:71 注释就是"签名链留到发布阶段"）：
  写了就要把密钥路径/口令暴露进构建配置或 `gradle.properties`，与"上游由脚本事后签名"同形的做法更安全。
- **换证书的代价**：证书不同 ⇒ `install -r` 必失败，现在这台 `e1cdb946` 第一次装发布包要先卸载
  （丢本地词库与设置）。当前只有一台开发机，**现在换比以后换便宜**。

**③ 可执行步骤与产物路径（本轮一律未执行）**：

1. 造密钥（一次，离线保管）：`keytool -genkeypair -storetype PKCS12 -alias qingshan-release -keyalg RSA
   -keysize 3072 -validity 10000 -dname "CN=Qingshan IME Release" -keystore <文件> -storepass <口令>`。
   **PKCS12 会静默忽略 `-keypass`**（见探针第 1 条），所以密钥口令 == 库口令，别指望分开设。
2. 出包：`./gradlew --project-dir qingshan/host/gradle-app assembleRelease`
   （本工程自带 `gradlew`，不需要上游那个 Tauri wrapper）。
   产物按 AGP 惯例应是 `qingshan/host/gradle-app/app/build/outputs/apk/release/app-release-unsigned.apk`
   ——**文件名与"lint NewApi 门禁在 assembleRelease 里到底跑不跑"两条都未实测**，
   是下一轮出第一个包时要先看的东西。
3. `zipalign -P 16 4 <unsigned> <aligned>` → 4. `apksigner sign --ks … --ks-key-alias qingshan-release
   --ks-pass env:… --key-pass env:… --out <signed> <aligned>` → 5. `apksigner verify --verbose --print-certs`
   → 6. `zipalign -c -P 16 4 <signed>`（第 6 步本轮在 debug 包上实测退出码 0，可当正样例）。
7. 复跑 `engine/tools/verify_apk.py`（它按参数取 APK 路径，换成 release 路径即可）。
8. 产物命名建议 `qingshan-android-<version>.apk`，**同目录必须放 `<apk>.sha256`**——
   宿主 `qingshan/host/java/app/msime/android/account/UpdateApi.java:36` 写明
   「没有校验文件的发布一律不装」。
9. 密钥备份与 CI：本项目**没有 CI**（`find qingshan -maxdepth 2 -name ".github" -o -name "*.yml"` 零命中），
   所以密钥先落在本机 + 立刻做一份加密离线副本；将来接 CI 再照 yml:79-86 的 secrets + `trap rm` 抄。

**探针实测的两条新陷阱（都是本轮造的可复现证据，不是文档转述）**：

1. **PKCS12 的 keypass 被静默忽略**：`keytool` 生成时只出
   `警告: PKCS12 密钥库不支持其他存储和密钥口令。正在忽略用户指定的-keypass值。`
   且 **exit 0、密钥照样生成**（注意 keytool 在中文 Windows 上是 GBK 输出，得 `iconv -f GBK` 才读得清）。
   随后 `apksigner sign` 用那个"不同的 key-pass" ⇒ **exit 2**，原文
   `Failed to load signer "signer #1"` / `java.io.IOException: Failed to obtain key with alias "probe" from probe.p12. Wrong password?`。
   ⇒ 脚本里两个口令要么写同一个值，要么干脆不传 `--key-pass`（apksigner 会退回库口令）。
2. **zipalign 必须在签名之前**：对已签名的包再跑 `zipalign -P 16 4` ⇒ `apksigner verify` **exit 1**，
   `DOES NOT VERIFY` + `JAR signature META-INF/PROBE.SF indicates the APK is signed using APK Signature Scheme v2 but no such signature was found. Signature stripped?`。
   反过来"签名前不 align"在 Gradle 产物上看不出问题（Gradle 已对齐），**所以这条错只在手搓包时才会撞上**。
   附带一条：`apksigner --out` 会在旁边留一个 `<out>.idsig`（探针实测 398,691 B），
   上游靠 `yml:93` 只 `cp "$apk_name.apk"` 规避，我们的脚本得自己别把它当产物传。
   既有两条继续有效：`apksigner` 是 `.bat`、`$JAVA_HOME` 没 source 时报"文件不存在"；
   `apksigner verify --verbose 1 <apk>` 会把 `1` 当 APK 路径（环境陷阱第 11 条）。

**探针与还原**：临时密钥 `a3b7/probe.p12` 与 `probe-in/b/c/d.apk`、`.idsig`、各 log
都在本轮 `rm` 掉并复验（`ls | grep -i probe` 无输出），仓库里 `find` 过 `*.p12|*.keystore|*.jks` **零命中**。
探针用的是 2048-bit 临时 RSA，重签后包体 49,896,069 → 50,001,372 B（**+105,303 B**，
这个增量只适用于探针证书，正式 3072-bit 会略有不同）。

**④ GPL-3.0 §5a 与版本策略**：

- **声明本身齐了**（`qingshan/NOTICE.md:22-51` 已列 Java 4 行改动、未取的两个 `.kt`、
  applicationId、独立 Gradle 工程、不链接水杉引擎），**但"分发"这一半没落地**：
  `qingshan/NOTICE.md:24` 说"本仓库即完整对应源码"，而实测 **`projects/qingshan` 没有 `.git`**
  （只有 `.gitignore`），上游两个拷贝目录也没有 ⇒ 现在这份"仓库"没有任何可公开访问的位置。
  发出去一个 APK 就要有可访问的对应源码（§5a 的"网络服务器"或随包/明示 offers），
  **这是发布前必须补的一条，且要先补 README 的 release 构建步骤**：
  `qingshan/NOTICE.md:26-28` 目前只写了 debug 产物路径 `app-debug.apk`，
  改口要连 release 的 zipalign/apksigner 次序一起写清（§5 要的是"用于生成与重新组合该份代码的脚本"）。
  **不要为了省事把私钥当"安装信息"交出去**——密钥不在对应源码的定义里，公开它等于永久放弃更新能力。
- **发布阻塞项（本轮读码新发现，属功能问题不是安全问题）**：宿主的"检查更新"
  写死了水杉的发行地址：`qingshan/host/java/app/msime/android/account/UpdateApi.java:45` `https://msime.app/api/releases?platform=android`、
  `:47` `https://github.com/metasequoiaime/msime/releases/download/`、`:49` 镜像 `dl.msime.app`，
  edition 取 `full`（`qingshan/host/gradle-app/app/build.gradle.kts:48`）。而 `qingshan/host/java/app/msime/android/home/AboutPage.java:52`
  说明这三项只在非 Play 安装时显示 ⇒ 我们的包**会显示"检查更新"**，点了会去下水杉的 `msime-android.apk`。
  **不会装错**：`qingshan/host/java/app/msime/android/account/UpdateApi.java:404` 安装前核对"包名与本应用相同 + 版本号更高 + 签名证书与当前一致"，
  `app.msime.android ≠ app.qingshan.ime` 第一道就挡掉。真实后果是
  **"提示有新版 → 下载几十 MB → 拒绝安装"的死循环**。三条路：① 指向青杉自己的仓库；
  ② 关掉这三项入口；③ 先只发不经应用内更新的包。**本轮没在真机点过这个按钮**（可达性未验，
  它是纯 Java `HttpURLConnection`、不经桩，所以大概率活着）。
- **版本策略**：现值 `0.1.0` / `1000`（实测），公式与上游同形
  （`qingshan/host/gradle-app/app/build.gradle.kts:34`），上游 `version.txt` 是 `0.3.1`（=3001）。
  建议：① **debug 与 release 共用同一个 `host/version.txt`**，不另开号；
  ② `versionCode` 只增不减，第一个发布包定 `0.1.0`=1000 之后，任何回退都发不出去；
  ③ 渠道后缀走 tag/release 名，`versionName` 保持纯 `X.Y.Z`（上游 yml:38 就是这么分的：
  main 无后缀、develop `-beta`、其他 `-alpha`，带后缀的按 prerelease 发布、应用内更新会自动跳过）；
  ④ 与水杉 `0.3.1` 的高低关系不必处理——包名不同、可共存，但如果将来要迁移动水杉用户，
  这个号必须重排，属产品决定。
- `prd.md:821` 那行人工/法务审查**继续独立**，不并进上面任何技术项（你原话）。


### refreshHostRaw：方案① 立项，排下一轮

**目标**：关掉「已知失效项」第 8 条（升级不重解包）。做法 = 把 `(name, bytes)` 烘进 `.so`，
`refreshHostRaw` 只 `stat` 资源目录里锁列的文件比长度，不符就回 `dictionary_outdated:`。
零新依赖、约 80 行、不动 Java。细节见上一节 a–g。

**新纪律（必须同时写进 README 构建步骤）**：锁烘在二进制里 ⇒ **每次改词库都要重编 `.so`**，
否则二进制里的锁还是旧的，每次启动都误报 outdated、每次重解包 4.2 MB。
今天 A3 落表只需重打 APK，立项后这条耦合就成立。

**验收**：`install -r` 换一份**不同长度**的表，**不 `pm clear`**，看
`run-as … ls -l files/bootstrap/resources/` 的字节数有没有跟着变。

**方案②（长度 + sha256）不做**，三条理由：等长更新在我们流程里概率极低（每次改表长度都变）；
多约 74 ms/启动（实测单次采样）；要引入 `sha2` 新依赖。等真出现等长更新再加。

**排期**：A3 扩到 1,000 条并装机验证之后，**独立一轮做**，不与 A3 扩量混。

### refreshHostRaw ① 落地（2026-10-13 20:44，真机两侧都验过）

**做法**（与上面立项一致，零 Java 改动、零新依赖）：`engine/tools/build_glossary_subset.py`
生成表时顺手把 `desktop-dictionary.lock.json` 重写成 `{"artifacts":[{"name":…,"bytes":…}]}`；
`engine/src/resources_lock.rs` 用 `include_str!` 把这份锁烘进 `.so`，`refreshHostRaw` 读
runtime-options.json 的 `resources`，**只 stat 锁里列的那几个文件比长度**。
宿主 `Bootstrap.java:278` 只取 `getString("name")`，多出来的 `bytes` 它不看——这就是不改一行
Java 能成立的原因。`helpcodes/` 与日文那一组（`Bootstrap.java:246-249`）本来就不在锁里，
不需要在这里判豁免。

**分档**（单测 7 条，全在 `resources_lock.rs`，宿主 `cargo test` 跑得到）：

| 情况 | 结论 | 错误文本 |
|:---|:---|:---|
| 长度全等 | `Ready` | 无（回 `ok:true`） |
| 长度不符 | `Outdated` | `dictionary_outdated: <名>:length-<实>-want-<锁>` |
| 文件缺失 / 不是普通文件 | `Outdated` | `dictionary_outdated: <名>:missing` / `:not-a-file` |
| 配置不存在 / 不是普通文件 / 超 1 MiB / 非法 JSON / 无 `resources` | `Blocked` | `refresh_options_*`，**不带过期前缀** |
| 锁本身缺 `bytes`（老格式） | `Blocked` | `refresh_lock_artifact_without_bytes` |

第四档是刻意的：上游 `outdated_resources`（`crates/host-api/src/lib.rs:1629-1640`）只把
`Integrity` 与 `ExistingGeneration` 映射成 `dictionary_outdated`，注释写明「读不出的文件与
非法的编译期锁不是重新下载能修好的」。若这里也报过期，一份坏配置会让每次启动都重解包 4.2 MB。

**真机验收**（`e1cdb946`，全程不 `pm clear`）：

| 步骤 | 解包件 `glossary-en.tsv` | 日志 |
|:---|---:|:---|
| 起点（16:39 那次 `pm clear` 之后） | 2,150,622 B | — |
| 装「表更长 +28 B」的包，冷启动 | **2,150,650 B**（20:38） | `dictionary_outdated` 各 1 行 |
| 装回原包，冷启动 | **2,150,622 B**（20:44） | `[msime_android::wire] 回 ok:false: dictionary_outdated: glossary-en.tsv:length-2150650-want-2150622` + `MSIMEBootstrap: Runtime options refresh failed: dictionary_outdated` |
| 什么都不改，再冷启动一次 | 2,150,622 B | **两条都 0 行**（不误报、不重复解包） |

**我自己撞的第一个坑：只重打 APK、没重编 `.so`。** 第一次装「更长的包」时设备**没有**重解包，
我一度以为实现错了。查产物才看清顺序：`.so` 20:31:27 编、锁与表 20:33:39 才改，
`grep -a -c 2150650` 在 `.so` 里 **0 命中**、`2150622` 1 命中 ⇒ 二进制里烘的是**旧锁**，
它与设备上那份旧表长度相等，所以判 `Ready` 完全正确。⇒ README 构建步骤那条新纪律不是抽象要求，
**这就是它的实测后果**。重跑 `cargo ndk` 后同一套流程立刻按预期工作（cargo 会顺着
`include_str!` 记依赖，改锁不需要手动 touch 源文件——实测 `.so` 磁盘大小 arm64
4,341,128 → 4,349,376 B、x86_64 4,583,272 → 4,595,232 B，同口径 `ls -l` 相减）。

**第二个坑：`refreshHostRaw` 可能就是本进程的第一个桥调用。** 日志桥原先只挂在
`prepareHostRaw`/`createRaw`（`logcat.rs:27` 的注释还写着「这两点覆盖了全部入口」）。升级路径
既不走 prepare 也没有会话，所以第一次验收时宿主那行 `Runtime options refresh failed` 有、
**我们自己的原因行 0 行**——`tracing` 没 subscriber，`warn!` 是静默 no-op。补了
`logcat::init()` 到 `refreshHostRaw`（幂等），并把 `logcat.rs` 两处注释改成三个点。
上表 20:44 那行的原因就是补完之后才第一次看见的。

**顺带一次自误（记下来防再犯）**：我用一段 python 批量改注释，读文件时用了通用换行模式，
把 **CRLF 写成了 LF**——`view.rs` 有快照（`view.rs.before-probe` CR=656）能证明并已从快照还原
（还原后 diff 只剩 2 行注释）；`wire.rs` 本轮没存快照，**它原始行尾无法判定**，只能如实记着。
本仓 `.rs` 行尾本来就是混的（实测 CR 计数：keys/logcat/view 有 CR，assemble/session/stub/lib 无）。
**规矩**：批量改文件要么用 Edit 工具，要么 `io.open(..., newline="")` 读写两端都保留行尾；
改注释之前先存快照，否则连"我只改了两行"都证明不了。

**计数同步**：真实现 **17** + 桩 **85** = 102（`gen_stub.py` 打印：`REAL=17 生成 stub=85 合计 102`，
`verify_symbols.py` 两 ABI 均「导出 JNI=102 缺失=0 多余=0」）；`cargo test --release` **39 passed**
（32 + 本模块 7；现值 43，2026-10-14 加了 4 条落盘用例，见「flush_learning 落盘」一节）；`cargo fmt --check` **17 hunk**，全在 `lib.rs`——16 个是刻意保留的旧紧凑签名，
第 17 个是 `refreshHostRaw` 沿用了同一写法（**没有**顺手格式化旧的）；`resources_lock.rs` 自己
按 rustfmt 结果写，0 hunk。`scan_bridges.py --summary` 与基线逐项相同（杀会话 5 / 降级 32 /
不解析 1 / 需人工定性 1 / real 23），因为 `refreshHostRaw` 的调用点在 `Bootstrap.java`，
不在 `scan_bridges.py` 扫的 `MSIMEInputService.java` 里——它的异常由 `Bootstrap.java:379`
的 `catch (Exception | LinkageError)` 只记一条 `Log.w`，不碰会话。

## 与 PRD 的冲突（待产品复核）


> PRD §七 风险表（`prd.md:841`，章节标题在 `:827`「七、风险与缓解」）：
> 「使用 `.qj` 压缩容器，按需加载；考虑首次启动时下载可选语言模型」。
> 实测事实：`.qj` 对词库反而更大（26 MB → 46 MB），机制见「上游性能与体积事实」第 2 条。
> PRD 这条对词库不成立，`.qj` 的收益是加载耗时（240 → 12 ms）不是体积。
> 结论：阶段一按 TSV 交付；阶段二若引入语言模型再评估 .qj。

> **撤销窗口只到进程存活期（2026-10-15 用户裁定接受，不改 PRD 原文）**：PRD 1.3「错选可撤销」
> 的直觉是"随时能撤"，现状是方案甲-2 按包名挂起 `EngineSession`，进程被杀 / 冷启动后
> `recent_commits` 丢失、无撤销入口。理由：跨冷启动要把账本序列化，Core 既没给 `Serialize`
> 也没给恢复 API（越界，已排除）；甲-2 是不改 Core 的前提下唯一干净的可达方案。
> 这是产品决定，写清楚即可（README「已知失效项」第 10 条同步）。

## 横屏候选条调查（2026-10-15 23:05–2026-10-16 00:14，只读 + 真机，代码净改动 0）

用户报的现象：**横屏时候选条整条不显示**（待办第 21 条、环境陷阱第 47 条那次撞见的）。本轮按
「试用页 / 第三方输入框 / 读码 / 与上游比对」四步查。结论先说：**横屏本身不坏，候选条在横屏
正常出；但 20:35 那次"0 候选"本轮没能复现，它的成因仍未知；顺着这条线量到两条真缺陷。**

### 〇、先记一次自我撤回：我自己的计数器写坏了，差点把假结论写成"已定位"

本轮中途我一度宣布"已证伪：`input text` 不建组合，所以候选条空与方向无关"，并把它写进了
文档。**那个证伪本身是错的，全部撤回。** 错因是我这轮新写的计数函数：

```python
cand=[n for n in ns if n['cls']=='Button' and '你' in n['text']]   # 错
```

`uiautomator` 的 `class` 属性是**全限定名** `android.widget.Button`，等值比较 `'Button'` **恒假**
⇒ 这一支永远数出 0 条，看起来"完美印证"了假设。改成 `n['cls'].endswith('Button')` 之后，
同一批 dump 立刻数出 6–10 条。**⇒ 新记环境陷阱第 48 条：计数器必须先跑正样例。**
下表是**用修正后的计数器重跑**的结果，每格都打出了候选词原文，不是光秃秃的 0。

### 一、真机实测（OPPO PEGT10 / Android 13 / 1080×2400 / arm64，包 `app.qingshan.ime`）

| # | 场景 | 探针 | 候选条 | 证据 |
|---|---|---|---|---|
| 1 | 试用页**横屏**、键位逐字点 `nihao` | 屏幕键位 | **正常 10 条** + `1/5` + 展开键 | `a3bm/L4.xml`、`L5.png` |
| 2 | 系统设置搜索框**横屏**、键位逐字点 | 屏幕键位 | **正常 10 条** | `a3bm/S4.png` |
| 3 | 试用页**竖屏**、`input text nihao` | `input text` | **6 条**（`你好/你好好/你好像/你好久`…） | 本轮 2×2 矩阵 |
| 4 | 试用页**横屏**、`input text nihao` | `input text` | **10 条**（同上多 4 条） | 本轮 2×2 矩阵 |

⇒ **`input text` 会建组合**（第 3、4 格都是候选照常出现），我上面那个"不建组合"的说法作废。
⇒ 横屏 + 竖屏、两种探针，**四格全部正常**，"横屏候选条不显示"在本机本轮口径下**复现不出来**。
20:35 那次的 0 候选**仍未定位**，剩下两个没排除的差别：① 当时是**真实传感器旋转**，本轮是
`cmd window user-rotation lock 1` + `fixed-to-user-rotation enabled` 强转（见第 49 条），
两者给 IME 窗口的度量未必一致；② 当时是 40 条预检脚本**按竖屏坐标表**去点横屏键位，
点空 ⇒ 根本没建组合（这条最像，但没证据，属推测）。

### 二、读码：候选条的可见性里**没有方向这一维**

- 候选条不是 `onCreateCandidatesView` 那条独立窗口，是在 `onCreateInputView()` 里纯代码搭的：
  `qingshan/host/java/app/msime/android/core/MSIMEInputService.java:6859`（`candidates` 那一行）、
  `:6871`（`imeToolbar.addCandidateLine(...)`）。全仓 grep `onCreateCandidatesView` **0 命中**。
- 三行可见性判定（同一文件）：`qingshan/host/java/app/msime/android/core/MSIMEInputService.java:7818`
  工具栏行 `= !heightMode && idle && !hasDiagnostic && (...)`、`:7832` 候选行
  `= !heightMode && !idle && !hasDiagnostic`、`:7836` 候选视口 `= !idle && !hasDiagnostic`。
  三个式子里**都没有 orientation**。
- 全仓与方向相关的分支只有两处，都不碰候选条：
  `qingshan/host/java/app/msime/android/core/MSIMEInputService.java:3549`（分离键盘）、
  `qingshan/host/java/app/msime/android/core/ImeBottomBar.java:186`（底部留白）。
- **`layout-land` / `values-land` 在本仓和上游都不存在**：
  `find msime-develop/platforms/android/res qingshan/host/res -maxdepth 1 -type d | grep -c land` → **0**
  （构建产物 `build/intermediates/.../merged.dir/values-land` 是 androidx 依赖带的，不算源）。
- 候选行高度与方向无关：`qingshan/host/java/app/msime/android/core/ImeToolbar.java:22`
  `CANDIDATE_LINE_DP = 42`，由 `updateCandidateViewportHeight()` 一处算，不读 orientation。

### 三、两条真缺陷

**① 旋转把组合冲掉。** 竖屏组合中（preedit `nihao` 在、候选条在）→ 锁到横屏 → preedit 节点消失、
候选条消失，且字母**原样落进输入框**
（`a3bm/Y4.xml` 里 `tryout_field` 的文本是 `nihaonihao`，即组合那串字母原样进了框；
本轮框里本来就有测试残留，所以**只报"多出原样字母"这一件事，不报起始值**）。
logcat 同一时刻出现一整条会话建立：
`[msime_android::session] 会话建立 125 ms（含词库载入），挂起键=Some("app.qingshan.ime")，换回挂起上下文=true`
（`qingshan/engine/src/session.rs:467` 那句日志），词库 58 ms + 译词表 67 ms 重新载入一遍；
`dumpsys window` 里 IME 窗口 `req=2400x794` → `req=2400x683`（差 111 px ≈ 标题行 34dp + 候选行 42dp）。
调用链：`qingshan/host/java/app/msime/android/core/MSIMEInputService.java:1700` 的
`NativeClient.create(...)` 由 `startEngineSession` 发出，`scheduleEngineStartup`（同文件 `:1629`）
在输入会话重启时再走一次 ⇒ 新会话没有组字状态，`idle=true`，候选行按 `:7832` 隐藏，**这是正确行为**。
**未定**：上游水杉在旋转时是否也重建会话（那段 Java 是上游原样代码，我们的改动只有
`:1698` 那行 `options.put("application", currentEditorPackage)`）。要定它必须装上游 APK 做对照，
本机装不了（包名同为 `app.msime.android` 系 + 证书不同，见环境陷阱第 46 条）。
但**词库每次重建都重解 125 ms 这一半是我们自己的**——上游 C++ 引擎有没有跨会话缓存未核实。

**② 偏好读取恒失败，连带候选条在任何方向都会消失 4 秒。** 键盘上那句
`设置读取或应用失败，保留当前设置` 不是偶发：它的源头 `loadPreferencesRaw` 还躺在
`qingshan/engine/src/stub.rs:100` 的 85 个占位里，回 `{"ok":false,"error":"not_implemented"}`
→ `qingshan/host/java/app/msime/android/core/NativeClient.java:66` 原样交给宿主 →
`qingshan/host/java/app/msime/android/core/MSIMEInputService.java:2145` 的 `value(response)`
（同文件 `:844`：`ok` 不为真就 `throw new JSONException("Shared runtime rejected operation")`）抛出去
→ `:2147` 的 catch 必进 → `:2149` 置 `preferencesNotice`，永久挂在那儿。
**（此处 2026-10-16 00:41 更正一次：上一版我写的是"`PreferencesReloader.java:39` 的 catch 把 result 置 null
⇒ `reloadPreferences(null)` 抛 `Preferences unavailable`"，那是推的、不成立——桩返回的是合法 JSON 字符串，
不抛也不为 null。真抛点是 `:2145` 的 `value()`。结论不变，机制以此为准，详见「loadPreferences 真实现评估」一节第 〇 条。）**
空闲渲染那一趟
（同文件 `:7813` `if (idle && !modeLabel) announceIdleNotice(preferencesNotice);`）会把它
经 `notice()` 提升成诊断行，`hasDiagnostic`（`:7804`）一真，`:7818` 与 `:7832` **同时**为假
⇒ **工具栏行和候选行一起没了**，4 秒后自动恢复（`qingshan/host/java/app/msime/android/policy/InputDiagnosticPolicy.java:5`
`DISMISS_DELAY_MILLIS = 4_000L`）。去重靠 `announcedIdleNotice`（同文件 `:468`/`:473`），
所以每次进程存活期内只炸一次，但**炸的时候正好覆盖"旋转完接着打字"这个动作**，
这就是用户眼里"横屏候选条不显示"最可能的样子。

### 四、与上游比对：这套逻辑我们一行都没动

`diff -rq msime-develop/platforms/android/java qingshan/host/java` 实测 **4 行**：
`MSIMEInputService.java` 与 `UpdateJobService.java` differ，`AccountPlugin.kt`、`VoicePlugin.kt`
仅上游有。候选条可见性那四句在两边**逐字一致**，只是行号差 2（我们前面多了一个 2 行的 hunk）：

| 语句 | 上游行 | 本仓行 |
|---|---|---|
| `boolean toolbarRow = !heightMode && idle && !hasDiagnostic && (...)` | 7816 | 7818 |
| `ViewPolicy.setVisible(candidateLine, !heightMode && !idle && !hasDiagnostic)` | 7830 | 7832 |
| `preferencesNotice = " · 设置读取或应用失败，保留当前设置"` | 2147 | 2149 |
| `if (idle && !modeLabel) announceIdleNotice(preferencesNotice)` | 7811 | 7813 |

⇒ **分类结论：不是"试用页独有"，不是"全平台普遍"，也不是"适配层缺口造成的横屏渲染问题"。**
候选条的显示规则是**上游已知**代码；本仓的**适配层缺口**只有第 ② 条那一项（`loadPreferences`
是 stub），它造成的现象与方向无关；第 ① 条（旋转丢组合）是上游会话生命周期 + 我们引擎无跨会话
缓存的叠加，上游那半**未核实**。

### 五、下一轮要看得更准，还缺这些观测

1. 上游 APK 的旋转对照（要先解决同包名共存：换 `applicationId` 或找第二台机）。
2. 用户侧的复现口径：他是在**哪个应用**、**打字中途转屏**还是**转屏后再打字**看到的——
   前者指向第 ① 条，后者若发生在诊断那 4 秒内指向第 ② 条。
3. 九键 / 手写面板横屏未测（本轮只测了 26 键拼音）。
4. 分离键盘（`:3549` 那条方向分支）横屏未测——它是唯一一处"横屏会换画法"的代码。

## loadPreferences 真实现评估（2026-10-16 00:41，只读，代码净改动 0）

用户给的门槛：**< 100 行且不破 Java 零改动就直接做，否则停下出方案。** 结论先说：
**≥ 100 行，且"只实现 load"根本消不掉那句提示 ⇒ 本轮不实现，方案如下。**

### 〇、先更正上一轮我自己写错的一条因果

上一轮「横屏候选条调查」第 ② 条里我写的是：桩回 `{"ok":false,…}` → `PreferencesReloader.java:39`
那个 `catch` 把 `result` 置 null → `reloadPreferences(null)` 里 `throw new JSONException("Preferences unavailable")`。
**这条链是推的，不是测的，中间那步不成立**：桩返回的是一个**合法的 JSON 字符串**（不是 null、也不抛），
所以 `reader.read()` 正常返回、`result` 非 null，`"Preferences unavailable"` 那一支**走不到**。
真正抛的是下一行：`qingshan/host/java/app/msime/android/core/MSIMEInputService.java:2145` 的
`JSONObject snapshot = value(response);`，而 `value()`（同文件 `:844`）是

```java
        if (!JsonPolicy.strictTrue(envelope.opt("ok")))
            throw new JSONException("Shared runtime rejected operation");
```

⇒ 抛的是 **"Shared runtime rejected operation"**，`catch` 在 `:2147`、置提示在 `:2149`。
结论不变（那句提示确实是 stub 造成的），**但机制要按这条改**，两处已同步更正。

### 一、偏好这条路上有**五个** native，全是桩

`qingshan/engine/src/stub.rs` 实测命中（行号是本轮 grep 的）：`defaultPreferencesRaw` 84 行、
`loadPreferencesRaw` 100 行、`restoreDefaultPreferencesRaw` 120 行、`savePreferencesRaw` 121 行、
`updatePreferencesRaw` 141 行。宿主侧对应五个包装（`.../core/NativeClient.java:66`、`:85`、`:90`、`:280`、`:592`）。

### 二、为什么"只做 load"消不掉那句提示（这是本轮最值钱的一条）

`applyPreferencesSnapshot` 的签名是 `throws JSONException`，而它**自己内部又要过一次 native**：
`qingshan/host/java/app/msime/android/core/MSIMEInputService.java:2260`

```java
        JSONObject result = value(NativeClient.updatePreferences(session, sessionSnapshot.toString()));
```

`:2332` 紧接着 `JSONObject nextView = result.getJSONObject("view");`。这一整段都在
`:2143` 那个 `try` 里。⇒ **load 就算返回合法文档，`updatePreferences` 仍是桩 ⇒ `value()` 照样抛
⇒ 同一个 catch ⇒ 同一句「设置读取或应用失败，保留当前设置」。** 只做 load 是**假修复**。

### 三、契约（真实现必须回什么）

- 信封：`{"ok":true,"value":{…}}`（`wire::ok_value` 已是这套，见既有 17 个真实现）。
- `loadPreferences` 的 value：**`{revision: >=0 的整数, preferences: {…}}`**。
  `revision` 缺失/负数直接抛（`PreferencesRevisionPolicy.read(accepted.opt("revision"), -1)`，
  `qingshan/host/java/app/msime/android/core/MSIMEInputService.java:2183`）；
  `preferences` 是**严格** `getJSONObject`（同文件 `:2186`），但**给了 `{}` 就够**：
  我逐行数过 `applyPreferencesSnapshot` 里对快照的取字段方式，**除 `preferences` 那一次之外
  全是 `optBoolean/optJSONObject/strictInt(…, 默认值)`**，缺字段一律落宿主自带默认。
- `updatePreferences` 的 value：`{view: {…}}`，`view` 用现成的 `build_view`（`qingshan/engine/src/lib.rs:427`）
  就能造 ⇒ 与候选长按五件同一套「ok:true + 原样 View」止血形态，**有先例、有用户裁定在前**。
- `savePreferences(directory, expectedRevision, snapshot)`：宿主**传整份文档**，共 5 个调用点
  （`:4912`、`:5075`、`:5497`、`:5641`、`:7312`）。⇒ 存储侧可以把文档**当不透明 JSON 原样存取**，
  不需要在 Rust 里建模那几十个字。`expectedRevision` 是比较并交换（上游同语义）。
- 数据来源：**不是** `runtime-options.json`，是宿主给的 `preferencesDirectory`
  （`qingshan/host/java/app/msime/android/core/MSIMEInputService.java:1722` 赋值），
  就是我们已经在写的 `files/` 之下——`engine/src/assemble.rs:60` 的 `DataPaths` 里加一个路径字段即可，
  原子写 Core 侧已有 `write_atomic`（`session.rs:62` 注释里那条落盘路径就在用它）。
- 上游参照：`msime-develop/crates/client-core/src/preferences.rs` —— **3,432 行**，
  含文件锁、修订号 CAS、损坏修复、原子写、版本迁移、`Preferences` 全量建模。
  **我们不需要照搬**，但"上游那份有多重"这件事本身就是范围证据。

### 四、范围量级（按本仓既有真实现的体量校准：JNI 导出层 13–36 行，重活在 session/view/wire）

| 切片 | 内容 | 估行（Rust，含 1–2 条单测） | 破 Java？ |
|---|---|---|---|
| 甲 | 只做 load | 40–55 | 否 |
| 乙 | **load + update（止血形态）** | **85–110** | 否 |
| 丙 | 乙 + save（修订号 CAS + 原子写）⇒ 设置**真能存住** | **145–175** | 否 |
| 丁 | 丙 + default / restoreDefault（「恢复默认设置」那颗按钮） | 200–240 | 否 |

⇒ **甲单独做是假修复（第二节已证）；乙是"消掉提示"的最小真实切片，已经踩在 100 行门槛上；
丙才是把功能修好。** 按用户门槛，乙/丙都**不满足"< 100 行"**，所以本轮**只出方案不动手**。

### 五、附带问题：做完解锁 D 路线吗？—— **解锁，而且是白送的**

PIN 第 1411 行那条"D 挂到偏好读写 P1 之后"的判据本轮读准了：门是
`qingshan/host/java/app/msime/android/core/MSIMEInputService.java:2221`

```java
        boolean nextCandidateGloss = preferences.optBoolean("candidate_english_gloss", true);
```

**默认值是 `true`**。今天它恒假，只是因为 `applyPreferencesSnapshot` 在 `:2260` 就抛了、
永远走不到 `:2295` 的赋值（复位路径 `:1580` 把它写死 `false`）。
⇒ **只要切片乙落地（load + update 都返回合法信封），`candidateEnglishGloss` 立刻变 true，
D 路线的门自己开**，不需要额外实现。这条要在动手那天写进 README 的 D 待办里。

### 六、建议的落地次序（等用户拍）

1. 先拍**丙**（load + update + save，约 150–175 行）：一次把"提示消失 + 设置真存住"做完，
   顺带白送 D 的门。分两步提交：先乙（消提示）、再 save（存设置），每步都能单独真机验收。
2. **丁**（default / restoreDefault）另开一轮——它要一份"默认文档"，而那份文档的字段清单
   只能从上游 `Preferences::default()` 抄，抄错就是静默改用户设置，风险与前三片不同级。
3. 动手那轮必须带的对照验收：改前"提示 4 秒内候选行被隐藏"这一条要**复现一次再修一次**，
   否则无法证明修的是它（上轮只读到规则，没量到 4 秒的现场）。

## 待办

分发声明见仓库根的 `NOTICE.md`（上游与固定依据、GPL-3.0 §5a 修改清单、随包数据来源与许可、
运行时第三方组件）。发布前仍未了结的是 `prd.md:821` 那行「许可证合规审查」——那是人工/法务动作，
本仓库能自证的只到 NOTICE 与两份 LICENSE 齐全为止。

`## 环境陷阱` 第 8 条起内容偏「上游事实」而非「陷阱」；若再加第 9、10 条仍属事实类，就把第 8 条及以后整体挪到 `## 上游性能与体积事实` 或单开一节。一次整理，不要零散挪。

**排队（2026-10-13 18:10 立；20:50 第 1 项落地；23:45 第 3 项落地；2026-10-14 00:54 第 5 项落地）**：

1. ~~**refreshHostRaw 方案①**~~ **已落地（2026-10-13 20:44）**：做法、四种分档、真机两侧验收
   与我撞的两个坑，全在上面「refreshHostRaw ① 落地」一节。「已知失效项」第 8 条已关闭；
   新纪律「改词库就要重编 `.so`」已写进 README 构建步骤（并且有实测后果作证）。
2. ~~量 A3 的「可达率」~~ **已完成（2026-10-13 18:07）**：观测口 = ① 临时探针（② 结构可行但无出口，
   理由见「可达率测量」一节），结果 **18/23 = 78.3%**，探针已还原、干净版已装机复验。
   下一件事不是"再测"，是等用户对 A3 扩量拍板。
3. ~~A3 第 4 批待拍板~~ **已按 ② 全落 520 条（2026-10-13 23:37）**：表 89,828 行、
   文件 2,167,528 B、APK 49,843,905 B，`.so` 已重编、`install -r` 不 `pm clear` 自动重解包，
   抽样 7/8 露出且位次全 0/1。全过程与二值行为确认见「A3 第 4 批落表」一节。
4. **下一批 A3（若继续）不再开观测口**：按二值行为规矩用「候选条前 5 格」代理指标预检即可；
   某批出现位次 ≥ 2 就停下重评。方案②（长度+sha256）仍等真出现等长更新再说。
5. ~~flush_learning 落盘~~ **已落地（2026-10-14 00:54）**：方案 2 =「会话销毁必落 + 上屏满 60 秒
   节流落」，`host/java` 零改动；真机三张表、跨冷启动累加、写盘 0-9 ms、force-stop 缺口已量化。
   见「flush_learning 落盘」一节与 README「已知失效项」第 6 条的关闭记录。
6. ~~**下一轮排期：实现纠错入口 + A3 第 5 批**~~ **方案 A 已落地（2026-10-14 03:25，见
   「方案 A 实现与真机验收」一节），B/C/D/E 按裁定不做。**
   **下一轮 = A3 第 5 批**（优先挑单字段占比 ≤ 1/3 的句式，实测在表 82% vs 全批 53%；
   用「候选条前 5 格」代理预检，**不开观测口**）；**B（候选条两行排版）排在 A3 第 5 批之后**，
   做之前要先按格宽重量一遍。本轮两条都没碰。
   **（现值 2026-10-14 13:25：A3 第 5 批做了，但预检触发停手判据、未落表；
   「≤1/3 挑句式」这条规则的证据本轮降级，见第 10 条与「A3 第 5 批」一节。）**
7. ~~**新缺口：「学习让排序变好」这句没有被证明**~~ **已闭合（2026-10-14 01:56）**：
   决定性实验做完，**学习有效**——`suoyi` 的 蓑衣（词频 48）在**第 1 次上屏后**就顶到第 1 格，
   压过 所以（155,225），冷启动后仍在；两张学习表一起挪开则退回（负对照）。
   上一轮那个「sha256 相同」的零结果是**测错了对象**（`nihaonihao` 的次序由排序键前 4 级决定，
   第 5/6 级没轮到比）。公式、预期与逐轮位次见「『学习改变排序』决定性实验」一节。
8. ~~学习这条线剩下没证的三条~~ **2026-10-14 03:05 已跑完，结论各不相同**（见「『错选纠正入口』评估」第五节）：
   ① **整句路径**：点**非最优整句**一笔都不记（Core 只按重算出的最优路径给词序列，
   `commit/mod.rs:420` 的等值判断），实测连点 6 次三张表全空 ⇒ 这条路上"个人 n-gram 影响整句"
   **喂不进去**；词级上屏积累的 n-gram 会不会影响整句次序，仍未测。
   ② **撇号写法**：**共用同一个 choice 键**（`suo'yi` 存成 `suoyi`），两种写法都让 蓑衣@0。
   ③ **μ 封顶 0.5 / 20 万条减半**：**设备侧不可测**（要几十次才过拐点、要 20 万条才减半），
   只能靠 Core 单测 ⇒ 从排期里划掉，别再当"便宜实验"。
9. **下一轮 = 「跨会话撤销」先评估，不动手**（2026-10-14 用户拍板：排下一轮，且**只评估**）。
   方案 A 只把撤销账本（`Engine::note_backspace` 的 `recent_commits`）做在**内存**里，
   会话一销毁（离开输入框 `onFinishInput` → `destroy`）就没了 ⇒ 「上一条上屏的字
   跨会话还能撤回」目前做不到。评估要交回的清单（每条都要 `file:line` 出处 + 实测/未核实标注）：
   ① `recent_commits` 在 Core 里的结构与容量上限（`engine/commit/` 那一层），
   以及它有没有现成的序列化口子可搭 `flush_learning` 那三张表；
   ② 若持久化，**撤销窗口**该定多长（按次数？按时间？跨 app？跨冷启动？）与产品语义
   （"撤销"和"删字"混在同一个 ⌫ 上会不会误删）；
   ③ 体积与耗时代价：往 `user*.tsv` 里加一张表对 `destroy` 落盘 7–9 ms 的影响，
   以及随包/用户目录的配额面；
   ④ 触发入口：只有 ⌫ 还是也要一个显式「撤销上屏」键（后者要 Java 破例，按第 7 条的先例评估）；
   ⑤ 与第 10 条的关系（如果 A3 要重开观测口，一次改动两件事一起做完更省）。
   **本轮只立这一条排期，一行代码都没碰。**
   **（现值 2026-10-14 23:35：①–⑤ 已在 20:50 那轮交回（见「跨会话撤销评估」一节），
   用户拍板方案甲；23:35 的实现轮撞到前提不成立——native 侧没有任何"输入框身份"可当缓存键，
   详见「算不拢测清 + 方案甲卡在『输入框身份』上」一节 ⑥，等裁甲-1 / 甲-2 / 甲-3。
   代码净改动仍为 0。）**
   **（现值 2026-10-15 01:26：用户裁甲-2 ⇒ 已落地并真机验收。**Java 破例 1 行代码 + 1 行注释
   （`core/MSIMEInputService.java:1697-1698`），Rust 按包名挂起/换回 `EngineSession`，
   上限 8 份、不落盘、私密框双向作废；`cargo test --release` 59 passed、
   `cargo fmt --check` 仍 17 hunk 全在 `lib.rs`、符号数仍 102。**本条结案**：
   跨会话撤销的入口仍是 ⌫（没有加显式"撤销上屏"键），窗口 = 进程存活期 + app 级粒度，
   冷启动后无入口（要跨冷启动只能改 Core，已排除）。详见「方案甲-2 落地」一节。）**
10. **A3 第 5 批停在门口，等用户裁三选一**（2026-10-14 13:25）：500 条待落表、316 条备用、
    11 行派生拼音已修正（`/d/toolchains/tmp/qingshan-snap/a3b5/` 有全套脚本与数据，
    见「A3 第 5 批」一节）。
    停手原因是**代理预检出现位次 2**（`话筒没有电`，两次复测同格同位次），这推翻的是
    「引擎二值行为」那条前提，不是"这批句子写得不好"。三个选项：
    ① A3 冻结；② 只落复测位次 0/1 的子集（要再跑一轮更大抽样）；
    ③ 再开一次观测口把"在不在表"与"排第几"分开测（一次代码改动 + 用完还原）。
    **在此之前不再扩量、不 `--append`、不重编 `.so`。**
    疑问清单 **31 条**（A 判据 3 / B 逐条句子 14 / C 词典 4 / D 流程 6 / E 英文 4）落在
    `/d/toolchains/tmp/qingshan-snap/a3b5/questions-b5.md`，本轮按「不擅自建文件」没进仓库。
    **（现值 2026-10-14 14:34：用户已选 ③，观测口已开完并还原、四档已出，D/E/B 31 条已裁并执行；
    批从 500 条变成 491 条（B 弃 9 条、E4 的 12 条缩短按实测还原）。仍**未落表**，
    等用户在「全落 491」与「只落位次 ≤1」之间拍板。详见「观测口第 2 轮与 E4 推翻」
    与「A3 第 5 批裁定执行结果」两节。）**
11. **C1 词典错读音：方案已列，等拍板**（2026-10-14 13:47–14:00 评估完，本轮没动任何数据）。
    根因是青简短语生成器的 tie-break（`phrases/mod.rs:231` 的"取词频最高的那条"），
    确证真错 **10 行**（`长时间/很长/好长/不长/这么长/太长/头发长/多长/那么长` 的 zhang→chang
    + `睡不着` 的 zhe→zhao），另有 `长的`/`不着` 两条歧义该出双行；
    277/2,476 那两个数都是**上界不是错数**。
    **推荐方案 C**：在我们 `engine/src/assemble.rs` 里挂 `engine.set_extra_dictionaries(...)`
    （挂点 `crates/qingjian-core/src/engine/setup.rs:495`、并入查词面 `:505 all_dictionaries`，
    本轮读码确认；**性能与实际效果未测**），把 10 条正确读音作为附加词库烘进 `.so`——
    随包 `dict.tsv` 与青简上游**保持 `cmp` 逐字节相同**，不改 Core、不删任何现有行。
    方案 A（直接改 dict.tsv）会把分叉做进数据文件；方案 B 越界；方案 D 是 C 的子集。
    **（现值 2026-10-14 16:58：用户拍板 C 并已落地、真机复测通过，见「C1 方案 C 落地与真机复测」一节；
    范围按 D 做满 12 条 —— 10 条错读音 + `长的`/`不着` 两条双行。）**
12. **下一轮 = 读清 Core 的模糊音/纠错代价表（D3）+ 跨会话撤销评估**，同一轮做完、都不动代码。
    **（现值 2026-10-14 17:02：用户改成**再下一轮**——中间先插一轮 A3 落表三合一，见第 13 条。**）**
    **（现值 2026-10-14 18:10：第 13 条那一轮已做完，所以这一项又变回**下一轮**；
    范围按第 14 条②扩大——除模糊音代价表与跨会话撤销，还要算"附加词库改变组句边"这件事。）**
    本轮实测证据：敲 `sijizailukoudengwo` 首格给 `自己在路口等我`（si→zi）、
    敲 `huatongmeiyoudian` 前格给 `话筒没有点/店`，而上轮 `xueyapiangao` 给过 `血压偏好`
    ⇒ 引擎在做平翘舌/编辑距离类的**模糊与纠错展开**（`crates/qingjian-core/src/correction/`
    确有 `costs.rs` `edit.rs` `typo/`，`fuzzy/rules.rs:106 expand()`），
    **这直接决定可达率天花板**，必须读清并把规则算进 A3 的挑句判据。
    第 9 条的跨会话撤销评估照挂，两项共用一轮；两件事都要读同一批文件（`commit/`、`fuzzy/`、`learning/`）。
    **（现值 2026-10-14 20:50：这一轮已做完——D3 读清见同名一节，跨会话撤销 5 项 + 四个候选方案见
    「跨会话撤销评估」一节，两者都**只读只评估、代码净改动 0**。留下的下一件事是第 15 项那条
    "算不拢"，以及第 9 项的撤销方案待用户拍板。）**
13. **下一轮 = A3 第 5 批落表三合一**（2026-10-14 17:02 用户拍板；本轮只记录，**没动表、没 `--append`、没重编**）：
    ① **A3 第 5 批全落 491 条**（不选"只落位次 ≤1"）。理由记在案：裁定后批实测 **24/31 = 77%**、
    位次 ≥2 只 1 条，比观测口那次的 67.5% 更贴近最终形态；全落比只落位次 ≤1 多约 50 条可见整句，
    代价是约 23% 的死行（已证无害：不影响别的条目、不报错）。数据在
    `/d/toolchains/tmp/qingshan-snap/a3b5b/set.tsv`（491 行）+ `reserve.tsv`（316 行备用）。
    ② **E2 一致性**：已落表语料里 `i will` 开头的 **169 条**（A3 源件与随包表两侧同数，实测）**连它一起改**，
    保持语料一代风格，不回滚新批的 198 处。做法：对 A3 源件跑一次"去掉 `^i will `"的批量改写，
    改前存快照、改后逐条 diff 数对上 169。
    ③ **B14 随行修订**：第 4 批已落表那 9 条（`我下楼取餐` `汤是凉的` `蜡烛插上` `商家不回复`
    `地址填错了` `我看得见` `我挂个号` `我补个觉` `地还没拖`）按本轮 B 的同一标准处理
    （拼音对但同音高频压过 ⇒ 改写或删行；句子不自然 ⇒ 删）。
    **一次 `--append` 写表 + 重生成锁 + 重编 `.so`（两条 ABI）+ `clean assembleDebug` + `install -r` 不 `pm clear`**，
    落表前按 D2 跑 `poly.py`（`assert mismatch==0`）、按 D6 抽 **40 条**代理预检，位次 ≥2 就停下重报。
    注意：②③ 是**动已落表内容**，与 ① 的"只加行"不同，改完后 `glossary-en.tsv` 行数与译词命中都要重新量。
    **（现值 2026-10-14 18:10：三合一已落地——源件 1,479→1,968 行、表 89,828→90,317 行、
    APK 49,861,845 B、`install -r` 不 `pm clear` 解包件 2,167,528→2,189,354 B 实测；
    E2 覆盖 166+3=169、B14 落为 7 改写 + 2 删行；逐行对盘零意外。
    见「A3 第 5 批落表三合一」一节。）**
14. **落表后新出的两件待裁（2026-10-14 18:06 实测，我没擅自动表）**：
    ① `这里不能长时间停` 上轮按"整表不可达"删掉，方案 C 之后它**到了位次 2**
    （修正表把 `长时间` 变成词，组句图多了这条边）⇒ 要不要加回源件再落一次。
    **我建议不加**：第 3 格 + 语义无优势，收益低于再走一遍"改表 + 重编 + 重装"。
    ② D3 的前提被改写：读 `correction/` 与 `fuzzy/rules.rs:106 expand()` 时，
    必须把**附加词库带来的组句边变化**一起算——可达率不只由模糊音决定，也由词表决定。
    这条要写进第 12 项那一轮的评估范围里。
    **（现值 2026-10-14 20:50：① 用户裁定**不加回**——第 3 格、语义无优势，不值得为它单独再走一遍
    "改表 + 重生成锁 + 重编 `.so` + 重装"；将来若与别的表改动同批，届时再评估。
    ② 也已在同一轮读清，见「D3 读清」一节第 ⑦ 条：附加词库确实进组句图。）**
15. **D3 已读清（2026-10-14 20:50，见「D3 读清」一节）⇒ 下一轮第一件事**：
    开一次观测口测清那个**算不拢的地方**——`sijizailukoudengwo` 全表里到底有没有 `司机在路口等我`。
    按公式（`ln(454875/14613) ≈ 3.44 < 替代边代价 5.0`）该是"司机"赢，实测"自己"占第 0 格。
    这一条不定下来，A3 第 6 批的挑句判据就只能靠抽样试错；测清后才知道该按"图上没边"
    还是"进了图但挤不进前 3 条路径"（`SENTENCE_CANDIDATES=3`）来设计规则。
    **（现值 2026-10-14 23:35：已测清并关闭——全表里**没有** `司机在路口等我`，
    8 条 −1 全部是"进了图但挤不进前 3 条路径"、"图上没边" 0 条；
    "格子剪枝"这个解释被排除。数据与判据见「算不拢测清…」一节 ②③。
    遗留：为什么 `司机` 那条路径连前三都进不去，仍未解释（同一节 ④），
    那不是候选表这个仪器能看的，要读路径分数。）**
16. **A3 冻结在 1,968 条（2026-10-14 用户裁定）**：第 6 批**不先定挑句规则就不起草**。
    前置三件按顺序做：① 未落表状态跑 40 条代理预检（D6）；② 把"位次 −1 的同音/相邻键替代模式"
    写成可机械检查的挑句判据（哪些模式必沉——本轮已给出两类样本：`偏高` 根本不是词、
    `司机` 被 `自己` 压）；③ 规则定了再起草，每批 ≤300 条（D5）。
    本轮不落表、不扩量、不动 `dict.tsv` / 表 / 锁 / `.so`。
    **（现值 2026-10-14 23:35：② 的判据有了硬数据——三类必沉模式：
    (i) 首字/关键字被同音更高频词抢走（`司机`→`自己`、`账号`→`帐号`、`改成`→`改称`），
    (ii) 目标句的词边齐全但整句挤不进 k=3 的束，
    (iii) 3 字母输入没有任何纠正路（`xan`/`sam` 实测缺），挑句时**不要**指望引擎纠出来。
    冻结与"每批 ≤300 条"照旧有效。）**
    **（判据重写 2026-10-15 00:30，用户拍板：第 6 批只防束宽，本轮记录、不起草、不落表）**
    - **防的对象只有一个**：目标句进了图但挤不进 k=3 的束（第 15 条实测 8/8 全是这一类，
      "图上没边"0 条）。所以**别再往表里加词边条目来救句子**——本轮那 8 条见证切分
      （`司机/在/路口/等我` 等）本来就边齐，加了也不上前 3。
    - 机械可检查的沉底信号（三条，都能离线算，不必真机）：
      ① **同音/近音更高频替代词**：目标句里某个跨度 `span` 上，存在同拼音、词频显著更高的另一个词
      （`自己 454,875` vs `司机 14,613` = 31 倍；`帐号 1,126` vs `账号 777`；`改称` vs `改成`）。
      判据写成：对目标句的每种切分，逐跨度取「该拼音串在词库里的最高词频词」，
      若拼出来的句子 ≠ 目标句，则该句**大概率沉**——这正是本轮 8 条的共同形状。
      ② **异体/近形词**：`账号/帐号`、`退票手续`这类，字不同、拼音同 → 表加条目治不好，起草时就排除。
      ③ **短音节输入**：整句里出现 ≤3 字母的音节段（本轮 `MIN_LETTERS=4` 实测 3 字母没有任何纠正路），
      不要把希望放在引擎纠字上。
    - 流程不变：① 未落表状态跑 40 条代理预检（用截图口径，且**按第 37 条改判别式**：
      要看候选的 `syllables` 或把对照放在首键，别用"首屏格子里有没有那个词"判纠错）；
      ② 判据如上；③ 规则过了再起草，每批 ≤300 条。
    - **停手判据沿用**：预检里位次 ≥2 超过 5 条就立刻停下汇报，不落表。
    - **（现值 2026-10-15 02:05：第 6 批已按此判据起草完并出疑问清单，见「A3 第 6 批起草」一节；
      未落表。停手判据那一条要按第 15 条的新结论改写——位次 ≥2 已被证实现存（`加湿器要加水` @2、
      `话筒没有电` @2），它不再是"推翻二值行为"的信号，只是"这批里有句子挤到第 3 格"的提示。
      真正的停手判据应该看**离线名次 vs 真机位次的错配率**，见同一节 ④。）**
    - **（现值 2026-10-15 03:20，用户裁定：第 6 批 294 条不落表。** 三条理由记在案：
      ① 40 条预检露出率只有 **5/40 = 12.5%**，落上去约 87% 是死行，"看到更多整句"这个目标达不到；
      ② 换来的代价是重生成锁 + 重编 `.so` + 重打 APK + 装机复验一整轮，与"多出 5 句"不成比例；
      ③ **离线判据正式降级为粗筛，落表与否以真机预检为准**（名次 1 的 10 条真机只露 5 条，
      名次 2–6 的 20 条与名次外的 10 条各露 0 条）。那 294 条草稿与 316 条备用池都只活在
      `/d/toolchains/tmp/qingshan-snap/a3b5b/reserve.tsv` 与 `a3b6/batch6-draft4-clean.tsv`
      （**不在仓里，也不是我能保证长期存在的**），日后要回收形状合判据的那几条得先从这两份文件里捞。
      第 7 批按新判据重写并测到 **35/40 = 87.5%**，
      见「A3 第 7 批起草」一节；那节末尾的对照说明写清了"形状"与"只留名次 1"两个变量没分开。）**
    - **（现值 2026-10-15 18:50，用户裁定：第 7 批直接落，不扩写到 200 ⇒ 已落表 63 条。**
      判据里补一条本轮验证过的做法：**起草阶段就按形状写、离线名次只当验收**（187 → 69 = 38% 存活），
      不要"先写完再筛"。落表前又加了两条机械冗余检查（与源件**同英文**、**中文头互为包含**），
      第 7 批因此删 6 条、改写英文 14 条 ⇒ 69 → **63**；表 90,317 → **90,380 数据行**。
      "形状 vs 名次"两变量没分开这条学术问题按你的裁定不追。冻结解除与否没定，
      下一批要不要继续扩见待办第 18 条 ④。）**
17. **下一轮 = 签名链与发布准备**（2026-10-15 用户拍板集中做；本轮只立项，**没碰任何签名相关文件**）。
    四件事，按顺序：
    ① 读上游 `platforms/android/scripts/signing.sh`（注意：路径带 `platforms/android/` 前缀，
       本仓 `README` 与 PIN 里曾出现过裸写 `scripts/signing.sh` 的错，见环境陷阱第 12 条那一组）
       与 `platforms/android/build-native.sh`，确认密钥生成/存储/引用方式；
    ② release APK 的生成路径：要不要 keystore（现在只有 `CN=Android Debug` 的调试签名，
       `apksigner verify` 实测过）、是走上游脚本还是我们 `host/gradle-app/app/build.gradle.kts:69`
       自己接签名配置；顺带确认 `zipalign -c -P 16 -v 4` 在 release 产物上也过；
    ③ GPL-3.0 §5a 的**完整性**：`NOTICE.md` 已记 Java 侧 4 行改动（两处）与未取的两个 `.kt`，
       但"源码分发"是否齐（要不要随包给完整 `engine/` + `host/java` 树、上游两份拷贝的许可声明）
       仍未了结——就是 `prd.md:821` 那行人工/法务审查的同一件事，别混进技术项；
    ④ `versionName` / `versionCode` 策略：现值 `0.1.0` / `1000`（`verify_apk.py` 实测），
       发布前要不要锁 `versionCode` 单调递增、debug/release 是否共用一个号。
    约束提醒：`apksigner` 是 `.bat`，`$JAVA_HOME` 没 source 时报的是"文件不存在"（环境陷阱第 11 条）；
    `apksigner verify --verbose 1 <apk>` 会把 `1` 当 APK 路径（同条）。
    **（现值 2026-10-15 03:20：①②③④ 四件已在「签名链与发布准备评估」一节交回，本条结案。**
    结论摘要：① 上游流程读全（一把密钥覆盖所有渠道、缺口令就硬失败、无 CI 时退开发密钥）；
    ② **必须新造青杉自己的发布密钥**（上游那把属 `app.msime.android`，且不在我们手上），
    走上游 `signing.sh` 不成立（`security` 本机没有、无扩展名调用 exit 127），
    建议另写 `host/scripts/sign-apk.sh`、不把 `signingConfig` 写进 Gradle；
    ③ §5a 的**声明**齐、**分发**没落地（实测 `projects/qingshan` 无 `.git`，
    `qingshan/NOTICE.md:26-28` 只写了 debug 产物路径），另新查出一条发布阻塞项：
    宿主「检查更新」写死水杉地址（`qingshan/host/java/app/msime/android/account/UpdateApi.java:45/47/49`），后果是"提示新版→下载→拒装"的死循环，
    不是装错包（`qingshan/host/java/app/msime/android/account/UpdateApi.java:404` 先核包名）；
    ④ 版本策略：debug/release 共用 `host/version.txt`、`versionCode` 只增不减、渠道差异只进 tag。
    **没做的一件**：`zipalign -c -P 16 -v 4` 只在**现有 debug 包**上实测退出码 0，
    release 产物上**没验过**（本轮按你的约束没出过 release 包）。**
    **（现值 2026-10-15 18:50：那三件"没验过"的这轮全验了，见「签名链落地：第一个 release 包」一节——
    产物名 `app-release-unsigned.apk`、`lintVitalRelease` 确实在发布路径上跑、
    `run-as` 在 release 上失效。上一段结案块里那句"没做的一件"到此作废。**）
18. **发布收尾三件 + A3 下一步（2026-10-15 18:50 立，等用户排）**：
    ① **密钥只剩一份**：`D:\toolchains\keys\qingshan-release.p12` 与它的 `.password` 是这台机器上
    的唯一副本，口令本轮生成后没有出现在任何日志/文档里（我也没打印过）。
    **要用户自己做一份加密离线备份**——丢了就等于放弃所有已装机的更新通道，换钥匙不可行。
    ② **公开源码的位置还没有**：`NOTICE.md` §5 那句"本仓库即完整对应源码"目前落不到一个能被
    第三方取到的地址（本目录无 `.git`），且 `host/assets/dictionary/*.tsv` 在 `.gitignore` 里，
    公开仓库还要能重建词库。发第一个对外包之前要先定这件事（与 `prd.md:821` 的法务审查并行、不混）。
    ③ **「检查更新」三选一还没裁**（现象与证据见 `README.md` 已知失效项第 11 条）：
    指向青杉仓库 / 关掉那三项入口 / 只发不经应用内更新的包。本轮按你的裁定只落笔、没改 Java，
    也**没在真机点过那个按钮**（装完 release 包后设备无外网，点了也测不出下载行为）。
    ④ **A3 下一步**：第 7 批 63 条已落表并真机可见（位次 0 带译词）。要继续扩就照本轮这条链走
    （起草按形状 → `derive5` → `poly` 关卡 → `path` 只留名次 1 → 与源件做"同英文/互为包含"两条
    冗余检查 → 40 条真机确认 → `--append` 正规通道 → 重编 `.so` → `install -r`）；
    第 6 批那 294 条草稿与 316 条备用池仍在 `/d/toolchains/tmp/`（**不保证长期存在**），
    要回收得先在那儿还在的时候捞。
    **（现值 2026-10-15 20:55：草稿归档位置已逐个 `ls -l` 确认，都在 `qingshan-snap/` 里，
    且根目录是 `D:\toolchains\tmp\`（不是 Windows Temp，不会被静默清）——
    `a3b5b/reserve.tsv` 22,394 B、`a3b6/batch6-draft4-clean.tsv` 20,807 B、
    `a3b7/draft7-raw.tsv` 8,458 B、`a3bj/land7.tsv` 4,049 B（已落表那 63 条）。
    另注：`/d/toolchains/tmp/` 顶层还散着 `a3b5/`、`a3b5b/`、`min4/`、`probe15/` 等同名旧目录，
    **本轮用的辅助脚本一律取自 `qingshan-snap/a3b5b/`**（`derive5.py`/`poly.py`/`strip.py`/`pick.py`），
    下次要收敛这份重复，别顺手 `cd` 到 Loose 的那份。**）
19. **GPL 源码分发：等用户拍方案**（2026-10-15 评估完，本轮**没改 `.gitignore`、没改 `NOTICE.md`**）。
    结论一句话：**缺的不是那 4.2 MB 数据，是一个第三方取得到的公开仓库地址**——
    `glossary-en.tsv` 实测可由「青简 @ `4a3e963d…` + 我们的源件 + 我们的脚本」**逐字节重建**、
    `dict.tsv` 与青简那份**逐字节相同**，`LICENSE:59` 允许不给可重建的东西。
    三条方案（B 推荐 / A 只进 `dict.tsv` / C LFS 不推荐）与代价、条款原文都在
    「GPL 源码分发完整性评估」一节。**拍板后要做的**：建公开仓（用户侧决定何时、放哪个账号）、
    改 `NOTICE.md` §5 与 `README.md` 构建前提、顺手修那三处过期数字（词库字节数 / 锁 52 B / 86 桩）。
    **（现值 2026-10-15 21:55：方案 B 已落地 ⇒ 本条只剩「公开地址待填」一件事。**
    上面那句「没改 `.gitignore`、没改 `NOTICE.md`」是上一轮的状态，本轮两处都改了：
    `.gitignore` 只**加**了两条（`__pycache__/`、`*.pyc`，因为 `git add -A` 把字节码收进来了），
    `host/assets/dictionary/*.tsv` 按 B 继续忽略；NOTICE §5 与 README「构建前提」按 B 改写，
    三处过期数字已修（词库 2,192,288 B / 90,380 行 / 口径改成词库全量、锁 167 B、桩 85 个）。
    `git init -b main` + 首次提交 `db5ad5a`（579 文件 / 92,024 行）已建，另加了 `.gitattributes`
    （这台机器 `core.autocrlf=true` 会把 `sign-apk.sh` 检出成 CRLF）。
    **待用户做**：建 GitHub 仓 → 换掉 NOTICE/README 里两处「⚠ 待填」→ 自己跑
    `git remote add origin <URL>` + `git push -u origin main`（agent 不碰凭据）。**）
20. **「检查更新」等用户拍三选一**（2026-10-15 读码完，**没改 Java**）。推荐 **b**：
    破例面实测 **1 个文件 1 行**（`UpdateJobService.java:123-125` 那个谓词恒真），
    一次关掉「检查更新」药丸 + 「自动更新」「更新通道」两行 + 每天一次的后台任务；
    且 `:92-94` 显示自动更新默认关 ⇒ 现状（方案 c）的危害只是"用户主动点一下白下一趟"。
    方案 a 要 5–6 处 + 一个 releases 列表端点 + 发布侧上传 `.sha256`，且依赖第 19 条先落地。
    **`local_mode` 不是这条路**（它是键盘视图状态，7 处读它，见该节）。
    **（现值 2026-10-15 21:55：按 b 落地，本条结案。** 第二个提交改 `UpdateJobService.java`
    （**11 增 1 删**，宿主 Java 树第 3 处破例），形式是命名开关 `IN_APP_UPDATES_DISABLED = true`
    短路，不是上面说的「1 行恒真」——直接 `return true` 会让 `installer()` 与 `PLAY_STORE`
    变死代码，翻开关回退才是一行（按「可逆优先于删除」那条既有裁定）。
    `compileDebugJavaWithJavac` 退出码 0；**本轮未重打 APK ⇒ 真机未验**，
    下一轮出包时到关于页确认那三样确实不显示了。**）
    **（现值 2026-10-16 00:06：真机三处全部验毕，本条正式关掉。** 23:05 `clean assembleDebug`
    出包 49,897,529 B → 23:07 `install -r`（不 `pm clear`）→ 关于页两屏 `uiautomator dump --windows`：
    顶部 `版本 0.2.0 · Android` 之后直接是 `设备信息`（药丸没了），滚到底那一屏也没有
    「自动更新」「更新通道」两行；后台任务查 `dumpsys jobscheduler` 全量输出，
    `app.qingshan.ime` 只命中 3 处（`TopAppTimer` / `ShrinkableDebits` / UID 表 `10695`），
    **零条 `JobStatus`** ⇒ 每天一次的更新任务没排上。`engine/tools/verify_apk.py` 复跑
    包内容=True 符号=True 清单=True。**）
21. **横屏下候选条整条不显示：本轮复现不出来，降级为"未定位"**（2026-10-15 20:35 撞见，见环境陷阱第 47 条）。
    现象：`mCurrentOrientation=90` 时试用页键位正常、会话活着、`input text nihao` 后
    **候选面板一个节点都没有**；锁回竖屏立刻恢复。本轮为了做预检直接锁屏绕过了，
    **没查根因**（属宿主布局/测量还是 `onEvaluateInputViewShown` 那条硬件键盘分支，未知）。
    真机上横屏打字是常见场景，值得单独一轮：先确认是不是只有试用页这样，再看任何输入法的横屏表现。
    **（现值 2026-10-16 00:14：单独一轮做完了，**这条没有结案，但也没有复现**。**
    试用页横屏键位点击 10 条候选、系统设置搜索框横屏 10 条、竖屏两种探针 6 / 10 条
    ⇒ **四格全部正常，横屏本身不坏**；读码也证实候选条可见性三行里**没有方向这一维**
    （`qingshan/host/java/app/msime/android/core/MSIMEInputService.java:7818`/`:7832`/`:7836`），
    全仓与上游都**没有 `layout-land`/`values-land`**。
    **中途我犯过一次错**：先用一支写坏的计数器（`cls=='Button'` 等值比较，恒假）"证明"这是
    `input text` 不建组合的假象，并把撤回写进了文档；改用 `endswith` 重跑后同一批操作
    竖屏 6 条、横屏 10 条，**那个解释作废**（教训见环境陷阱第 48 条）。
    20:35 那次的 0 候选**仍未定位**，两个没排除的差别：真实传感器旋转 vs 本轮 `cmd window` 强转、
    以及预检脚本拿竖屏坐标表去点横屏键位。本条**保持打开**，等用户侧的复现口径
    （哪个应用、转屏前还是转屏后打的字）。同一轮量到两条**真的**缺陷，另立第 22 条。**）
    **（裁定 2026-10-16 00:41（用户）：不追，除非有可复现的操作序列。** 四格实测已证横屏本身不坏，
    那次 0 候选最可能是"旋转重建会话"那一瞬的效应（见第 22 条 ①），继续追的成本是装一个上游对照包
    （本机装不了，包名 + 证书双冲突），收益不明。⇒ **本条冻结**：不再单独立轮，
    只有当用户给出**可复现的操作序列**（哪个应用、哪一步）时才重开。**）
22. **旋转丢组合 + 偏好读取恒失败：两条从第 21 条那一轮量出来的真缺陷**（2026-10-16 00:14 立项，**本轮没改任何代码**）。
    ① **旋转把组合冲掉**：竖屏组合中（preedit `nihao` 在、候选条在），锁到横屏之后
    `tryout_field` 变成 `nihaonihao`（**字母原样进了输入框**）、preedit 节点消失、候选条没了；
    截图 `a3bm/Y3-after-rot.png` 上那一行只剩诊断提示文字，连工具栏行也不在。
    logcat 同一时刻出现一整条 `[msime_android::session] 会话建立 125 ms（含词库载入）`
    ⇒ 旋转走了一次全新会话，组合上下文随之丢掉；`dumpsys window` 里 IME 窗口
    `req=2400x794` → `2400x683`（差 111 px ≈ 标题行 34dp + 候选行 42dp）。
    **口径声明**：这一轮"候选数从 7 掉到 0"那组数字**不算证据**（那支探针把招呼语气泡也数进去了，
    见环境陷阱第 48 条），站得住的是"字段多出原样字母 + preedit 消失 + logcat 会话重建 + 截图"这四样。
    **未定**：上游水杉是否同样重建会话（那段 Java 调用链是上游的，我们的改动只有
    `options.put("application", …)` 那一行），
    要定它得能装上游 APK 做对照，本机装不了（包名冲突 + 证书不同）。
    ② **偏好读取恒失败**：键盘上那句 `设置读取或应用失败，保留当前设置` 每次必出，源头是
    `loadPreferencesRaw` 还在 `qingshan/engine/src/stub.rs:100` 的 85 个占位里，回
    `{"ok":false,"error":"not_implemented"}` ⇒ `qingshan/host/java/app/msime/android/core/MSIMEInputService.java:2145`
    的 `value()` 抛 `Shared runtime rejected operation` → `:2147` catch → `:2149` 置提示
    （**机制 2026-10-16 更正过一次，上一版写的"`PreferencesReloader` 置 null"是错的**）。
    它不只是难看：`hasDiagnostic` 一真就**同时**隐藏工具栏行与候选行
    （同文件 `qingshan/host/java/app/msime/android/core/MSIMEInputService.java:7818`、`:7832`、`:7836` 三行，
    4 秒后自动恢复，见 `qingshan/host/java/app/msime/android/policy/InputDiagnosticPolicy.java:5`），
    所以"候选条忽然整条不见"在**任何方向**都能发生。
    **（现值 2026-10-16 00:41：`loadPreferences` 真实现已评估完，**本轮按"< 100 行才动手"的门槛判定不实现**。**
    三个硬结论：① 偏好这条路上有**五个** native 全是桩（load / save / update / default / restoreDefault），
    ② **只做 load 消不掉那句提示**——`applyPreferencesSnapshot` 自己在
    `qingshan/host/java/app/msime/android/core/MSIMEInputService.java:2260` 还要过一次
    `updatePreferences`，照样抛、照样进同一个 catch；③ 切片估行：只 load 40–55 行（假修复）、
    load+update 85–110 行、再加 save 145–175 行、再加 default/restoreDefault 200–240 行，
    **全部不破 Java 零改动**。附带好消息：**load+update 一落地，
    `candidate_english_gloss` 的默认 true 就会把 D 路线的门打开**（同文件 `:2221`/`:2295`，
    今天恒假只是因为 `:2260` 先抛了）。方案与落地次序见「loadPreferences 真实现评估」一节，**等用户拍**。**）
