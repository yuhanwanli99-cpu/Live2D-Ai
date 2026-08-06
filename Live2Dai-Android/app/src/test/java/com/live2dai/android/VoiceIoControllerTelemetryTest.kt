package com.live2dai.android

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestCoroutineScheduler
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.mockito.kotlin.mock
import org.mockito.kotlin.whenever

/**
 * VoiceIoController 遥测埋点（V1.2 模块7「可观测性」，plan §5.7.3）单元测试。
 *
 * 验证 speak()/listen() 在正确时机向 [VoiceTelemetry] 发射 [VoiceEvent]：
 * - speak（成功）：TtsStarted(首选) → TtsSucceeded
 * - speak（降级）：TtsStarted → TtsFailed(首选) → TtsFallback(首选→次选) → TtsSucceeded(次选)
 * - speak（全失败）：TtsStarted → 每个 Provider 各 TtsFailed（仅中间切换发 TtsFallback），无 TtsSucceeded
 * - speak（指定 providerId）：TtsStarted 的 providerId = 指定引擎
 * - speak（空文本）：不发射任何事件
 * - listen（成功）：AsrStarted → AsrResult(text 非空)
 * - listen（空结果）：AsrStarted → AsrResult(text=null)
 * - listen（异常）：AsrStarted → AsrFailed
 *
 * 断言读取 VoiceTelemetry.recentEvents()（环形缓冲区，@Before clear() 隔离）；
 * 忽略 timestamp 字段（真实时钟），只按类型 + 业务字段匹配。
 */
@OptIn(ExperimentalCoroutinesApi::class)
class VoiceIoControllerTelemetryTest {

    private lateinit var context: Context
    private lateinit var edgeProvider: TtsProvider
    private lateinit var systemProvider: TtsProvider
    private lateinit var asrService: AsrProvider
    private lateinit var scheduler: TestCoroutineScheduler
    private lateinit var controller: VoiceIoController

    @Before
    fun setUp() {
        VoiceTelemetry.clear()
        // lipSyncScope 默认 Dispatchers.Main：setMain 后控制器 init 的播放状态
        // 收集协程可在测试调度器上运行（不触碰真实主线程）
        scheduler = TestCoroutineScheduler()
        Dispatchers.setMain(StandardTestDispatcher(scheduler))

        context = mock<Context>()
        edgeProvider = mockProvider("edge_tts", 1)
        systemProvider = mockProvider("system_tts", 2)
        asrService = mockAsrProvider("system_speech_recognizer", 1)
        controller = VoiceIoController(context, listOf(edgeProvider, systemProvider), listOf(asrService))
    }

    @After
    fun tearDown() {
        VoiceTelemetry.clear()
        Dispatchers.resetMain()
    }

    /** 构造带 stub info + isSpeaking 的 TtsProvider mock（控制器日志/遥测读 info.id）。 */
    private fun mockProvider(id: String, priority: Int): TtsProvider {
        val provider = mock<TtsProvider>()
        whenever(provider.info).thenReturn(
            TtsProviderInfo(
                id = id,
                name = "stub-$id",
                category = TtsProviderCategory.LOCAL,
                requiresNetwork = false,
                health = MutableStateFlow(TtsHealth.OK),
                priority = priority,
                configurable = false,
            )
        )
        whenever(provider.isSpeaking).thenReturn(MutableStateFlow(false))
        return provider
    }

    /** 构造带 stub info 的 AsrProvider mock。 */
    private fun mockAsrProvider(id: String, priority: Int): AsrProvider {
        val provider = mock<AsrProvider>()
        whenever(provider.info).thenReturn(
            AsrProviderInfo(
                id = id,
                name = "stub-$id",
                requiresNetwork = false,
                health = MutableStateFlow(TtsHealth.OK),
                priority = priority,
            )
        )
        return provider
    }

    // ─────────────────────────────────────────────
    // speak 埋点（TTS 链路）
    // ─────────────────────────────────────────────

    @Test
    fun testSpeakEmitsStartedThenSucceeded() = runTest {
        // 成功路径：TtsStarted(首选) → TtsSucceeded(首选)，无 Failed/Fallback
        controller.speak("主人好喵")
        val events = VoiceTelemetry.recentEvents()
        assertEquals("成功路径应发射 2 个事件（TtsStarted + TtsSucceeded）", 2, events.size)
        val started = events[0] as VoiceEvent.TtsStarted
        assertEquals("TtsStarted.providerId 应为首选 Provider", "edge_tts", started.providerId)
        assertEquals("TtsStarted.textLength 应为文本长度", "主人好喵".length, started.textLength)
        val succeeded = events[1] as VoiceEvent.TtsSucceeded
        assertEquals("TtsSucceeded.providerId 应为成功 Provider", "edge_tts", succeeded.providerId)
        assertTrue("TtsSucceeded.durationMs 不应为负", succeeded.durationMs >= 0)
    }

    @Test
    fun testSpeakEmitsFailedAndFallbackThenSucceeded() = runTest {
        // 首选失败 → TtsFailed(首选) + TtsFallback(首选→次选)；次选成功 → TtsSucceeded
        runBlocking {
            whenever(edgeProvider.speak("测试")).thenThrow(RuntimeException("edge boom"))
        }
        controller.speak("测试")
        val events = VoiceTelemetry.recentEvents()
        assertEquals("降级成功应发射 4 个事件", 4, events.size)
        assertTrue(events[0] is VoiceEvent.TtsStarted)
        val failed = events[1] as VoiceEvent.TtsFailed
        assertEquals("TtsFailed.providerId 应为失败的首选", "edge_tts", failed.providerId)
        assertEquals("TtsFailed.reason 应为异常消息", "edge boom", failed.reason)
        val fallback = events[2] as VoiceEvent.TtsFallback
        assertEquals("TtsFallback.fromProvider 应为失败 Provider", "edge_tts", fallback.fromProvider)
        assertEquals("TtsFallback.toProvider 应为降级目标", "system_tts", fallback.toProvider)
        assertEquals("TtsFallback.reason 应为异常消息", "edge boom", fallback.reason)
        val succeeded = events[3] as VoiceEvent.TtsSucceeded
        assertEquals("TtsSucceeded.providerId 应为降级后成功者", "system_tts", succeeded.providerId)
    }

    @Test
    fun testSpeakAllFailEmitsFailedPerProviderWithoutSucceeded() = runTest {
        // 全失败：每个 Provider 各一个 TtsFailed；仅中间切换（非最后一个）发 TtsFallback
        runBlocking {
            whenever(edgeProvider.speak("测试")).thenThrow(RuntimeException("edge boom"))
            whenever(systemProvider.speak("测试")).thenThrow(RuntimeException("system boom"))
        }
        controller.speak("测试")
        val events = VoiceTelemetry.recentEvents()
        assertEquals("全失败应发射 4 个事件（Started + 2×Failed + Fallback）", 4, events.size)
        assertTrue(events[0] is VoiceEvent.TtsStarted)
        // 发射顺序：TtsFailed(edge) → TtsFallback(edge→system) → TtsFailed(system)
        // （TtsFailed 紧跟失败；TtsFallback 仅在还有下一个 Provider 时发射）
        val failed1 = events[1] as VoiceEvent.TtsFailed
        assertEquals("edge_tts", failed1.providerId)
        assertEquals("edge boom", failed1.reason)
        val fallback = events[2] as VoiceEvent.TtsFallback
        assertEquals("edge_tts", fallback.fromProvider)
        assertEquals("system_tts", fallback.toProvider)
        val failed2 = events[3] as VoiceEvent.TtsFailed
        assertEquals("system_tts", failed2.providerId)
        assertEquals("system boom", failed2.reason)
        assertTrue("全失败不应发射 TtsSucceeded", events.none { it is VoiceEvent.TtsSucceeded })
    }

    @Test
    fun testSpeakStartedUsesPreferredProviderWhenSpecified() = runTest {
        // 指定 providerId → TtsStarted 的 providerId 应为指定引擎（非降级链首位）
        controller.speak("测试", providerId = "system_tts")
        val started = VoiceTelemetry.recentEvents().first() as VoiceEvent.TtsStarted
        assertEquals("指定 providerId 时 TtsStarted 应为该 Provider", "system_tts", started.providerId)
    }

    @Test
    fun testSpeakSkipsTelemetryForBlankText() = runTest {
        // 空文本直接短路（不进入降级链）→ 不应发射任何遥测事件
        controller.speak("   ")
        assertTrue("空文本不应发射任何遥测事件", VoiceTelemetry.recentEvents().isEmpty())
    }

    // ─────────────────────────────────────────────
    // listen 埋点（ASR 链路）
    // ─────────────────────────────────────────────

    @Test
    fun testListenEmitsStartedThenResultOnSuccess() = runTest {
        runBlocking { whenever(asrService.listen()).thenReturn("你好世界") }
        val result = controller.listen()
        assertEquals("你好世界", result)
        val events = VoiceTelemetry.recentEvents()
        assertEquals("成功路径应发射 2 个事件（AsrStarted + AsrResult）", 2, events.size)
        val started = events[0] as VoiceEvent.AsrStarted
        assertEquals("system_speech_recognizer", started.providerId)
        val asrResult = events[1] as VoiceEvent.AsrResult
        assertEquals("AsrResult.text 应为识别文本", "你好世界", asrResult.text)
        assertTrue("AsrResult.durationMs 不应为负", asrResult.durationMs >= 0)
    }

    @Test
    fun testListenEmitsNullResultOnEmptyResult() = runTest {
        // 空结果（无匹配）→ AsrResult(text=null)，降级链继续
        runBlocking { whenever(asrService.listen()).thenReturn("") }
        assertEquals("", controller.listen())
        val events = VoiceTelemetry.recentEvents()
        assertEquals(2, events.size)
        assertTrue(events[0] is VoiceEvent.AsrStarted)
        val asrResult = events[1] as VoiceEvent.AsrResult
        assertNull("空结果应发射 text=null 的 AsrResult", asrResult.text)
    }

    @Test
    fun testListenEmitsFailedOnException() = runTest {
        // 异常 → AsrFailed(reason=异常消息)，返回 ""
        runBlocking { whenever(asrService.listen()).thenThrow(RuntimeException("asr boom")) }
        assertEquals("", controller.listen())
        val events = VoiceTelemetry.recentEvents()
        assertEquals(2, events.size)
        assertTrue(events[0] is VoiceEvent.AsrStarted)
        val failed = events[1] as VoiceEvent.AsrFailed
        assertEquals("system_speech_recognizer", failed.providerId)
        assertEquals("asr boom", failed.reason)
    }
}
