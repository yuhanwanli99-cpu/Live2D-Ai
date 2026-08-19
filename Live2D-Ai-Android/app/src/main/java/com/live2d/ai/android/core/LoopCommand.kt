package com.live2d.ai.android.core

/**
 * 状态机输出的副作用指令（顺序语义 = 列表顺序，契约 refactor-0814 §2.1 转移表）。
 *
 * 命令由 [LoopCoordinator] 按序执行；纯状态机只产出指令，不执行。
 * 说明：`ExprRelease` / `UiAppendStreamBubble` 为包级 top-level 声明（契约 I4/I5 词汇，测试 bare 引用）；
 * 其余命令嵌套于 [LoopCommand]（`LoopCommand.X` 引用）。
 */
sealed interface LoopCommand {
    // ---- LLM 链路 ----
    /** `llm.send(t)`：启动一次 LLM 请求（冷流单消费者）。 */
    data class LlmSend(val text: String) : LoopCommand

    /** `llm.reset()`：清空 LLM 上下文。 */
    data object LlmReset : LoopCommand

    // ---- UI 壳 ----
    /** UI 加用户气泡。 */
    data class UiAddUserBubble(val text: String) : LoopCommand

    /** UI 错误气泡（中文友好文案；quotaExhausted 供额度提示）。 */
    data class UiShowError(val message: String, val quotaExhausted: Boolean) : LoopCommand

    // ---- TTS 播报（壳→B）----
    /** `speech.stop()`：立即打断当前播报（幂等）。 */
    data object SpeechStop : LoopCommand

    /** `speech.speak(cleanText)`：唯一 speak 落点（I2/I3 单点）。 */
    data class SpeechSpeak(val text: String) : LoopCommand

    // ---- 嘴型（I4）----
    /** `mouth.zero()`：嘴型参数归零（离开 SPEAKING 必发）。 */
    data object MouthZero : LoopCommand

    // ---- 表情（I5）----
    /** `expr.apply(emotion)`：应用表情（播报期资源）。 */
    data class ExprApply(val emotion: String?) : LoopCommand
}

/**
 * `expr.release()`：表情收敛回 neutral（幂等；离开 SPEAKING 必被调度，I5）。
 * top-level 声明：契约 I5 词汇，测试以 bare 引用。
 */
data object ExprRelease : LoopCommand

/**
 * UI 追加流式气泡（契约转移表「UI 追加流式气泡」；top-level 声明）。
 */
data class UiAppendStreamBubble(val fragment: String) : LoopCommand
