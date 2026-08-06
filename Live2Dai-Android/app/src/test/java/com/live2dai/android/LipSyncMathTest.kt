package com.live2dai.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * LipSyncMath 纯函数单元测试（级2-B：音频响度驱动的随机波动 + 停顿模拟，替代 V1.1 正弦嘴型）。
 *
 * 验证 randomizedMouth：
 * - 首帧从闭嘴起步（0.0）
 * - 说话中值 ∈ [0.3, 0.7] 波动，或 0.0（停顿闭口）——从不越界
 * - 确定性（帧号作随机种子 → 同帧同值）
 * - 停顿概率 ~8%（大样本统计落在 [1%, 30%] 安全统计带内）
 * - 跨帧持续变化（嘴型在动，非恒定值）
 *
 * 验证 smoothMouth：
 * - 向目标线性插值（默认 factor=0.3），结果恒在 current/target 之间（不越界、不反向）
 * - 多次插值收敛到目标
 */
class LipSyncMathTest {

    @Test
    fun randomizedMouthFirstFrameStartsClosed() {
        // 首帧语义：嘴型从闭嘴起步（配合 smoothMouth 平滑张开，模拟开口说话）
        assertEquals("首帧应从闭嘴起步", 0f, LipSyncMath.randomizedMouth(0, true), 0f)
        // 任意帧号 + isFirstFrame=true 都返回 0.0
        assertEquals(0f, LipSyncMath.randomizedMouth(123L, true), 0f)
    }

    @Test
    fun randomizedMouthStaysWithinFluctuationRangeOrPause() {
        // 说话中值要么是 0.0（停顿闭口），要么在随机波动区间 [MIN_MOUTH, MAX_MOUTH] 内
        for (frame in 1L..2000L) {
            val value = LipSyncMath.randomizedMouth(frame, false)
            val inFluctuation =
                value >= LipSyncMath.MIN_MOUTH - 1e-4f &&
                    value <= LipSyncMath.MAX_MOUTH + 1e-4f
            assertTrue(
                "frame=$frame value=$value 应为 0.0（停顿）或 " +
                    "[${LipSyncMath.MIN_MOUTH}, ${LipSyncMath.MAX_MOUTH}]（波动）",
                value == 0.0f || inFluctuation
            )
        }
    }

    @Test
    fun randomizedMouthIsDeterministic() {
        // 帧号作随机种子 → 同帧两次计算完全一致（可重复验证的确定性纯函数）
        for (frame in listOf(0L, 1L, 2L, 10L, 100L, 999L)) {
            assertEquals(
                "frame=$frame 两次计算应完全一致",
                LipSyncMath.randomizedMouth(frame, false),
                LipSyncMath.randomizedMouth(frame, false),
                0f
            )
        }
    }

    @Test
    fun randomizedMouthInsertsPausesOccasionally() {
        // 2000 帧统计：停顿（0.0）占比应约 8%（期望 160，σ≈12），
        // 落在 [1%, 30%] 安全统计带内（±20σ 外，杜绝偶发失败）
        val total = 2000
        val pauses = (1L..total.toLong()).count { LipSyncMath.randomizedMouth(it, false) == 0.0f }
        val ratio = pauses.toFloat() / total
        assertTrue(
            "停顿占比应接近 ${LipSyncMath.PAUSE_PROBABILITY}，实际 $pauses/$total=$ratio",
            ratio >= 0.01f && ratio <= 0.30f
        )
    }

    @Test
    fun randomizedMouthFluctuatesAcrossFrames() {
        // 说话中值应持续变化（嘴型在动而非恒定值）
        val values = (1L..200L).map { LipSyncMath.randomizedMouth(it, false) }
        assertTrue(
            "200 帧内嘴型值应有明显变化，实际: ${values.distinct().size} 个不同值",
            values.distinct().size >= 50
        )
    }

    @Test
    fun smoothMouthLerpsTowardTargetWithinBounds() {
        // 显式 factor=0.5 → 恰为中点
        assertEquals(0.5f, LipSyncMath.smoothMouth(0f, 1f, 0.5f), 1e-6f)
        // 默认 factor=0.3：向目标靠近 30%
        assertEquals(0.3f, LipSyncMath.smoothMouth(0f, 1f), 1e-6f)
        // 结果恒在 current/target 之间（不越界、不反向）
        for (i in 0..200) {
            val current = (i % 100) / 100f
            val target = ((i * 7) % 100) / 100f
            val smoothed = LipSyncMath.smoothMouth(current, target)
            val lo = minOf(current, target)
            val hi = maxOf(current, target)
            assertTrue(
                "smooth($current,$target)=$smoothed 越界",
                smoothed >= lo - 1e-6f && smoothed <= hi + 1e-6f
            )
        }
    }

    @Test
    fun smoothMouthConvergesToTarget() {
        // 多次插值后应逼近目标（收敛性，平滑不产生振荡）
        var current = 0f
        repeat(50) { current = LipSyncMath.smoothMouth(current, 0.7f) }
        assertEquals(0.7f, current, 1e-3f)
    }
}
