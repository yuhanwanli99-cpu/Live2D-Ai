package com.live2d.ai.android.core

/**
 * 进入状态机的事件（契约 refactor-0814 §2.1 锁定，逐字照抄）。
 */
sealed interface LoopEvent {
    /** 用户提交文本（IDLE→SENDING；SPEAKING→SENDING 打断）。 */
    data class SubmitText(val text: String) : LoopEvent

    /** LLM 流式增量（SENDING 内 UI 追加气泡）。 */
    data class LlmDelta(val fragment: String) : LoopEvent

    /** LLM 回复完成（SENDING→SPEAKING 唯一入口，I2/I3）。 */
    data class LlmCompleted(val fullText: String, val emotion: String?) : LoopEvent

    /** LLM 失败（SENDING→IDLE，中文友好文案气泡）。quotaExhausted 默认 false（测试单参构造）。 */
    data class LlmFailed(val message: String, val quotaExhausted: Boolean = false) : LoopEvent

    /** 播报开始（信息性事件；转移表无该行 → 各态均拒绝，态不变）。 */
    data object SpeakStarted : LoopEvent

    /** 播报自然结束（SPEAKING→IDLE：嘴型归零 + 延迟 N 秒表情收敛）。 */
    data object SpeakFinished : LoopEvent

    /** 播报失败（SPEAKING→IDLE：嘴型归零 + 表情收敛）。 */
    data object SpeakFailed : LoopEvent

    /** 任意态通配：收敛回 IDLE（speech.stop + 嘴型归零 + 表情收敛 + llm.reset）。 */
    data object Reset : LoopEvent
}
