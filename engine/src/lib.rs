//! 青杉 JNI 适配层。
//!
//! 产物名 `libmsime_android.so`，顶替水杉原来由 C++ 胶水层
//! `platforms/android/native/client_jni.cpp`
//! 编译出的那个库。Java 侧 `NativeClient.java:36` 硬编码 `System.loadLibrary("msime_android")`，
//! 而 102 个 `private static native` 方法全靠 JNI 命名约定查找符号（上游没有
//! `JNI_OnLoad`、也没有 `RegisterNatives`），所以本 crate 必须把**这 102 个符号全部导出**：
//! 少一个就是 `UnsatisfiedLinkError`，而 `:ime` 进程里并非每个调用点都 catch 了
//! `LinkageError`。
//!
//! 分工：
//! - 本文件：28 个真实现——输入路径的 8 个方法，加中/英模式切换 `setEnglishModeRaw`
//!   （它被宿主的启动路径与中/英键硬解析，桩会让会话建不起来）、候选长按里的「优先显示」
//!   `pinCandidateRaw`（走 `Learner` 记一次选择，见 `pin_candidate`）与另外三件
//!   `fixCandidatePositionRaw` / `clearCandidatePositionRaw` /
//!   `selectEdgeRaw`（Core 无对应能力，回「原样 View + `handled:false`」止血）、
//!   `removeCandidateRaw`（接 `Engine::forget`）、展开面板 `allCandidatesRaw`
//!   与它唯一的写入口 `selectAnyCandidateRaw`，`refreshHostRaw`（比随包锁的长度，
//!   让升级安装包能重新解包），偏好读写方案丙的三件：`loadPreferencesRaw` 读、
//!   `savePreferencesRaw` 写、`updatePreferencesRaw` 把快照画回界面
//!   （三者必须成对，理由见 `preferences.rs`），以及剪贴板共享存储的
//!   `mobileClipboardHistoryRaw`（切片 1：只接 `load` 与 `capture`，
//!   其余四个操作仍回 `not_implemented`），以及隐私开关 `setPrivateSessionRaw`
//!   （把宿主的私密标记交给 `Engine::set_private`，见 `session::mark_private`）。
//!   **计数以 `tools/gen_stub.py` 的 `REAL` 集合为准。**
//! - `stub.rs`：其余 74 个，由 `tools/gen_stub.py` 从 Java 声明生成，回 `not_implemented`。
//! - `session.rs`：会话表 + 宿主游标；`keys.rs`：键码与命令号分派；`view.rs`：JSON 镜像；
//!   `wire.rs`：信封与 `byte[]` 收发；`resources_lock.rs`：随包锁的解析与长度比对；
//!   `preferences.rs`：偏好档的读写（原子写 + 等值 CAS）、应答体的字段要求与
//!   `learning` 单向门；`clipboard.rs`：剪贴板历史档的读写与它的 at-rest 防护。
//!
//! panic 全部由 `wire` 里的 `with_env` 兜（`jni-0.22.4/src/env.rs:4801` 的 `catch_unwind`），
//! 所以本文件的每个导出都不需要自己包 `catch_unwind`。

mod assemble;
mod clipboard;
mod keys;
mod logcat;
mod preferences;
mod resources_lock;
mod session;
mod view;
mod wire;

#[rustfmt::skip]
mod stub;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use jni::EnvUnowned;
use jni::objects::JObject;
use jni::sys::{jboolean, jbyteArray, jint, jlong};
use serde_json::json;

use assemble::{DataPaths, HostRequest, prepare_host};
use keys::{CommandOutcome, apply_command, char_for};
use session::{Session, create_session, destroy_session, with_session};

// 下面 9 个签名逐位对齐 NativeClient.java:594-704 的声明。

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_prepareHostRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, options: jbyteArray,
) -> jbyteArray {
    // Bootstrap.java:43-44 发 {"resources","state_root"}，:57 把我们回的 value 原样
    // 写进 runtime-options.json，故这里必须回显原值（理由见 assemble.rs 的模块文档）。
    // 这是宿主第一个桥调用，日志桥在这里装（幂等，见 logcat.rs）。
    logcat::init();
    let Some(text) = wire::read_text(&mut env, options) else {
        return wire::not_implemented(&mut env);
    };
    let Some(request) = host_request(&text) else {
        return wire::fail(&mut env, "malformed_prepare_request");
    };
    match prepare_host(&request) {
        Ok(resources) => match serde_json::to_value(resources) {
            Ok(value) => wire::ok_value(&mut env, value),
            Err(error) => wire::fail(&mut env, &format!("encode_failed: {error}")),
        },
        Err(error) => wire::fail(&mut env, &format!("prepare_failed: {error}")),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_createRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, options: jbyteArray,
) -> jbyteArray {
    // Java 读的是 View：`view = value(create(..)); session = strictCandidateLong(view,"session")`
    // （MSIMEInputService.java:1698-1699），所以这里的 value 是 View，不是 Transition。
    logcat::init();
    let Some(text) = wire::read_text(&mut env, options) else {
        return wire::not_implemented(&mut env);
    };
    let Some(request) = host_request(&text) else {
        return wire::fail(&mut env, "malformed_create_request");
    };
    let paths = data_paths(&request);
    let identity = session_identity(&text);
    match create_session(&paths, &identity) {
        Ok(id) => match with_session(id, |session| build_view(session, id)) {
            Some(view) => match serde_json::to_value(view) {
                Ok(value) => wire::ok_value(&mut env, value),
                Err(error) => wire::fail(&mut env, &format!("encode_failed: {error}")),
            },
            None => wire::fail(&mut env, "session_vanished"),
        },
        // 词库缺失/损坏时 Bootstrap 会显示准备失败并允许重试，比默默起一个空会话好。
        Err(error) => wire::fail(&mut env, &format!("create_failed: {error}")),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_destroyRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, session: jlong,
) -> jbyteArray {
    // Java 丢掉返回值（MSIMEInputService.java:1598 只 catch LinkageError），
    // 但 TextPolicy.utf8 会先解出字符串，所以仍要给一个能解析的信封。
    destroy_session(session as u64);
    wire::ok_value(&mut env, json!({ "handled": true }))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_focusRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, session: jlong, focused: jboolean,
) -> jbyteArray {
    transition(&mut env, session as u64, |state| {
        state.focused = focused;
        CommandOutcome::Consumed
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_setPrivateSessionRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, session: jlong, enabled: jboolean,
) -> jbyteArray {
    // 待办 33 的第二个入口：宿主有三条路走到这里，其中两条**不重建会话**——建会话之后
    // （`MSIMEInputService.java:1710`）、键盘重弹时按新的 EditorInfo 重算（`onStartInputView` 的
    // `:1384`）、偏好快照之后（`:2261`）。只靠 `createRaw` 那半条盖不住第二条：同一个会话
    // 从普通框挪到密码框时 Java 不换 id。返回值在 `markPrivateSession()`（`:1912-1914`）被丢弃。
    //
    // **形状刻意不是 `transition`**：`run_transition` 在动作之后 `advance()` 一次
    // （`session.rs:278`），而代次一变，`candidate_index` 的等值校验就把宿主手里**每一个候选 id**
    // 判成过期（`session.rs:322-328`）——这条通道每次键盘重弹都会走一遍，等于每弹一次键盘，
    // 「优先显示 / 删除词条」这类按 id 的入口当场全废。真机探针实测到 `in gen=21 mine=28`。
    // 所以这里只改状态、回一个最小信封（`destroyRaw` 同形），不涨代次、不重画面板。
    let marked = with_session(session as u64, |state| {
        session::mark_private(state, enabled);
    });
    match marked {
        Some(()) => wire::ok_value(&mut env, json!({ "handled": true })),
        None => wire::not_implemented(&mut env),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_characterRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, session: jlong, ascii: jint, shift: jboolean,
) -> jbyteArray {
    let character = char_for(ascii, shift);
    transition(&mut env, session as u64, move |state| match character {
        Some(character) => {
            state.engine.push(character);
            CommandOutcome::Consumed
        }
        // 不是可打印字符：交回宿主，Java 会自己 commitText。绝不能回 ok:false——
        // 那会让 apply() 抛 JSONException 并把整个会话降级（见 wire.rs 文件头）。
        None => CommandOutcome::Passthrough,
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_commandRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, session: jlong, command: jint,
) -> jbyteArray {
    transition(&mut env, session as u64, move |state| {
        apply_command(state, command)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_selectRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, session: jlong, generation: jlong, index: jlong,
) -> jbyteArray {
    transition(&mut env, session as u64, move |state| {
        match state.candidate_index(generation as u64, index as usize) {
            Some(index) => CommandOutcome::Commit(state.engine.commit(&state.candidates[index])),
            // 代次或页窗口对不上：当作没处理，而不是错误。
            None => CommandOutcome::Passthrough,
        }
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_viewRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, session: jlong,
) -> jbyteArray {
    // 只读当前缓存，不重查：Java 用它取整页候选（如九键展开面板），重查会白白推进代次。
    let id = session as u64;
    match with_session(id, |state| build_view(state, id)) {
        Some(view) => match serde_json::to_value(view) {
            Ok(value) => wire::ok_value(&mut env, value),
            Err(error) => wire::fail(&mut env, &format!("encode_failed: {error}")),
        },
        None => wire::not_implemented(&mut env),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_setEnglishModeRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, session: jlong, enabled: jboolean,
) -> jbyteArray {
    // 两处调用点都把 `value` 当 View 读，不是 Transition：启动路径 `MSIMEInputService.java:1709`
    // 与中/英键 `:3750`，都是 `view = value(NativeClient.setEnglishMode(..))`。
    // 编排对齐上游 `runtime.rs:476-480` 的 `advance(); set_dedicated_english(); refresh()`。
    let id = session as u64;
    let built = with_session(id, |state| {
        let _ = state.advance();
        state.engine.set_english_mode(enabled);
        state.requery();
        build_view(state, id)
    });
    match built {
        Some(view) => match serde_json::to_value(view) {
            Ok(value) => wire::ok_value(&mut env, value),
            Err(error) => wire::fail(&mut env, &format!("encode_failed: {error}")),
        },
        None => wire::not_implemented(&mut env),
    }
}

/// 方案丙第一步的两条偏好桥之一。宿主每个会话生命周期都会经
/// `PreferencesReloader` 反复读它（`MSIMEInputService.java:1223` 起，1 秒一轮），
/// 桩回 `ok:false` 时 `value()`（`:844`）必抛 ⇒ 键盘上永久挂着「设置读取或应用失败」。
/// 第二步之后它**真读盘**：`<preferences_directory>/preferences.json`。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_loadPreferencesRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, directory: jbyteArray,
) -> jbyteArray {
    // 这一条有失败分支（坏文件要回 ok:false），所以日志桥要在这里也装一次，
    // 否则 `wire::fail` 的 warn 是静默 no-op（环境陷阱里 refreshHostRaw 那条同因）。
    logcat::init();
    // 目录读不出来时仍回默认文档：那说明宿主还没走到写 `preferences_directory` 那步
    // （`MSIMEInputService.java:1720` 之前），不是盘上有东西坏了。
    let Some(path) = wire::read_text(&mut env, directory) else {
        return wire::ok_value(&mut env, preferences::load_document());
    };
    match preferences::load(Path::new(&path)) {
        preferences::Loaded::Document(document) => wire::ok_value(&mut env, document),
        preferences::Loaded::Default => wire::ok_value(&mut env, preferences::load_document()),
        // 坏文件**不能**回 ok:true + 默认值：那会让下一轮 save 把事故现场直接覆盖掉。
        preferences::Loaded::Unreadable => wire::fail(&mut env, "preferences_unreadable"),
    }
}

/// 偏好落盘（方案丙第二步）。宿主在 5 个地方调它（`:4912`、`:5075`、`:5497`、`:5641`、`:7312`），
/// 传的是**整份快照**（`pending = new JSONObject(preferencesSnapshot.toString())`，`:4868`）
/// 加它读到的修订号，所以 `value` 必须回一份宿主能直接喂给 `applyPreferencesSnapshot` 的文档
/// ——它随后就读 `value.revision`（`:4970`）与 `value.preferences`（`:4975` 那条路径）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_savePreferencesRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, directory: jbyteArray,
    expected_revision: jlong, snapshot: jbyteArray,
) -> jbyteArray {
    logcat::init();
    let (Some(path), Some(body)) = (
        wire::read_text(&mut env, directory),
        wire::read_text(&mut env, snapshot),
    ) else {
        return wire::fail(&mut env, "preferences_arguments_unreadable");
    };
    if expected_revision < 0 {
        return wire::fail(&mut env, "negative_expected_revision");
    }
    match preferences::save(Path::new(&path), expected_revision as u64, body.as_bytes()) {
        preferences::Saved::Applied(document) | preferences::Saved::Conflict(document) => {
            // 冲突回 ok:true + **当前那份**，不是回失败：宿主本来就有这一支
            // （`PreferencesSavePolicy.shouldApplyResponse` 判"有更新的读盘赢了这场竞赛"，
            // 然后只提示「设置已更新」）。回 ok:false 会把一次良性的竞赛报成「保存失败，保留原设置」。
            wire::ok_value(&mut env, document)
        }
        preferences::Saved::Rejected(reason) => wire::fail(&mut env, reason),
    }
}

/// 剪贴板共享存储（切片 1：`load` 与 `capture`）。宿主把**六个**操作打包成一份
/// `{directory, action}` 请求从同一个入口递进来（`NativeClient.java:639` 只有一个
/// `mobileClipboardHistoryRaw`，不存在按操作拆开的符号），分派在 `clipboard::handle`。
/// 这里的 `directory` 是宿主自己的 `filesDir`（`MSIMEInputService.java:6799` 传的就是
/// `files`），**不是**偏好那个 `bootstrap/state`；落盘 `<filesDir>/MSIME/clipboard_history.json`
/// ——与上游同名同位置，因为 iOS / HarmonyOS / macOS 宿主读的是同一份档。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_mobileClipboardHistoryRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, request: jbyteArray,
) -> jbyteArray {
    logcat::init();
    let Some(body) = wire::read_text(&mut env, request) else {
        return wire::fail(&mut env, "mobile_clipboard_request_unreadable");
    };
    match clipboard::handle(body.as_bytes()) {
        clipboard::Reply::Ok(value) => wire::ok_value(&mut env, value),
        clipboard::Reply::Fail(reason) => wire::fail(&mut env, reason),
    }
}

/// 把宿主送来的整份偏好快照应用到这个会话，并回一份完整 View。
///
/// 这一条是「只做 load 消不掉那句提示」的另一半：`applyPreferencesSnapshot` 自己在
/// `MSIMEInputService.java:2260` 也要过一次 `value()`，`:2332` 硬取 `view`。
/// 本引擎目前只消费快照里的 `preferences.learning`（与 `createRaw` 那半同一个字段），
/// 其余偏好项由宿主自己应用（皮肤、字号、几何都在 Java 侧），所以这里不回 Transition。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_updatePreferencesRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, session: jlong, snapshot: jbyteArray,
) -> jbyteArray {
    logcat::init();
    let id = session as u64;
    let Some(text) = wire::read_text(&mut env, snapshot) else {
        return wire::not_implemented(&mut env);
    };
    let suppress_learning = preferences::wants_learning_off(&text);
    let built = with_session(id, |state| {
        if suppress_learning {
            state.identity.learning_allowed = false;
            // 光记在身份上不够：那只管挂起/换回（`session.rs` 的 `create_session` 与
            // `destroy_session` 读它），Core 那两个写开关得当场跟上——用户在设置里关掉
            // 「记忆新词」之后，当前这个会话的下一次上屏就不该再进 `user.tsv`。
            session::apply_learning_policy(&mut state.engine, false);
        }
        build_view(state, id)
    });
    match built {
        // 信封形态与 `viewRaw` **不同**：宿主在 `MSIMEInputService.java:2332` 读
        // `value(...).getJSONObject("view")`，在 `:2387` 读 `strictBoolean(result, "deferred")`，
        // 而 `strictBoolean` 对缺键也抛 ⇒ 两处都得给。字段要求集中在 `preferences::update_body`。
        Some(view) => match serde_json::to_value(view) {
            Ok(value) => wire::ok_value(&mut env, preferences::update_body(value, false)),
            Err(error) => wire::fail(&mut env, &format!("encode_failed: {error}")),
        },
        None => wire::not_implemented(&mut env),
    }
}

/// 宿主手里那张表的内容指纹：**逐格的候选文本**。
///
/// 判据只取文本，不取 `kind` / `syllables` / 译词——`candidates[i].text` 正是点中第 i 格
/// 会上屏的字（`selectRaw` 就是 `engine.commit(&state.candidates[index])`），文本一格没动
/// 就不可能出现「旧下标选中另一个词」；其余字段只影响这一格怎么画，而画的是我们这一次
/// 刚发过去的新 View。
///
/// 每次编排分配「表长 × 一个短字符串」两份（前后各一次）。这不是本编排的开销大头：
/// `requery()` 本身重建成品表，每条候选都带 `Vec<String>` 音节与 `Option<Translation>`。
fn table_texts(state: &Session) -> Vec<String> {
    state
        .candidates
        .iter()
        .map(|candidate| candidate.text.clone())
        .collect()
}

/// 一次「动作 → 内容变了才涨号 → 重查 → 出 Transition」的完整编排。
///
/// **顺序是这一处的核心修复（两代）。** 第一代：原先 `advance()` 无条件跑在动作**之前**，
/// 而动作内部的代次校验（`candidate_index` / `candidate_index_in_table`）比的是宿主传来的
/// `generation` 与 `self.generation` —— 先 +1 就永远不等，于是 `selectRaw` /
/// `removeCandidateRaw` / `selectAnyCandidateRaw` 三个带 id 的入口恒回 `handled:false`，
/// 宿主点候选从来不上屏。阶段一验收是用空格（`commandRaw` 命令 1，不带 id）提交「你好」的，
/// 所以没暴露。
///
/// 第二代（2026-10-18 轮 AH）：涨号条件从「outcome 是 `Consumed`/`Commit`」换成
/// **「重查出来的表逐格文本与动作前不同」**。理由是那条旧规则覆盖不到的三类调用
/// 只回传 UI 状态、一格内容都没换，却照样涨号：`focusRaw` 写 `focused`、
/// 点读音行移组字光标（`MSIMEInputService.java:3167-3179` 连发命令 4/5）、
/// 候选条翻页（命令 100/101）。每涨一次，宿主手里**每一个**候选 id 都被
/// `candidate_index` 判过期（`session.rs:322-328`），「优先显示 / 删除词条 / 点选」
/// 一起失效——待办 35 的真机红（`in gen=21 mine=28`）就是这一族。
///
/// 旧规则留着的那条不对称担心的是「漏涨 ⇒ 旧下标选中另一个字，上错字」，而新判据把它
/// 反着关死了：文本逐格相等时那是不可能事件；文本一变就涨，**哪怕动作回的是
/// `Passthrough`**（空组字下的 ⌫：`Engine::note_backspace()` 退掉上一笔学习，
/// 表会换，而那一支不涨是旧规则的漏涨）。
///
/// 表没换时顺带保住 `highlighted`：`requery()` 里的 `refresh()` 把游标归零
/// （`session.rs:288-291`），原先无条件重查会把命令 100/101 刚翻过去的那一页抹掉。
///
/// `advance()` 与 `requery()` 成对出现在这里而不是散进 `keys.rs`，对齐上游
/// `runtime.rs:321-324` 的 `advance(); refresh();`。锁只在这一个 `with_session` 里持有，
/// 所以动作与重查之间不会被别的 JNI 调用插入。
/// 代次耗尽（`u64::MAX`）时保留旧号继续：阶段一不可能到达，而回 `ok:false` 的代价是会话降级。
///
/// 单独成函数（而不是留在 `transition` 里）是为了让宿主 `cargo test` 能驱动这条编排：
/// 它以前和 JNI 序列化焊在一起，25 个测试全部绕过它，这个 bug 因此活完了整个阶段一。
fn run_transition(
    state: &mut Session,
    id: u64,
    action: impl FnOnce(&mut Session) -> CommandOutcome,
) -> view::Transition {
    let before = table_texts(state);
    let outcome = action(state);
    let committed = matches!(outcome, CommandOutcome::Commit(_));
    // 游标要在**动作之后、重查之前**取：命令 100-105 正是靠改 `highlighted` 来表达的，
    // 取早了会把刚翻过去的那一页又按回去（这条是测试逼出来的，见
    // `session::tests::a_page_turn_keeps_its_page_and_the_generation`）。
    let cursor = state.highlighted;
    state.requery();
    if table_texts(state) == before {
        state.highlighted = cursor;
    } else {
        let _ = state.advance();
    }
    // 学习数据落盘挂在提交上，但走 60 秒节流（`session.rs` 的 `LEARNING_FLUSH_INTERVAL`），
    // 且排在 `requery()` 之后——先把用户要的候选算出来，再碰磁盘。
    if committed {
        state.note_commit();
    }
    let (handled, commit, diagnostic) = match outcome {
        CommandOutcome::Consumed => (true, None, None),
        CommandOutcome::Commit(text) => (true, Some(text), None),
        CommandOutcome::Passthrough => (false, None, None),
        CommandOutcome::NotSupported(command) => {
            (false, None, Some(format!("not_supported:{command}")))
        }
    };
    let commit_context = commit.as_ref().map(|_| view::output_context());
    view::Transition {
        handled,
        commit,
        commit_context,
        diagnostic,
        view: build_view(state, id),
    }
}

fn transition<F>(env: &mut EnvUnowned<'_>, id: u64, action: F) -> jbyteArray
where
    F: FnOnce(&mut Session) -> CommandOutcome,
{
    let built = with_session(id, |state| run_transition(state, id, action));
    match built {
        Some(transition) => match serde_json::to_value(transition) {
            Ok(value) => wire::ok_value(env, value),
            Err(error) => wire::fail(env, &format!("encode_failed: {error}")),
        },
        // 会话不在（id 已销毁）：桩语义，回 not_implemented。
        None => wire::not_implemented(env),
    }
}

/// 止血站点共用的空操作应答：候选长按三件 `fixCandidatePosition` / `clearCandidatePosition` /
/// `selectEdge`，加 `glide` / `setNineKeyFilter`。
/// （第四件 `pinCandidate`「优先显示」已于 2026-10-18 转真，走 `Learner` 记一次选择，见 `pin_candidate`。）
///
/// 为什么不是桩：这五个调用点的 catch 体都是 `fail()`（`MSIMEInputService.java:6046`、
/// `:6243`、`:3147`、`:3160`），回 `not_implemented` 等于「点一下就死会话」。青简 Core
/// 没有对应能力（逐个核实见 `third-party/PIN.md`「会话建立后的 stub 降级面」与
/// 「剩余 3 个杀会话站点」），先止血。
///
/// 只读会话：不 `advance`、不 `requery`。代次一变，宿主会按
/// `(session, generation, page)` 把候选条滚回最左（`MSIMEInputService.java:8101-8122`）。
/// `handled:false` 在这六件上都安全：六个调用点全把 `apply()` 的返回值丢掉。
fn no_op(env: &mut EnvUnowned<'_>, session: jlong) -> jbyteArray {
    let id = session as u64;
    match with_session(id, |state| build_view(state, id)) {
        Some(view) => wire::no_op_view(env, &view),
        None => wire::not_implemented(env),
    }
}

/// 「优先显示」= 用户主动多选了一次这个候选，所以记的就是上屏时那两条学习记录。
///
/// **为什么两条都得记**（`crates/qingjian-core/src/ranking/scored.rs:38` 的 `SortKey` 写得很清楚）：
/// 排序键第 5 位是 `Reverse(choice)`（同一输入串下选过的次数），第 6 位才是含
/// `weight_bonus` 的上下文分数 ⇒ **只记 `Learner::record`（词频）在紧对手身上翻不了位**，
/// 要置顶必须记 `record_choice`。上屏那条路也正是两条一起调
/// （`crates/qingjian-core/src/engine/commit/mod.rs:112-114`）。
///
/// `choice` 的键：查询时用的是"当前作用域里被这个候选覆盖的那段原样字母"
/// （`crates/qingjian-core/src/engine/query/phonetic.rs:146` 取 `choice_key(scope, scope.len())`、
/// `:155` 再按覆盖长度切前缀），所以我们从**作用域**切，不用候选自带的音节表——
/// 理由写在下面的注释里（前缀候选会记错键）。简拼与纠错生效时 Core 用的是原串前缀，
/// 我们同样切原串 ⇒ 一致；只有"音节字母数超过作用域"这一种切法会与 Core 相同而非更差。
/// 音节为空的候选（整句、英文、emoji）**直接交回宿主**：Core 上屏它们时本来也不记词频。
///
/// 代次不在这里碰，由 `transition` 统一推进；推进后宿主按新 View 重画，
/// 被置顶的那个当场就排到第一格。
fn pin_candidate(state: &mut Session, generation: u64, index: usize) -> CommandOutcome {
    if !state.identity.learning_allowed {
        // 私密框或用户关了「记忆新词」：不写学习数据。回 `Passthrough` 让宿主弹
        // 「当前候选不支持此操作」（`MSIMEInputService.java:6043`），那是实话——
        // 比假装播报「已优先显示」诚实。
        return CommandOutcome::Passthrough;
    }
    let Some(index) = state.candidate_index(generation, index) else {
        return CommandOutcome::Passthrough;
    };
    // 先 clone 再动 engine，否则 `&state.candidates` 与 `&mut state.engine` 同源借用冲突
    // （`remove_candidate` 同一手法）。
    let candidate = state.candidates[index].clone();
    if !matches!(candidate.kind, qingjian_core::CandidateKind::Chinese) {
        // 整句 / 英文 / emoji 候选没有"词频"可提：Core 上屏它们时本来就不调 `record`
        // （`crates/qingjian-core/src/engine/commit/mod.rs:110-130`），这里也不假装置顶了。
        return CommandOutcome::Passthrough;
    }
    // 键 = 当前作用域里**被这个候选覆盖的那段**原样字母（去掉隔音符号），与 Core 上屏时的
    // `choice_key(scope, consumed)` 同一条规则（查询侧 `crates/qingjian-core/src/engine/query/phonetic.rs:146`
    // 与 `:155` 就是拿这段去查 `choice_weight` 的）。
    // **不能直接把音节表拼起来**：`nihao` 下会出前缀候选「你好像」（音节 ni hao xiang），
    // 但它这次只覆盖 `nihao`——按音节拼就记到一个查询永远取不到的键上，
    // 表现为"点了优先显示，下次还是没置顶"。
    let raw = state.engine.composition().scope();
    let letters: usize = candidate.syllables.iter().map(|s| s.chars().count()).sum();
    let key: String = raw
        .chars()
        .take(letters.min(raw.chars().count()))
        .filter(|c| *c != '\'')
        .collect();
    let learner = state.engine.learner_mut();
    learner.record(&candidate);
    if !key.is_empty() {
        learner.record_choice(&key, &candidate.text);
    }
    CommandOutcome::Consumed
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_pinCandidateRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, session: jlong, generation: jlong, index: jlong,
) -> jbyteArray {
    // 边界用页窗口（与 `removeCandidate` 同一条上游规则：`crates/input-runtime/src/runtime.rs:1796-1811`
    // 把 `Pin`/`Fix`/`Clear`/`Remove`/`SelectEdge` 放在同一个 `if let` 组里按当页校验）。
    transition(&mut env, session as u64, move |state| {
        pin_candidate(state, generation as u64, index as usize)
    })
}

/// 待办 29 的 (b)(c) 两件**仍停在止血**：Core 没有"置顶/固定位置"的位可存
/// （`Candidate` 六个字段、引擎 `pub fn` 全清单、`Learner` trait 三处都查过），
/// 要做「固定到首位 / 取消固定」得在我们这边自建固定表，并先读上游的固定语义
/// （持久化吗、按词还是按读音、与调频怎么合）。见 PIN 待办 29。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_fixCandidatePositionRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, session: jlong, _generation: jlong,
    _index: jlong, _position: jint,
) -> jbyteArray {
    no_op(&mut env, session)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_clearCandidatePositionRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, session: jlong, _generation: jlong, _index: jlong,
) -> jbyteArray {
    no_op(&mut env, session)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_selectEdgeRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, session: jlong, _generation: jlong,
    _index: jlong, _edge: jint,
) -> jbyteArray {
    // `handled:false` 是这条桥的正当出口：Java 拿到 false 后自己走
    // `CandidateTextPolicy.fallbackCommit` + `command(3)` + `commitText`
    // （`MSIMEInputService.java:6236-6240`；`NativeClient.java:520-523` 的 javadoc 写明
    // 「the caller falls back to the candidate's own text」）。回 true 会让它提前 return、
    // 把兜底跳过，什么也不上屏。`command(3)` 我们是真实现（`keys.rs:79-83`）。
    no_op(&mut env, session)
}

/// `removeCandidate` 的判定：页窗口内、代次对得上，且 Core 真删掉了东西，才算处理过。
/// 单独成函数是为了能在宿主 `cargo test` 里驱动它——导出层要 JVM 才跑得动。
/// 代次不在这里碰，由 `transition` 统一推进。
fn remove_candidate(state: &mut Session, generation: u64, index: usize) -> CommandOutcome {
    let Some(index) = state.candidate_index(generation, index) else {
        return CommandOutcome::Passthrough;
    };
    // 先 clone 再动 engine，否则 `&state.candidates` 与 `&mut state.engine` 同源借用冲突
    // （`keys.rs:72` 同一手法）。
    let candidate = state.candidates[index].clone();
    // `Forgotten::is_nothing()` = 词库里的普通词且没学过 ⇒ 没什么可删。这时回 `handled:false`
    // 让宿主弹「当前候选不支持此操作」（`MSIMEInputService.java:6042`），不假装删掉了。
    if state.engine.forget(&candidate).is_nothing() {
        CommandOutcome::Passthrough
    } else {
        CommandOutcome::Consumed
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_removeCandidateRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, session: jlong, generation: jlong, index: jlong,
) -> jbyteArray {
    // 边界用**页窗口**：上游把 `RemoveCandidate` 与 `Select`/`PinCandidate`/
    // `FixCandidatePosition`/`ClearCandidatePosition`/`SelectEdge` 放在同一个 `if let` 组里，
    // 按 `(highlighted / page_size) * page_size` 校验
    // （`crates/input-runtime/src/runtime.rs:1796-1811`）；只有 `SelectAnyCandidate` 走全表
    // （`:1788-1794`）。所以这里复用 `Session::candidate_index`，不新增校验函数。
    transition(&mut env, session as u64, move |state| {
        remove_candidate(state, generation as u64, index as usize)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_allCandidatesRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, session: jlong,
) -> jbyteArray {
    // 只读，而且**不走 `transition`**：它会把 generation 推进一格，而宿主拿这次应答的
    // `generation` 去和它手上那份 View 比（`sameCandidateVersion`，
    // `MSIMEInputService.java:6015-6024`），一动就永远不等、面板静默打不开。口径同 `viewRaw`。
    let id = session as u64;
    match with_session(id, |state| {
        view::all_candidates_view(&snapshot_of(state, id))
    }) {
        Some(body) => match serde_json::to_value(body) {
            Ok(value) => wire::ok_value(&mut env, value),
            Err(error) => wire::fail(&mut env, &format!("encode_failed: {error}")),
        },
        None => wire::not_implemented(&mut env),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_selectAnyCandidateRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, session: jlong, generation: jlong, index: jlong,
) -> jbyteArray {
    // 与 `selectRaw` 唯一的差别是边界：展开面板画的是全表，点的候选通常在当页之外，
    // 所以走 `candidate_index_in_table`（上游 `runtime.rs:1788-1794`），不查页窗口。
    transition(&mut env, session as u64, move |state| {
        match state.candidate_index_in_table(generation as u64, index as usize) {
            Some(index) => CommandOutcome::Commit(state.engine.commit(&state.candidates[index])),
            None => CommandOutcome::Passthrough,
        }
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_refreshHostRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, path: jbyteArray,
) -> jbyteArray {
    // 入参是 runtime-options.json 的绝对路径（`NativeClient.java:41`）。宿主
    // `Bootstrap.java:374-377` 只在 error 以 `dictionary_outdated` 开头时才重新解包，
    // 所以「配置读不出」绝不能带这个前缀——四种情况的分档与单测都在 `resources_lock.rs`，
    // 这一层只做信封（信封要 JVM 才跑得动，宿主测试测不到）。
    // 这一条可能就是本进程的第一个桥调用（升级路径不走 prepareHostRaw），所以日志桥
    // 要在这里也装一次，否则下面 `wire::fail` 的 warn 是静默 no-op——真机实测过：
    // 不装时宿主那行 `Runtime options refresh failed` 有，我们自己的原因行没有。
    logcat::init();
    let Some(text) = wire::read_text(&mut env, path) else {
        return wire::fail(&mut env, "refresh_path_unreadable");
    };
    match resources_lock::refresh_with_packaged_lock(Path::new(&text)).error() {
        None => wire::ok_value(&mut env, json!(null)),
        Some(reason) => wire::fail(&mut env, &reason),
    }
}

/// 会话 → 快照。`build_view` 与 `allCandidatesRaw` 共用，两条路径读同一份游标。
fn snapshot_of(state: &Session, id: u64) -> view::Snapshot<'_> {
    view::Snapshot {
        id,
        generation: state.generation,
        focused: state.focused,
        english: state.english_mode(),
        editing: state.editing(),
        caret: state.caret(),
        page: state.page(),
        page_size: state.page_size,
        page_count: state.page_count(),
        highlighted: state.highlighted,
        fullwidth: state.fullwidth,
        candidates: &state.candidates,
    }
}

/// 把会话当前状态拼成宿主 View。序列化留给调用点（createRaw/viewRaw 要 `to_value`，
/// transition 要把 View 塞进 Transition.view）。
fn build_view(state: &Session, id: u64) -> view::View {
    view::build_view(&snapshot_of(state, id))
}

fn host_request(text: &str) -> Option<HostRequest> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let resources = value.get("resources")?.as_str()?;
    let state_root = value
        .get("state_root")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    Some(HostRequest {
        resources: PathBuf::from(resources),
        state_root: PathBuf::from(state_root),
    })
}

/// 从建会话的 options 里解出输入框身份（方案甲-2 的挂起键）。
///
/// 键是**包名**：Java 那边只有 `EditorInfo.packageName` 能跨「离开输入框再回来」存活，
/// 所以粒度是 app 级，同一个 app 里两个输入框共用一份挂起上下文（已记录为甲-2 的已知限制）。
/// `preferences.learning == false` 是「这个框不许个性化学习，或隐私模式开着」
/// （`MSIMEInputService.java:1905-1907` 的 `learningSuppressed()`，在 `:1298` 写进这份 JSON），
/// 这样的会话既不换回也不挂起——理由见 `session::SessionIdentity::learning_allowed`。
/// 解不出键时回 `Default`（不挂起也不换回），行为与引入本特性之前逐字相同。
fn session_identity(text: &str) -> session::SessionIdentity {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return session::SessionIdentity::default();
    };
    let application = value
        .get("application")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let learning_allowed = value
        .get("preferences")
        .and_then(|preferences| preferences.get("learning"))
        .and_then(|learning| learning.as_bool())
        .unwrap_or(true);
    session::SessionIdentity {
        application,
        learning_allowed,
    }
}

/// 设备上的词库路径由 `resources` 拍平推出（`Bootstrap.java:259-260`：
/// `assets/dictionary/<name>` → `resources/<name>`，没有 `dictionary/` 子目录）；
/// 学习数据落在 `<filesDir>/qingshan/user.tsv`（PIN.md 环境陷阱第 4 条），
/// `filesDir` 由 `resources` 去掉尾部的 `bootstrap/resources` 得到。
fn data_paths(request: &HostRequest) -> DataPaths {
    DataPaths {
        dict_path: request.resources.join("dict.tsv"),
        glossary_path: request.resources.join("glossary-en.tsv"),
        user_tsv: files_dir(&request.resources)
            .join("qingshan")
            .join("user.tsv"),
    }
}

fn files_dir(resources: &Path) -> PathBuf {
    let bootstrap = resources.parent();
    match bootstrap {
        Some(bootstrap) if bootstrap.file_name() == Some(OsStr::new("bootstrap")) => {
            bootstrap.parent().unwrap_or(bootstrap).to_path_buf()
        }
        Some(bootstrap) => bootstrap.to_path_buf(),
        None => resources.to_path_buf(),
    }
}

/// 「回声当前 View」这一类应答的应答体：**整个 `value` 就是 View 本身**。
///
/// 宿主那两个消费点写的都是 `view = value(..)`（
/// `host/java/app/msime/android/core/MSIMEInputService.java:4451` 与 `:4465`），
/// 和 no_op 那五件走的 `apply(..)` 不是同一种形状——后者读的是 `value.view` 与 `value.handled`。
/// 两种形状混用**不会崩**：把 View 套一层再回，宿主拿到的 `view` 里 32 个键全落到默认值上，
/// 于是方案、`local_mode`、候选面一起读成"没有会话"，键盘外观静默错掉。所以形状单独成函数、
/// 单独有测试。
fn echo_view_body(view: &crate::view::View) -> Option<serde_json::Value> {
    serde_json::to_value(view).ok()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_setCharacterWidthRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, session: jlong, fullwidth: jboolean,
) -> jbyteArray {
    // 戊-C：把宽度**记进会话**再回声。宿主 `MSIMEInputService.java:4451-4453` 拿到回声后会用
    // `character_width` 反向覆盖它自己的 `fullWidthInput`——回声诚实，那个覆盖才是恒等操作。
    // 刻意不做的"naive 版"是：不记状态、回一个恒半角的 View，那等于每次建会话都把用户的全角打回半角。
    //
    // 治的是哪一件：桩回 `ok:false` ⇒ `value()` 抛 ⇒ 同文件 `:4454-4456` 弹
    // 「全角状态未能同步到输入引擎」⇒ 点亮 `hasDiagnostic`（`:7804`）⇒ 标题行 / 工具栏行 /
    // 候选行 / 候选视口四行一起隐藏 4 秒。它与标点那件是**同一处连坐的两个触发器**，
    // 而且它先抛（`:1712` 早于 `:1713`），所以只做甲只是换了屏幕上那句话。
    let id = session as u64;
    let built = with_session(id, |state| {
        state.fullwidth = fullwidth;
        build_view(state, id)
    });
    match built.and_then(|view| echo_view_body(&view)) {
        Some(value) => wire::ok_value(&mut env, value),
        None => wire::not_implemented(&mut env),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_setChinesePunctuationRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, session: jlong, enabled: jboolean,
) -> jbyteArray {
    // 甲（止血）+ 待办 28（接线）两件事，治的不是同一个问题：
    //
    // 甲治的是连坐：桩回 `ok:false` ⇒ 宿主 `value()` 抛 ⇒
    // `qingshan/host/java/app/msime/android/core/MSIMEInputService.java:4467`
    // 弹「标点状态未能同步到输入引擎」⇒ 点亮 `hasDiagnostic` ⇒ 标题行 / 工具栏行 /
    // 候选行 / 候选视口四行一起隐藏。所以下面**不 advance、不 requery**，只回声当前 View。
    //
    // 待办 28 治的是"上一轮的断言错了"：青简 Core **有**中文标点开关
    // （`qingjian/crates/qingjian-core/src/engine/setup.rs:9`），所以这里把宿主传来的值
    // 真的写进引擎。为什么值得写、以及它在设备上为什么看不见，都记在
    // `Session::set_chinese_punctuation` 的注释里，不在这儿重复。
    //
    // 标点面**不再只由宿主决定**：本轮起 `punctuationWithContextRaw` 转真，组字中按标点
    // 由我们先收尾再问 Core 转不转（见 `crate::keys::punctuation`）。Java 那个
    // `chinesePunctuation` 字段仍然决定键面（判定入口
    // `qingshan/host/java/app/msime/android/core/MSIMEInputService.java:3586`），
    // 两边靠下面这一次 `set_chinese_punctuation` 保持一致。
    //
    // 回声的形状必须是"整份 View"（`view = value(..)` 那类站点），不是 Transition
    // （`apply(..)` 那类）——混了不会崩，只会把 32 个 View 键静默读成默认值。
    logcat::init();
    let id = session as u64;
    let built = with_session(id, |state| {
        state.set_chinese_punctuation(enabled);
        build_view(state, id)
    });
    match built.and_then(|view| echo_view_body(&view)) {
        Some(value) => wire::ok_value(&mut env, value),
        None => wire::not_implemented(&mut env),
    }
}

/// 标点键。宿主在 `qingshan/host/java/app/msime/android/core/MSIMEInputService.java:3273`
/// 调它，签名 `(session, ascii, preceding)`——`preceding` 是宿主读来的光标前一个码点，
/// **我们不用它**，选路差异与理由全写在 `crate::keys::punctuation` 的注释里。
///
/// 这一件从桩转真治的是：组字中按标点 ⇒ 桩回 `ok:false` ⇒ 宿主 `apply()`
/// （同文件 `:2391`）抛 ⇒ 该调用点的 catch 调 `fail()` ⇒ 整个会话降级成「仅直接输入」。
/// 它属「启动路径的击穿判定」第五类（杀会话组），所以改完要复验三条路径。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_punctuationWithContextRaw(
    mut env: EnvUnowned<'_>,
    _class: JObject<'_>,
    session: jlong,
    ascii: jint,
    _preceding: jint,
) -> jbyteArray {
    logcat::init();
    transition(&mut env, session as u64, |state| {
        crate::keys::punctuation(state, ascii)
    })
}

/// `resetCache`。三个调用点里唯一**真做**的一件：候选缓存就是 `Session.candidates`，
/// 丢掉它再由 `run_transition` 重建，宿主那句「已清除候选缓存」才不是空话。
///
/// 入口只有硬件键盘和弦 Ctrl+Shift+Alt+C（判定入口
/// `qingshan/host/java/app/msime/android/policy/HardwareMaintenancePolicy.java:36`），
/// 触屏上没有这条路。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_resetCacheRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, session: jlong,
) -> jbyteArray {
    logcat::init();
    transition(&mut env, session as u64, |state| {
        crate::keys::reset_cache(state)
    })
}

/// 滑行输入（一笔划过的路径）。**止血，不是实现**：青简 Core 没有路径解码能力，
/// 真做要自建「点列 → 键序列 → 候选」的解码器（方案见 PIN「剩余 3 个杀会话站点」第二节）。
///
/// 回的是 `no_op` 那一份（`ok:true` + 当前 View + `handled:false`），与宿主自己那句注释
/// 「引擎不收（handled=false）时什么也不输入」（
/// `qingshan/host/java/app/msime/android/core/MSIMEInputService.java:3144`）逐字一致：
/// 抬手后什么也不上屏，但会话活着。这里的 `handled:false` 不触发命令 9——
/// `void glide(String)` 把 `apply()` 的返回值丢了（同文件 `:3145-3148`），
/// 与标点键那轮的情形不同（那里 `punctuation()` 的布尔被读）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_glideRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, session: jlong, _request: jbyteArray,
) -> jbyteArray {
    no_op(&mut env, session)
}

/// 九键候选的单字/笔顺筛选。**止血，不是实现**，而且这个站点在青杉**当前不可达**：
/// 布局由 View 决定（`touchLayout(value)` 读 `nine_key` 与 `touch_keyboard_layout`，
/// 同文件 `:3770-3776`），而 `qingshan/engine/src/view.rs:184` 恒 `nine_key = false`、
/// `:189` 恒 `TwentySixKey` ⇒ 九键面板永不出现，那五个调用点全是死路。
///
/// 真做筛选要先让 Core 有 T9 输入与笔顺数据（全域 `nine_key` 0 命中），属 PIN 里那条
/// 「View 的 12 个零值字段」的大面，不在本轮。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_setNineKeyFilterRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, session: jlong, _single_character: jboolean,
    _strokes: jbyteArray,
) -> jbyteArray {
    no_op(&mut env, session)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 甲-2 的输入框身份怎么从建会话选项里读出来：包名 + `preferences.learning`。
    /// 这两样都是 Java 已经写在选项里的（`phrase_preedit` 同一层），没有新增调用。
    #[test]
    fn identity_reads_the_package_and_the_learning_flag() {
        let text = json!({
            "resources": "/data/user/0/app.qingshan.ime/files/bootstrap/resources",
            "state_root": "/data/user/0/app.qingshan.ime/files/bootstrap/state",
            "phrase_preedit": true,
            "application": "com.example.chat",
            "preferences": { "learning": false },
        })
        .to_string();
        let identity = session_identity(&text);
        assert_eq!(identity.application.as_deref(), Some("com.example.chat"));
        assert!(
            !identity.learning_allowed,
            "learning=false 必须读成「这个框不许挂起」"
        );

        // 没写 `learning` 时按「允许」处理：漏读不能把普通输入框当成私密框。
        let open = session_identity(
            &json!({ "resources": "/x", "application": "com.example.chat" }).to_string(),
        );
        assert!(open.learning_allowed);
    }

    /// 读不到包名（老 Java、或 `packageName` 为空）时必须是「不挂起也不换回」，
    /// 而不是拿空串当键把两个 app 的上下文并成一份。
    #[test]
    fn identity_without_a_package_never_becomes_a_cache_key() {
        for text in [
            json!({ "resources": "/x" }).to_string(),
            json!({ "resources": "/x", "application": "" }).to_string(),
            "不是 JSON".to_string(),
        ] {
            let identity = session_identity(&text);
            assert_eq!(identity.application, None, "{text} 不该解出包名");
            assert!(
                identity.learning_allowed,
                "{text} 没写 learning，默认该是允许"
            );
        }
    }

    /// 标点回声的**形状**：整个 `value` 就是 View 本身，不带 `handled`、不再套一层 `view`。
    /// 这条锁的是"顺手抄 no_op 那五件的形状"——抄了不崩、不报错，只会让宿主把 View 的
    /// 32 个键全读成默认值（方案变 -1、`local_mode` 变 none、候选面跟着错），属静默降级。
    #[test]
    fn punctuation_echo_is_the_view_itself_not_a_transition_envelope() {
        let view = crate::view::mock_view();
        let echoed = echo_view_body(&view).expect("View 一定序列得出来");
        let object = echoed.as_object().expect("回声的顶层必须是对象");
        assert!(
            object.get("handled").is_none(),
            "标点回声走 `view = value(..)`，不是 Transition"
        );
        assert!(object.get("view").is_none(), "不该把 View 再套一层");
        assert_eq!(
            object.get("session").and_then(serde_json::Value::as_u64),
            Some(view.session),
            "会话号要原样带回去"
        );
        assert_eq!(
            object.get("scheme").and_then(serde_json::Value::as_u64),
            Some(view.scheme as u64)
        );
        // 判别器：把上面任何一处改成 `wire::no_op_body(..)`，这几条就会红。
        assert_ne!(
            echoed,
            wire::no_op_body(&view),
            "两种形状必须不同，相同就说明有人抄错了"
        );
    }

    /// `resetCache` 走完编排后的形状：`handled` 为真、表按当前组字重建，而且**不带
    /// diagnostic**——点亮 `hasDiagnostic` 会让候选条那四行整排消失
    /// （连坐面见 `third-party/PIN.md`「戊-A」那几节），一次清缓存不该换来这个。
    ///
    /// 代次**不涨**（2026-10-18 轮 AH 改的判据，原先这里断言"要涨一代"）：清掉的缓存
    /// 按同一份组字重建出来是逐格相同的一张表，宿主手里每个 id 仍指向同一个词。
    /// 涨号的唯一正当理由是「某个下标对应的词变了」。
    ///
    /// 判别器（实跑）：把 `keys::reset_cache` 的 `CommandOutcome::Consumed` 换成
    /// `Passthrough` ⇒ `handled` 这条当场红，后面两条**不会跑到**（panic 直接终止这个测试），
    /// 所以本测试里真正起作用的只有这一条。「表非空」那条**抓不到"没清"**——
    /// `run_transition` 无条件 `requery()`，清没清都会被重建，那条锁在
    /// `keys::tests::reset_cache_drops_the_whole_candidate_table` 里。
    #[test]
    fn reset_cache_transition_keeps_the_generation_and_carries_no_diagnostic() {
        let engine = qingjian_core::Engine::new(
            qingjian_dictionary::Dictionary::parse("你好\tni hao\t9000\n").expect("夹具词库"),
        );
        let mut state = Session::new(engine, 7);
        for c in "nihao".chars() {
            let _ = state.engine.push(c);
        }
        state.requery();
        assert!(!state.candidates.is_empty(), "夹具：先要查出候选");
        let before = state.generation;
        let transition = run_transition(&mut state, 7, |s| keys::reset_cache(s));
        assert!(
            transition.handled,
            "清缓存是引擎收下的一次操作，不是交回宿主"
        );
        assert!(
            transition.diagnostic.is_none(),
            "不该顺手点亮 hasDiagnostic"
        );
        assert_eq!(
            state.generation, before,
            "重建出来逐格相同的表不涨号，用户手里的旧候选号仍然有效"
        );
        assert!(
            !state.candidates.is_empty(),
            "丢掉之后要按当前组字重建，不能留一张空表"
        );
    }

    /// 「优先显示」的夹具：同一读音 `ni hao` 下三个词，词频 你好 > 拟好 > 泥壕。
    const PIN_SAMPLE: &str = "你好\tni hao\t9000\n拟好\tni hao\t3000\n泥壕\tni hao\t800\n";

    fn pin_session() -> Session {
        let engine = qingjian_core::Engine::new(
            qingjian_dictionary::Dictionary::parse(PIN_SAMPLE).expect("夹具词库"),
        )
        .with_learner(Box::new(qingjian_learning::FrequencyLearner::default()));
        let mut state = Session::new(engine, 11);
        for c in "nihao".chars() {
            let _ = state.engine.push(c);
        }
        state.requery();
        state
    }

    /// 「优先显示」= 用户又多选了一次这个候选：两条学习记录都要写上，
    /// 且**当场重查就把它顶到第一格**。
    ///
    /// 判别器两条都实跑过（各红一条，因为 panic 会终止测试，后面的断言跑不到）：
    /// 删掉 `record_choice` ⇒ `choice_weight` 那条红（实测 `left: 0 / right: 1`）；
    /// 删掉 `record` ⇒ `weight` 那条红（同实测）。"排到第一"那条是这两条之后的独立证据。
    #[test]
    fn promoting_a_candidate_records_both_and_moves_it_first() {
        let mut state = pin_session();
        let index = state
            .candidates
            .iter()
            .position(|c| c.text == "泥壕")
            .expect("夹具里泥壕该有一格");
        let generation = state.generation;
        assert_eq!(state.candidates[0].text, "你好", "置顶之前首选是 你好");
        assert!(
            matches!(
                pin_candidate(&mut state, generation, index),
                CommandOutcome::Consumed
            ),
            "记上了就该回 Consumed，宿主才会播报「已优先显示」而不是「不支持此操作」"
        );
        assert_eq!(state.engine.learner().weight("泥壕"), 1, "词频记了一次");
        assert_eq!(
            state.engine.learner().choice_weight("nihao", "泥壕"),
            1,
            "同一读音下的选择记了一次"
        );
        state.requery();
        assert_eq!(
            state.candidates[0].text, "泥壕",
            "排序键第 5 位是 choice，记过就该压过词频排到第一"
        );
    }

    /// 私密框或用户关了「记忆新词」：不写学习数据，也不假装播报置顶。
    #[test]
    fn promoting_declines_when_learning_is_off() {
        let mut state = pin_session();
        let index = state
            .candidates
            .iter()
            .position(|c| c.text == "泥壕")
            .expect("夹具里泥壕该有一格");
        let generation = state.generation;
        state.identity.learning_allowed = false;
        assert!(
            matches!(
                pin_candidate(&mut state, generation, index),
                CommandOutcome::Passthrough
            ),
            "不许学习时就回 false，让宿主说实话"
        );
        assert_eq!(state.engine.learner().weight("泥壕"), 0, "一条都不该记上");
    }

    /// 代次对不上（菜单是旧那一帧的 id）⇒ 交回宿主，不能把选择记到别的词头上。
    #[test]
    fn promoting_declines_on_a_stale_generation() {
        let mut state = pin_session();
        let stale = state.generation + 1;
        assert!(
            matches!(
                pin_candidate(&mut state, stale, 0),
                CommandOutcome::Passthrough
            ),
            "过期代次必须被 `candidate_index` 挡下来"
        );
        assert_eq!(state.engine.learner().weight("你好"), 0);
    }

    /// **前缀候选的键要按作用域切，不能按音节表拼**（这是本函数实现里唯一一处非平凡的判断）。
    /// 真机上 `nihao` 会出「你好像」这类读音更长的候选，它这次只覆盖 `nihao`；
    /// 若按音节拼成 `nihaoxiang` 去记，查询侧（按作用域前缀取 `choice_weight`）永远取不到，
    /// 症状就是"点了优先显示，下次还是没置顶"。
    ///
    /// 判别器实跑过：把键换回"音节表直接拼" ⇒ **只有这一条红**
    /// （`记的键是被它覆盖的那段作用域`，`left: 0 / right: 1`），另外三条全绿——
    /// 也就是说没有这条测试，那个 bug 会静悄悄上线。
    #[test]
    fn promoting_a_prefix_candidate_records_the_scope_prefix() {
        let mut state = pin_session();
        let long = qingjian_core::Candidate {
            text: "你好像".to_owned(),
            kind: qingjian_core::CandidateKind::Chinese,
            syllables: vec!["ni".to_owned(), "hao".to_owned(), "xiang".to_owned()],
            reading: None,
            translation: None,
            aux_code: None,
        };
        let mut list = state.candidates.clone();
        list.push(long);
        let index = list.len() - 1;
        let generation = state.generation;
        state.refresh(list);
        assert!(
            matches!(
                pin_candidate(&mut state, generation, index),
                CommandOutcome::Consumed
            ),
            "中文候选就该收下"
        );
        assert_eq!(
            state.engine.learner().choice_weight("nihao", "你好像"),
            1,
            "记的键是被它覆盖的那段作用域 `nihao`"
        );
        assert_eq!(
            state.engine.learner().choice_weight("nihaoxiang", "你好像"),
            0,
            "整条读音上不该有记录——查询侧永远取不到那个键"
        );
    }
}
