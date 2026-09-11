//! 动作子系统的根级契约守护：自然完成事实只在「与当前 active 完全一致」时
//! 收尾恰好一次；被抢占者 A 的迟到完成绝不能结束在演的后来者 B（P0-6）。

use super::*;

/// P0-6：自然完成事实只在「与当前 active 完全一致」时收尾恰好一次；
/// 被抢占者 A 的迟到完成绝不能结束在演的后来者 B。
#[test]
fn action_playback_finished_is_identity_checked() {
    let mut state = State::default();
    state.action.capabilities = ModelCapabilities::all();
    let nod = SemanticAction::new(ActionId::Nod, Strength::Low, ActionSource::LlmTool);
    let tilt = SemanticAction::new(ActionId::Tilt, Strength::High, ActionSource::LlmTool);

    // A（nod）起演 → 用户命令抢占为 B（tilt）：Transition 效果，A 已退场。
    apply(
        &mut state,
        Event::Action {
            epoch: E0,
            command: ActionCommand::Play { action: nod },
        },
    );
    assert_eq!(state.action.current, Some(nod));
    apply(
        &mut state,
        Event::Action {
            epoch: E0,
            command: ActionCommand::Play { action: tilt },
        },
    );
    assert_eq!(state.action.current, Some(tilt));

    // A 的迟到自然完成：身份不符 → StaleActionCompletion，B 完好。
    let effects = apply(
        &mut state,
        Event::ActionPlaybackFinished {
            epoch: E0,
            action: nod,
        },
    );
    assert_eq!(
        effects,
        super::dropped(DropReason::StaleActionCompletion {
            finished: nod,
            current: Some(tilt),
        })
    );
    assert_eq!(state.action.current, Some(tilt), "后来者不得被误伤");

    // B 自己的自然完成：身份一致 → 恰一个 End，动作清空。
    let effects = apply(
        &mut state,
        Event::ActionPlaybackFinished {
            epoch: E0,
            action: tilt,
        },
    );
    assert_eq!(
        effects,
        vec![Effect::Action(ActionEffect::End { action: tilt })]
    );
    assert!(state.action.current.is_none());

    // 完成事实重复到达（此刻空闲）→ 丢弃，状态不变。
    let effects = apply(
        &mut state,
        Event::ActionPlaybackFinished {
            epoch: E0,
            action: tilt,
        },
    );
    assert_eq!(
        effects,
        super::dropped(DropReason::StaleActionCompletion {
            finished: tilt,
            current: None,
        })
    );
    assert!(state.action.current.is_none());
}
