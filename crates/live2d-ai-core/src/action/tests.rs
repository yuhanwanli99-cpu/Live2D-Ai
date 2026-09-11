//! 动作子系统的契约守护单测：Strength 协议映射、ActionId 协议名 / 优先级、
//! 能力集合 fail-closed、Play/End 仲裁（含 capability / 幂等 / 优先级 / 抢占）。

use super::{
    ActionCommand, ActionDropReason, ActionEffect, ActionId, ActionSource, ActionState,
    ModelCapabilities, SemanticAction, Strength, apply_action,
};

fn action(id: ActionId, strength: Strength, source: ActionSource) -> SemanticAction {
    SemanticAction::new(id, strength, source)
}

fn state_with(capabilities: ModelCapabilities) -> ActionState {
    ActionState {
        capabilities,
        current: None,
    }
}

fn dropped(reason: ActionDropReason) -> Vec<ActionEffect> {
    vec![ActionEffect::Dropped { reason }]
}

// ------------------------------------------------ Strength 1..=3 映射

#[test]
fn strength_maps_to_levels_1_to_3() {
    assert_eq!(Strength::ALL.len(), 3);
    for (i, s) in Strength::ALL.iter().enumerate() {
        assert_eq!(s.level(), (i + 1) as u8);
        assert_eq!(Strength::from_level((i + 1) as u8), Some(*s));
    }
    assert_eq!(Strength::default(), Strength::Low);
    assert_eq!(Strength::from_level(0), None);
    assert_eq!(Strength::from_level(4), None);
    assert_eq!(Strength::from_level(255), None);
}

// ------------------------------------------------ ActionId / 优先级

#[test]
fn action_id_names_match_rfc_without_release() {
    let names: Vec<&str> = ActionId::ALL.iter().map(|a| a.name()).collect();
    assert_eq!(
        names,
        vec![
            "nod",
            "shake_no",
            "tilt",
            "look_around",
            "listen",
            "surprise"
        ]
    );
    assert_eq!(ActionId::ALL.len(), 6);
}

#[test]
fn source_priorities_are_fixed_and_ordered() {
    assert_eq!(
        (
            ActionSource::UserCommand.priority(),
            ActionSource::LlmTool.priority(),
            ActionSource::RuleFallback.priority()
        ),
        (100, 90, 50)
    );
    assert!(ActionSource::UserCommand.priority() > ActionSource::LlmTool.priority());
    assert!(ActionSource::LlmTool.priority() > ActionSource::RuleFallback.priority());
}

// ------------------------------------------------ 能力集合

#[test]
fn capability_set_is_fail_closed_by_default() {
    let mut caps = ModelCapabilities::EMPTY;
    assert!(!caps.supports(ActionId::Nod));
    caps.add(ActionId::Nod);
    caps.add(ActionId::Surprise);
    assert!(caps.supports(ActionId::Nod));
    assert!(!caps.supports(ActionId::Tilt));

    // with() 不改动原集合。
    let chained = caps.with(ActionId::Tilt);
    assert!(chained.supports(ActionId::Tilt));
    assert!(!caps.supports(ActionId::Tilt));

    let all = ModelCapabilities::all();
    assert!(ActionId::ALL.iter().all(|a| all.supports(*a)));
}

#[test]
fn capability_missing_drops_play_even_for_user() {
    let caps = ModelCapabilities::EMPTY
        .with(ActionId::Nod)
        .with(ActionId::Tilt);
    let incoming = action(
        ActionId::Surprise,
        Strength::High,
        ActionSource::UserCommand,
    );
    let mut s = state_with(caps);
    assert_eq!(
        apply_action(&mut s, ActionCommand::Play { action: incoming }),
        dropped(ActionDropReason::CapabilityMissing { action: incoming })
    );
    assert_eq!(s.current, None);
}

// ------------------------------------------------ 优先级仲裁

#[test]
fn user_preempts_tool_with_single_transition() {
    let mut s = state_with(ModelCapabilities::all());
    let tool_nod = action(ActionId::Nod, Strength::Low, ActionSource::LlmTool);

    assert_eq!(
        apply_action(&mut s, ActionCommand::Play { action: tool_nod }),
        vec![ActionEffect::Start { action: tool_nod }]
    );

    // user shake_no（100 > 90）抢占 tool nod：单一 Transition 效果。
    let user_shake = action(
        ActionId::ShakeNo,
        Strength::Medium,
        ActionSource::UserCommand,
    );
    assert_eq!(
        apply_action(&mut s, ActionCommand::Play { action: user_shake }),
        vec![ActionEffect::Transition {
            from: tool_nod,
            to: user_shake,
        }]
    );
    assert_eq!(s.current, Some(user_shake));
}

#[test]
fn lower_priority_cannot_preempt() {
    let mut s = state_with(ModelCapabilities::all());
    let tool_nod = action(ActionId::Nod, Strength::Low, ActionSource::LlmTool);
    let rule_shake = action(ActionId::ShakeNo, Strength::Low, ActionSource::RuleFallback);

    apply_action(&mut s, ActionCommand::Play { action: tool_nod });
    assert_eq!(
        apply_action(&mut s, ActionCommand::Play { action: rule_shake }),
        dropped(ActionDropReason::LowerPriority {
            incoming: rule_shake,
            active: tool_nod,
        })
    );
    assert_eq!(s.current, Some(tool_nod));

    // user nod 表演中：tool 也不能抢（90 < 100）。
    let user_nod = action(ActionId::Nod, Strength::High, ActionSource::UserCommand);
    apply_action(&mut s, ActionCommand::Play { action: user_nod });
    let tool_tilt = action(ActionId::Tilt, Strength::Medium, ActionSource::LlmTool);
    assert_eq!(
        apply_action(&mut s, ActionCommand::Play { action: tool_tilt }),
        dropped(ActionDropReason::LowerPriority {
            incoming: tool_tilt,
            active: user_nod,
        })
    );
    assert_eq!(s.current, Some(user_nod));
}

#[test]
fn equal_priority_latest_wins_but_identical_is_idempotent() {
    let mut s = state_with(ModelCapabilities::all());
    let user_nod = action(ActionId::Nod, Strength::Low, ActionSource::UserCommand);

    apply_action(&mut s, ActionCommand::Play { action: user_nod });
    let user_tilt = action(ActionId::Tilt, Strength::Medium, ActionSource::UserCommand);
    assert_eq!(
        apply_action(&mut s, ActionCommand::Play { action: user_tilt }),
        vec![ActionEffect::Transition {
            from: user_nod,
            to: user_tilt,
        }]
    );

    // 完全相同指令：幂等拒绝。
    assert_eq!(
        apply_action(&mut s, ActionCommand::Play { action: user_tilt }),
        dropped(ActionDropReason::AlreadyActive { action: user_tilt })
    );
    assert_eq!(s.current, Some(user_tilt));

    // 同动作不同强度：不算相同，后来者胜。
    let louder = action(ActionId::Tilt, Strength::High, ActionSource::UserCommand);
    assert_eq!(
        apply_action(&mut s, ActionCommand::Play { action: louder }),
        vec![ActionEffect::Transition {
            from: user_tilt,
            to: louder,
        }]
    );
}

// ------------------------------------------------ release / interrupt

#[test]
fn release_and_interrupt_both_end_with_single_effect() {
    let mut s = state_with(ModelCapabilities::all());
    // 空闲时 release/interrupt 是无操作。
    assert_eq!(apply_action(&mut s, ActionCommand::Release), Vec::new());
    assert_eq!(apply_action(&mut s, ActionCommand::Interrupt), Vec::new());

    let tool_nod = action(ActionId::Nod, Strength::Low, ActionSource::LlmTool);
    apply_action(&mut s, ActionCommand::Play { action: tool_nod });
    assert_eq!(
        apply_action(&mut s, ActionCommand::Release),
        vec![ActionEffect::End { action: tool_nod }]
    );
    assert_eq!(s.current, None);

    let tool_tilt = action(ActionId::Tilt, Strength::Low, ActionSource::LlmTool);
    apply_action(&mut s, ActionCommand::Play { action: tool_tilt });
    assert_eq!(
        apply_action(&mut s, ActionCommand::Interrupt),
        vec![ActionEffect::End { action: tool_tilt }]
    );
    assert_eq!(s.current, None);
}
