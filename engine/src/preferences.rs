//! 偏好读写里「不需要 JVM 就能测」的那一半。导出层在 `lib.rs`，信封要桥才跑得动。
//!
//! 宿主契约（逐条对过 `host/java/app/msime/android/core/MSIMEInputService.java`）：
//! - `loadPreferences` 的 value 必须有 `revision`（`>= 0`，见 `:2183` 的
//!   `PreferencesRevisionPolicy.read(..., -1)`，负数直接抛）与 `preferences`
//!   （`:2186` 是这一路唯一的严格 `getJSONObject`）。其余字段宿主一律
//!   `optBoolean` / `strictInt(…, 默认值)`，缺了就落它自己的默认值。
//! - `updatePreferences` 的 value 必须有 `view`（`:2332` 硬取）。

use serde_json::Value;

/// `loadPreferencesRaw` 的应答体。
///
/// **方案丙第一步不落盘**：`savePreferencesRaw` 还在桩里（`stub.rs:121`），没有任何东西
/// 可读，所以固定回「修订号 0 + 空偏好」，让宿主用它自己那套默认值把键盘跑起来。
/// 修订号恒定 0 是有意的：宿主按修订号决定要不要重新应用整份快照，恒定值 ⇒ 不会每秒重渲染。
pub fn load_document() -> Value {
    serde_json::json!({ "revision": 0, "preferences": {} })
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
