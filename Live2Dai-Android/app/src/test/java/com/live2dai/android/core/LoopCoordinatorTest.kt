package com.live2dai.android.core

import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.emptyFlow
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertSame
import org.junit.Test

/**
 * LoopCoordinator 行为 RED（契约 refactor-0814 §2.2 壳侧接缝 + 架构 shell §5，Wave 2C-1）。
 *
 * 覆盖生产文件 `core/LoopCoordinator.kt`（**新增，RED 待实现**，单例编排者）。
 *
 * 本测试只锚定「确定性、同步、可 JVM 判读」的行为（双播报/冻结免疫的**结构**断言）：
 *   - I6 / CORE2-A4：attachView = **替换语义**（新 host 覆盖旧 host，绝不追加）→ 绑定恒 1 份。
 *   - I1：sendMessage 重入守卫——SENDING 在途期间重复 sendMessage → 只产生 1 次 llm 发送。
 *   - 事件分发（Completed）序：expr.apply 先于 speech.speak（沿用状态机 §2.1 副作用顺序语义）。
 *
 * 依赖 seam 签名（对齐契约 §2.2 锁定接缝；由 coder 对齐；测试用 Fake 以契约签名为准实现）：
 *   interface LlmLink {
 *       fun send(text: String): Flow<LoopEvent>            // 冷流单消费者（契约）
 *       fun reset()                                        // 清上下文（契约）
 *   }
 *   interface ExpressionSink { fun apply(emotion: String?); fun release() }  // release 幂等（契约）
 *   interface SpeechSink {
 *       suspend fun speak(cleanText: String)               // suspend、互斥（契约）
 *       fun stop()                                         // 幂等（契约）
 *       val isSpeaking: StateFlow<Boolean>                 // 至多一个合成在途（契约）
 *   }
 *   interface LoopViewHost { fun onState(state: LoopState) }  // 视图接线目标（attachView 参数；生产类型见 RED 报告 §6 [UNSURE-2]）
 *
 * Coordinator 入口 spec（由 coder 对齐；单例 + 依赖注入 seam）：
 *   object LoopCoordinator {
 *       fun withDependencies(llm, expr, speech)             // 测试 seam / 接线入口
 *       fun attachView(host: LoopViewHost)                   // 替换语义（I6）
 *       var boundHost: LoopViewHost?                         // 可观测单一绑定
 *       fun sendMessage(text: String)                        // 重入守卫（I1）：同步进入 SENDING 后再拒重入
 *       fun onEvent(e: LoopEvent)                            // 事件分发 → 状态机 + 副作用执行
 *   }
 *
 * 说明：本测试为「契约级 RED」。生产符号不存在（core/ 目录亦不存在）→ 编译期即 RED（预期）。
 * 状态经 sendMessage/onEvent 同步驱动（状态机是纯函数，Coordinator 读入后同步推进状态），
 * 不依赖真实协程调度做判定；异步副作用执行（suspend speak / Flow 采集时序）见 RED 报告 §6 UNSURE-2/3。
 */
class LoopCoordinatorTest {

    // ------------------- Fake 依赖（签约契约签名） -------------------

    private class FakeLlmLink : LlmLink {
        val sendCalls = mutableListOf<String>()
        var resetCalls = 0
        override fun send(text: String): Flow<LlmEvent> {
            sendCalls.add(text)
            // emptyFlow 永不发射 → 合同上 LLM 永不回 → 状态保持 SENDING（在途），供 I1 重入守卫判定
            return emptyFlow()
        }
        override fun reset() { resetCalls++ }
    }

    private class FakeExpressionSink : ExpressionSink {
        val applies = mutableListOf<String?>()
        var releases = 0
        override fun apply(emotion: String?) { applies.add(emotion) }
        override fun release() { releases++ }          // release 幂等：可重复调用，语义由接缝保证
    }

    private class FakeSpeechSink : SpeechSink {
        val speaks = mutableListOf<String>()
        var stops = 0
        private val _isSpeaking = MutableStateFlow(false)
        override suspend fun speak(cleanText: String) { speaks.add(cleanText) }
        override fun stop() { stops++; _isSpeaking.value = false }
        override val isSpeaking: StateFlow<Boolean> get() = _isSpeaking
    }

    private class FakeHost(val id: Int) : LoopViewHost {
        var attachCalls = 0
        var detachCalls = 0
        var zeroMouths = 0
        override fun onAttach() { attachCalls++ }
        override fun onDetach() { detachCalls++ }
        override fun zeroMouth() { zeroMouths++ }
    }

    // ═══════════════════════════════════════════════════════════════
    // I6 / CORE2-A4：attachView 替换语义（修复后不追加、不累积）
    // ═══════════════════════════════════════════════════════════════

    /** attachView(H1) → 绑定单一 host H1（绑定计数=1）。 */
    @Test
    fun attachView_firstHost_isBound_singly() {
        val llm = FakeLlmLink(); val expr = FakeExpressionSink(); val speech = FakeSpeechSink()
        val c = LoopCoordinator.withDependencies(llm, expr, speech)

        val h1 = FakeHost(1)
        c.attachView(h1)
        assertNotNull("attachView 后应有绑定", c.boundHost)
        assertSame("绑定恰为新 host H1（替换语义）", h1, c.boundHost)
        assertEquals("H1 onAttach 恰一次（首次绑定）", 1, h1.attachCalls)
        // 分发验证：Reset → MouthZero 只到当前绑定 host
        c.onEvent(LoopEvent.Reset)
        assertEquals("仅绑定 host H1 收到嘴型归零", 1, h1.zeroMouths)
    }

    /** attachView(H1) → attachView(H2) → 旧 H1 被新 H2 覆盖（绝不追加），绑定=H2 且仍恰 1 份。 */
    @Test
    fun attachView_replaces_oldHost_notAccumulates() {
        val llm = FakeLlmLink(); val expr = FakeExpressionSink(); val speech = FakeSpeechSink()
        val c = LoopCoordinator.withDependencies(llm, expr, speech)

        val h1 = FakeHost(1)
        val h2 = FakeHost(2)
        c.attachView(h1)
        c.attachView(h2)

        assertSame("替换后绑定为新 H2，非追加", h2, c.boundHost)
        assertEquals("H1 onAttach 一次（首次绑定）", 1, h1.attachCalls)
        assertEquals("H1 onDetach 一次（被替换解绑）", 1, h1.detachCalls)
        assertEquals("H2 onAttach 一次（替换绑定）", 1, h2.attachCalls)
        // 双播报/双 hook 免疫核心：旧 H1 不应再成为分发目标；嘴型归零只到新 host H2
        c.onEvent(LoopEvent.Reset)
        assertEquals("旧 host H1 不再收到嘴型归零（替换语义，拆绑）", 0, h1.zeroMouths)
        assertEquals("新 host H2 是唯一嘴型分发目标", 1, h2.zeroMouths)
        assertSame("绑定恰 1 份（H2）", h2, c.boundHost)
    }

    /** attachView 调用 3 次 → 绑定始终 1 份（最终=最后一次），无累积（freeze factory 重跑兜底）。 */
    @Test
    fun attachView_threeTimes_stillSingleActiveBinding() {
        val llm = FakeLlmLink(); val expr = FakeExpressionSink(); val speech = FakeSpeechSink()
        val c = LoopCoordinator.withDependencies(llm, expr, speech)

        val h1 = FakeHost(1); val h2 = FakeHost(2); val h3 = FakeHost(3)
        c.attachView(h1)
        c.attachView(h2)
        c.attachView(h3)

        assertSame("三次替换后绑定=最后一次 H3", h3, c.boundHost)
        assertEquals("H1 onAttach/onDetach 各一次", 1, h1.attachCalls)
        assertEquals("H1 被替换解绑一次", 1, h1.detachCalls)
        assertEquals("H2 onAttach/onDetach 各一次", 1, h2.attachCalls)
        assertEquals("H2 被替换解绑一次", 1, h2.detachCalls)
        assertEquals("H3 onAttach 一次（最终绑定）", 1, h3.attachCalls)
        assertEquals("H3 绑定后未解绑", 0, h3.detachCalls)
        c.onEvent(LoopEvent.Reset)
        assertEquals("旧 H1 已拆绑（嘴型分发不到）", 0, h1.zeroMouths)
        assertEquals("旧 H2 已拆绑（嘴型分发不到）", 0, h2.zeroMouths)
        assertEquals("新 H3 是唯一分发目标", 1, h3.zeroMouths)
    }

    // ═══════════════════════════════════════════════════════════════
    // I1：sendMessage 重入守卫（至多一个 LLM 请求在途）
    // ═══════════════════════════════════════════════════════════════

    /** 二次 sendMessage（首次仍在途 SENDING）→ 只产生 1 次 llm.send（重入被拒绝/忽略）。 */
    @Test
    fun sendMessage_reentryWhileInFlight_onlyOneSend() {
        val llm = FakeLlmLink(); val expr = FakeExpressionSink(); val speech = FakeSpeechSink()
        val c = LoopCoordinator.withDependencies(llm, expr, speech)

        c.sendMessage("第一条")
        // 首次调用后状态=SENDING（emptyFlow 永不完成的 llm 链）; 立即重入
        c.sendMessage("第二条重入")

        assertEquals(
            "I1: 在途重入禁止双发——两次 sendMessage 只落一次 llm.send",
            listOf("第一条"),
            llm.sendCalls,
        )
    }

    // ═══════════════════════════════════════════════════════════════
    // 事件分发序：Coordinator 把状态机副作用指令映射到 FakeSinks 的调用顺序
    // ═══════════════════════════════════════════════════════════════

    /** sendMessage → Completed：expr.apply 与 speech.speak 均被触发（expr 记录 list 先于 speak 记录的契约序）。 */
    @Test
    fun completedEvent_expressionApplied_andSpeechTriggered() {
        val llm = FakeLlmLink(); val expr = FakeExpressionSink(); val speech = FakeSpeechSink()
        val c = LoopCoordinator.withDependencies(llm, expr, speech)

        c.sendMessage("你好")
        c.onEvent(LoopEvent.LlmCompleted("你好回复", "joy"))

        // expr.apply 与 speech.speak 的确切执行由 Coordinator 内部异步编排承载（见 RED 报告 UNSURE-3）；
        // 此处只锚定确定性契约面：接收 Completed 后，表情被应用、且 speak 执行序上表达式先（沿用 §2.1 顺序语义）。
        assertEquals("Completed 后 expr.apply(joy) 被调度", listOf("joy"), expr.applies)
        assertEquals("Completed 后 speech.speak(cleanText) 被调度", listOf("你好回复"), speech.speaks)
        // I3 双播报免疫：一条回复只 speak 一次
        assertEquals("双播报免疫：speak 恰一次", 1, speech.speaks.size)
    }

    // ═══════════════════════════════════════════════════════════════
    // I5：SpeakFinished 表情收敛延迟（默认 5s）/ SpeakFailed 立即收敛
    // ═══════════════════════════════════════════════════════════════

    /** SpeakFinished → 嘴型立即归零（I4）；表情收敛按默认延迟 5s 调度（未到期 releases==0）。 */
    @Test
    fun speakFinished_releaseDelayedByDefault5s() {
        val llm = FakeLlmLink(); val expr = FakeExpressionSink(); val speech = FakeSpeechSink()
        val c = LoopCoordinator.withDependencies(llm, expr, speech)   // 默认 expressionReleaseDelaySeconds=5
        val host = FakeHost(1)
        c.attachView(host)

        c.sendMessage("你好")
        c.onEvent(LoopEvent.LlmCompleted("你好回复", "joy"))
        c.onEvent(LoopEvent.SpeakFinished)

        assertEquals("I5: SpeakFinished 后表情收敛被延迟（默认 5s 未到期，releases==0）", 0, expr.releases)
        assertEquals("I4: SpeakFinished 后嘴型立即归零", 1, host.zeroMouths)
    }

    /** SpeakFailed → 表情收敛立即执行（releases==1；注入 expressionReleaseDelaySeconds=0 验证同步面）。 */
    @Test
    fun speakFailed_releasesImmediately() {
        val llm = FakeLlmLink(); val expr = FakeExpressionSink(); val speech = FakeSpeechSink()
        val c = LoopCoordinator.withDependencies(llm, expr, speech, expressionReleaseDelaySeconds = 0)

        c.sendMessage("你好")
        c.onEvent(LoopEvent.LlmCompleted("你好回复", "joy"))
        c.onEvent(LoopEvent.SpeakFailed)

        assertEquals("I5: SpeakFailed 后表情立即收敛（releases==1）", 1, expr.releases)
    }
}
