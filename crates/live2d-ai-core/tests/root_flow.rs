//! 根级集成测试：跨子系统场景，证明根 `State` 是唯一 epoch owner。
//!
//! 覆盖：stop 不漂移 epoch（含 ID 重用隔离）、优先级仲裁经根事件生效、
//! release/interrupt 经根事件收尾、多句 turn、音频队列串行。

use live2d_ai_core::{
    ActionCommand, ActionDropReason, ActionEffect, ActionId, ActionSource, DropReason, Effect,
    Epoch, Event, ModelCapabilities, SemanticAction, State, Strength, apply,
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

/// 以当前 epoch 盖章宣告音频块就绪。
fn ready(state: &mut State, turn: u64, sentence: u64, chunk: u64) -> Vec<Effect> {
    let event = Event::AudioChunkReady {
        epoch: state.epoch,
        turn_id: turn.into(),
        sentence_id: sentence.into(),
        chunk_id: chunk.into(),
    };
    apply(state, event)
}

/// 以当前 epoch 盖章宣告播放完成。
fn done(state: &mut State, chunk: u64) -> Vec<Effect> {
    let event = Event::AudioChunkFinished {
        epoch: state.epoch,
        chunk_id: chunk.into(),
    };
    apply(state, event)
}

/// 一个已就绪的多句会话：turn#1 含句 1/2/3，chunk 10 播放中、11/12 排队。
fn multi_sentence_state_with_queue() -> State {
    let mut state = State::default();
    assert_eq!(
        submit(&mut state, 1, 1),
        vec![Effect::StartThinking { turn_id: 1.into() }]
    );
    add_sentence(&mut state, 1, 2);
    add_sentence(&mut state, 1, 3);
    assert_eq!(
        ready(&mut state, 1, 1, 10),
        vec![Effect::PlayAudio {
            chunk_id: 10.into()
        }]
    );
    // 忙碌时后续块静默入队（无 PlayAudio）。
    assert_eq!(ready(&mut state, 1, 2, 11), Vec::<Effect>::new());
    assert_eq!(ready(&mut state, 1, 3, 12), Vec::<Effect>::new());
    assert_eq!(state.active_audio, Some(10_u64.into()));
    assert_eq!(state.pending_audio, [11_u64.into(), 12_u64.into()]);
    state
}

/// stop 推进 epoch **恰好一次**并原子清空全部子系统；此后旧代次的
/// 音频/动作/turn 迟到事件全部被根闸门丢弃，新代次用同样的 id 一切照常
/// —— 无 epoch 漂移。
#[test]
fn stop_does_not_drift_epoch_and_gates_every_subsystem() {
    let mut state = multi_sentence_state_with_queue();
    state.action.capabilities = ModelCapabilities::all();
    let nod = act(ActionId::Nod, Strength::Low, ActionSource::LlmTool);
    assert_eq!(
        apply(&mut state, play(0, nod)),
        vec![Effect::Action(ActionEffect::Start { action: nod })]
    );

    // ---- stop：一次事件清空 audio 队列 + 动作 + turn，epoch 只 +1。----
    let effects = apply(&mut state, Event::StopRequested);
    assert_eq!(state.epoch, Epoch::from(1));
    assert_eq!(state.phase, live2d_ai_core::Phase::Idle);
    assert!(state.active_turn.is_none());
    assert_eq!(state.active_audio, None);
    assert!(state.pending_audio.is_empty());
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
        Event::AudioChunkReady {
            epoch: E0,
            turn_id: 1.into(),
            sentence_id: 1.into(),
            chunk_id: 13.into(),
        },
        Event::AudioChunkFinished {
            epoch: E0,
            chunk_id: 10.into(),
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
        Event::TurnFinished {
            epoch: E0,
            turn_id: 1.into(),
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
    assert_eq!(
        ready(&mut state, 1, 1, 10), // chunk id 重用 → 立刻开播
        vec![Effect::PlayAudio {
            chunk_id: 10.into()
        }]
    );
    assert_eq!(
        done(&mut state, 10), // 新代次的完成正常消费
        Vec::<Effect>::new()
    );
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
/// 音频队列不受影响继续串行；之后 stop 仍能原子清理剩余子系统。
#[test]
fn release_and_interrupt_end_actions_without_epoch_bump() {
    let mut state = multi_sentence_state_with_queue(); // 无动作在演
    state.action.capabilities = ModelCapabilities::all();

    let nod = act(ActionId::Nod, Strength::Low, ActionSource::LlmTool);
    apply(&mut state, play(0, nod));

    // release：一个 End；audio 队列与 epoch 完全不受影响。
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
    assert_eq!(state.active_audio, Some(10_u64.into()));
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

    // 音频队列继续串行：front 完成 → 启动下一块。
    assert_eq!(
        done(&mut state, 10),
        vec![Effect::PlayAudio {
            chunk_id: 11.into()
        }]
    );

    // 最后 stop 一次性收掉剩余的 turn + audio。
    let effects = apply(&mut state, Event::StopRequested);
    assert_eq!(state.epoch, Epoch::from(1));
    assert_eq!(state.active_audio, None);
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

/// 多句 + 队列串行端到端：块按计划顺序串行消费，重复就绪被拒，
/// TurnFinished 后可开下一轮。
#[test]
fn multi_sentence_turn_serializes_audio_in_plan_order() {
    let mut state = multi_sentence_state_with_queue();

    // 依次完成 front 块：10 → 11 → 12，每步只启动下一块。
    assert_eq!(
        done(&mut state, 10),
        vec![Effect::PlayAudio {
            chunk_id: 11.into()
        }]
    );
    assert_eq!(
        done(&mut state, 11),
        vec![Effect::PlayAudio {
            chunk_id: 12.into()
        }]
    );

    // 中途重复就绪正在播的 12 → DuplicateChunk。
    assert_eq!(
        ready(&mut state, 1, 3, 12),
        vec![Effect::Dropped {
            reason: DropReason::DuplicateChunk {
                chunk_id: 12.into()
            }
        }]
    );

    assert_eq!(done(&mut state, 12), Vec::<Effect>::new());

    // TurnFinished 收尾：无残留音频 → 仅 GoIdle，可开下一轮。
    let turn_finished = Event::TurnFinished {
        epoch: state.epoch,
        turn_id: 1.into(),
    };
    assert_eq!(apply(&mut state, turn_finished), vec![Effect::GoIdle]);
    assert_eq!(
        submit(&mut state, 2, 9),
        vec![Effect::StartThinking { turn_id: 2.into() }]
    );
}
