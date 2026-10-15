//! 装配层：从 Java Bootstrap 的请求构造 HostResources，从设备路径构造 Engine。
//!
//! 与 Java 的契约（Bootstrap.java:43-44 / :57）：
//!   Java 发  {"resources": ".../bootstrap/resources", "state_root": ".../bootstrap/state"}
//!   我们回  {"resources": <回显>, "state_root": <回显>, "preferences_directory": <state_root>}
//!   之后 Java 把 value 写进 runtime-options.json，全 App 读它。
//!
//! 关键约束：
//!   1. resources 必须回显请求原值——Bootstrap.java:376-377 会拿它做 canonical 比对，
//!      不等则 APK 升级后永不重解词库（静默失效）。
//!   2. preferences_directory 必须是 state_root——ResourcePacks.java:18 与
//!      MSIMEInputService.java:217 都把 filesDir/bootstrap/state 当成 preferences_directory
//!      读（5 处用途 + CandidateTranslationPolicy 的 resource-packs 查找）。
//!   3. state_root 只发不回读，但按对称性一并回显。
//!   4. dict_path 必须由 HostRequest.resources.join("dict.tsv") 得到，不能用仓库路径。
//!      Bootstrap.java:259-260 把 assets/dictionary/<name> 拍平复制到 resources/<name>，
//!      所以设备上是 resources/dict.tsv，没有 dictionary/ 子目录。

use std::path::PathBuf;

use qingjian_core::Engine;
use qingjian_core::{Language, NoTranslator, Translator};
use qingjian_dictionary::{Dictionary, DictionaryError};
use qingjian_learning::FrequencyLearner;
use qingjian_translate::Glossary;

/// C1 方案 C 的读音修正表（加行不删行），编进 `.so`，运行时不读盘。
/// 表本身是可 diff 的 TSV，理由与逐条出处见文件头的注释。
const READING_FIXES_TSV: &str = include_str!("reading-fixes.tsv");

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("数据文件不存在: {0}")]
    DataFileMissing(PathBuf),
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("词库加载失败: {0}")]
    Dictionary(#[from] DictionaryError),
}

/// Java Bootstrap 发来的请求（prepareHostRaw 的输入）。
#[derive(Debug, Clone)]
pub struct HostRequest {
    /// filesDir/bootstrap/resources（Java 侧 Bootstrap.java:43）
    pub resources: PathBuf,
    /// filesDir/bootstrap/state（Java 侧 Bootstrap.java:44）
    pub state_root: PathBuf,
}

/// 回给 Java 的 options（会被写进 runtime-options.json）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct HostResources {
    pub resources: String,
    pub state_root: String,
    pub preferences_directory: String,
}

/// 构造 Engine 所需的设备路径（createRaw 的输入）。
#[derive(Debug, Clone)]
pub struct DataPaths {
    /// = HostRequest.resources.join("dict.tsv")
    pub dict_path: PathBuf,
    /// = HostRequest.resources.join("glossary-en.tsv")，与 `dict_path` 同一层
    /// （`Bootstrap.java:259-260` 把 `assets/dictionary/<name>` 拍平到 `resources/<name>`）。
    pub glossary_path: PathBuf,
    /// filesDir/qingshan/user.tsv（与水杉 bootstrap/state 分目录）
    pub user_tsv: PathBuf,
}

/// 只做「校验 + 回显」，不构造 Engine，也不建目录。
///
/// 不构造 Engine 的理由：Bootstrap 先调 prepareHost 拿 options 落盘
/// （Bootstrap.java:48），之后 createRaw(options) 才建会话
/// （MSIMEInputService:1698）。在 prepare_host 里构造会被立即析构。
///
/// user_tsv 的父目录由 create_engine 按需建（FrequencyLearner 会在那里写回）。
pub fn prepare_host(request: &HostRequest) -> Result<HostResources, Error> {
    let dict_path = request.resources.join("dict.tsv");
    if !dict_path.is_file() {
        return Err(Error::DataFileMissing(dict_path));
    }
    Ok(HostResources {
        resources: request.resources.to_string_lossy().into_owned(),
        state_root: request.state_root.to_string_lossy().into_owned(),
        preferences_directory: request.state_root.to_string_lossy().into_owned(),
    })
}

/// 从设备路径构造 Engine，供 3.4.7 的 createRaw 调用。
///
/// FrequencyLearner::from_path 失败时退回 default()，抄
/// apps/linux/server/src/assembly/mod.rs:99-107 的现有做法——
/// 不拿空表覆盖用户文件。
pub fn create_engine(paths: &DataPaths) -> Result<Engine, Error> {
    if let Some(parent) = paths.user_tsv.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // 词库载入是真机上唯一想知道的量（宿主侧测过 50-61 ms，设备未测），所以就地打一条。
    let started = std::time::Instant::now();
    let bytes = std::fs::metadata(&paths.dict_path).map_or(0, |m| m.len());
    let dictionary = Dictionary::from_path(&paths.dict_path)?;
    tracing::info!(
        "词库载入 {} ms / {} B",
        started.elapsed().as_millis(),
        bytes
    );
    let learner = FrequencyLearner::from_path(&paths.user_tsv).unwrap_or_else(|e| {
        tracing::warn!("FrequencyLearner 构造失败，退回内存学习: {}", e);
        FrequencyLearner::default()
    });
    // 译词表缺失/坏行只降级成「没有释义」，绝不升级成 createRaw 的 Err：
    // `assets/glossary/README.md:6-7` 写明「任一行缺词或缺释义，整张表加载失败」，
    // 而 Err 会让 Java 走 `fail()`，那是整个会话停掉，不是少一行英文。
    let glossary_started = std::time::Instant::now();
    let glossary_bytes = std::fs::metadata(&paths.glossary_path).map_or(0, |m| m.len());
    let translator: Box<dyn Translator> =
        match Glossary::from_path(Language::English, &paths.glossary_path) {
            Ok(glossary) => Box::new(glossary),
            Err(error) => {
                tracing::warn!("Glossary 加载失败，退回 NoTranslator: {error}");
                Box::new(NoTranslator)
            }
        };
    tracing::info!(
        "译词表载入 {} ms / {} B",
        glossary_started.elapsed().as_millis(),
        glossary_bytes
    );
    let dictionary_total = dictionary.total_frequency();
    let mut engine = Engine::new(dictionary)
        .with_learner(Box::new(learner))
        .with_translator(translator);
    // setter 不是 builder（`engine/setup.rs:436` 返回 ()），只能单独一句。
    // 开着时 Shift 字母按小写参与匹配、原样上屏还原大写（`engine/composing.rs:181-189`），
    // 与 char_for 把 shift 补成大写的做法配套。
    engine.set_shift_letter_compose(true);
    // C1 方案 C：把读音修正表当**附加词库**挂上（`engine/setup.rs:495`），查词时与主词库取并集
    // （同文件 `:505 all_dictionaries`）。只加行、随包 `dict.tsv` 一字不改，所以
    // 错读音那条路照旧能打通（见 `the_wrong_reading_row_still_works`）。
    // 这里用 `?` 而不是像 Glossary 那样降级：表是 include_str! 烘进二进制的常量，
    // 解析失败只可能是有人改坏了这张表，属于"编译期就该红"的错误；上游水杉的
    // 修正清单同样是"对不上就失败"（resources/dictionary-sources/pinyin-reading-corrections.txt
    // 表头写明）。`reading_fixes_match_the_shipped_dictionary` 这条单测先把住同一道关。
    let fixes = Dictionary::parse(READING_FIXES_TSV)?;
    tracing::info!(
        "读音修正 {} 条 / 词频合计 {}，主词库词频 {}（修正表占 {:.4}%）",
        fixes.len(),
        fixes.total_frequency(),
        dictionary_total,
        fixes.total_frequency() as f64 * 100.0 / dictionary_total.max(1) as f64
    );
    engine.set_extra_dictionaries(vec![fixes]);
    Ok(engine)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真词库 + 真译词表（就是打进 APK 的那两份），学习目录按测试名隔离并先清掉。
    /// 与 `session.rs` 里的 `real_paths()` 同料不同目录，避免两组用例互相写。
    fn real_paths(tag: &str) -> DataPaths {
        let dir = std::env::temp_dir().join("qingshan-c1fix-tests").join(tag);
        let _ = std::fs::remove_dir_all(&dir);
        DataPaths {
            dict_path: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../host/assets/dictionary/dict.tsv"),
            glossary_path: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../host/assets/dictionary/glossary-en.tsv"),
            user_tsv: dir.join("user.tsv"),
        }
    }

    /// 敲一整串拼音，回候选表里的文本。
    fn typed(engine: &mut qingjian_core::Engine, pinyin: &str) -> Vec<String> {
        for c in pinyin.chars() {
            engine.push(c);
        }
        engine
            .query()
            .map(|q| q.candidates.items.iter().map(|c| c.text.clone()).collect())
            .unwrap_or_default()
    }

    /// 「长时间」在随包词库里只有生成器拼出来的 `zhang shi jian`（`dict.tsv:86482`），但
    /// 敲 `changshijian` **今天就能拿到它**（实测第 0 格，走的是单字 `长 chang` + `时间` 的合词路）。
    /// 所以这条不是本轮的驱动用例，是和下面两条一起立的**回归守护**：加修正行之后它还得是第 0 格。
    #[test]
    fn typing_changshijian_reaches_chang_shi_jian() {
        let mut engine = create_engine(&real_paths("changshijian")).expect("真词库应能装配");
        let texts = typed(&mut engine, "changshijian");
        assert_eq!(
            texts.first().map(String::as_str),
            Some("长时间"),
            "敲 changshijian 的第 0 格应当还是 长时间，前 12 格 = {:?}",
            &texts[..texts.len().min(12)]
        );
    }

    /// 随包词库里「太长」只有生成器拼出来的 `tai zhang`（`dict.tsv:25840`），所以按词义敲
    /// `taichang` 时引擎**整表都不产出「太长」**（实测 380 格里没有它，首格是「提倡」）。
    /// 修好之后这一串必须能拿到它。
    #[test]
    fn typing_taichang_reaches_tai_chang() {
        let mut engine = create_engine(&real_paths("taichang")).expect("真词库应能装配");
        let texts = typed(&mut engine, "taichang");
        assert!(
            texts.iter().any(|t| t == "太长"),
            "敲 taichang 的 {} 格里没有 太长，前 12 格 = {:?}",
            texts.len(),
            &texts[..texts.len().min(12)]
        );
    }

    /// 同上：`henchang` 现在整表没有「很长」（实测 442 格里没有，首格「合唱」——`hen chang`
    /// 本身就是「合唱」的读音，压得更死）。
    #[test]
    fn typing_henchang_reaches_hen_chang() {
        let mut engine = create_engine(&real_paths("henchang")).expect("真词库应能装配");
        let texts = typed(&mut engine, "henchang");
        assert!(
            texts.iter().any(|t| t == "很长"),
            "敲 henchang 的 {} 格里没有 很长，前 12 格 = {:?}",
            texts.len(),
            &texts[..texts.len().min(12)]
        );
    }

    /// 加行不删行：词典里原有的 `zhang shi jian` 那条必须照旧能用（上面两条测的是"新增通路
    /// 打开"，这条守的是"旧通路没被动过"）。
    #[test]
    fn the_wrong_reading_row_still_works() {
        let mut engine = create_engine(&real_paths("zhangshijian")).expect("真词库应能装配");
        let texts = typed(&mut engine, "zhangshijian");
        assert!(
            texts.iter().any(|t| t == "长时间"),
            "敲 zhangshijian 不该拿不到 长时间，前 12 格 = {:?}",
            &texts[..texts.len().min(12)]
        );
    }

    /// 修正表的可机械校验部分（防我自己抄错）：
    /// ① 表能过 `Dictionary::parse`，行数 = 12；
    /// ② 每个词头在随包 `dict.tsv` 里**本来就有一行**（我们只补读音，不发明词条）；
    /// ③ 补的这条读音里，每个音节都必须是对应汉字在 `dict.tsv` 里的单字读音之一；
    /// ④ 词频沿用 `dict.tsv` 里那个词头的频次，不许新造数字。
    /// 这四条同时断言"表是干净的"，任何一条不满足就是我把表改坏了。
    #[test]
    fn reading_fixes_match_the_shipped_dictionary() {
        let fixes =
            Dictionary::parse(READING_FIXES_TSV).expect("读音修正表必须能过 Dictionary::parse");
        assert_eq!(
            fixes.len(),
            12,
            "修正表条目数变了，README/PIN 的计数要跟着改"
        );

        let dict_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../host/assets/dictionary/dict.tsv");
        let text = std::fs::read_to_string(&dict_path).expect("读随包词库");
        let mut head_freq: std::collections::HashMap<String, Vec<u32>> =
            std::collections::HashMap::new();
        let mut char_syllables: std::collections::HashMap<
            String,
            std::collections::HashSet<String>,
        > = std::collections::HashMap::new();
        for line in text.lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut f = line.split('\t');
            let (Some(word), Some(pinyin), Some(freq)) = (f.next(), f.next(), f.next()) else {
                continue;
            };
            let freq: u32 = freq.trim().parse().unwrap_or(0);
            head_freq.entry(word.to_string()).or_default().push(freq);
            let syllables: Vec<&str> = pinyin.split_whitespace().collect();
            if word.chars().count() == 1 && syllables.len() == 1 {
                char_syllables
                    .entry(word.to_string())
                    .or_default()
                    .insert(syllables[0].to_string());
            }
        }

        let rows: Vec<(String, String, u32)> = READING_FIXES_TSV
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|l| {
                let mut f = l.split('\t');
                (
                    f.next().unwrap().to_string(),
                    f.next().unwrap().to_string(),
                    f.next().unwrap().trim().parse::<u32>().unwrap(),
                )
            })
            .collect();
        assert_eq!(rows.len(), 12);
        for (word, pinyin, freq) in rows {
            let syllables: Vec<&str> = pinyin.split_whitespace().collect();
            let chars: Vec<char> = word.chars().collect();
            assert_eq!(
                syllables.len(),
                chars.len(),
                "{word} 的音节数与字数不等，没法逐字核对读音"
            );
            for (ch, syl) in chars.iter().zip(syllables.iter()) {
                let key = ch.to_string();
                let allowed = char_syllables
                    .get(&key)
                    .unwrap_or_else(|| panic!("{word}：单字 {key} 在 dict.tsv 里没有读音行"));
                assert!(
                    allowed.contains(*syl),
                    "{word}：{key} 补的读音 {syl} 不在 dict.tsv 的读音集 {:?} 里（多半是我抄错）",
                    allowed
                );
            }
            let freqs = head_freq.get(&word).unwrap_or_else(|| {
                panic!("{word}：dict.tsv 里根本没有这个词头，修正表不该凭空加词条")
            });
            assert!(
                freqs.contains(&freq),
                "{word}：修正表用的词频 {freq} 与 dict.tsv 里的 {freqs:?} 不一致"
            );
        }
    }

    /// 附加词库会把 `total_frequency` 的分母抬高（12 条合计 94,886，主词库 275,814,283，
    /// 设备日志实测占 0.0344%），而基础分值是 `ln(f+1) - ln(total) - 4`（`sentence/mod.rs:76-84`）——
    /// 这是一道**对所有候选同减**的平移，理论上不动相对次序。这里用真词库实测守住它：
    /// 挂与不挂修正表，几串与修正表无关的输入，首页次序必须逐格相同。
    #[test]
    fn the_extra_dictionary_does_not_reshuffle_unrelated_inputs() {
        let paths = real_paths("no-reshuffle");
        for py in ["shuijiao", "zhaojia", "nihao", "women", "bushicongming"] {
            // Engine 不是 Clone，所以每串都新建两台：一台走 create_engine（挂了修正表），
            // 一台只用主词库（没挂）。
            let mut with_fixes = create_engine(&paths).expect("真词库应能装配");
            let dictionary = Dictionary::from_path(&paths.dict_path).expect("真词库应能解析");
            let mut without = qingjian_core::Engine::new(dictionary);
            let head_a = typed(&mut with_fixes, py);
            let head_b = typed(&mut without, py);
            assert_eq!(
                head_a[..head_a.len().min(10)],
                head_b[..head_b.len().min(10)],
                "敲 {py} 时挂上修正表改变了首页前 10 格（上面是带修正表的，下面是没挂的）"
            );
        }
    }

    /// 随包的 `glossary-en.tsv` 必须过 `Glossary::parse`。这条不测 `create_engine`，只测那份
    /// 数据本身：解析失败在真机上只表现为「没有译词」（`create_engine` 退回
    /// `NoTranslator` 并 `warn!`，而宿主测试没有 subscriber，那行日志看不见），
    /// 所以只能在这里断言。文案里不写行数/字节数——那张表会随 A1/A3 继续长，
    /// 写死的数字一定再烂一次。
    #[test]
    fn shipped_glossary_subset_parses() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../host/assets/dictionary/glossary-en.tsv");
        qingjian_translate::Glossary::from_path(qingjian_core::Language::English, &path)
            .expect("随包 glossary-en.tsv 必须能过 Glossary::parse，否则线上静默退回 NoTranslator");
    }
}
