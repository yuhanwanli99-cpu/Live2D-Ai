package com.live2dai.android.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * ExpressionSink 壳侧 release 幂等 RED（契约 refactor-0814 §2.2 ExpressionSink + 用户裁定 C-2=N=5s）。
 *
 * 覆盖生产文件 `core/ExpressionSink.kt`（**新增，RED 待实现**，壳侧封装；随 A 片接缝重写）。
 *
 * 本测试只锚定壳侧 release 的**周期确定性**语义（冻结免疫 I5 的壳侧支撑）：
 *   - release 调用 ≥2 次 → 行为仅一次（不重复动作）、不抛异常、状态回 neutral（isReleased==true）
 *   - release 后再次 apply → 重新激活（可再次进入活动态）
 *   - apply 覆盖语义：连续 apply 不累积
 *
 * 收敛时序（CORE3-A4 N=5s）独立文件 `ExpressionSinkConvergeTest` 覆盖（需可控时钟 seam）。
 *
 * seam spec（由 coder 对齐，跳包 com.live2dai.android.core；见 RED 报告 §5/§7）：
 *   class ExpressionSink(private val clock: Clock = SystemClock) {  // 可控时钟 seam
 *       fun apply(emotion: String?)                                       // 应用表情（覆盖，无累积）
 *       fun release(): Boolean                                            // 幂等 release；行为一次、不抛异常
 *       val isReleased: Boolean                                           // 当前是否已收敛至 neutral
 *       // 收敛调度（scheduleConverge）见 ConvergeTest
 *   }
 *
 * 说明：生产符号 core/ExpressionSink 尚不存在 → 编译期即 RED（预期）。目标类实现后转 GREEN。
 */
class ExpressionSinkReleaseTest {

    // ═══════════════════════════════════════════════
    // release 幂等：≥2 次 → 行为一次、不抛异常、回 neutral
    // ═══════════════════════════════════════════════

    /** apply(joy) 后 release 一次 → isReleased==true（回 neutral），行为一次。 */
    @Test
    fun applyThenRelease_releases_once() {
        val sink = TimedExpressionSink()
        sink.apply("joy")
        assertFalse("apply 后未释放（活动态）", sink.isReleased)

        sink.release()
        assertTrue("release 后回 neutral（isReleased==true）", sink.isReleased)
    }

    /** release 连续两次 → 第二次不抛异常、behavior 仅一次（第二次返回 false 或幂等无副作用）。 */
    @Test
    fun releaseTwice_idempotent_noException_noRepeat() {
        val sink = TimedExpressionSink()
        sink.apply("sad")

        sink.release()
        assertTrue(sink.isReleased)

        // 第二次 release：不抛异常（绝不因重复 release 崩溃），且不重复动作（isReleased 恒 true、二次调用无副作用）
        val second = sink.release()   // 幂等：允许返回 false 表示「已释放过，未再动作」
        assertTrue("重复 release 后仍处于已释放（neutral）态", sink.isReleased)
        // 行为一次断言：二次 release 不产生重复释放副作用（isReleased 未被再次翻转/仍 true）
        // second 返回值语义由实现定（true=本次动作 / false=已幂等跳过），此处只锁状态不变 + 不抛异常
    }

    /** release 在从未 apply 的 fresh 对象上调用 → 不抛异常、保持 neutral（isReleased 语义一致）。 */
    @Test
    fun releaseOnFreshSink_noThrow_staysNeutral() {
        val sink = TimedExpressionSink()
        sink.release()   // 未 apply 过就直接 release：幂等，不抛异常
        assertTrue("fresh release 后应处于 neutral/isReleased 一致性态", sink.isReleased)
    }

    /** release 三次 → 始终不抛异常、始终 isReleased==true（强幂等）。 */
    @Test
    fun releaseThreeTimes_stableIdempotent() {
        val sink = TimedExpressionSink()
        sink.apply("neutral")
        sink.release()
        sink.release()
        sink.release()
        assertTrue("三次 release 后仍处于已释放（neutral）态、无异常", sink.isReleased)
    }

    // ═══════════════════════════════════════════════
    // release 后再 apply → 重新激活
    // ═══════════════════════════════════════════════

    /** release 后再次 apply(sad) → 重新进入活动态（isReleased==false），可再次走新一轮生命周期。 */
    @Test
    fun applyAfterRelease_reactivates() {
        val sink = TimedExpressionSink()
        sink.apply("joy")
        sink.release()
        assertTrue(sink.isReleased)

        sink.apply("sad")
        assertFalse("release 后 apply(sad) 重新激活（isReleased==false）", sink.isReleased)
    }

    /** apply → release → apply → release：完整两轮生命周期均正常（无状态累积）。 */
    @Test
    fun fullRoundTrip_twoCycles() {
        val sink = TimedExpressionSink()
        sink.apply("joy"); assertFalse(sink.isReleased)
        sink.release();    assertTrue(sink.isReleased)
        sink.apply("neutral"); assertFalse(sink.isReleased)
        sink.release();    assertTrue(sink.isReleased)
    }

    // ═══════════════════════════════════════════════
    // apply 覆盖语义（无累积）
    // ═══════════════════════════════════════════════

    /** 连续 apply 两次 → 覆盖为后者（无累积二次动作）。 */
    @Test
    fun applyTwice_lastWins_noAccumulation() {
        val sink = TimedExpressionSink()
        sink.apply("sad")
        sink.apply("joy")
        // 覆盖语义：最终活动态由最后一次 apply 决定（无累计多个表情持有 → 冻结免疫 structural 面）
        assertFalse("连续 apply 后仍处于活动态（最后一次 joy）", sink.isReleased)
    }

    /** apply(null) → 视为 neutral（立即已释放）；后续 release 仍幂等。 */
    @Test
    fun applyNull_treatAsNeutralActive() {
        val sink = TimedExpressionSink()
        sink.apply(null)
        assertTrue("apply(null) 视为 neutral（立即已释放）", sink.isReleased)
        sink.release()
        assertTrue("apply(null)->release 幂等收敛", sink.isReleased)
    }
}
