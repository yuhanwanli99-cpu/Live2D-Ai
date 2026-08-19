package com.live2d.ai.android.core

/**
 * 状态转换结果（契约 §2.1「合法转换输出 nextState+commands」）。
 *
 * 非法转换 = `nextState == 当前态` 且 `commands` 为空（态不变、无副作用指令）。
 * [commands] 保序（顺序语义 = 转移表副作用指令顺序）。
 */
data class LoopTransition(
    val nextState: LoopState,
    val commands: List<LoopCommand>,
)

/**
 * 核心环路三态状态机（纯 JVM，无 Android 依赖；架构 C §5.1，选型 C-13=4B）。
 *
 * 事件→状态→副作用指令三元组；本类只做纯转换，不执行副作用（执行归 [LoopCoordinator]）。
 *
 * ## 两种入口
 * - 纯函数：`transition(state, event)` —— 显式传态，不修改内部状态（转移表穷举测试用）。
 * - 有态入口：`transition(event)` —— 推进内部 [state] 并在转换后回调 [onState]（Coordinator 用）。
 *
 * ## 转移表（契约 refactor-0814 §2.1 锁定，逐字实现）
 *
 * | 当前态 | 事件 | 下一态 | 副作用指令（顺序） |
 * |---|---|---|---|
 * | IDLE | SubmitText(t) | SENDING | LlmSend(t), UiAddUserBubble(t) |
 * | SPEAKING | SubmitText(t) | SENDING | SpeechStop, MouthZero, ExprRelease, LlmSend(t)（打断序） |
 * | SENDING | LlmDelta(f) | SENDING | UiAppendStreamBubble(f) |
 * | SENDING | LlmCompleted(text, emotion) | SPEAKING | ExprApply(emotion), SpeechSpeak(text) |
 * | SENDING | LlmFailed(msg, quota) | IDLE | UiShowError(msg, quota) |
 * | SPEAKING | SpeakFinished | IDLE | MouthZero, ExprRelease |
 * | SPEAKING | SpeakFailed | IDLE | MouthZero, ExprRelease |
 * | 任意 | Reset | IDLE | SpeechStop, MouthZero, ExprRelease, LlmReset |
 *
 * 其余组合（含 SpeakStarted 全态、以及任何未列事件）均为非法转换：态不变、零指令、不抛异常（I7 穷举安全）。
 */
class LoopStateMachine(initialState: LoopState = LoopState.IDLE) {

    /** 当前状态（私有 set；仅经有态 [transition] / [forceIdle] 变更）。 */
    @Volatile
    var state: LoopState = initialState
        private set

    /** 状态变更回调（默认空实现；测试可继承覆写记录状态序列）。 */
    open fun onState(state: LoopState) {}

    /**
     * 纯函数入口（实例形式）：`(state, event) → (nextState, commands)`，不修改内部状态。
     * 穷举安全：全部态×事件组合（含未知子类型）有明确出口，不抛异常（I7）。
     */
    fun transition(state: LoopState, event: LoopEvent): LoopTransition =
        transitionTable(state, event)

    /**
     * 有态入口：按 [LoopEvent] 转换并推进内部状态；转换后回调 [onState]。
     * 非法转换不改变状态、不回调；任何事件（含未知子类型）不抛异常（I7）。
     */
    fun transition(event: LoopEvent): LoopTransition {
        val result = transitionTable(state, event)
        if (result.nextState != state) {
            state = result.nextState
            onState(state)
        }
        return result
    }

    /** fail-safe（I7）：任意异常后强制收敛回 IDLE，不崩溃。 */
    fun forceIdle() {
        if (state != LoopState.IDLE) {
            state = LoopState.IDLE
            onState(state)
        }
    }

    companion object {
        /** 纯函数入口（伴生形式）：`(state, event) → (nextState, commands)`。 */
        fun transition(state: LoopState, event: LoopEvent): LoopTransition =
            transitionTable(state, event)

        internal fun transitionTable(state: LoopState, event: LoopEvent): LoopTransition = when (state) {
            LoopState.IDLE -> when (event) {
                is LoopEvent.SubmitText -> LoopTransition(
                    LoopState.SENDING,
                    listOf(
                        LoopCommand.LlmSend(event.text),
                        LoopCommand.UiAddUserBubble(event.text),
                    ),
                )
                is LoopEvent.Reset -> LoopTransition(LoopState.IDLE, resetCommands())
                // LlmDelta/LlmCompleted/LlmFailed/SpeakStarted/SpeakFinished/SpeakFailed 在 IDLE 非法 → 态不变
                else -> LoopTransition(LoopState.IDLE, emptyList())
            }

            LoopState.SENDING -> when (event) {
                is LoopEvent.LlmDelta -> LoopTransition(
                    LoopState.SENDING,
                    listOf(UiAppendStreamBubble(event.fragment)),
                )
                is LoopEvent.LlmCompleted -> LoopTransition(
                    LoopState.SPEAKING,
                    listOf(
                        LoopCommand.ExprApply(event.emotion),
                        LoopCommand.SpeechSpeak(event.fullText),
                    ),
                )
                is LoopEvent.LlmFailed -> LoopTransition(
                    LoopState.IDLE,
                    listOf(LoopCommand.UiShowError(event.message, event.quotaExhausted)),
                )
                is LoopEvent.Reset -> LoopTransition(LoopState.IDLE, resetCommands())
                // I1：SENDING 期间 SubmitText 拒绝/忽略（无第二个 llm.send）；SpeakStarted/Finished/Failed 非法
                else -> LoopTransition(LoopState.SENDING, emptyList())
            }

            LoopState.SPEAKING -> when (event) {
                // C-4=立即打断：speech.stop → mouth.zero → expr.release → llm.send（打断序）
                is LoopEvent.SubmitText -> LoopTransition(
                    LoopState.SENDING,
                    listOf(
                        LoopCommand.SpeechStop,
                        LoopCommand.MouthZero,
                        ExprRelease,
                        LoopCommand.LlmSend(event.text),
                    ),
                )
                is LoopEvent.SpeakFinished -> LoopTransition(
                    LoopState.IDLE,
                    listOf(LoopCommand.MouthZero, ExprRelease),
                )
                is LoopEvent.SpeakFailed -> LoopTransition(
                    LoopState.IDLE,
                    listOf(LoopCommand.MouthZero, ExprRelease),
                )
                is LoopEvent.Reset -> LoopTransition(LoopState.IDLE, resetCommands())
                // LlmDelta/LlmCompleted/LlmFailed/SpeakStarted 在 SPEAKING 非法 → 态不变
                else -> LoopTransition(LoopState.SPEAKING, emptyList())
            }
        }

        /** Reset 通配副作用：speech.stop + 收敛（mouth.zero + expr.release）+ llm.reset。 */
        private fun resetCommands(): List<LoopCommand> = listOf(
            LoopCommand.SpeechStop,
            LoopCommand.MouthZero,
            ExprRelease,
            LoopCommand.LlmReset,
        )
    }
}
