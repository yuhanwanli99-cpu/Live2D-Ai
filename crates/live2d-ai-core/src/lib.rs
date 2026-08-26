//! `live2d-ai-core` — 纯 reducer 状态机（零依赖、无 I/O、无 async）。
//!
//! 核心不变量（shell 可据此安全驱动真实世界）：
//! 1. **epoch 单一 owner**：只有根 [`State::epoch`]；只有 [`Event::StopRequested`]
//!    推进它（恰好 +1）。除 stop 外一切事件都携带产生它的 epoch，先过**根闸门**
//!    （不匹配当前代次 → [`DropReason::StaleEpoch`]，状态不变），再进各子规则——
//!    包括动作子系统（[`Event::Action`]）。这是防「ID 重用后迟到事件」的唯一权威闸门。
//! 2. **单主 turn / 多句**：至多一个活动 turn；首个句子经 [`Event::UserSubmitted`]
//!    开 turn，后续句子经 [`Event::SentenceAdded`] 追加进计划。
//! 3. **单音频播放器**：同一时刻至多一个块在播（[`State::active_audio`]），
//!    其余按序排队（[`State::pending_audio`]）。就绪块仅在空闲时立刻
//!    [`Effect::PlayAudio`]，否则静默入队；完成信号必须精确匹配 active 块，
//!    完成后串行启动队首。重复 chunk id 被拒；非 front 的完成被丢弃。
//! 4. **单 active 动作**：动作子状态 [`ActionState`] 不持有 epoch；抢占是单一显式
//!    效果 [`ActionEffect::Transition`]，任何终止路径（release / interrupt / stop）
//!    恰产生一个 [`ActionEffect::End`]。文本规则 fallback 见 [`action::rule_fallback`]。
//! 5. **stop 原子性**：一次 stop 同时清空 turn 计划、音频（active+pending）、动作，
//!    并推进 epoch 恰好一次；此后旧代次的迟到事件全部被闸门确定性隔离。
//! 6. **表演参数曲线**：[`performance`] 把六动作用纯函数采样为项目自有
//!    [`ParameterFrame`](performance::ParameterFrame)（零分配、确定性、限幅在
//!    Bai 常规安全范围），供未来 l2d adapter 做字段→参数 ID 映射。
//!
//! 本 crate 为纯逻辑；shell 负责把 [`Effect`] 翻译成真实播放/渲染动作。
//!
//! 许可：**AGPL-3.0-only**，以仓库根 `LICENSE` 为准。

pub mod action;
pub mod performance;

use std::collections::VecDeque;
use std::fmt;

pub use action::{
    ActionCommand, ActionDropReason, ActionEffect, ActionId, ActionSource, ActionState,
    ModelCapabilities, SemanticAction, Strength, apply_action,
};
pub use performance::{ParameterFrame, PerformancePlayer, SampleStatus};

// ---------------------------------------------------------------- 领域 id

macro_rules! id_newtype {
    ($(#[$doc:meta])* $name:ident, $label:literal) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            /// 底层数值。
            pub const fn get(self) -> u64 {
                self.0
            }
        }

        impl From<u64> for $name {
            fn from(value: u64) -> Self {
                Self(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!($label, "#{}"), self.0)
            }
        }
    };
}

id_newtype!(
    /// 会话代次：每次 `StopRequested` 递增。事件携带其所属代次，
    /// 代次不符的事件在 [`apply`] 处被确定性丢弃。**唯一 owner 是根 [`State]**。
    Epoch,
    "epoch"
);

id_newtype!(
    /// 一轮主对话（main turn）的唯一标识。v0 同时只允许一个活动 turn。
    ///
    /// 允许跨 [`Epoch`] 重用数值：同一 `TurnId` 在新旧两代里互不可见
    /// （迟到事件由 epoch 闸门拦截）。
    TurnId,
    "turn"
);

id_newtype!(
    /// turn 内一个句子的唯一标识（一个 turn 可含多句，顺序即播放计划顺序）。
    SentenceId,
    "sentence"
);

id_newtype!(
    /// 一个 TTS 音频块（chunk）的唯一标识。
    AudioChunkId,
    "chunk"
);

impl Epoch {
    /// 初始代次（0）。
    pub const ZERO: Self = Self(0);

    /// 递增（饱和加法，避免溢出回绕破坏单调性）。
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl Default for Epoch {
    fn default() -> Self {
        Self::ZERO
    }
}

// ---------------------------------------------------------------- 领域状态

/// 主流程相位。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Phase {
    /// 空闲（可接受新输入）。
    #[default]
    Idle,
    /// LLM/编排思考中。
    Thinking,
    /// 正在说话（音频播放中）。
    Speaking,
}

/// 一个主 turn 的骨架信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    pub id: TurnId,
    /// 本 turn 规划/已出现的句子（顺序即计划顺序）。
    pub sentences: Vec<SentenceId>,
}

/// 完整 reducer 状态（epoch 的唯一 owner）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct State {
    /// 会话代次：仅 [`Event::StopRequested`] 推进。
    pub epoch: Epoch,
    pub phase: Phase,
    /// 当前主 turn（含句子计划）。
    pub active_turn: Option<Turn>,
    /// 正在播放的音频块（单播放器；None = 空闲）。
    pub active_audio: Option<AudioChunkId>,
    /// 待播队列（FIFO，队首是下一块）。
    pub pending_audio: VecDeque<AudioChunkId>,
    /// 动作子状态（不含 epoch；见模块 [`action`]）。
    pub action: ActionState,
}

// ---------------------------------------------------------------- 输入事件

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
    /// 一个 TTS 音频块就绪（空闲则立刻开播，否则排队）。
    AudioChunkReady {
        epoch: Epoch,
        turn_id: TurnId,
        sentence_id: SentenceId,
        chunk_id: AudioChunkId,
    },
    /// 一个音频块播放完成（必须匹配当前 active 块）。
    AudioChunkFinished {
        epoch: Epoch,
        chunk_id: AudioChunkId,
    },
    /// 整个 turn 正常结束。
    TurnFinished { epoch: Epoch, turn_id: TurnId },
    /// 表演控制命令（play / interrupt / release）。
    ///
    /// epoch 由**根闸门**统一校验，通过后才转交动作子 reducer（[`apply_action`]）。
    Action {
        epoch: Epoch,
        command: ActionCommand,
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
            | Event::AudioChunkReady { epoch, .. }
            | Event::AudioChunkFinished { epoch, .. }
            | Event::TurnFinished { epoch, .. }
            | Event::Action { epoch, .. } => Some(*epoch),
            Event::StopRequested => None,
        }
    }
}

// ---------------------------------------------------------------- 输出 Effect

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
    /// 引用了 turn 内不存在的句子。
    UnknownSentence {
        turn_id: TurnId,
        sentence_id: SentenceId,
    },
    /// 音频块 id 已在播放或队列中（重复就绪被拒）。
    DuplicateChunk { chunk_id: AudioChunkId },
    /// 完成信号与当前正在播放的块不符（含非 front 的迟到完成）——丢弃。
    ChunkNotActive { chunk_id: AudioChunkId },
}

/// Reducer 输出的命令（shell 据此驱动真实世界）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// 进入思考（thinking 表现/等待 LLM）。
    StartThinking { turn_id: TurnId },
    /// 播放指定音频块（仅在块成为 active 时发出：首次开播或前一块完成后接续）。
    PlayAudio { chunk_id: AudioChunkId },
    /// 停止当前播放（全部）。
    StopPlayback,
    /// turn 被中止（stop 时产生，携带中止后的新 epoch）。
    TurnAborted { turn_id: TurnId, epoch: Epoch },
    /// 回到空闲。
    GoIdle,
    /// 动作子系统的渲染命令（已过根 epoch 闸门）。
    Action(ActionEffect),
    /// 事件被丢弃（附原因）。
    Dropped { reason: DropReason },
}

// ---------------------------------------------------------------- reducer

/// 纯 reducer：把 `event` 应用到 `state`，返回需要 shell 执行的效果列表。
///
/// 判定顺序固定且确定：
/// 1. **根 epoch 闸门**（除 `StopRequested` 外）：`event.epoch() != state.epoch`
///    → `Dropped(StaleEpoch)`，状态不变；
/// 2. 同代次规则（turn / 句子 / 音频队列 / 动作子 reducer）。
///
/// 不 panic、不 I/O；同类事件的处理结果只依赖 `state` 与 `event` 自身。
pub fn apply(state: &mut State, event: Event) -> Vec<Effect> {
    // ---- 1. 根 epoch 闸门：旧代次（含 ID 重用后的迟到事件）确定性丢弃。----
    if let Some(event_epoch) = event.epoch() {
        if event_epoch != state.epoch {
            return vec![Effect::Dropped {
                reason: DropReason::StaleEpoch {
                    event_epoch,
                    current_epoch: state.epoch,
                },
            }];
        }
    }

    // ---- 2. 同代次规则。----
    match event {
        Event::UserSubmitted {
            turn_id,
            sentence_id,
            ..
        } => begin_turn(state, turn_id, sentence_id),
        Event::SentenceAdded {
            turn_id,
            sentence_id,
            ..
        } => add_sentence(state, turn_id, sentence_id),
        Event::AudioChunkReady {
            turn_id,
            sentence_id,
            chunk_id,
            ..
        } => chunk_ready(state, turn_id, sentence_id, chunk_id),
        Event::AudioChunkFinished { chunk_id, .. } => chunk_finished(state, chunk_id),
        Event::TurnFinished { turn_id, .. } => turn_finished(state, turn_id),
        Event::Action { command, .. } => {
            // epoch 已由根闸门校验；动作子状态自身不持有、不推进 epoch。
            apply_action(&mut state.action, command)
                .into_iter()
                .map(Effect::Action)
                .collect()
        }
        Event::StopRequested => stop(state),
    }
}

/// 开启主 turn（单主 turn 规则）。
fn begin_turn(state: &mut State, turn_id: TurnId, sentence_id: SentenceId) -> Vec<Effect> {
    if let Some(active) = &state.active_turn {
        return vec![Effect::Dropped {
            reason: DropReason::TurnAlreadyActive {
                incoming: turn_id,
                active: active.id,
            },
        }];
    }
    state.active_turn = Some(Turn {
        id: turn_id,
        sentences: vec![sentence_id],
    });
    state.phase = Phase::Thinking;
    vec![Effect::StartThinking { turn_id }]
}

/// 向当前 turn 追加后续句子。
fn add_sentence(state: &mut State, turn_id: TurnId, sentence_id: SentenceId) -> Vec<Effect> {
    let Some(turn) = &mut state.active_turn else {
        return vec![Effect::Dropped {
            reason: DropReason::UnknownTurn { turn_id },
        }];
    };
    if turn.id != turn_id {
        return vec![Effect::Dropped {
            reason: DropReason::UnknownTurn { turn_id },
        }];
    }
    turn.sentences.push(sentence_id);
    Vec::new()
}

/// 音频块就绪：空闲立刻开播，否则入队；拒绝重复 id。
fn chunk_ready(
    state: &mut State,
    turn_id: TurnId,
    sentence_id: SentenceId,
    chunk_id: AudioChunkId,
) -> Vec<Effect> {
    let Some(turn) = &state.active_turn else {
        return vec![Effect::Dropped {
            reason: DropReason::UnknownTurn { turn_id },
        }];
    };
    if turn.id != turn_id {
        return vec![Effect::Dropped {
            reason: DropReason::UnknownTurn { turn_id },
        }];
    }
    if !turn.sentences.contains(&sentence_id) {
        return vec![Effect::Dropped {
            reason: DropReason::UnknownSentence {
                turn_id,
                sentence_id,
            },
        }];
    }
    if state.active_audio == Some(chunk_id) || state.pending_audio.contains(&chunk_id) {
        return vec![Effect::Dropped {
            reason: DropReason::DuplicateChunk { chunk_id },
        }];
    }
    if state.active_audio.is_none() {
        state.active_audio = Some(chunk_id);
        state.phase = Phase::Speaking;
        vec![Effect::PlayAudio { chunk_id }]
    } else {
        state.pending_audio.push_back(chunk_id);
        Vec::new()
    }
}

/// 播放完成：必须精确匹配 active 块；完成后串行启动队首。
fn chunk_finished(state: &mut State, chunk_id: AudioChunkId) -> Vec<Effect> {
    if state.active_audio != Some(chunk_id) {
        return vec![Effect::Dropped {
            reason: DropReason::ChunkNotActive { chunk_id },
        }];
    }
    state.active_audio = None;
    match state.pending_audio.pop_front() {
        Some(next) => {
            state.active_audio = Some(next);
            vec![Effect::PlayAudio { chunk_id: next }]
        }
        None => Vec::new(),
    }
}

/// turn 正常结束：清空音频与 turn，回 Idle。
fn turn_finished(state: &mut State, turn_id: TurnId) -> Vec<Effect> {
    let matches = state.active_turn.as_ref().is_some_and(|t| t.id == turn_id);
    if !matches {
        return vec![Effect::Dropped {
            reason: DropReason::UnknownTurn { turn_id },
        }];
    }
    let had_audio = state.active_audio.is_some() || !state.pending_audio.is_empty();
    state.active_turn = None;
    state.active_audio = None;
    state.pending_audio.clear();
    state.phase = Phase::Idle;
    let mut effects = Vec::new();
    if had_audio {
        effects.push(Effect::StopPlayback);
    }
    effects.push(Effect::GoIdle);
    effects
}

/// 会话停止：原子清空 turn/音频/动作并推进 epoch 恰好一次。
fn stop(state: &mut State) -> Vec<Effect> {
    state.epoch = state.epoch.next();
    let aborted_turn = state.active_turn.take();
    state.active_audio = None;
    state.pending_audio.clear();
    state.phase = Phase::Idle;
    let mut effects = vec![Effect::StopPlayback, Effect::GoIdle];
    if let Some(action) = state.action.current.take() {
        effects.push(Effect::Action(ActionEffect::End { action }));
    }
    if let Some(turn) = aborted_turn {
        effects.push(Effect::TurnAborted {
            turn_id: turn.id,
            epoch: state.epoch,
        });
    }
    effects
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    const E0: Epoch = Epoch::ZERO;

    /// 以当前 state.epoch 盖章提交文本。
    fn submit(state: &mut State, turn: u64, sentence: u64) -> Vec<Effect> {
        apply(
            state,
            Event::UserSubmitted {
                epoch: state.epoch,
                turn_id: turn.into(),
                sentence_id: sentence.into(),
                text: "hello".to_owned(),
            },
        )
    }

    fn chunk_ready(epoch: Epoch, turn: u64, sentence: u64, chunk: u64) -> Event {
        Event::AudioChunkReady {
            epoch,
            turn_id: turn.into(),
            sentence_id: sentence.into(),
            chunk_id: chunk.into(),
        }
    }

    fn finished(epoch: Epoch, chunk: u64) -> Event {
        Event::AudioChunkFinished {
            epoch,
            chunk_id: chunk.into(),
        }
    }

    fn dropped(reason: DropReason) -> Vec<Effect> {
        vec![Effect::Dropped { reason }]
    }

    fn stale(event_epoch: u64, current_epoch: u64) -> Vec<Effect> {
        dropped(DropReason::StaleEpoch {
            event_epoch: Epoch::from(event_epoch),
            current_epoch: Epoch::from(current_epoch),
        })
    }

    /// 要求：stop 推进 epoch 恰好一次（可重复、单调）并原子清空全部子系统。
    #[test]
    fn stop_clears_everything_and_advances_epoch_once() {
        let mut state = State::default();
        submit(&mut state, 1, 1);
        apply(&mut state, chunk_ready(E0, 1, 1, 10));
        state.action.capabilities = ModelCapabilities::all();
        apply(
            &mut state,
            Event::Action {
                epoch: E0,
                command: ActionCommand::Play {
                    action: SemanticAction::new(
                        ActionId::Nod,
                        Strength::Low,
                        ActionSource::LlmTool,
                    ),
                },
            },
        );
        assert_eq!(state.active_audio, Some(AudioChunkId::from(10)));
        assert!(state.action.current.is_some());

        let effects = apply(&mut state, Event::StopRequested);
        assert_eq!(state.epoch, Epoch::from(1));
        assert_eq!(state.phase, Phase::Idle);
        assert!(state.active_turn.is_none());
        assert_eq!(state.active_audio, None);
        assert!(state.pending_audio.is_empty());
        assert!(state.action.current.is_none());
        assert_eq!(
            effects,
            vec![
                Effect::StopPlayback,
                Effect::GoIdle,
                Effect::Action(ActionEffect::End {
                    action: SemanticAction::new(
                        ActionId::Nod,
                        Strength::Low,
                        ActionSource::LlmTool
                    ),
                }),
                Effect::TurnAborted {
                    turn_id: TurnId::from(1),
                    epoch: Epoch::from(1),
                },
            ]
        );

        // 再次 stop：epoch 继续 +1（此时无动作/turn，效果只剩停播+回闲）。
        let effects = apply(&mut state, Event::StopRequested);
        assert_eq!(state.epoch, Epoch::from(2));
        assert_eq!(effects, vec![Effect::StopPlayback, Effect::GoIdle]);
    }

    /// 要求：旧代次事件在根闸门被确定性丢弃，状态不变（覆盖所有事件种类）。
    #[test]
    fn stale_events_of_every_kind_are_dropped_after_stop() {
        let mut state = State::default();
        submit(&mut state, 1, 1);
        apply(&mut state, chunk_ready(E0, 1, 1, 10));
        apply(&mut state, Event::StopRequested); // epoch → 1
        let snapshot = state.clone();

        let cases = vec![
            chunk_ready(E0, 1, 1, 11),
            Event::SentenceAdded {
                epoch: E0,
                turn_id: 1.into(),
                sentence_id: 2.into(),
            },
            finished(E0, 10),
            Event::TurnFinished {
                epoch: E0,
                turn_id: 1.into(),
            },
            Event::Action {
                epoch: E0,
                command: ActionCommand::Release,
            },
            Event::UserSubmitted {
                epoch: E0,
                turn_id: 2.into(),
                sentence_id: 1.into(),
                text: "late".to_owned(),
            },
        ];
        for event in cases {
            assert_eq!(apply(&mut state, event), stale(0, 1));
            assert_eq!(state, snapshot, "stale event must not change state");
        }

        // 超前代次同样拒绝（确定性：只认当前代次）。
        assert_eq!(
            apply(
                &mut state,
                Event::TurnFinished {
                    epoch: 2.into(),
                    turn_id: 9.into(),
                }
            ),
            stale(2, 1)
        );
    }

    /// 核心场景：ID 重用后，旧代次迟到事件不得污染新代次会话。
    #[test]
    fn reused_ids_across_epochs_are_isolated_by_epoch_gate() {
        let mut state = State::default();

        // 代次 0：turn#7 句#1，播放 chunk#100 并排队 chunk#101。
        submit(&mut state, 7, 1);
        apply(&mut state, chunk_ready(E0, 7, 1, 100));
        apply(&mut state, chunk_ready(E0, 7, 1, 101));
        assert_eq!(state.active_audio, Some(AudioChunkId::from(100)));
        apply(&mut state, Event::StopRequested); // epoch → 1，全部清空

        // 代次 1：shell 重用相同数值 turn#7 / 句#1 / chunk#100 开新会话——合法。
        assert_eq!(
            submit(&mut state, 7, 1),
            vec![Effect::StartThinking {
                turn_id: TurnId::from(7)
            }]
        );

        // 旧代次迟到的音频块：id 完全命中当前 turn/句子，但 epoch=0 → 必须丢弃，
        // 不得入队、不得把 Thinking 翻成 Speaking。
        assert_eq!(apply(&mut state, chunk_ready(E0, 7, 1, 100)), stale(0, 1));
        assert_eq!(state.active_audio, None);
        assert_eq!(state.phase, Phase::Thinking);

        // 新代次重新排队 chunk#100（数值重用）→ 立刻成为 active。
        let epoch1 = state.epoch;
        assert_eq!(
            apply(&mut state, chunk_ready(epoch1, 7, 1, 100)),
            vec![Effect::PlayAudio {
                chunk_id: AudioChunkId::from(100)
            }]
        );

        // 旧代次迟到的“播放完成”：若只看 id 会误删新代次的 chunk#100；
        // epoch 闸门必须先行拦下。
        assert_eq!(apply(&mut state, finished(E0, 100)), stale(0, 1));
        assert_eq!(state.active_audio, Some(AudioChunkId::from(100)));

        // 新代次真正的完成信号正常消费。
        assert_eq!(
            apply(&mut state, finished(epoch1, 100)),
            Vec::<Effect>::new()
        );
        assert_eq!(state.active_audio, None);

        // 旧代次迟到的 TurnFinished：id 命中但 epoch 不符 → 丢弃，新 turn 存活。
        assert_eq!(
            apply(
                &mut state,
                Event::TurnFinished {
                    epoch: E0,
                    turn_id: 7.into(),
                }
            ),
            stale(0, 1)
        );
        assert_eq!(state.active_turn.as_ref().unwrap().id, TurnId::from(7));
    }

    /// 要求：单主 turn —— 活动 turn 存在时新 UserSubmitted 被丢弃；结束后可再开。
    #[test]
    fn single_main_turn() {
        let mut state = State::default();

        assert_eq!(
            submit(&mut state, 1, 1),
            vec![Effect::StartThinking {
                turn_id: TurnId::from(1)
            }]
        );

        let effects = submit(&mut state, 2, 1);
        assert_eq!(
            effects,
            dropped(DropReason::TurnAlreadyActive {
                incoming: TurnId::from(2),
                active: TurnId::from(1),
            })
        );
        assert_eq!(state.active_turn.as_ref().unwrap().id, TurnId::from(1));

        let effects = apply(
            &mut state,
            Event::TurnFinished {
                epoch: E0,
                turn_id: 1.into(),
            },
        );
        assert_eq!(effects, vec![Effect::GoIdle]);
        assert_eq!(state.phase, Phase::Idle);

        assert_eq!(
            submit(&mut state, 3, 1),
            vec![Effect::StartThinking {
                turn_id: TurnId::from(3)
            }]
        );
    }

    /// 要求：多句 —— SentenceAdded 把后续句子追加进当前 turn 计划。
    #[test]
    fn sentence_added_extends_active_turn_plan() {
        let mut state = State::default();
        submit(&mut state, 1, 1);

        assert_eq!(
            apply(
                &mut state,
                Event::SentenceAdded {
                    epoch: E0,
                    turn_id: 1.into(),
                    sentence_id: 2.into(),
                }
            ),
            Vec::<Effect>::new()
        );
        assert_eq!(
            state.active_turn.as_ref().unwrap().sentences,
            vec![SentenceId::from(1), SentenceId::from(2)]
        );

        // 第二句的音频可以正常入队播放（证明追加进了计划）。
        assert_eq!(
            apply(&mut state, chunk_ready(E0, 1, 2, 20)),
            vec![Effect::PlayAudio {
                chunk_id: AudioChunkId::from(20)
            }]
        );

        // 引用别的 turn 的 SentenceAdded → UnknownTurn，状态不变。
        assert_eq!(
            apply(
                &mut state,
                Event::SentenceAdded {
                    epoch: E0,
                    turn_id: 42.into(),
                    sentence_id: 9.into(),
                }
            ),
            dropped(DropReason::UnknownTurn {
                turn_id: TurnId::from(42)
            })
        );
        assert_eq!(state.active_turn.unwrap().sentences.len(), 2);
    }

    /// 要求：单音频播放器 —— 空闲才发 PlayAudio，其余排队；完成必须匹配 active；
    /// 完成后串行启动下一块；重复 id 与非 front 完成被拒。
    #[test]
    fn audio_player_queues_and_serializes() {
        let mut state = State::default();
        submit(&mut state, 1, 1);
        apply(
            &mut state,
            Event::SentenceAdded {
                epoch: E0,
                turn_id: 1.into(),
                sentence_id: 2.into(),
            },
        );

        // 第一块：空闲 → 立刻 PlayAudio。
        assert_eq!(
            apply(&mut state, chunk_ready(E0, 1, 1, 10)),
            vec![Effect::PlayAudio {
                chunk_id: AudioChunkId::from(10)
            }]
        );
        assert_eq!(state.phase, Phase::Speaking);

        // 后续块：忙碌 → 静默入队，无效果。
        assert_eq!(
            apply(&mut state, chunk_ready(E0, 1, 2, 11)),
            Vec::<Effect>::new()
        );
        assert_eq!(
            apply(&mut state, chunk_ready(E0, 1, 1, 12)),
            Vec::<Effect>::new()
        );
        assert_eq!(
            state.pending_audio,
            [AudioChunkId::from(11), AudioChunkId::from(12)]
        );

        // 重复 id（active 或排队中）→ DuplicateChunk。
        assert_eq!(
            apply(&mut state, chunk_ready(E0, 1, 1, 10)),
            dropped(DropReason::DuplicateChunk {
                chunk_id: AudioChunkId::from(10)
            })
        );
        assert_eq!(
            apply(&mut state, chunk_ready(E0, 1, 1, 12)),
            dropped(DropReason::DuplicateChunk {
                chunk_id: AudioChunkId::from(12)
            })
        );

        // 非 front 完成（chunk 12 还在排队）→ ChunkNotActive，队列不变。
        assert_eq!(
            apply(&mut state, finished(E0, 12)),
            dropped(DropReason::ChunkNotActive {
                chunk_id: AudioChunkId::from(12)
            })
        );

        // front 完成 → 出队并串行启动下一块。
        assert_eq!(
            apply(&mut state, finished(E0, 10)),
            vec![Effect::PlayAudio {
                chunk_id: AudioChunkId::from(11)
            }]
        );
        assert_eq!(
            apply(&mut state, finished(E0, 11)),
            vec![Effect::PlayAudio {
                chunk_id: AudioChunkId::from(12)
            }]
        );
        // 队列耗尽：完成只出队，无新效果（等待 TurnFinished）。
        assert_eq!(apply(&mut state, finished(E0, 12)), Vec::<Effect>::new());
        assert_eq!(state.active_audio, None);

        // 已消费块的迟到完成 → ChunkNotActive。
        assert_eq!(
            apply(&mut state, finished(E0, 10)),
            dropped(DropReason::ChunkNotActive {
                chunk_id: AudioChunkId::from(10)
            })
        );
    }

    /// 相位流转：Thinking →（首个音频）→ Speaking →（turn 结束）→ Idle；
    /// 结束时若仍有音频应顺带 StopPlayback。
    #[test]
    fn phase_transitions_and_turn_finish_cleanup() {
        let mut state = State::default();
        submit(&mut state, 1, 1);
        assert_eq!(state.phase, Phase::Thinking);

        apply(&mut state, chunk_ready(E0, 1, 1, 10));
        apply(&mut state, chunk_ready(E0, 1, 1, 11));
        assert_eq!(state.phase, Phase::Speaking);
        apply(&mut state, finished(E0, 10));
        assert_eq!(state.phase, Phase::Speaking); // 还有 active 块

        let effects = apply(
            &mut state,
            Event::TurnFinished {
                epoch: E0,
                turn_id: 1.into(),
            },
        );
        assert_eq!(effects, vec![Effect::StopPlayback, Effect::GoIdle]);
        assert_eq!(state.phase, Phase::Idle);
        assert!(state.active_turn.is_none());
        assert_eq!(state.active_audio, None);
        assert!(state.pending_audio.is_empty());
    }

    /// 引用未知 turn/句子 → 安全丢弃（epoch 相符前提下按 id 判废）。
    #[test]
    fn unknown_references_are_dropped() {
        let mut state = State::default();
        submit(&mut state, 1, 1);
        let epoch = state.epoch;

        assert_eq!(
            apply(&mut state, chunk_ready(epoch, 1, 99, 10)), // 句子不在计划内
            dropped(DropReason::UnknownSentence {
                turn_id: TurnId::from(1),
                sentence_id: SentenceId::from(99),
            })
        );
        assert_eq!(
            apply(&mut state, chunk_ready(epoch, 42, 1, 10)), // 非当前 turn
            dropped(DropReason::UnknownTurn {
                turn_id: TurnId::from(42)
            })
        );
        assert_eq!(
            apply(&mut state, chunk_ready(epoch, 42, 99, 10)), // 无活动 turn
            dropped(DropReason::UnknownTurn {
                turn_id: TurnId::from(42)
            })
        );
    }

    /// 事件代次访问器：除 StopRequested 外都携带 epoch。
    #[test]
    fn event_epoch_accessor() {
        assert_eq!(Event::StopRequested.epoch(), None);
        assert_eq!(
            Event::Action {
                epoch: 5.into(),
                command: ActionCommand::Release,
            }
            .epoch(),
            Some(Epoch::from(5))
        );
    }
}
