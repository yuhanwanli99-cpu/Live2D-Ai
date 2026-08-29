//! 领域状态：根 [`State`] + 单个 [`Turn`] 的双闩锁 + 相位 [`Phase`]。
//!
//! - [`State::epoch`] 是 epoch 的**唯一 owner**，仅由 [`crate::Event::StopRequested`]
//!   推进；
//! - [`Turn`] 同时持有「生成闩」[`Turn::generation`] 与「播放闩」
//!   [`Turn::playback`]：turn 终态由两者**同时**成立决定（节点 A 裁决 P0-1/P0-3）；
//! - [`Phase`] 是对外的轻量状态机：驱动 shell 决定「是否展示思考/说话 UI」。

use crate::action::ActionState;
use crate::events::GenerationOutcome;
use crate::ids::{Epoch, SentenceId, TurnId};

/// 主流程相位。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Phase {
    /// 空闲（可接受新输入）。
    #[default]
    Idle,
    /// LLM/编排思考中。
    Thinking,
    /// 正在说话（声卡在消费本 turn 的 PCM）。
    Speaking,
}

/// 播放侧事实状态机（turn 内单调迁移；事实事件见 [`crate::Event`]）。
///
/// ```text
/// NeverStarted ──Started──► Playing ──Drained──► Drained（终态）
///      │  ▲                    │
///      └──┴─Cleared────────────┴──► Cleared（终态）
/// ```
///
/// - `NeverStarted` + 生成终态 ⇒ turn 立即可完成（如空回答，从未有 PCM 入环）；
/// - `Playing` 必须等到 `Drained`（真实 DAC 排空）才允许 turn 完成；
/// - `Cleared` 由 supervisor 清环后报告（D8 失败/故障策略）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackState {
    /// 尚未有 PCM 开始消费。
    #[default]
    NeverStarted,
    /// 声卡回调正在消费本 turn 的 PCM。
    Playing,
    /// ring 已排空且回调消费完最后样本（DAC 播放完毕）。
    Drained,
    /// supervisor 主动清空了待播内容（失败/故障策略）。
    Cleared,
}

impl PlaybackState {
    /// 是否为播放终态（允许 turn 结束）。
    ///
    /// 依节点 A 裁决（P0-3）：`audio_terminal = Drained | Cleared | NeverStarted`。
    /// `NeverStarted` 在「生成已结束」语境下同样是终态——引擎契约保证
    /// `Done` 只在 TTS worker 完全排空后发出，故生成终态到达时若从未起播，
    /// 意味着**零 PCM 曾被产出**（空回答），无需再等待任何播放事实。
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Drained | Self::Cleared | Self::NeverStarted)
    }
}

/// 一个主 turn 的骨架信息 + 双闩锁。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    pub id: TurnId,
    /// 本 turn 规划/已出现的句子（顺序即计划顺序）。
    pub sentences: Vec<SentenceId>,
    /// 生成闩：`Some` = 已收到 [`crate::Event::GenerationFinished`]。
    pub generation: Option<GenerationOutcome>,
    /// 播放闩：当前播放侧事实状态。
    pub playback: PlaybackState,
}

/// 完整 reducer 状态（epoch 的唯一 owner）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    /// 会话代次：仅 [`crate::Event::StopRequested`] 推进。
    pub epoch: Epoch,
    pub phase: Phase,
    /// 当前主 turn（含句子计划与双闩锁）。
    pub active_turn: Option<Turn>,
    /// 动作子状态（不含 epoch；见模块 [`crate::action`]）。
    pub action: ActionState,
}
