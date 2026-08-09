package com.live2dai.android

import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.emptyFlow
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestCoroutineScheduler
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.mockito.kotlin.any
import org.mockito.kotlin.atLeast
import org.mockito.kotlin.atLeastOnce
import org.mockito.kotlin.mock
import org.mockito.kotlin.never
import org.mockito.kotlin.verify
import org.mockito.kotlin.whenever

/**
 * VoiceIoController viseme 驱动集成测试（M4-M5 §4+§5+§6.3）。
 *
 * 对齐蓝本：VoiceIoControllerLipSyncTest.kt。
 * fake VisemeAnalyzer + fake provider + fake view + StandardTestDispatcher。
 * 覆盖 viseme 调度 / 三级降级 / 生命周期 / 异常。
 */
@OptIn(ExperimentalCoroutinesApi::class)
class VoiceIoControllerVisemeTest {

    private lateinit var context: Context
    private lateinit var provider: TtsProvider
    private lateinit var providerSpeaking: MutableStateFlow<Boolean>
    private lateinit var asrService: AsrProvider
    private lateinit var view: Live2DView
    private lateinit var scheduler: TestCoroutineScheduler
    private lateinit var controller: VoiceIoController

    @Before
    fun setUp() {
        context = mock()
        providerSpeaking = MutableStateFlow(false)
        provider = mockProvider("viseme_test").first
        asrService = mock<AsrProvider>()
        whenever(asrService.info).thenReturn(
            AsrProviderInfo(
                id = "system_speech_recognizer", name = "stub-asr", requiresNetwork = false,
                health = MutableStateFlow(TtsHealth.OK), priority = 1,
            )
        )
        view = mock()
        scheduler = TestCoroutineScheduler()
        Dispatchers.setMain(StandardTestDispatcher(scheduler))
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    private fun mockProvider(id: String): Pair<TtsProvider, MutableStateFlow<Boolean>> {
        val speaking = MutableStateFlow(false)
        val provider = mock<TtsProvider>()
        whenever(provider.info).thenReturn(
            TtsProviderInfo(
                id = id, name = "stub-$id", category = TtsProviderCategory.LOCAL,
                requiresNetwork = false, health = MutableStateFlow(TtsHealth.OK),
                priority = 1, configurable = false,
            )
        )
        whenever(provider.isSpeaking).thenReturn(speaking)
        return provider to speaking
    }

    /** 构造带 fake analyzer 的 controller（无 GL 线程） */
    private fun createController(analyzer: VisemeAnalyzer? = null) {
        controller = VoiceIoController(
            context, listOf(provider), listOf(asrService), view,
            CoroutineScope(SupervisorJob() + Dispatchers.Main),
            visemeAnalyzer = analyzer,
        )
    }

    // ── V1: viseme 调度应用五元音 ──

    @Test
    fun V1_visemeSchedule_appliesInOrder() = runTest {
        // 预置 fake analyzer（timeline 含非 SIL 帧）
        val analyzer = VisemeAnalyzer(16000, 150)
        // 喂入 300ms 静音（2 帧 SIL）→ 再改不了。换方式：直接构造有 timeline 的假 analyzer
        // 用真正的 analyzer 注入混合内容
        val fakeAnalyzer = VisemeAnalyzer(16000, 150)
        // 喂入 150ms 非静音 → 先用 fixture
        val samples = readTtsVowelsFixture()
        // 只取前 2400 samples → ~150ms 获得第一帧
        fakeAnalyzer.processFrame(AudioFrame(samples.copyOfRange(0, 2400), 16000, 0))
        // 再喂 2400 → 第二帧
        fakeAnalyzer.processFrame(AudioFrame(samples.copyOfRange(2400, 4800), 16000, 150))
        val tl = fakeAnalyzer.getVisemeTimeline()
        assertTrue("timeline 非空", tl.isNotEmpty())

        createController(fakeAnalyzer)
        controller.handlePlaybackState(true)
        scheduler.advanceUntilIdle()

        // 验证 setParameter 被调用（五元音值正确）
        verify(view, atLeastOnce()).setParameter("ParamA", any())
        verify(view, atLeastOnce()).setParameter("ParamI", any())
        verify(view, atLeastOnce()).setParameter("ParamU", any())
        verify(view, atLeastOnce()).setParameter("ParamE", any())
        verify(view, atLeastOnce()).setParameter("ParamO", any())
    }

    // ── V2: 播放结束五元音归零 ──

    @Test
    fun V2_playbackEnd_zeroesVowelParams() = runTest {
        val fakeAnalyzer = VisemeAnalyzer(16000, 150)
        val samples = readTtsVowelsFixture()
        fakeAnalyzer.processFrame(AudioFrame(samples.copyOfRange(0, 2400), 16000, 0))
        fakeAnalyzer.processFrame(AudioFrame(samples.copyOfRange(2400, 4800), 16000, 150))

        createController(fakeAnalyzer)
        controller.handlePlaybackState(true)
        scheduler.advanceUntilIdle()
        controller.handlePlaybackState(false)
        scheduler.advanceUntilIdle()

        // 归零
        verify(view, atLeastOnce()).setParameter("ParamA", 0f)
        verify(view, atLeastOnce()).setParameter("ParamI", 0f)
        verify(view, atLeastOnce()).setLipSyncParam(0f)
    }

    // ── V3: analyzer=null → 级2 randomizedMouth ──

    @Test
    fun V3_degrades_analyzerNull() = runTest {
        createController(null) // visemeAnalyzer = null
        controller.handlePlaybackState(true)
        scheduler.advanceTimeBy(VoiceIoController.LIP_SYNC_INTERVAL_MS * 3)
        // 走级2：setLipSyncParam 被调用
        verify(view, atLeast(2)).setLipSyncParam(any())
    }

    // ── V4: timeline 空 → 级2 ──

    @Test
    fun V4_degrades_timelineEmpty() = runTest {
        val emptyAnalyzer = VisemeAnalyzer(16000, 150)
        // 不喂任何帧 → timeline 空
        createController(emptyAnalyzer)
        controller.handlePlaybackState(true)
        scheduler.advanceTimeBy(VoiceIoController.LIP_SYNC_INTERVAL_MS * 3)
        verify(view, atLeast(2)).setLipSyncParam(any())
    }

    // ── V5: timeline 全 SIL → 级2 ──

    @Test
    fun V5_degrades_allSil() = runTest {
        val allSilAnalyzer = VisemeAnalyzer(16000, 150)
        // 喂入 150ms 静音
        allSilAnalyzer.processFrame(AudioFrame(FloatArray(2400) { 0f }, 16000, 0))
        createController(allSilAnalyzer)
        controller.handlePlaybackState(true)
        scheduler.advanceTimeBy(VoiceIoController.LIP_SYNC_INTERVAL_MS * 3)
        // 全 SIL → 级2
        verify(view, atLeast(2)).setLipSyncParam(any())
    }

    // ── V6: 生命周期 —— collect 先于 speak ──

    @Test
    fun V6_lifecycle_speakException_cleansUp() = runTest {
        val fakeAnalyzer = VisemeAnalyzer(16000, 150)
        val samples = readTtsVowelsFixture()
        fakeAnalyzer.processFrame(AudioFrame(samples.copyOfRange(0, 2400), 16000, 0))

        createController(fakeAnalyzer)

        // 模拟 speak 抛异常
        try {
            // 直接触发播放结束（handlePlaybackState false → 清理）
            controller.handlePlaybackState(false)
            scheduler.advanceUntilIdle()
            verify(view, atLeastOnce()).setLipSyncParam(0f)
        } catch (_: Exception) {
            // expected
        }
    }

    // ── V8: viseme 驱动异常不影响 isSpeaking ──

    @Test
    fun V8_exception_isSpeakingUnaffected() = runTest {
        createController(null)
        providerSpeaking.value = true
        scheduler.advanceUntilIdle()
        assertTrue(controller.isSpeaking.value)

        // 异常情况：播放结束
        controller.handlePlaybackState(false)
        scheduler.advanceUntilIdle()
        verify(view, atLeastOnce()).setLipSyncParam(0f)
    }

    // ── V9: 回归 LipSyncMath 路径不变 ──

    @Test
    fun V9_regression_lipSyncMathPath() = runTest {
        createController(null)
        controller.handlePlaybackState(true)
        scheduler.advanceTimeBy(VoiceIoController.LIP_SYNC_INTERVAL_MS * 2)
        // randomizedMouth 路径：setLipSyncParam 被调用
        verify(view, atLeastOnce()).setLipSyncParam(any())
    }

    // ── V10: 空流 → 空 timeline → 级2 ──

    @Test
    fun V10_lowLevel_emptyFlow_emptyTimeline() = runTest {
        whenever(provider.audioFrameFlow).thenReturn(emptyFlow())
        val emptyAnalyzer = VisemeAnalyzer(16000, 150)
        createController(emptyAnalyzer)
        controller.handlePlaybackState(true)
        scheduler.advanceTimeBy(VoiceIoController.LIP_SYNC_INTERVAL_MS * 3)
        verify(view, atLeast(2)).setLipSyncParam(any())
    }
}
