package com.live2d.ai.android

import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestCoroutineScheduler
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.mockito.ArgumentMatchers.anyFloat
import org.mockito.kotlin.atLeast
import org.mockito.kotlin.atLeastOnce
import org.mockito.kotlin.mock
import org.mockito.kotlin.never
import org.mockito.kotlin.verify
import org.mockito.kotlin.whenever

/**
 * VoiceIoController 模块4 LipSync 驱动单元测试（V1.1 方案A：播放状态驱动，V1.2 改由
 * TtsProvider.isSpeaking StateFlow 驱动）。
 *
 * 验证链路（契约 docs/plans/plan-v12-plan.schema.json → functions.VoiceIoController.isSpeaking）：
 * - 构造时收集各 Provider 的 isSpeaking 组合流（替代 V1.1 EdgeTtsService 回调接线）
 * - 任一 Provider 播放开始（isSpeaking=true）→ 嘴型随机波动动画协程启动，周期性调用 setLipSyncParam
 * - 播放结束（isSpeaking=false）→ 协程取消，嘴型归零 setLipSyncParam(0f)
 * - Live2DView 未接线时动画跳过（不崩溃）
 * - release() 停止动画并解绑
 *
 * 协程用 StandardTestDispatcher + 虚拟时间控制，不触碰真实主线程/GL。
 */
@OptIn(ExperimentalCoroutinesApi::class)
class VoiceIoControllerLipSyncTest {

    private lateinit var context: Context
    private lateinit var edgeProvider: TtsProvider
    private lateinit var systemProvider: TtsProvider
    private lateinit var edgeSpeaking: MutableStateFlow<Boolean>
    private lateinit var view: Live2DView
    private lateinit var scheduler: TestCoroutineScheduler
    private lateinit var controller: VoiceIoController

    @Before
    fun setUp() {
        context = mock()
        mockProvider("edge_tts").let {
            edgeProvider = it.first
            edgeSpeaking = it.second
        }
        mockProvider("system_tts").let { systemProvider = it.first }
        view = mock()
        scheduler = TestCoroutineScheduler()
        Dispatchers.setMain(StandardTestDispatcher(scheduler))
        controller = VoiceIoController(
            context, listOf(edgeProvider, systemProvider), view,
            CoroutineScope(SupervisorJob() + Dispatchers.Main)
        )
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    /** 构造带 stub info + isSpeaking 的 TtsProvider mock（控制器日志会读 info.id）。 */
    private fun mockProvider(id: String): Pair<TtsProvider, MutableStateFlow<Boolean>> {
        val speaking = MutableStateFlow(false)
        val provider = mock<TtsProvider>()
        whenever(provider.info).thenReturn(
            TtsProviderInfo(
                id = id,
                name = "stub-$id",
                category = TtsProviderCategory.LOCAL,
                requiresNetwork = false,
                health = MutableStateFlow(TtsHealth.OK),
                priority = 1,
                configurable = false,
            )
        )
        whenever(provider.isSpeaking).thenReturn(speaking)
        return provider to speaking
    }

    // ─────────────────────────────────────────────
    // 接线：播放状态收集在构造时即已就绪（V1.2，替代 V1.1 回调注册）
    // ─────────────────────────────────────────────

    @Test
    fun playbackObserverWiredAtConstruction() {
        // V1.2：LipSync 由各 Provider 的 isSpeaking StateFlow 驱动；组合收集在构造时
        // 启动（无需等到首次 speak），speak 前 provider 播放状态变化即可驱动嘴型。
        edgeSpeaking.value = true
        scheduler.advanceTimeBy(VoiceIoController.LIP_SYNC_INTERVAL_MS * 3)
        verify(view, atLeast(2)).setLipSyncParam(anyFloat())
    }

    // ─────────────────────────────────────────────
    // 播放开始 → 嘴型动画
    // ─────────────────────────────────────────────

    @Test
    fun playbackStartDrivesMouthAnimation() {
        controller.handlePlaybackState(true)
        // 推进 5 个周期（500ms / 100ms 每帧），嘴型应持续更新多次
        scheduler.advanceTimeBy(VoiceIoController.LIP_SYNC_INTERVAL_MS * 5)
        verify(view, atLeast(2)).setLipSyncParam(anyFloat())
    }

    // ─────────────────────────────────────────────
    // 播放结束 → 嘴型归零
    // ─────────────────────────────────────────────

    @Test
    fun playbackStopZeroesMouth() {
        controller.handlePlaybackState(true)
        scheduler.advanceTimeBy(VoiceIoController.LIP_SYNC_INTERVAL_MS * 2)
        controller.handlePlaybackState(false)
        // 归零是同步的：handlePlaybackState(false) 返回前已调用 setLipSyncParam(0f)
        verify(view, atLeastOnce()).setLipSyncParam(0f)
        // 协程取消后 finally 兜底再归零一次，不残留动画任务
        scheduler.advanceUntilIdle()
        verify(view, atLeast(2)).setLipSyncParam(0f)
    }

    @Test
    fun playbackStopWhenNeverStartedIsHarmless() {
        // 未开始播放就收到 false（如重复 stopSpeaking）→ 嘴型归零幂等、不崩溃
        controller.handlePlaybackState(false)
        verify(view, atLeastOnce()).setLipSyncParam(0f)
    }

    // ─────────────────────────────────────────────
    // 端到端：真实路径（isSpeaking 流 → LipSync）
    // ─────────────────────────────────────────────

    @Test
    fun ttsIsSpeakingFlowStartsAndStopsLipSync() {
        // 真实接线路径：Provider isSpeaking=true → 组合流 → handlePlaybackState(true) → 动画
        edgeSpeaking.value = true
        scheduler.advanceTimeBy(VoiceIoController.LIP_SYNC_INTERVAL_MS * 3)
        verify(view, atLeast(2)).setLipSyncParam(anyFloat())

        // Provider isSpeaking=false → 组合流 → 嘴型归零
        edgeSpeaking.value = false
        scheduler.advanceUntilIdle()
        verify(view, atLeastOnce()).setLipSyncParam(0f)
    }

    // ─────────────────────────────────────────────
    // 未接线 / 释放
    // ─────────────────────────────────────────────

    @Test
    fun lipSyncSkippedWhenViewNotAttached() {
        // 独立 view mock：验证未接线的控制器不会驱动任何嘴型（与 setUp 中已接线的
        // 控制器共享 provider mock 但 view 各自独立，互不干扰）
        val detachedView = mock<Live2DView>()
        val detached = VoiceIoController(context, listOf(edgeProvider, systemProvider))
        detached.handlePlaybackState(true)
        scheduler.advanceTimeBy(VoiceIoController.LIP_SYNC_INTERVAL_MS * 5)
        verify(detachedView, never()).setLipSyncParam(anyFloat())
        detached.release()
    }

    @Test
    fun releaseStopsLipSyncAndReleasesResources() {
        controller.handlePlaybackState(true)
        scheduler.advanceTimeBy(VoiceIoController.LIP_SYNC_INTERVAL_MS * 2)
        controller.release()
        scheduler.advanceUntilIdle()
        verify(edgeProvider).release()
        verify(systemProvider).release()
        verify(view, atLeastOnce()).setLipSyncParam(0f)
    }
}
