package com.live2dai.android

import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestCoroutineScheduler
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.mockito.ArgumentMatcher
import org.junit.After
import org.junit.Assert.assertNotNull
import org.junit.Before
import org.junit.Test
import org.mockito.kotlin.atLeastOnce
import org.mockito.kotlin.mock
import org.mockito.kotlin.never
import org.mockito.kotlin.verify
import org.mockito.kotlin.whenever

/**
 * Wave 2 · 2B-2 帧链「打断幂等」（用户裁定 B-4 = T1 预发射整段）— RED 测试。
 *
 * 契约锚（`.pi/spoq/contract-refactor-0814.md` §2.2 不变式 / 架构 B §2.3/§3.3）：
 *   - TtsProvider.audioFrameFlow「replay=0；stop() 后不再 emit」
 *   - 打断幂等：stop() 后任意到达的新帧 → 零输出（嘴型参数零写入，setLipSyncParam 归 0），
 *     不残留、不产生非零张/闭写入。
 *   - 「构造时 collect 帧流」；「打断后 isSpeaking 收敛 false 且帧源停发」
 *   - 再次 speak 后恢复（新一轮预发射帧可重新驱动嘴型）
 *
 * 隔离 seam：注入 fake 帧源（可控 audioFrameFlow 的 TtsProvider）+ fake Live2DView
 * （口径 `setLipSyncParam(Float)`）+ StandardTestDispatcher 虚拟时间。不触碰真实网络/GL。
 *
 * RED 状态：当前生产帧链未在 stop 后对「迟到的帧」做幂等钳制（stop 后仍可能被迟到帧驱动
 * 非零写入）→ 运行期 RED；若 stop 后帧流语义为「不再 emit」则迟到帧不存在于流，但消费侧
 * 对已缓冲帧表/迟到帧的钳制仍须成立——测试锚定「停止后 mouth 参数不得非零化」这一机械不变式。
 *
 * 纪律：禁读实现代码对答案；帧注入全程受控以便机械断言。
 */
@OptIn(ExperimentalCoroutinesApi::class)
class VoiceIoRefactorInterruptIdempotentTest {

    private lateinit var context: Context
    private lateinit var providerSpeaking: MutableStateFlow<Boolean>
    private lateinit var frameFlow: MutableSharedFlow<AudioFrame>
    private lateinit var view: Live2DView
    private lateinit var scheduler: TestCoroutineScheduler
    private lateinit var controller: VoiceIoController
    private lateinit var provider: TtsProvider

    private fun lipSyncScope(s: TestCoroutineScheduler): CoroutineScope =
        CoroutineScope(SupervisorJob() + StandardTestDispatcher(s))

    @Before
    fun setUp() {
        context = mock()
        providerSpeaking = MutableStateFlow(false)
        frameFlow = MutableSharedFlow(extraBufferCapacity = 64)
        val p = mock<TtsProvider>()
        whenever(p.info).thenReturn(
            TtsProviderInfo(
                id = "sherpa_onnx_kokoro", name = "fake-sherpa", category = TtsProviderCategory.LOCAL,
                requiresNetwork = false, health = MutableStateFlow(TtsHealth.OK), priority = 0,
                configurable = false,
            )
        )
        whenever(p.isSpeaking).thenReturn(providerSpeaking)
        whenever(p.audioFrameFlow).thenReturn(
            frameFlow as Flow<AudioFrame> // 可控 fake 帧源
        )
        provider = p
        view = mock()
        scheduler = TestCoroutineScheduler()
        Dispatchers.setMain(StandardTestDispatcher(scheduler))
        controller = VoiceIoController(
            context, listOf(provider), view,
            lipSyncScope(scheduler),
        )
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    private fun someAudioFrame(ts: Long, rms: Float = 0.3f): AudioFrame {
        val n = 150 * 24000 / 1000 // 150ms @24kHz
        val samples = FloatArray(n) { if (rms > 0f) 0.4f else 0f }
        return AudioFrame(samples, 24000, ts)
    }

    // ─────────────────────────────────────────────
    // 打断后帧源迟到帧 → 零输出（嘴量参数零写入）
    // ─────────────────────────────────────────────

    @Test
    fun framesArrivingAfterStop_produceNoNonZeroMouthWrite() {
        // 开始播放（帧链触发）
        controller.handlePlaybackState(true)

        // 打断（stop）
        controller.stopSpeaking()
        controller.handlePlaybackState(false)

        // stop 之后任意新帧到达 → 不得驱动任何非零 setLipSyncParam（幂等钳制）
        frameFlow.tryEmit(someAudioFrame(150L))   // 迟到帧
        frameFlow.tryEmit(someAudioFrame(300L, 0.9f)) // 高幅迟到帧
        scheduler.advanceTimeBy(VoiceIoController.LIP_SYNC_INTERVAL_MS * 3)

        // 零非零嘴量写入：stop 后任意 setLipSyncParam(x) 不得为 >0（只可为 0.0f 或完全不调用）
        controller.handlePlaybackState(false)
        scheduler.advanceUntilIdle()
        val positive = object : ArgumentMatcher<Float> {
            override fun matches(argument: Float?): Boolean = argument != null && argument > 0f
        }
        verify(view, never()).setLipSyncParam(org.mockito.kotlin.argThat(positive))
    }

    // ─────────────────────────────────────────────
    // 打断后恢复：再次 speak 后新帧重新驱动嘴型
    // ─────────────────────────────────────────────

    @Test
    fun afterStop_reSpeak_restoresFrameDrivenMouth() {
        // 第一轮：播放
        controller.handlePlaybackState(true)
        frameFlow.tryEmit(someAudioFrame(150L, 0.5f))
        Thread.sleep(50) // 帧收集跑真实线程（Dispatchers.Default）：虚拟时间 advance 前先让发射落地
        scheduler.advanceTimeBy(VoiceIoController.LIP_SYNC_INTERVAL_MS)
        // 打断并清场
        controller.stopSpeaking()
        controller.handlePlaybackState(false)
        scheduler.advanceUntilIdle()
        verify(view, atLeastOnce()).setLipSyncParam(0f) // 归零发生

        // 再次 speak（新一轮预发射）→ 嘴型恢复非零驱动
        controller.handlePlaybackState(true)
        frameFlow.tryEmit(someAudioFrame(0L))    // 首帧 ts=0
        frameFlow.tryEmit(someAudioFrame(150L, 0.6f))
        frameFlow.tryEmit(someAudioFrame(300L, 0.5f))
        Thread.sleep(50) // 帧收集跑真实线程（Dispatchers.Default）：虚拟时间 advance 前先让发射落地
        scheduler.advanceTimeBy(VoiceIoController.LIP_SYNC_INTERVAL_MS * 4)
        val positive = object : ArgumentMatcher<Float> {
            override fun matches(argument: Float?): Boolean = argument != null && argument > 0f
        }
        verify(view, atLeastOnce()).setLipSyncParam(org.mockito.kotlin.argThat(positive))

        // 收尾归零
        controller.handlePlaybackState(false)
        scheduler.advanceUntilIdle()
    }

    // ─────────────────────────────────────────────
    // stop 幂等：重复 stop 不崩溃、最终 mouth 归零
    // ─────────────────────────────────────────────

    @Test
    fun repeatedStop_isIdempotent_andZeroesMouth() {
        controller.handlePlaybackState(true)
        scheduler.advanceTimeBy(VoiceIoController.LIP_SYNC_INTERVAL_MS * 2)
        controller.stopSpeaking()
        controller.stopSpeaking() // 重复 stop 幂等，不崩溃
        controller.handlePlaybackState(false)
        scheduler.advanceUntilIdle()
        verify(view, atLeastOnce()).setLipSyncParam(0f)
    }

    // ─────────────────────────────────────────────
    // 未开始播放就 stop：无害（I7 fail-safe）
    // ─────────────────────────────────────────────

    @Test
    fun stopBeforePlayStart_isHarmless() {
        controller.stopSpeaking()
        controller.handlePlaybackState(false)
        scheduler.advanceUntilIdle()
        // 不崩溃（此处仅验证未抛异常路径可达；归零幂等）
        assertNotNull("controller 应可复用", controller)
    }
}
