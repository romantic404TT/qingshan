//! 跨 JNI 边界返回给水杉 Java 侧的三份 JSON 契约。
//!
//! 逐字段镜像**水杉**的 `crates/input-runtime/src/types.rs:20-156`（注意：`input-runtime`
//! 是水杉的 crate，青简没有这个 crate）。字段名、类型、枚举序列化形式都以那份源码为准，
//! 因为 Java 侧 `MSIMEInputService.apply():2389` 是严格解析：
//! `value.view` 缺失、`view.editing_text` 用 `getString` 硬取、`handled` 必须是真
//! JSON boolean（`strictBoolean():851`），任一条不满足就 `JSONException` → `fail():3116`
//! → 退化成「仅直接输入」。
//!
//! 与上游的两处**有意差异**（都已核实等价）：
//! - 上游 `View.phrase_prefix` 带 `#[serde(skip_serializing_if = "String::is_empty")]`
//!   （types.rs:117），这里去掉 skip、永远输出。Java 只在
//!   `InputViewValuePolicy.textOr(view,"phrase_prefix","")` 处读它（`MSIMEInputService:2407`
//!   与 `:7747`），而 `textOr` 走 `JsonPolicy.strictString(raw, fallback)`，注释明写
//!   「不把 number / boolean / JSON null 转成文本」→ 缺失与 `""` 等价。
//! - 其余未逐个核实读法的字段一律用**非 Option 类型 + 零值**输出，绝不输出 `null`。
//!   理由：万一某字段 Java 侧其实是硬取，输出 `null` 会抛异常导致降级；输出零值最坏只是
//!   显示为空。**降级优于崩溃**，这是路线 B 的一贯取向。
//!   唯一的 `Option` 是 `Candidate.translation`（与上游一致，带 skip），因为 Java 侧
//!   `ImeCandidates.java:120` 显式 `isNull("translation")` 判断，null 是语义正确的值。

use serde::Serialize;

/// types.rs:71-75。整个 View 里**唯一**不带 `rename_all` 的枚举，所以值是 PascalCase
/// （`"Fullwidth"` / `"Halfwidth"`），其余字段一律 snake_case，抄的时候最容易错在这里。
///
/// 戊-C 之后两个变体都会构造：`setCharacterWidthRaw` 记进会话、`build_view` 按它报真值，
/// 因为宿主拿这份回声**反向覆盖**它自己的 `fullWidthInput`——恒半角的回声等于把全角打回半角。
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum CharacterWidth {
    Fullwidth,
    Halfwidth,
}

/// `client-core/src/preferences.rs:46-55`（input-runtime 经 `pub use msime_client_core::…` 引入）。
/// Java 侧 `apply()` 用 `displayedTouchLayout(view)` 的前后差异决定是否重建键位行
/// （`MSIMEInputService:2391`、`:2424`），给错值会让键盘该重绘时不重绘。
///
/// 同理：阶段一只给 `TwentySixKey`，另外两个拼写是格式的一部分，不是待实现的分支。
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TouchKeyboardLayout {
    TwentySixKey,
    NineKey,
    Handwriting,
}

/// types.rs:6-11。选词回传身份三元组，**是嵌套对象**，不是平铺在 `Candidate` 上：
/// Java 从 `candidates[i].id.{session,generation,index}` 取值再原样送回
/// `selectRaw(session, generation, index)`（`MSIMEInputService:6254-6264`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct CandidateId {
    pub session: u64,
    pub generation: u64,
    pub index: usize,
}

/// types.rs:20-39，9 字段。
#[derive(Clone, Debug, Serialize)]
pub struct Candidate {
    pub id: CandidateId,
    pub text: String,
    /// 产生该候选的引擎输入码，与 `text` 对齐；只用于展示，不参与选择与上屏。
    pub code: String,
    /// 引擎派生的显示后缀，永不进入选择或上屏文本。
    pub annotation: String,
    pub source: u8,
    pub corrected: bool,
    pub fixed_position: u8,
    pub highlighted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
}

/// types.rs:138-146，4 字段。上屏时随 `commit` 一起回，告诉宿主这次要不要做繁简转换、
/// 要不要计入打字统计。
#[derive(Debug, Serialize)]
pub struct OutputContext {
    pub scheme: u8,
    pub local_mode: String,
    pub script_conversion: bool,
    pub typing_statistics: bool,
}

/// types.rs:77-136，32 字段。容器上没有任何 `#[serde(...)]` 属性，key 即字段名。
#[derive(Clone, Debug, Serialize)]
pub struct View {
    pub scheme: u8,
    pub chinese_text: bool,
    pub script_conversion: bool,
    pub nine_key: bool,
    pub nine_key_spellings: Vec<String>,
    pub nine_key_reading: String,
    pub nine_key_single_character: bool,
    pub nine_key_strokes: String,
    pub touch_keyboard_layout: TouchKeyboardLayout,
    pub character_width: CharacterWidth,
    pub microsoft_shuangpin: bool,
    pub shuangpin_profile: String,
    pub answered_by_pinyin_fallback: bool,
    pub local_mode: String,
    pub spelling_symbols: String,
    pub dedicated_english: bool,
    /// `strictCandidateLong(view, "session")` 硬取，必须是非 null 整数。
    pub session: u64,
    pub generation: u64,
    pub focused: bool,
    pub preedit: String,
    pub phrase_prefix: String,
    pub reading: String,
    /// `next.getString("editing_text")` 硬取，绝不能是 null。
    pub editing_text: String,
    /// `editing_text` 内的字节偏移（上游注释：ASCII，不是 OS 的 UTF-16 偏移）。
    pub caret_position: usize,
    pub conversion: String,
    pub conversion_focus_start: usize,
    pub conversion_focus_end: usize,
    pub page: usize,
    /// 每页候选数。取上游同一个常量 `MAX_CANDIDATE_PAGE_SIZE = 10`
    /// （`client-core/src/preferences.rs:3373`；数字键 1–9 选前九个、0 选第十个），
    /// 这样 Java 侧对键位与翻页按钮状态的隐含假设不被打破。
    pub page_size: usize,
    pub page_count: usize,
    pub candidate_list_open: bool,
    pub candidates: Vec<Candidate>,
}

/// types.rs:148-156，5 字段。`character/command/focus/select/destroy` 的 `value` 就是它。
/// 三个 `Option` 上游**没有** skip，序列化出来就是 `null`，这里保持一致。
#[derive(Debug, Serialize)]
pub struct Transition {
    pub handled: bool,
    pub commit: Option<String>,
    pub commit_context: Option<OutputContext>,
    pub diagnostic: Option<String>,
    pub view: View,
}

/// 各结构的字段数，来自对 `types.rs:78-136` 的程序化统计（数 struct 体内的 `pub` 声明）。
/// 只给下面的断言用；cdylib 里 `pub` 不构成导出，留在全局会被 dead_code 报。
#[cfg(test)]
pub const VIEW_FIELD_COUNT: usize = 32;
#[cfg(test)]
pub const CANDIDATE_FIELD_COUNT: usize = 9;
#[cfg(test)]
pub const TRANSITION_FIELD_COUNT: usize = 5;

/// 阶段一只做全拼：`SchemeType::Quanpin = 0`
/// （`msime-develop/crates/engine/src/types.rs:63-65`）。Java 侧按编号分支
/// （`keyboard/ShuangpinKeyHintPolicy.java:18` 用 `scheme == 1`，
/// `policy/ChineseHelpcodePolicy.java:10` 用 `scheme == 0 || scheme == 1`）。
pub const SCHEME_QUANPIN: u8 = 0;

/// 装配一次 `View` 所需的会话侧状态。
pub struct Snapshot<'a> {
    pub id: u64,
    pub generation: u64,
    pub focused: bool,
    pub english: bool,
    pub editing: String,
    pub caret: usize,
    pub page: usize,
    pub page_size: usize,
    pub page_count: usize,
    pub highlighted: usize,
    /// 会话记的半角/全角，原样进 `View.character_width`（戊-C）。
    pub fullwidth: bool,
    pub candidates: &'a [qingjian_core::Candidate],
}

/// 由青简 Core 的候选与游标拼出宿主 `View`。
///
/// 32 个字段里只有 Core 真给得出的那些有值；九键 / 双拼 / 注音 / 笔画相关的 12 个字段
/// 按「降级 > 崩溃」给零值——Java 侧读它们全部走 `textOr` / `booleanValue` 容错
/// （`MSIMEInputService.java:2401-2412`），不会因为零值抛异常。
/// `page_size` 由调用方从 `Session` 取，不在这里引常量。
pub fn build_view(source: &Snapshot) -> View {
    View {
        scheme: SCHEME_QUANPIN,
        chinese_text: !source.english,
        script_conversion: !source.english,
        nine_key: false,
        nine_key_spellings: Vec::new(),
        nine_key_reading: String::new(),
        nine_key_single_character: false,
        nine_key_strokes: String::new(),
        touch_keyboard_layout: TouchKeyboardLayout::TwentySixKey,
        // 戊-C：报会话记的真值，不再恒半角。宿主 `MSIMEInputService.java:4451-4453` 会
        // 把回声里的宽度**反向覆盖**它自己的 `fullWidthInput`，所以回声必须是诚实的——
        // 恒半角的回声等于把用户的全角设置打回半角（那就是"naive 回声"，上一轮已判定不做）。
        character_width: if source.fullwidth {
            CharacterWidth::Fullwidth
        } else {
            CharacterWidth::Halfwidth
        },
        microsoft_shuangpin: false,
        shuangpin_profile: String::new(),
        answered_by_pinyin_fallback: false,
        local_mode: "none".into(),
        spelling_symbols: String::new(),
        dedicated_english: source.english,
        session: source.id,
        generation: source.generation,
        focused: source.focused,
        preedit: source.editing.clone(),
        phrase_prefix: String::new(),
        reading: String::new(),
        editing_text: source.editing.clone(),
        caret_position: source.caret,
        conversion: String::new(),
        conversion_focus_start: 0,
        conversion_focus_end: 0,
        page: source.page,
        page_size: source.page_size,
        page_count: source.page_count,
        candidate_list_open: !source.candidates.is_empty(),
        candidates: page_candidates(source),
    }
}

/// 展开候选面板（`allCandidatesRaw`）的应答体。**不是 View**：宿主这条路径走 `value()`
/// 而不走 `apply()`，从不 `getJSONObject("view")`（`MSIMEInputService.java:6334`），
/// 只读这五个键——`session` 与 `generation` 给 `sameCandidateVersion`（`:6015-6024`）
/// 跟它手上那份 View 比，`candidates` 是全表（`:6340` 要求比当页多），
/// `reading`/`preedit` 是面板顶部的组词文案（`ImeCandidates.java:280-282`）。
#[derive(Serialize)]
pub struct AllCandidates {
    pub session: u64,
    pub generation: u64,
    pub candidates: Vec<Candidate>,
    pub reading: String,
    pub preedit: String,
}

/// 由同一份快照拼展开面板的应答。`reading` 与 View 里一样留空（全拼不产读音行），
/// 宿主会退到 `preedit`。
pub fn all_candidates_view(source: &Snapshot) -> AllCandidates {
    AllCandidates {
        session: source.id,
        generation: source.generation,
        candidates: all_candidates(source),
        reading: String::new(),
        preedit: source.editing.clone(),
    }
}

/// 全表候选（不切页）。`id.index` 同样是整表下标——宿主的 `selectAnyCandidate` 就按它选，
/// 边界口径见 `runtime.rs:1788-1794`。
pub(crate) fn all_candidates(source: &Snapshot) -> Vec<Candidate> {
    source
        .candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| candidate_row(source, index, candidate))
        .collect()
}

/// 把 Core 的译词压成 View 里那一行字符串。口径抄上游两个壳
/// （`apps/macos/src/candidates/row.rs:46-56`、`apps/windows/server/src/ui/candidates/row.rs:19-29`）：
/// 释义按序取（`Translation::MAX_SENSES = 2` 已在构造时截过），词性用 `Display` 前缀成
/// `"v. "`，两条之间用 `" · "` 连。
///
/// 有意偏离上游两处：上游 `" · "` 的条件是 `i > 0 || !annotation.is_empty()`，因为前面可能
/// 压了 `candidate.reading`——我们没有读音前缀，所以只看 `i > 0`；上游取文本走
/// `Sense::furigana()` 并给 `fresh`/词性分 `Tone`，而 View 的 `translation` 是一个 `String`、
/// 没有着色通道，所以直接取 `sense.text`，生词标记传不出去。
///
/// 返回值必须单行：宿主按 `\n` 数行撑候选条高度
/// （`host/java/app/msime/android/candidate/CandidateTranslationPolicy.java:133-139`）。
///
/// `&Option<Translation>` 会被 clippy 的 `ref_option` 点。项目当前不跑 clippy，
/// 保持这个签名与调用方（`candidate_row` 里 `&candidate.translation`）一致。
/// 若将来接 clippy，与其它 `&Option<T>` 一起改 `Option<&T>`。
pub fn translation_to_string(translation: &Option<qingjian_core::Translation>) -> Option<String> {
    let senses = translation.as_ref()?.senses();
    if senses.is_empty() {
        return None;
    }
    let mut out = String::new();
    for (index, sense) in senses.iter().enumerate() {
        if index > 0 {
            out.push_str(" · ");
        }
        match sense.part_of_speech {
            Some(part_of_speech) => {
                out.push_str(&format!("{part_of_speech} {}", sense.text));
            }
            None => out.push_str(&sense.text),
        }
    }
    Some(out)
}

/// 单行候选。`page_candidates` 与 `all_candidates` 共用它，两条路径的形状才不会分叉。
pub(crate) fn candidate_row(
    source: &Snapshot,
    index: usize,
    candidate: &qingjian_core::Candidate,
) -> Candidate {
    Candidate {
        id: CandidateId {
            session: source.id,
            generation: source.generation,
            index,
        },
        text: candidate.text.clone(),
        code: candidate.syllables.join(" "),
        annotation: String::new(),
        source: 0,
        corrected: false,
        fixed_position: 0,
        highlighted: index == source.highlighted,
        translation: translation_to_string(&candidate.translation),
    }
}

/// 当页候选。镜像上游 `runtime.rs:700-717`：`start = page * page_size`，只取
/// `[start, start + page_size)`，而 `id.index` 用整表下标（上游 `.enumerate().skip(start)`）。
/// 发整表的话宿主第一页会把 44 个候选全画出来，`page` / `page_count` 就对不上了。
pub(crate) fn page_candidates(source: &Snapshot) -> Vec<Candidate> {
    let start = (source.page * source.page_size).min(source.candidates.len());
    let end = (start + source.page_size).min(source.candidates.len());
    source.candidates[start..end]
        .iter()
        .enumerate()
        .map(|(offset, candidate)| candidate_row(source, start + offset, candidate))
        .collect()
}

/// 上屏时随 `commit` 一起回的上下文。四个字段与 `types.rs:138-146` 同序；
/// 阶段一恒为全拼、非本地模式、要做繁简转换、计入统计。
pub fn output_context() -> OutputContext {
    OutputContext {
        scheme: SCHEME_QUANPIN,
        local_mode: "none".into(),
        script_conversion: true,
        typing_statistics: true,
    }
}

/// 给别的模块的单测用（`lib.rs` 要拿它断言应答形状）。填字段的意义在下面那份注释里。
#[cfg(test)]
pub(crate) use tests::mock_view;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// 手工填满 32 个字段：这样以后结构增删字段会直接编译失败，不会悄悄漏。
    pub(crate) fn mock_view() -> View {
        View {
            scheme: 1,
            chinese_text: true,
            script_conversion: true,
            nine_key: false,
            nine_key_spellings: vec![],
            nine_key_reading: String::new(),
            nine_key_single_character: false,
            nine_key_strokes: String::new(),
            touch_keyboard_layout: TouchKeyboardLayout::TwentySixKey,
            character_width: CharacterWidth::Halfwidth,
            microsoft_shuangpin: false,
            shuangpin_profile: String::new(),
            answered_by_pinyin_fallback: false,
            local_mode: "none".into(),
            spelling_symbols: String::new(),
            dedicated_english: false,
            session: 7,
            generation: 5,
            focused: true,
            preedit: "ni hao".into(),
            phrase_prefix: String::new(),
            reading: String::new(),
            editing_text: "nihao".into(),
            caret_position: 5,
            conversion: String::new(),
            conversion_focus_start: 0,
            conversion_focus_end: 0,
            page: 0,
            page_size: 10,
            page_count: 1,
            candidate_list_open: false,
            candidates: vec![
                Candidate {
                    id: CandidateId {
                        session: 7,
                        generation: 5,
                        index: 0,
                    },
                    text: "你好".into(),
                    code: "nihao".into(),
                    annotation: String::new(),
                    source: 0,
                    corrected: false,
                    fixed_position: 0,
                    highlighted: true,
                    translation: None,
                },
                Candidate {
                    id: CandidateId {
                        session: 7,
                        generation: 5,
                        index: 1,
                    },
                    text: "拟好".into(),
                    code: "nihao".into(),
                    annotation: String::new(),
                    source: 0,
                    corrected: false,
                    fixed_position: 0,
                    highlighted: false,
                    translation: Some("hello".into()),
                },
            ],
        }
    }

    fn keys(value: &Value) -> Vec<String> {
        value
            .as_object()
            .expect("不是 JSON 对象")
            .keys()
            .cloned()
            .collect()
    }

    /// 断言 JSON 的 key 集合与给定名单完全一致（双向差集为空）。
    fn assert_exact_keys(value: &Value, expected: &[&str], label: &str) {
        let got = keys(value);
        assert_eq!(got.len(), expected.len(), "{label} key 数不符：{got:?}");
        for k in expected {
            assert!(got.iter().any(|g| g == k), "{label} 缺字段 {k}");
        }
        for g in &got {
            assert!(expected.contains(&g.as_str()), "{label} 多出字段 {g}");
        }
    }

    #[test]
    fn view_emits_exactly_32_keys() {
        let view = serde_json::to_value(mock_view()).unwrap();
        assert_exact_keys(
            &view,
            &[
                "scheme",
                "chinese_text",
                "script_conversion",
                "nine_key",
                "nine_key_spellings",
                "nine_key_reading",
                "nine_key_single_character",
                "nine_key_strokes",
                "touch_keyboard_layout",
                "character_width",
                "microsoft_shuangpin",
                "shuangpin_profile",
                "answered_by_pinyin_fallback",
                "local_mode",
                "spelling_symbols",
                "dedicated_english",
                "session",
                "generation",
                "focused",
                "preedit",
                "phrase_prefix",
                "reading",
                "editing_text",
                "caret_position",
                "conversion",
                "conversion_focus_start",
                "conversion_focus_end",
                "page",
                "page_size",
                "page_count",
                "candidate_list_open",
                "candidates",
            ],
            "View",
        );
        assert_eq!(keys(&view).len(), VIEW_FIELD_COUNT);
        // phrase_prefix 空串也必须出现（我们去掉了上游的 skip）
        assert_eq!(view["phrase_prefix"], Value::String(String::new()));
    }

    #[test]
    fn candidate_and_transition_key_sets() {
        let candidate = serde_json::to_value(&mock_view().candidates[0]).unwrap();
        let candidate_keys = [
            "id",
            "text",
            "code",
            "annotation",
            "source",
            "corrected",
            "fixed_position",
            "highlighted",
        ];
        assert_exact_keys(&candidate, &candidate_keys, "Candidate");
        // 结构体 9 个字段，translation=None 被 skip 后线上只剩 8 个 key。
        assert_eq!(candidate_keys.len() + 1, CANDIDATE_FIELD_COUNT);
        assert!(
            !candidate.as_object().unwrap().contains_key("translation"),
            "translation 为 None 时应被 skip（与上游一致）"
        );
        assert!(
            serde_json::to_value(&mock_view().candidates[1])
                .unwrap()
                .as_object()
                .unwrap()
                .contains_key("translation")
        );
        assert_eq!(candidate["id"].as_object().unwrap().len(), 3);
        assert_exact_keys(
            &candidate["id"],
            &["session", "generation", "index"],
            "CandidateId",
        );

        let transition = serde_json::to_value(Transition {
            handled: true,
            commit: None,
            commit_context: None,
            diagnostic: None,
            view: mock_view(),
        })
        .unwrap();
        let transition_keys = ["handled", "commit", "commit_context", "diagnostic", "view"];
        assert_exact_keys(&transition, &transition_keys, "Transition");
        assert_eq!(transition_keys.len(), TRANSITION_FIELD_COUNT);
        // 三个 Option 不 skip：必须是显式 null，而不是缺 key
        for k in ["commit", "commit_context", "diagnostic"] {
            assert!(
                transition.as_object().unwrap().contains_key(k),
                "Transition 缺 {k}"
            );
            assert!(transition[k].is_null(), "Transition.{k} 应为 null");
        }
    }

    #[test]
    fn enum_serialization_spellings() {
        assert_eq!(
            serde_json::to_value(CharacterWidth::Fullwidth).unwrap(),
            "Fullwidth"
        );
        assert_eq!(
            serde_json::to_value(CharacterWidth::Halfwidth).unwrap(),
            "Halfwidth"
        );
        assert_eq!(
            serde_json::to_value(TouchKeyboardLayout::TwentySixKey).unwrap(),
            "twenty_six_key"
        );
        assert_eq!(
            serde_json::to_value(TouchKeyboardLayout::NineKey).unwrap(),
            "nine_key"
        );
        assert_eq!(
            serde_json::to_value(TouchKeyboardLayout::Handwriting).unwrap(),
            "handwriting"
        );
    }

    /// 不参与断言，只在看不到形状时手动跑：cargo test -- --nocapture view_shape_sample
    #[test]
    fn view_shape_sample() {
        println!("{}", serde_json::to_string_pretty(&mock_view()).unwrap());
    }

    /// 造 Core 侧候选：`Candidate` 没有 `Default`，6 个字段逐个写。
    fn core_fake(text: &str) -> qingjian_core::Candidate {
        qingjian_core::Candidate {
            text: text.to_string(),
            kind: qingjian_core::CandidateKind::Chinese,
            syllables: Vec::new(),
            reading: None,
            translation: None,
            aux_code: None,
        }
    }

    /// 戊-C：`View.character_width` 必须跟着会话记的宽度走，不能再恒半角。
    /// 宿主拿到回声后会用它**反向覆盖**自己的 `fullWidthInput`
    /// （`qingshan/host/java/app/msime/android/core/MSIMEInputService.java:4451-4453`），
    /// 所以恒半角的回声等于每次建会话都把用户的全角打回半角。
    #[test]
    fn character_width_follows_the_session_flag() {
        fn snapshot<'a>(fullwidth: bool, all: &'a [qingjian_core::Candidate]) -> Snapshot<'a> {
            Snapshot {
                id: 7,
                generation: 5,
                focused: true,
                english: false,
                editing: String::new(),
                caret: 0,
                page: 0,
                page_size: 10,
                page_count: 1,
                highlighted: 0,
                fullwidth,
                candidates: all,
            }
        }
        let none: Vec<qingjian_core::Candidate> = Vec::new();
        assert_eq!(
            serde_json::to_value(build_view(&snapshot(true, &none)).character_width).unwrap(),
            "Fullwidth",
            "会话记了全角就要报全角；报成半角会覆盖掉用户的设置"
        );
        assert_eq!(
            serde_json::to_value(build_view(&snapshot(false, &none)).character_width).unwrap(),
            "Halfwidth",
            "没要求全角时保持半角，与戊-C 之前逐字相同"
        );
    }

    #[test]
    fn only_the_current_page_reaches_the_host() {
        let all: Vec<qingjian_core::Candidate> =
            (0..25).map(|i| core_fake(&format!("c{i}"))).collect();

        let second = build_view(&Snapshot {
            id: 7,
            generation: 5,
            focused: true,
            english: false,
            editing: String::new(),
            caret: 0,
            page: 1,
            page_size: 10,
            page_count: 3,
            highlighted: 12,
            fullwidth: false,
            candidates: &all,
        });
        let index: Vec<usize> = second.candidates.iter().map(|c| c.id.index).collect();
        let want: Vec<usize> = (10..20).collect();
        assert_eq!(index, want, "第二页发整表下标 10..20");
        assert_eq!(second.candidates.len(), second.page_size);
        let lit = second.candidates.iter().filter(|c| c.highlighted).count();
        assert_eq!(lit, 1, "整页只有一个高亮");
        assert!(second.candidates[2].highlighted, "高亮 = 整表下标 12");

        let third = build_view(&Snapshot {
            id: 7,
            generation: 5,
            focused: true,
            english: false,
            editing: String::new(),
            caret: 0,
            page: 2,
            page_size: 10,
            page_count: 3,
            highlighted: 20,
            fullwidth: false,
            candidates: &all,
        });
        assert_eq!(third.candidates.len(), 5, "末页只有 25-20=5 个");
        assert_eq!(third.candidates[0].id.index, 20);
    }

    /// `translation_to_string` 的五条口径。用手搭的 `Translation`，不碰真译词表——
    /// 表本身能不能解析由 `assemble::tests::shipped_glossary_subset_parses` 守。
    #[test]
    fn translation_to_string_rules() {
        use qingjian_core::{Language, PartOfSpeech, Sense, Translation};
        let sense = |part_of_speech: Option<PartOfSpeech>, text: &str| Sense {
            part_of_speech,
            text: text.to_owned(),
            reading: None,
            fresh: false,
        };
        let packed = |senses: Vec<Sense>| Some(Translation::new(Language::English, senses));

        assert_eq!(translation_to_string(&None), None, "没有译词就是 None");
        assert_eq!(
            translation_to_string(&packed(vec![])),
            None,
            "空 senses 不吐空字符串"
        );
        assert_eq!(
            translation_to_string(&packed(vec![sense(None, "hello")])),
            Some("hello".to_owned()),
            "无词性就不前置"
        );
        assert_eq!(
            translation_to_string(&packed(vec![sense(Some(PartOfSpeech::Verb), "hello")])),
            Some("v. hello".to_owned()),
            "词性走 Display，后面带一个空格"
        );
        let two = translation_to_string(&packed(vec![
            sense(Some(PartOfSpeech::Verb), "hello"),
            sense(Some(PartOfSpeech::Noun), "greeting"),
        ]));
        assert_eq!(two, Some("v. hello · n. greeting".to_owned()));
        assert!(
            !two.unwrap().contains('\n'),
            "含换行会被宿主数成多行、撑高候选条"
        );
    }
}
