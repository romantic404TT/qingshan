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
//! - 本文件：19 个真实现——输入路径的 8 个方法，加中/英模式切换 `setEnglishModeRaw`
//!   （它被宿主的启动路径与中/英键硬解析，桩会让会话建不起来）、候选长按四件
//!   `pinCandidateRaw` / `fixCandidatePositionRaw` / `clearCandidatePositionRaw` /
//!   `selectEdgeRaw`（Core 无对应能力，回「原样 View + `handled:false`」止血）、
//!   `removeCandidateRaw`（接 `Engine::forget`）、展开面板 `allCandidatesRaw`
//!   与它唯一的写入口 `selectAnyCandidateRaw`，`refreshHostRaw`（比随包锁的长度，
//!   让升级安装包能重新解包），以及偏好读写方案丙第一步的 `loadPreferencesRaw` 与
//!   `updatePreferencesRaw`（两条必须成对，理由见 `preferences.rs`）。
//!   **计数以 `tools/gen_stub.py` 的 `REAL` 集合为准。**
//! - `stub.rs`：其余 83 个，由 `tools/gen_stub.py` 从 Java 声明生成，回 `not_implemented`。
//! - `session.rs`：会话表 + 宿主游标；`keys.rs`：键码与命令号分派；`view.rs`：JSON 镜像；
//!   `wire.rs`：信封与 `byte[]` 收发；`resources_lock.rs`：随包锁的解析与长度比对；
//!   `preferences.rs`：偏好应答体的字段要求与 `learning` 单向门。
//!
//! panic 全部由 `wire` 里的 `with_env` 兜（`jni-0.22.4/src/env.rs:4801` 的 `catch_unwind`），
//! 所以本文件的每个导出都不需要自己包 `catch_unwind`。

mod assemble;
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
/// 信封与字段要求都在 `preferences.rs`，那边走单测。
#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_loadPreferencesRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, _directory: jbyteArray,
) -> jbyteArray {
    // 这一条没有任何失败分支，所以不装日志桥（装了也是空转）：`load_document` 恒回 ok。
    wire::ok_value(&mut env, preferences::load_document())
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

/// 一次「动作 → 按结果涨号 → 重查 → 出 Transition」的完整编排。
///
/// **顺序是这一处的核心修复。** 原先 `advance()` 无条件跑在动作之前，而动作内部的代次
/// 校验（`candidate_index` / `candidate_index_in_table`）比的是宿主传来的 `generation`
/// 与 `self.generation` —— 先 +1 就永远不等，于是 `selectRaw` / `removeCandidateRaw` /
/// `selectAnyCandidateRaw` 三个带 id 的入口恒回 `handled:false`，宿主点候选从来不上屏。
/// 阶段一验收是用空格（`commandRaw` 命令 1，不带 id）提交「你好」的，所以没暴露。
///
/// 涨号谓词刻意偏向「多涨」：漏涨会让宿主拿旧表的下标选中另一个候选（上错字，
/// 数据正确性），多涨只会 `handled:false` 加候选条滚回最左（体验）。所以
/// `Passthrough` / `NotSupported` 不涨，`Consumed` / `Commit` 涨。
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
    let outcome = action(state);
    let committed = matches!(outcome, CommandOutcome::Commit(_));
    if matches!(
        outcome,
        CommandOutcome::Consumed | CommandOutcome::Commit(_)
    ) {
        let _ = state.advance();
    }
    state.requery();
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

/// 候选长按四件共用的空操作应答：`pinCandidate` / `fixCandidatePosition` /
/// `clearCandidatePosition` / `selectEdge`。
///
/// 为什么不是桩：这四个调用点的 catch 体是 `fail()`（`MSIMEInputService.java:6046`、
/// `:6243`），回 `not_implemented` 等于「点一下就死会话」。青简 Core 没有对应能力
/// （逐个核实见 `third-party/PIN.md`「会话建立后的 stub 降级面」），先止血。
///
/// 只读会话：不 `advance`、不 `requery`。代次一变，宿主会按
/// `(session, generation, page)` 把候选条滚回最左（`MSIMEInputService.java:8101-8122`）。
fn no_op(env: &mut EnvUnowned<'_>, session: jlong) -> jbyteArray {
    let id = session as u64;
    match with_session(id, |state| build_view(state, id)) {
        Some(view) => wire::no_op_view(env, &view),
        None => wire::not_implemented(env),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_msime_android_NativeClient_pinCandidateRaw(
    mut env: EnvUnowned<'_>, _class: JObject<'_>, session: jlong, _generation: jlong, _index: jlong,
) -> jbyteArray {
    no_op(&mut env, session)
}

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
}
