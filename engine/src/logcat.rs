//! Rust 侧的日志桥：把 `tracing` 事件与 panic 送到 logcat（Android）或 stderr（宿主）。
//!
//! 装之前 `tracing` 没有 subscriber，所有 `warn!`/`error!` 都是静默 no-op——FFI 边界
//! 拦下的 panic、`ok:false` 的原因、词库载入耗时，全都落不到任何地方。
//!
//! 刻意**不引入** `tracing-subscriber` 与 `android_log-sys`：`Subscriber` 手写 7 个方法，
//! `field::Visitor` 只有 `record_debug` 是必需的（`tracing-core-0.1.36/src/field.rs`），
//! `liblog` 用 `#[link]` 直连（NDK sysroot 的 `liblog.so` stub 从 API 21 起就导出
//! `__android_log_write`）。装 subscriber 而不是换成自家宏，是因为青简 Core 自己用了
//! `tracing`（实测 23 处 `warn!` + 6 处 `info!` + 30 处 `debug!`），一并接出来且不改 Core。

#[cfg(target_os = "android")]
use std::ffi::{CString, c_char};
use std::fmt;
use std::sync::Once;

use tracing::field::{Field, Visit};
use tracing::span;
use tracing::{Event, Level, Metadata, Subscriber};

/// logcat 的 tag。Android 对 tag 长度有历史上限（16/23 字符），固定短串最稳；
/// 真正的来源（`metadata.target()`，如 `qingjian_core::engine::query`）写进正文。
const TAG: &str = "qingshan";

static INIT: Once = Once::new();

/// 幂等安装。由 `prepareHostRaw`、`createRaw` 与 `refreshHostRaw` 各调一次——其余导出都
/// 必须先有会话，而会话只能由 `createRaw` 建立。`refreshHostRaw` 是 2026-10-13 补的第三个点：
/// 「升级安装包」那次启动走的是 `refreshExistingConfiguration` → `refreshHost`，
/// 既没有 prepare 也没有会话，不补的话它回 `ok:false` 时的原因行是静默 no-op（真机实测）。
/// 不用 `JNI_OnLoad`：上游没有它，`verify_symbols.py`/`verify_apk.py` 把「`JNI_OnLoad` 计数为 0」
/// 当不变量在验，不值得为日志破坏它。
pub fn init() {
    INIT.call_once(|| {
        if let Err(error) = tracing::subscriber::set_global_default(Bridge) {
            // 只有"别处已经装过 subscriber"才会走到这里；不 panic，退到 stderr 继续干活。
            eprintln!("{TAG}: 日志桥安装失败: {error}");
        }
        install_panic_hook();
        write(Level::INFO, "logcat", "日志桥已装好（级别门限 INFO）");
    });
}

/// 级别门限：`Level` 的 `Ord` 是「越啰嗦越大」（`tracing-core` 的 `metadata.rs` 文档与
/// doctest：`Level::ERROR < Level::WARN`、`Level::TRACE > Level::DEBUG`），
/// 所以留 ERROR/WARN/INFO、挡掉 Core 那 30 处 `debug!` 的写法是 `<= INFO`。
fn keeps(metadata: &Metadata<'_>) -> bool {
    *metadata.level() <= Level::INFO
}

struct Bridge;

impl Subscriber for Bridge {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        keeps(metadata)
    }

    fn event(&self, event: &Event<'_>) {
        let metadata = event.metadata();
        if !keeps(metadata) {
            return;
        }
        let mut fields = Fields::default();
        event.record(&mut fields);
        write(*metadata.level(), metadata.target(), &fields.render());
    }

    // 我们不建 span，这五个只是 trait 要求的空壳。
    fn new_span(&self, _span: &span::Attributes<'_>) -> span::Id {
        span::Id::from_u64(0)
    }

    fn record(&self, _span: &span::Id, _values: &span::Record<'_>) {}

    fn record_follows_from(&self, _span: &span::Id, _follows: &span::Id) {}

    fn enter(&self, _span: &span::Id) {}

    fn exit(&self, _span: &span::Id) {}
}

/// 把事件字段拼成一行：`message` 进正文，其余以 `名=值` 跟在后面。
/// 实测（宿主 `--nocapture`）：`&str` 值由 `record_str` 接走、正文不带引号（`s=字符串字段`），
/// 整型/布尔由 `record_debug` 接走（`n=42`、`b=true`）；消息无论是字面量还是 `{}` 插值
/// 都渲染成纯文本，所以不必特殊处理。
#[derive(Default)]
struct Fields {
    message: String,
    rest: Vec<String>,
}

impl Fields {
    fn render(&self) -> String {
        if self.rest.is_empty() {
            return self.message.clone();
        }
        format!("{} {}", self.message, self.rest.join(" "))
    }
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.push(field.name(), value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.push(field.name(), format!("{value:?}"));
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.push(field.name(), value.to_string());
    }
}

impl Fields {
    fn push(&mut self, name: &str, text: String) {
        if name == "message" {
            self.message = text;
        } else {
            self.rest.push(format!("{name}={text}"));
        }
    }
}

/// panic 走 hook 而不是只靠 FFI 边界：青简 Core 会起后台线程
/// （`engine/rescoring/worker.rs` 的 `qingjian-rescore`，由 `setup.rs` 在装了
/// sentence scorer 时 spawn），那条线程上的 panic 不经过我们的 `with_env`。
/// FFI 路径的 panic 仍然由 `wire.rs` 记一条带导出名的日志——两条互补，不是冗余。
/// 调用方只有 `init()`，而它被 `Once` 挡着，所以这里不必再防重复装 hook。
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let payload = info.payload();
        let text = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("(非字符串 panic)");
        let location = match info.location() {
            Some(at) => format!("{}:{}", at.file(), at.line()),
            None => "未知位置".to_string(),
        };
        // 刻意不 force_capture：release 里 strip = true，强抓也拿不到符号，还慢。
        // `Backtrace::capture()` 自己尊重 RUST_BACKTRACE：未开时它的 `Display` 是一行提示。
        let backtrace = std::backtrace::Backtrace::capture();
        write(
            Level::ERROR,
            "panic",
            &format!("{text}（位置 {location}，栈 {backtrace}；若源自 FFI 调用另有边界拦截一条）"),
        );
    }));
}

#[cfg(target_os = "android")]
fn priority(level: Level) -> i32 {
    // `android/log.h` 的枚举：VERBOSE 2 / DEBUG 3 / INFO 4 / WARN 5 / ERROR 6。
    // （第一版记成了 ERROR 3 / INFO 6，正好反了 —— 真机上三条 INFO 全被打成 `E`。）
    // `Level` 的 Ord 是「越啰嗦越大」，所以从最严重的那端开始比。
    if level <= Level::ERROR {
        6
    } else if level <= Level::WARN {
        5
    } else if level <= Level::INFO {
        4
    } else {
        3
    }
}

/// Android 侧走 `liblog`，宿主侧走 stderr，subscriber 是同一份。
/// 注意宿主 `cargo test` 默认看不到任何一行：`init()` 只挂在 `prepareHostRaw`/`createRaw`
/// /`refreshHostRaw` 三个 FFI 入口上，单元用例不走那里（实测 `--nocapture` 跑
/// `real_dictionary_nihao_round_trip` 零输出）。要看就临时在测试里调一次 `init()`，
/// 本文件的桥本身已这样验过。
fn write(level: Level, target: &str, message: &str) {
    let line = format!("[{target}] {message}");
    #[cfg(target_os = "android")]
    {
        unsafe extern "C" {
            // bionic 的 `c_char` 是 `u8`（不是 host x86_64 的 `i8`），签名必须跟着走，
            // 否则 `CString::as_ptr()` 传不进来。
            fn __android_log_write(prio: i32, tag: *const c_char, buf: *const c_char) -> i32;
        }
        let tag = CString::new(TAG).expect("tag 里没有 NUL");
        match CString::new(line) {
            Ok(text) => {
                let _ =
                    unsafe { __android_log_write(priority(level), tag.as_ptr(), text.as_ptr()) };
            }
            // 消息里带 NUL（理论上只可能来自上游数据）：丢掉这条，别把桥弄崩。
            Err(error) => eprintln!("{TAG}: 日志正文含 NUL，已丢弃: {error}"),
        }
    }
    #[cfg(not(target_os = "android"))]
    eprintln!("{TAG}:{level} {line}");
}
