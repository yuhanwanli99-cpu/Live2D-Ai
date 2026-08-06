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
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.mockito.kotlin.any
import org.mockito.kotlin.mock
import org.mockito.kotlin.never
import org.mockito.kotlin.verify
import org.mockito.kotlin.whenever

/**
 * VoiceIoController（V1.1 模块1 语音闭环协调器 + V1.2 模块1 降级链）单元测试。
 *
 * TTS Provider（TtsProvider）与 VoiceInputService 以 Mockito mock 注入，验证控制器的
 * 接线语义（降级链遍历 / 指定引擎 / 容错 / 打断 / 释放 / 空文本跳过 / 组合 isSpeaking），
 * 不触碰真实网络与系统服务。
 *
 * 契约见 docs/plan-v12-plan.schema.json → functions.VoiceIoController.*：
 * - speak(text, providerId?): 降级链遍历播报，失败静默（throws: []）
 * - stopSpeaking(): 同步打断所有 Provider 播报
 * - isSpeaking: StateFlow，任一 Provider 播放中 = true
 * - listen(): 返回识别文本；失败返回 ""
 * - release(): 释放所有 Provider + ASR 资源
 */
@OptIn(ExperimentalCoroutinesApi::class)
class VoiceIoControllerTest {

    private lateinit var context: Context
    private lateinit var edgeProvider: TtsProvider
    private lateinit var systemProvider: TtsProvider
    private lateinit var edgeSpeaking: MutableStateFlow<Boolean>
    private lateinit var systemSpeaking: MutableStateFlow<Boolean>
    private lateinit var asrService: AsrProvider
    private lateinit var scheduler: TestCoroutineScheduler
    private lateinit var controller: VoiceIoController

    @Before
    fun setUp() {
        // lipSyncScope 默认 Dispatchers.Main：setMain 后控制器 init 的播放状态收集
        // 协程可在测试调度器上运行（不触碰真实主线程）
        scheduler = TestCoroutineScheduler()
        Dispatchers.setMain(StandardTestDispatcher(scheduler))

        context = mock<Context>()
        mockProvider("edge_tts", 1).let {
            edgeProvider = it.first
            edgeSpeaking = it.second
        }
        mockProvider("system_tts", 2).let {
            systemProvider = it.first
            systemSpeaking = it.second
        }
        asrService = mockAsrProvider("system_speech_recognizer", 1)
        controller = VoiceIoController(context, listOf(edgeProvider, systemProvider), listOf(asrService))
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    /** 构造带 stub info + isSpeaking 的 TtsProvider mock（控制器日志会读 info.id）。 */
    private fun mockProvider(id: String, priority: Int): Pair<TtsProvider, MutableStateFlow<Boolean>> {
        val speaking = MutableStateFlow(false)
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
        whenever(provider.isSpeaking).thenReturn(speaking)
        return provider to speaking
    }

    /** 构造带 stub info 的 AsrProvider mock（V1.2 模块4：控制器降级链读 info.id）。 */
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
    // speak（说）— V1.2 降级链遍历
    // ─────────────────────────────────────────────

    @Test
    fun testSpeakForwardsToFirstProviderOnly() = runTest {
        controller.speak("主人好喵")
        verify(edgeProvider).speak("主人好喵")
        verify(systemProvider, never()).speak(any())
    }

    @Test
    fun testSpeakSkipsBlankText() = runTest {
        controller.speak("   ")
        verify(edgeProvider, never()).speak(any())
        verify(systemProvider, never()).speak(any())
    }

    @Test
    fun testSpeakFallsBackToNextProviderOnFailure() = runTest {
        // 首选 Provider 抛异常 → 降级到下一个 Provider（降级链核心语义）
        runBlocking {
            whenever(edgeProvider.speak("测试")).thenThrow(RuntimeException("edge boom"))
        }
        controller.speak("测试")
        verify(edgeProvider).speak("测试")
        verify(systemProvider).speak("测试")
    }

    @Test
    fun testSpeakAllProvidersFailSilently() = runTest {
        // 全部失败 → 静默降级，不向调用方抛错（契约 throws: []）
        runBlocking {
            whenever(edgeProvider.speak("测试")).thenThrow(RuntimeException("edge boom"))
            whenever(systemProvider.speak("测试")).thenThrow(RuntimeException("system boom"))
        }
        controller.speak("测试")
        verify(edgeProvider).speak("测试")
        verify(systemProvider).speak("测试")
    }

    @Test
    fun testSpeakWithSpecifiedProviderIdPrefersIt() = runTest {
        // 指定 providerId → 该引擎优先；成功即停，不再触碰降级链其余部分
        controller.speak("测试", providerId = "system_tts")
        verify(systemProvider).speak("测试")
        verify(edgeProvider, never()).speak(any())
    }

    @Test
    fun testSpeakWithUnknownProviderIdFallsThroughCascade() = runTest {
        // 指定不存在的 providerId → 空首选 + 全量降级链
        controller.speak("测试", providerId = "cosyvoice_ws")
        verify(edgeProvider).speak("测试")
    }

    @Test
    fun testSpeakSwallowsExceptionWithoutProviders() = runTest {
        val empty = VoiceIoController(context, emptyList(), listOf(asrService))
        empty.speak("测试") // 无 provider：静默，不崩溃
        verify(edgeProvider, never()).speak(any())
    }

    @Test
    fun testSpeakSanitizesBeforeForwardingToProvider() = runTest {
        // task-fix-tts-sanitize：speak 内部第一行统一净化（[emotion] 标签 + 颜文字），
        // 到达 Provider 前的文本必须已剥离——覆盖 MainActivity 两条播报路径（统一入口）。
        controller.speak("[joy] 主人好喵 (=^･ω･^=)")
        verify(edgeProvider).speak("主人好喵")
        verify(systemProvider, never()).speak(any())
    }

    @Test
    fun testSpeakSkipsProviderWhenSanitizedTextIsBlank() = runTest {
        // 纯标签/纯颜文字 → 净化后为空 → 不进入降级链（与空文本短路同语义）
        controller.speak("(=^･ω･^=)")
        verify(edgeProvider, never()).speak(any())
        verify(systemProvider, never()).speak(any())
    }

    // ─────────────────────────────────────────────
    // isSpeaking（组合播放状态）
    // ─────────────────────────────────────────────

    @Test
    fun testIsSpeakingReflectsAnyProviderSpeaking() {
        // 任一 Provider 播放中 → 组合状态 true；全部空闲 → false
        edgeSpeaking.value = true
        scheduler.advanceUntilIdle()
        assertTrue("edge 播放中时组合状态应为 true", controller.isSpeaking.value)

        edgeSpeaking.value = false
        scheduler.advanceUntilIdle()
        assertFalse("edge 停止后组合状态应为 false", controller.isSpeaking.value)

        systemSpeaking.value = true
        scheduler.advanceUntilIdle()
        assertTrue("system 播放中时组合状态应为 true", controller.isSpeaking.value)
        scheduler.advanceUntilIdle()
    }

    // ─────────────────────────────────────────────
    // listen（听）— V1.2 模块4 ASR 降级链遍历
    // ─────────────────────────────────────────────

    @Test
    fun testListenReturnsRecognizedText() = runTest {
        runBlocking { whenever(asrService.listen()).thenReturn("你好世界") }
        assertEquals("你好世界", controller.listen())
    }

    @Test
    fun testListenReturnsEmptyOnAsrFailure() = runTest {
        // ASR 抛异常 → 返回空字符串（UI 无变化，降级为文本输入）
        runBlocking { whenever(asrService.listen()).thenThrow(RuntimeException("asr boom")) }
        assertEquals("", controller.listen())
    }

    @Test
    fun testListenFallsBackToNextProviderWhenFirstEmpty() = runTest {
        // 降级链：sherpa(0) 返回空 → 系统 ASR(1) 成功 → 返回系统结果（plan §4.4.4）
        val sherpa = mockAsrProvider("sherpa_sensevoice", 0)
        val system = mockAsrProvider("system_speech_recognizer", 1)
        runBlocking {
            whenever(sherpa.listen()).thenReturn("")
            whenever(system.listen()).thenReturn("系统识别结果")
        }
        val chainController = VoiceIoController(context, listOf(edgeProvider), listOf(sherpa, system))
        assertEquals("系统识别结果", chainController.listen())
        verify(sherpa).listen()
        verify(system).listen()
    }

    @Test
    fun testListenPrefersProviderIdFirst() = runTest {
        // V1.2 模块5：listen(providerId) — 指定首选 ASR 引擎（SettingsRepository.asrProviderId），
        // 指定引擎先被尝试；失败/为空后仍按降级链顺序兜底
        val sherpa = mockAsrProvider("sherpa_sensevoice", 0)
        val system = mockAsrProvider("system_speech_recognizer", 1)
        runBlocking {
            // 首选 system（priority=1）：应优先于 priority=0 的 sherpa 被尝试
            whenever(system.listen()).thenReturn("系统识别结果")
            whenever(sherpa.listen()).thenReturn("不应被尝试") // 首个 provider 成功即返回，sherpa 不该被调
        }
        val chainController = VoiceIoController(context, listOf(edgeProvider), listOf(sherpa, system))
        assertEquals("系统识别结果", chainController.listen("system_speech_recognizer"))
        // 首选引擎先被调用；sherpa 未被触碰（成功即返回，无降级）
        verify(system).listen()
        verify(sherpa, never()).listen()
    }

    @Test
    fun testListenPreferredProviderFailsThenFallsBackInPriorityOrder() = runTest {
        // 首选引擎失败 → 其余 Provider 仍按降级链 priority 顺序兜底（plan §4.4.4）
        val sherpa = mockAsrProvider("sherpa_sensevoice", 0)
        val system = mockAsrProvider("system_speech_recognizer", 1)
        runBlocking {
            whenever(system.listen()).thenReturn("") // 首选 system 返回空 → 降级
            whenever(sherpa.listen()).thenReturn("离线识别")
        }
        val chainController = VoiceIoController(context, listOf(edgeProvider), listOf(sherpa, system))
        assertEquals("离线识别", chainController.listen("system_speech_recognizer"))
        verify(system).listen()
        verify(sherpa).listen()
    }

    @Test
    fun testListenReturnsEmptyWhenAllProvidersFail() = runTest {
        // 全部失败/为空 → 静默返回 ""（契约 AsrProvider.listen 语义）
        val sherpa = mockAsrProvider("sherpa_sensevoice", 0)
        val system = mockAsrProvider("system_speech_recognizer", 1)
        runBlocking {
            whenever(sherpa.listen()).thenThrow(RuntimeException("sherpa boom"))
            whenever(system.listen()).thenReturn("")
        }
        val chainController = VoiceIoController(context, listOf(edgeProvider), listOf(sherpa, system))
        assertEquals("", chainController.listen())
        verify(system).listen()
    }

    @Test
    fun testListenTimesOutAndReturnsEmpty() = runTest {
        // 超时保护：Provider 挂起不返回 → 超时后返回 "" 且调用 stopListening（降级）
        var stopped = false
        val hanging = object : AsrProvider {
            override val info = AsrProviderInfo(
                id = "sherpa_sensevoice", name = "hanging", requiresNetwork = false,
                health = MutableStateFlow(TtsHealth.OK), priority = 0,
            )
            override suspend fun listen(): String =
                kotlinx.coroutines.suspendCancellableCoroutine { } // 永久挂起（等超时取消）
            override fun stopListening() {
                stopped = true
            }
            override fun release() {}
        }
        val system = mockAsrProvider("system_speech_recognizer", 1)
        runBlocking { whenever(system.listen()).thenReturn("") }
        val chainController = VoiceIoController(context, listOf(edgeProvider), listOf(hanging, system))
        assertEquals("", chainController.listen())
        assertTrue("超时应触发 stopListening", stopped)
        // 超时按空结果处理 → 继续尝试下一个 Provider（降级链语义）
        verify(system).listen()
    }

    @Test
    fun testListenWithEmptyChainReturnsEmpty() = runTest {
        val empty = VoiceIoController(context, listOf(edgeProvider), emptyList<AsrProvider>())
        assertEquals("", empty.listen())
    }

    // ─────────────────────────────────────────────
    // stopSpeaking（打断）
    // ─────────────────────────────────────────────

    @Test
    fun testStopSpeakingStopsAllProviders() {
        // plan §3.1.7：遍历降级链调用 stop()，确保切 Provider 后旧引擎资源释放
        controller.stopSpeaking()
        verify(edgeProvider).stop()
        verify(systemProvider).stop()
    }

    // ─────────────────────────────────────────────
    // release（释放）
    // ─────────────────────────────────────────────

    @Test
    fun testReleaseReleasesAllProvidersAndAsr() {
        controller.release()
        verify(edgeProvider).release()
        verify(systemProvider).release()
        verify(asrService).stopListening()
    }

    @Test
    fun testReleaseAfterReleaseIsIdempotent() {
        controller.release()
        controller.release()
        // 多次 release 不崩溃，且每次都会对所有 Provider 调用 release()（幂等）
        verify(edgeProvider, org.mockito.kotlin.atLeastOnce()).release()
        verify(systemProvider, org.mockito.kotlin.atLeastOnce()).release()
    }
}
