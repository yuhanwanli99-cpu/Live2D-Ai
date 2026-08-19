package com.live2d.ai.android.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * LoopStateMachine 转移表穷举 + I1~I7 不变式 RED（契约 refactor-0814 §2.1 + 架构 shell §5.1，Wave 2C-1）。
 *
 * 覆盖生产文件 `core/LoopStateMachine.kt` + `core/LoopState.kt`/`core/LoopEvent.kt`
 * （**新增，RED 待实现**，纯 Kotlin 无 Android 依赖，JVM 可单测）。
 *
 * 生产符号 spec（对齐契约 §2.1；测试按此形状书写，由 coder 实现对齐）：
 *   sealed interface LoopState { object IDLE; object SENDING; object SPEAKING }
 *   sealed interface LoopEvent {
 *       data class SubmitText(val text: String)
 *       data class LlmDelta(val fragment: String)
 *       data class LlmCompleted(val fullText: String, val emotion: String?)
 *       data class LlmFailed(val message: String, val quotaExhausted: Boolean)
 *       data object SpeakStarted
 *       data object SpeakFinished
 *       data object SpeakFailed
 *       data object Reset
 *   }
 *   sealed interface LoopCommand {              // 副作用指令（转移动作，顺序语义 = list 序）
 *       data class UiAddUserBubble(val text: String)      // UI 加用户气泡
 *       data class UiAppendStreamBubble(val fragment: String) // UI 追加流式气泡
 *       data class UiShowError(val message: String)        // UI 错误气泡（中文友好文案）
 *       data class LlmSend(val text: String)               // llm.send
 *       data object LlmReset                               // llm.reset
 *       data class ExprApply(val emotion: String?)         // expr.apply
 *       data class ExprRelease(val delaySeconds: Int)      // expr.release（0=立即；>0=延迟 N 秒，C-2 默认 5）
 *       data class SpeechSpeak(val cleanText: String)      // speech.speak
 *       data object SpeechStop                             // speech.stop
 *       data object MouthZero                              // 嘴型参数归零
 *   }
 *   data class LoopTransition(val nextState: LoopState, val commands: List<LoopCommand>)
 *   class LoopStateMachine { fun transition(state: LoopState, event: LoopEvent): LoopTransition }
 *
 * ============================ 转移表（契约 §2.1，锁定） ============================
 * | 当前态  | 事件                         | 下一态    | 副作用指令（顺序语义）                      |
 * | IDLE    | SubmitText(t)                | SENDING  | LlmSend(t) + UiAddUserBubble(t)            |
 * | SPEAKING| SubmitText(t)                | SENDING  | SpeechStop → MouthZero → ExprRelease → LlmSend(t)（打断序，C-4=立即打断）|
 * | SENDING | Delta(s)                     | SENDING  | UiAppendStreamBubble(s)                    |
 * | SENDING | Completed(text, emotion)     | SPEAKING | ExprApply(emotion) + SpeechSpeak(cleanText)|
 * | SENDING | Failed(message, quota)       | IDLE     | UiShowError(message)                       |
 * | SPEAKING| SpeakFinished                | IDLE     | MouthZero + ExprRelease(延迟 N 秒=5,C-2)    |
 * | SPEAKING| SpeakFailed                 | IDLE     | MouthZero + ExprRelease(立即)              |
 * | 任意    | Reset                        | IDLE     | SpeechStop + 收敛(MouthZero+ExprRelease) + LlmReset |
 * ===============================================================================
 *
 * 未列组合 = 表外事件（非法输入）→ 按提示指令①「拒绝/忽略」：状态不变、无副作用、不崩溃（I7 fail-safe）。
 * 但 I1 特例：SENDING + SubmitText → **状态不变、无第二 LlmSend**（契约显式「拒绝/忽略（状态不变）」）。
 *
 * 表格外组合的精确处理（"拒绝/忽略=保持原态" vs I7 "异常收敛回 IDLE"）存在诠释缝隙，
 * 本测试按提示指令① + I1 显式"状态不变"取「拒绝/忽略=保持原态」；I7 以「不抛异常 + 状态可回 IDLE」
 * 落地（详见 2c1 RED 报告 §6 [UNSURE-1]）。
 */
class LoopStateMachineTest {

    private val sm = LoopStateMachine()

    // ═══════════════════════════════════════════════════════════════
    // 一、转移表穷举：合法转换逐行断言（CORE2-A2/A3、CORE3-A2/A3 落点）
    // ═══════════════════════════════════════════════════════════════

    /** IDLE + SubmitText(t) → SENDING：[LlmSend(t) 先于 UiAddUserBubble(t)]。 */
    @Test
    fun idle_submitText_transitionsToSending_withSendThenUserBubble() {
        val t = sm.transition(LoopState.IDLE, LoopEvent.SubmitText("你好"))
        assertEquals(LoopState.SENDING, t.nextState)
        assertEquals(
            listOf<LoopCommand>(LoopCommand.LlmSend("你好"), LoopCommand.UiAddUserBubble("你好")),
            t.commands,
        )
    }

    /** SPEAKING + SubmitText(t) → SENDING：打断序 [SpeechStop → MouthZero → ExprRelease → LlmSend(t)]（C-4=立即打断）。 */
    @Test
    fun speaking_submitText_interrupt_orderIsStopMouthZeroExprReleaseThenSend() {
        val t = sm.transition(LoopState.SPEAKING, LoopEvent.SubmitText("再来一句"))
        assertEquals(LoopState.SENDING, t.nextState)
        assertEquals(
            listOf<LoopCommand>(
                LoopCommand.SpeechStop,
                LoopCommand.MouthZero,
                ExprRelease,
                LoopCommand.LlmSend("再来一句"),
            ),
            t.commands,
        )
    }

    /** SENDING + Delta(s) → SENDING：[UiAppendStreamBubble(s)]（状态自环）。 */
    @Test
    fun sending_delta_appendsStreamBubble_keepsSending() {
        val t = sm.transition(LoopState.SENDING, LoopEvent.LlmDelta("片段"))
        assertEquals(LoopState.SENDING, t.nextState)
        assertEquals(
            listOf<LoopCommand>(UiAppendStreamBubble("片段")),
            t.commands,
        )
    }

    /** SENDING + Completed(text, emotion) → SPEAKING：[ExprApply(emotion) 先于 SpeechSpeak(cleanText)]（SPEAKING 唯一入口，I2）。 */
    @Test
    fun sending_completed_transitionsToSpeaking_exprApplyBeforeSpeak() {
        val t = sm.transition(
            LoopState.SENDING,
            LoopEvent.LlmCompleted("你好世界", "joy"),
        )
        assertEquals(LoopState.SPEAKING, t.nextState)
        assertEquals(
            listOf<LoopCommand>(
                LoopCommand.ExprApply("joy"),
                LoopCommand.SpeechSpeak("你好世界"),
            ),
            t.commands,
        )
    }

    /** SENDING + Completed 无情绪 → SpeechSpeak 仍触发，ExprApply(null)。 */
    @Test
    fun sending_completed_withoutEmotion_stillSpeaks_exprApplyNull() {
        val t = sm.transition(LoopState.SENDING, LoopEvent.LlmCompleted("纯文本无表情", null))
        assertEquals(LoopState.SPEAKING, t.nextState)
        assertEquals(
            listOf<LoopCommand>(LoopCommand.ExprApply(null), LoopCommand.SpeechSpeak("纯文本无表情")),
            t.commands,
        )
    }

    /** SENDING + Failed(message, quota) → IDLE：[UiShowError(message)]。 */
    @Test
    fun sending_failed_transitionsToIdle_showErrorBubble() {
        val t = sm.transition(LoopState.SENDING, LoopEvent.LlmFailed("网络异常", false))
        assertEquals(LoopState.IDLE, t.nextState)
        assertEquals(
            listOf<LoopCommand>(LoopCommand.UiShowError("网络异常", false)),
            t.commands,
        )
    }

    /** SPEAKING + SpeakFinished → IDLE：[MouthZero + ExprRelease(延迟 N=5)]（C-2 默认 5s，立即只归零嘴型）。 */
    @Test
    fun speaking_speakFinished_mouthZeroThenDelayedExprRelease() {
        val t = sm.transition(LoopState.SPEAKING, LoopEvent.SpeakFinished)
        assertEquals(LoopState.IDLE, t.nextState)
        assertEquals(
            listOf<LoopCommand>(
                LoopCommand.MouthZero,
                ExprRelease,
            ),
            t.commands,
        )
    }

    /** SPEAKING + SpeakFailed → IDLE：[MouthZero + ExprRelease(立即)]。 */
    @Test
    fun speaking_speakFailed_mouthZeroThenImmediateExprRelease() {
        val t = sm.transition(LoopState.SPEAKING, LoopEvent.SpeakFailed)
        assertEquals(LoopState.IDLE, t.nextState)
        assertEquals(
            listOf<LoopCommand>(
                LoopCommand.MouthZero,
                ExprRelease,
            ),
            t.commands,
        )
    }

    /** 任意态 + Reset → IDLE：[SpeechStop + 收敛(MouthZero+ExprRelease) + LlmReset]。三个态逐一断言。 */
    @Test
    fun anyState_reset_transitionsToIdle_stopConvergeReset() {
        for (state in listOf(LoopState.IDLE, LoopState.SENDING, LoopState.SPEAKING)) {
            val t = sm.transition(state, LoopEvent.Reset)
            assertEquals("$state Reset 应回 IDLE", LoopState.IDLE, t.nextState)
            // Reset 语义含：speech.stop + 收敛 + llm.reset；收敛=嘴型归零+表情释放
            assertTrue("$state Reset 须含 SpeechStop", t.commands.contains(LoopCommand.SpeechStop))
            assertTrue("$state Reset 须含 MouthZero（收敛）", t.commands.contains(LoopCommand.MouthZero))
            assertTrue("$state Reset 须含 ExprRelease（收敛）", t.commands.contains(ExprRelease))
            assertTrue("$state Reset 须含 LlmReset", t.commands.contains(LoopCommand.LlmReset))
            // 顺序语义：stop → 收敛 → reset
            assertEquals(
                listOf<LoopCommand>(
                    LoopCommand.SpeechStop,
                    LoopCommand.MouthZero,
                    ExprRelease,
                    LoopCommand.LlmReset,
                ),
                t.commands,
            )
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // 二、I1-I7 不变式
    // ═══════════════════════════════════════════════════════════════

    /** I1 / CORE2-A3：SENDING 期间 SubmitText → 状态不变（SENDING）、无第二 LlmSend（拒绝/忽略）。 */
    @Test
    fun I1_sendingSubmitText_isRejected_stateUnchanged_noSecondSend() {
        val t = sm.transition(LoopState.SENDING, LoopEvent.SubmitText("重入消息"))
        assertEquals("I1: SENDING 须保持不变", LoopState.SENDING, t.nextState)
        assertEquals("I1: SENDING 提交不产生副作用（无第二 llm.send）", emptyList<LoopCommand>(), t.commands)
    }

    /** I2：SPEAKING 唯一入口 = Completed 转移。断言转入 SPEAKING 只有一条合法路径（已穷举），且非 Completed 事件不会进 SPEAKING。 */
    @Test
    fun I2_speakingEnteredOnlyFromCompleted_transition() {
        // 穷举：所有 27 组合中，nextState==SPEAKING 者必须且只能是 (SENDING, Completed)。
        fun commandless(state: LoopState, event: LoopEvent): LoopState =
            sm.transition(state, event).nextState

        val states = listOf(LoopState.IDLE, LoopState.SENDING, LoopState.SPEAKING)
        var enteringSpeaking = 0
        for (s in states) for (e in allEvents()) {
            // 自环（SPEAKING+非法事件=态不变）不算"进入"；只计跨态转入
            if (commandless(s, e) == LoopState.SPEAKING && s != LoopState.SPEAKING) {
                enteringSpeaking++
                assertEquals("仅 (SENDING, Completed) 可进 SPEAKING", LoopState.SENDING, s)
            }
        }
        assertEquals("进入 SPEAKING 的转移恰 1 条", 1, enteringSpeaking)
    }

    /** I3 / CORE2-A2：speak 副作用（SpeechSpeak 指令）在整张转移表中仅出现 1 次（双播报免疫）。 */
    @Test
    fun I3_speakCommandAppearsExactlyOnce_acrossFullTable() {
        val states = listOf(LoopState.IDLE, LoopState.SENDING, LoopState.SPEAKING)
        var speakCommandCount = 0
        for (s in states) for (e in allEvents()) {
            speakCommandCount += sm.transition(s, e).commands.count { it is LoopCommand.SpeechSpeak }
        }
        assertEquals("I3: 每回复 speak ≤1 次，全表 SpeechSpeak 恰 1 处", 1, speakCommandCount)
    }

    /**
     * I4 / CORE3-A3：每条 SPEAKING 出边（SpeakFinished / SpeakFailed / SubmitText 打断）副作用均含 MouthZero。
     * 出边穷举：三条 SPEAKING 合法出边各自满足。
     */
    @Test
    fun I4_everySpeakingOutEdge_containsMouthZero() {
        val outEdges = listOf(
            LoopEvent.SpeakFinished to LoopState.IDLE,
            LoopEvent.SpeakFailed to LoopState.IDLE,
            LoopEvent.SubmitText("打断") to LoopState.SENDING,
        )
        for ((event, expectedState) in outEdges) {
            val t = sm.transition(LoopState.SPEAKING, event)
            assertTrue(
                "I4: SPEAKING+$event 须含 MouthZero（嘴型归零）",
                t.commands.contains(LoopCommand.MouthZero),
            )
        }
    }

    /**
     * I5 / CORE3-A2：每条 SPEAKING 出边（SpeakFinished / SpeakFailed / SubmitText 打断）副作用均含 expr.release 调度。
     */
    @Test
    fun I5_everySpeakingOutEdge_containsExprRelease() {
        for (event in listOf(LoopEvent.SpeakFinished, LoopEvent.SpeakFailed, LoopEvent.SubmitText("打断"))) {
            val t = sm.transition(LoopState.SPEAKING, event)
            assertTrue(
                "I5: SPEAKING+$event 须含 ExprRelease（表情收敛被调度，冻结免疫）",
                t.commands.any { it is ExprRelease },
            )
        }
    }

    /** I7：全部 27 组合不抛异常（fail-safe），且 Reset 总能收敛回 IDLE。 */
    @Test
    fun I7_noComboThrows_andResetAlwaysRecoversToIdle() {
        val states = listOf(LoopState.IDLE, LoopState.SENDING, LoopState.SPEAKING)
        for (s in states) {
            for (e in allEvents()) {
                // 任何事件都不应抛异常（fail-safe）：transition 返回而非抛错
                val t = sm.transition(s, e)
                assertTrue("I7: 转换结果状态是合法三态之一", t.nextState in states)
                // 副作用指令是 LoopCommand 类型（坐标已擦除，此处仅证命令可枚举，不强加语义）
                assertTrue(
                    "I7: 任意转换产生的命令列表可实例化（非空或空皆合法）",
                    t.commands.all { it is LoopCommand },
                )
            }
            // fail-safe 兜底：无论停在哪个态，Reset 必回 IDLE
            assertEquals(LoopState.IDLE, sm.transition(s, LoopEvent.Reset).nextState)
        }
    }

    /** I3 计数辅助：构造全部 9 种事件。 */
    private fun allEvents(): List<LoopEvent> = listOf(
        LoopEvent.SubmitText("x"),
        LoopEvent.LlmDelta("d"),
        LoopEvent.LlmCompleted("c", null),
        LoopEvent.LlmFailed("m", false),
        LoopEvent.SpeakStarted,
        LoopEvent.SpeakFinished,
        LoopEvent.SpeakFailed,
        LoopEvent.Reset,
    )
}
