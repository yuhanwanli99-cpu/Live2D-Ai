package com.live2dai.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * LipSyncMath 纯函数单元测试（V1.1 模块4 方案A：播放状态驱动正弦嘴型，plan §4.1.4）。
 *
 * 验证 mouth 公式 0.65 + 0.25 * sin(frame * 0.5)：
 * - 任意帧号都在 [0.4, 0.9] 区间内（0=闭嘴, 1=最大张口）
 * - 确定性（同帧号同值）
 * - 随时间变化（嘴型在动，不是恒定值）
 */
class LipSyncMathTest {

    @Test
    fun mouthValueStaysWithinPlanRange() {
        // 0.65 ± 0.25 → 理论区间 [0.4, 0.9]，任何帧号都不越界
        for (frame in 0L..500L) {
            val value = LipSyncMath.mouthValue(frame)
            assertTrue(
                "frame=$frame value=$value 低于下限 ${LipSyncMath.MIN_MOUTH}",
                value >= LipSyncMath.MIN_MOUTH - 1e-4f
            )
            assertTrue(
                "frame=$frame value=$value 高于上限 ${LipSyncMath.MAX_MOUTH}",
                value <= LipSyncMath.MAX_MOUTH + 1e-4f
            )
        }
    }

    @Test
    fun mouthValueIsDeterministic() {
        for (frame in listOf(0L, 1L, 2L, 10L, 100L)) {
            assertEquals(
                "frame=$frame 两次计算应完全一致",
                LipSyncMath.mouthValue(frame),
                LipSyncMath.mouthValue(frame),
                0f
            )
        }
    }

    @Test
    fun mouthValueStartsAtMidpoint() {
        // sin(0)=0 → frame 0 恰为 0.65（区间中点，非闭嘴也非最大张口）
        assertEquals(0.65f, LipSyncMath.mouthValue(0), 1e-6f)
    }

    @Test
    fun mouthValueVariesAcrossFrames() {
        // 相邻帧数值持续变化（嘴型在动而非恒定值）
        val values = (0L..20L).map { LipSyncMath.mouthValue(it) }
        assertTrue(
            "20 帧内嘴型值应有明显变化，实际: $values",
            values.distinct().size >= 10
        )
    }
}
