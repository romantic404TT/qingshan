//! 随包资源锁与 `refreshHostRaw` 的长度比对（PIN「refreshHostRaw：方案① 立项」）。
//!
//! 锁文件烘进二进制，刷新时只 `stat` 资源目录里**锁列的那几个文件**比长度：不符就回
//! `dictionary_outdated:`，宿主 `Bootstrap.refreshExistingConfiguration` 才会重新解包——
//! 这一条关掉的是 README「已知失效项」第 8 条（升级安装包不重解包）。
//!
//! 上游口径照 `crates/host-api/src/lib.rs:1626-1640`：**只有长度/内容不符**算过期，
//! 配置读不出、锁本身非法都**不算**（重新下载修不好它）。
//!
//! 锁的 `bytes` 由 `tools/build_glossary_subset.py` 生成表时一起写出；本模块末尾那条
//! `packaged_lock_matches_shipped_assets` 单测负责在两者脱节时立刻红，不靠人记得住。

use std::path::{Path, PathBuf};

/// 随包锁：与 APK 里 `assets/desktop-dictionary.lock.json` 是同一份文件。
pub const PACKAGED_LOCK: &str = include_str!("../../host/assets/desktop-dictionary.lock.json");

/// 上游错误文本前缀（`crates/host-api/src/lib.rs:1626`），宿主按 `startsWith` 判定
/// （`Bootstrap.java:375`）。
pub const OUTDATED_PREFIX: &str = "dictionary_outdated:";

/// 配置文件长度上限，与宿主 `Bootstrap.readConfiguredResources` 的 1 MiB 同口径。
const MAX_OPTIONS_BYTES: u64 = 1_048_576;

/// 锁里的一项：文件名 + 随包字节数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockedArtifact {
    pub name: String,
    pub bytes: u64,
}

/// 一次刷新的结论。
#[derive(Debug, PartialEq, Eq)]
pub enum Refresh {
    /// 资源目录与锁一致，宿主继续用现有配置。
    Ready,
    /// 长度不符或文件缺失。**只有这一种**允许带 `dictionary_outdated:` 前缀。
    Outdated(String),
    /// 配置读不出、锁非法：回错误但不带过期前缀，宿主因此不动资源目录。
    Blocked(String),
}

impl Refresh {
    /// 交给信封的错误文本；`Ready` 回 `None`（调用方发 `ok:true`）。
    ///
    /// 文本里只出现文件名，不出现绝对路径——宿主那条日志的注释明确要求「不含路径」。
    pub fn error(&self) -> Option<String> {
        match self {
            Refresh::Ready => None,
            Refresh::Outdated(detail) => Some(format!("{OUTDATED_PREFIX} {detail}")),
            Refresh::Blocked(reason) => Some(format!("refresh_{reason}")),
        }
    }
}

/// 与宿主 `Bootstrap.packagedArtifacts` 同一条规则：正则 `[A-Za-z0-9_.-]+` 且不含 `..`
/// （`Bootstrap.java:278`）。`/` 与 `\` 不在字符类里，所以被字符集直接挡掉。
fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains("..")
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
}

/// 解析锁文本。字段缺任何一样都是构建期错误（`Err`），不是运行期资源问题。
pub fn parse_lock(text: &str) -> Result<Vec<LockedArtifact>, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| format!("lock_unparsable:{error}"))?;
    let items = value
        .get("artifacts")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "lock_missing_artifacts".to_owned())?;
    let mut artifacts = Vec::with_capacity(items.len());
    for item in items {
        let name = item
            .get("name")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "lock_artifact_without_name".to_owned())?;
        if !is_safe_name(name) {
            return Err(format!("lock_invalid_name:{name}"));
        }
        let bytes = item
            .get("bytes")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| format!("lock_artifact_without_bytes:{name}"))?;
        artifacts.push(LockedArtifact {
            name: name.to_owned(),
            bytes,
        });
    }
    if artifacts.is_empty() {
        return Err("lock_empty".to_owned());
    }
    Ok(artifacts)
}

/// 比长度。只看锁里列的名字，所以 `helpcodes/` 与日文那一组（`Bootstrap.java:246-249`、
/// `JAPANESE_ARTIFACTS`）天然不参与——它们本就不在锁里，不是靠这里判断豁免的。
pub fn check(artifacts: &[LockedArtifact], resources: &Path) -> Refresh {
    for artifact in artifacts {
        let path = resources.join(&artifact.name);
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(_) => return Refresh::Outdated(format!("{}:missing", artifact.name)),
        };
        if !meta.is_file() {
            return Refresh::Outdated(format!("{}:not-a-file", artifact.name));
        }
        if meta.len() != artifact.bytes {
            return Refresh::Outdated(format!(
                "{}:length-{}-want-{}",
                artifact.name,
                meta.len(),
                artifact.bytes
            ));
        }
    }
    Refresh::Ready
}

/// 读配置里的 `resources` 目录。读不出（非法 JSON、缺键、空值）一律 `Err`。
fn configured_resources(text: &str) -> Result<PathBuf, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| format!("options_unparsable:{error}"))?;
    let resources = value
        .get("resources")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "options_without_resources".to_owned())?;
    if resources.is_empty() {
        return Err("options_empty_resources".to_owned());
    }
    Ok(PathBuf::from(resources))
}

/// 用随包锁刷新一份配置。
pub fn refresh_with_packaged_lock(path: &Path) -> Refresh {
    refresh(path, PACKAGED_LOCK)
}

/// 同上，但锁文本由调用方给（单测用它构造不符场景）。
pub fn refresh(path: &Path, lock_text: &str) -> Refresh {
    let artifacts = match parse_lock(lock_text) {
        Ok(artifacts) => artifacts,
        Err(reason) => return Refresh::Blocked(reason),
    };
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(_) => return Refresh::Blocked("options_missing".to_owned()),
    };
    if !meta.is_file() {
        return Refresh::Blocked("options_not_regular".to_owned());
    }
    if meta.len() > MAX_OPTIONS_BYTES {
        return Refresh::Blocked("options_too_large".to_owned());
    }
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(_) => return Refresh::Blocked("options_unreadable".to_owned()),
    };
    let resources = match configured_resources(&text) {
        Ok(resources) => resources,
        Err(reason) => return Refresh::Blocked(reason),
    };
    check(&artifacts, &resources)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const LOCK: &str = concat!(
        r#"{"artifacts":[{"name":"dict.tsv","bytes":12},"#,
        r#"{"name":"glossary-en.tsv","bytes":34}]}"#,
    );

    /// 每个用例一个独立子目录；名字写死，跑前清一次，避免上一次的残留冒充本次结果。
    fn sandbox(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("qingshan-refresh-tests")
            .join(tag);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("bootstrap").join("resources")).expect("建临时资源目录");
        dir
    }

    fn fill(dir: &Path, name: &str, len: usize) {
        fs::write(dir.join(name), vec![b'x'; len]).expect("写测试文件");
    }

    fn options(dir: &Path) -> PathBuf {
        let path = dir.join("runtime-options.json");
        let resources = dir.join("bootstrap").join("resources");
        let body = serde_json::json!({ "resources": resources.to_string_lossy() });
        fs::write(&path, body.to_string()).expect("写配置");
        path
    }

    #[test]
    fn equal_lengths_are_ready() {
        let dir = sandbox("equal");
        let resources = dir.join("bootstrap").join("resources");
        fill(&resources, "dict.tsv", 12);
        fill(&resources, "glossary-en.tsv", 34);
        assert_eq!(refresh(&options(&dir), LOCK), Refresh::Ready);
    }

    #[test]
    fn length_mismatch_reports_outdated() {
        let dir = sandbox("length");
        let resources = dir.join("bootstrap").join("resources");
        fill(&resources, "dict.tsv", 12);
        fill(&resources, "glossary-en.tsv", 33);
        let outcome = refresh(&options(&dir), LOCK);
        match &outcome {
            Refresh::Outdated(detail) => assert!(detail.starts_with("glossary-en.tsv:length-")),
            other => panic!("长度不符必须判过期，实际 {other:?}"),
        }
        assert!(outcome.error().unwrap().starts_with(OUTDATED_PREFIX));
    }

    #[test]
    fn missing_file_reports_outdated() {
        let dir = sandbox("missing");
        let resources = dir.join("bootstrap").join("resources");
        fill(&resources, "dict.tsv", 12);
        assert_eq!(
            refresh(&options(&dir), LOCK),
            Refresh::Outdated("glossary-en.tsv:missing".to_owned())
        );
    }

    /// 第四种：配置读不出**不能**报过期——上游 `outdated_resources` 只映射
    /// `Integrity` 与 `ExistingGeneration`，重新下载修不好一份坏配置。
    #[test]
    fn unreadable_options_never_reports_outdated() {
        let dir = sandbox("unreadable");
        let resources = dir.join("bootstrap").join("resources");
        fill(&resources, "dict.tsv", 12);
        fill(&resources, "glossary-en.tsv", 34);

        let absent = dir.join("no-such-options.json");
        let broken = dir.join("broken.json");
        fs::write(&broken, "not json at all").expect("写坏配置");
        let keyless = dir.join("keyless.json");
        fs::write(&keyless, r#"{"state_root":"/tmp"}"#).expect("写无 resources 的配置");

        for path in [absent, broken, keyless] {
            let outcome = refresh(&path, LOCK);
            assert!(
                !matches!(outcome, Refresh::Outdated(_)),
                "{} 读不出却判了过期",
                path.display()
            );
            let text = outcome.error().unwrap();
            assert!(!text.starts_with(OUTDATED_PREFIX), "错误文本越界：{text}");
            assert!(
                matches!(outcome, Refresh::Blocked(_)),
                "应为 Blocked：{outcome:?}"
            );
        }
    }

    /// 资源目录里的额外内容（`helpcodes/` 与日文那一组）不参与比对。
    #[test]
    fn extra_files_are_not_compared() {
        let dir = sandbox("extra");
        let resources = dir.join("bootstrap").join("resources");
        fill(&resources, "dict.tsv", 12);
        fill(&resources, "glossary-en.tsv", 34);
        fs::create_dir_all(resources.join("helpcodes")).expect("建 helpcodes");
        fill(&resources, "msime-japanese.dat", 1);
        assert_eq!(refresh(&options(&dir), LOCK), Refresh::Ready);
    }

    #[test]
    fn lock_without_bytes_is_a_build_error_not_outdated() {
        // 老格式的锁只有 name：必须 Blocked，绝不能因此每次都重解包 4.2 MB。
        let dir = sandbox("legacy-lock");
        fill(&dir.join("bootstrap").join("resources"), "dict.tsv", 12);
        let outcome = refresh(&options(&dir), r#"{"artifacts":[{"name":"dict.tsv"}]}"#);
        assert!(matches!(outcome, Refresh::Blocked(_)), "实际 {outcome:?}");
    }

    /// 机械防线：烘进二进制的锁必须与随包资源**实际字节数**一致。改了表忘了重生成锁，
    /// 这条就红——不靠人记得住。
    #[test]
    fn packaged_lock_matches_shipped_assets() {
        let artifacts = parse_lock(PACKAGED_LOCK).expect("随包锁必须能解析");
        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../host/assets/dictionary");
        assert!(!artifacts.is_empty());
        for artifact in &artifacts {
            let path = assets.join(&artifact.name);
            let actual = fs::metadata(&path)
                .unwrap_or_else(|error| panic!("{} 读不到：{error}", path.display()))
                .len();
            assert_eq!(
                actual, artifact.bytes,
                "锁里 {} 记的是 {} B，实际 {} B——改了词库要重跑 build_glossary_subset.py",
                artifact.name, artifact.bytes, actual
            );
        }
    }
}
