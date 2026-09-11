//! `live2d-ai-core` 单元测试（纯逻辑状态机的契约守护）。
//!
//! 关键不变量：根 epoch 闸门、双闩锁（生成闩 + 播放闩）、ID 重用隔离、
//! 动作自然完成身份校验（绝不在被抢占后误伤后来者）。
//!
//! 子模块布局：
//! - [`latches`]：双闩锁（生成闩 + 播放闩）与 turn 终结语义。
//! - [`actions`]：动作自然完成身份校验（防止被抢占后的迟到完成误伤在演后来者）。

mod actions;
mod latches;

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

/// 以当前 epoch 盖章宣告播放开始。
fn started(state: &mut State, turn: u64) -> Vec<Effect> {
    apply(
        state,
        Event::PlaybackStarted {
            epoch: state.epoch,
            turn_id: turn.into(),
        },
    )
}

/// 以当前 epoch 盖章宣告 DAC 排空。
fn drained(state: &mut State, turn: u64) -> Vec<Effect> {
    apply(
        state,
        Event::PlaybackDrained {
            epoch: state.epoch,
            turn_id: turn.into(),
        },
    )
}

/// 以当前 epoch 盖章宣告播放被清空（D8 策略事实）。
fn cleared(state: &mut State, turn: u64) -> Vec<Effect> {
    apply(
        state,
        Event::PlaybackCleared {
            epoch: state.epoch,
            turn_id: turn.into(),
        },
    )
}

/// 以当前 epoch 盖章宣告生成终态。
fn generation(state: &mut State, turn: u64, outcome: GenerationOutcome) -> Vec<Effect> {
    apply(
        state,
        Event::GenerationFinished {
            epoch: state.epoch,
            turn_id: turn.into(),
            outcome,
        },
    )
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

// ------------------------------------------------ 根闸门 / ID 隔离

/// 要求：stop 推进 epoch 恰好一次（可重复、单调）并原子清空全部子系统。
#[test]
fn stop_clears_everything_and_advances_epoch_once() {
    let mut state = State::default();
    submit(&mut state, 1, 1);
    started(&mut state, 1);
    state.action.capabilities = ModelCapabilities::all();
    apply(
        &mut state,
        Event::Action {
            epoch: E0,
            command: ActionCommand::Play {
                action: SemanticAction::new(ActionId::Nod, Strength::Low, ActionSource::LlmTool),
            },
        },
    );
    assert_eq!(state.phase, Phase::Speaking);
    assert!(state.action.current.is_some());

    let effects = apply(&mut state, Event::StopRequested);
    assert_eq!(state.epoch, Epoch::from(1));
    assert_eq!(state.phase, Phase::Idle);
    assert!(state.active_turn.is_none());
    assert!(state.action.current.is_none());
    assert_eq!(
        effects,
        vec![
            Effect::StopPlayback,
            Effect::GoIdle,
            Effect::Action(ActionEffect::End {
                action: SemanticAction::new(ActionId::Nod, Strength::Low, ActionSource::LlmTool)
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
    started(&mut state, 1);
    apply(&mut state, Event::StopRequested); // epoch → 1
    let snapshot = state.clone();

    let cases = vec![
        Event::SentenceAdded {
            epoch: E0,
            turn_id: 1.into(),
            sentence_id: 2.into(),
        },
        Event::GenerationFinished {
            epoch: E0,
            turn_id: 1.into(),
            outcome: GenerationOutcome::Completed,
        },
        Event::PlaybackStarted {
            epoch: E0,
            turn_id: 1.into(),
        },
        Event::PlaybackDrained {
            epoch: E0,
            turn_id: 1.into(),
        },
        Event::PlaybackCleared {
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
            Event::GenerationFinished {
                epoch: 2.into(),
                turn_id: 9.into(),
                outcome: GenerationOutcome::Completed,
            }
        ),
        stale(2, 1)
    );
}

/// 核心场景：ID 重用后，旧代次迟到事件不得污染新代次会话。
#[test]
fn reused_ids_across_epochs_are_isolated_by_epoch_gate() {
    let mut state = State::default();

    // 代次 0：turn#7 句#1，播放已开始。
    submit(&mut state, 7, 1);
    started(&mut state, 7);
    assert_eq!(state.phase, Phase::Speaking);
    apply(&mut state, Event::StopRequested); // epoch → 1，全部清空

    // 代次 1：shell 重用相同数值 turn#7 / 句#1 开新会话——合法。
    assert_eq!(
        submit(&mut state, 7, 1),
        vec![Effect::StartThinking {
            turn_id: TurnId::from(7)
        }]
    );

    // 旧代次迟到的播放事实：turn id 完全命中当前 turn，但 epoch=0 → 必须丢弃，
    // 不得把 Thinking 翻成 Speaking、不得落任何闩。
    assert_eq!(
        apply(
            &mut state,
            Event::PlaybackStarted {
                epoch: E0,
                turn_id: 7.into()
            }
        ),
        stale(0, 1)
    );
    assert_eq!(
        apply(
            &mut state,
            Event::PlaybackDrained {
                epoch: E0,
                turn_id: 7.into()
            }
        ),
        stale(0, 1)
    );
    assert_eq!(
        apply(
            &mut state,
            Event::GenerationFinished {
                epoch: E0,
                turn_id: 7.into(),
                outcome: GenerationOutcome::Completed,
            }
        ),
        stale(0, 1)
    );
    assert_eq!(state.phase, Phase::Thinking);
    let turn = state.active_turn.as_ref().unwrap();
    assert_eq!(turn.playback, PlaybackState::NeverStarted);
    assert_eq!(turn.generation, None);

    // 新代次的同 id 事实正常消费。
    let epoch1 = state.epoch;
    assert_eq!(started(&mut state, 7), Vec::<Effect>::new());
    assert_eq!(state.phase, Phase::Speaking);
    assert_eq!(
        apply(
            &mut state,
            Event::GenerationFinished {
                epoch: epoch1,
                turn_id: 7.into(),
                outcome: GenerationOutcome::Completed,
            }
        ),
        Vec::<Effect>::new()
    );
    // 生成已完成但仍在 Playing → turn 必须存活（P0-3 双闩锁核心语义）。
    assert!(state.active_turn.is_some());

    // 新代次真正的排空信号 → 恰好一次 TurnCompleted。
    let effects = drained(&mut state, 7);
    assert_eq!(
        effects,
        vec![
            Effect::TurnCompleted {
                turn_id: TurnId::from(7),
                outcome: GenerationOutcome::Completed,
            },
            Effect::GoIdle
        ]
    );
    assert!(state.active_turn.is_none());
    assert_eq!(state.phase, Phase::Idle);
}

// ------------------------------------------------ 工具

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
