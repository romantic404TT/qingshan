//! Java 键码语义 → 青简 Core 的方法调用。
//!
//! Core 不认 keyCode，`push` 只吃一个 `char`（`qingjian-core/src/engine/composing.rs:168`），
//! 键码到字符的映射归宿主。水杉 Java 已经替我们做完了这一步：`NativeClient.characterRaw(session, ascii, shift)`
//! 传的 `ascii` 就是映射后的字符码，所以这里不需要再抄 Linux 壳 `dispatch/key/input.rs`
//! 那张表，只需要做窄校验与 shift 还原。
//!
//! 命令号语义在**水杉**的 `crates/host-api/src/ffi/input.rs`：宿主函数
//! `msime_client_command` 在 `:157`，25 条分支占 `:159-183`，未知编号在 `:184` 回
//! `"unknown input command"`。Android 侧 `platforms/android/native/client_jni.cpp:593-594`
//! 不做任何校验、原样 `static_cast<uint32_t>` 转调同一个函数，所以水杉 Android 用的就是这张表。
//! 本文件的 `match` 与它逐条对齐，`Core 侧对应` 一列的出处都写在行内注释里。

use jni::sys::{jboolean, jint};

use crate::session::Session;

/// 一次命令的处理结果。四态的分工见各变体注释。
#[derive(Debug)]
pub enum CommandOutcome {
    /// 引擎或游标已变动，宿主随后重查并下发新 View。
    Consumed,
    /// 有文本要上屏，进 `Transition.commit`。
    Commit(String),
    /// 这个键不归引擎处理，交回宿主。与水杉上游的 `KeyResult::unhandled()` 同义
    /// （`crates/engine/src/session/input.rs:865-867` 对 `ConvertHanja`/`ConversionLeft`/`ConversionRight`
    /// 就是 unhandled），不是「我们没做」。
    Passthrough,
    /// 青简 Core 没有对应能力（`9 Finish`、`11 CommitReading`、`15 CommitRawWithoutLearning`
    /// 以及未知编号）。回给 Java 时**不能**回 `ok:false`：`MSIMEInputService.java:3125` 的 catch
    /// 吃 `JSONException | LinkageError`，而 `value()`（`:844`）对 `ok:false` 信封会走异常路径，
    /// 结果 `fail()` 把整个会话降级成「仅直接输入」。必须回 `ok:true` + `handled:false`。
    NotSupported(i32),
}

/// 上游 Java 侧 `NativeClient.character()` 在跨进 native 之前已做 ASCII 门槛
/// （`NativeClient.java:456`：`if (ascii < 0 || ascii > 127) throw new IllegalArgumentException`），
/// 所以 `characterRaw` 永远只收到 0..127。
/// 我们用 `char::from_u32` + `!is_control()` 兜住 0 与控制字符。
/// `shift` 为真时对 ASCII 字母 `to_ascii_uppercase`（非字母不动）。
///
/// 大写形式是 Core 的契约要求的：`Engine::push` 里的 `shift_letter_compose` 分支要求收到的
/// 就是大写字母，再由 `Composition::push_shifted`（`composition.rs:62-68`）按小写进缓冲区、
/// 原样上屏时经 `typed_text()` 还原。该开关在 `assemble::create_engine` 里显式打开。
/// Java 侧调用点分三种习惯（已大写 + shift=true、原样 + 真 shift、单参走 `letterCase`），
/// 对已是大写的字母再大写是恒等操作，故三向安全。
///
/// `jni 0.22` 把 `jboolean` 映射成 Rust 的 `bool`（不是 C 的 `uint8_t`），故直接对 `shift` 求值。
pub fn char_for(ascii: jint, shift: jboolean) -> Option<char> {
    let c = char::from_u32(ascii as u32)?;
    if c.is_control() {
        return None;
    }
    Some(if shift { c.to_ascii_uppercase() } else { c })
}

/// 把水杉命令号翻译成一次 Core 调用或游标移动。
///
/// 代次推进与候选缓存刷新**不在这里**：那由调用方（`lib.rs` 的 `commandRaw`）成对做
/// `advance()` → 动作 → `query()` → `refresh()`，与上游 `runtime.rs:321-324` 的
/// `advance(); refresh();` 编排一致。
pub fn apply_command(session: &mut Session, command: jint) -> CommandOutcome {
    let command = command as i32;
    match command {
        // 0 = Backspace。Engine::backspace 在辅码态删码段，与 Core 自己的规则一致。
        // 返回 false 说明组句是空的：这一删落在应用里刚上屏的字上，交给 `note_backspace` 记账
        // （用户接着重打同一段拼音选别的词时，Core 把上一次的学习整笔退回），并回 `Passthrough`
        // 让宿主自己去删那个字——上游 macOS 壳同法（`apps/macos/src/imk/controller/command.rs:11-24`）。
        // 以前这里无条件回 `Consumed`：宿主见 `handled:true` 就不删字，⌫ 在空组句下是死的。
        0 => {
            if session.engine.backspace() {
                CommandOutcome::Consumed
            } else {
                session.engine.note_backspace();
                CommandOutcome::Passthrough
            }
        }
        // 1 = SelectHighlighted。高亮下标在宿主（Core 没有游标概念），
        // 上游对应 runtime.rs:2022-2025：有候选就选高亮的那个。
        // 先 clone 出候选再动 engine，否则 &session 与 &mut session.engine 会同源借用冲突。
        1 => match session.highlighted_candidate() {
            Some(candidate) => CommandOutcome::Commit(session.engine.commit(&candidate)),
            None => CommandOutcome::Passthrough,
        },
        // 2 = CommitRaw：整段原样上屏，不参与学习。
        2 => CommandOutcome::Commit(session.engine.take_raw()),
        // 3 = Cancel。上游 crates/engine/src/session/input.rs:858-863 只做 reset_composition + 打断 chain，
        // 逐项对应 Engine::clear（composing.rs:208-218）。
        // 不用 discard_input：它是 EngineSession 的方法（session.rs:89）且连 english_mode 一起清。
        3 => {
            session.engine.clear();
            CommandOutcome::Consumed
        }
        4 => {
            session.engine.move_cursor_left();
            CommandOutcome::Consumed
        }
        5 => {
            session.engine.move_cursor_right();
            CommandOutcome::Consumed
        }
        6 => {
            session.engine.move_cursor_home();
            CommandOutcome::Consumed
        }
        7 => {
            session.engine.move_cursor_end();
            CommandOutcome::Consumed
        }
        8 => {
            session.engine.delete_forward();
            CommandOutcome::Consumed
        }
        // 9 = Finish：上游是 Action 而非 Command，其在青简侧的等价行为未核实 ⇒ 不猜。
        9 => CommandOutcome::NotSupported(command),
        // 10 = CycleKanaVariant（日文假名变体循环）：水杉侧也留给宿主，但青简无此概念。
        10 => CommandOutcome::Passthrough,
        // 11 = CommitReading（日文读音上屏）：无 Core 对应，未核实。
        11 => CommandOutcome::NotSupported(command),
        // 12 = SegmentBackspace：删一个音节。
        12 => {
            session.engine.delete_syllable_backward();
            CommandOutcome::Consumed
        }
        13 => {
            session.engine.move_cursor_syllable_left();
            CommandOutcome::Consumed
        }
        14 => {
            session.engine.move_cursor_syllable_right();
            CommandOutcome::Consumed
        }
        // 15 = CommitRawWithoutLearning：「不学习」在哪一层未核实。
        15 => CommandOutcome::NotSupported(command),
        // 16/17/18 = 日文汉字转换。上游本身即 unhandled（crates/engine/src/session/input.rs:865-867）。
        16 | 17 | 18 => CommandOutcome::Passthrough,
        // 100/101 = 翻页。游标在宿主，见 Session::turn_page。
        100 => {
            session.next_page();
            CommandOutcome::Consumed
        }
        101 => {
            session.previous_page();
            CommandOutcome::Consumed
        }
        // 102-105 = 候选游标移动。上游同样是宿主行为（runtime.rs:1852）。
        102 => {
            session.next_candidate();
            CommandOutcome::Consumed
        }
        103 => {
            session.previous_candidate();
            CommandOutcome::Consumed
        }
        104 => {
            session.first_candidate();
            CommandOutcome::Consumed
        }
        105 => {
            session.last_candidate();
            CommandOutcome::Consumed
        }
        // 上游同样拒绝未知编号（crates/host-api/src/ffi/input.rs:184），但我们回 ok:true + handled:false 而非 ok:false。
        _ => CommandOutcome::NotSupported(command),
    }
}

/// 标点键：`punctuationWithContext` 的落点。
///
/// 契约出处（本轮逐字读过）：入口 `crates/host-api/src/ffi/input.rs:199`，
/// 选路 `crates/client-core/src/punctuation.rs:32`，收尾
/// `crates/input-runtime/src/runtime.rs:1684`。三件关键事实：
/// 1. **组字中一律走 Engine**（`crates/client-core/src/punctuation.rs:33` 第一句就是
///    `if context.has_composition || !context.host_context_available { return Engine }`）
///    ⇒ 用户最高频的那条路必须由我们收尾。回 `handled:false` 不是"交回宿主"这么干净：
///    宿主 `qingshan/host/java/app/msime/android/core/MSIMEInputService.java:3223` 见 false 会发命令 9
///    （`FINISH_COMPOSITION_COMMAND`，同文件 `:102`），而命令 9 在我们这边是 `NotSupported` ⇒ 组字被丢、标点还是半角。
/// 2. **先上屏高亮候选，再问转换**（`crates/input-runtime/src/runtime.rs:1699-1700`
///    的注释写明：对着还在组字的状态调标点会选到零号候选）。
/// 3. **不转也要把组字收掉**（`crates/input-runtime/src/runtime.rs:1726-1728`
///    把原字符接在那次 commit 后面，一次性上屏）。
///
/// 与上游的两处**刻意差异**，都记在这里：
/// - `preceding`（宿主读来的光标前一个码点）我们**不参与选路**。上游用它做
///   "数字/字母后面那个 `,` `.` `:` 保持半角"的判断（`crates/client-core/src/punctuation.rs:44-56`），
///   而 Core 自己有一份等价状态：`crates/qingjian-core/src/punctuation.rs:52`
///   的 `'.' if after_digit => return None`，由 `push`/`note_passthrough` 喂。
///   也就是说"数字后的点保持半角"这条我们照做，只是依据的是引擎自己看到的序列，
///   不是编辑器光标前那个字符。
/// - 上游对非法入参回 `ok:false`（`crates/host-api/src/ffi/input.rs:204`、`:213`）。我们**一个 `ok:false` 都不回**：
///   按 `wire.rs` 文件头的既定口径，`ok:false` 在这个宿主里等于会话降级，
///   而入参不合法不是"引擎不可用"，是"这键我们不接管" ⇒ 回 `Passthrough`，
///   宿主自己写原字符，会话活着。
pub fn punctuation(session: &mut Session, ascii: jint) -> CommandOutcome {
    let code = ascii as u32;
    // 上游门槛是 `is_ascii_punctuation`；越界的直接交回宿主，不猜它想打什么。
    let Some(c) = char::from_u32(code) else {
        return CommandOutcome::Passthrough;
    };
    if !c.is_ascii_punctuation() {
        return CommandOutcome::Passthrough;
    }
    // Core 的 `convert` 不看语言（`crates/qingjian-core/src/punctuation.rs:48` 起只查表与 `after_digit`），
    // 「中文模式下才转」这件事按契约由调用方负责 ⇒ 英文模式交回宿主。
    if session.english_mode() {
        return CommandOutcome::Passthrough;
    }
    // 先收尾：有组字就把高亮的那个候选上屏（与命令 1 同一条路）。
    let base = match session.highlighted_candidate() {
        Some(candidate) => session.engine.commit(&candidate),
        None => String::new(),
    };
    match session.engine.punctuate(c) {
        Some(mark) => CommandOutcome::Commit(format!("{base}{mark}")),
        // 引擎不转这个标点：有组字就把原字符接在后面一起上屏（上游同法），
        // 没组字就交回宿主——此时 `hasEngineComposition()` 为假，宿主不会再发命令 9。
        None if !base.is_empty() => {
            let mut text = base;
            text.push(c);
            CommandOutcome::Commit(text)
        }
        None => CommandOutcome::Passthrough,
    }
}

/// `resetCache` 的落点：把整张候选缓存丢掉。`run_transition` 紧接着 `advance()` 与
/// `requery()`，所以返回时代次已换、表按当前组字重建 —— 宿主那句「已清除候选缓存」到此为真。
///
/// 上游同名动作还顺带释放日文字典的进程级缓存（`crates/engine/src/ime/mod.rs:624`），
/// 我们没有那一层缓存，语义到这张表为止。
pub fn reset_cache(session: &mut Session) -> CommandOutcome {
    session.candidates.clear();
    CommandOutcome::Consumed
}

#[cfg(test)]
mod tests {
    use super::*;
    use qingjian_core::{Candidate, CandidateKind, Engine};
    use qingjian_dictionary::Dictionary;

    const SAMPLE: &str = "你好\tni hao\t9000\n拟好\tni hao\t3000\n泥壕\tni hao\t800\n";

    fn session(count: usize) -> Session {
        let engine = Engine::new(Dictionary::parse(SAMPLE).expect("夹具词库"));
        let mut session = Session::new(engine, 3);
        session.refresh(
            (0..count)
                .map(|i| Candidate {
                    text: format!("c{i}"),
                    kind: CandidateKind::Chinese,
                    syllables: Vec::new(),
                    reading: None,
                    translation: None,
                    aux_code: None,
                })
                .collect(),
        );
        session
    }

    /// 挂一个内存版学习器：撤销的账要真的记上，才测得出「误选被退回」。
    fn learning_session() -> Session {
        let engine = Engine::new(Dictionary::parse(SAMPLE).expect("夹具词库"))
            .with_learner(Box::new(qingjian_learning::FrequencyLearner::default()));
        Session::new(engine, 3)
    }

    fn type_pinyin(session: &mut Session, letters: &str) {
        for c in letters.chars() {
            session.engine.push(c);
        }
        session.requery();
    }

    fn commit_word(session: &mut Session, word: &str) {
        let candidate = session
            .candidates
            .iter()
            .find(|c| c.text == word)
            .cloned()
            .unwrap_or_else(|| panic!("候选里没有 {word}"));
        session.engine.commit(&candidate);
    }

    /// 重新打一遍这段拼音，看第一格是谁（学习的作用只体现在次序上，这样断言不依赖内部计数）。
    /// 读完就清组句——不留残串，否则调用方下一次 `type_pinyin` 会接在后面变成两段。
    fn first_cell(session: &mut Session, letters: &str) -> String {
        apply_command(session, 3);
        session.requery();
        type_pinyin(session, letters);
        let first = session
            .candidates
            .first()
            .map_or_else(String::new, |c| c.text.clone());
        apply_command(session, 3);
        session.requery();
        first
    }

    #[test]
    fn backspace_inside_a_composition_is_consumed_and_leaves_the_erase_ledger_alone() {
        let mut session = learning_session();
        assert_eq!(
            first_cell(&mut session, "nihao"),
            "你好",
            "夹具前提：静态次序"
        );
        type_pinyin(&mut session, "nihao");
        commit_word(&mut session, "拟好");
        assert_eq!(
            first_cell(&mut session, "nihao"),
            "拟好",
            "夹具前提：学一次的 拟好（3000）要顶过 你好（9000）"
        );
        type_pinyin(&mut session, "nihao");
        assert!(
            matches!(apply_command(&mut session, 0), CommandOutcome::Consumed),
            "组句非空时这一删是引擎自己的事，不许交回宿主"
        );
        assert_eq!(session.engine.composition().text(), "niha");
        apply_command(&mut session, 3);
        assert_eq!(
            first_cell(&mut session, "nihao"),
            "拟好",
            "组句内的 ⌫ 不是「删掉刚上屏的词」，学习不许被退回"
        );
    }

    #[test]
    fn empty_composition_backspace_passes_through_and_retracts_the_mis_pick() {
        let mut session = learning_session();
        type_pinyin(&mut session, "nihao");
        commit_word(&mut session, "拟好");
        assert_eq!(first_cell(&mut session, "nihao"), "拟好", "误选已学到");
        assert!(
            session.engine.composition().is_empty(),
            "下面两下 ⌫ 落在应用里刚上屏的字上"
        );
        for _ in 0..2 {
            assert!(
                matches!(apply_command(&mut session, 0), CommandOutcome::Passthrough),
                "组句为空时回 Passthrough，宿主才会去删那个字（以前回 Consumed，⌫ 是死的）"
            );
        }
        type_pinyin(&mut session, "nihao");
        commit_word(&mut session, "你好");
        assert_eq!(
            first_cell(&mut session, "nihao"),
            "你好",
            "删干净后重打同一段拼音选了别的词 ⇒ 上一次的学习要整笔退回"
        );
    }

    #[test]
    fn char_for_accepts_printable_ascii_only() {
        assert_eq!(char_for('a' as i32, false), Some('a'));
        assert_eq!(char_for('a' as i32, true), Some('A'), "shift 还原大写");
        assert_eq!(
            char_for('A' as i32, true),
            Some('A'),
            "已大写的再大写是恒等"
        );
        assert_eq!(char_for('7' as i32, true), Some('7'), "非字母不动");
        assert_eq!(char_for(0, false), None, "NUL 是 control");
        assert_eq!(char_for(10, false), None, "换行是 control");
        assert_eq!(char_for(-1, false), None, "负数不可能是字符");
        assert_eq!(
            char_for(0xD800, false),
            None,
            "代理项被 char::from_u32 拒掉"
        );
        // 上游 Java 已挡 0..127；这里证明即便收到超界值也不会 panic 或产坏字符。
        assert_eq!(char_for('中' as i32, false), Some('中'));
    }

    #[test]
    fn commands_map_to_engine_calls_without_touching_generation() {
        let mut s = session(25);
        let generation = s.generation;
        assert!(matches!(
            apply_command(&mut s, 100),
            CommandOutcome::Consumed
        ));
        assert_eq!(s.highlighted, 10, "翻到第 1 页首格");
        assert_eq!(s.generation, generation, "翻页不涨代次");
        assert!(matches!(
            apply_command(&mut s, 102),
            CommandOutcome::Consumed
        ));
        assert_eq!(s.highlighted, 11);
        assert!(matches!(
            apply_command(&mut s, 105),
            CommandOutcome::Consumed
        ));
        assert_eq!(s.highlighted, 24);
    }

    #[test]
    fn unsupported_and_passthrough_arms_are_separated() {
        let mut s = session(1);
        assert!(matches!(
            apply_command(&mut s, 9),
            CommandOutcome::NotSupported(9)
        ));
        assert!(matches!(
            apply_command(&mut s, 999),
            CommandOutcome::NotSupported(999)
        ));
        assert!(matches!(
            apply_command(&mut s, 16),
            CommandOutcome::Passthrough
        ));
        assert!(matches!(
            apply_command(&mut s, 10),
            CommandOutcome::Passthrough
        ));
    }

    #[test]
    fn select_highlighted_with_no_candidates_does_not_panic() {
        let mut s = session(0);
        assert!(matches!(
            apply_command(&mut s, 1),
            CommandOutcome::Passthrough
        ));
    }

    /// 组字中按逗号（本轮真机撞到的那条路）：先把高亮候选上屏，再接 Core 转出来的全角逗号，
    /// **一次 Commit 说完**，不留组字、也不回 `ok:false`。
    #[test]
    fn punctuation_finishes_the_composition_and_appends_the_converted_mark() {
        let mut session = learning_session();
        type_pinyin(&mut session, "nihao");
        assert_eq!(
            session.highlighted_candidate().map(|c| c.text),
            Some("你好".to_string()),
            "夹具里 ni hao 的首选是 你好，这条断言是下面那条的前提"
        );
        match punctuation(&mut session, b',' as jint) {
            CommandOutcome::Commit(text) => {
                assert_eq!(text, "你好，", "高亮候选 + 全角逗号一次性上屏")
            }
            other => panic!("应当是 Commit，实际 {other:?}"),
        }
    }

    /// 待办 28 的那根线在这里接上：关掉「中文标点」之后 Core 不转，
    /// 但组字仍要收尾 ⇒ 上屏的是原字符。**这条测试就是 `punctuate()` 的调用点。**
    #[test]
    fn punctuation_after_turning_the_switch_off_keeps_the_ascii_mark() {
        let mut session = learning_session();
        session.set_chinese_punctuation(false);
        type_pinyin(&mut session, "nihao");
        match punctuation(&mut session, b',' as jint) {
            CommandOutcome::Commit(text) => {
                assert_eq!(text, "你好,", "开关关了就不转，但组字照样收")
            }
            other => panic!("应当是 Commit，实际 {other:?}"),
        }
    }

    /// 三种"我们不接管"的情形都回 `Passthrough`：英文模式、非 ASCII 标点入参。
    /// 回 `ok:true` + `handled:false` 而不是 `ok:false`——后者在这个宿主里等于会话降级。
    #[test]
    fn punctuation_declines_without_killing_the_session() {
        let mut english = learning_session();
        english.engine.set_english_mode(true);
        assert!(
            matches!(
                punctuation(&mut english, b',' as jint),
                CommandOutcome::Passthrough
            ),
            "英文模式不转中文标点"
        );
        let mut letters = learning_session();
        assert!(
            matches!(
                punctuation(&mut letters, b'n' as jint),
                CommandOutcome::Passthrough
            ),
            "不是 ASCII 标点就不接（宿主本来也不会这样调）"
        );
        assert!(
            matches!(
                punctuation(&mut letters, 9_000 as jint),
                CommandOutcome::Passthrough
            ),
            "超出 char 范围的入参也不能 panic"
        );
    }

    /// `resetCache` 要的语义：**整张**候选缓存真的被丢掉，不是"回个 ok 就算做了"。
    /// 判别器两条都实跑过：函数体换成 `let _ = &mut session.candidates;` ⇒ 最后那条红
    /// （`缓存里一条都不该留`）；返回值换成 `CommandOutcome::Passthrough` ⇒ 第二条红。
    #[test]
    fn reset_cache_drops_the_whole_candidate_table() {
        let mut session = session(3);
        assert_eq!(session.candidates.len(), 3, "夹具：先要有一张非空缓存");
        assert!(
            matches!(reset_cache(&mut session), CommandOutcome::Consumed),
            "要回 Consumed，`run_transition` 才会推进代次并按当前组字重建"
        );
        assert!(session.candidates.is_empty(), "缓存里一条都不该留");
    }
}
