//! 根级集成测试：跨子系统场景，证明根 `State` 是唯一 epoch owner。
//!
//! 覆盖：stop 不漂移 epoch（含 ID 重用隔离）、优先级仲裁经根事件生效、
//! release/interrupt 经根事件收尾、多句 turn、**turn 终态双闩锁**
//! （生成终态 + 播放终态，节点 A P0-1/P0-3）。

use live2d_ai_core::{
    ActionCommand, ActionDropReason, ActionEffect, ActionId, ActionSource, DropReason, Effect,
    Epoch, Event, GenerationOutcome, ModelCapabilities, SemanticAction, State, Strength, apply,
};

const E0: Epoch = Epoch::ZERO;

fn act(id: ActionId, strength: Strength, source: ActionSource) -> SemanticAction {
    SemanticAction::new(id, strength, source)
}

fn play(epoch: u64, action: SemanticAction) -> Event {
    Event::Action {
        epoch: epoch.into(),
        command: ActionCommand::Play { action },
    }
}

/// 以当前 epoch 盖章提交文本。
fn submit(state: &mut State, turn: u64, sentence: u64) -> Vec<Effect> {
    let event = Event::UserSubmitted {
        epoch: state.epoch,
        turn_id: turn.into(),
        sentence_id: sentence.into(),
        text: String::new(),
    };
    apply(state, event)
}

/// 以当前 epoch 盖章追加句子。
fn add_sentence(state: &mut State, turn: u64, sentence_id: u64) -> Vec<Effect> {
    let event = Event::SentenceAdded {
        epoch: state.epoch,
        turn_id: turn.into(),
        sentence_id: sentence_id.into(),
    };
    apply(state, event)
}

/// 以当前 epoch 盖章宣告播放开始（首个非空 PCM 块成功入环的事实）。
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

/// 一个已就绪的多句会话：turn#1 含句 1/2/3，播放已开始（Speaking）。
fn multi_sentence_state_speaking() -> State {
    let mut state = State::default();
    assert_eq!(
        submit(&mut state, 1, 1),
        vec![Effect::StartThinking { turn_id: 1.into() }]
    );
    add_sentence(&mut state, 1, 2);
    add_sentence(&mut state, 1, 3);
    // 播放事实只翻相位，不产生效果（PCM 由 supervisor 直接入环）。
    assert_eq!(started(&mut state, 1), Vec::<Effect>::new());
    assert_eq!(state.phase, live2d_ai_core::Phase::Speaking);
    state
}

/// stop 推进 epoch **恰好一次**并原子清空全部子系统；此后旧代次的
/// 播放/动作/turn 迟到事件全部被根闸门丢弃，新代次用同样的 id 一切照常
/// —— 无 epoch 漂移。
#[test]
fn stop_does_not_drift_epoch_and_gates_every_subsystem() {
    let mut state = multi_sentence_state_speaking();
    state.action.capabilities = ModelCapabilities::all();
    let nod = act(ActionId::Nod, Strength::Low, ActionSource::LlmTool);
    assert_eq!(
        apply(&mut state, play(0, nod)),
        vec![Effect::Action(ActionEffect::Start { action: nod })]
    );

    // ---- stop：一次事件清空播放簿记 + 动作 + turn，epoch 只 +1。----
    let effects = apply(&mut state, Event::StopRequested);
    assert_eq!(state.epoch, Epoch::from(1));
    assert_eq!(state.phase, live2d_ai_core::Phase::Idle);
    assert!(state.active_turn.is_none());
    assert!(state.action.current.is_none());
    assert_eq!(
        effects,
        vec![
            Effect::StopPlayback,
            Effect::GoIdle,
            Effect::Action(ActionEffect::End { action: nod }),
            Effect::TurnAborted {
                turn_id: 1.into(),
                epoch: 1.into(),
            },
        ]
    );

    // ---- 旧代次（epoch 0）迟到事件：全部 StaleEpoch 且状态逐字节不变。----
    let snapshot = state.clone();
    let late = vec![
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
        Event::GenerationFinished {
            epoch: E0,
            turn_id: 1.into(),
            outcome: GenerationOutcome::Completed,
        },
        play(0, nod),
        Event::Action {
            epoch: E0,
            command: ActionCommand::Release,
        },
        Event::SentenceAdded {
            epoch: E0,
            turn_id: 1.into(),
            sentence_id: 4.into(),
        },
        Event::UserSubmitted {
            epoch: E0,
            turn_id: 2.into(),
            sentence_id: 1.into(),
            text: String::new(),
        },
    ];
    for event in late {
        let event_epoch = event.epoch().unwrap();
        assert_eq!(
            apply(&mut state, event.clone()),
            vec![Effect::Dropped {
                reason: DropReason::StaleEpoch {
                    event_epoch,
                    current_epoch: 1.into(),
                }
            }]
        );
        assert_eq!(state, snapshot, "stale event must not change anything");
    }

    // ---- 新代次：完全重用的 id 一切照常。----
    assert_eq!(
        submit(&mut state, 1, 1),
        vec![Effect::StartThinking { turn_id: 1.into() }]
    );
    add_sentence(&mut state, 1, 2);
    assert_eq!(started(&mut state, 1), Vec::<Effect>::new()); // id 重用照常起播
    assert_eq!(drained(&mut state, 1).len(), 0); // 无生成闩 → 只落播放闩，不终态
    assert_eq!(
        apply(&mut state, play(1, nod)), // 新代次的表演正常起播
        vec![Effect::Action(ActionEffect::Start { action: nod })]
    );
    assert_eq!(state.epoch, Epoch::from(1)); // 全程未再推进
}

/// 优先级经根事件仲裁：rule 不能抢 tool，user 用单一 Transition 抢占 tool。
#[test]
fn priority_arbitration_flows_through_root_events() {
    let mut state = State::default();
    state.action.capabilities = ModelCapabilities::all();
    submit(&mut state, 1, 1);

    let tool_nod = act(ActionId::Nod, Strength::Low, ActionSource::LlmTool);
    assert_eq!(
        apply(&mut state, play(0, tool_nod)),
        vec![Effect::Action(ActionEffect::Start { action: tool_nod })]
    );

    // rule（50）不能抢 tool（90）。
    let rule_shake = act(ActionId::ShakeNo, Strength::Low, ActionSource::RuleFallback);
    assert_eq!(
        apply(&mut state, play(0, rule_shake)),
        vec![Effect::Action(ActionEffect::Dropped {
            reason: ActionDropReason::LowerPriority {
                incoming: rule_shake,
                active: tool_nod,
            }
        })]
    );
    assert_eq!(state.action.current, Some(tool_nod));

    // user（100）抢占 tool（90）：单一 Transition 效果。
    let user_tilt = act(ActionId::Tilt, Strength::Medium, ActionSource::UserCommand);
    assert_eq!(
        apply(&mut state, play(0, user_tilt)),
        vec![Effect::Action(ActionEffect::Transition {
            from: tool_nod,
            to: user_tilt,
        })]
    );
    assert_eq!(state.action.current, Some(user_tilt));

    // 规则 fallback 产出的动作也走同一入口（rules → Play 的集成路径）：
    // "yes!" 命中 Nod(Low, RuleFallback)，但 user 正在表演 → LowerPriority。
    let from_rule = live2d_ai_core::action::rule_fallback("yes!", &ModelCapabilities::all())
        .expect("yes! nods");
    assert_eq!(
        apply(&mut state, play(0, from_rule)),
        vec![Effect::Action(ActionEffect::Dropped {
            reason: ActionDropReason::LowerPriority {
                incoming: from_rule,
                active: user_tilt,
            }
        })]
    );
}

/// release / interrupt 经根事件收尾：恰好一个 End 效果，**不**推进 epoch；
/// 播放侧簿记不受影响；之后 stop 仍能原子清理剩余子系统。
#[test]
fn release_and_interrupt_end_actions_without_epoch_bump() {
    let mut state = multi_sentence_state_speaking(); // 无动作在演
    state.action.capabilities = ModelCapabilities::all();

    let nod = act(ActionId::Nod, Strength::Low, ActionSource::LlmTool);
    apply(&mut state, play(0, nod));

    // release：一个 End；播放簿记与 epoch 完全不受影响。
    let epoch = state.epoch;
    let effects = apply(
        &mut state,
        Event::Action {
            epoch,
            command: ActionCommand::Release,
        },
    );
    assert_eq!(
        effects,
        vec![Effect::Action(ActionEffect::End { action: nod })]
    );
    assert!(state.action.current.is_none());
    assert_eq!(state.phase, live2d_ai_core::Phase::Speaking);
    assert_eq!(state.epoch, E0);

    // interrupt：同样一个 End。
    let tilt = act(ActionId::Tilt, Strength::High, ActionSource::UserCommand);
    apply(&mut state, play(0, tilt));
    let effects = apply(
        &mut state,
        Event::Action {
            epoch,
            command: ActionCommand::Interrupt,
        },
    );
    assert_eq!(
        effects,
        vec![Effect::Action(ActionEffect::End { action: tilt })]
    );
    assert_eq!(state.epoch, E0);

    // 最后 stop 一次性收掉剩余的 turn + 播放簿记。
    let effects = apply(&mut state, Event::StopRequested);
    assert_eq!(state.epoch, Epoch::from(1));
    assert_eq!(
        effects,
        vec![
            Effect::StopPlayback,
            Effect::GoIdle,
            Effect::TurnAborted {
                turn_id: 1.into(),
                epoch: 1.into(),
            },
        ]
    );
}

/// 端到端双闩锁：流式时序（Started → 生成完成 → Drained）下 turn 恰好一次
/// 收口，且完成后可开下一轮——「Engine Done ≠ turn 完成」的集成证据。
#[test]
fn happy_path_double_latch_then_next_turn() {
    let mut state = multi_sentence_state_speaking();

    // 生成先于播放结束：turn 必须存活（等待 DAC 排空）。
    let gen_evt = Event::GenerationFinished {
        epoch: state.epoch,
        turn_id: 1.into(),
        outcome: GenerationOutcome::Completed,
    };
    assert_eq!(apply(&mut state, gen_evt.clone()), Vec::<Effect>::new());
    assert!(state.active_turn.is_some());

    // 排空 ⇒ 恰好一次 TurnCompleted + GoIdle。
    let effects = drained(&mut state, 1);
    assert_eq!(
        effects,
        vec![
            Effect::TurnCompleted {
                turn_id: 1.into(),
                outcome: GenerationOutcome::Completed,
            },
            Effect::GoIdle
        ]
    );
    assert_eq!(state.phase, live2d_ai_core::Phase::Idle);

    // 完成后可开下一轮（pending Say 启动的前提条件）。
    assert_eq!(
        submit(&mut state, 2, 9),
        vec![Effect::StartThinking { turn_id: 2.into() }]
    );

    // 上一轮迟到的重复 GenerationFinished（同 id 同代次不可能重现，
    // 这里用旧轮 id）→ UnknownTurn，不污染新一轮。
    assert_eq!(
        apply(&mut state, gen_evt),
        vec![Effect::Dropped {
            reason: DropReason::UnknownTurn { turn_id: 1.into() }
        }]
    );
    assert!(state.active_turn.is_some());
}
