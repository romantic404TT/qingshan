//! 剪贴板历史的共享存储。宿主侧 `ClipboardHistoryStore.java:104-135` 把六个操作
//! 打包成一份 `{directory, action}` 请求递过来，这一层负责落盘与取回。
//!
//! **六个操作全部是真实现**：`load` / `capture` / `remove` / `set_pinned` / `replace` /
//! `clear`，没有一个还回 `not_implemented`。只有未知操作名回它，形状与当初的桩逐字同形。
//! 开关关着时宿主每秒一次的 `clearQuietly()`（`MSIMEInputService.java:2345`）必须被
//! `clear` 当幂等成功接下来，否则那句 `Clipboard history clear refused` 又会开始刷屏。

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::{Value, json};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use unicode_segmentation::UnicodeSegmentation;

/// 共享档的文件名。上游同一个名字（`host.rs` 里拼的是 `<directory>/MSIME/clipboard_history.json`），
/// 因为 iOS / HarmonyOS / macOS 宿主读的是同一份。
pub const FILE_NAME: &str = "clipboard_history.json";
/// 请求文档的字节上限，对齐 `client_jni.cpp:136` 的 `kMobileClipboardHistoryRequestLimit`。
pub const MAX_REQUEST_BYTES: usize = 524_288;

const MAX_ENTRIES: usize = 50;
/// 解码前先给条数封顶：去重与上限裁剪都发生在解码**之后**，
/// 不先封顶的话一个塞满小字符串的数组就能把内存吃掉。对齐上游 `client-core/src/clipboard.rs:12`。
const MAX_INPUT_ENTRIES: usize = MAX_ENTRIES * 100;
/// 移动端按 Apple 的 Character 与 UTF-8 双上限存，不截断文字。对齐上游 `client-core/src/clipboard.rs:15-16`。
const MAX_MOBILE_TEXT_BYTES: usize = 40_000;
const MAX_MOBILE_TEXT_CHARACTERS: usize = 10_000;
/// JSON 可以把每个输入字节逃成 6 个 ASCII 字节，所以读上限按最坏膨胀算。对齐上游 `client-core/src/clipboard.rs:33`。
const MAX_HISTORY_BYTES: u64 =
    (MAX_ENTRIES as u64) * ((MAX_MOBILE_TEXT_BYTES * 6) as u64 + 128) + 2;
/// 目录字符串的长度上限，对齐 `host.rs` 里那个 16384。
const MAX_DIRECTORY_BYTES: usize = 16_384;
/// 我们自己在 `<directory>` 下开的那一层。
const STORAGE_DIRECTORY: &str = "MSIME";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Entry {
    text: String,
    timestamp_ms: u64,
    pinned: bool,
}

/// 落盘格式有两代：结构化对象数组，和更早的**裸字符串数组**。
/// 后者按"下标越大越早"补一个确定性时间戳读进来，等第一次成功的写入顺手升级。
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Stored {
    Structured(Vec<Entry>),
    Legacy(Vec<String>),
}

/// 宿主那份请求文档。刻意**不**加 `deny_unknown_fields`：宿主以后多带一个键
/// 不该让"读历史"整个失败。（上游用的是 `deny_unknown_fields`，这是本层与它的一处有意差异。）
#[derive(Debug, Deserialize)]
struct Envelope {
    directory: String,
    action: Action,
}

#[derive(Debug, Deserialize)]
struct Action {
    operation: String,
    text: Option<String>,
    /// 只有 `set_pinned` 带这个键（宿主 `ClipboardHistoryStore.java:113` 按操作名加）。
    /// 所以它是 `Option` 且**缺了就拒**——猜一个默认值等于凭空固定或凭空取消一条。
    pinned: Option<bool>,
    /// 只有 `replace` 带。同样是"缺了就拒"：缺了新文字却去改，等于把条目改成空。
    replacement: Option<String>,
}

/// 一次编辑的五种结局。上游同名枚举在 `client-core/src/clipboard.rs`，
/// 这里只在本模块内用，但**取值一个不能少**：宿主要按它分四种提示文案。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edit {
    /// 改成了新文字，时间戳与固定状态都不动 ⇒ 它留在原来的位次上。
    Replaced,
    /// 新文字撞上另一条已有历史：两条并一条，留在**被编辑那条**的位置，固定状态取两者之或。
    Merged,
    /// 新文字与原文字相同：一个字节都不写。
    Unchanged,
    /// 原条目已经不在了（别的进程删过或清过）：幂等成功，不写盘。
    NotFound,
    /// 新文字不合规：不合规判定在**持锁之前**，所以连目录与锁侧车都不该出现。
    Invalid,
}

/// 一个操作的结果：要么是一份 `value`（宿主 `:133` 取 `optJSONObject("value")`），
/// 要么是一个拒绝理由——理由进信封的 `error`，宿主 `:127` 会把它拼进异常消息。
pub enum Reply {
    Ok(Value),
    Fail(&'static str),
}

/// 唯一的入口。`mobileClipboardHistoryRaw` 拿到请求字节后直接交给这里。
pub fn handle(request: &[u8]) -> Reply {
    if request.len() > MAX_REQUEST_BYTES {
        return Reply::Fail("mobile_clipboard_request_too_large");
    }
    let Ok(envelope) = serde_json::from_slice::<Envelope>(request) else {
        return Reply::Fail("invalid_mobile_clipboard_request");
    };
    if envelope.directory.len() > MAX_DIRECTORY_BYTES
        || !Path::new(&envelope.directory).is_absolute()
    {
        return Reply::Fail("invalid_mobile_clipboard_directory");
    }
    let path = history_path(Path::new(&envelope.directory));
    match envelope.action.operation.as_str() {
        "load" => match load_entries(&path) {
            Ok(entries) => Reply::Ok(json!({ "entries": entries_of(&entries) })),
            Err(_) => Reply::Fail("mobile_clipboard_history_unavailable"),
        },
        "capture" => {
            let Some(text) = envelope.action.text.as_deref() else {
                return Reply::Fail("invalid_mobile_clipboard_request");
            };
            if !mobile_text_is_valid(text) {
                // 拒收不是故障：上游也回 ok:true + captured:false + reason:"invalid"，
                // 因为宿主把 reason 直接画成「不保存」的提示，抛异常反而会把会话拖进 fail()。
                return Reply::Ok(json!({ "captured": false, "reason": "invalid" }));
            }
            match capture_entry(&path, text) {
                Ok((captured, entries)) => Reply::Ok(json!({
                    "captured": captured,
                    "reason": (!captured).then_some("full"),
                    "entries": entries_of(&entries)
                })),
                Err(_) => Reply::Fail("mobile_clipboard_capture_failed"),
            }
        }
        "remove" => match entry_text(envelope.action.text.as_deref()) {
            Ok(text) => match remove_entry(&path, text) {
                Ok((removed, entries)) => Reply::Ok(json!({
                    "removed": removed,
                    "entries": entries_of(&entries)
                })),
                Err(_) => Reply::Fail("mobile_clipboard_removal_failed"),
            },
            Err(reason) => Reply::Fail(reason),
        },
        "set_pinned" => match (
            entry_text(envelope.action.text.as_deref()),
            envelope.action.pinned,
        ) {
            (Ok(text), Some(pinned)) => match pin_entry(&path, text, pinned) {
                Ok((updated, entries)) => Reply::Ok(json!({
                    "updated": updated,
                    "entries": entries_of(&entries)
                })),
                Err(_) => Reply::Fail("mobile_clipboard_pin_update_failed"),
            },
            (Err(reason), _) => Reply::Fail(reason),
            (_, None) => Reply::Fail("invalid_mobile_clipboard_request"),
        },
        "replace" => match (
            entry_text(envelope.action.text.as_deref()),
            envelope.action.replacement.as_deref(),
        ) {
            (Ok(text), Some(replacement)) => match replace_entry(&path, text, replacement) {
                Ok((outcome, entries)) => Reply::Ok(json!({
                    // 宿主 `ClipboardHistoryStore.java:78-81` 先看 replaced 再看 merged：
                    // replaced:true + merged:false 它弹「已保存」，true + true 弹「合并成了一条」，
                    // false 才去读 reason。两个布尔**永远都在**，少一个就少判一种结局。
                    "replaced": outcome != Edit::NotFound && outcome != Edit::Invalid,
                    "merged": outcome == Edit::Merged,
                    "reason": match outcome {
                        Edit::NotFound => Some("not_found"),
                        Edit::Invalid => Some("invalid"),
                        _ => None,
                    },
                    "entries": entries_of(&entries)
                })),
                Err(_) => Reply::Fail("mobile_clipboard_replace_failed"),
            },
            (Err(reason), _) => Reply::Fail(reason),
            (_, None) => Reply::Fail("invalid_mobile_clipboard_request"),
        },
        "clear" => match clear_entries(&path) {
            Ok(()) => Reply::Ok(json!({ "cleared": true, "entries": [] })),
            Err(_) => Reply::Fail("mobile_clipboard_clear_failed"),
        },
        _ => Reply::Fail("not_implemented"),
    }
}

/// `remove` / `set_pinned` 用文字定位条目。判据与上游一致：只挡"空"与"过长"两件事，
/// **不挡纯空白**——一条全是空格的复制内容确实可能存在，它不是空的。
/// 不合规是**硬错误**（上游也回 `Err`），不是 `captured:false` 那种良性拒收：
/// 空文字定位不到任何条目，多半是调用方写错了，静默成功会把 bug 藏起来。
fn entry_text(text: Option<&str>) -> Result<&str, &'static str> {
    let Some(text) = text else {
        return Err("invalid_mobile_clipboard_request");
    };
    if text.is_empty() || text.len() > MAX_MOBILE_TEXT_BYTES {
        return Err("invalid_mobile_clipboard_entry");
    }
    Ok(text)
}

/// 按内容删，不按行号——宿主手里那份卡片列表随时可能是旧的（另一个进程刚删过或刚排过序）。
/// **没删掉任何东西就不写盘**：一次过时的删除不该把用户的历史重写一遍。
fn remove_entry(path: &Path, text: &str) -> io::Result<(bool, Vec<Entry>)> {
    let _lock = lock_history(path)?;
    let mut entries = load_entries(path)?;
    let before = entries.len();
    entries.retain(|entry| entry.text != text);
    let removed = before != entries.len();
    if removed {
        persist(path, &entries)?;
    }
    Ok((removed, entries))
}

/// 固定 / 取消固定，同样按内容定位。值没变就不写盘（幂等，也省一次 fsync），
/// 但仍回 `updated:true`——"这条存在且已经是你要的状态"是成功，不是失败。
fn pin_entry(path: &Path, text: &str, pinned: bool) -> io::Result<(bool, Vec<Entry>)> {
    let _lock = lock_history(path)?;
    let mut entries = load_entries(path)?;
    let Some(entry) = entries.iter_mut().find(|entry| entry.text == text) else {
        return Ok((false, entries));
    };
    if entry.pinned != pinned {
        entry.pinned = pinned;
        sort_entries(&mut entries);
        persist(path, &entries)?;
    }
    Ok((true, entries))
}

/// 把一条历史的文字改成 `replacement`。四种会写盘的写法之外，三种不写：
/// `Invalid`（且**在持锁之前**判，所以连目录与锁侧车都不出现）、`Unchanged`、`NotFound`。
/// "留在原位"不是靠跳过排序实现的，而是靠**不动时间戳**——`sort_entries` 之后它自然
/// 还占原来那一格。合并时固定状态取两者之或，且活下来的是**被编辑那一条**的时间戳。
fn replace_entry(path: &Path, text: &str, replacement: &str) -> io::Result<(Edit, Vec<Entry>)> {
    if !mobile_text_is_valid(replacement) {
        return Ok((Edit::Invalid, Vec::new()));
    }
    let _lock = lock_history(path)?;
    let mut entries = load_entries(path)?;
    let Some(index) = entries.iter().position(|entry| entry.text == text) else {
        return Ok((Edit::NotFound, entries));
    };
    if text == replacement {
        return Ok((Edit::Unchanged, entries));
    }
    let outcome = match entries.iter().position(|entry| entry.text == replacement) {
        Some(duplicate) => {
            entries[index].pinned |= entries[duplicate].pinned;
            entries[index].text = replacement.to_owned();
            entries.remove(duplicate);
            Edit::Merged
        }
        None => {
            entries[index].text = replacement.to_owned();
            Edit::Replaced
        }
    };
    sort_entries(&mut entries);
    persist(path, &entries)?;
    Ok((outcome, entries))
}

/// 清空历史。**没有档也算成功**（幂等）——宿主每 1 秒重载都会在开关关着时踩一次
/// `clearQuietly()`（`MSIMEInputService.java:2345`），第二、三次若回 `ok:false`，
/// `ClipboardHistoryStore.java:127` 就抛、`:99` 自吞并打 warning，刷屏原样回来。
/// 锁侧车刻意留在原地，理由见 `lock_path`。
fn clear_entries(path: &Path) -> io::Result<()> {
    let _lock = lock_history(path)?;
    reject_symlink(path)?;
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn history_path(directory: &Path) -> PathBuf {
    directory.join(STORAGE_DIRECTORY).join(FILE_NAME)
}

fn entries_of(entries: &[Entry]) -> Value {
    serde_json::to_value(entries).unwrap_or_else(|_| json!([]))
}

/// 移动端的合规判据：非空白、字节与图素簇双上限、不含 NUL 与其它控制字符里的坏东西。
/// 与上游 `client-core/src/clipboard.rs:364` 同形（上游另有一条 `valid_characters`，见那里的注释）。
fn mobile_text_is_valid(text: &str) -> bool {
    !text.trim().is_empty()
        && text.len() <= MAX_MOBILE_TEXT_BYTES
        && !text.contains('\0')
        && text.graphemes(true).count() <= MAX_MOBILE_TEXT_CHARACTERS
}

fn valid_stored(text: &str) -> bool {
    !text.is_empty() && mobile_text_is_valid(text)
}

/// 读一份历史档。缺档 = 空表；坏档 = `Err`（**绝不**当空表读，否则下一次 capture
/// 就会把"读不出来"变成"用户的历史没了"）。
fn load_entries(path: &Path) -> io::Result<Vec<Entry>> {
    let file = match open_private_file(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let bytes = read_bounded_file(&file)?;
    validate_item_count(&bytes)?;
    let stored: Stored = serde_json::from_slice(&bytes).map_err(|_| malformed())?;
    let mut values = match stored {
        Stored::Structured(values) => values,
        Stored::Legacy(values) => {
            let length = values.len() as u64;
            values
                .into_iter()
                .enumerate()
                .map(|(index, text)| Entry {
                    text,
                    timestamp_ms: length.saturating_sub(index as u64),
                    pinned: false,
                })
                .collect()
        }
    };
    values.retain(|value| valid_stored(&value.text));
    sort_entries(&mut values);
    let mut entries: Vec<Entry> = Vec::with_capacity(MAX_ENTRIES);
    for value in values {
        if !entries.iter().any(|entry| entry.text == value.text) {
            entries.push(value);
            if entries.len() == MAX_ENTRIES {
                break;
            }
        }
    }
    Ok(entries)
}

/// 一次 capture：持锁 → 重读最新 → 改 → 原子落盘。
/// **重读是必须的**：`:ime` 与前台 app 主进程都写同一份，宿主手里那份列表随时可能是旧的。
/// 返回 `(是否收下, 收下后的完整列表)`——列表全满且没有可挤掉的条目时收下为 `false`。
fn capture_entry(path: &Path, text: &str) -> io::Result<(bool, Vec<Entry>)> {
    let _lock = lock_history(path)?;
    let mut entries = load_entries(path)?;
    if let Some(index) = entries.iter().position(|entry| entry.text == text) {
        let mut moved = entries.remove(index);
        moved.timestamp_ms = next_timestamp(&entries);
        entries.push(moved);
    } else {
        if entries.len() == MAX_ENTRIES {
            let Some(index) = entries.iter().rposition(|entry| !entry.pinned) else {
                sort_entries(&mut entries);
                return Ok((false, entries));
            };
            entries.remove(index);
        }
        entries.push(Entry {
            text: text.to_owned(),
            timestamp_ms: next_timestamp(&entries),
            pinned: false,
        });
    }
    sort_entries(&mut entries);
    persist(path, &entries)?;
    Ok((true, entries))
}

/// 新条目永远比已有的最新一条更靠后，墙钟倒流（改系统时间、NTP 回调）也不会让顺序乱掉。
/// 对齐上游 `client-core/src/clipboard.rs:345`。
fn next_timestamp(entries: &[Entry]) -> u64 {
    let wall_clock = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|span| span.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0);
    entries
        .iter()
        .map(|entry| entry.timestamp_ms)
        .max()
        .unwrap_or(0)
        .saturating_add(1)
        .max(wall_clock)
}

/// 固定条目永远在前，其次按时间新到旧。对齐上游 `client-core/src/clipboard.rs:336`。
fn sort_entries(entries: &mut [Entry]) {
    entries.sort_by(|left, right| {
        right
            .pinned
            .cmp(&left.pinned)
            .then_with(|| right.timestamp_ms.cmp(&left.timestamp_ms))
    });
}

/// 原子落盘：同目录唯一暂存 → fsync → rename。暂存**必须与目标同目录**，
/// 跨目录的 rename 在 POSIX 上不是原子操作（甚至直接失败）。
fn persist(path: &Path, entries: &[Entry]) -> io::Result<()> {
    let bytes = serde_json::to_vec(entries)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    create_directory_and_check(parent)?;
    let staged = staged_path(path);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&staged)?;
    let outcome = (|| {
        file.write_all(&bytes)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        fs::rename(&staged, path)
    })();
    if outcome.is_err() {
        let _ = fs::remove_file(&staged);
    }
    outcome
}

fn staged_path(path: &Path) -> PathBuf {
    path.with_extension(format!("json.{}.tmp", process_stamp()))
}

/// 暂存名的唯一性来源：进程号 + 纳秒时钟。两个进程同时写时谁都不该撞上谁，
/// 撞上了就是 `create_new` 直接失败（不是静默覆盖）。
fn process_stamp() -> u128 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|span| span.as_nanos())
        .unwrap_or(0);
    nanos ^ (std::process::id() as u128) << 64
}

/// 写侧的排他锁。**跨进程**才有效，所以不能省：`:ime` 与前台 app 主进程各有一份 store。
/// 锁档是侧车（`<档名>.lock`）且刻意不删——删了另一个进程就会在同一个路径上锁到另一个 inode。
/// 锁侧车的路径：`<档名>.lock`。它**不随清空删除**——删了之后另一个进程
/// 会在同一路径上锁到一个新 inode，两个写者就各自持有一把锁了。
fn lock_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".lock");
    PathBuf::from(name)
}

fn lock_history(path: &Path) -> io::Result<File> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    create_directory_and_check(parent)?;
    let lock = lock_path(path);
    reject_symlink(&lock)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    options
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .mode(0o600);
    let file = options.open(lock)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || !has_single_link(&metadata) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "lock file is not a single-link regular file",
        ));
    }
    lock_exclusive(&file)?;
    Ok(file)
}

#[cfg(not(target_os = "android"))]
fn lock_exclusive(file: &File) -> io::Result<()> {
    file.lock()
}

/// Android 上标准库的文件锁不支持（上游 `file_lock.rs:1` 那句注释就是这件事），
/// 走 rustix 的 flock，并且把 EINTR 当成"再来一次"而不是失败。
#[cfg(target_os = "android")]
fn lock_exclusive(file: &File) -> io::Result<()> {
    loop {
        match rustix::fs::flock(file, rustix::fs::FlockOperation::LockExclusive) {
            Ok(()) => return Ok(()),
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => return Err(error.into()),
        }
    }
}

/// 以"不跟随符号链接、不截断、owner-only"的方式打开一份已有的私有档。
fn open_private_file(path: &Path) -> io::Result<File> {
    reject_symlink(path)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || !has_single_link(&metadata) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "history is not a single-link regular file",
        ));
    }
    Ok(file)
}

/// 读，但先按 `metadata.len()` 与字节上限各挡一次：一个 12 MB 的历史档
/// 不是"用户复制得多"，是有人在喂我们，直接拒读。
fn read_bounded_file(file: &File) -> io::Result<Vec<u8>> {
    if file.metadata()?.len() > MAX_HISTORY_BYTES {
        return Err(malformed());
    }
    let mut bytes = Vec::new();
    (&*file)
        .take(MAX_HISTORY_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_HISTORY_BYTES {
        return Err(malformed());
    }
    Ok(bytes)
}

/// 解码**之前**先数条目：`Vec<Entry>` 一旦解出来，内存已经花掉了。
/// 用一个只走结构的 visitor，数到上限就报错。对齐上游 `client-core/src/clipboard.rs:378`。
fn validate_item_count(bytes: &[u8]) -> io::Result<()> {
    use serde::de::{SeqAccess, Visitor};
    struct CountVisitor;
    impl<'de> Visitor<'de> for CountVisitor {
        type Value = ();
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a bounded clipboard history array")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<(), A::Error> {
            let mut count = 0usize;
            while sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {
                count += 1;
                if count > MAX_INPUT_ENTRIES {
                    return Err(serde::de::Error::custom(
                        "too many clipboard history entries",
                    ));
                }
            }
            Ok(())
        }
    }
    let mut reader = serde_json::Deserializer::from_slice(bytes);
    serde::Deserializer::deserialize_any(&mut reader, CountVisitor)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "unbounded history array"))
}

fn malformed() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid clipboard history document",
    )
}

/// 目录不存在就建，但建之前先看最近的已存在祖先有没有被符号链接替换过。
/// 上游靠一张"可信系统别名"表放行 `/data` 这类系统级链接（`msime_path_trust`），
/// 我们没有那张表，所以只对自己创建的那一段严格：`MSIME` 与它下面的档。
fn create_directory_and_check(path: &Path) -> io::Result<()> {
    reject_symlink(path)?;
    fs::create_dir_all(path)?;
    if !fs::symlink_metadata(path)?.file_type().is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "clipboard history parent is not a directory",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn reject_symlink(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "clipboard history path is a symbolic link",
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(not(unix))]
fn reject_symlink(_path: &Path) -> io::Result<()> {
    // Windows 主机上这一层是空操作：创建符号链接需要特权，单测也就造不出攻击现场。
    // 真机（Android）走的是上面那条 cfg(unix) 分支。
    Ok(())
}

#[cfg(unix)]
fn has_single_link(metadata: &fs::Metadata) -> bool {
    metadata.nlink() == 1
}

#[cfg(not(unix))]
fn has_single_link(_metadata: &fs::Metadata) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 每个测试一个独立目录，跑完留在原地——失败时直接 `cat` 比重新造现场有用。
    fn sandbox(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join("qingshan-clipboard").join(name);
        let _ = fs_remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("建沙箱目录");
        root
    }

    fn fs_remove_dir_all(path: &Path) -> io::Result<()> {
        match std::fs::remove_dir_all(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            rest => rest,
        }
    }

    fn request_bytes(directory: &Path, action: Value) -> Vec<u8> {
        json!({
            "directory": directory.to_string_lossy(),
            "action": action
        })
        .to_string()
        .into_bytes()
    }

    fn value_of(reply: Reply) -> Value {
        match reply {
            Reply::Ok(value) => value,
            Reply::Fail(reason) => panic!("期望 ok:true，却被拒绝：{reason}"),
        }
    }

    fn texts(entries: &Value) -> Vec<String> {
        entries["entries"]
            .as_array()
            .expect("应答里必须有 entries 数组")
            .iter()
            .map(|entry| entry["text"].as_str().expect("每条都有 text").to_owned())
            .collect()
    }

    fn capture(directory: &Path, text: &str) -> Value {
        value_of(handle(&request_bytes(
            directory,
            json!({ "operation": "capture", "text": text }),
        )))
    }

    /// 盘上真实躺着的那几条。凡是"读侧顺手也能兜住"的规则，都要用它来问写侧。
    fn stored_entries(directory: &Path) -> Vec<Entry> {
        let bytes = std::fs::read(history_path(directory)).expect("历史档应在");
        serde_json::from_slice(&bytes).expect("盘上应是结构化数组")
    }

    fn load(directory: &Path) -> Value {
        value_of(handle(&request_bytes(
            directory,
            json!({ "operation": "load" }),
        )))
    }

    #[test]
    fn 没有文件时读到空表而不是错误() {
        let directory = sandbox("empty");
        let body = load(&directory);
        assert_eq!(body["entries"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn capture写进去的条目load读得回来() {
        let directory = sandbox("roundtrip");
        let written = capture(&directory, "验证码 123456");
        assert_eq!(written["captured"], json!(true));
        assert_eq!(texts(&load(&directory)), vec!["验证码 123456".to_owned()]);
    }

    #[test]
    fn 重复文字不新增条目只把它挪到最前() {
        let directory = sandbox("dedupe");
        capture(&directory, "第一段");
        capture(&directory, "第二段");
        capture(&directory, "第一段");
        assert_eq!(
            texts(&load(&directory)),
            vec!["第一段".to_owned(), "第二段".to_owned()]
        );
        // 只看 load 的话，"读的时候顺手去重"也能过这条——所以直接数盘上有几条。
        // 写侧不去重的后果是磁盘无限长，而每次读都要重排一遍。
        let stored: Vec<Entry> =
            serde_json::from_slice(&std::fs::read(history_path(&directory)).unwrap()).unwrap();
        assert_eq!(
            stored.len(),
            2,
            "盘上只该有两条，实际 {}",
            serde_json::to_string(&stored).unwrap()
        );
    }

    #[test]
    fn 超过五十条时挤掉最旧的非固定条目() {
        let directory = sandbox("cap");
        for index in 0..MAX_ENTRIES {
            capture(&directory, &format!("条目 {index:02}"));
        }
        assert_eq!(texts(&load(&directory)).len(), MAX_ENTRIES);
        capture(&directory, "最新的一条");
        let after = texts(&load(&directory));
        assert_eq!(after.len(), MAX_ENTRIES);
        assert_eq!(after.first().map(String::as_str), Some("最新的一条"));
        assert!(
            !after.contains(&"条目 00".to_owned()),
            "最旧的非固定条目应当被挤掉，实际还剩 {after:?}"
        );
        // 写侧的上限必须自己守住：只靠读侧截断的话，盘会一直长，每次读都要重排一遍。
        assert_eq!(
            stored_entries(&directory).len(),
            MAX_ENTRIES,
            "盘上就该只有 50 条"
        );
    }

    /// 全满且带固定条目时的规则：挤掉最旧的**未固定**那条，固定的原地不动。
    /// 本切片没有 `set_pinned`（还在桩里），所以直接手写一份带 pinned 的档进来。
    /// 注意总数必须是 50（不是 51）：超过 50 的档在**读**的时候就被截断了，
    /// 截掉的正是一条未固定的，那样这条测试测的就不是写侧的挤掉规则。
    #[test]
    fn 全满时固定条目不被挤掉() {
        let directory = sandbox("pinned");
        std::fs::create_dir_all(directory.join(STORAGE_DIRECTORY)).unwrap();
        let path = history_path(&directory);
        let mut entries = Vec::new();
        for index in 0..MAX_ENTRIES - 1 {
            entries.push(Entry {
                text: format!("固定 {index:02}"),
                timestamp_ms: 1_000 + index as u64,
                pinned: true,
            });
        }
        entries.push(Entry {
            text: "唯一没固定的一条".to_owned(),
            timestamp_ms: 900,
            pinned: false,
        });
        assert_eq!(entries.len(), MAX_ENTRIES);
        std::fs::write(&path, serde_json::to_vec(&entries).unwrap()).unwrap();
        let body = capture(&directory, "又复制了一段");
        assert_eq!(body["captured"], json!(true));
        let after = texts(&load(&directory));
        assert_eq!(after.len(), MAX_ENTRIES);
        // 排序是"固定的永远在前"，所以新条目不会在最前——它是唯一那条未固定的，落在末尾。
        assert_eq!(after.last().map(String::as_str), Some("又复制了一段"));
        assert!(
            !after.contains(&"唯一没固定的一条".to_owned()),
            "该被挤掉的是那条未固定的"
        );
        for index in 0..MAX_ENTRIES - 1 {
            assert!(
                after.contains(&format!("固定 {index:02}")),
                "固定条目 {index:02} 不该被挤掉"
            );
        }
        assert_eq!(
            after[..MAX_ENTRIES - 1].to_vec(),
            (0..MAX_ENTRIES - 1)
                .rev()
                .map(|index| format!("固定 {index:02}"))
                .collect::<Vec<String>>(),
            "固定的 49 条要整块排在前面，且时间新的在前"
        );
    }

    /// 50 条全固定 = 没有可挤掉的对象。这不是故障：上游回 `captured:false` +
    /// `reason:"full"`，宿主把它画成「50 条历史均已固定」的提示。
    #[test]
    fn 全固定时拒收新条目而不是丢旧的() {
        let directory = sandbox("full");
        std::fs::create_dir_all(directory.join(STORAGE_DIRECTORY)).unwrap();
        let entries: Vec<Entry> = (0..MAX_ENTRIES)
            .map(|index| Entry {
                text: format!("固定 {index:02}"),
                timestamp_ms: 1_000 + index as u64,
                pinned: true,
            })
            .collect();
        std::fs::write(
            history_path(&directory),
            serde_json::to_vec(&entries).unwrap(),
        )
        .unwrap();
        let body = capture(&directory, "挤不进去的一条");
        assert_eq!(body["captured"], json!(false));
        assert_eq!(body["reason"], json!("full"));
        assert!(
            !texts(&load(&directory)).contains(&"挤不进去的一条".to_owned()),
            "拒收不能顺手把旧的挤掉"
        );
    }

    #[test]
    fn 纯空白与超长文字被拒且不落盘() {
        let directory = sandbox("reject");
        let blank = capture(&directory, "   \n ");
        assert_eq!(blank["captured"], json!(false));
        assert_eq!(blank["reason"], json!("invalid"));
        let too_long = capture(&directory, &"字".repeat(MAX_MOBILE_TEXT_CHARACTERS + 1));
        assert_eq!(too_long["reason"], json!("invalid"));
        assert!(
            !history_path(&directory).exists(),
            "两次拒收都不该产生历史档"
        );
    }

    #[test]
    fn 坏文档报不可读且原档不被覆盖() {
        let directory = sandbox("corrupt");
        capture(&directory, "还能读的一条");
        let path = history_path(&directory);
        std::fs::write(&path, b"{ this is not a history document").unwrap();
        match handle(&request_bytes(&directory, json!({ "operation": "load" }))) {
            Reply::Fail(_) => {}
            Reply::Ok(body) => panic!("坏文档不该被当成空表读出来：{body}"),
        }
        assert_eq!(
            std::fs::read(&path).expect("坏文档应仍在原地"),
            b"{ this is not a history document".to_vec()
        );
        // 读不出来不等于允许写：capture 同样必须拒绝，否则一次复制就把用户的旧档清了。
        match handle(&request_bytes(
            &directory,
            json!({ "operation": "capture", "text": "趁乱写一条" }),
        )) {
            Reply::Fail(_) => {}
            Reply::Ok(body) => panic!("坏文档之上不该直接覆盖：{body}"),
        }
        assert_eq!(
            std::fs::read(&path).expect("坏文档仍在原地"),
            b"{ this is not a history document".to_vec()
        );
    }

    #[test]
    fn 结构对但字段缺一条也算坏档() {
        // 语法完全合法、结构也像个数组，只有条目字段不全。validate_item_count 放行，
        // 真正挡住它的是类型化解析那一步——所以这条锁的是"解析失败不当空表读"，
        // 上面那条锁的是"语法失败不当空表读"，两条不是同一条路径。
        let directory = sandbox("partial");
        std::fs::create_dir_all(directory.join(STORAGE_DIRECTORY)).unwrap();
        std::fs::write(
            history_path(&directory),
            r#"[{"text":"少了两个字段"}]"#.as_bytes(),
        )
        .unwrap();
        match handle(&request_bytes(&directory, json!({ "operation": "load" }))) {
            Reply::Fail(_) => {}
            Reply::Ok(body) => panic!("字段不全的档不该被当成空表：{body}"),
        }
    }

    #[test]
    fn 裸字符串数组读得进来并在下次写入时升级() {
        let directory = sandbox("legacy");
        std::fs::create_dir_all(directory.join("MSIME")).unwrap();
        std::fs::write(
            history_path(&directory),
            r#"["最新的","较早的","最早的"]"#.as_bytes(),
        )
        .unwrap();
        assert_eq!(
            texts(&load(&directory)),
            vec![
                "最新的".to_owned(),
                "较早的".to_owned(),
                "最早的".to_owned()
            ]
        );
        capture(&directory, "新的一条");
        let stored = std::fs::read_to_string(history_path(&directory)).unwrap();
        assert!(
            stored.contains("timestampMs"),
            "一次成功的写入应把旧格式升级成结构化：{stored}"
        );
    }

    #[test]
    fn 写路径不留暂存文件且与目标同目录() {
        let directory = sandbox("atomic");
        capture(&directory, "一条");
        let entries: Vec<String> = std::fs::read_dir(directory.join("MSIME"))
            .unwrap()
            .map(|row| row.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        let leftovers: Vec<&String> = entries
            .iter()
            .filter(|name| name.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "不该有暂存文件残留：{leftovers:?}");
        assert!(
            entries.iter().any(|name| name == FILE_NAME),
            "历史档必须落在 <directory>/MSIME 下，实际 {entries:?}"
        );
    }

    #[test]
    fn 未知操作仍回未实现() {
        let directory = sandbox("stubbed");
        let reason = match handle(&request_bytes(
            &directory,
            json!({ "operation": "teleport" }),
        )) {
            Reply::Fail(reason) => reason,
            Reply::Ok(body) => panic!("teleport 竟回了 ok:true：{body}"),
        };
        assert_eq!(reason, "not_implemented");
    }

    /// `replace` 的应答体两个布尔字段必须**永远都在**：宿主
    /// `ClipboardHistoryStore.java:78-81` 先读 `replaced` 再读 `merged`，
    /// 少一个字段就少判一种结局。
    fn replace(directory: &Path, text: &str, replacement: &str) -> Value {
        value_of(handle(&request_bytes(
            directory,
            json!({
                "operation": "replace",
                "text": text,
                "replacement": replacement
            }),
        )))
    }

    fn stored_timestamps(directory: &Path) -> Vec<(String, u64, bool)> {
        stored_entries(directory)
            .iter()
            .map(|entry| (entry.text.clone(), entry.timestamp_ms, entry.pinned))
            .collect()
    }

    #[test]
    fn replace改内容后条目留在原位() {
        let directory = sandbox("replace_basic");
        capture(&directory, "甲");
        capture(&directory, "乙");
        capture(&directory, "丙");
        let before = stored_timestamps(&directory);
        let 乙的时间戳 = before
            .iter()
            .find(|(text, _, _)| text == "乙")
            .expect("前提：乙在")
            .1;
        let body = replace(&directory, "乙", "乙改");
        assert_eq!(body["replaced"], json!(true));
        assert_eq!(body["merged"], json!(false));
        assert_eq!(
            texts(&body),
            vec!["丙".to_owned(), "乙改".to_owned(), "甲".to_owned()],
            "条数不变、只有内容换、顺序不换"
        );
        let after = stored_timestamps(&directory);
        assert_eq!(after.len(), 3);
        assert_eq!(
            after
                .iter()
                .find(|(text, _, _)| text == "乙改")
                .expect("乙改在盘上")
                .1,
            乙的时间戳,
            "编辑不该让它跳到最前——它靠时间戳占位，改文字不动时间戳"
        );
    }

    #[test]
    fn replace成新内容与已有条目相同时合并并取固定的或() {
        let directory = sandbox("replace_merge");
        capture(&directory, "甲");
        capture(&directory, "乙");
        capture(&directory, "丙");
        handle(&request_bytes(
            &directory,
            json!({ "operation": "set_pinned", "text": "乙", "pinned": true }),
        ));
        // 顺序：固定的乙在最前，其余按时间新的在前。
        assert_eq!(
            texts(&load(&directory)),
            vec!["乙".to_owned(), "丙".to_owned(), "甲".to_owned()]
        );
        let 甲的时间戳 = stored_timestamps(&directory)
            .iter()
            .find(|(text, _, _)| text == "甲")
            .expect("前提：甲在")
            .1;
        let body = replace(&directory, "甲", "乙");
        assert_eq!(body["replaced"], json!(true));
        assert_eq!(
            body["merged"],
            json!(true),
            "撞上已有条目要说合并，宿主据此弹「合并成了一条」"
        );
        // 第三条（丙）是故意留的：合并后若只剩一条，"要不要重排序"根本观察不到。
        assert_eq!(
            texts(&body),
            vec!["乙".to_owned(), "丙".to_owned()],
            "合并完固定的那条还必须在最前——不重排序就会让丙插到它前面"
        );
        let after = stored_timestamps(&directory);
        assert_eq!(after.len(), 2, "两条并一条");
        assert_eq!(
            after[0].1, 甲的时间戳,
            "合并后留在被编辑那条的位置，不是活下来那条的位置"
        );
        assert!(
            after[0].2,
            "固定状态取两者之或：乙本来固定，合并后这条仍是固定的"
        );
    }

    #[test]
    fn replace成新内容与原内容相同时一字不改() {
        // 60 条的档：多余的写会把它截成 50 条 ⇒ 字节比较测不出"没写盘"，这个能。
        let directory = sandbox("replace_unchanged");
        std::fs::create_dir_all(directory.join(STORAGE_DIRECTORY)).unwrap();
        let path = history_path(&directory);
        let many: Vec<Entry> = (0..MAX_ENTRIES + 10)
            .map(|index| Entry {
                text: format!("条目 {index:03}"),
                timestamp_ms: 1_000 + index as u64,
                pinned: false,
            })
            .collect();
        std::fs::write(&path, serde_json::to_vec(&many).unwrap()).unwrap();
        let before = std::fs::read(&path).unwrap();
        let body = replace(&directory, "条目 059", "条目 059");
        assert_eq!(body["replaced"], json!(true), "内容已经对了，算成功");
        assert_eq!(body["merged"], json!(false));
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "Unchanged 也有一次写盘 ⇒ 60 条被截成 50 条"
        );
    }

    #[test]
    fn replace定位不到时幂等成功且不写盘() {
        let directory = sandbox("replace_not_found");
        capture(&directory, "在的一条");
        let path = history_path(&directory);
        let before = std::fs::read(&path).unwrap();
        let body = replace(&directory, "早就没了的一条", "换成新的");
        assert_eq!(body["replaced"], json!(false));
        assert_eq!(body["merged"], json!(false));
        assert_eq!(
            body["reason"],
            json!("not_found"),
            "宿主据此弹「这条记录已经不在了」"
        );
        assert_eq!(texts(&body), vec!["在的一条".to_owned()]);
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn replace的新内容不合规时连磁盘都不碰() {
        // 上游把合规判定放在**持锁之前**（`client-core/src/clipboard.rs` 的 249 到 251 行），
        // 所以一次非法编辑既不该建档、也不该建锁侧车。
        let directory = sandbox("replace_invalid");
        for replacement in ["", "   ", &"字".repeat(MAX_MOBILE_TEXT_CHARACTERS + 1)] {
            let body = replace(&directory, "不存在也无所谓", replacement);
            assert_eq!(body["replaced"], json!(false));
            assert_eq!(body["reason"], json!("invalid"));
        }
        assert!(
            !directory.join(STORAGE_DIRECTORY).exists(),
            "非法编辑不该创建 MSIME 目录或任何档"
        );
    }

    #[test]
    fn replace缺replacement字段算请求不合法() {
        let directory = sandbox("replace_missing_field");
        capture(&directory, "甲");
        let reason = match handle(&request_bytes(
            &directory,
            json!({ "operation": "replace", "text": "甲" }),
        )) {
            Reply::Fail(reason) => reason,
            Reply::Ok(body) => panic!("缺 replacement 竟回了 ok:true：{body}"),
        };
        assert_eq!(reason, "invalid_mobile_clipboard_request");
        assert_eq!(texts(&load(&directory)), vec!["甲".to_owned()]);
    }

    #[test]
    fn remove删掉指定那条其余原样() {
        let directory = sandbox("remove");
        capture(&directory, "甲");
        capture(&directory, "乙");
        capture(&directory, "丙");
        let body = value_of(handle(&request_bytes(
            &directory,
            json!({ "operation": "remove", "text": "乙" }),
        )));
        assert_eq!(body["removed"], json!(true));
        assert_eq!(texts(&body), vec!["丙".to_owned(), "甲".to_owned()]);
        assert_eq!(
            texts(&load(&directory)),
            vec!["丙".to_owned(), "甲".to_owned()],
            "应答里的列表必须与盘上一致，否则面板下一次读到的和这次显示的不是一回事"
        );
    }

    #[test]
    fn remove对不存在的条目算成功且不写盘() {
        // 幂等：宿主手里那份卡片列表随时可能过时（另一个进程刚删过）。
        // 算成功，但不能顺手把盘重写一遍。
        let directory = sandbox("remove_missing");
        capture(&directory, "唯一的一条");
        let path = history_path(&directory);
        let before = std::fs::read(&path).unwrap();
        let body = value_of(handle(&request_bytes(
            &directory,
            json!({ "operation": "remove", "text": "早就不在了的一条" }),
        )));
        assert_eq!(body["removed"], json!(false));
        assert_eq!(texts(&body), vec!["唯一的一条".to_owned()]);
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "没删掉任何东西就不该写盘"
        );
    }

    #[test]
    fn set_pinned置真后该条排到最前() {
        let directory = sandbox("pin");
        capture(&directory, "甲");
        capture(&directory, "乙");
        capture(&directory, "丙");
        let body = value_of(handle(&request_bytes(
            &directory,
            json!({
                "operation": "set_pinned",
                "text": "甲",
                "pinned": true
            }),
        )));
        assert_eq!(body["updated"], json!(true));
        assert_eq!(body["entries"][0]["text"], json!("甲"));
        assert_eq!(body["entries"][0]["pinned"], json!(true));
        assert_eq!(
            texts(&load(&directory)),
            vec!["甲".to_owned(), "丙".to_owned(), "乙".to_owned()],
            "固定的永远在前，其余按时间新的在前"
        );
    }

    #[test]
    fn set_pinned取消后置假且排序回落() {
        let directory = sandbox("unpin");
        capture(&directory, "甲");
        capture(&directory, "乙");
        handle(&request_bytes(
            &directory,
            json!({ "operation": "set_pinned", "text": "甲", "pinned": true }),
        ));
        let body = value_of(handle(&request_bytes(
            &directory,
            json!({ "operation": "set_pinned", "text": "甲", "pinned": false }),
        )));
        assert_eq!(body["updated"], json!(true));
        assert_eq!(
            texts(&load(&directory)),
            vec!["乙".to_owned(), "甲".to_owned()],
            "取消固定后该条要回到时间序里，不该继续占在最前"
        );
        assert_eq!(stored_entries(&directory)[1].pinned, false);
    }

    #[test]
    fn set_pinned对不存在的条目算成功但不写盘() {
        // 与 remove 同一类：宿主手里那份列表可能已经过时。回 updated:false 是实话，
        // 但不是故障——不能因此让面板报「无法修改剪贴板历史」。
        let directory = sandbox("pin_missing");
        capture(&directory, "唯一的一条");
        let path = history_path(&directory);
        let before = std::fs::read(&path).unwrap();
        let body = value_of(handle(&request_bytes(
            &directory,
            json!({ "operation": "set_pinned", "text": "早就不在了的一条", "pinned": true }),
        )));
        assert_eq!(body["updated"], json!(false));
        assert_eq!(texts(&body), vec!["唯一的一条".to_owned()]);
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "什么都没改就不该写盘"
        );
    }

    #[test]
    fn 过时的删除不会顺手截断盘上那份() {
        // 「没删掉东西就不写盘」这条规则的真正代价，用"内容一样"是测不出来的
        // （重写一遍同样的数组，字节完全相同）。所以造一份超过 50 条的档：
        // 读的时候会被截到 50，若删失败还照样写盘，用户第 51 条以后就凭空消失了。
        let directory = sandbox("no_truncate");
        std::fs::create_dir_all(directory.join(STORAGE_DIRECTORY)).unwrap();
        let path = history_path(&directory);
        let many: Vec<Entry> = (0..MAX_ENTRIES + 10)
            .map(|index| Entry {
                text: format!("条目 {index:03}"),
                timestamp_ms: 1_000 + index as u64,
                pinned: false,
            })
            .collect();
        std::fs::write(&path, serde_json::to_vec(&many).unwrap()).unwrap();
        let before = std::fs::read(&path).unwrap();
        let body = value_of(handle(&request_bytes(
            &directory,
            json!({ "operation": "remove", "text": "根本不存在的一条" }),
        )));
        assert_eq!(body["removed"], json!(false));
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "删失败就把 60 条截成了 50 条 ⇒ 那次写盘必须根本不发生"
        );
        // 同一条规则对 set_pinned 也成立：固定一条不存在的，不该动盘。
        value_of(handle(&request_bytes(
            &directory,
            json!({ "operation": "set_pinned", "text": "根本不存在的另一条", "pinned": true }),
        )));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        // 以及"已经是这个值"：置一个本来就 false 的条目为 false，也不该写盘。
        // 这条断言不是洁癖——上面那份 60 条的档一旦被重写就会被截成 50 条。
        let again = value_of(handle(&request_bytes(
            &directory,
            json!({ "operation": "set_pinned", "text": "条目 059", "pinned": false }),
        )));
        assert_eq!(again["updated"], json!(true), "状态已经对了，算成功");
        assert_eq!(std::fs::read(&path).unwrap(), before, "值没变就不该动盘");
    }

    #[test]
    fn set_pinned缺pinned字段算请求不合法() {
        // 宿主只在 set_pinned 时带 pinned（ClipboardHistoryStore.java:113）。
        // 缺了就是有人在乱发请求，不能猜一个默认值——猜成 true 会凭空固定一条。
        let directory = sandbox("pin_missing_field");
        capture(&directory, "甲");
        let reason = match handle(&request_bytes(
            &directory,
            json!({ "operation": "set_pinned", "text": "甲" }),
        )) {
            Reply::Fail(reason) => reason,
            Reply::Ok(body) => panic!("缺 pinned 竟回了 ok:true：{body}"),
        };
        assert_eq!(reason, "invalid_mobile_clipboard_request");
        assert_eq!(texts(&load(&directory)), vec!["甲".to_owned()]);
    }

    #[test]
    fn 空文字的删除与固定被拒() {
        let directory = sandbox("empty_text_entry");
        capture(&directory, "甲");
        for operation in ["remove", "set_pinned"] {
            let request = request_bytes(&directory, json!({ "operation": operation, "text": "" }));
            let reason = match handle(&request) {
                Reply::Fail(reason) => reason,
                Reply::Ok(body) => panic!("{operation} 的空 text 竟回了 ok:true：{body}"),
            };
            assert_eq!(reason, "invalid_mobile_clipboard_entry");
        }
    }

    #[test]
    fn clear之后档不存在且读回来是空表() {
        let directory = sandbox("clear");
        capture(&directory, "要被清掉的一条");
        assert!(history_path(&directory).exists(), "前提：先要有一条");
        let body = value_of(handle(&request_bytes(
            &directory,
            json!({ "operation": "clear" }),
        )));
        assert_eq!(body["cleared"], json!(true));
        assert_eq!(body["entries"].as_array().unwrap().len(), 0);
        assert!(
            !history_path(&directory).exists(),
            "「关闭即清空」要真的把明文从盘上拿掉，留一个空数组等于没清"
        );
        assert_eq!(load(&directory)["entries"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn clear对没有历史档的目录算成功() {
        // 幂等：宿主每 1 秒重载都会踩一次清空，第一次之后档就不存在了。
        // 第二、三次若回 ok:false，那句 "Clipboard history clear refused" 就又会刷屏。
        let directory = sandbox("clear_missing");
        for round in 0..3 {
            let body = value_of(handle(&request_bytes(
                &directory,
                json!({ "operation": "clear" }),
            )));
            assert_eq!(body["cleared"], json!(true), "第 {round} 次清空");
        }
    }

    #[test]
    fn clear之后还能再收一条() {
        // 锁侧车刻意不删（删了另一个进程会在同一路径上锁到另一个 inode），
        // 所以清空之后写侧必须照常用。这条同时锁住"侧车没被顺手删掉"。
        let directory = sandbox("clear_then_capture");
        capture(&directory, "第一条");
        handle(&request_bytes(&directory, json!({ "operation": "clear" })));
        let body = capture(&directory, "清空之后又复制的一条");
        assert_eq!(body["captured"], json!(true));
        assert_eq!(
            texts(&load(&directory)),
            vec!["清空之后又复制的一条".to_owned()]
        );
        assert!(
            lock_path(&history_path(&directory)).exists(),
            "锁侧车应留在原地"
        );
    }

    #[test]
    fn 目录不是绝对路径时拒绝而不是往当前目录写() {
        let request = json!({
            "directory": "bootstrap/state",
            "action": { "operation": "load" }
        })
        .to_string()
        .into_bytes();
        match handle(&request) {
            Reply::Fail(_) => {}
            Reply::Ok(body) => panic!("相对路径必须被拒：{body}"),
        }
    }
}
