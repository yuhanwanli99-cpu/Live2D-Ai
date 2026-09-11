//! 纯 reducer `apply`：把 `event` 应用到 `state`，返回需要 shell 执行的效果列表。
//!
//! 判定顺序固定且确定：
//! 1. **根 epoch 闸门**（除 `StopRequested` 外）：`event.epoch() != state.epoch`
//!    → `Dropped(StaleEpoch)`，状态不变；
//! 2. 同代次规则（turn / 句子 / 音频队列 / 动作子 reducer）。
//!
//! 不 panic、不 I/O；同类事件的处理结果只依赖 `state` 与 `event` 自身。

use crate::action::apply_action;
use crate::events::{DropReason, Effect, Event, GenerationOutcome};
use crate::ids::{SentenceId, TurnId};
use crate::state::{Phase, PlaybackState, State, Turn};

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
    if let Some(event_epoch) = event.epoch()
        && event_epoch != state.epoch
    {
        return vec![Effect::Dropped {
            reason: DropReason::StaleEpoch {
                event_epoch,
                current_epoch: state.epoch,
            },
        }];
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
        Event::GenerationFinished {
            turn_id, outcome, ..
        } => {
            // 生成闩落锁后立即检查双闩：若播放侧早已终态（如空回答
            // NeverStarted、失败路径已 Cleared），turn 此刻即完成。
            let effects = generation_finished(state, turn_id, outcome);
            maybe_finish_turn(effects, state)
        }
        Event::PlaybackStarted { turn_id, .. } => playback_started(state, turn_id),
        Event::PlaybackDrained { turn_id, .. } => {
            let effects =
                advance_playback(state, turn_id, "playback_drained", PlaybackState::Drained);
            maybe_finish_turn(effects, state)
        }
        Event::PlaybackCleared { turn_id, .. } => {
            let effects =
                advance_playback(state, turn_id, "playback_cleared", PlaybackState::Cleared);
            maybe_finish_turn(effects, state)
        }
        Event::Action { command, .. } => {
            // epoch 已由根闸门校验；动作子状态自身不持有、不推进 epoch。
            apply_action(&mut state.action, command)
                .into_iter()
                .map(Effect::Action)
                .collect()
        }
        Event::ActionPlaybackFinished { action, .. } => action_playback_finished(state, action),
        Event::StopRequested => stop(state),
    }
}

/// 开启主 turn（单主 turn 规则）；双闩初始化为「生成未定 / 从未播放」。
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
        generation: None,
        playback: PlaybackState::NeverStarted,
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

/// 生成闩落锁：恰一次；随后由调用方检查是否可完成 turn。
///
/// 注意：`Failed/Cancelled` **不**在此清播放状态——是否清环是 D8 策略，
/// 由 supervisor 执行 [`Effect::StopPlayback`] / 报告
/// [`Event::PlaybackCleared`]；reducer 只认事实，不猜策略。
fn generation_finished(
    state: &mut State,
    turn_id: TurnId,
    outcome: GenerationOutcome,
) -> Vec<Effect> {
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
    if turn.generation.is_some() {
        return vec![Effect::Dropped {
            reason: DropReason::TurnFactMismatch {
                fact: "generation_finished",
            },
        }];
    }
    turn.generation = Some(outcome);
    Vec::new()
}

/// 播放闩迁移 `Playing`（事实 [`Event::PlaybackStarted`]；仅 `NeverStarted` 可达）。
fn playback_started(state: &mut State, turn_id: TurnId) -> Vec<Effect> {
    let effects = advance_playback_inner(state, turn_id, "playback_started", |current| {
        matches!(current, PlaybackState::NeverStarted)
    });
    // 迁移成功（效果向量为空）时首次起播翻相位；被丢弃则状态未变、无相位变化。
    if effects.is_empty()
        && let Some(turn) = &state.active_turn
        && turn.playback == PlaybackState::Playing
        && state.phase == Phase::Thinking
    {
        state.phase = Phase::Speaking;
    }
    effects
}

/// 播放闩迁移到 `expected`（`Drained` / `Cleared`），随后做双闩终态检查。
fn advance_playback(
    state: &mut State,
    turn_id: TurnId,
    fact: &'static str,
    expected: PlaybackState,
) -> Vec<Effect> {
    // Drained 只能从 Playing 到达；Cleared 允许从 NeverStarted（尚未起播即被清）
    // 或 Playing 到达——与模块文档的状态机图一致。
    advance_playback_inner(state, turn_id, fact, |current| match expected {
        PlaybackState::Drained => matches!(current, PlaybackState::Playing),
        PlaybackState::Cleared => {
            matches!(
                current,
                PlaybackState::NeverStarted | PlaybackState::Playing
            )
        }
        _ => false,
    })
}

/// 动作自然完成事实（P0-6）：仅当完成者与当前 active 完全一致时收尾
/// （恰好一个 `End` 效果）；否则按 stale completion 丢弃，绝不误伤后来者。
fn action_playback_finished(
    state: &mut State,
    action: crate::action::SemanticAction,
) -> Vec<Effect> {
    match state.action.current {
        Some(current) if current == action => {
            state.action.current = None;
            vec![Effect::Action(crate::action::ActionEffect::End { action })]
        }
        other => vec![Effect::Dropped {
            reason: DropReason::StaleActionCompletion {
                finished: action,
                current: other,
            },
        }],
    }
}

/// 播放闩迁移共用骨架：turn 存在性 → 迁移合法性 → 落锁。
fn advance_playback_inner(
    state: &mut State,
    turn_id: TurnId,
    fact: &'static str,
    allowed_from: impl FnOnce(&PlaybackState) -> bool,
) -> Vec<Effect> {
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
    if !allowed_from(&turn.playback) {
        return vec![Effect::Dropped {
            reason: DropReason::TurnFactMismatch { fact },
        }];
    }
    turn.playback = match fact {
        "playback_started" => PlaybackState::Playing,
        "playback_drained" => PlaybackState::Drained,
        _ => PlaybackState::Cleared,
    };
    Vec::new()
}

/// 双闩检查：两闩同时成立 ⇒ 清 turn、回 Idle、发恰好一个
/// [`Effect::TurnCompleted`]（P0-3：生成结束 ≠ turn 完成）。
/// 以 `prefix` 为前缀返回（保持「一次 apply 一个效果向量」的风格）。
fn maybe_finish_turn(prefix: Vec<Effect>, state: &mut State) -> Vec<Effect> {
    let Some(turn) = &state.active_turn else {
        return prefix;
    };
    let Some(outcome) = turn.generation else {
        return prefix;
    };
    if !turn.playback.is_terminal() {
        return prefix;
    }
    let (turn_id, outcome) = (turn.id, outcome);
    state.active_turn = None;
    state.phase = Phase::Idle;
    let mut effects = prefix;
    effects.push(Effect::TurnCompleted { turn_id, outcome });
    effects.push(Effect::GoIdle);
    effects
}

/// 会话停止：原子清空 turn/动作并推进 epoch 恰好一次。
/// 播放侧清理由 shell 执行（[`Effect::StopPlayback`] 无条件发出，对空 ring 幂等）。
fn stop(state: &mut State) -> Vec<Effect> {
    state.epoch = state.epoch.next();
    let aborted_turn = state.active_turn.take();
    state.phase = Phase::Idle;
    let mut effects = vec![Effect::StopPlayback, Effect::GoIdle];
    if let Some(action) = state.action.current.take() {
        effects.push(Effect::Action(crate::action::ActionEffect::End { action }));
    }
    if let Some(turn) = aborted_turn {
        effects.push(Effect::TurnAborted {
            turn_id: turn.id,
            epoch: state.epoch,
        });
    }
    effects
}
