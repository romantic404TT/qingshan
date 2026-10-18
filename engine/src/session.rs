//! 会话表：`session id → Session`，用 `Mutex` 串行化，并持有宿主侧的候选游标。
//!
//! `Engine` 内含多处 `RefCell`（`qingjian-core/src/engine/mod.rs:147/220/223/226/294/184`），
//! 因此 `Engine: !Sync` 是确定的；`Send` 上游没有显式断言，靠下面的 `assert_send` 常驻钉住。
//! JNI 回调都发生在 `:ime` 进程的调用线程上，所以用 `Mutex` 包住整个会话，宁可串行也不共享可变引用。
//!
//! 表本身必须是 `static`：水杉的 `NativeClient` 全部是静态方法，Java 侧只拿 `long session`
//! 句柄，Rust 侧没有任何随进程存活的实例可挂状态。
//!
//! 锁中毒一律 `into_inner` 继续用，不 `unwrap` 抛：`with_env` 的 `catch_unwind` 会接住 panic，
//! 但 guard 展开时已把 `Mutex` 标成 poisoned，若据此 panic 则此后每次 JNI 调用都失败，
//! 键盘永久退化到进程被杀。而 `Engine` 内的借用冲突是瞬时状态，不是数据损坏。
//!
//! ## 为什么游标在宿主而不在 Core
//!
//! 青简 Core 不分页、也没有「高亮第几个」的概念：`Engine::query(&self)` 一次给出排好序的
//! 整表（`engine/query/mod.rs:26`），`Query` 里既没有 `page` 也没有 `generation`。而水杉
//! Java 的候选行渲染要求 `View.page/page_count/page_size` 与每个候选的 `highlighted`
//! （已镜像在 `view.rs`），`selectRaw` 又要求我们能认出 `CandidateId{session,generation,index}`。
//! 上游水杉自己也是把这三样放在宿主层的：`input-runtime/src/runtime.rs:754` 的
//! `page_count`、`:701` 的 `page = highlighted / page_size`、`:75` 的 `generation` 字段，
//! 全在 runtime 而不在引擎。
//!
//! ## 代次校验为什么必须我们做、且每次都涨号
//!
//! Java 送回 `(generation, index)` 时**不做任何代次校验**：
//! `MSIMEInputService.java:6259-6260` 只比 `session`，`NativeClient.java:496` 只挡 `index < 0`，
//! `generation` 原样转发。所以过期 View 的 `index` 会命中新列表的同一下标——
//! 水杉自己在 `runtime.rs:620-621` 写明了同一件事：「A reorder has to advance the generation
//! (old IDs would otherwise select by the new seats)」。青简的 `push`/`clear` 返回 `()`，
//! 没有「内容变没变」的信号，因此采用无条件涨号（比上游的 `moved` 条件更宽，不存在旧号存活窗口）。

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use qingjian_core::{Candidate, CandidateList, Engine, EngineSession};

use crate::assemble::{DataPaths, Error, create_engine};

/// 常驻断言：会话表能作为 `static` 存在的前提是 `Engine: Send`
/// （`Mutex<T>: Sync` 要求 `T: Send`）。上游若引入非 `Send` 字段，这里立即编译失败，
/// 而不是等到 3.5 交叉编译或设备运行时才暴露。
const fn assert_send<T: Send>() {}
const _: () = assert_send::<Engine>();
const _: () = assert_send::<Session>();
/// 挂起表是 `static`，所以 `EngineSession` 也必须 `Send`（同样道理见上一条 `Session`）。
const _: () = assert_send::<EngineSession>();

/// 每页候选数。值抄自水杉 `crates/client-core/src/preferences.rs:3373` 的
/// `pub const MAX_CANDIDATE_PAGE_SIZE: u8 = 10;`——本工程不依赖水杉任何 crate
/// （`Cargo.lock` 里 msime 系包名 0 命中），只能本地重述。
/// 上游同文件 `:2608` 校验 `1..=MAX_CANDIDATE_PAGE_SIZE`，那个下限的存在理由是
/// `page_count` 用 `div_ceil(page_size)`，除 0 会 panic；本项目 page_size 恒为 10，不可配
/// （读写偏好的 native 入口属于 85 个 `not_implemented` 桩，本来也收不到那个值）。
const MAX_CANDIDATE_PAGE_SIZE: usize = 10;

/// 学习数据落盘的最小间隔。抄上游 Linux 壳 `apps/linux/server/src/dispatch/mod.rs:24`
/// 的 `LEARNING_FLUSH_INTERVAL`（Windows 壳同值，`apps/windows/server/src/dispatch/mod.rs:42`）。
/// 为什么必须节流：`FrequencyLearner::flush` 走 `write_atomic`，是**落盘级**操作，
/// 挂在每次上屏上就是拿用户可感知的路径做磁盘 IO。
const LEARNING_FLUSH_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("候选代次已耗尽")]
    IdentityExhausted,
}

/// 会话的输入框身份（方案甲-2 的缓存键）。粒度是**应用级**：`EditorInfo.packageName`，
/// 同一个 app 里两个输入框共用一份挂起上下文，这是甲-2 已知且已记录的粒度限制。
#[derive(Debug, Clone)]
pub struct SessionIdentity {
    /// 包名；Java 那边 `currentEditorPackage` 为空串时按 `None` 处理（不挂起也不换回）。
    pub application: Option<String>,

    /// 这个框允许不允许个性化学习。假 = 水杉的 `learningSuppressed()` 为真
    /// （输入框不许个性化，或隐私模式开着，`MSIMEInputService.java:1905-1907`），
    /// 此时 Java 会把建会话选项里的 `preferences.learning` 置 false（`:1298`）。
    /// 这样的会话**既不换回也不挂起**，并且会清掉本包名已有的挂起上下文——
    /// Core 在 `engine/session.rs:90` 写明了理由：「不把私密文本带到下一次普通输入」。
    pub learning_allowed: bool,
}

impl Default for SessionIdentity {
    /// 「没有身份」= 普通输入框：不挂起也不换回，但**不是**私密框。
    /// 这里不能图省事用 `derive(Default)`——它会把 `learning_allowed` 弄成 `false`，
    /// 等于把所有没带身份的会话都当不许学习的框处理，方向正好反了。
    fn default() -> Self {
        SessionIdentity {
            application: None,
            learning_allowed: true,
        }
    }
}

impl SessionIdentity {
    /// 能当挂起键的包名；`None` 时既不挂起也不换回（`packageName` 拿不到时 Java 给的是空串）。
    fn cache_key(&self) -> Option<&str> {
        self.application.as_deref().filter(|key| !key.is_empty())
    }
}

#[cfg(test)]
impl SessionIdentity {
    /// 测试夹具：包名 + 允许学习。生产路径的实例由 `host_request()` 从建会话选项里解出来。
    fn of(application: &str) -> Self {
        SessionIdentity {
            application: (!application.is_empty()).then(|| application.to_owned()),
            learning_allowed: true,
        }
    }

    /// 测试夹具：不许学习 / 隐私模式的输入框。
    fn private(application: &str) -> Self {
        SessionIdentity {
            application: (!application.is_empty()).then(|| application.to_owned()),
            learning_allowed: false,
        }
    }
}

/// 一次输入法会话：引擎 + 宿主侧的候选列表快照与游标。
pub struct Session {
    pub engine: Engine,
    /// 本会话对应的输入框身份，销毁时决定挂起键与要不要挂起。
    pub identity: SessionIdentity,
    /// 本会话候选列表的代次号。创建时取 session id（天然唯一），此后 `advance()` 逐次 +1。
    pub generation: u64,
    /// 当前高亮候选在整表中的下标（不是页内下标；上游 `runtime.rs:1852` 同样是整表下标）。
    pub highlighted: usize,
    /// 每页候选数，恒为 [`MAX_CANDIDATE_PAGE_SIZE`]。
    pub page_size: usize,
    /// 最近一次 `query()` 得到的整表，`selectRaw` 与命令 1/102-105 都以它为准。
    pub candidates: Vec<Candidate>,
    /// Java 的 `focusRaw(session, focused)` 设置，原样写进 `View.focused`。
    pub focused: bool,
    /// Java 的 `setCharacterWidthRaw(session, fullwidth)` 设置，原样写进
    /// `View.character_width`（戊-C）。
    ///
    /// 存**会话级**而不是进程级：宿主每次建会话都在 `MSIMEInputService.java:1712` 用
    /// `fullWidthPreference` 重新推一遍，所以新会话第一帧就是对的，不需要跨会话记住了什么。
    pub fullwidth: bool,
    /// 上次学习数据落盘的时刻。会话创建即计时，所以头 60 秒内的上屏只在 destroy 时落。
    last_flush: Instant,
    /// 自上次落盘后有没有新的上屏（`note_commit` 置位）。
    ///
    /// 这只是「要不要费劲去问 learner」的前置门；真正的「有没有东西可写」由
    /// `FrequencyLearner` 自己的 `dirty` 决定（`learner_impl.rs:209` 起，不 dirty 就不写文件）。
    dirty: bool,
}

impl Session {
    pub fn new(engine: Engine, generation: u64) -> Self {
        Session {
            engine,
            identity: SessionIdentity::default(),
            generation,
            highlighted: 0,
            page_size: MAX_CANDIDATE_PAGE_SIZE,
            candidates: Vec::new(),
            focused: false,
            fullwidth: false,
            last_flush: Instant::now(),
            dirty: false,
        }
    }

    /// 把宿主的「中文标点」开关写进引擎。引擎侧的名字是 `full_width_punctuation`
    /// （`qingjian/crates/qingjian-core/src/engine/setup.rs:9`，注释逐字「设置中文模式的标点转换。」），
    /// 读它的是 `qingjian/crates/qingjian-core/src/engine/composing.rs:16` 的 `punctuate()`。
    ///
    /// **现状要说白**：我们这条壳路上目前**没有** `punctuate()` 的调用点（宿主按标点键走的是
    /// `punctuationWithContext`，那是桩、且在杀会话组，见 PIN「启动路径的击穿判定」），
    /// 所以这个开关**真机不可观测**。它仍然要做，是因为：① 它让引擎状态与宿主卡片一致，
    /// 将来把标点路由到 Core 时不必再补；② 每次建会话宿主都会重放一遍
    /// （`qingshan/host/java/app/msime/android/core/MSIMEInputService.java:1713`），
    /// 挂起再恢复的引擎不会带着旧值回来。
    pub fn set_chinese_punctuation(&mut self, enabled: bool) {
        self.engine.set_full_width_punctuation(enabled);
    }

    /// 上屏之后调：记脏，到点才真写盘。
    pub fn note_commit(&mut self) {
        self.dirty = true;
        if self.last_flush.elapsed() >= LEARNING_FLUSH_INTERVAL {
            // 这条路径在用户能感知的链上，所以耗时用 `info!` 打出来——真机就是靠它量
            // 「节流之后还剩多少可感知延迟」（PIN 该节第 2 方案的前置测量）。
            self.flush_now(true, "上屏满 60 秒");
        }
    }

    /// 会话销毁前调，**不看脏标记也不看时间**：`Engine::forget`（删词）也会让 learner 变脏，
    /// 而那条路没有 `note_commit`。learner 自己不 dirty 就不写文件
    /// （`learner_impl.rs:209` 起的 `if self.dirty`），所以这里无代价。
    ///
    /// 日志口径：这一条每次离开输入框都会走，所以只在「本会话确实攒了上屏」时打 `info!`，
    /// 否则打 `debug!`（日志桥按 level 挡 `debug!`，`logcat.rs:44`）。真机要能看见销毁
    /// 这条路有没有真的落盘，不然整个方案在设备上不可观测。
    pub fn flush_at_destroy(&mut self) {
        self.flush_now(self.dirty, "会话销毁");
    }

    fn flush_now(&mut self, verbose: bool, reason: &str) {
        let started = Instant::now();
        self.engine.flush_learning();
        self.dirty = false;
        self.last_flush = Instant::now();
        let elapsed = started.elapsed().as_millis();
        if verbose {
            tracing::info!(
                "学习数据落盘 {elapsed} ms（{reason}；词频 / 用户词 / n-gram 的 write_atomic）"
            );
        } else {
            tracing::debug!("学习数据落盘 {elapsed} ms（{reason}）");
        }
    }

    /// 重查并刷新候选缓存。
    ///
    /// 不返回 `Result` 或 `bool`：青简的 `ParseError` 三个变体
    /// （`Empty` / `InvalidChar` / `NoSegmentation`，出处 `qingjian-core/src/parser/error.rs:2-16`）
    /// **全部是「这串读不出拼音」**，不是运行时故障。上游 `runtime.rs:469-473` 在
    /// `editing_text` 为空时照样 `refresh()` 并期望 Ok；`engine/query/phonetic.rs:40-46`
    /// 那段 Err 分支的语义也是「拼音这边没戏，英文/快捷也补不出来 ⇒ 无候选」，
    /// 上游对无候选的表现是给空列表，不是给错误。
    ///
    /// 这条边界是真机逼出来的：实测按一次 `i`（或 `im`）就给 `NoSegmentation`，
    /// 若把它当故障回 `ok:false`，宿主 `apply()` 抛异常 → `fail()` → 会话当场降级成
    /// 纯直接输入（`MSIMEInputService.java:1707` 起就是这条链）。
    ///
    /// 前提写死在此：若青简未来引入真正的运行时故障类型，这里需要重新设计返回类型；
    /// 当前没有这种类型。
    ///
    /// 不推进代次——那是 `advance()` 的职责，两者由 `lib.rs` 的导出函数成对编排
    /// （对齐上游 `runtime.rs:321-324` 的 `advance(); refresh();`）。
    /// 重查候选。**`annotate` 必须夹在 `query` 与 `refresh` 之间**：`query()` 交回来的
    /// 候选 `translation` 恒为 `None`（`engine/query/result.rs:53` 的注释是接口契约），
    /// `annotate` 是唯一填它的地方，而 `refresh` 拿走这批 `Candidate` 的所有权——
    /// 顺序反了就再也拿不回来。`CandidateList` 整体传给 `annotate`，`.items` 最后才取。
    ///
    /// 只记「全零」这一种失败：正常打字不打日志，而 `hits == 0 && total > 0` 恰好是
    /// 「表没加载 / 路径不对」的形状——有候选却一条都没查到译词。
    pub fn requery(&mut self) {
        let mut list = match self.engine.query() {
            Ok(query) => query.candidates,
            Err(_) => CandidateList { items: Vec::new() },
        };
        let report = self.engine.annotate(&mut list);
        if report.hits == 0 && report.total > 0 {
            tracing::warn!(
                "译词全未命中：{} 个候选，检查 glossary-en.tsv 是否加载",
                report.total
            );
        }
        self.refresh(list.items);
    }

    /// View 里显示用的编辑态：Shift 敲的字母还原大写后的缓冲区文本
    /// （`Composition::typed_text`，`composition.rs:76`）。
    pub fn editing(&self) -> String {
        self.engine.composition().typed_text()
    }

    /// 编辑态内的字节偏移，交给 `View.caret_position`。
    pub fn caret(&self) -> usize {
        self.engine.composition().cursor()
    }

    pub fn english_mode(&self) -> bool {
        self.engine.english_mode()
    }

    /// 推进代次。`u64::MAX` 时报错而不是回绕——水杉同样宁可不重排（`runtime.rs:625` 的
    /// `checked_add(1).is_none()` 前置判断）。
    pub fn advance(&mut self) -> Result<(), SessionError> {
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(SessionError::IdentityExhausted)?;
        Ok(())
    }

    /// 换上新一次 `query()` 的整表，并把游标归零（上游 `runtime.rs:637` 同做法）。
    /// 不自增代次——那由调用方的 `advance()` 显式负责，两者成对出现。
    pub fn refresh(&mut self, candidates: Vec<Candidate>) {
        self.candidates = candidates;
        self.highlighted = 0;
    }

    /// 当前页号（从 0 起）。空表时为 0，与 `page_count()` 的 0 配对（上游 `runtime.rs:754`
    /// 的 `0usize.div_ceil(10) = 0`，实测边界 1→1 / 10→1 / 11→2 / 25→3）。
    pub fn page(&self) -> usize {
        if self.page_size == 0 {
            return 0;
        }
        self.highlighted / self.page_size
    }

    pub fn page_count(&self) -> usize {
        self.candidates.len().div_ceil(self.page_size)
    }

    /// 当前页 `[start, end)`，越界处按表长截断；空表时是 `(0, 0)`。
    /// `view.rs` 按同一个窗口切当页候选（上游 `runtime.rs:701-717`），两边下标都是整表的。
    fn page_bounds(&self) -> (usize, usize) {
        let start = (self.page() * self.page_size).min(self.candidates.len());
        let end = (start + self.page_size).min(self.candidates.len());
        (start, end)
    }

    /// selectRaw / 命令 1 的入口校验。
    /// 界为当前页窗口（上游 `runtime.rs:1796-1810` 的 `Action::Select` 那组）。
    /// 全表界属于 `Action::SelectAnyCandidate`（`runtime.rs:1788-1794`），是下面的
    /// `candidate_index_in_table`。
    ///
    /// Java 不校验 generation（`MSIMEInputService.java:6259-6260` 只比 session，
    /// `NativeClient.java:496` 只挡 index<0），所以代次校验 100% 在 Rust 侧。
    /// `id.session` 那一项由 HashMap 查键覆盖，不重复。
    pub fn candidate_index(&self, generation: u64, index: usize) -> Option<usize> {
        if generation != self.generation {
            return None;
        }
        let (start, end) = self.page_bounds();
        (index >= start && index < end).then_some(index)
    }

    /// `selectAnyCandidateRaw` 的入口校验：只查代次与整表边界，**不查页窗口**。
    /// 展开候选面板画的是全表，用户点的候选通常在当页之外，用页窗口判会把面板变成摆设
    /// （上游同口径：`runtime.rs:1788-1794` 只比 `session`/`generation` 与
    /// `index >= self.cached.candidates.len()`）。
    pub fn candidate_index_in_table(&self, generation: u64, index: usize) -> Option<usize> {
        if generation != self.generation {
            return None;
        }
        (index < self.candidates.len()).then_some(index)
    }

    /// 命令 1：高亮候选的快照。`Engine::commit` 收 `&Candidate`，但从闭包里拿引用会跨
    /// 借用域，所以这里返回 owned clone。
    pub fn highlighted_candidate(&self) -> Option<Candidate> {
        self.candidates.get(self.highlighted).cloned()
    }

    /// 命令 102-105：整表下标移动，越界钳住（上游 `runtime.rs:1852` 的
    /// `(self.highlighted + 1).min(len - 1)`）。空表时不动。
    pub fn next_candidate(&mut self) {
        if let Some(last) = self.candidates.len().checked_sub(1) {
            self.highlighted = (self.highlighted + 1).min(last);
        }
    }

    pub fn previous_candidate(&mut self) {
        self.highlighted = self.highlighted.saturating_sub(1);
    }

    pub fn first_candidate(&mut self) {
        self.highlighted = 0;
    }

    pub fn last_candidate(&mut self) {
        if !self.candidates.is_empty() {
            self.highlighted = self.candidates.len() - 1;
        }
    }

    /// 命令 100/101：整页移动。**只在页号真的变了时**调 `Engine::note_page_turn()`，
    /// 落点取目标页首格并钳在表尾——两条都抄已验证的壳实现
    /// （`apps/windows/server/src/dispatch/composed/mod.rs:110-121`，linux 同处在 `:85-101`，
    /// 两者的 `page_count` 都用 `div_ceil`）。已在末页/首页时无操作，因此也不调 note_page_turn。
    ///
    /// 不涨代次：翻页不改变候选内容，而拦截过期 id 的唯一防线就是页窗口界
    /// （见 `candidate_index` 的注释）。
    pub fn next_page(&mut self) {
        self.turn_page(1);
    }

    pub fn previous_page(&mut self) {
        self.turn_page(-1);
    }

    fn turn_page(&mut self, step: isize) {
        let count = self.candidates.len();
        if count == 0 || self.page_size == 0 {
            return;
        }
        let current = (self.highlighted / self.page_size) as isize;
        let target = (current + step).clamp(0, self.page_count() as isize - 1);
        if target != current {
            self.engine.note_page_turn();
        }
        self.highlighted = (target as usize * self.page_size).min(count - 1);
    }
}

/// 发给 Java 的 `CandidateId.session` 取会话表键，`CandidateId.generation` 取 [`Session::generation`]。
static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);
static SESSIONS: OnceLock<Mutex<HashMap<u64, Session>>> = OnceLock::new();

/// 挂起上下文的容量上限（按包名一份）。用户切过的 app 越多，常驻的输入上下文不能无限涨，
/// 所以到顶按「最久没再挂」淘汰一份。淘汰只丢内存里的输入上下文，学习数据另有落盘那条路。
const PARKED_SESSIONS: usize = 8;

/// 挂起的输入上下文，按包名。`EngineSession` 只装组句、标点、撤销账本、学习链这些小结构，
/// 词库与学习者仍在 `Engine` 里（Core 的模块文档 `engine/session.rs:1`：
/// 「词库与用户学习服务仍由同一个 Engine 持有」），所以一份挂起项不复制词库。
/// **只在进程存活期内存在，不落盘**：水杉那边 `LastCommit` 没有 `Serialize`，
/// 而 `flush_learning` 也不含它，跨冷启动恢复只能改 Core（已排除）。
static PARKED: OnceLock<Mutex<HashMap<String, (EngineSession, u64)>>> = OnceLock::new();
static NEXT_PARK_STAMP: AtomicU64 = AtomicU64::new(1);

fn parked() -> &'static Mutex<HashMap<String, (EngineSession, u64)>> {
    PARKED.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 挂起一份输入上下文；超过 [`PARKED_SESSIONS`] 就丢最久没再挂的那份。
/// 换回时条目是被 `remove` 走的，所以「最久没再挂」= 用户最久没离开的那个 app。
fn park(key: String, context: EngineSession) {
    let stamp = NEXT_PARK_STAMP.fetch_add(1, Ordering::Relaxed);
    let mut guard = parked().lock().unwrap_or_else(|e| e.into_inner());
    guard.insert(key, (context, stamp));
    if guard.len() > PARKED_SESSIONS {
        let stalest = guard
            .iter()
            .min_by_key(|(_, (_, used))| *used)
            .map(|(key, _)| key.clone());
        if let Some(stalest) = stalest {
            guard.remove(&stalest);
        }
    }
}

/// 取走某个包名挂起的输入上下文（取不走就是没有，或先前被上限淘汰 / 被私密会话作废）。
fn take_parked(key: &str) -> Option<EngineSession> {
    parked()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(key)
        .map(|(context, _)| context)
}

/// 作废某个包名挂起的输入上下文。
fn drop_parked(key: &str) {
    parked()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(key);
}

fn sessions() -> &'static Mutex<HashMap<u64, Session>> {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 把「这个会话不许个性化学习」真的交给 Core：两个开关一起上。
///
/// Core 侧 `MutedLearner`（`crates/qingjian-core/src/engine/learning/muted.rs:37`）吞的是**写**
/// （record / record_choice / 撤销类），**读**照常转发——排序仍按已有的个人数据算。本轮的边界
/// 就定在这条上：只「不学」，不「失忆」。`set_private` 在我们这个装配里除了学习碰不到别的东西：
/// 预测 / 输入日志 / 生词记录 / 释义兜底四件用的都是 Core 默认的空实现
/// （`crates/qingjian-core/src/engine/mod.rs:409` `NoPredictor`、`:423` `NoInputLogger`、
/// `:435` `NoVocabularyTracker`、`:436` `NoGlossFiller`），而 `engine/src/assemble.rs:131-132`
/// 只装了 learner 与 translator。
pub fn apply_learning_policy(engine: &mut Engine, learning_allowed: bool) {
    engine.set_learning(learning_allowed);
    engine.set_private(!learning_allowed);
}

/// 把「这个框现在是隐私会话」告诉 Core——**只**动 `private` 这一个开关。
///
/// 不能顺手把 `set_learning` 一起翻开：宿主这条通道带的是 `learningSuppressed()`
/// （`host/java/app/msime/android/core/MSIMEInputService.java:1907-1908`：密码框或隐私模式），
/// 里面**不含**用户在设置里关掉的「记忆新词」。Core 也把这两个当独立开关
/// （`crates/qingjian-core/src/engine/setup.rs:82`），所以这里回 `false` 只解私密，
/// 那条学习开关留在它原来的位置。
pub fn mark_private(state: &mut Session, enabled: bool) {
    state.engine.set_private(enabled);
}

/// 加载词库、建会话表项，返回 session id（同时充当首个 generation）。
///
/// `identity` 是输入框身份（方案甲-2）：同一个包名再次建会话时，把上次销毁前挂起的
/// `EngineSession` 换回来，撤销账本与学习链因此跨会话存活。
pub fn create_session(paths: &DataPaths, identity: &SessionIdentity) -> Result<u64, Error> {
    let started = std::time::Instant::now();
    let mut engine = create_engine(paths)?;
    // 换回来的那份上下文不带这两个开关：`swap_session` 换的全是输入上下文
    // （`crates/qingjian-core/src/engine/session.rs:57-86`），私密状态留在 Engine 上由调用方负责
    // （同文件 `:55` 的注释）。所以在这里上一次，之后换回挂起项也不会被冲掉。
    apply_learning_policy(&mut engine, identity.learning_allowed);
    let mut restored = false;
    if let Some(key) = identity.cache_key() {
        if identity.learning_allowed {
            if let Some(mut context) = take_parked(key) {
                // 换进来的是这个包名上次离开时的输入上下文（组句、标点、撤销账本
                // `recent_commits`、学习链、`history`），换出去的是刚建好的那份空状态，随
                // `context` 一起丢弃。
                engine.swap_session(&mut context);
                restored = true;
            }
        } else {
            // 不许学习 / 隐私模式的框：本包名挂着的那份上下文当场作废，
            // 既不让这个框继承，也不留给下一次普通输入。
            drop_parked(key);
        }
    }
    let id = NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed);
    let mut state = Session::new(engine, id);
    state.identity = identity.clone();
    sessions()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(id, state);
    // 用户感知的延迟就是这一条（含上面那条词库载入）；两条分开打才分得清瓶颈在哪。
    tracing::info!(
        "会话建立 {} ms（含词库载入），挂起键={:?}，换回挂起上下文={}",
        started.elapsed().as_millis(),
        identity.cache_key(),
        restored
    );
    Ok(id)
}

/// 移除 session。返回是否真的删掉了。
/// 移除 session。返回是否真的删掉了。
///
/// 移除后**先落盘再放手**：`NativeClient.destroy` 全树只有一个调用点
/// （`MSIMEInputService.java:1598`，在 `stop()` 的 `if (session != 0)` 块里），
/// 而 `stop()` 由 `onFinishInput()` / `onDestroy()` / `fail()` / 会话重建失败四条路到达，
/// 所以挂在这里等于覆盖「离开输入框 / 键盘销毁 / 会话被杀」三个时机。
/// 盖不住的是 `am force-stop` 与低内存回收（没有 Java 回调）——那正是上屏路径还要
/// 60 秒节流的理由。写盘在锁外做，不占着会话表做 IO。
pub fn destroy_session(id: u64) -> bool {
    let taken = sessions()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&id);
    match taken {
        Some(mut state) => {
            state.flush_at_destroy();
            // 挂起之前先丢掉组句：离开输入框时水杉已经把组合文本从文档里 abandon 了
            // （`MSIMEInputService.java:1602-1603`），换回来时不该凭空多出一段用户已放弃的拼音。
            // `Engine::clear()`（`composing.rs:208-218`）只动 composition / aux / retype / page_turns
            // 与 `chain.leave_buffer()`，**不碰 `recent_commits` 也不碰链上的前两个词**，
            // 所以撤销账本照旧跟着换进挂起项。
            state.engine.clear();
            let key = state.identity.cache_key().map(str::to_owned);
            let mut context = EngineSession::default();
            // 换出这份会话的输入上下文，`state.engine` 因此回到空状态，随 `state` 一起 drop。
            state.engine.swap_session(&mut context);
            match (key, state.identity.learning_allowed) {
                (Some(key), true) => {
                    park(key.clone(), context);
                    tracing::info!(
                        "会话挂起 键={:?}，表内 {} 份",
                        key,
                        parked().lock().unwrap_or_else(|e| e.into_inner()).len()
                    );
                }
                // 私密会话不往外挂，顺手把它可能继承过的那份也作废（`create_session` 里已经
                // 作过一次，这里是防有别的入口把私密会话的 `learning_allowed` 漏过去）。
                (Some(key), false) => {
                    tracing::info!(
                        "会话不挂起（不许学习/隐私模式），并作废旧的挂起项 键={:?}",
                        key
                    );
                    drop_parked(&key);
                }
                (None, _) => {}
            }
            true
        }
        None => false,
    }
}

/// 在会话上执行一段闭包，返回结果。锁自动释放。
///
/// 闭包拿的是 `&mut Session` 而不是 `&mut Engine`：游标与候选缓存和引擎必须一起可读写
/// （命令 1 要读 `candidates`、写 `highlighted`、再调 `engine.commit`）。
pub fn with_session<F, R>(id: u64, f: F) -> Option<R>
where
    F: FnOnce(&mut Session) -> R,
{
    let mut guard = sessions().lock().unwrap_or_else(|e| e.into_inner());
    guard.get_mut(&id).map(f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{CommandOutcome, apply_command};
    use qingjian_core::CandidateKind;
    use qingjian_dictionary::Dictionary;

    /// 夹具格式抄自上游 `qingjian-core/src/engine/tests/mod.rs:25` 的 `SAMPLE`：
    /// `词 \t 空格分隔的音节 \t 词频`。内联字符串即可，不碰 2MB 的 `dict.tsv`。
    const SAMPLE: &str = "你好\tni hao\t9000\n拟好\tni hao\t3000\n泥壕\tni hao\t800\n";

    fn session_with(count: usize) -> Session {
        let engine = Engine::new(Dictionary::parse(SAMPLE).expect("夹具词库应当可解析"));
        let mut session = Session::new(engine, 7);
        session.refresh((0..count).map(|i| fake(&format!("c{i}"))).collect());
        session
    }

    /// `Candidate` 没有 `Default` derive（`qingjian-core/src/candidate/mod.rs:27`），
    /// 6 个字段逐个写。游标算术只看 `len()`，内容无关。
    fn fake(text: &str) -> Candidate {
        Candidate {
            text: text.to_string(),
            kind: CandidateKind::Chinese,
            syllables: Vec::new(),
            reading: None,
            translation: None,
            aux_code: None,
        }
    }

    #[test]
    fn page_count_matches_upstream() {
        // 上游 runtime.rs:754 的 len().div_ceil(page_size)；实测边界同上。
        for (len, want) in [(0, 0), (1, 1), (10, 1), (11, 2), (25, 3)] {
            assert_eq!(session_with(len).page_count(), want, "len={len}");
        }
        let empty = session_with(0);
        assert_eq!((empty.page(), empty.page_count()), (0, 0));
    }

    #[test]
    fn page_bounds_clamp_to_the_candidate_list() {
        // (表长, 高亮) → 期望窗口。上游 runtime.rs:701-717 也是先算页、再按表长截断。
        let cases = [(25usize, 0usize, (0usize, 10usize)), (3, 0, (0, 3))];
        for (count, highlighted, want) in cases {
            let mut s = session_with(count);
            s.highlighted = highlighted;
            assert_eq!(s.page_bounds(), want, "表长 {count}、高亮 {highlighted}");
        }
        assert_eq!(session_with(0).page_bounds(), (0, 0), "空表窗口是 (0, 0)");

        let mut second = session_with(25);
        second.highlighted = 12;
        assert_eq!(second.page_bounds(), (10, 20), "高亮第 12 格 → 第 1 页");

        let mut last = session_with(25);
        last.highlighted = 24;
        assert_eq!(last.page_bounds(), (20, 25), "末页 5 个");
    }

    #[test]
    fn candidate_index_requires_current_generation_and_page_window() {
        let s = session_with(25); // generation 7，游标在第 0 页
        assert_eq!(s.candidate_index(7, 0), Some(0));
        assert_eq!(s.candidate_index(7, 9), Some(9));
        assert_eq!(s.candidate_index(7, 10), None, "第 10 格不在当前页窗口内");
        assert_eq!(s.candidate_index(6, 0), None, "旧代次必须拒绝");
        assert_eq!(s.candidate_index(8, 0), None);
    }

    #[test]
    fn stale_index_from_previous_page_is_rejected_after_page_turn() {
        // 翻页不涨号（裁定 1 的表），此时唯一防线就是页窗口界。
        let mut s = session_with(25);
        s.next_page();
        assert_eq!(s.page(), 1);
        assert_eq!(s.generation, 7, "翻页不得推进代次");
        assert_eq!(s.candidate_index(7, 0), None, "上一页的第 0 格已不可选");
        assert_eq!(s.candidate_index(7, 10), Some(10));
    }

    #[test]
    fn cursor_movement_clamps_at_both_ends() {
        let mut s = session_with(25);
        for _ in 0..40 {
            s.next_candidate();
        }
        assert_eq!(s.highlighted, 24);
        for _ in 0..40 {
            s.previous_candidate();
        }
        assert_eq!(s.highlighted, 0);
        s.last_candidate();
        assert_eq!(s.highlighted, 24);
        s.first_candidate();
        assert_eq!(s.highlighted, 0);
        let mut empty = session_with(0);
        empty.next_candidate();
        empty.last_candidate();
        assert_eq!(empty.highlighted, 0);
    }

    #[test]
    fn page_turns_stop_at_the_edges() {
        let mut s = session_with(25); // 3 页
        s.next_page();
        s.next_page();
        s.next_page();
        assert_eq!(s.page(), 2);
        assert_eq!(s.highlighted, 20);
        s.previous_page();
        s.previous_page();
        s.previous_page();
        assert_eq!((s.page(), s.highlighted), (0, 0));
    }

    #[test]
    fn advance_walks_generation_and_refuses_to_wrap() {
        let mut s = session_with(1);
        assert_eq!(s.generation, 7);
        s.advance().expect("常规推进不该失败");
        assert_eq!(s.generation, 8);
        s.generation = u64::MAX;
        assert!(matches!(s.advance(), Err(SessionError::IdentityExhausted)));
        assert_eq!(s.generation, u64::MAX, "失败时不能留下回绕后的值");
    }

    #[test]
    fn refresh_resets_the_cursor_but_not_the_generation() {
        let mut s = session_with(25);
        s.last_candidate();
        s.refresh(vec![fake("x"), fake("y")]);
        assert_eq!(s.highlighted, 0);
        assert_eq!(
            s.generation, 7,
            "refresh 不负责推进代次，那是 advance() 的事"
        );
    }

    /// 阶段一最小闭环用的真词库与临时学习目录。
    ///
    /// 词库就是打进 APK 的那一份（`host/assets/dictionary/dict.tsv`），所以这条测的
    /// 与设备上是同一个文件；学习目录放系统临时目录，不污染仓库。
    fn real_paths() -> DataPaths {
        let dictionary = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../host/assets/dictionary/dict.tsv");
        let glossary = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../host/assets/dictionary/glossary-en.tsv");
        let user = std::env::temp_dir()
            .join("qingshan-engine-tests")
            .join("user.tsv");
        DataPaths {
            dict_path: dictionary,
            glossary_path: glossary,
            user_tsv: user,
        }
    }

    /// 落盘用例专用：`user.tsv` 与它的五张兄弟表（`user-words/-ngram/-choices/-english/-typos.tsv`）
    /// 的文件名是库里写死的常量（`qingjian-learning/src/frequency_learner/mod.rs:18-33`，
    /// 派生见 `tables.rs:183-189`），**所以隔离只能按目录分，不能只换文件名**。
    fn flush_paths(tag: &str) -> DataPaths {
        let mut paths = real_paths();
        let dir = std::env::temp_dir().join("qingshan-flush-tests").join(tag);
        let _ = std::fs::remove_dir_all(&dir);
        paths.user_tsv = dir.join("user.tsv");
        paths
    }

    /// 敲 `nihao` 并提交第 0 格，返回提交的那个词。
    ///
    /// 这里刻意成对调 `engine.commit` + `note_commit`：真机上这两个调用点在
    /// `lib.rs` 的 `run_transition()` 里成对出现，而那个函数要 JVM 才跑得动，
    /// 宿主测试只能测到这一层（接线由真机日志证）。
    fn commit_head(paths: &DataPaths) -> (Session, String) {
        let mut session = Session::new(create_engine(paths).expect("真词库 + 真译词表"), 91);
        for c in "nihao".chars() {
            session.engine.push(c);
        }
        session.requery();
        let text = session.candidates[0].text.clone();
        session.engine.commit(&session.candidates[0]);
        session.note_commit();
        (session, text)
    }

    fn user_file(paths: &DataPaths) -> bool {
        std::fs::metadata(&paths.user_tsv).is_ok()
    }

    /// 节流本身：刚提交完（远不到 60 秒）**不许**写盘——这是「不每次按键 fsync」的正证。
    #[test]
    fn commit_does_not_write_before_the_interval() {
        let paths = flush_paths("throttle");
        let (session, text) = commit_head(&paths);
        assert!(!text.is_empty());
        assert!(
            !user_file(&paths),
            "不到 60 秒就写了盘：节流没生效，用户每次上屏都在等磁盘"
        );
        drop(session);
    }

    /// 显式落盘（destroy 走的就是它）：文件真的出现，且内容里有刚提交的那个词。
    #[test]
    fn flush_at_destroy_writes_the_learned_word() {
        let paths = flush_paths("destroy");
        let (mut session, text) = commit_head(&paths);
        session.flush_at_destroy();
        let body = std::fs::read_to_string(&paths.user_tsv)
            .unwrap_or_else(|e| panic!("{} 没写出来：{e}", paths.user_tsv.display()));
        assert!(
            body.contains(&text),
            "落盘内容里没有 {text}，实际内容：{body}"
        );
    }

    /// 会话销毁这条路：`destroy_session` 必须把数据带走（覆盖「离开输入框 / 键盘销毁」）。
    #[test]
    fn destroy_session_flushes_learning() {
        let paths = flush_paths("session-destroy");
        let id = create_session(&paths, &SessionIdentity::default()).expect("建会话");
        with_session(id, |state| {
            for c in "nihao".chars() {
                state.engine.push(c);
            }
            state.requery();
            state.engine.commit(&state.candidates[0]);
            state.note_commit();
        })
        .expect("会话在");
        assert!(!user_file(&paths), "销毁前不该有文件");
        assert!(destroy_session(id), "destroy_session 该返回 true");
        assert!(
            user_file(&paths),
            "destroy 之后 {} 仍不存在：学习数据随进程丢了",
            paths.user_tsv.display()
        );
    }

    /// 反方向：没提交过就别凭空造文件（learner 自己不 dirty 就不写）。
    #[test]
    fn destroy_without_commit_creates_no_file() {
        let paths = flush_paths("empty-destroy");
        let id = create_session(&paths, &SessionIdentity::default()).expect("建会话");
        assert!(destroy_session(id));
        assert!(
            !user_file(&paths),
            "一次上屏都没有，却写出了 {}：落盘门失效",
            paths.user_tsv.display()
        );
    }

    /// 待办 33 的洞：私密框与「记忆新词」关从来没传给 Core，密码框上屏照样写进 `user.tsv`。
    /// 建会话时按 `learning_allowed` 上那两个开关，私密会话上屏后**一个字节都不该落盘**。
    #[test]
    fn a_private_session_writes_no_learning() {
        let paths = flush_paths("private-learning");
        let id = create_session(&paths, &SessionIdentity::private("com.example.password"))
            .expect("私密会话");
        let text = commit_in(id, "nihao");
        let weight =
            with_session(id, |state| state.engine.learner().weight(&text)).expect("会话在");
        assert_eq!(weight, 0, "私密会话把 {text} 记进了学习链：权重 {weight}");
        assert!(destroy_session(id));
        assert!(
            !user_file(&paths),
            "私密会话销毁后写出了 {}：密码框上屏的词进了用户词库",
            paths.user_tsv.display()
        );
    }

    /// 反方向的正样例：普通会话照旧学、照旧落盘——这条现在就该是绿的，
    /// 它锁的是「别把私密开关误开到所有会话上」。
    #[test]
    fn an_ordinary_session_still_writes_learning() {
        let paths = flush_paths("ordinary-learning");
        let id =
            create_session(&paths, &SessionIdentity::of("com.example.note")).expect("普通会话");
        let text = commit_in(id, "nihao");
        let weight =
            with_session(id, |state| state.engine.learner().weight(&text)).expect("会话在");
        assert!(
            weight > 0,
            "普通会话没记上 {text}：私密开关被误开到普通框上了"
        );
        assert!(destroy_session(id));
        assert!(
            user_file(&paths),
            "普通会话销毁后 {} 还不存在：学习数据随进程丢了",
            paths.user_tsv.display()
        );
    }

    /// 键盘重弹那条路：`onStartInputView` 不换会话，只调 `setPrivateSession`
    /// （`host/java/app/msime/android/core/MSIMEInputService.java:1384`），所以会话建好之后
    /// 才得知「这是密码框」，也必须当场停止写入。
    #[test]
    fn marking_private_silences_a_live_session() {
        let paths = flush_paths("live-private");
        let id = create_session(&paths, &SessionIdentity::default()).expect("建会话");
        with_session(id, |state| mark_private(state, true)).expect("会话在");
        commit_in(id, "nihao");
        assert!(destroy_session(id));
        assert!(
            !user_file(&paths),
            "标了私密还在写盘：{} 里有了这个框上屏的词",
            paths.user_tsv.display()
        );
    }

    /// 撤掉私密标记要能恢复写入，且私密期间**读**照常——已有历史仍参与本次排序
    /// （本轮的边界只做「不写」，不动「读」，理由见 `apply_learning_policy`）。
    #[test]
    fn lifting_private_restores_writes_and_keeps_reads() {
        let paths = flush_paths("live-open");
        let id = create_session(&paths, &SessionIdentity::of("com.example.chat")).expect("建会话");
        commit_in(id, "nihao");
        let learned =
            with_session(id, |state| state.engine.learner().weight("你好")).expect("会话在");
        assert!(learned > 0, "夹具一次都没记上，后面两条断言就没有意义");
        with_session(id, |state| mark_private(state, true)).expect("会话在");
        assert_eq!(
            with_session(id, |state| state.engine.learner().weight("你好")).expect("会话在"),
            learned,
            "私密期间读被改掉了：排序本该照旧用已有历史"
        );
        with_session(id, |state| mark_private(state, false)).expect("会话在");
        commit_in(id, "nihao");
        assert!(destroy_session(id));
        assert!(
            user_file(&paths),
            "撤了私密标记之后还是没落盘：写开关关了就再也没开回来"
        );
    }

    /// 在会话表里的那个会话上敲 `letters` 并提交第 0 格，返回提交的词。
    /// 与 `commit_head` 同形，只是走 `create_session`/`with_session`，因为挂起逻辑挂在
    /// 这两个函数上（`commit_head` 直接构造 `Session`，绕开了会话表，测不到跨会话）。
    fn commit_in(id: u64, letters: &str) -> String {
        with_session(id, |state| {
            for c in letters.chars() {
                state.engine.push(c);
            }
            state.requery();
            let text = state.candidates[0].text.clone();
            state.engine.commit(&state.candidates[0]);
            state.note_commit();
            text
        })
        .expect("会话在")
    }

    /// 读会话的「本会话最近上屏前文」。`EngineSession::history`（`engine/session.rs:48`）    /// 与 `recent_commits`（`:21`）是同一次 `swap_session` 换进换出的两样
    /// （`:63` 与 `:81`），而 `history()` 是 Core 公开可读的（`engine/setup.rs:352`），
    /// 所以它是「挂起的上下文真被换回来了」的可见证据——账本本身没有 getter。
    fn history_of(id: u64) -> String {
        with_session(id, |state| state.engine.history().text().to_owned()).expect("会话在")
    }

    /// 挂起表里现在有几份上下文。只给测试用——生产路径不需要知道容量，
    /// 但「不无界增长」这条得有个地方能断言。
    fn parked_count() -> usize {
        parked().lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// 挂起表是**进程级 static**，下面六个用例共用一份，而 `cargo test` 默认多线程跑 ⇒
    /// 断言条数的用例会被别的用例挂进去的条目污染（实测：单独跑过、并发跑红）。
    /// 所以这几个用例先串到一把锁上，再把表清空——按包名取的断言本来互不相干，
    /// 但「上限」「不该收无身份会话」这两条看的是整张表。
    fn park_serial() -> std::sync::MutexGuard<'static, ()> {
        static SERIAL: OnceLock<Mutex<()>> = OnceLock::new();
        let guard = SERIAL
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        parked().lock().unwrap_or_else(|e| e.into_inner()).clear();
        guard
    }

    /// 甲-2 的容量锁：挂起表有上限，到顶淘汰最久没再挂的那份。
    #[test]
    fn parked_cache_is_bounded_and_evicts_the_stalest() {
        let _serial = park_serial();
        let paths = flush_paths("park-bound");
        for index in 0..=PARKED_SESSIONS {
            let identity = SessionIdentity::of(&format!("com.example.app{index}"));
            let id = create_session(&paths, &identity).expect("建会话");
            commit_in(id, "nihao");
            assert!(destroy_session(id));
            assert!(
                parked_count() <= PARKED_SESSIONS,
                "挂起到第 {} 份还没封顶：现在 {}",
                index + 1,
                parked_count()
            );
        }

        let oldest =
            create_session(&paths, &SessionIdentity::of("com.example.app0")).expect("最老的回来");
        let history = history_of(oldest);
        assert!(
            history.is_empty(),
            "最久没再用那份本该被淘汰，却还带回了上下文：history={history:?}"
        );
        destroy_session(oldest);

        let newest = create_session(
            &paths,
            &SessionIdentity::of(&format!("com.example.app{PARKED_SESSIONS}")),
        )
        .expect("最新那个回来");
        let history = history_of(newest);
        assert!(
            history.contains("你好"),
            "淘汰挑错了人：最新挂起的那份反而没了，history={history:?}"
        );
        destroy_session(newest);
    }

    /// 空包名不能当缓存键：`packageName` 拿不到时 Java 那边是空串
    /// （`MSIMEInputService.java:446` 的字段初值与 `:1259` 的三元），
    /// 如果让空串进挂起表，所有无身份会话就并成一份上下文——正是要防的跨框污染。
    #[test]
    fn empty_package_name_is_never_a_cache_key() {
        let _serial = park_serial();
        let paths = flush_paths("park-empty-package");
        let unnamed = || SessionIdentity {
            application: Some(String::new()),
            learning_allowed: true,
        };
        let first = create_session(&paths, &unnamed()).expect("无包名会话 A");
        commit_in(first, "nihao");
        assert!(destroy_session(first));
        assert_eq!(parked_count(), 0, "空包名被挂起了：挂起表不该收无身份会话");

        let second = create_session(&paths, &unnamed()).expect("无包名会话 B");
        let history = history_of(second);
        assert!(
            history.is_empty(),
            "两个无身份会话串成了一份上下文：history={history:?}"
        );
        destroy_session(second);
    }

    /// 甲-2 主功能：同一个包名离开再回来，上次上屏的文本还在会话里。
    #[test]
    fn same_app_gets_its_parked_context_back() {
        let _serial = park_serial();
        let paths = flush_paths("park-same-app");
        let identity = SessionIdentity::of("com.example.chat");
        let first = create_session(&paths, &identity).expect("建会话");
        let word = commit_in(first, "nihao");
        assert!(destroy_session(first), "第一次 destroy 该删掉会话");

        let second = create_session(&paths, &identity).expect("同包名再建");
        let history = history_of(second);
        assert!(
            history.contains(&word),
            "同包名回来带不回上次上屏的「{word}」：挂起没生效（history={history:?}）"
        );
        destroy_session(second);
    }

    /// 甲-2 的副作用锁：挂起的是「账本与学习链」，不是半截拼音。
    /// 离开输入框时水杉已经把组合文本从文档里 abandon 掉了
    /// （`MSIMEInputService.java:1602-1603` 的 `bridge.abandon(...)`），
    /// 回来却看见 `nihao` 还挂在组字区 = 凭空多回来一段用户已经放弃的输入。
    #[test]
    fn parked_context_comes_back_without_the_abandoned_composition() {
        let _serial = park_serial();
        let paths = flush_paths("park-composition");
        let identity = SessionIdentity::of("com.example.note");
        let first = create_session(&paths, &identity).expect("建会话");
        with_session(first, |state| {
            for c in "nihao".chars() {
                state.engine.push(c);
            }
            state.requery();
        })
        .expect("会话在");
        assert_eq!(
            with_session(first, |state| state.editing()).expect("会话在"),
            "nihao",
            "夹具本身没建立起组句，后面的断言就没有意义"
        );
        assert!(destroy_session(first));

        let second = create_session(&paths, &identity).expect("同包名再建");
        let (editing, candidates) =
            with_session(second, |state| (state.editing(), state.candidates.len()))
                .expect("会话在");
        assert_eq!(editing, "", "半截拼音跟着挂起的上下文回来了：{editing:?}");
        assert_eq!(candidates, 0, "没有组句却带着 {} 个候选", candidates);
        destroy_session(second);
    }

    /// 甲-2 的隐私锁：不许学习的输入框既不继承挂起的上下文，也不往外挂。
    /// 这种框水杉照样建会话，只是把建会话选项里的 `preferences.learning` 置 false
    /// （`MSIMEInputService.java:1298`，条件 `learningSuppressed()` = 框不许个性化或隐私模式开着，
    /// `:1905-1907`），所以上屏文本仍会进 `history` 与 `recent_commits`。
    /// 不拦住就等于把「不把私密文本带到下一次普通输入」（Core `engine/session.rs:90`）
    /// 这条自己定下的边界漏掉。
    #[test]
    fn private_field_neither_restores_nor_parks() {
        let _serial = park_serial();
        let paths = flush_paths("park-private");
        let ordinary = SessionIdentity::of("com.example.mail");
        let first = create_session(&paths, &ordinary).expect("普通会话");
        let word = commit_in(first, "nihao");
        assert!(destroy_session(first), "普通框销毁该挂起");

        let private = create_session(&paths, &SessionIdentity::private("com.example.mail"))
            .expect("私密会话");
        let inherited = history_of(private);
        assert!(
            !inherited.contains(&word),
            "不许学习的框继承了同包名的上屏文本「{word}」：history={inherited:?}"
        );
        let secret = commit_in(private, "women");
        assert!(destroy_session(private));

        let back = create_session(&paths, &ordinary).expect("普通会话回来");
        let history = history_of(back);
        assert!(
            !history.contains(&secret) && !history.contains(&word),
            "私密框进出之后普通框还看得见上屏文本：history={history:?}（该是空的）"
        );
        destroy_session(back);
    }

    /// 甲-2 的键锁：缓存键真的是包名。单槽启发式（甲-1）这条必红——
    /// 它会把 B 刚挂起的那份上下文装进 A 的新会话，于是 A 看不见自己的「你好」、
    /// 反倒看见 B 的「我们」。
    #[test]
    fn different_apps_do_not_share_parked_contexts() {
        let _serial = park_serial();
        let paths = flush_paths("park-two-apps");
        let a = create_session(&paths, &SessionIdentity::of("com.example.a")).expect("建 A");
        let word_a = commit_in(a, "nihao");
        assert!(destroy_session(a));
        let b = create_session(&paths, &SessionIdentity::of("com.example.b")).expect("建 B");
        let word_b = commit_in(b, "women");
        assert!(destroy_session(b));
        assert_ne!(word_a, word_b, "两条输入撞车了，这个测试就分不出包名了");

        let again_a =
            create_session(&paths, &SessionIdentity::of("com.example.a")).expect("A 回来");
        let history_a = history_of(again_a);
        assert!(
            history_a.contains(&word_a) && !history_a.contains(&word_b),
            "A 回来该只看见自己的「{word_a}」，实际 history={history_a:?}"
        );
        destroy_session(again_a);

        let again_b =
            create_session(&paths, &SessionIdentity::of("com.example.b")).expect("B 回来");
        let history_b = history_of(again_b);
        assert!(
            history_b.contains(&word_b) && !history_b.contains(&word_a),
            "B 回来该只看见自己的「{word_b}」，实际 history={history_b:?}"
        );
        destroy_session(again_b);
    }

    /// Core 级：加载真词库 → 敲 `nihao` → 候选里有「你好」→ commit 原样吐回。
    ///
    /// 顺带量出加载耗时。**这是 Windows x64 host 上的数字，不等于 arm64 真机**——
    /// PIN.md「上游性能与体积事实」第 1 条讲的正是别把这类数字搬来搬去。
    #[test]
    fn real_dictionary_nihao_round_trip() {
        let paths = real_paths();
        let size = std::fs::metadata(&paths.dict_path)
            .unwrap_or_else(|error| panic!("词库读不到（{}）：{error}", paths.dict_path.display()))
            .len();
        let started = std::time::Instant::now();
        let dictionary = Dictionary::from_path(&paths.dict_path).expect("真词库应当可解析");
        println!(
            "实测 Dictionary::from_path: {} B 用时 {:?}（host x64，非设备）",
            size,
            started.elapsed()
        );
        assert_eq!(size, 2_053_612, "词库字节数须与 3.4 立约时实测一致");

        let mut engine = Engine::new(dictionary);
        for c in "nihao".chars() {
            engine.push(c);
        }
        let started = std::time::Instant::now();
        let query = engine.query().expect("查询不应失败");
        println!(
            "实测 engine.query(nihao): {:?}，候选 {} 个",
            started.elapsed(),
            query.candidates.items.len()
        );
        let texts: Vec<&str> = query
            .candidates
            .items
            .iter()
            .map(|candidate| candidate.text.as_str())
            .collect();
        assert!(
            texts.contains(&"你好"),
            "候选里没有「你好」，前 10 个：{texts:?}"
        );

        let target = query
            .candidates
            .items
            .iter()
            .find(|candidate| candidate.text == "你好")
            .expect("刚断言过存在");
        assert_eq!(engine.commit(target), "你好");
        assert!(engine.composition().is_empty(), "上屏后组字缓冲区应清空");
    }

    /// 适配层级：真会话表 + 真游标 + 命令 1（空格选高亮）+ 过期代次拒绝 + 销毁。
    #[test]
    fn session_table_space_command_commits() {
        let paths = real_paths();
        let id = create_session(&paths, &SessionIdentity::default()).expect("真词库应能建会话");
        assert!(with_session(id, |_| ()).is_some(), "会话应当可查");

        let candidate_count = with_session(id, |state| {
            for c in "nihao".chars() {
                state.engine.push(c);
            }
            state.requery();
            state.candidates.len()
        });
        assert!(candidate_count.is_some_and(|n| n > 0), "候选非空");

        let (texts, generation) = with_session(id, |state| {
            (
                state
                    .candidates
                    .iter()
                    .map(|candidate| candidate.text.clone())
                    .collect::<Vec<String>>(),
                state.generation,
            )
        })
        .expect("会话在");
        assert!(texts.iter().any(|text| text == "你好"), "候选: {texts:?}");
        assert_eq!(generation, id, "首个代次取 session id，二者同源");

        // Java 不校验 generation（MSIMEInputService.java:6259-6260 只比 session），
        // 所以过期必须我们拦。外层 Option 是「会话在否」，内层是「下标可用否」。
        assert_eq!(
            with_session(id, |state| state.candidate_index(generation ^ 1, 0)),
            Some(None),
            "旧代次应被拒"
        );
        assert_eq!(
            with_session(id, |state| state.candidate_index(generation, 0)),
            Some(Some(0)),
            "当前页第 0 格应可用"
        );

        let outcome = with_session(id, |state| {
            let _ = state.advance();
            apply_command(state, 1)
        });
        match outcome {
            Some(CommandOutcome::Commit(text)) => {
                assert_eq!(text, "你好", "命令 1 应上屏高亮的那个候选");
            }
            other => panic!("命令 1 没有产出上屏：{other:?}"),
        }

        assert!(destroy_session(id));
        assert!(!destroy_session(id), "重复销毁应返回 false");
        assert!(with_session(id, |_| ()).is_none(), "销毁后查不到会话");
    }

    fn sample_session() -> Session {
        Session::new(
            Engine::new(Dictionary::parse(SAMPLE).expect("夹具词库应当可解析")),
            7,
        )
    }

    /// 读不出拼音不是故障：空组字给 `ParseError::Empty`，实测按 `i` / `im` 给
    /// `NoSegmentation`，两者都应当落成「空候选的一页」而不是让导出回 `ok:false`。
    #[test]
    fn unreadable_input_yields_an_empty_page() {
        let mut session = sample_session();
        session.requery();
        assert!(session.candidates.is_empty(), "空组字应得空候选");
        assert_eq!(session.page_bounds(), (0, 0), "空表窗口是 (0, 0)");
        assert_eq!(session.page_count(), 0);

        for c in "im".chars() {
            session.engine.push(c);
        }
        session.requery();
        assert!(
            session.page_bounds().1 <= session.candidates.len(),
            "窗口不得越出候选表"
        );
        assert_eq!(session.highlighted, 0, "refresh 会把游标归零");
    }

    /// `setEnglishModeRaw` 的会话侧契约：涨一代、模式翻转、View 跟着翻转、仍是 32 个 key。
    #[test]
    fn set_english_mode_advances_generation_and_flips_the_view() {
        let mut session = sample_session();
        for c in "ni".chars() {
            session.engine.push(c);
        }
        session.requery();

        let before = session.generation;
        let _ = session.advance();
        session.engine.set_english_mode(true);
        session.requery();
        assert_eq!(session.generation, before + 1, "一次切换只涨一代");
        assert!(session.english_mode(), "Core 的开关已翻过去");
        assert!(
            crate::build_view(&session, 7).dedicated_english,
            "View.dedicated_english 跟着翻转"
        );

        session.engine.set_english_mode(false);
        session.requery();
        let view = crate::build_view(&session, 7);
        assert!(!view.dedicated_english, "切回中文模式");
        let keys = serde_json::to_value(view).expect("View 应可序列化");
        assert_eq!(
            keys.as_object().expect("View 序列化出来是对象").len(),
            crate::view::VIEW_FIELD_COUNT,
            "仍是 32 个 key"
        );
    }

    /// B1 的四个止血导出共用 `wire::no_op_view`，而导出层要 JVM 才跑得动，所以测共用体。
    /// 断言对着宿主 `apply()` 的三条硬读取去（`MSIMEInputService.java:2391` 取 `view`、
    /// `:2407` 取 `editing_text`、`:2424` 的 `strictBoolean`），再加「不改变状态」这条回归锁。
    #[test]
    fn benign_no_ops_return_a_parseable_view_and_change_nothing() {
        let mut session = sample_session();
        for c in "nihao".chars() {
            session.engine.push(c);
        }
        session.requery();

        let before_generation = session.generation;
        let before = crate::build_view(&session, 7);
        let body = crate::wire::no_op_body(&before);

        assert!(
            body["view"].is_object(),
            "apply() 在 :2391 用 getJSONObject(\"view\") 硬取，缺它就是 JSONException → fail()"
        );
        assert_eq!(
            body["view"].as_object().expect("view 是对象").len(),
            crate::view::VIEW_FIELD_COUNT,
            "必须回完整 View；回个骨架会让 :2407 的 getString 抛"
        );
        assert!(
            body["view"]["editing_text"].is_string(),
            ":2407 对 editing_text 用 getString，必须是字符串"
        );
        assert_eq!(
            body["handled"],
            serde_json::Value::Bool(false),
            "handled 必须是真 JSON boolean（:2424 的 strictBoolean），且为 false"
        );

        let after = crate::build_view(&session, 7);
        assert_eq!(
            serde_json::to_value(&after).expect("View 应可序列化"),
            serde_json::to_value(&before).expect("View 应可序列化"),
            "空操作前后 View 必须逐字段相同"
        );
        assert_eq!(session.generation, before_generation, "空操作不许推进代次");
    }

    /// B2：`remove_candidate` 的三条分支。导出层要 JVM，所以直接驱动 `crate::remove_candidate`；
    /// `transition` 的 advance/requery 编排不在这里验（它由 `selectRaw` 那条同形路径覆盖）。
    #[test]
    fn remove_candidate_drops_a_forgettable_candidate_and_declines_the_rest() {
        // 夹具会话要挂真 learner：默认的 `Learner::record_choice` 是空实现（learner.rs:16），
        // 不挂 FrequencyLearner 的话下面那条前置断言会红。
        let mut session = Session::new(
            Engine::new(Dictionary::parse(SAMPLE).expect("夹具词库应当可解析"))
                .with_learner(Box::new(qingjian_learning::FrequencyLearner::default())),
            7,
        );
        for c in "nihao".chars() {
            session.engine.push(c);
        }
        session.requery();
        let generation = session.generation;
        assert!(!session.candidates.is_empty(), "夹具里 nihao 应当有候选");

        // 分支一：没学过的词库词 ⇒ 没什么可删 ⇒ Passthrough（不是假装删了）。
        assert!(matches!(
            crate::remove_candidate(&mut session, generation, 0),
            crate::keys::CommandOutcome::Passthrough
        ));

        // 分支二：记过一次选择 ⇒ forget 真清掉了东西 ⇒ Consumed。
        let first = session.candidates[0].clone();
        let input = first.syllables.join("");
        session
            .engine
            .learner_mut()
            .record_choice(&input, &first.text);
        assert_eq!(
            session.engine.learner().choice_weight(&input, &first.text),
            1,
            "前置条件：learner 得真的记下这次选择，否则这条测试什么都没验"
        );
        assert!(matches!(
            crate::remove_candidate(&mut session, generation, 0),
            crate::keys::CommandOutcome::Consumed
        ));
        assert_eq!(
            session.engine.learner().choice_weight(&input, &first.text),
            0,
            "forget 之后那份按输入串记的选择要没了"
        );

        // 分支三：代次对不上、下标越出当页窗口，一律 Passthrough。
        // 页窗口口径见上游 runtime.rs:1796-1811（RemoveCandidate 与 Select 同组）。
        assert!(matches!(
            crate::remove_candidate(&mut session, generation + 1, 0),
            crate::keys::CommandOutcome::Passthrough
        ));
        let past_end = session.candidates.len();
        assert!(matches!(
            crate::remove_candidate(&mut session, generation, past_end),
            crate::keys::CommandOutcome::Passthrough
        ));
    }

    /// A：展开面板要的是全表，且**不能推进代次**——`sameCandidateVersion` 比的就是它。
    #[test]
    fn all_candidates_are_the_whole_table_and_do_not_bump_the_generation() {
        let paths = real_paths();
        let dictionary = Dictionary::from_path(&paths.dict_path).expect("真词库");
        let mut session = Session::new(Engine::new(dictionary), 7);
        for c in "nihao".chars() {
            session.engine.push(c);
        }
        session.requery();
        let snap = crate::snapshot_of(&session, 7);
        let all = crate::view::all_candidates(&snap);
        let page = crate::view::page_candidates(&snap);

        assert_eq!(all.len(), session.candidates.len(), "全表应答要覆盖整张表");
        assert!(
            page.len() < all.len(),
            "当页必须比全表少，否则宿主 `:6340` 的 entries<=visible 判据会拒绝开面板"
        );
        for (index, row) in all.iter().enumerate() {
            assert_eq!(row.id.index, index, "id.index 必须是整表绝对下标");
            assert_eq!(row.id.session, 7);
            assert_eq!(row.id.generation, session.generation);
        }
        // 抽出 candidate_row 之后，两条路径在重叠窗口上必须逐字段相同（9 个字段全比）。
        for (offset, row) in page.iter().enumerate() {
            assert_eq!(
                serde_json::to_value(row).expect("候选可序列化"),
                serde_json::to_value(&all[offset]).expect("候选可序列化"),
                "当页第 {offset} 行与全表的对应行不一致，说明抽函数改过形状"
            );
        }

        let before = session.generation;
        let body =
            serde_json::to_value(crate::view::all_candidates_view(&snap)).expect("应答可序列化");
        assert_eq!(session.generation, before, "拼应答不许推进代次");
        assert_eq!(
            body["generation"].as_u64().expect("generation 是整数"),
            before,
            "应答里的 generation 要等于会话当前代次，宿主拿它跟手上那份 View 比"
        );
        assert_eq!(body["session"].as_u64(), Some(7));
        assert!(
            body["preedit"]
                .as_str()
                .is_some_and(|text| !text.is_empty()),
            "preedit 为空的话面板顶部的组词文案会消失（reading 我们恒空）"
        );
    }

    /// A：`selectAnyCandidate` 的边界是全表；页窗口版（`selectRaw` 用的那个）一行没动。
    #[test]
    fn select_any_reaches_beyond_the_page_window_and_rejects_only_out_of_table() {
        let paths = real_paths();
        let dictionary = Dictionary::from_path(&paths.dict_path).expect("真词库");
        let mut session = Session::new(Engine::new(dictionary), 7);
        for c in "nihao".chars() {
            session.engine.push(c);
        }
        session.requery();
        let generation = session.generation;
        let (start, end) = session.page_bounds();
        let past = session.candidates.len();
        assert!(
            end < past,
            "这条测试要一个还有下一页的表，实测页尾 {end} 表长 {past}"
        );

        // 页窗口内：两个口径都给过。
        assert_eq!(session.candidate_index(generation, start), Some(start));
        assert_eq!(
            session.candidate_index_in_table(generation, start),
            Some(start)
        );
        // 页窗口外、表内：只有全表口径放行——面板要的就是这个。
        assert_eq!(session.candidate_index(generation, end), None);
        assert_eq!(session.candidate_index_in_table(generation, end), Some(end));
        // 越出表长、代次不符：两个口径都拒。
        assert_eq!(session.candidate_index_in_table(generation, past), None);
        assert_eq!(
            session.candidate_index_in_table(generation + 1, start),
            None
        );
        // 拒只是回 Passthrough，读取本身不动会话。
        assert_eq!(session.candidates.len(), past);
        assert_eq!(session.generation, generation);
    }

    /// 真词库 + `nihao` 的会话，三条代次时序测试共用。
    fn nihao_session() -> Session {
        let paths = real_paths();
        let dictionary = Dictionary::from_path(&paths.dict_path).expect("真词库");
        let mut session = Session::new(Engine::new(dictionary), 7);
        for c in "nihao".chars() {
            session.engine.push(c);
        }
        session.requery();
        session
    }

    /// 带译词表的会话：走 `create_engine`，`annotate` 才查得到东西。
    ///
    /// 刻意不与 `nihao_session()` 合并：那 3 个调用点都是代次回归锁，不该为译词表
    /// 多付词库 ~50 ms + 译词表 ~62 ms，也不该让译词数据的变动去影响它们的结论。
    fn nihao_session_with_glossary() -> Session {
        let paths = real_paths();
        let mut session = Session::new(create_engine(&paths).expect("真词库 + 真译词表"), 7);
        for c in "nihao".chars() {
            session.engine.push(c);
        }
        session.requery();
        session
    }

    /// 第 7 项正向锁：`requery()` 里的 `annotate` 真的把译词填进了 View。
    /// 断言「至少一条」而不是「每条」——80k 子集对 `nihao` 的实测命中率是 30/44。
    #[test]
    fn translation_shows_up_after_requery() {
        let session = nihao_session_with_glossary();
        let view = crate::build_view(&session, 7);
        assert!(
            view.candidates.iter().any(|row| row.translation.is_some()),
            "真译词表已接进 Engine，首屏候选里至少要有一条译词"
        );
    }

    /// 反方向：不许有人把「命中」写成「全有」。这条同时守住建表口径——
    /// 哪天有人拿全量 23.2 万行替了 80k 子集，这里会先响。
    #[test]
    fn translation_is_not_on_every_candidate() {
        let session = nihao_session_with_glossary();
        let view = crate::build_view(&session, 7);
        assert!(
            view.candidates.iter().any(|row| row.translation.is_none()),
            "80k 子集不是全量表，必有候选查不到译词"
        );
    }

    /// 代次 bug 的回归锁。原先 `transition` 在动作**之前**无条件 `advance()`，
    /// 于是宿主拿自己那份 View 的 `generation` 来选候选，永远被判成过期。
    #[test]
    fn passthrough_does_not_bump_the_generation() {
        let mut session = nihao_session();
        let generation = session.generation;
        let past = session.candidates.len();
        let body = crate::run_transition(&mut session, 7, move |state| {
            match state.candidate_index_in_table(generation, past) {
                Some(_) => crate::keys::CommandOutcome::Consumed,
                None => crate::keys::CommandOutcome::Passthrough,
            }
        });
        assert!(!body.handled, "越出表长的下标必须判没处理");
        assert_eq!(
            session.generation, generation,
            "没处理就不许涨代次——原先恒 +1，正是这个 bug 的形状"
        );
        assert_eq!(body.commit, None, "没处理就没有要上屏的文本");
    }

    /// bug 的正面对照：宿主看到的代次 + 页窗口内的下标，现在必须真的上屏。
    #[test]
    fn select_with_the_generation_the_host_saw_commits() {
        let mut session = nihao_session();
        let seen = crate::build_view(&session, 7).candidates[1].id.generation;
        let expected = session.candidates[1].text.clone();
        let body = crate::run_transition(&mut session, 7, move |state| {
            match state.candidate_index(seen, 1) {
                Some(index) => crate::keys::CommandOutcome::Commit(
                    state.engine.commit(&state.candidates[index]),
                ),
                None => crate::keys::CommandOutcome::Passthrough,
            }
        });
        assert!(body.handled, "宿主看到的代次必须能用，否则点候选永远不上屏");
        assert_eq!(
            body.commit.as_deref(),
            Some(expected.as_str()),
            "上屏的必须正是那一个候选"
        );
        assert_eq!(session.generation, seen + 1, "处理过才涨代次，且只涨一代");
    }

    /// 「多涨可接受」写进测试：动作没碰引擎但回 `Consumed`（`focusRaw` 就是这种），
    /// 照样涨号。将来若有人想用「长度 + 首项」的启发式省掉这次涨号，这条会红。
    #[test]
    fn consumed_bumps_even_when_the_candidate_list_is_unchanged() {
        let mut session = nihao_session();
        let before = session.generation;
        let list_len = session.candidates.len();
        let body =
            crate::run_transition(&mut session, 7, |_| crate::keys::CommandOutcome::Consumed);
        assert!(body.handled);
        assert_eq!(
            session.generation,
            before + 1,
            "Consumed 就涨号，多涨是安全侧"
        );
        assert_eq!(session.candidates.len(), list_len, "而候选表本身没变");
    }

    /// 待办 28：宿主的「中文标点」开关要真的落进引擎。
    /// **判别器**：把 `set_chinese_punctuation` 的函数体删掉，第一条断言当场就红
    /// （引擎出厂是开，所以先断"关掉不转"再断"开着才转"）。
    #[test]
    fn chinese_punctuation_switch_reaches_the_engine() {
        let mut session = session_with(0);
        session.set_chinese_punctuation(false);
        assert_eq!(
            session.engine.punctuate(','),
            None,
            "关掉之后逗号原样，不转全角"
        );
        session.set_chinese_punctuation(true);
        assert_eq!(
            session.engine.punctuate(','),
            Some("，"),
            "开着才转成全角逗号"
        );
    }
}
