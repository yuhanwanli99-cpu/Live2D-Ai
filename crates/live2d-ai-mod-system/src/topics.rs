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
    /// **时序提醒（记忆 / 人设类 Mod 必读）**：本事件在 supervisor 决议本轮
    /// system_prompt **之前**投递，且 host 对该话题走**同步投递**
    ///（`ModRegistry::dispatch_event_and_flush`：入队后等 worker 回执）——
    /// 所以 Mod 在这里写的会话注入槽（memory 的 MEMORY 块 / persona 的卡）
    /// **本轮请求体就会带上**（2026-09-15 起；旧实现排在决议之后，只能等下一轮）。
    /// 「先检索、再注入本轮」正是这个时序的用法（见
    /// `docs/plans/parallel-mods/PARALLEL-WAVE2-2026-09-14.md` §2）。
    TurnPrompt,
    /// **一轮 turn 收口**（Wave 3 新增，2026-09-14）。
    ///
    /// 发点在 supervisor 空闲态：`turn::run_one_turn` 返回之后（无论该轮是
    /// 正常收口还是 `TurnStatus::Failed`），payload = 该轮 turn id（与
    /// `TurnStarted` 同一个序号）。语义是「这一轮结束了」，**不是**「成功」——
    /// 需要区分成败的 Mod 应结合自己订阅的事件判断，不要把这个主题当成功回执。
    ///
    /// 用途：让 Mod 在**轮末**收口自己的 per-turn 状态（例：导演把本轮的
    /// 决策日志结项、记忆把「本轮实际用了哪些命中」写进计数），而不是只能
    /// 在下一轮开头补救。**时序**：与 `TurnPrompt` 同款——发出时下一轮请求体
    /// 尚未构建，因此这里做的 `apply_settings` 对**下一轮**生效。轮末写回
    /// 本来就是「为下一轮准备」，这一条没变（与 TurnPrompt 的同轮注入不是一回事）。
    TurnEnded,
    /// **助手侧正文（已说出 / 已上屏）**（Wave 3 新增，2026-09-21）。
    ///
    /// 发点在 supervisor 空闲态：turn::run_one_turn 返回之后、TurnEnded 之前。
    /// payload = JSON {"turn":<turn id>,"role":"assistant","text":"<清洗后正文>",
    /// "interrupted":<bool>}。
    ///
    /// # 为什么需要它
    ///
    /// TurnPrompt 只给 Mod **用户侧**正文；SentenceReady 是按句锚点
    /// （一轮多条、且描述的是「即将送 TTS」，不是一轮的整体口径）。记忆 /
    /// 摘要类 Mod 要按 conversation 记「助手说过什么」，缺的就是这个「一轮的
    /// 助手侧正文」入口——不补它，摘要永远只能压用户话（见 memory 头注 §4.1）。
    ///
    /// # 纪律
    ///
    /// - text 是**确定性清洗后**的上屏正文（与送 TTS 同源，见
    ///   live2d_ai_runtime::clean_for_tts）；role 固定 "assistant"；
    /// - **投递非阻塞**（try_send，与 SentenceReady 同款）：慢 / 失败的 Mod
    ///   只影响它自己的行内容，绝不阻塞主链；
    /// - Mod 侧处理失败只允许 **warn**（记忆是增量能力，写不进去不该打断一轮）；
    /// - interrupted=true 表示本轮被 /stop 或 quit 中断（正文可能只说了一半），
    ///   消费方自行决定记不记——本主题只如实投递，不替消费方裁决。
    AssistantReplied,
    /// **一句正文已切出、即将交给 TTS**（2026-09-16，P1-2）。
    ///
    /// 发点在 `push_job` **之前**（见 `EngineEvent::SentenceReady`）：该句 TTS 合成
    /// 的 0.5–2s 是异步导演的天然预算窗口。payload = JSON
    /// `{"epoch":..,"sentence_seq":..,"ts_ms":..,"text":".."}`。
    ///
    /// **投递是非阻塞的**（`dispatch_event` / try_send）：主链绝不等待导演；
    /// 慢 / 失败的 Mod 只影响它自己的行内容，不影响 TTS 与上屏。
    SentenceReady,
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
        ModEventTopic::TurnEnded,
        ModEventTopic::AssistantReplied,
        ModEventTopic::SentenceReady,
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
            ModEventTopic::TurnEnded => "turn_ended",
            ModEventTopic::AssistantReplied => "assistant_replied",
            ModEventTopic::SentenceReady => "sentence_ready",
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

    /// Wave 3：`TurnEnded` 必须进 `ALL`（否则 Mod 发现面看不到轮末钩子），
    /// 且 id 稳定。
    #[test]
    fn turn_ended_is_listed_and_stable() {
        assert!(
            ModEventTopic::ALL.contains(&ModEventTopic::TurnEnded),
            "TurnEnded 必须出现在 ALL 里"
        );
        assert_eq!(ModEventTopic::TurnEnded.as_str(), "turn_ended");
    }

    /// Wave 3（2026-09-21）：AssistantReplied 必须进 ALL（否则记忆类 Mod 的
    /// 发现面看不到「助手侧正文」这个可订阅主题），且 id 稳定。
    #[test]
    fn assistant_replied_is_listed_and_stable() {
        assert!(
            ModEventTopic::ALL.contains(&ModEventTopic::AssistantReplied),
            "AssistantReplied 必须出现在 ALL 里"
        );
        assert_eq!(
            ModEventTopic::AssistantReplied.as_str(),
            "assistant_replied"
        );
    }
}
