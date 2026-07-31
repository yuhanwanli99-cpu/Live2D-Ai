package com.live2dai.plugin.proactive

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import androidx.work.ListenableWorker
import androidx.work.testing.TestListenableWorkerBuilder
import androidx.work.workDataOf
import com.live2dai.pluginsdk.ChatHook
import com.live2dai.pluginsdk.PersonaSnapshot
import com.live2dai.pluginsdk.PluginHost
import com.live2dai.pluginsdk.ToolDefinition
import java.util.Calendar
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * [ProactiveCheckWorker] 决策分支测试（docs/plan-task-plugin-proactive.md §7 验收标准 2-5）。
 *
 * 用 WorkManager 官方测试工具 [TestListenableWorkerBuilder] 直接构造 Worker（不依赖真实定时器），
 * 配合可注入时钟 [ProactiveCheckWorker.nowMillis] + 预置 SharedPreferences 时间戳，
 * 对每个分支做确定性验证。
 */
@RunWith(RobolectricTestRunner::class)
class ProactiveCheckWorkerTest {

    private lateinit var context: Context
    private lateinit var tracker: LastInteractionTracker

    /**
     * 固定"现在"：**明天 12:00**（本地时区）。
     * 永不在默认静默时段 23:00-07:00 内，保证"应触发"类用例不受真实运行时刻影响。
     */
    private val fixedNow: Long by lazy { tomorrowAtHour(12, 0) }

    private val fallbackGreetings = listOf("兜底文案A", "兜底文案B", "兜底文案C")

    private class FakeHost : PluginHost {
        val sentMessages = mutableListOf<String>()
        override fun registerHook(hook: ChatHook) {}
        override fun registerTool(tool: ToolDefinition) {}
        override fun sendAssistantMessage(text: String, emotion: String?) { sentMessages += text }
        override fun getPersona(): PersonaSnapshot =
            PersonaSnapshot(name = "小喵", systemPrompt = "你是小喵，一只黏人的猫娘。", model = "test-model")
    }

    private class FakeGreetingClient(
        var result: String? = "小喵想你啦~",
        var shouldThrow: Boolean = false,
    ) : GreetingLlmClient {
        override suspend fun generateGreeting(personaName: String, systemPrompt: String, idleMinutes: Long): String? {
            if (shouldThrow) throw RuntimeException("LLM 网络异常")
            return result
        }
    }

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        tracker = LastInteractionTracker(context)
        clearPrefs()
        ProactiveHostBridge.host = FakeHost()
        ProactiveHostBridge.greetingLlmClient = FakeGreetingClient()
    }

    @After
    fun tearDown() {
        clearPrefs()
        ProactiveHostBridge.host = null
        ProactiveHostBridge.greetingLlmClient = null
    }

    private fun clearPrefs() {
        context.getSharedPreferences(LastInteractionTracker.PREFS_NAME, Context.MODE_PRIVATE)
            .edit().clear().commit()
    }

    private fun buildWorker(
        idleThresholdMinutes: Long = 180L,
        cooldownMinutes: Long = 360L,
        quietHoursStart: Int = 23,
        quietHoursEnd: Int = 7,
    ): ProactiveCheckWorker =
        TestListenableWorkerBuilder<ProactiveCheckWorker>(context)
            .setInputData(
                workDataOf(
                    ProactiveCheckWorker.KEY_IDLE_THRESHOLD_MINUTES to idleThresholdMinutes,
                    ProactiveCheckWorker.KEY_COOLDOWN_MINUTES to cooldownMinutes,
                    ProactiveCheckWorker.KEY_QUIET_HOURS_START to quietHoursStart,
                    ProactiveCheckWorker.KEY_QUIET_HOURS_END to quietHoursEnd,
                    ProactiveCheckWorker.KEY_FALLBACK_GREETINGS to fallbackGreetings.toTypedArray(),
                )
            )
            .build()
            .apply { nowMillis = { fixedNow } }

    private fun sentMessages(): List<String> = (ProactiveHostBridge.host as FakeHost).sentMessages

    /** 返回本地时区"明天 hour:minute"的时间戳（严格在未来，且与 fixedNow 相互独立）。 */
    private fun tomorrowAtHour(hour: Int, minute: Int): Long {
        val cal = Calendar.getInstance().apply {
            add(Calendar.DAY_OF_YEAR, 1)
            set(Calendar.HOUR_OF_DAY, hour)
            set(Calendar.MINUTE, minute)
            set(Calendar.SECOND, 0)
            set(Calendar.MILLISECOND, 0)
        }
        return cal.timeInMillis
    }

    // ── 验收标准 2：idle 未到 → 不触发 ──

    @Test
    fun doWork_doesNotTrigger_whenNotIdleEnough() = runBlocking {
        tracker.recordInteraction(fixedNow - 60 * 60 * 1000L)   // 1 小时前对话（idle 阈值 180 未到）
        tracker.recordProactiveMessage(fixedNow - 10 * 60 * 60 * 1000L)

        val result = buildWorker().doWork()

        assertEquals(ListenableWorker.Result.success(), result)
        assertTrue("未到 idle 阈值不应发消息", sentMessages().isEmpty())
    }

    // ── 验收标准 3：满足所有条件 → 发送且文案来自 mock LLM 返回值 ──

    @Test
    fun doWork_triggers_withLlmGeneratedGreeting() = runBlocking {
        tracker.recordInteraction(fixedNow - 4 * 60 * 60 * 1000L)    // 4 小时前对话（idle 240 ≥ 180）
        tracker.recordProactiveMessage(fixedNow - 10 * 60 * 60 * 1000L) // 10 小时前主动（cooldown 600 ≥ 360）

        val result = buildWorker().doWork()

        assertEquals(ListenableWorker.Result.success(), result)
        assertEquals("文案应来自 mock GreetingLlmClient 的返回值", listOf("小喵想你啦~"), sentMessages())
    }

    // ── 验收标准 4：LLM 抛异常 → 回退 fallbackGreetings，流程不中断 ──

    @Test
    fun doWork_fallsBackToStaticGreetings_whenLlmThrows() = runBlocking {
        tracker.recordInteraction(fixedNow - 4 * 60 * 60 * 1000L)
        tracker.recordProactiveMessage(fixedNow - 10 * 60 * 60 * 1000L)
        ProactiveHostBridge.greetingLlmClient = FakeGreetingClient(shouldThrow = true)

        val result = buildWorker().doWork()

        assertEquals(ListenableWorker.Result.success(), result)
        val sent = sentMessages().single()
        assertTrue("应回退到静态文案池，实际发送: $sent", sent in fallbackGreetings)
    }

    @Test
    fun doWork_fallsBackToStaticGreetings_whenLlmReturnsNull() = runBlocking {
        tracker.recordInteraction(fixedNow - 4 * 60 * 60 * 1000L)
        tracker.recordProactiveMessage(fixedNow - 10 * 60 * 60 * 1000L)
        ProactiveHostBridge.greetingLlmClient = FakeGreetingClient(result = null)

        val result = buildWorker().doWork()

        assertEquals(ListenableWorker.Result.success(), result)
        assertTrue("LLM 返回 null 应回退到静态文案池", sentMessages().single() in fallbackGreetings)
    }

    // ── 验收标准 5：静默时段内 → 不触发 ──

    @Test
    fun doWork_doesNotTrigger_duringQuietHours() = runBlocking {
        val quietNow = tomorrowAtHour(0, 30) // 次日 00:30，默认静默时段 23:00-07:00 内
        tracker.recordInteraction(quietNow - 4 * 60 * 60 * 1000L)
        tracker.recordProactiveMessage(quietNow - 10 * 60 * 60 * 1000L)

        val worker = buildWorker().apply { nowMillis = { quietNow } }
        val result = worker.doWork()

        assertEquals(ListenableWorker.Result.success(), result)
        assertTrue("静默时段内不应发消息", sentMessages().isEmpty())
    }

    // ── 冷却期 → 不触发（§2.2 三重限流之一）──

    @Test
    fun doWork_doesNotTrigger_duringCooldown() = runBlocking {
        tracker.recordInteraction(fixedNow - 4 * 60 * 60 * 1000L)
        tracker.recordProactiveMessage(fixedNow - 2 * 60 * 60 * 1000L) // 2 小时前主动（cooldown 360 未到）

        val result = buildWorker().doWork()

        assertEquals(ListenableWorker.Result.success(), result)
        assertTrue("冷却期内不应发消息", sentMessages().isEmpty())
    }

    // ── 触发成功后记录 last_proactive，供下次冷却判断 ──

    @Test
    fun doWork_recordsProactiveMessageAfterTrigger() = runBlocking {
        tracker.recordInteraction(fixedNow - 4 * 60 * 60 * 1000L)
        tracker.recordProactiveMessage(fixedNow - 10 * 60 * 60 * 1000L)

        buildWorker().doWork()
        // 触发后立刻再跑一次：刚发过 → 冷却期内不再发
        buildWorker().doWork()

        assertEquals("触发后应进入冷却期，只发一次", 1, sentMessages().size)
    }

    // ── 插件未加载（桥为空）→ 安全跳过 ──

    @Test
    fun doWork_skipsGracefully_whenHostBridgeEmpty() = runBlocking {
        tracker.recordInteraction(fixedNow - 4 * 60 * 60 * 1000L)
        tracker.recordProactiveMessage(fixedNow - 10 * 60 * 60 * 1000L)
        val host = ProactiveHostBridge.host as FakeHost
        ProactiveHostBridge.host = null // 模拟插件已卸载、桥被清空

        val result = buildWorker().doWork()

        assertEquals(ListenableWorker.Result.success(), result)
        assertTrue("桥为空时应安全跳过", host.sentMessages.isEmpty())
    }

    // ── 静默时段纯函数边界 ──

    @Test
    fun quietHours_pureFunction_crossMidnightInterval() {
        // 默认 23..7（跨天）：23:00-次日 6:59 静默
        assertTrue(ProactiveCheckWorker.isWithinQuietHours(23, 23, 7))
        assertTrue(ProactiveCheckWorker.isWithinQuietHours(0, 23, 7))
        assertTrue(ProactiveCheckWorker.isWithinQuietHours(6, 23, 7))
        assertFalse("07:00 是区间结束，不应静默", ProactiveCheckWorker.isWithinQuietHours(7, 23, 7))
        assertFalse(ProactiveCheckWorker.isWithinQuietHours(12, 23, 7))
        assertFalse(ProactiveCheckWorker.isWithinQuietHours(22, 23, 7))
    }

    @Test
    fun quietHours_pureFunction_sameDayInterval_andEmptyInterval() {
        // 不跨天 9..17：9:00-16:59 静默
        assertTrue(ProactiveCheckWorker.isWithinQuietHours(9, 9, 17))
        assertTrue(ProactiveCheckWorker.isWithinQuietHours(16, 9, 17))
        assertFalse(ProactiveCheckWorker.isWithinQuietHours(17, 9, 17))
        assertFalse(ProactiveCheckWorker.isWithinQuietHours(8, 9, 17))
        // start == end → 空区间，恒为 false
        assertFalse(ProactiveCheckWorker.isWithinQuietHours(0, 0, 0))
        assertFalse(ProactiveCheckWorker.isWithinQuietHours(12, 0, 0))
    }
}
