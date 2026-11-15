//! **主链热槽的宿主侧服务**（2026-11-16 热槽记忆 M2–M5）。
//!
//! # 一句话
//!
//! 纯函数在 `live2d_ai_runtime::hotmem`；本模块负责**把它们接进主链**：
//! 每轮说话前组装「系统句 + 钉住块 + 热卡」（[HotSlot::compose_system_text]），
//! 草稿来时换热卡（[HotSlot::retrieve]），一轮结束后记事实（[HotSlot::record_turn]）。
//!
//! # 边界（**不要越界**）
//!
//! - **不写 `live2d-ai.toml`**：四块只进这一轮请求，不落盘、不写会话槽；
//! - **不打 marker**：请求体里不会出现 `<!-- live2d-ai:memory-v0` 那种串；
//! - **不阻塞说话**：记是后台任务，说的 HTTP 响应不等它；查最多等
//!   [RETRIEVE_WAIT_MS] 毫秒，超时按空热卡开口；
//! - **不碰 `desk_pet/` / 工厂表 / `--desk-pet`**；
//! - **不读旧 Mod 的 `memory.jsonl`**：那是另一条已退休路径的文件。
//!
//! # 时钟
//!
//! 事实的年龄靠「现在几小时」算。生产用 [clock_hours]（Unix 纪元小时），
//! 测试直接 `advance_to`。不引 chrono，只用 `SystemTime`。
//!
//! # 线程
//!
//! [HotSlot] 是 `Arc<Mutex<..>>` 的句柄：supervisor 每轮读、HTTP 线程写、
//! 后台任务写。锁只包住内存操作（没有 IO 在锁里），与 `SessionScopeStore` 同一条
//! 纪律：**锁中毒不 panic**，拿回内部数据继续（一个后台任务崩了不该让产品不能说话）。

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use live2d_ai_runtime::hotmem::{
    CardLine, HotCard, HotMemory, HotMemorySnapshot, PinnedItem, Recollection, compose_system_text,
    render_pinned, system_sentence,
};

/// 「查」的最长等待（毫秒）。超时按空热卡开口，不追改已经送去 TTS 的句子。
pub const RETRIEVE_WAIT_MS: u64 = 1_500;

/// 「记」的抽取结果上限（条）。一次一轮不该抽出几十条事实。
pub const MAX_RECALLED_PER_TURN: usize = 8;

/// 抽取失败时兜底 episode 的截断长度（字符）。
pub const FALLBACK_EPISODE_CHARS: usize = 200;

/// 现在几小时（Unix 纪元起算；失败回 0，**不 panic**）。
pub fn clock_hours() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64() / 3600.0)
        .unwrap_or(0.0)
}

/// 一次说话请求的四块（**只用于这一轮**，不落盘）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpokenBlocks {
    /// 系统句（用户存的非空句，或出厂句）。
    pub sentence: String,
    /// 钉住块正文（空 = 没有）。
    pub pinned: String,
    /// 热卡渲染后的正文（空 = 没有）。
    pub hot_card: String,
    /// 拼好的 system 文本。
    pub system: String,
}

/// 热槽服务句柄（Clone 出的是同一份状态的句柄）。
#[derive(Clone)]
pub struct HotSlot {
    inner: Arc<Mutex<SlotInner>>,
}

struct SlotInner {
    mem: HotMemory,
    /// 空热卡口径：`true` = 查还没就绪 / 失败时**按空热卡开口**。
    ///
    /// 这不是开关，而是产品语义的落点：写在这里让「空热卡」有一个可断言的
    /// 位置，而不是散落在调用点的 `unwrap_or_default()`。
    empty_on_miss: bool,
}

impl Default for HotSlot {
    fn default() -> Self {
        Self::new()
    }
}

impl HotSlot {
    /// 空热槽（时钟 = 现在）。
    pub fn new() -> Self {
        let mut mem = HotMemory::new();
        mem.advance_to(clock_hours());
        Self {
            inner: Arc::new(Mutex::new(SlotInner {
                mem,
                empty_on_miss: true,
            })),
        }
    }

    /// 取锁；**中毒不 panic**（见模块头注「线程」）。
    fn guard(&self) -> MutexGuard<'_, SlotInner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 把时钟推到「现在」（每轮开始前调一次；只增不减）。
    pub fn tick(&self) {
        self.guard().mem.advance_to(clock_hours());
    }

    /// **说**：组装这一轮的四块。
    ///
    /// - `user_prompt`：`[persona].system_prompt` 原文（空 → 出厂句）；
    /// - `session`：本轮会话（钉住块的复读抑制按会话记）；
    /// - 热卡取该会话**当前**那一张（草稿没来 / 查失败 = 空卡）。
    pub fn compose(&self, user_prompt: &str, session: Option<&str>) -> SpokenBlocks {
        let mut inner = self.guard();
        inner.mem.advance_to(clock_hours());
        let sentence = system_sentence(user_prompt).to_string();
        let items = inner.mem.pinned_items();
        let spoken = inner.mem.spoken(session).to_vec();
        let pinned = render_pinned(&items, &spoken);
        let card = inner.mem.card(session);
        let hot_card = live2d_ai_runtime::hotmem::render_hot_card(card);
        let system = compose_system_text(&sentence, &pinned, card.filter(|c| !c.is_empty()));
        SpokenBlocks {
            sentence,
            pinned,
            hot_card,
            system,
        }
    }

    /// 本轮钉住块的条目 id（**本轮说完之后**记为「已说」，下一轮跳过）。
    pub fn pinned_ids_now(&self, session: Option<&str>) -> Vec<u64> {
        let inner = self.guard();
        let spoken = inner.mem.spoken(session);
        inner
            .mem
            .pinned_items()
            .into_iter()
            .filter(|item| !spoken.contains(&item.id))
            .map(|item| item.id)
            .collect()
    }

    /// 记下「这些钉住条目上一轮说过了」（下一轮渲染跳过；**条目仍在库里**）。
    pub fn note_spoken(&self, session: Option<&str>, ids: Vec<u64>) {
        self.guard().mem.note_spoken(session, ids);
    }

    /// **查**：把草稿的命中整块写成本轮热卡（**替换**，不是追加）。
    ///
    /// 最小实现（M3）：候选 = 库里的钉住 + 未过期事实，按保留率分档、按 200 token
    /// 裁剪，与最近几轮原文重复的行丢掉。**没有向量**——向量检索在 M3 之后接
    /// embedding trait；这里先给出「无向量时热卡为空 / 有事实时按保留率成卡」的
    /// 确定行为，测试据此断言。
    ///
    /// 返回写好的热卡（空卡 = 清掉了该会话的热卡）。
    pub fn retrieve(&self, session: Option<&str>, draft: &str, recent: &[String]) -> HotCard {
        let mut inner = self.guard();
        inner.mem.advance_to(clock_hours());
        let facts = inner.mem.candidates();
        let draft = draft.trim();
        // 草稿为空 = 没有检索依据 → 空热卡（**不拿旧卡顶着**）。
        let card = if draft.is_empty() {
            HotCard::empty()
        } else {
            let mut candidates: Vec<CardLine> = facts
                .iter()
                .filter_map(|f| f.tier().map(|t| CardLine::new(t, f.text.clone())))
                .collect();
            candidates.sort_by_key(|line| line.tier);
            HotCard::from_lines(candidates, recent)
        };
        inner.mem.set_card(session, card.clone());
        card
    }

    /// 丢掉某会话的热卡（草稿话题变了 / 含 `[资料]` 的卡整块作废）。
    pub fn clear_card(&self, session: Option<&str>) -> bool {
        self.guard().mem.clear_card(session)
    }

    /// 用给定的行**整块替换**某会话的热卡（M5 外挂资料走这条：资料只进 `[资料]` 行）。
    pub fn set_card(&self, session: Option<&str>, card: HotCard) {
        self.guard().mem.set_card(session, card);
    }

    /// **记**：把一轮抽取出来的事实写进库（加固逐字同文本的旧条目）。
    ///
    /// 返回真正写入 / 加固的条数。
    pub fn record(&self, items: Vec<Recollection>) -> usize {
        let mut inner = self.guard();
        inner.mem.advance_to(clock_hours());
        let mut written = 0usize;
        for item in items.into_iter().take(MAX_RECALLED_PER_TURN) {
            if item.text.trim().is_empty() {
                continue;
            }
            // 「用户原文复述」这条加固判据：逐字相同 → 加固，不新增。
            if let Some(id) = inner.mem.exact_match(&item.text) {
                inner.mem.reinforce(id);
            } else {
                inner.mem.remember(item);
            }
            written += 1;
        }
        // 顺手清掉该忘的 episode（不删 durable）。
        inner.mem.prune_expired();
        written
    }

    /// 记一条**兜底 episode**（抽取失败时把这一轮原文记成 episode）。
    pub fn record_fallback_episode(&self, text: &str) -> usize {
        self.record(vec![Recollection::fallback_episode(text)])
    }

    /// 某会话的滚动摘要（替换语义；没有 = 空串）。
    pub fn summary(&self, session: Option<&str>) -> String {
        self.guard().mem.summary(session).to_string()
    }

    /// 写某会话的滚动摘要（**替换**上一条；超过 300 字截断）。
    pub fn set_summary(&self, session: Option<&str>, summary: &str) {
        self.guard().mem.set_summary(session, summary);
    }

    /// 某会话**有没有**热卡（诊断 / 测试；不暴露内部类型）。
    pub fn card_is_some(&self, session: Option<&str>) -> bool {
        self.guard().mem.card(session).is_some()
    }

    /// 某会话热卡的渲染正文（空串 = 没有热卡）。
    pub fn card_text(&self, session: Option<&str>) -> String {
        let inner = self.guard();
        live2d_ai_runtime::hotmem::render_hot_card(inner.mem.card(session))
    }

    /// 状态面快照。
    pub fn snapshot(&self) -> HotMemorySnapshot {
        self.guard().mem.snapshot()
    }

    /// 库里的钉住候选（状态面 / 测试）。
    pub fn pinned_items(&self) -> Vec<PinnedItem> {
        self.guard().mem.pinned_items()
    }
}
