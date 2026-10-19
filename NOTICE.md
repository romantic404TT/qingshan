# 许可与来源声明（NOTICE）

青杉输入法（Qingshan IME）是下面两个上游项目的**修改/组合作品**，以
**GPL-3.0-or-later** 分发。本文件是 PRD §5.3「许可证合规」要求的分发声明；
两份许可证全文随包：`LICENSE`（GPL-3 正文，取水杉那份）与 `LICENSE-QINGJIAN`（GPL-3 正文，取青简那份）。

## 上游与固定依据

| 组件 | 取自 | 许可证 | 原文位置 |
| --- | --- | --- | --- |
| Android 宿主（Java / res / Manifest） | 水杉输入法 MSIME，公开仓库 <https://github.com/metasequoiaime/msime>（出处：其 `README.md:8` 的「GitHub 仓库」链接），本地拷贝 `projects/msime-develop` | **GPL-3.0-only**（其 `README.md:136`） | `LICENSE` |
| 输入引擎 Core | 青简输入法 Qingjian，公开仓库 <https://github.com/romantic404TT/qingjian.git>（出处：本地那份拷贝的 `git remote -v` 的 origin），HEAD `4a3e963dfe299126574bfe3c370ff70a3f85ee77`，本地拷贝 `projects/qingjian` | **GPL-3.0-or-later**（其 `README.md:12`） | `LICENSE-QINGJIAN` |
| JNI 适配层 `engine/`、Gradle 工程 `host/gradle-app/`、本目录全部脚本 | 本项目新写 | GPL-3.0-or-later | 同上两份 |

两个上游的路径、HEAD、水杉那份归档的 MD5 与字节数以 `third-party/PIN.md` 的
「青简 Qingjian」「水杉 MSIME」两节为准。**水杉这份本地拷贝没有 `.git`**（它由
`msime-develop.zip` 解包而来），所以给不出 commit 号，固定依据是归档本身：
zip 名 `msime-develop.zip`、MD5 `cfdab2624a8e6469b588984477f1ed65`、29,461,249 字节；
要核对就从上表那个公开仓库的 `develop` 分支重新取一份，与本归档逐文件比对。

**名称与 logo 不在代码授权范围内**：青简 `README.md:55` 原话是「代码采用 GPL-3.0-or-later
许可；项目名称与 logo 不包含在代码授权中」。本项目不自称水杉或青简，也不使用二者的名称与图标；
`README.md` 的「已知失效项」第 4 条记录了宿主 Java 树里残留的 35 处上游品牌字及其处理方式。

## 完整对应源码与修改说明（GPL-3.0 §5）

### 本仓库的公开地址

**本仓库暂不公开（用户 2026-10-16 拍板）** —— 本节下面所有"从哪取"的承诺**在公开之后才生效**。
公开地址届时填在这里：`https://github.com/<用户名>/<仓库名>.git`。
本机仓库现状：`git` 历史在（`db5ad5a` + `a86de33` + `d3b88d6`），**没有 remote，也没有 push 过**，
所以此刻任何人都取不到本仓库的源码。
**⚠ 合规红线不变**：一旦**对外分发本 APK**（任何渠道，包括发给朋友），GPL-3.0 的
"完整对应源码"义务立刻生效，**必须先公开仓库并把上面那行地址填上**再分发
（判据与三条取源路径见下面 §5，`third-party/PIN.md` 待办第 19 条）。
**（现值 2026-10-16 12:34：** 上一轮之前这里写的是「⚠ 待填」，容易被读成"agent 卡在等一个 URL"。
改成现在这句是照实记录：**不是没拿到地址，是用户决定暂不公开**，因此上一轮生成的
"填 URL + `git remote add` + `git push`"那组命令**已撤销**，本轮起不再生成 push 命令。
上面两个上游地址（水杉 `metasequoiaime/msime`、青简 `romantic404TT/qingjian`）已走 HTTP 复核，
两仓都真实存在，那部分是公开与否都要履行的义务，不受本条影响。**）

分发本 APK 时，本仓库即**完整对应源码**（complete corresponding source），包含：
`engine/`（Rust 适配层全部源码与 `Cargo.toml`）、`host/`（Java 树、res、Manifest、assets 里的
锁文件与生成脚本、Gradle 工程）、`third-party/PIN.md`、本文件、`LICENSE` 与 `LICENSE-QINGJIAN`。
构建步骤与工具链版本写在 `README.md` 的「构建前提」「构建步骤」两节，产物路径为
`engine/jniLibs/<abi>/libmsime_android.so`、`host/gradle-app/app/build/outputs/apk/debug/app-debug.apk`
与发布包 `app-release-unsigned.apk`（签名见「构建步骤」第 9 步）。

### 不随源码分发的那两份数据，以及怎么重建（2026-10-15 拍板：方案 B）

`.gitignore` 里 `host/assets/dictionary/*.tsv` 让两份**随包数据**不进仓：
`dict.tsv`（2,053,612 B）与 `glossary-en.tsv`（2,192,288 B / 90,380 数据行）。
依据是本许可证文本 `LICENSE:59`——「The Corresponding Source need not include anything that users
can regenerate automatically from other parts of the Corresponding Source.」两者都能重建：

| 随包文件 | 怎么得到 | 实测依据 |
|:---|:---|:---|
| `host/assets/dictionary/dict.tsv` | 从青简仓库（上表地址，HEAD `4a3e963d…`）取 `assets/lexicon/dict.tsv`，**原样复制**，本项目不改它 | 2026-10-15 20:47 `cmp` 与 `projects/qingjian/assets/lexicon/dict.tsv` **逐字节相同** |
| `host/assets/dictionary/glossary-en.tsv` | 在 `engine/` 下跑一次：`python -X utf8 tools/build_glossary_subset.py --out ../host/assets/dictionary/glossary-en.tsv`（默认 `--append` 指本仓的 A3 源件，默认 `--dict`/`--glossary` 指青简的 `assets/lexicon/dict.tsv` 与 `assets/glossary/glossary-en.tsv`） | 同一命令在仓库外重跑，产物与随包那份 **逐字节相同**（2,192,288 B） |
| `host/assets/desktop-dictionary.lock.json` | **在仓内**（167 B），由上一条命令顺手按磁盘实际字节重写；它被 `include_str!` 烘进 `.so`，所以**换表必须重跑生成 + 重编 `.so`** | 脚本对不上就报错不写；`engine/src/resources_lock.rs` 的 `packaged_lock_matches_shipped_assets` 单测在两者脱节时红 |
| `engine/src/reading-fixes.tsv`（1,734 B）与 `engine/tools/glossary-sentences-zh-en.tsv`（74,167 B，A3 整句源件） | **都在仓内**，不受 `.gitignore` 影响（那条只挡 `host/assets/dictionary/` 下的 `*.tsv`/`*.qj`） | `git check-ignore` 等价扫描：两者都在"将被提交"清单里 |

也就是说：clone 本仓库 + clone 上表两个上游 + 跑这一条命令，得到的随包数据与发布包里的一致。
**这一段此前的状态（2026-10-15 03:20 核对、18:55 更新）**是"本目录还没有版本库、release 步骤没写进
README"——后者已在当天 18:19–18:20 出过并验过第一个发布包（0.2.0 / versionCode 2000），
前者由本轮的 `git init` 与首次提交解决，只剩上面那个**公开地址待填**。
**（现值 2026-10-16 12:34：上面那句"待填"已改成"暂不公开"——不是等地址，是用户决定这一步先不做。
**分发 APK 之前仍必须填回真地址**，那是 GPL 的对应源码义务，不因暂不公开而免除。**）
细节见 `third-party/PIN.md`「GPL 源码分发完整性评估」与「签名链落地：第一个 release 包」两节。

**许可证文本本身不含私钥义务**：签名私钥不属于"对应源码"，任何时候都不随源码分发
（公开它等于永久放弃更新能力）。`.gitignore` 已把 `*.p12` / `*.keystore` / `*.jks` / `*.password`
全挡在库外，密钥放在仓库外的 `D:\toolchains\keys\qingshan-release.p12`。
`prd.md:821` 要求的人工/法务审查是**另一件事**，不因本节补齐而作废。

对上游代码所做的修改（§5a 要求的改动声明）：

1. `host/java/app/msime/android/core/MSIMEInputService.java` 改了 **4 行**（两处）：
   - `:5242` 与 `:5249` 的启动失败提示里的品牌字（各 1 行，共 2 行）。
   - `:1697-1698` 新增 2 行（1 行注释 + 1 行代码）：建会话前把已经算好的包名写进建会话选项，
     作为跨会话撤销的挂起键。理由：native 侧收到的建会话请求里只有 `resources` 与 `state_root`
     （`core/Bootstrap.java:43-44`），没有任何输入框身份，而能跨「离开输入框再回来」存活的身份
     只有 `EditorInfo.packageName`（宿主自己在 `:1233-1235` 的 `currentDocumentIdentifier`
     回到同一个框也会换新号，不能当键）。**不改任何交互、不加符号**（导出仍是 102 个，
     `engine/tools/verify_symbols.py` 双向差集为空），也不动 `stop()`/`destroy` 的调用时机。
     Java 树其余 393 个文件逐字未改（第 7 条另计一处，改后为 392 个）。
   **追加（2026-10-18 21:40，用户拍板「待办 36 乙」）**：同一个文件又改了 **2 处**（改前 `:1298` 与 `:7599`，现 `:1306` 与 `:7614`，各 2 行代码 + 4 行注释）：
   两处 `options.getJSONObject("preferences")` 之前各加一句「缺时先 `put` 一个空对象」。
   理由与真机读数见 `third-party/PIN.md`「P0 隐私死锁修复与假入口清理」；这是**第 1 处破例
   那个文件的续修**，不另计处数（未改文件数改口见第 10 条末尾）。
2. 宿主树**未取**上游两个 Kotlin 插件文件
   （`java/app/msime/android/account/AccountPlugin.kt`、`java/app/msime/android/voice/VoicePlugin.kt`）。
   它们只被一处 javadoc `{@code}` 提及，不构成编译依赖；`java`+`res` 文件数 539 对上游 541。
   详见 `README.md` 已知失效项第 5 条。
3. `AndroidManifest.xml` 的 `applicationId` 为 `app.qingshan.ime`，`namespace` 与全部 Java
   包名仍是 `app.msime.android`（394 个文件声明该包名，且 `AppEdition` 反射硬编码这个类名）。
4. `host/gradle-app/` 是本项目新写的独立 Gradle 工程，不是上游 `platforms/android` 的构建脚本；
   上游的产品版本（edition）机制与 6 个 productFlavor 未沿用，**签名链也没沿用而是另写的一套**
   （`host/scripts/sign-apk.sh` + 青杉自己的发布密钥，理由见 `third-party/PIN.md`
   「签名链与发布准备评估」；上游那份 `platforms/android/scripts/signing.sh` 依赖 macOS 钥匙串，
   且它的 `msime-release` 密钥属于水杉的 `app.msime.android`，与我们的包名无关）。
5. 引擎侧**不链接**水杉的 `input-runtime` / `engine-bridge` / C++ `MSIME-Engine`，改由
   `engine/` 顶替水杉原来由 `platforms/android/native/client_jni.cpp` 编出的那个库。
   水杉与青简两个上游目录本身**一个文件都没改**（只读引用）。
6. **新增一个 debug 专属测试页**（本项目新写的两个文件，不在上游树里，也不属于 release 包的内容）：
   `host/gradle-app/app/src/debug/java/app/msime/android/TestPasswordActivity.java` 与
   `host/gradle-app/app/src/debug/AndroidManifest.xml`。它们放在 Gradle 的 **debug 源集**里，
   release 变体既不编译也不合并清单（实测：release 包清单里 `TestPasswordActivity` 出现 0 次）。
   用途是验「密码框不建会话」这条隐私硬门（`README.md` 已知失效项第 4 条与 PIN「密码框路径 C」）。
   它不新增 native 符号（导出仍是 102 个），不改任何上游文件。
7. `host/java/app/msime/android/home/UpdateJobService.java` 改了 **12 行（1 处，11 增 1 删）**——
   宿主 Java 树的**第 3 处破例**（2026-10-15 用户拍板方案 b）：新增
   `private static final boolean IN_APP_UPDATES_DISABLED = true;` 及其 8 行注释，并把
   `installedFromPlay()` 改成 `return IN_APP_UPDATES_DISABLED || PLAY_STORE.equals(installer(context));`。
   理由：`UpdateApi` 的三个地址写死水杉的发行版，我们的包点「检查更新」会白下一趟再被包名核对挡掉；
   借 Play 那道现成的门关掉三处消费（关于页药丸、两行设置、每日后台任务）。
8. `host/java/app/msime/android/policy/InputDiagnosticPolicy.java` 改了 **1 行**（1 处）——
   宿主 Java 树的**第 4 处破例**（2026-10-18 用户拍板「戊-A 方案 B」）：
   `DISMISS_DELAY_MILLIS` 由 `4_000L` 改为 `1_000L`。
   理由：键盘上任何一句诊断提示都会点亮
   `qingshan/host/java/app/msime/android/core/MSIMEInputService.java:7804` 那个 `hasDiagnostic`，
   而它一真就**同时**让标题行、工具栏行、候选行、候选视口四行看不见（同文件 `:7812`、`:7818`、
   `:7832`、`:7836` 四行判据），到自动消失才恢复。本项目还有 79 个 native 桩，其中
   **「组字中长按候选 → 点优先显示」**这条普通用户手势会稳定撞上（B1 回 `handled:false`，
   宿主据同在 `qingshan/host/java/app/msime/android/core/MSIMEInputService.java:6044`
   弹「当前候选不支持此操作」）。按等级收窄判据要逐个判 32 个提示调用点、且**打不中现状**
   （评估全文见 `third-party/PIN.md`「待办 27 实验①② + 全角真机数 + 戊-A 评估」第五节），
   缩短窗口是 1 行的最小破例面。**代价如实记**：真实失败提示的可读时间也一并从 4 秒缩到 1 秒。
   导出符号与 `deferred` 语义都不受影响（`engine/tools/verify_symbols.py` 双向差集仍为空）。
   **没动 `UpdateApi.java`，也没删 Play 来源判定**（翻回 `false` 即恢复原行为）。
   其余 392 个 Java 文件逐字未改。（现值 390 个，见第 9 条；这条当时就该是 391——`find host/java -name "*.java"` 得 394，
   减去第 1、7、8 条那三个文件。本轮改用 `diff -rq` 实测，不再靠减法。**）
9. `host/java/app/msime/android/home/KeyboardOptionsPage.java` 改了 **1 行**（1 处）——
   宿主 Java 树的**第 5 处破例**（2026-10-18 用户拍板「待办 30 方案 A」）：`layoutPair`
   （`:288-295`）的 `case QUANPIN, QUANPIN_NINE_KEY` 由「返回 26 键与 9 键那一对」改成 `-> null`（`:290`）。
   理由：青杉的键盘布局是从引擎回声的 View 算的（`host/java/app/msime/android/core/MSIMEInputService.java:3770-3775`
   的 `touchLayout(value)`），而我们 `engine/src/view.rs:184` 的 `nine_key` 恒为 `false`、`:189` 恒为
   `TwentySixKey` ⇒ 九键面板永不出现，可设置页那一行还让用户点「9 键」，这是「静默不答应」。
   改成 `null` 走的正是上游自己留的降级出口（同文件 `:93-97`：`pair == null` 时那一行变成只读说明、
   点击动作传 `null`），屏幕上那句「当前方案只有一种键盘，在「输入」里换方案」是上游现成的话术，
   不是我们新造的。**为什么不能只改配置**：`host/java/app/msime/android/keyboard/KeyboardScheme.java:12`
   里 `QUANPIN_NINE_KEY` 的 engine 方案与 26 键同为 `"quanpin"`，而
   `host/java/app/msime/android/core/AppEdition.java:80-82` 的 `offers(String engineScheme)` 只看 engine
   方案 ⇒ 只要本版本有全拼，九键就被算成「提供」。
   **已知的残留**：这一处改动之前已经选过「9 键」的用户，存储里仍是 `QUANPIN_NINE_KEY`，`:91` 那句
   `nineKey ? "9 键" : "26 键"` 让这个只读行继续显示「9 键」，而实际键盘照旧是 26 键；去「输入」页换一次
   方案再换回来即可复位。要在设置页直接显示「26 键」是第 2 行（把 `:91` 的判据改成常量），
   本轮按拍板只做 1 行，多的那行留给下一次。
   native 导出仍是 102 个（本轮没加符号），`engine/` 之外只有这一个 Java 文件动过。
   **追加（2026-10-18 18:20，用户拍板补第 2 行）**：同一个文件又改了 **2 处代码**（`:98` 的 `nineKey`
   判据加上 `current != QUANPIN_NINE_KEY`；`:101` 的 `pairOffered` 加上 `pair.length == 2`）与 8 行注释。
   合起来本条累计 **1 处 null 化 + 2 处判据 = 3 处、代码 3 行**，仍只算这一个文件、仍记为第 5 处破例。
   **未改文件数不变**：`diff -rq msime-develop/platforms/android/java qingshan/host/java` 实测 differ 仍
   **4 个**、`Only in qingshan` 0 个、`find host/java -name "*.java"` 394 个 ⇒ 仍 **390 个逐字未改**
   （本轮改的是已经计入的那一个文件）。
   **实测口径**：`diff -rq msime-develop/platforms/android/java qingshan/host/java` 得「differ」4 个文件
   （MSIMEInputService / ImePanels / KeyboardOptionsPage / UpdateJobService / InputDiagnosticPolicy），
   `Only in qingshan` 0 个；`find host/java -name "*.java"` 394 个 ⇒ **389 个逐字未改**。
   （现值 2026-10-18 23:20 轮 AJ 实测：differ **12 个**、`Only in qingshan` 0 个、`find` 394 个
   ⇒ **382 个逐字未改**；比上一记多出来的 7 个全在第 11 条那批品牌字替换里。
   另：`host/res/values/strings.xml` 也改了 1 处，它不在这棵 java 树的计数里。）

11. **品牌字批量替换：8 个 Java 文件 + 1 个资源文件，共 14 处字符串**（2026-10-18 轮 AJ，
   用户拍板"低风险的就改"）——把**指"这个应用"本身**的水杉改成青杉，逐条：
   `core/ImePanels.java`（剪贴板搜索的 contentDescription 1 处）、
   `home/ClipboardSearchPage.java`（3 处：两句未开启/空态提示 + 复制时的 clip label）、
   `home/HelpPage.java`（1 处行标题「键盘里没有青杉」）、
   `home/KeyboardTryoutActivity.java`（1 处 AI 提示词）、`home/LexiconPage.java`（1 处导出文件名）、
   `home/OnboardingActivity.java`（4 处引导文案）、`voice/VoiceRecognitionActivity.java`（1 处系统语音框提示）、
   `home/BackupPage.java`（1 处「这不是青杉输入法导出的备份文件」）、`res/values/strings.xml`（1 处试用页说明）。
   **改前三条逐一核过**：① 这 14 处**都不在** `home/PageId.java` 的搜索关键词里（那张表只有 4 处水杉，
   全部留在没动的"成对档"里）；② 都不是服务声明（「水杉云 / 水杉账号 / 水杉 MCP / 水杉 ID」共约 17 处
   刻意不改——我们没跑那些后端，改了就等于告诉用户数据进一个不存在的服务）；
   ③ 都不参与逻辑匹配（`PageId` 与行标题是 `contains` 关系，改错一侧会让设置页搜不到那一行）。
   **代价如实记**：§5a 的改动文件数从 5 个涨到 12 个（java 树）+ 1 个资源文件，
   将来同步上游时这 9 个文件都要手工重放；分档全文见 `third-party/PIN.md`
   「v1.0 综合验收与清理」第四节与 `README.md` 已知失效项第 4 条的现值。

10. `host/java/app/msime/android/core/ImePanels.java` 改了 **2 行**（1 处）——
   宿主 Java 树的**第 6 处破例**（2026-10-18 用户拍板「待办 37 顺手修掉」）：
   `renderSchemePicker` 里取列表那一行（现 `:1041-1043`，改前 `:1036`）由
   `= s.visibleSchemes` 改成先拷一份、再 `schemes.remove(KeyboardScheme.QUANPIN_NINE_KEY)`。
   理由：键盘上这个「输入方案选择器」是九键的**第二个入口**，而九键面板在本项目画不出来
   （布局从引擎回声的 View 算，`engine/src/view.rs:184` 恒 `false`、`:189` 恒 `TwentySixKey`）；
   第 9 条只挡掉了设置页那一行。**为什么是剔除而不是标灰只读**：这是一张会按下去的卡片，
   留着它灰着不动就是「用户按了、什么都不发生、然后自己猜」，正是待办 30 立项时那个病；
   设置页那条是只读说明行，上游本来就有一句降级文案，两处形状不同。
   **已知残留**（与第 9 条同族）：存储里已经是九键的老用户，这里没有卡片会高亮
   ——实际键盘本来就是 26 键，去「输入」页换一次方案再换回来即可复位。
   native 导出仍是 102 个（本轮没加符号；Rust 侧改的是 `assemble.rs` 的回显**内容**，不是接口）。

12. `host/java/app/msime/android/home/KeyboardOptionsPage.java` 改了 **2 行**（1 处，2026-10-19 轮 AN 用户拍板）
   ——把「手势」组里的**「滑行输入」开关行隐藏**：原来那句 `gestures.toggle(...)` 前多接一行拿到
   `GroupCard.Row`、后面加一行 `glideRow.view().setVisibility(View.GONE)`，只改渲染、不碰逻辑与设置存储。
   理由：滑行在引擎侧是**刻意的** `no_op`（Rust 那份导出的注释写明了"抬手什么也不上屏"），
   开着它划动就是"按了没反应、让用户自己猜"——v1.0 不收这种入口，留待 1.1 要么真做要么把设置项一起清掉。
   **为什么是隐藏而不是删那三行**：设置键 `platform.android.glide_typing` 与 `ImeGlideTyping` 的读取路径都不动，
   删行会把"这一项从此不存在"和"这一项还在、只是没人能改"两种语义混在一起。
   **已知残留（诚实记）**：若某台设备在这之前手工打开过这个开关，入口隐藏后它**没有 UI 可以再关掉**，
   只能清应用数据或重装。本机在轮 AM 测完已复位成关，本轮装机冒烟也实测它是关的。
   **账目不变**：这个文件在第 9 条（设置页那行钉成 26 键）里就已经算进 differ 集合，
   本轮复测 `diff -rq` 上游 java 树仍是 **differ 12 / java 文件 394 个 ⇒ 382 个逐字未改**，
   native 导出仍是 102 个（本轮没加符号，改的是 Java 的一处渲染）。

## 随包数据

`host/assets/dictionary/dict.tsv`（2,053,612 B）是青简的拼音词库产物，其源数据与各自的许可
按青简 `docs/design/landscape.md` 的数据来源表：

- 《通用规范汉字表》8105 字、《现代汉语常用词表（草案）》56k 词（教育部 2008 的 liuxilu
  校对版，自带拼音）——官方文件的转录，转录仓库未附许可证；
- THUOCL 领域词（清华大学自然语言处理实验室，**MIT**）；
- jieba 分词词表（fxsjy/jieba，**MIT**，用于缺失高频词对照与词频折算）；
- 读音取自 **Unihan**（Unicode License v3）。

`host/assets/dictionary/glossary-en.tsv`（**现值 2,192,288 B / 90,380 数据行**；2026-10-13 A1 之前是
1,815,026 B / 76,273 行）是青简英文释义表的**子集**，
由 `engine/tools/build_glossary_subset.py` 从青简 `assets/glossary/glossary-en.tsv`
（6,788,267 B，232,213 行）裁出：**A1 起取词库全量（带词频的 93,236 行）与全量译词表的交集**
（此前是"按词频降序的前 80,000 个词"，那个 80k 是个会无声掉覆盖的活雷，已在脚本里改成动态上限），
再追加 A3 手写整句源件 2,031 条。
许可与来源按青简 `assets/glossary/README.md` 原话：`:9`「本目录各表随代码以
**GPL-3.0-or-later** 发布，与仓库一致」，`:13`「由 `tools/gloss-gen` 用 LLM 离线批量生成
（英语 / 日语用 DeepSeek，越南语用 gpt-5.5），**不含任何第三方词典内容**」。
本项目的裁剪不改写任何一行释义内容，逐行原样搬运，因此不产生新的衍生数据问题。

同表还记录了一条与本项目直接相关的取舍：青简在 2026-09-05 移除雾凇拼音，理由是它标的是
**GPL-3.0-only**、与青简自身 GPL-3.0-or-later 的收口方式不兼容。本项目因此逐项核对
「-only」与「-or-later」的差别：水杉宿主是 GPL-3.0-only，它按**自己的**原条款留在包里，
除这两个上游之外没有再引入任何 GPL-3.0-only 的代码或数据。
「水杉 GPL-3.0-only 与青简 GPL-3.0-or-later 合并后以 GPL-3.0-or-later 分发」这一结论取自
`prd.md:63`（技术可行性分析一节），属产品决定，本文件只是照它执行并留痕，不构成法律意见。
`host/assets/desktop-dictionary.lock.json`（**现值 167 B**；早期只有 `name` 时是 52 B，2026-10-13 起加了 `bytes`）是本项目为满足水杉 `Bootstrap` 解包校验而写的锁文件，
不含第三方内容。

本项目**不打包**水杉的日文词库（Mozc / IPAdic）、英文 SCOWL 词表、语言词库与离线释义——
那些资源随上游各自的锁文件按需下载，而其 native 入口在本项目属于 **81 个** `not_implemented` 桩（102 个导出减去真实现的 21 个；计数口径 = `engine/tools/gen_stub.py` 的 `REAL` 集合，2026-10-16 20:40 实测 21 + 81 = 102），
因此相应分发义务不落在本包内（见 `README.md` 已知失效项第 1 条）。

## 运行时第三方组件

debug APK 里除自有代码外，实际打进去的是 `host/gradle-app/app/build.gradle.kts` 声明的依赖，
按包内条目可核对为这些组：AndroidX（appcompat / core / activity / constraintlayout /
recyclerview / viewpager2 / credentials / lifecycle / room …）、Material Components、
Google ML Kit `digital-ink-recognition`（含 `libdigitalink.so`，arm64-v8a 6,891,488 B、
x86_64 8,771,480 B）、Markwon core（`org.commonmark`）、OkHttp、Kotlin 标准库与
Google Play services / Firebase 传输组件。

各组件按**自己的**许可证分发，不由 GPL 覆盖；上游清单以水杉 `docs/third-party.md` 为准，其中手写识别
那一项与水杉 README `:138` 的说法一致：**Google ML Kit 按其服务条款授权，不是开源许可证**。
本项目没有修改这些库的任何源码，只是按坐标链接。

### Rust 侧（链接进 `libmsime_android.so` 的 crate）

**本节 2026-10-16 才补上**：上面那段"运行时第三方组件"此前只覆盖 Gradle/Java 一侧，
`.so` 里链接的 Rust crate 一个都没列——这是这份声明的一个既有缺口，不是本轮改动造成的，
本轮只是把它补住。`engine/Cargo.toml` 直接声明的八个，连同锁定的版本与各自许可证：

| crate | 锁定版本 | 许可证 |
|---|---|---|
| `jni` | 0.22.4 | MIT OR Apache-2.0 |
| `serde` | 1.0.229 | MIT OR Apache-2.0 |
| `serde_json` | 1.0.152 | MIT OR Apache-2.0 |
| `thiserror` | 2.0.21 | MIT OR Apache-2.0 |
| `tracing` | 0.1.44 | MIT |
| `libc`（仅 `cfg(unix)`，剪贴板档的 `O_NOFOLLOW` / `0o600` 常量） | 0.2.190 | MIT OR Apache-2.0 |
| `rustix`（仅 `cfg(target_os = "android")`，剪贴板档的 flock） | 1.1.5 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| `unicode-segmentation`（剪贴板文字的图素簇上限） | 1.13.3 | MIT OR Apache-2.0 |

再加青简 Core 的六个 path 依赖（`qingjian-core` / `-dictionary` / `-format` / `-learning` /
`-lm` / `-translate`，许可证见 `LICENSE-QINGJIAN`）。**它们的传递闭包以 `engine/Cargo.lock` 为准**
（实测 110 个包），本表不逐条重述——重述就会腐烂。2026-10-16 提直接依赖这一次，
**锁里真正新增的只有 `unicode-segmentation` 一个包**：`libc` 与 `rustix`（以及只被 `rustix`
拉进来的 `errno`、`linux-raw-sys`）本来就在锁里（`rustix` 经 `tempfile`、`libc` 经
`getrandom` 与 `memmap2`），本轮只是从"在锁里但没链进 `.so`"变成"被我们的代码直接调用"。
