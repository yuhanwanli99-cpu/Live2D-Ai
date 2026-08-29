//! Reducer 输入输出：`Event`（输入事实）、`Effect`（输出命令）、`DropReason`（拒绝原因）。
//!
//! 所有事件都通过 `live2d_ai_core::{Event, Effect, DropReason, GenerationOutcome,
//! PlaybackState}` 路径对外可见；本模块只把它们从 `lib.rs` 拆分以缩短主文件。

use crate::action::{ActionCommand, ActionEffect, SemanticAction};
use crate::ids::{Epoch, SentenceId, TurnId};

/// 生成侧终态：LLM 流结束 **且** TTS worker 完全排空的事实结果
/// （事实 [`Event::GenerationFinished`]，每 turn 恰一次）。
///
/// 注意：这只覆盖「合成完毕」，**不含**声卡是否播完——后者是播放闩的职责。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationOutcome {
    /// 全部句子成功合成。
    Completed,
    /// 合成过程发生失败（已入播放链的内容按 D8 策略处理）。
    Failed,
    /// 生成被取消（正常路径下 stop 已先推进 epoch，此变体到达时会被闸门丢弃，
    /// 保留它只为让 supervisor 在异常时序下也有确定表达）。
    Cancelled,
}

/// Reducer 的输入事件（外部世界发生的事实）。
///
/// 除 [`Event::StopRequested`] 外，每个事件都携带 `epoch`：
/// 该事件所属的会话代次。shell 派发时应以事件产生时刻的 `state.epoch` 盖章。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// 用户提交一条文本（开启主 turn，携带首个句子）。
    ///
    /// 仅在 epoch 匹配且无活动 turn 时被接受；否则确定性丢弃
    /// （[`DropReason::StaleEpoch`] / [`DropReason::TurnAlreadyActive`]，单主 turn）。
    UserSubmitted {
        epoch: Epoch,
        turn_id: TurnId,
        sentence_id: SentenceId,
        /// 文本载荷：骨架阶段仅记录、不参与判定。
        text: String,
    },
    /// 向当前 turn 追加一个后续句子（多句支持）。
    SentenceAdded {
        epoch: Epoch,
        turn_id: TurnId,
        sentence_id: SentenceId,
    },
    /// 生成侧终态事实：LLM 已结束且 TTS worker 完全排空（每 turn 恰一次）。
    ///
    /// **不等于 turn 完成**——若播放闩仍在 [`PlaybackState::Playing`]，
    /// turn 必须等待 [`Event::PlaybackDrained`]（P0-3 双闩锁）。
    GenerationFinished {
        epoch: Epoch,
        turn_id: TurnId,
        outcome: GenerationOutcome,
    },
    /// 播放侧事实：声卡回调已开始消费本 turn 的 PCM
    /// （supervisor 在首个非空块成功入环后同步报告，保证先于 Drained）。
    PlaybackStarted { epoch: Epoch, turn_id: TurnId },
    /// 播放侧事实：ring 已排空、回调消费完最后样本（DAC 播放完毕）。
    PlaybackDrained { epoch: Epoch, turn_id: TurnId },
    /// 播放侧事实：supervisor 按 D8 策略主动清空了待播内容（失败/故障）。
    PlaybackCleared { epoch: Epoch, turn_id: TurnId },
    /// 表演控制命令（play / interrupt / release）。
    ///
    /// epoch 由**根闸门**统一校验，通过后才转交动作子 reducer（[`apply_action`]）。
    Action {
        epoch: Epoch,
        command: ActionCommand,
    },
    /// 表演事实：一个动作**自然播放完毕**（P0-6 裁决；不是用户 release/interrupt）。
    ///
    /// 必须携带完成者的完整身份：reducer 仅当它与**当前 active 动作完全一致**
    /// 时才收尾（恰好一个 [`Effect::Action`](`Effect::Action`)::End）；
    /// 不匹配（如被抢占后迟到的旧完成）按 [`DropReason::StaleActionCompletion`]
    /// 确定性丢弃——绝不允许无身份的「释放」误伤后来者。
    ActionPlaybackFinished {
        epoch: Epoch,
        /// 自然完成的那个动作（与 [`State::active_turn`] 无关，
        /// 与动作子状态 current 判等）。
        action: SemanticAction,
    },
    /// 用户/系统请求停止：原子清空 turn/音频/动作并推进 epoch 恰好一次，回到 Idle。
    ///
    /// 不携带 epoch：它本身就是推进 epoch 的动作，永远作用于当前状态；
    /// 连续两次 stop 即两次递增（幂等性由调用方保证不重复派发）。
    StopRequested,
}

impl Event {
    /// 事件携带的会话代次；[`Event::StopRequested`] 返回 `None`。
    pub fn epoch(&self) -> Option<Epoch> {
        match self {
            Event::UserSubmitted { epoch, .. }
            | Event::SentenceAdded { epoch, .. }
            | Event::GenerationFinished { epoch, .. }
            | Event::PlaybackStarted { epoch, .. }
            | Event::PlaybackDrained { epoch, .. }
            | Event::PlaybackCleared { epoch, .. }
            | Event::Action { epoch, .. }
            | Event::ActionPlaybackFinished { epoch, .. } => Some(*epoch),
            Event::StopRequested => None,
        }
    }
}

/// 事件被丢弃的原因（供 shell 观测/日志）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropReason {
    /// 事件所属代次已过期（或超前）：stop 之后迟到的旧代次事件一律在此丢弃，
    /// **先于**一切 id 判定，状态保证不变。这是防 ID 重用歧义的核心语义。
    StaleEpoch {
        event_epoch: Epoch,
        current_epoch: Epoch,
    },
    /// 已有活动 turn，新输入被拒（单主 turn）。
    TurnAlreadyActive { incoming: TurnId, active: TurnId },
    /// 引用了未知/已结束的 turn。
    UnknownTurn { turn_id: TurnId },
    /// turn 级事实与当前双闩状态不兼容（重复报告、乱序迁移）——确定性丢弃。
    ///
    /// 当前闩状态可从 [`State::active_turn`] 直接观测，这里只带事实名便于日志定位。
    TurnFactMismatch {
        /// 事实名（如 `"playback_started"` / `"generation_finished"`）。
        fact: &'static str,
    },
    /// 动作自然完成事实与当前表演不符（被抢占后的迟到完成 / 空闲期完成）——
    /// 确定性丢弃，**绝不结束在演的后来者**（P0-6）。
    StaleActionCompletion {
        /// 迟到完成所声明的动作。
        finished: SemanticAction,
        /// reducer 判定时实际在演的动作（`None` = 无动作在演）。
        current: Option<SemanticAction>,
    },
}

/// Reducer 输出的命令（shell 据此驱动真实世界）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// 进入思考（thinking 表现/等待 LLM）。
    StartThinking { turn_id: TurnId },
    /// 停止当前播放（清空声卡待播内容；对空 ring 幂等）。
    StopPlayback,
    /// turn 被中止（stop 时产生，携带中止后的新 epoch）。
    TurnAborted { turn_id: TurnId, epoch: Epoch },
    /// **turn 正常到达终态**：生成闩与播放闩同时成立（双闩锁，恰好一次）。
    ///
    /// shell 只有收到本效果才允许：回到 Idle 受理新输入、提交 history、
    /// 启动 pending Say（P0-3/P0-4）。
    TurnCompleted {
        turn_id: TurnId,
        outcome: GenerationOutcome,
    },
    /// 回到空闲。
    GoIdle,
    /// 动作子系统的渲染命令（已过根 epoch 闸门）。
    Action(ActionEffect),
    /// 事件被丢弃（附原因）。
    Dropped { reason: DropReason },
}
