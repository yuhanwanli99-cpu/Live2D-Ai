//! Mod 事件主题（host → Mod 的事件订阅源）。
//!
//! Mod 通过 [`crate::registry::ModRegistrar::subscribe`] 订阅这些主题。
//! host 把内部 AppEvent 投影到这些主题后经**有界 channel**投递给 Mod 的
//! 独立 worker（慢 Mod 不卡 supervisor，失败仅 disable）。

/// 可订阅的 Mod 事件主题。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModEventTopic {
    /// 一轮 turn 开始（文本进入 LLM）。
    TurnStarted,
    /// LLM 流式文本增量。
    TextDelta,
    /// 动作自然播放完成（含动作身份）。
    ActionFinished,
    /// 语音输出开始（口型活跃）。
    VoiceStarted,
    /// 语音输出结束（口型归零）。
    VoiceEnded,
    /// 模型激活发生切换。
    ModelActivated,
}

impl ModEventTopic {
    /// 全部主题（供 Mod 管理 UI / 发现）。
    pub const ALL: &'static [ModEventTopic] = &[
        ModEventTopic::TurnStarted,
        ModEventTopic::TextDelta,
        ModEventTopic::ActionFinished,
        ModEventTopic::VoiceStarted,
        ModEventTopic::VoiceEnded,
        ModEventTopic::ModelActivated,
    ];

    /// 稳定字符串 id（日志/JSON/前端用）。
    pub const fn as_str(self) -> &'static str {
        match self {
            ModEventTopic::TurnStarted => "turn_started",
            ModEventTopic::TextDelta => "text_delta",
            ModEventTopic::ActionFinished => "action_finished",
            ModEventTopic::VoiceStarted => "voice_started",
            ModEventTopic::VoiceEnded => "voice_ended",
            ModEventTopic::ModelActivated => "model_activated",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_topics_have_stable_ids() {
        for t in ModEventTopic::ALL {
            assert!(!t.as_str().is_empty());
        }
    }
}
