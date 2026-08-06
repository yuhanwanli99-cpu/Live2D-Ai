package com.live2dai.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Debug 短周期开关验证（任务自证要求：断言 DEBUG 下周期/阈值与 release 不同）。
 *
 * docs/plan-v12-plan.md §5.6.4：debug 构建必须内置缩短版周期/阈值，否则长周期功能
 * E2E 验收几乎无法等待（记忆插件 10 轮阈值教训）。
 */
class ProactiveTuningTest {

    @Test
    fun debugAndRelease_timingValuesDiffer() {
        assertNotEquals(
            "周期：debug（15min 钳制下限）应不同于 release（1h）",
            ProactiveTuning.DEBUG.workManagerIntervalMinutes,
            ProactiveTuning.RELEASE.workManagerIntervalMinutes,
        )
        assertNotEquals(
            "空闲阈值：debug（5min）应不同于 release（30min）",
            ProactiveTuning.DEBUG.idleThresholdMinutes,
            ProactiveTuning.RELEASE.idleThresholdMinutes,
        )
        assertNotEquals(
            "冷却：debug（2min）应不同于 release（6h）",
            ProactiveTuning.DEBUG.cooldownMinutes,
            ProactiveTuning.RELEASE.cooldownMinutes,
        )
    }

    @Test
    fun debugValues_areShorterThanRelease() {
        assertTrue(ProactiveTuning.DEBUG.workManagerIntervalMinutes < ProactiveTuning.RELEASE.workManagerIntervalMinutes)
        assertTrue(ProactiveTuning.DEBUG.idleThresholdMinutes < ProactiveTuning.RELEASE.idleThresholdMinutes)
        assertTrue(ProactiveTuning.DEBUG.cooldownMinutes < ProactiveTuning.RELEASE.cooldownMinutes)
    }

    @Test
    fun debugValues_matchE2eAcceptanceDefaults() {
        // E2E 验收口径（任务 §9）：debug 周期/阈值缩短（周期 5min→15min 硬约束下限、阈值 2min 档）、
        // release 用生产值（周期 1h、阈值 30min）
        assertEquals(15L, ProactiveTuning.DEBUG.workManagerIntervalMinutes)
        assertEquals(5L, ProactiveTuning.DEBUG.idleThresholdMinutes)
        assertEquals(2L, ProactiveTuning.DEBUG.cooldownMinutes)

        assertEquals(60L, ProactiveTuning.RELEASE.workManagerIntervalMinutes)
        assertEquals(30L, ProactiveTuning.RELEASE.idleThresholdMinutes)
        assertEquals(360L, ProactiveTuning.RELEASE.cooldownMinutes)
    }

    @Test
    fun forDebug_selectsByBuildType() {
        assertEquals(ProactiveTuning.DEBUG, ProactiveTuning.forDebug(true))
        assertEquals(ProactiveTuning.RELEASE, ProactiveTuning.forDebug(false))
    }
}
