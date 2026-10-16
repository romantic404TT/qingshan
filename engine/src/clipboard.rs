//! 剪贴板历史的共享存储。宿主侧 `ClipboardHistoryStore.java:106-134` 把六个操作
//! 打包成一份 `{directory, action}` 请求递过来，这一层负责落盘与取回。
//!
//! **切片 1 只做 `load` 与 `capture`。** 其余四个（`remove` / `set_pinned` /
//! `replace` / `clear`）继续回 `not_implemented`，与它们还是桩时逐字同形——
//! 尤其 `clear` 保持拒绝，宿主那句每秒一次的
//! `Clipboard history clear refused` 才会照旧出现（本轮验收要拿它当"写路径没做"的证据）。

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
        "clear" => match clear_entries(&path) {
            Ok(()) => Reply::Ok(json!({ "cleared": true, "entries": [] })),
            Err(_) => Reply::Fail("mobile_clipboard_clear_failed"),
        },
        _ => Reply::Fail("not_implemented"),
    }
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
    fn 其余三个操作与未知操作仍回未实现() {
        let directory = sandbox("stubbed");
        for operation in ["remove", "set_pinned", "replace", "teleport"] {
            let request = request_bytes(&directory, json!({ "operation": operation }));
            let reason = match handle(&request) {
                Reply::Fail(reason) => reason,
                Reply::Ok(body) => panic!("{operation} 竟回了 ok:true：{body}"),
            };
            assert_eq!(reason, "not_implemented", "{operation} 应回未实现");
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
