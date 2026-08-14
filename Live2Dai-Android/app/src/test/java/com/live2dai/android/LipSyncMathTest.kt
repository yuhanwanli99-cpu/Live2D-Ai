package com.live2dai.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * LipSyncMath 纯函数单元测试。
 *
 * randomizedMouth（伪随机嘴型）已按契约 M-AND-2（AND2-A2 结构断言）移除，
 * 对应旧用例已删除；RME 幅度驱动（rmsNormalized/applyEnvelope/SILENCE_THRESHOLD）
 * 属新契约断言，由 MAnd2RmsLipSyncTest 覆盖。本文件仅保留 smoothMouth 回归测试：
 * - 向目标线性插值（默认 factor=0.3），结果恒在 current/target 之间（不越界、不反向）
 * - 多次插值收敛到目标
 */
class LipSyncMathTest {

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
