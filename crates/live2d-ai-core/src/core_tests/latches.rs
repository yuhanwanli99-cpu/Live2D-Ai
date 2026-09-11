//! 双闩锁（生成闩 + 播放闩）相关契约守护：单主 turn、多句、生成/排空顺序、
//! Failed + Cleared 收口、事实重复/乱序拒绝、相位流转、未知 turn 引用。
//!
//! 测试与辅助函数（`submit` / `started` / `drained` / `cleared` /
//! `generation` / `dropped`）来自父模块 [`super`]；每个测试用绝对入口
//! 保持独立性。

use super::*;

/// 要求：单主 turn —— 活动 turn 存在时新 UserSubmitted 被丢弃；结束后可再开。
#[test]
fn single_main_turn() {
    let mut state = State::default();

    assert_eq!(
        super::submit(&mut state, 1, 1),
        vec![Effect::StartThinking {
            turn_id: TurnId::from(1)
        }]
    );

    let effects = super::submit(&mut state, 2, 1);
    assert_eq!(
        effects,
        super::dropped(DropReason::TurnAlreadyActive {
            incoming: TurnId::from(2),
            active: TurnId::from(1),
        })
    );
    assert_eq!(state.active_turn.as_ref().unwrap().id, TurnId::from(1));

    // 空回答路径：生成 Completed + 从未起播 ⇒ 双闩同时成立，立即完成。
    let effects = super::generation(&mut state, 1, GenerationOutcome::Completed);
    assert_eq!(
        effects,
        vec![
            Effect::TurnCompleted {
                turn_id: TurnId::from(1),
                outcome: GenerationOutcome::Completed,
            },
            Effect::GoIdle
        ]
    );
    assert_eq!(state.phase, Phase::Idle);

    assert_eq!(
        super::submit(&mut state, 3, 1),
        vec![Effect::StartThinking {
            turn_id: TurnId::from(3)
        }]
    );
}

/// 要求：多句 —— SentenceAdded 把后续句子追加进当前 turn 计划。
#[test]
fn sentence_added_extends_active_turn_plan() {
    let mut state = State::default();
    super::submit(&mut state, 1, 1);

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
        super::dropped(DropReason::UnknownTurn {
            turn_id: TurnId::from(42)
        })
    );
    assert_eq!(state.active_turn.unwrap().sentences.len(), 2);
}

/// P0-3 核心场景：**生成结束 ≠ turn 完成**——Completed 但 DAC 未排空时，
/// turn 必须保持存活，直到 Drained 才恰好产生一次 TurnCompleted。
#[test]
fn completed_generation_before_drain_keeps_turn_alive() {
    let mut state = State::default();
    super::submit(&mut state, 1, 1);
    super::started(&mut state, 1);
    assert_eq!(state.phase, Phase::Speaking);

    // 生成先于播放结束（流式合成的常态）。
    let effects = super::generation(&mut state, 1, GenerationOutcome::Completed);
    assert_eq!(effects, Vec::<Effect>::new());
    assert!(state.active_turn.is_some(), "播放未排空，turn 不得结束");
    assert_eq!(state.phase, Phase::Speaking);

    // 排空事实到达 ⇒ 恰好一次 TurnCompleted + GoIdle。
    let effects = super::drained(&mut state, 1);
    assert_eq!(
        effects,
        vec![
            Effect::TurnCompleted {
                turn_id: TurnId::from(1),
                outcome: GenerationOutcome::Completed,
            },
            Effect::GoIdle
        ]
    );
    assert!(state.active_turn.is_none());
    assert_eq!(state.phase, Phase::Idle);

    // 完成后可立即开下一轮（pending Say 由 supervisor 在此后启动）。
    assert_eq!(
        super::submit(&mut state, 2, 1),
        vec![Effect::StartThinking {
            turn_id: TurnId::from(2)
        }]
    );
}

/// D8 失败路径：supervisor 清环后报告 Cleared；与 Failed 生成终态一起
/// 构成双闩，turn 以 Failed 收口（不提交 history 是 supervisor 的职责）。
#[test]
fn failed_generation_with_cleared_playback_finishes_turn_as_failed() {
    let mut state = State::default();
    super::submit(&mut state, 1, 1);
    super::started(&mut state, 1);

    // 只有 Failed、环还没清 → 双闩未齐，turn 存活（等 supervisor 的事实）。
    super::generation(&mut state, 1, GenerationOutcome::Failed);
    assert!(state.active_turn.is_some());

    // supervisor 清环并报告事实 → 立即以 Failed 终态收口。
    let effects = super::cleared(&mut state, 1);
    assert_eq!(
        effects,
        vec![
            Effect::TurnCompleted {
                turn_id: TurnId::from(1),
                outcome: GenerationOutcome::Failed,
            },
            Effect::GoIdle
        ]
    );
    assert_eq!(state.phase, Phase::Idle);
}

/// 播放/生成事实的重复与乱序必须被确定性拒绝。
#[test]
fn duplicate_or_out_of_order_facts_are_rejected() {
    let mut state = State::default();
    super::submit(&mut state, 1, 1);

    // Drained 不能从 NeverStarted 到达（没起播就「排空」是矛盾事实）。
    assert_eq!(
        super::drained(&mut state, 1),
        super::dropped(DropReason::TurnFactMismatch {
            fact: "playback_drained",
        })
    );

    // Started 正常；重复 Started 拒绝且状态不变。
    super::started(&mut state, 1);
    let snapshot = state.clone();
    assert_eq!(
        super::started(&mut state, 1),
        super::dropped(DropReason::TurnFactMismatch {
            fact: "playback_started",
        })
    );
    assert_eq!(state, snapshot, "被拒事实不得改变状态");

    // 生成终态重复报告 → 拒绝。
    super::generation(&mut state, 1, GenerationOutcome::Completed);
    assert_eq!(
        super::generation(&mut state, 1, GenerationOutcome::Completed),
        super::dropped(DropReason::TurnFactMismatch {
            fact: "generation_finished",
        })
    );

    // 排空完成终态后，迟到的播放事实按 UnknownTurn 丢弃（turn 已清）。
    super::drained(&mut state, 1);
    assert!(state.active_turn.is_none());
    assert_eq!(
        super::started(&mut state, 1),
        super::dropped(DropReason::UnknownTurn {
            turn_id: TurnId::from(1)
        })
    );
}

/// 相位流转：Thinking →（PlaybackStarted）→ Speaking →（双闩齐）→ Idle；
/// stop 无条件发 StopPlayback（对空 ring 幂等）。
#[test]
fn phase_transitions_and_turn_finish_cleanup() {
    let mut state = State::default();
    super::submit(&mut state, 1, 1);
    assert_eq!(state.phase, Phase::Thinking);

    super::started(&mut state, 1);
    assert_eq!(state.phase, Phase::Speaking);

    // 生成完成但仍在播放 → 相位不变、turn 存活。
    super::generation(&mut state, 1, GenerationOutcome::Completed);
    assert_eq!(state.phase, Phase::Speaking);

    let effects = super::drained(&mut state, 1);
    assert_eq!(
        effects,
        vec![
            Effect::TurnCompleted {
                turn_id: TurnId::from(1),
                outcome: GenerationOutcome::Completed,
            },
            Effect::GoIdle
        ]
    );
    assert_eq!(state.phase, Phase::Idle);
    assert!(state.active_turn.is_none());
}

/// 引用未知 turn 的播放/生成事实 → 安全丢弃（epoch 相符前提下按 id 判废）；
/// stop 后旧代次的一切迟到事实同理（epoch 闸门先行）。
#[test]
fn unknown_references_are_dropped() {
    let mut state = State::default();
    super::submit(&mut state, 1, 1);
    let epoch = state.epoch;

    assert_eq!(
        apply(
            &mut state,
            Event::PlaybackStarted {
                epoch,
                turn_id: 42.into()
            }
        ), // 非当前 turn
        super::dropped(DropReason::UnknownTurn {
            turn_id: TurnId::from(42)
        })
    );
    assert_eq!(
        apply(
            &mut state,
            Event::GenerationFinished {
                epoch,
                turn_id: 42.into(),
                outcome: GenerationOutcome::Failed,
            }
        ), // 非当前 turn
        super::dropped(DropReason::UnknownTurn {
            turn_id: TurnId::from(42)
        })
    );
    // 无活动 turn 时同样丢弃。
    apply(&mut state, Event::StopRequested); // epoch → 1，turn 清空
    assert_eq!(
        apply(
            &mut state,
            Event::PlaybackDrained {
                epoch: E0, // 旧代次先撞 epoch 闸门
                turn_id: 1.into(),
            }
        ),
        super::stale(0, 1)
    );
    // 同代次（epoch 1）但 turn 已不存在 → UnknownTurn，状态不变。
    let snapshot = state.clone();
    assert_eq!(
        apply(
            &mut state,
            Event::PlaybackDrained {
                epoch: 1.into(),
                turn_id: 1.into(),
            }
        ),
        super::dropped(DropReason::UnknownTurn {
            turn_id: TurnId::from(1)
        })
    );
    assert_eq!(
        apply(
            &mut state,
            Event::PlaybackStarted {
                epoch: 1.into(),
                turn_id: 7.into(),
            }
        ),
        super::dropped(DropReason::UnknownTurn {
            turn_id: TurnId::from(7)
        })
    );
    assert_eq!(state, snapshot);
}
