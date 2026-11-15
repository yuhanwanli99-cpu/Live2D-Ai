//! 进程内事实库 + 每会话热卡 / 摘要（**纯内存、无 IO、无时钟**）。
//!
//! # 时钟为什么是「调用方给的小时数」
//!
//! 遗忘曲线要算「距今多久」。这里**不读** `SystemTime`：宿主每轮调一次
//! [HotMemory::advance_to]，测试直接传 0 / 18 / 500。于是「三天后这条还清不清楚」
//! 在单测里逐字可复现，不依赖机器速度。
//!
//! # 落盘不在这里
//!
//! `hotmem.jsonl` / `hotmem-pinned.json` 的**读写**在宿主（desktop）侧；
//! 本模块只提供 [StoredFact] 这类可直接序列化的形状。理由是 M1 的边界：
//! 纯函数层不出 IO，于是 `cargo test -p live2d-ai-runtime` 不需要临时目录。
//!
//! # 每会话三样东西
//!
//! - **热卡**：同一会话只留一张，新请求整块替换（[HotMemory::set_card]）；
//! - **摘要**：≤300 字，替换上一条（[HotMemory::set_summary]）；
//! - **已说索引**：上一轮渲染过的钉住条目 id（[HotMemory::note_spoken]），
//!   用来跳过复读——条目不删。

use std::collections::BTreeMap;

use super::card::HotCard;
use super::fact::{FactKind, HotFact};
use super::pin::PinnedItem;

/// 「不带会话」的桶键（与 held-out 的会话 id 空间不冲突：归一化后的 id 不会是空串）。
pub const NO_SESSION: &str = "";

/// 落盘形状的一条事实（`hotmem.jsonl` 的一行 / `hotmem-pinned.json` 的一项）。
#[derive(Debug, Clone, PartialEq)]
pub struct StoredFact {
    /// 稳定 id。
    pub id: u64,
    /// 正文。
    pub text: String,
    /// 种类。
    pub kind: FactKind,
    /// 强度（小时）。
    pub s_hours: f64,
    /// 上次加固时刻（**小时**，与 [HotMemory::advance_to] 同一个坐标系）。
    pub reinforced_at: f64,
}

impl StoredFact {
    /// 投影成纯函数层用的 [HotFact]（年龄 = 现在 - 加固时刻，负数按 0）。
    pub fn project(&self, now_hours: f64) -> HotFact {
        HotFact {
            id: self.id,
            text: self.text.clone(),
            kind: self.kind,
            s_hours: self.s_hours,
            age_hours: (now_hours - self.reinforced_at).max(0.0),
        }
    }
}

/// 「记」的产物：抽取出来的事实（落盘前的形状）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recollection {
    /// 事实正文。
    pub text: String,
    /// episode / durable。
    pub kind: FactKind,
}

impl Recollection {
    /// 造一条。
    pub fn new(text: impl Into<String>, kind: FactKind) -> Self {
        Self {
            text: text.into(),
            kind,
        }
    }

    /// 抽取失败时的兜底：把这一轮的**用户原文**记成 episode（截断到 200 字）。
    pub fn fallback_episode(text: &str) -> Self {
        let trimmed = text.trim();
        let cut: String = trimmed.chars().take(200).collect();
        Self::new(cut, FactKind::Episode)
    }
}

/// 状态面快照（`GET` 诊断 / 测试；不承诺稳定 wire 契约）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotMemorySnapshot {
    /// 事实总数。
    pub facts: usize,
    /// durable 条数。
    pub durable: usize,
    /// 当前「清楚」档的 durable 条数（= 钉住块候选数）。
    pub pinned_clear: usize,
    /// 有热卡的会话数。
    pub cards: usize,
    /// 有摘要的会话数。
    pub summaries: usize,
}

/// 进程内事实库 + 每会话热卡 / 摘要 / 已说索引。
#[derive(Debug, Default)]
pub struct HotMemory {
    facts: Vec<StoredFact>,
    next_id: u64,
    /// 单调小时钟（只增不减；[Self::advance_to] 传入更小值 = no-op）。
    now_hours: f64,
    cards: BTreeMap<String, HotCard>,
    summaries: BTreeMap<String, String>,
    spoken: BTreeMap<String, Vec<u64>>,
}

impl HotMemory {
    /// 空库。
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前小时钟。
    pub fn now_hours(&self) -> f64 {
        self.now_hours
    }

    /// 推进时钟（**只增不减**：时钟回拨不生效，否则年龄会变负）。
    pub fn advance_to(&mut self, now_hours: f64) {
        if now_hours.is_finite() && now_hours > self.now_hours {
            self.now_hours = now_hours;
        }
    }

    /// 落盘形状的事实（宿主写 `hotmem.jsonl` 用）。
    pub fn stored(&self) -> &[StoredFact] {
        &self.facts
    }

    /// 读回一条（宿主启动装载用；id 冲突以先到者为准）。
    pub fn insert_stored(&mut self, fact: StoredFact) {
        if self.facts.iter().any(|f| f.id == fact.id) {
            return;
        }
        self.next_id = self.next_id.max(fact.id + 1);
        self.facts.push(fact);
    }

    /// 记一条新事实，返回 id。
    pub fn remember(&mut self, item: Recollection) -> u64 {
        let text = item.text.trim().to_string();
        if text.is_empty() {
            return 0;
        }
        let id = self.next_id.max(1);
        self.next_id = id + 1;
        self.facts.push(StoredFact {
            id,
            text,
            kind: item.kind,
            s_hours: item.kind.initial_hours(),
            reinforced_at: self.now_hours,
        });
        id
    }

    /// 当前所有事实的投影（年龄已算好）。
    pub fn facts(&self) -> Vec<HotFact> {
        self.facts
            .iter()
            .map(|f| f.project(self.now_hours))
            .collect()
    }

    /// 按 id 加固（**再被提到**）：强度 × 倍率、封顶、时间基点归到现在。
    ///
    /// 返回是否真找到——没找到就什么都不做（不新增）。
    pub fn reinforce(&mut self, id: u64) -> bool {
        let now = self.now_hours;
        let Some(fact) = self.facts.iter_mut().find(|f| f.id == id) else {
            return false;
        };
        let mut projected = fact.project(now);
        projected.reinforce();
        fact.s_hours = projected.s_hours;
        fact.reinforced_at = now;
        true
    }

    /// 与给定原文**逐字**相同的已有事实的 id（「用户原文复述」这一条加固判据）。
    pub fn exact_match(&self, text: &str) -> Option<u64> {
        let needle = text.trim();
        if needle.is_empty() {
            return None;
        }
        self.facts
            .iter()
            .find(|f| f.text.trim() == needle)
            .map(|f| f.id)
    }

    /// 清掉该遗忘的（**只清 episode**：`R < 0.05` 且超过 180 天）。
    ///
    /// 返回清掉的条数。durable 一条不删。
    pub fn prune_expired(&mut self) -> usize {
        let now = self.now_hours;
        let before = self.facts.len();
        self.facts.retain(|f| !f.project(now).is_expired());
        before - self.facts.len()
    }

    /// durable 且「清楚」的事实 → 钉住块候选（**新加固的在前**）。
    pub fn pinned_items(&self) -> Vec<PinnedItem> {
        let now = self.now_hours;
        let mut items: Vec<(f64, PinnedItem)> = self
            .facts
            .iter()
            .filter(|f| f.kind == FactKind::Durable)
            .map(|f| {
                let projected = f.project(now);
                let retention = projected.retention();
                (
                    f.reinforced_at,
                    PinnedItem::new(f.id, projected.text, retention),
                )
            })
            .filter(|(_, item)| item.is_clear())
            .collect();
        // 新加固的在前；同一时刻按 id 升序（稳定，测试可断言）。
        items.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.1.id.cmp(&b.1.id))
        });
        items.into_iter().map(|(_, item)| item).collect()
    }

    // ------------------------------------------------------------- 每会话状态

    /// 归一化会话键（`None` / 空串 → [NO_SESSION]）。
    fn key(session: Option<&str>) -> &str {
        match session {
            Some(s) if !s.trim().is_empty() => s.trim(),
            _ => NO_SESSION,
        }
    }

    /// 整块替换某会话的热卡（**新请求就是换掉，不是追加**）。
    pub fn set_card(&mut self, session: Option<&str>, card: HotCard) {
        let key = Self::key(session).to_string();
        if card.is_empty() {
            self.cards.remove(&key);
        } else {
            self.cards.insert(key, card);
        }
    }

    /// 取某会话的热卡（没有 = `None`，说话侧按「空热卡」处理）。
    pub fn card(&self, session: Option<&str>) -> Option<&HotCard> {
        self.cards.get(Self::key(session))
    }

    /// 清掉某会话的热卡（草稿话题变了 → 含 `[资料]` 的热卡整块作废）。
    pub fn clear_card(&mut self, session: Option<&str>) -> bool {
        self.cards.remove(Self::key(session)).is_some()
    }

    /// 记下某会话上一轮渲染过的钉住条目 id（下一轮渲染时跳过）。
    pub fn note_spoken(&mut self, session: Option<&str>, ids: Vec<u64>) {
        let key = Self::key(session).to_string();
        if ids.is_empty() {
            self.spoken.remove(&key);
        } else {
            self.spoken.insert(key, ids);
        }
    }

    /// 上一轮说过的钉住条目 id（缺省空）。
    pub fn spoken(&self, session: Option<&str>) -> &[u64] {
        self.spoken
            .get(Self::key(session))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// 写某会话的滚动摘要（**替换**，不追加）。
    pub fn set_summary(&mut self, session: Option<&str>, summary: impl Into<String>) {
        let key = Self::key(session).to_string();
        let text = summary.into();
        let text = text.trim();
        if text.is_empty() {
            self.summaries.remove(&key);
            return;
        }
        let clipped: String = text.chars().take(super::SUMMARY_MAX_CHARS).collect();
        self.summaries.insert(key, clipped);
    }

    /// 取某会话的摘要（没有 = 空串）。
    pub fn summary(&self, session: Option<&str>) -> &str {
        self.summaries
            .get(Self::key(session))
            .map(String::as_str)
            .unwrap_or("")
    }

    /// 状态面快照。
    pub fn snapshot(&self) -> HotMemorySnapshot {
        HotMemorySnapshot {
            facts: self.facts.len(),
            durable: self
                .facts
                .iter()
                .filter(|f| f.kind == FactKind::Durable)
                .count(),
            pinned_clear: self.pinned_items().len(),
            cards: self.cards.len(),
            summaries: self.summaries.len(),
        }
    }

    /// 供「查」用的候选事实：未过期、且 `R >= 0.2`（模糊及以上），**近的在前**。
    pub fn candidates(&self) -> Vec<HotFact> {
        let now = self.now_hours;
        let mut out: Vec<HotFact> = self
            .facts
            .iter()
            .map(|f| f.project(now))
            .filter(|f| f.tier().is_some())
            .collect();
        out.sort_by(|a, b| {
            a.age_hours
                .partial_cmp(&b.age_hours)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.id.cmp(&b.id))
        });
        out
    }
}
