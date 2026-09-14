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
    /// **本轮的输入正文**（Wave 2 新增，2026-09-14）。
    ///
    /// 与 [`Self::TurnStarted`] 的差别：`TurnStarted` 的 payload 是 turn id
    /// （纯序号），拿不到本轮**说了什么**——这对「记忆 / 导演」类 Mod 是硬缺口：
    /// 它们要按输入正文做检索 / 情绪判断，而不是只知道「开了一轮」。
    /// 本主题 payload = **送进主链路的原始输入文本**（聊天框 / external-input /
    /// voice 转写渲染后的同一份字符串），与 `TurnStarted` 在同一提交点、
    /// **紧随其后**发出。
    ///
    /// **时序提醒（记忆类 Mod 必读）**：本事件发出时该轮请求体**马上**就会构建，
    /// 因此 Mod 在本事件里做的配置写回（`apply_settings`）只对**下一轮**生效——
    /// 「先检索、再注入下一轮」正是这个时序的预期用法（见
    /// `docs/plans/parallel-mods/PARALLEL-WAVE2-2026-09-14.md` §2）。
    TurnPrompt,
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
        ModEventTopic::TurnPrompt,
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
            ModEventTopic::TurnPrompt => "turn_prompt",
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

    /// Wave 2：`TurnPrompt` 必须进 `ALL`（前端「Mod 管理」按它列可订阅主题），
    /// 且 id 稳定。
    #[test]
    fn turn_prompt_is_listed_and_stable() {
        assert!(
            ModEventTopic::ALL.contains(&ModEventTopic::TurnPrompt),
            "TurnPrompt 必须出现在 ALL 里（否则前端看不到这个可订阅主题）"
        );
        assert_eq!(ModEventTopic::TurnPrompt.as_str(), "turn_prompt");
    }
}
