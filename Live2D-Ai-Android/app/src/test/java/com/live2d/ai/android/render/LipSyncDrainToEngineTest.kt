package com.live2d.ai.android.render

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Wave 3 · 3-1 LipSync drain → engineSetLipSync 传递测试（JVM，fake engine sink seam）。
 *
 * 依据: contract-refactor-0814.md §2.3（consumer 仅 GL 线程帧首 drain → renderer.setLipSyncParam → 官方
 *       LipSyncUpdater → clamp[0,1] → ParamA + ParamMouthOpenY + adapter mouthOpenIndices 批量写）+ 不变式②
 *       （口型批量写+clamp 在新动画系统必须有等价落点）+ schedule-refactor-0814.md Wave3-1 「LipSync
 *       drain→engineSetLipSync」。
 * 契约锚: §2.3（clamp[0,1]、≤1 帧生效）；不变式②（等价落点）。
 *
 * 生产 seam 对齐（主代理下发）：
 *   - `LipSyncDrainBridge.drainAndPush(value)`：clamp[0,1] → `sink.setLipSync(value)`；
 *     每帧至多推 1 次（帧内防重复：本帧已推过则后续调用忽略）；
 *   - `onFrameEnd()`：帧末复位，允许下一帧再次推送。
 *
 * 判定方式: `LipSyncDrainBridge`（drain-value → `LipSyncEngineSink.setLipSync(value)`）seam：
 *             (a) drain 值完整传给 engine sink（0~1 常规值不改）；
 *             (b) 越界值（<0 / >1）被 clamp[0,1]（§2.3「clamp[0,1]」硬约束）；
 *             (c) 边界 0（闭嘴）与 1（全开）直通；
 *             (d) 跨帧多次 drain 每次独立传递（帧内至多 1 次，onFrameEnd 复位后独立，无前值残留）；
 *             (e) 帧内第二次 drain 被忽略（生产 seam：≤1 帧生效），onFrameEnd 后恢复推送。
 *          注: `engineSetLipSync` 真实 JNI 调用不在 JVM seam（native 于 JVM 抛 UnsatisfiedLinkError，
 *          先例 EngineJniDeclarationTest）；本波锁 sink 接口面（FLS sink→engine 传递 + clamp），真实 engine
 *          接线归 Wave 6 device/诊断。
 */
class LipSyncDrainToEngineTest {

    /** 记录每次 setLipSync 值的 fake engine sink。 */
    private class FakeSink : LipSyncEngineSink {
        val pushes = mutableListOf<Float>()
        override fun setLipSync(value: Float) { pushes.add(value) }
    }

    /** 每个 value 视为独立一帧：drain 后 onFrameEnd 复位（生产 seam：帧内至多 1 次推送）。 */
    private fun pushThrough(sink: FakeSink, values: List<Float>) {
        val bridge = LipSyncDrainBridge(sink)
        for (v in values) {
            bridge.drainAndPush(v)
            bridge.onFrameEnd()
        }
    }

    @Test
    fun drainValue_isForwardedToEngineSink() {
        val sink = FakeSink()
        pushThrough(sink, listOf(0.42f, 0.87f))
        assertEquals("drain 值应完整传递给 engine sink", listOf(0.42f, 0.87f), sink.pushes)
    }

    @Test
    fun outOfRangeValues_areClampedTo_0_1() {
        val sink = FakeSink()
        pushThrough(sink, listOf(-0.3f, 1.5f))
        assertEquals("越界值应 clamp[0,1]（§2.3 硬约束）", listOf(0.0f, 1.0f), sink.pushes)
    }

    @Test
    fun boundaryValues_passThroughUnchanged() {
        val sink = FakeSink()
        pushThrough(sink, listOf(0.0f, 1.0f))
        // 0=闭嘴 1=全开（§2.3 语义）边界直通不破坏
        assertEquals(listOf(0.0f, 1.0f), sink.pushes)
    }

    @Test
    fun repeatedDrainAcrossFrames_isIndependent_noStateLeak() {
        val sink = FakeSink()
        val bridge = LipSyncDrainBridge(sink)
        bridge.drainAndPush(0.9f); bridge.onFrameEnd()
        bridge.drainAndPush(0.1f); bridge.onFrameEnd()
        bridge.drainAndPush(0.5f)
        // 跨帧每次独立传递，无前值残留
        assertEquals(listOf(0.9f, 0.1f, 0.5f), sink.pushes)
    }

    @Test
    fun clampProduced_noOutOfBoundsPushes() {
        val sink = FakeSink()
        pushThrough(sink, listOf(-5f, 0f, 0.33f, 1f, 7f))
        assertTrue("所有 push 值必须在 [0,1] 闭合区间", sink.pushes.all { it in 0f..1f })
    }

    @Test
    fun withinFrame_secondDrainIgnored_untilFrameEnd() {
        // 生产 seam：帧内防重复（§2.3「≤1 帧生效」）——同帧第二次 drain 忽略，onFrameEnd 后恢复
        val sink = FakeSink()
        val bridge = LipSyncDrainBridge(sink)
        bridge.drainAndPush(0.9f)
        bridge.drainAndPush(0.1f)   // 同帧第二次 → 忽略
        assertEquals("同帧仅首推 1 次", listOf(0.9f), sink.pushes)
        bridge.onFrameEnd()
        bridge.drainAndPush(0.5f)   // 新帧恢复
        assertEquals("onFrameEnd 复位后新帧可再推", listOf(0.9f, 0.5f), sink.pushes)
    }

    /** 审计 R1：NaN 输入防御——视为 0f（闭嘴），不透传 NaN。 */
    @Test
    fun nanInput_treatedAsZero() {
        val sink = FakeSink()
        val bridge = LipSyncDrainBridge(sink)
        bridge.drainAndPush(Float.NaN)
        assertEquals("NaN 透传时 sink 收到 0f 而非 NaN", listOf(0f), sink.pushes)
        assertTrue("NaN 不得穿透", sink.pushes.all { !it.isNaN() })
    }
}
