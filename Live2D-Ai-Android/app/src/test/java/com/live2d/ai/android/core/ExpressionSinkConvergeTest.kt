package com.live2d.ai.android.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * ExpressionSink 收敛时序 RED（契约 CORE3-A4 + 用户裁定 C-2=N=5s，冻结免疫 I5 壳侧时序面）。
 *
 * 覆盖生产文件 `core/ExpressionSink.kt` 的延迟收敛调度（5A 壳侧无条件收敛的 N 秒参数防漂移锁定）。
 *
 * 本测试用**可控时钟 seam**（FakeClock 可步进）确定性驱动时序：
 *   - apply(joy) 后经过 < 5s → 未收敛（isReleased==false）
 *   - 经过 >= 5s → 收敛触发（isReleased==true），且只触发一次
 *   - N=5s 常量锁定（C-2 裁定值防漂移）
 *   - 收敛后再次 apply → 新一轮 5s 时序
 *
 * 常量锁定（RED 报告 §5）：
 *   companion object { const val CONVERGE_DELAY = 5_000L /* C-2=N=5s 毫秒 */ }
 *
 * seam spec（由 coder 对齐）：
 *   interface Clock { fun now(): Long }                        // 可注入时钟（现 use nanoTime 或 localTime 归实现）
 *   class ExpressionSink(private val clock: Clock = SystemClock) {
 *       fun apply(emotion: String?)
 *       fun release(): Boolean
 *       val isReleased: Boolean
 *       // 收敛调度：实现侧在 apply 时记录时间戳并按 CONVERGE_DELAY_NS 到期判定；schedule 触发点 coder 择定
 *   }
 *   object SystemClock : Clock { override fun now() = System.nanoTime() }
 *
 * 说明：生产符号 core/ExpressionSink / Clock 尚不存在 → 编译期即 RED（预期）。目标类实现后转 GREEN。
 */
class ExpressionSinkConvergeTest {

    private class FakeClock : Clock {
        private var t = 0L
        override fun nowMillis(): Long = t
        fun advance(ms: Long) { t += ms }
    }

    // ═══════════════════════════════════════════════
    // N=5s 常量锁定（C-2 裁定值防漂移）
    // ═══════════════════════════════════════════════

    /** C-2 裁定：表情保持时长 N=5 秒（毫秒常量，防漂移）。 */
    @Test
    fun convergeDelay_constantIs5s_perC2() {
        assertEquals("C-2 裁定值 5s（毫秒）", 5_000L, TimedExpressionSink.CONVERGE_DELAY_MS)
        // 同步锁定 ns 换算（与时钟 seam 单位一致）
        // ms 口径已由上一条断言锁定（5_000L），ns 换算断言移除（实现 seam 为毫秒）
    }

    // ═══════════════════════════════════════════════
    // 收敛时序（可控时钟驱动）
    // ═══════════════════════════════════════════════

    /** apply(joy) 后经过 < 5s → 未收敛。 */
    @Test
    fun applyThenAdvanceBelow5s_notConverged() {
        val clock = FakeClock()
        val sink = TimedExpressionSink(clock)

        sink.apply("joy")
        clock.advance(TimedExpressionSink.CONVERGE_DELAY_MS - 1)   // 差 1ns 到 5s
        assertFalse("5s 前未收敛（isReleased==false）", sink.isReleased)
    }

    /** apply(joy) 后经过恰 >= 5s → 已收敛（回 neutral）。 */
    @Test
    fun applyThenAdvanceTo5s_converged() {
        val clock = FakeClock()
        val sink = TimedExpressionSink(clock)

        sink.apply("joy")
        clock.advance(TimedExpressionSink.CONVERGE_DELAY_MS)        // 恰 5s
        assertTrue("5s 时已收敛（isReleased==true，回 neutral）", sink.isReleased)
    }

    /** 收敛只触发一次：持续推进时钟不反复收敛/不抛异常。 */
    @Test
    fun convergence_triggersOnce_over5sStable() {
        val clock = FakeClock()
        val sink = TimedExpressionSink(clock)

        sink.apply("joy")
        clock.advance(TimedExpressionSink.CONVERGE_DELAY_MS)
        assertTrue(sink.isReleased)         // 首次 5s 收敛

        clock.advance(TimedExpressionSink.CONVERGE_DELAY_MS)  // 再推进 5s（共 10s）
        clock.advance(Long.MAX_VALUE / 4)   // 大幅推进
        assertTrue("超过 5s 后仍稳定处于已收敛态，无异常、无重复翻转", sink.isReleased)
    }

    /** 收敛后再次 apply → 新一轮 5s（重新激活，到期再收敛）。 */
    @Test
    fun reactivateAfterConvergence_startsNewCycle() {
        val clock = FakeClock()
        val sink = TimedExpressionSink(clock)

        sink.apply("joy")
        clock.advance(TimedExpressionSink.CONVERGE_DELAY_MS)
        assertTrue(sink.isReleased)          // 第一轮收敛

        sink.apply("sad")                    // 重新激活
        assertFalse("重新 apply 后进入活动态", sink.isReleased)
        clock.advance(TimedExpressionSink.CONVERGE_DELAY_MS)
        assertTrue("第二轮 5s 到期再收敛", sink.isReleased)
    }

    /** 时序推进后 release 手工调用仍幂等（时钟驱动收敛与手动 release 兼容）。 */
    @Test
    fun manualReleaseAfterConverge_idempotent() {
        val clock = FakeClock()
        val sink = TimedExpressionSink(clock)

        sink.apply("joy")
        clock.advance(TimedExpressionSink.CONVERGE_DELAY_MS)
        assertTrue(sink.isReleased)          // 时序已收敛

        sink.release()                       // 已收敛后再手动 release：幂等不抛异常
        assertTrue("时序收敛后手动 release 幂等（状态稳定 neutral）", sink.isReleased)
    }
}
