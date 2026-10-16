//! 偏好读写里「不需要 JVM 就能测」的那一半。导出层在 `lib.rs`，信封要桥才跑得动。
//!
//! 宿主契约（逐条对过 `host/java/app/msime/android/core/MSIMEInputService.java`）：
//! - `loadPreferences` 的 value 必须有 `revision`（`>= 0`，见 `:2183` 的
//!   `PreferencesRevisionPolicy.read(..., -1)`，负数直接抛）与 `preferences`
//!   （`:2186` 是这一路唯一的严格 `getJSONObject`）。其余字段宿主一律
//!   `optBoolean` / `strictInt(…, 默认值)`，缺了就落它自己的默认值。
//! - `updatePreferences` 的 value 必须有 `view`（`:2332` 硬取）。

use serde_json::Value;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// 偏好文档的文件名，与上游 `PreferencesStore::path()` 逐字相同
/// （`msime-develop/crates/client-core/src/preferences.rs:2896`），
/// 目录就是宿主传进来的 `preferences_directory`（= `state_root`，见 `assemble.rs:85`）。
pub const FILE_NAME: &str = "preferences.json";
/// 同目录内的暂存文件名。用**固定名**而不是随机名：被杀进程留下的这一个，
/// 下一次写会整只盖掉，不会像上游那样攒成一堆需要扫的孤儿。
const STAGED_SUFFIX: &str = ".tmp";

/// 取快照里那个 `preferences` 对象。读写两头都要它，而且宿主对它的读取是严格的
/// （load 侧 `MSIMEInputService.java:2186` 用 `getJSONObject`，save 的响应随后也喂给
/// 同一个 `applyPreferencesSnapshot`），所以缺了它就必须拒，不能补一个空的蒙混过去。
fn preferences_of(value: &Value) -> Option<&serde_json::Map<String, Value>> {
    value.get("preferences").and_then(Value::as_object)
}

/// 默认文档。宿主对快照只有两处严格读取（见上面 `load_document` 的注释），
/// 所以"还没有任何文件"就回这一份，让它用自己那套 `opt*` 默认值画键盘。
pub fn load_document() -> Value {
    serde_json::json!({ "format_version": 1, "revision": 0, "preferences": {} })
}

/// 读盘的结果。分三档是刻意的：**缺文件**是正常初态，**坏文件**是事故，
/// 两者给宿主的答案必须不同（后者不能回 `ok:true`，否则下一轮 save 会把事故现场覆盖掉）。
#[derive(Debug)]
pub enum Loaded {
    Default,
    Document(Value),
    Unreadable,
}

pub fn document_path(directory: &Path) -> PathBuf {
    directory.join(FILE_NAME)
}

/// 读 `preferences.json`。缺文件 ⇒ `Default`；解析不出、或缺 `preferences` 对象 ⇒ `Unreadable`。
pub fn load(directory: &Path) -> Loaded {
    match fs::read(document_path(directory)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Loaded::Default,
        Err(_) => Loaded::Unreadable,
        Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
            Ok(document) if preferences_of(&document).is_some() => Loaded::Document(document),
            _ => Loaded::Unreadable,
        },
    }
}

/// 写盘的结果。`Conflict` 要把**当前**那份文档带回去，宿主才有余地按新值重画
/// （它读 `value.revision` 与 `value.preferences`，见 `MSIMEInputService.java:4970-4976`）。
#[derive(Debug)]
pub enum Saved {
    Applied(Value),
    Conflict(Value),
    Rejected(&'static str),
}

/// 比较并交换 + 原子写。语义逐条对齐上游 `save_locked`
/// （`msime-develop/crates/client-core/src/preferences.rs:2993-3047`）：
/// - **等值** CAS：`当前 revision != expected` ⇒ `Conflict`，什么都不写。
///   （不是"小于才拒"——上游要的是"我读到的那份还是现在这份"，跳号同样算冲突。）
/// - **内容没变就不涨号、不写盘**（上游 `:3031`，省一次 fsync，也免得所有读者白重载一遍）；
///   但文件不存在时第一次仍要写，所以那条还带一个 `revision > 0` 的前提。
/// - 修订号只增不减，`+1`。
///
/// 整份文档**当不透明 JSON 存**：宿主传什么就落什么（它传的是完整快照，
/// `pending = new JSONObject(preferencesSnapshot.toString())`，`MSIMEInputService.java:4868`），
/// 我们只负责 `revision` 这一项归自己算。
pub fn save(directory: &Path, expected: u64, snapshot: &[u8]) -> Saved {
    let Ok(incoming) = serde_json::from_slice::<Value>(snapshot) else {
        return Saved::Rejected("malformed_preferences_snapshot");
    };
    let Some(preferences) = preferences_of(&incoming).cloned() else {
        return Saved::Rejected("snapshot_without_preferences");
    };
    let current = match load(directory) {
        Loaded::Document(document) => document,
        Loaded::Default => load_document(),
        Loaded::Unreadable => return Saved::Rejected("preferences_unreadable"),
    };
    let Some(current_revision) = current.get("revision").and_then(Value::as_u64) else {
        return Saved::Rejected("stored_revision_unreadable");
    };
    if current_revision != expected {
        return Saved::Conflict(current);
    }
    if current_revision > 0 && preferences_of(&current) == Some(&preferences) {
        return Saved::Applied(current);
    }
    let mut next = incoming.clone();
    next["format_version"] = serde_json::json!(1);
    next["revision"] = serde_json::json!(current_revision + 1);
    next["preferences"] = Value::Object(preferences);
    let bytes = match serde_json::to_vec_pretty(&next) {
        Ok(bytes) => bytes,
        Err(_) => return Saved::Rejected("encode_failed"),
    };
    match atomic_write(directory, &bytes) {
        Ok(()) => Saved::Applied(next),
        Err(_) => Saved::Rejected("preferences_write_failed"),
    }
}

/// 暂存文件的路径。**必须与目标同目录**——跨文件系统的 `rename` 不是原子的
/// （POSIX 会直接 `EXDEV` 失败），所以这条是"原子写"成立的全部前提。
fn staged_path(directory: &Path) -> PathBuf {
    directory.join(format!("{FILE_NAME}{STAGED_SUFFIX}"))
}

/// 先写同目录的暂存文件、再 `rename` 覆盖目标：同文件系统内的 rename 是原子的，
/// 所以读者要么看到旧的一份、要么看到新的一份，**永远不会看到半截文件**。
fn atomic_write(directory: &Path, bytes: &[u8]) -> io::Result<()> {
    fs::create_dir_all(directory)?;
    let staged = staged_path(directory);
    fs::write(&staged, bytes)?;
    fs::rename(&staged, document_path(directory))
}

/// `updatePreferencesRaw` 的应答体（外面再套一层 `ok`/`value` 信封）。
///
/// 宿主对这一份 `value` 只有**两处**严格读取，逐行扫过 `MSIMEInputService.java:2260-2388`
/// 才敢说"只有两处"：`:2332` 的 `getJSONObject("view")` 与 `:2387` 的
/// `strictBoolean(result, "deferred")`。后者对**缺键也抛** `JSONException`
/// （同文件 `:850`：`if (!(value instanceof Boolean)) throw ...`），
/// 所以少一个 `deferred` 就等于把那句「设置读取或应用失败」原样留着——
/// 真机上一版就是这么翻车的，`view` 给了、`deferred` 没给。
///
/// `deferred=false` 对我们是**实话**，不是偷懒，理由记在这里免得被当缺口"补掉"：
///
/// 上游那个 `deferred:true` 背后**真有一个队列**——`NativeSmoke.java:114-122` 的次序是
/// "组词中 updatePreferences ⇒ `deferred:true`" → "命令 1 提交组词" → "再 updatePreferences
/// ⇒ `deferred:false`"，即 runtime 把快照存下、等组词结束再应用到引擎。我们**没有这个队列**：
/// 这一层当场把唯一消费的 `learning` 落进会话。若照抄成 `!editing_text.is_empty()`，
/// 宿主会把它经 `MSIMEInputService.java:2387` → `:7696` → `:7703` 写成表头那句
/// 「设置将在组词结束后应用」，而组词结束后**什么都不会发生**（偏好重载每 1 秒还会重写一次）。
///
/// 更反的是方向：`learning` 是**关掉个性化学习**的隐私开关，"推迟应用"= 用户已经要求停
/// 之后再多学几个键。⇒ 对这个字段当场生效才是对的。真要上"推迟"，得连行为一起做，
/// 且只在引擎开始消费别的偏好项时再评估（见 PIN「`deferred` 保真度修复」一节）。
///
/// 第二步 `save` 落地（2026-10-16）**没有**触发那个再评估的口子：写盘是"当场写、下次读"，
/// 读回来的是已经躺在档里的值，中间没有任何待应用队列 ⇒ 仍然没有东西可"推迟"。
pub fn update_body(view: Value, deferred: bool) -> Value {
    serde_json::json!({ "view": view, "deferred": deferred })
}

/// 宿主 `updatePreferences` 送来的整份快照，是否要求关掉个性化学习。
///
/// 只认明确的 `false`：缺字段、非布尔、JSON 坏一律 `false`（= 不动现状）。这是**单向门**，
/// 与宿主自己那半一致——它只在 `learningSuppressed()` 时把这一项写成 false
/// （`:2259`），从不写回 true；重新允许靠下一次 `createRaw`。
pub fn wants_learning_off(snapshot: &str) -> bool {
    serde_json::from_str::<Value>(snapshot)
        .ok()
        .as_ref()
        .and_then(|value| value.get("preferences"))
        .and_then(|preferences| preferences.get("learning"))
        .and_then(Value::as_bool)
        == Some(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 每条用例一个独占子目录，先清后建——挂起表那种"并发跑就红"的坑这里防一次：
    /// 目录名带用例名，用例之间不共享任何状态。
    fn 临时目录(名字: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("qingshan-prefs-{名字}"));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("建临时目录");
        base
    }

    fn 快照(修订号: u64, 剪贴板: bool) -> String {
        format!(
            r#"{{"format_version":1,"revision":{修订号},"preferences":{{"clipboard_history":{剪贴板}}}}}"#
        )
    }

    fn 存进去(dir: &Path, 期望: u64, 快照内容: &str) -> Value {
        match save(dir, 期望, 快照内容.as_bytes()) {
            Saved::Applied(document) | Saved::Conflict(document) => document,
            Saved::Rejected(reason) => panic!("写入被拒：{reason}"),
        }
    }

    #[test]
    fn 没有文件时读到的是默认档而不是错误() {
        let dir = 临时目录("missing");
        assert!(matches!(load(&dir), Loaded::Default), "缺文件应回默认档");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn 写进去的东西读得回来且修订号进一位() {
        let dir = 临时目录("roundtrip");
        let written = 存进去(&dir, 0, &快照(0, true));
        assert_eq!(written["revision"], 1, "第一次写要把修订号推到 1");
        assert_eq!(written["preferences"]["clipboard_history"], true);
        match load(&dir) {
            Loaded::Document(document) => {
                assert_eq!(document, written, "读回必须与写进去的同一份");
                assert_eq!(document["preferences"]["clipboard_history"], true);
            }
            other => panic!("读回不是文档：{other:?}"),
        }
        assert!(
            !dir.join(format!("{FILE_NAME}{STAGED_SUFFIX}")).exists(),
            "rename 之后不能留下半截暂存文件"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn 修订号不符时什么都不写并带回当前那份() {
        let dir = 临时目录("conflict");
        let first = 存进去(&dir, 0, &快照(0, true));
        let stale = 快照(0, false); // 拿着旧修订号来写，还想把剪贴板关掉
        match save(&dir, 0, stale.as_bytes()) {
            Saved::Conflict(document) => {
                assert_eq!(document, first, "冲突要带回盘上那份");
                assert_eq!(document["revision"], 1);
            }
            Saved::Applied(_) => panic!("等值 CAS 失效：旧修订号被接受了"),
            Saved::Rejected(reason) => panic!("冲突不该走拒绝分支：{reason}"),
        }
        // 跳号（拿着比盘上更新的修订号来）同样必须拒——这条才是"等值"与"小于才拒"的分水岭。
        // 上一版用例漏了它：把 CAS 放宽成"当前修订号大于来值才拒"时，全套测试照样全绿，
        // 补上这一格之后它当场红。
        match save(&dir, 5, 快照(5, false).as_bytes()) {
            Saved::Conflict(_) => {}
            Saved::Applied(_) => panic!("等值 CAS 失效：跳号的修订号被接受了"),
            Saved::Rejected(reason) => panic!("跳号该走冲突分支，不是拒绝：{reason}"),
        }
        match load(&dir) {
            Loaded::Document(document) => assert_eq!(
                document["preferences"]["clipboard_history"], true,
                "冲突之后盘上内容不能变"
            ),
            other => panic!("读回不是文档：{other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn 内容没变就不涨号不写盘() {
        let dir = 临时目录("noop");
        let first = 存进去(&dir, 0, &快照(0, true));
        assert_eq!(first["revision"], 1);
        let again = 存进去(&dir, 1, &快照(1, true));
        assert_eq!(again["revision"], 1, "同内容第二次不该涨号");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn 暂存文件必须与目标同目录否则rename不原子() {
        let dir = 临时目录("staged");
        assert_eq!(
            staged_path(&dir).parent(),
            document_path(&dir).parent(),
            "跨文件系统的 rename 会 EXDEV 失败，原子性就没了"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn 坏文件报不可读且拒绝被覆盖() {
        let dir = 临时目录("corrupt");
        std::fs::write(document_path(&dir), b"{ not json at all").expect("写坏文件");
        // 坏文件不能当默认档回给宿主，否则下一轮 save 会把事故现场覆盖掉。
        assert!(matches!(load(&dir), Loaded::Unreadable));
        match save(&dir, 0, 快照(0, true).as_bytes()) {
            Saved::Rejected("preferences_unreadable") => {}
            other => panic!("坏文件时写入必须被拒，不能覆盖事故现场：{other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn 应答体给齐update那两处严格读取() {
        let body = update_body(serde_json::json!({ "generation": 1 }), false);
        assert!(
            body.get("view").and_then(Value::as_object).is_some(),
            "宿主 `:2332` 用 getJSONObject(\"view\")"
        );
        assert!(
            body.get("deferred").and_then(Value::as_bool).is_some(),
            "宿主 `:2387` 的 strictBoolean 对缺键也抛 ⇒ deferred 必须是真的 JSON boolean"
        );
    }

    #[test]
    fn 应答体给齐宿主那两处严格读取() {
        let document = load_document();
        let revision = document.get("revision").expect("revision 必须在");
        assert!(
            revision.as_u64().is_some(),
            "revision 必须是非负整数，宿主读到负数就抛：{revision}"
        );
        assert!(
            document
                .get("preferences")
                .and_then(Value::as_object)
                .is_some(),
            "preferences 必须是 JSON 对象，宿主对它用 getJSONObject"
        );
    }

    #[test]
    fn 修订号恒定所以宿主不会反复重应用() {
        assert_eq!(load_document(), load_document(), "同一份应答体要稳定");
        assert_eq!(load_document()["revision"], 0);
    }

    #[test]
    fn 只有明确写false的快照才关学习() {
        assert!(wants_learning_off(r#"{"preferences":{"learning":false}}"#));
        assert!(!wants_learning_off(r#"{"preferences":{"learning":true}}"#));
        assert!(!wants_learning_off(r#"{"preferences":{}}"#));
        assert!(!wants_learning_off("{}"));
        let learning_as_string = r#"{"preferences":{"learning":"false"}}"#;
        assert!(
            !wants_learning_off(learning_as_string),
            "字符串 \"false\" 不算关掉学习"
        );
        assert!(!wants_learning_off("not json at all"));
    }
}
