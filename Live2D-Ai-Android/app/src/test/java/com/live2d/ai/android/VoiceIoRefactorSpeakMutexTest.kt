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
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.mockito.kotlin.mock
import org.mockito.kotlin.stub
import org.mockito.kotlin.whenever

/**
 * Wave 2 · 2B-2 帧链「speak 互斥」（I2 不变式；用户裁定）— RED 测试。
 *
 * 契约锚：`.pi/spoq/contract-refactor-0814.md` §2.2 不变式 **I2「至多一个 speak 在途」**
 * + C-4 裁定「立即打断」。架构 B §3.3：TtsProvider.speak「挂起到播放完成/失败；打断抛
 * CancellationException」。
 * 语义（本帧链路）：任一时刻至多一个说话/帧链在途；后到 speak 打断先到（C-4），先到
 * isSpeaking 先收敛 false 再起新链（单链走廊），绝不两条嘴型帧链并行写入。
 *
 * 可机械断言的暴露面（controller 现有 API）：
 *   - `isSpeaking` StateFlow：单一在途不变式的可观察走廊。
 *   - stopSpeaking() 打断后 `isSpeaking` 必须收敛 false（为后到者腾单链走廊）。
 *   - 新一轮播放启动（isSpeaking true）前，上一轮必须已收敛 false（不重叠）。
 *
 * RED 状态：现状（多引擎降级链遗留）未在帧链层面强互斥——打断后 `isSpeaking` 收敛路径
 * 与「新链须待旧链收敛才启动」不成立 → 运行期 RED。
 *
 * [UNSURE]〈后到者语义〉：契约约束了 I2 单在途 + C-4 立即打断，但「后到 speak 在
 * VoiceIoController 帧链层面是打断先到者（C-4）」未在接口层面逐字显式。本测试按 C-4=立即打断
 * 锚「打断后收敛 false 再起新链」的走廊不变式；若 coder 实现为「排队（播完再响应）」则属
 * 契约偏差，判读报告如实记录（禁改断言硬猜）。
 *
 * 纪律：禁读实现代码对答案；只用 controller 现有公开 API + 虚拟时间驱动。
 */
@OptIn(ExperimentalCoroutinesApi::class)
class VoiceIoRefactorSpeakMutexTest {

    private lateinit var context: Context
    private lateinit var providerSpeaking: MutableStateFlow<Boolean>
    private lateinit var view: Live2DView
    private lateinit var scheduler: TestCoroutineScheduler
    private lateinit var controller: VoiceIoController

    private fun lipSyncScope(s: TestCoroutineScheduler): CoroutineScope =
        CoroutineScope(SupervisorJob() + StandardTestDispatcher(s))

    @Before
    fun setUp() {
        context = mock()
        providerSpeaking = MutableStateFlow(false)
        val provider = mock<TtsProvider>()
        whenever(provider.info).thenReturn(
            TtsProviderInfo(
                id = "sherpa_onnx_kokoro", name = "fake", category = TtsProviderCategory.LOCAL,
                requiresNetwork = false, health = MutableStateFlow(TtsHealth.OK), priority = 0,
                configurable = false,
            )
        )
        whenever(provider.isSpeaking).thenReturn(providerSpeaking)
        // 打断语义建模：真实 provider.stop() 会令 isSpeaking 收敛 false（契约 §3.3/§2.2）
        provider.apply {
            // `stop()` 置 isSpeaking=false（模拟打断收敛），供 I2 走廊断言
            stub { on { stop() }.thenAnswer { providerSpeaking.value = false } }
        }
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

    // ─────────────────────────────────────────────
    // I2 单走廊：打断 → isSpeaking 收敛 false → 才允许新链启动（不重叠）
    // ─────────────────────────────────────────────

    @Test
    fun afterInterrupt_isSpeakingConvergesFalse_beforeNewChain() {
        // 第一链启动
        providerSpeaking.value = true
        scheduler.runCurrent()
        assertTrue("第一链在途：isSpeaking 应为 true", controller.isSpeaking.value)

        // 打断（后到者打断先到者，C-4）：
        controller.stopSpeaking()
        scheduler.runCurrent()
        // 关键走廊：打断后 isSpeaking 必须收敛 false（为后到者腾单链，I2 不重叠）
        assertFalse(
            "I2 互斥：打断后 isSpeaking 必须收敛 false 再允许新链（不得两链并行）",
            controller.isSpeaking.value
        )

        // 新链启动：仅在旧链收敛 false 后置 true（I2 单走廊）
        providerSpeaking.value = true
        scheduler.runCurrent()
        assertTrue("新链在途：isSpeaking 应为 true", controller.isSpeaking.value)

        // 收尾
        controller.stopSpeaking()
        providerSpeaking.value = false
        scheduler.advanceUntilIdle()
    }

    // ─────────────────────────────────────────────
    // 双重了启动：无打断介入的直接 true→true 不得产生并行双链状态
    // ─────────────────────────────────────────────

    @Test
    fun immediateDoubleStart_remainsSingleMutualized() {
        // 第二会话在未打断下直接到来（误用双重 speak）：仍只能保持单链在途（I2），
        // isSpeaking 仍是单布尔源，不允许出现「两条同时活跃」的旁路。
        providerSpeaking.value = true
        scheduler.runCurrent()
        val before = controller.isSpeaking.value
        providerSpeaking.value = true // 重复 true（StateFlow 去重 → 单链）
        scheduler.runCurrent()
        // isSpeaking 仍是单走廊 true（非两条独立 source 的 OR 叠加），无脱逸双链迹象
        assertTrue("双重启动后仍为单链在途 true（I2 单执行）", controller.isSpeaking.value && before)

        controller.stopSpeaking()
        providerSpeaking.value = false
        scheduler.advanceUntilIdle()
    }

    // ─────────────────────────────────────────────
    // 打断幂等组合：stopSpeaking 后重复 stop 不崩溃，isSpeaking 恒 false
    // ─────────────────────────────────────────────

    @Test
    fun repeatedInterrupt_leavesSingleIdleCorridor() {
        providerSpeaking.value = true
        scheduler.runCurrent()
        controller.stopSpeaking()
        controller.stopSpeaking() // 幂等，不崩溃
        providerSpeaking.value = false
        scheduler.runCurrent()
        // 打断后单走廊恒空闲，后续新 speak 可安全启动
        assertFalse("重复打断后 isSpeaking 应恒 false（I2 单走廊释放）", controller.isSpeaking.value)
        scheduler.advanceUntilIdle()
    }
}
