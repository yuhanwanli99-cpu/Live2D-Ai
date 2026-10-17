//! `AppEvent::Conversation` → Mod 事件投影（2026-10-17 从 `supervisor.rs` 拆出）。
//!
//! 拆出原因：3.0 队首第一片新增 `turn_in_flight` 标志后 `supervisor.rs` 涨到
//! 507 行，跨过「源码 ≤500 行」棘轮（`code-stats --only lines` 的 >500 计数
//! 42 → 43）。这里承接纯函数 `project_conversation_to_mod` + 它的单测，
//! **行为一行未改**。

use live2d_ai_mod_system::ModEventTopic;

use crate::app_event::{AppEvent, ConversationUiEvent};

/// 把 `AppEvent::Conversation` 投影为对应的 Mod 事件（纯函数，便于单测）。
///
/// 返回 `Some((ModEventTopic, payload))` 时，调用方经 `mod_events` 桥（`dispatch_event`）
/// 投递给 Mod worker。三变体：
/// - `TextDelta` → `ModEventTopic::TextDelta`（payload = 增量文本）;
/// - `VoiceStarted` → `ModEventTopic::VoiceStarted`（payload = epoch 字符串）;
/// - `VoiceEnded` → `ModEventTopic::VoiceEnded`（payload = epoch 字符串）。
///
/// 非 Conversation 变体（Render / RootAudit / Tray / ShutdownReady）返回 `None`，
/// 因为 Mod 事件主题只覆盖对话/动作生命周期（见 topics.rs）。
pub(crate) fn project_conversation_to_mod(ev: &AppEvent) -> Option<(ModEventTopic, String)> {
    match ev {
        AppEvent::Conversation(ConversationUiEvent::TextDelta { text, .. }) => {
            Some((ModEventTopic::TextDelta, text.clone()))
        }
        AppEvent::Conversation(ConversationUiEvent::VoiceStarted { epoch }) => {
            Some((ModEventTopic::VoiceStarted, epoch.to_string()))
        }
        AppEvent::Conversation(ConversationUiEvent::VoiceEnded { epoch }) => {
            Some((ModEventTopic::VoiceEnded, epoch.to_string()))
        }
        _ => None,
    }
}

/// `project_conversation_to_mod` 投影语义（AppEvent::Conversation → ModEventTopic）。
#[cfg(test)]
mod tests_mod_projections {
    use super::*;

    #[test]
    fn text_delta_projects_to_text_delta_with_payload() {
        let ev = AppEvent::Conversation(ConversationUiEvent::TextDelta {
            epoch: 3,
            ts_ms: 42,
            sentence_seq: 1,
            text: "你好".to_string(),
        });
        assert_eq!(
            project_conversation_to_mod(&ev),
            Some((ModEventTopic::TextDelta, "你好".to_string()))
        );
    }

    #[test]
    fn voice_started_projects_with_epoch_payload() {
        let ev = AppEvent::Conversation(ConversationUiEvent::VoiceStarted { epoch: 7 });
        assert_eq!(
            project_conversation_to_mod(&ev),
            Some((ModEventTopic::VoiceStarted, "7".to_string()))
        );
    }

    #[test]
    fn voice_ended_projects_with_epoch_payload() {
        let ev = AppEvent::Conversation(ConversationUiEvent::VoiceEnded { epoch: 9 });
        assert_eq!(
            project_conversation_to_mod(&ev),
            Some((ModEventTopic::VoiceEnded, "9".to_string()))
        );
    }

    #[test]
    fn non_conversation_events_yield_none() {
        // 2026-09-11：`AppEvent::Render` 已随动作系统删除。
        // 2026-10-01（W2-B）：`AppEvent::Tray(PetUserEvent)` 随原生壳移出而删除，
        // 这里改用剩下的两个非 Conversation 变体覆盖同一「不应投影」语义。
        let audit = AppEvent::RootAudit(crate::app_event::RootFact::Dropped);
        let cleared = AppEvent::RootAudit(crate::app_event::RootFact::PlaybackCleared { epoch: 1 });
        let shutdown = AppEvent::ShutdownReady;
        for ev in [audit, cleared, shutdown] {
            assert_eq!(
                project_conversation_to_mod(&ev),
                None,
                "非 Conversation 事件不应投影"
            );
        }
    }
}
