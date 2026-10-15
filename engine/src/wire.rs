//! JNI 边界的数据封装：所有回给 Java 的 `byte[]` 都在这里造。
//!
//! 水杉 Java 侧对返回信封是**严格解析**的（`MSIMEInputService.java:844` 的 `value()`、
//! `:851` 的 `strictBoolean()`），形状必须是 `{"ok":true,"value":{...}}` 或
//! `{"ok":false,"error":"..."}`，且 `ok` 必须是真 JSON boolean。
//!
//! 注意 `ok:false` 的代价：`value()` 见到非真 `ok` 就抛 `JSONException`，而 `apply()`
//! 的调用点多数只 catch `JSONException | LinkageError` 然后 `fail()`
//! （`:3125`、`:2389`）——也就是**整个会话降级成「仅直接输入」**。所以「这次没处理」
//! 必须走 `ok:true` + `handled:false`，`ok:false` 只留给真正的准备失败/不可用。
//!
//! jni 0.22 的 `with_env` 内部已经用 `catch_unwind` 包住闭包，并把 panic 作为
//! `Outcome::Panic` 交回来（`jni-0.22.4/src/env.rs:4801`，即
//! `$CARGO_HOME/registry/src/<registry-hash>/jni-0.22.4/src/env.rs`）——PRD 风险表里
//! 「FFI 边界用 catch_unwind 把 panic 转成错误码」这一条由它兜住，不需要每个方法再包一层。
//! 这里刻意**不用** `resolve::<ThrowRuntimeExAndDefault>()`：那会往 `:ime` 进程抛 Java
//! 异常，而水杉的降级分支吃的是 `ok:false` 信封，抛异常反而可能击穿没 catch 的调用点。

use jni::EnvUnowned;
use jni::Outcome;
use jni::objects::JByteArray;
use jni::sys::jbyteArray;

/// 非输入路径的占位应答（`stub.rs` 那批）。语义见文件头：这会让 Java 走 `fail()` 降级。
pub fn not_implemented(env: &mut EnvUnowned<'_>) -> jbyteArray {
    envelope(env, br#"{"ok":false,"error":"not_implemented"}"#)
}

/// 成功信封：`{"ok":true,"value":<value>}`。
pub fn ok_value(env: &mut EnvUnowned<'_>, value: serde_json::Value) -> jbyteArray {
    match serde_json::to_vec(&serde_json::json!({ "ok": true, "value": value })) {
        Ok(body) => envelope(env, &body),
        Err(error) => {
            // 只可能发生在 value 里含 NaN 之类无法编码的值；我们的类型不会产生，
            // 真出现了要能在 log 里看见，不能静默回 ok:true。
            tracing::error!(?error, "信封序列化失败");
            envelope(env, br#"{"ok":false,"error":"encode_failed"}"#)
        }
    }
}

/// 良性空操作的应答体：`{"view": <当前 View>, "handled": false}`。
/// 拆成纯函数是因为导出层要 JVM 才跑得动，宿主 `cargo test` 只能测到这里。
pub fn no_op_body(view: &crate::view::View) -> serde_json::Value {
    serde_json::json!({ "view": view, "handled": false })
}

/// 回 `ok:true` + 当前 View + `handled:false`：会话活着，宿主最多弹一条「不支持此操作」。
///
/// 必须回**完整 View**：`apply()` 在 `MSIMEInputService.java:2391` 硬取 `view` 对象、
/// `:2407` 对 `editing_text` 用 `getString`，缺任何一处都是 `JSONException` → 调用点的
/// catch（`:6046` 等）→ `fail()`（`:3116`）→ 会话死。`handled` 必须是真 JSON boolean
/// （`:2424` 的 `strictBoolean`），不能省也不能给字符串。
pub fn no_op_view(env: &mut EnvUnowned<'_>, view: &crate::view::View) -> jbyteArray {
    ok_value(env, no_op_body(view))
}

/// 带原因的失败信封。只用于 Bootstrap 那类「准备失败就该报错并允许重试」的入口。
pub fn fail(env: &mut EnvUnowned<'_>, reason: &str) -> jbyteArray {
    // 宿主对 ok:false 的处理是抛 JSONException 再走降级分支（`MSIMEInputService.java:844`），
    // 所以这一行等于「会话即将降级」的现场记录。桩走的是 `not_implemented`，
    // 不打日志——否则每次点不可用的入口都刷屏。
    tracing::warn!("回 ok:false: {reason}");
    match serde_json::to_vec(&serde_json::json!({ "ok": false, "error": reason })) {
        Ok(body) => envelope(env, &body),
        Err(_) => envelope(env, br#"{"ok":false,"error":"encode_failed"}"#),
    }
}

/// 读 Java 传来的 UTF-8 `byte[]`。空指针或取用失败时返回 `None`，由调用方回错误信封。
///
/// `convert_byte_array` 见 `jni-0.22.4/src/env.rs:3368`；入参是裸 `jbyteArray`，
/// 先 `from_raw` 包成 `JByteArray`（`jni 0.22` 的 `jboolean` 同理是 Rust `bool`，
/// 不是 C 的 `uint8_t`）。
pub fn read_text(env: &mut EnvUnowned<'_>, array: jbyteArray) -> Option<String> {
    if array.is_null() {
        return None;
    }
    // jni 0.22 的 convert_byte_array 收 `impl AsRef<JByteArray>`（`env.rs:3368`），
    // 而导出函数的入参是裸 jbyteArray，故在 with_env 内用 from_raw 包一层（`&Env` + 裸引用）。
    let outcome = env.with_env(move |env| {
        let array = unsafe { JByteArray::from_raw(env, array) };
        env.convert_byte_array(&array)
    });
    match outcome.into_outcome() {
        Outcome::Ok(bytes) => Some(String::from_utf8_lossy(&bytes).into_owned()),
        Outcome::Err(error) => {
            tracing::error!(?error, "读取 Java 入参数组失败");
            None
        }
        Outcome::Panic(panic) => {
            tracing::error!(?panic, "读取 Java 入参数组时 panic，已在 FFI 边界拦下");
            None
        }
    }
}

fn envelope(env: &mut EnvUnowned<'_>, body: &[u8]) -> jbyteArray {
    let outcome = env.with_env(|env| env.byte_array_from_slice(body));
    match outcome.into_outcome() {
        Outcome::Ok(array) => array.into_raw() as jbyteArray,
        Outcome::Err(error) => {
            // 建不出数组只有堆耗尽或 VM 已死两种可能。返回 null 让 Java 侧以
            // NullPointerException 暴露真问题，比伪造一份合法-looking 的空应答好。
            tracing::error!(?error, "构造 JNI 应答失败");
            std::ptr::null_mut()
        }
        Outcome::Panic(panic) => {
            let detail = panic
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| panic.downcast_ref::<&str>().copied())
                .unwrap_or("(非字符串 panic)");
            tracing::error!(
                detail,
                "Rust panic 已在 FFI 边界拦下（同时见 panic hook 输出）"
            );
            std::ptr::null_mut()
        }
    }
}
