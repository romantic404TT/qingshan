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
}
