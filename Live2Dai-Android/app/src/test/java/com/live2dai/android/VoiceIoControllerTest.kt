package com.live2dai.android

import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
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
 * TTS Provider（TtsProvider）以 Mockito mock 注入，验证控制器的
 * 接线语义（降级链遍历 / 指定引擎 / 容错 / 打断 / 释放 / 空文本跳过 / 组合 isSpeaking），
 * 不触碰真实网络与系统服务。
 *
 * 契约见 docs/plan-v12-plan.schema.json → functions.VoiceIoController.*：
 * - speak(text, providerId?): 降级链遍历播报，失败静默（throws: []）
 * - stopSpeaking(): 同步打断所有 Provider 播报
 * - isSpeaking: StateFlow，任一 Provider 播放中 = true
 * - release(): 释放所有 Provider 资源
 */
@OptIn(ExperimentalCoroutinesApi::class)
class VoiceIoControllerTest {

    private lateinit var context: Context
    private lateinit var edgeProvider: TtsProvider
    private lateinit var systemProvider: TtsProvider
    private lateinit var edgeSpeaking: MutableStateFlow<Boolean>
    private lateinit var systemSpeaking: MutableStateFlow<Boolean>
    private lateinit var scheduler: TestCoroutineScheduler
    private lateinit var lipSyncScope: CoroutineScope
    private lateinit var controller: VoiceIoController

    @Before
    fun setUp() {
        // M-AND-3 (C3) deviation：生产默认 lipSyncScope 由 Dispatchers.Main → Dispatchers.Default。
        // 为保持测试调度器可驱动 isSpeaking 组合观察协程（combine{...any}.collect），
        // 显式注入 StandardTestDispatcher 作用域（构造器支持 lipSyncScope 注入），并用
        // scheduler.advanceUntilIdle() 驱动虚拟时间（断言口径不变）。
        scheduler = TestCoroutineScheduler()
        Dispatchers.setMain(StandardTestDispatcher(scheduler))
        lipSyncScope = CoroutineScope(SupervisorJob() + StandardTestDispatcher(scheduler))

        context = mock<Context>()
        mockProvider("edge_tts", 1).let {
            edgeProvider = it.first
            edgeSpeaking = it.second
        }
        mockProvider("system_tts", 2).let {
            systemProvider = it.first
            systemSpeaking = it.second
        }
        controller = VoiceIoController(
            context,
            listOf(edgeProvider, systemProvider),
            lipSyncScope = lipSyncScope,
        )
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
        val empty = VoiceIoController(context, emptyList())
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
    fun testReleaseReleasesAllProviders() {
        controller.release()
        verify(edgeProvider).release()
        verify(systemProvider).release()
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
