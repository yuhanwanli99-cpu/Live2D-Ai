//! 决策账本：最近 N 条决策 + 计数——`state_json` 的唯一数据源。
//!
//! **顺序关联**：`TurnPrompt` 的 payload 是**正文**（不带 turn id），`TurnEnded`
//! 的 payload 才是 turn id。两者都由 host 在**同一个 Mod worker 上顺序投递**，
//! 且 supervisor 一轮内不并发（提交 turn → 跑完 → 发 TurnEnded），因此
//! 「最近一条未结项决策」就是该 TurnEnded 对应的那一轮。turn id 在结项时才写进条目。

use std::collections::VecDeque;

use serde_json::json;

use crate::decision::{Decision, EmotionHint, IntentHint, Lexicon, derive};

/// `log_capacity` 缺省值。
pub const DEFAULT_LOG_CAPACITY: usize = 20;
/// `log_capacity` 下限。
pub const MIN_LOG_CAPACITY: usize = 1;
/// `log_capacity` 上限。
pub const MAX_LOG_CAPACITY: usize = 200;

/// 钳位 `log_capacity`，**永不返回越界值**。
pub fn clamp_log_capacity(value: usize) -> usize {
    value.clamp(MIN_LOG_CAPACITY, MAX_LOG_CAPACITY)
}

/// 一条决策记录（只读快照；**没有任何投递通道**）。
#[derive(Debug, Clone, PartialEq)]
pub struct LedgerEntry {
    /// 决策序号（从 1 开始，等于产生时的 `decisions` 计数）。
    pub seq: u64,
    /// 结项时写上的 turn id；未结项为 `None`。
    pub turn: Option<String>,
    pub emotion: EmotionHint,
    pub intent: IntentHint,
    pub speed: f64,
    pub pitch: f64,
    /// 对应的 `TurnEnded` 是否已到达。
    pub closed: bool,
}

impl LedgerEntry {
    /// `state_json` 里的单条形状。
    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "seq": self.seq,
            "turn": self.turn,
            "emotion": self.emotion.as_str(),
            "intent": self.intent.as_str(),
            "suggested_tts": { "speed": self.speed, "pitch": self.pitch },
            "closed": self.closed,
            "delivered": false,
        })
    }
}

/// [`DecisionLedger::record_prompt`] 的结果（供 runtime 决定日志措辞）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PromptOutcome {
    /// 清洗后正文为空：**无决策**、零副作用。
    Silent,
    /// 记录了一条决策（仅日志 / 快照，未投递）。
    Decided {
        emotion: EmotionHint,
        intent: IntentHint,
        speed: f64,
        pitch: f64,
    },
}

/// 有界决策账本：只保留最近 `capacity` 条，计数不受容量影响。
#[derive(Debug, Clone)]
pub struct DecisionLedger {
    capacity: usize,
    entries: VecDeque<LedgerEntry>,
    turns_seen: u64,
    turns_ended: u64,
    decisions: u64,
    silent: u64,
    errors: u64,
    /// 是否有「已收到 TurnPrompt、尚未收到 TurnEnded」的在飞轮。
    open: bool,
}

impl DecisionLedger {
    /// 新建账本；`capacity` 走 [`clamp_log_capacity`]。
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: clamp_log_capacity(capacity),
            entries: VecDeque::new(),
            turns_seen: 0,
            turns_ended: 0,
            decisions: 0,
            silent: 0,
            errors: 0,
            open: false,
        }
    }

    /// `TurnPrompt`：观察一轮输入，必要时记录一条决策。
    pub fn record_prompt(&mut self, text: &str, lexicon: Lexicon) -> PromptOutcome {
        self.turns_seen += 1;
        self.open = true;
        let decision: Decision = derive(text, lexicon);
        if decision.is_silent() {
            self.silent += 1;
            return PromptOutcome::Silent;
        }
        self.decisions += 1;
        let entry = LedgerEntry {
            seq: self.decisions,
            turn: None,
            emotion: decision.emotion,
            intent: decision.intent,
            speed: decision.suggested_tts.speed,
            pitch: decision.suggested_tts.pitch,
            closed: false,
        };
        while self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
        PromptOutcome::Decided {
            emotion: decision.emotion,
            intent: decision.intent,
            speed: decision.suggested_tts.speed,
            pitch: decision.suggested_tts.pitch,
        }
    }

    /// `TurnEnded`：把最近一条未结项决策结项 + 写上 turn id。
    ///
    /// 返回 `false` = 没有在飞轮（乱序 / 重复的 TurnEnded）→ 计入 `errors`，
    /// **不是**主链失败（host 只把它当一次事件处理）。
    pub fn record_ended(&mut self, turn: &str) -> bool {
        if !self.open {
            self.errors += 1;
            return false;
        }
        self.open = false;
        self.turns_ended += 1;
        if let Some(entry) = self.entries.iter_mut().rev().find(|e| !e.closed) {
            entry.closed = true;
            entry.turn = Some(turn.to_string());
        }
        true
    }

    /// `state_json` 契约（见 crate 头注「状态面」）。
    pub fn state_json(&self) -> serde_json::Value {
        json!({
            "delivered": false,
            "channel": "none",
            "turns_seen": self.turns_seen,
            "turns_ended": self.turns_ended,
            "decisions": self.decisions,
            "silent": self.silent,
            "errors": self.errors,
            "log_capacity": self.capacity,
            "recent_decisions": self
                .entries
                .iter()
                .map(LedgerEntry::to_json)
                .collect::<Vec<_>>(),
        })
    }

    /// 账本容量（已钳位）。
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// 见过的 `TurnPrompt` 数（含静默轮）。
    pub fn turns_seen(&self) -> u64 {
        self.turns_seen
    }

    /// 结项过的 `TurnEnded` 数。
    pub fn turns_ended(&self) -> u64 {
        self.turns_ended
    }

    /// 产生过的决策数（不含静默轮、不受容量影响）。
    pub fn decisions(&self) -> u64 {
        self.decisions
    }

    /// 静默轮数（正文清洗后为空）。
    pub fn silent(&self) -> u64 {
        self.silent
    }

    /// 乱序 / 重复 `TurnEnded` 计数。
    pub fn errors(&self) -> u64 {
        self.errors
    }

    /// 最近 `capacity` 条决策（旧 → 新）。
    pub fn recent(&self) -> &VecDeque<LedgerEntry> {
        &self.entries
    }
}
