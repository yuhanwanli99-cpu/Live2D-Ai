package com.live2dai.android.render

import org.junit.Assert.assertEquals
import org.junit.Test
import java.util.concurrent.atomic.AtomicReference

/**
 * Wave 2 · 2A-1 渲染替换 —— 帧序 seam 测试。
 *
 * 依据: contract-refactor-0814.md §2.3-④（GL 帧序 = drain → update(dt) → draw，GL 线程单线程模型）
 *       + schedule-refactor-0814.md Wave2A「帧序 seam 测试（drain→update→draw，§2.3-④）」。
 * 契约锚: §2.3-④「GL 帧序 = drain → update(dt) → draw（嘴型提交 ≤1 帧生效）」；
 *         §6.3「onDrawFrame 帧序：drain → update(dt) → draw」。
 *
 * 判定方式: 薄壳帧编排按 seam 接口 `FrameSinks`/`RendererFrameSequencer` 注入 fake
 *           （不依赖 GL），机械断言：
 *             (a) 调用序 === [drain, update(dt), draw]（顺序硬约束）；
 *             (b) 前置状态：drain 写入的参数在 update 可见（参数写入先于更新）；
 *             (c) 每个 dt 正确透传（更新先于绘制）。
 *           生产 seam 形态: `RendererFrameSequencer(onUpdate: (Float)->Unit, onDraw: ()->Unit)`
 *           + `frame(dt, sinks: FrameSinks)`（无 step）；Fake 覆写 `FrameSinks.drain()` 记录序
 *           并返回取走值，`recordUpdate/recordDraw` 挂到 onUpdate/onDraw 记录 update/draw 步。
 */
class RendererFrameOrderTest {

    // 注入的 fake 帧表：覆写 drain 记录序并返回取走值（Map<String,Float> 形态同生产 FrameSinks）；
    // recordUpdate/recordDraw 由 RendererFrameSequencer(onUpdate=…, onDraw=…) 驱动，记录调用序
    private class FakeFrameSinks : FrameSinks() {
        val order = mutableListOf<String>()     // 调用序
        var latestDt = -1f                       // 最近 update 透传的 dt
        var updateSawValue = Float.NaN           // update 时读到 drain 写入的参数值
        private val lipSyncValue = AtomicReference(0f)  // drain 写入 → update 读取

        override fun drain(): Map<String, Float> {
            order.add("drain")
            val v = currentFrameValue
            lipSyncValue.set(v)
            return mapOf("lip_sync" to v)
        }

        fun recordUpdate(dt: Float) {
            order.add("update")
            latestDt = dt
            // 前置状态断言：update 必须读到【本帧】drain 已写入的参数（参数写入先于更新）
            updateSawValue = lipSyncValue.get()
        }

        fun recordDraw() {
            order.add("draw")
        }

        fun feed(valuePerFrame: List<Float>) {
            pendingValues.addAll(valuePerFrame)
        }

        private val pendingValues = mutableListOf<Float>()
        private val currentFrameValue: Float
            get() = pendingValues.removeAt(0)
    }

    @Test
    fun frameStep_callsDrainThenUpdateThenDraw() {
        val sinks = FakeFrameSinks()
        val seq = RendererFrameSequencer(onUpdate = sinks::recordUpdate, onDraw = sinks::recordDraw)
        sinks.feed(listOf(0.5f))

        seq.frame(dt = 0.016f, sinks = sinks)

        assertEquals("调用序必须恰好为 [drain, update, draw]", listOf("drain", "update", "draw"), sinks.order)
    }

    @Test
    fun paramWrittenAtDrain_isVisibleToUpdate_sameFrame() {
        val sinks = FakeFrameSinks()
        val seq = RendererFrameSequencer(onUpdate = sinks::recordUpdate, onDraw = sinks::recordDraw)
        sinks.feed(listOf(0.8f))   // 本帧口型值 0.8

        seq.frame(dt = 0.016f, sinks = sinks)

        assertEquals(
            "参数写入先于更新：update 必须读到本帧 drain 写入的值 0.8，实际 ${sinks.updateSawValue}",
            0.8f, sinks.updateSawValue, 1e-5f,
        )
    }

    @Test
    fun dtIsTransmittedToUpdate() {
        val sinks = FakeFrameSinks()
        val seq = RendererFrameSequencer(onUpdate = sinks::recordUpdate, onDraw = sinks::recordDraw)
        sinks.feed(listOf(1f))

        seq.frame(dt = 0.033f, sinks = sinks)

        assertEquals("update(dt) 必须透传调用方 dt=0.033", 0.033f, sinks.latestDt, 1e-6f)
    }

    @Test
    fun multipleFrames_preserveDrainSeparatedFromDraw() {
        val sinks = FakeFrameSinks()
        val seq = RendererFrameSequencer(onUpdate = sinks::recordUpdate, onDraw = sinks::recordDraw)
        sinks.feed(listOf(0.2f, 0.9f, 0.0f))

        seq.frame(dt = 0.016f, sinks = sinks)
        seq.frame(dt = 0.016f, sinks = sinks)
        seq.frame(dt = 0.016f, sinks = sinks)

        // 每帧内序 = [drain, update, draw]；帧间不穿插帧段
        val expected = listOf("drain", "update", "draw", "drain", "update", "draw", "drain", "update", "draw")
        assertEquals("多帧调用序必须逐帧隔离且每帧 drain→update→draw", expected, sinks.order)
    }
}
