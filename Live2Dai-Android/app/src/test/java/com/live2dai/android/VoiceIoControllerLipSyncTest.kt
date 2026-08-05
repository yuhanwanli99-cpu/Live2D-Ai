package com.live2dai.android

import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestCoroutineScheduler
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.mockito.ArgumentMatchers.anyFloat
import org.mockito.kotlin.argumentCaptor
import org.mockito.kotlin.atLeast
import org.mockito.kotlin.atLeastOnce
import org.mockito.kotlin.mock
import org.mockito.kotlin.never
import org.mockito.kotlin.verify
import kotlinx.coroutines.ExperimentalCoroutinesApi

/**
 * VoiceIoController 模块4 LipSync 驱动单元测试（V1.1 方案A：播放状态驱动）。
 *
 * 验证链路（契约 docs/plan-v11-plan.schema.json §5.4）：
 * - 构造时向 EdgeTtsService 注册 onPlaybackStateChanged 回调
 * - 回调 true（播放开始）→ 嘴型正弦动画协程启动，周期性调用 setLipSyncParam
 * - 回调 false（播放结束/中断）→ 协程取消，嘴型归零 setLipSyncParam(0f)
 * - Live2DView 未接线时动画跳过（不崩溃）
 * - release() 停止动画并解绑
 *
 * 协程用 StandardTestDispatcher + 虚拟时间控制，不触碰真实主线程/GL。
 */
@OptIn(ExperimentalCoroutinesApi::class)
class VoiceIoControllerLipSyncTest {

    private lateinit var context: Context
    private lateinit var ttsService: EdgeTtsService
    private lateinit var asrService: VoiceInputService
    private lateinit var view: Live2DView
    private lateinit var scheduler: TestCoroutineScheduler
    private lateinit var controller: VoiceIoController

    @Before
    fun setUp() {
        context = mock()
        ttsService = mock()
        asrService = mock()
        view = mock()
        scheduler = TestCoroutineScheduler()
        Dispatchers.setMain(StandardTestDispatcher(scheduler))
        controller = VoiceIoController(
            context, ttsService, asrService, view,
            CoroutineScope(SupervisorJob() + Dispatchers.Main)
        )
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    // ─────────────────────────────────────────────
    // 接线
    // ─────────────────────────────────────────────

    @Test
    fun playbackCallbackRegisteredBeforeSpeak() {
        // speak() 首次播报前注册播放状态回调（契约 schema §5.4）
        runBlocking { controller.speak("测试") }
        val captor = argumentCaptor<(Boolean) -> Unit>()
        verify(ttsService).onPlaybackStateChanged = captor.capture()
        val callback = captor.firstValue
        assertNotNull("onPlaybackStateChanged 必须注册回调", callback)
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

    @Test
    fun mouthAnimationFollowsSineFormula() {
        controller.handlePlaybackState(true)
        // frame 0/1/2 对应 t=0/100/200ms：值应等于 LipSyncMath 公式输出（证明嘴型在动）
        scheduler.advanceTimeBy(VoiceIoController.LIP_SYNC_INTERVAL_MS * 3)
        verify(view, atLeastOnce()).setLipSyncParam(LipSyncMath.mouthValue(0))
        verify(view, atLeastOnce()).setLipSyncParam(LipSyncMath.mouthValue(1))
        verify(view, atLeastOnce()).setLipSyncParam(LipSyncMath.mouthValue(2))
        // 三个值互不相同（正弦波动，非恒定值）
        val values = setOf(
            LipSyncMath.mouthValue(0),
            LipSyncMath.mouthValue(1),
            LipSyncMath.mouthValue(2)
        )
        assertTrue("嘴型值应有变化，实际: $values", values.size >= 2)
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
    // 端到端：真实回调路径
    // ─────────────────────────────────────────────

    @Test
    fun ttsCallbackStartsAndStopsLipSync() {
        // 触发真实接线路径：speak() 注册回调 → 捕获回调 → 驱动 LipSync
        runBlocking { controller.speak("测试") }
        val captor = argumentCaptor<(Boolean) -> Unit>()
        verify(ttsService).onPlaybackStateChanged = captor.capture()
        val callback = captor.firstValue

        // EdgeTtsService 回调 true → LipSync 动画启动
        callback(true)
        scheduler.advanceTimeBy(VoiceIoController.LIP_SYNC_INTERVAL_MS * 3)
        verify(view, atLeast(2)).setLipSyncParam(anyFloat())

        // EdgeTtsService 回调 false → 嘴型归零
        callback(false)
        scheduler.advanceUntilIdle()
        verify(view, atLeastOnce()).setLipSyncParam(0f)
    }

    // ─────────────────────────────────────────────
    // 未接线 / 释放
    // ─────────────────────────────────────────────

    @Test
    fun lipSyncSkippedWhenViewNotAttached() {
        val detached = VoiceIoController(context, ttsService, asrService)
        detached.handlePlaybackState(true)
        scheduler.advanceTimeBy(VoiceIoController.LIP_SYNC_INTERVAL_MS * 5)
        verify(view, never()).setLipSyncParam(anyFloat())
        detached.release()
    }

    @Test
    fun releaseStopsLipSyncAndReleasesResources() {
        controller.handlePlaybackState(true)
        scheduler.advanceTimeBy(VoiceIoController.LIP_SYNC_INTERVAL_MS * 2)
        controller.release()
        scheduler.advanceUntilIdle()
        verify(ttsService).release()
        verify(asrService).stopListening()
        verify(view, atLeastOnce()).setLipSyncParam(0f)
    }
}
