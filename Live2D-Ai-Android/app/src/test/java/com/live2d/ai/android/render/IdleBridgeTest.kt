package com.live2d.ai.android.render

import com.live2d.ai.android.data.IdleBridge
import com.live2d.ai.android.data.IdleBridgeConfig
import com.live2d.ai.android.data.IdleParamSink
import com.live2d.ai.android.data.IdleSample
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * A-3=C1 待机桥（程序化桥直写参数）桥接层 RED 测试。
 *
 * 依据（唯一真相源）：
 *  - .pi/spoq/architecture-refactor-render-0814.md §5.3（A-3 裁定 C1）：
 *      "IdleMotionController/SpeakingMotion/LiaisonArbiter 保留为纯 JVM 类，只改落点（直写参数 → engine）"
 *      ——待机桥 = 桥接层：取 producer 参数帧 → 节流 N 帧 → 写参数 sink。
 *  - 既有 data/IdleMotionControllerTest.kt（IdleMotionController.update(dt): IdleSample 纯函数，禁改只引用）。
 *
 * 桥只测【调用序 + 节流语义】，不测 IdleMotionController 内部数值（禁改内部，回归护栏 RG1-A3 覆盖既类）。
 *
 * 生产 seam 对齐（主代理下发，data/IdleBridge.kt 实际形状）：
 *   - `IdleBridge(sink: IdleParamSink, config: IdleBridgeConfig, producer: () -> Map<String,Float>)`；
 *   - `IdleParamSink.write(params: Map<String,Float>)`（fun interface，帧参数整体写出）；
 *   - `IdleBridgeConfig(throttleFrames: Int = 2, minIntervalMs: Long = 30)`；
 *   - `onFrame(frameIndex: Long): Boolean`（整帧界节流；首次调用强制写；返回本帧是否实际写）；
 *   - `forceFlush(): Map<String,Float>`（无参，返回本次写入帧）；
 *   - `writeCount: Long`。
 * 与原 RED 断言差异（报告已列）：producer 签名 (Float)->IdleSample → ()->Map<String,Float>（上游
 * IdleSample→参数帧映射由调用方完成）；onFrame 实参 Float 秒 → Long 帧序号；常量对象 →
 * IdleBridgeConfig 数据类默认值；forceFlush(deltaSeconds) → forceFlush()；TB05 期望值随
 * 「首次调用强制写」生产语义调整。
 */
class IdleBridgeTest {

    /** 记录所有写入帧的 sink（生产 seam：write(Map)）。 */
    private class RecordingSink : IdleParamSink {
        val writes = mutableListOf<Map<String, Float>>()
        override fun write(params: Map<String, Float>) { writes.add(params) }
    }

    // ═══════════════════════════════════════════════════════════
    // TB-01：节流 N 帧语义（throttle=2 → 每 2 帧写一次）
    // ═══════════════════════════════════════════════════════════

    @Test
    fun TB01_throttleEveryNthFrame_onFrameWriteFlag() {
        val sink = RecordingSink()
        val bridge = IdleBridge(sink, IdleBridgeConfig(throttleFrames = 2)) { mapOf("ParamEyeBallX" to 0.1f) }
        var writeFlags = 0
        for (i in 1..6) {
            if (bridge.onFrame(i.toLong())) writeFlags++
        }
        assertEquals("6 帧 throttle=2 → 写 3 次（帧 1/3/5）", 3, writeFlags)
        assertEquals("writeCount==onFrame true 次数", 3L, bridge.writeCount)
    }

    // ═══════════════════════════════════════════════════════════
    // TB-02：桥调 producer() 且把参数帧经 sink 写出（不改内部）
    // ═══════════════════════════════════════════════════════════

    @Test
    fun TB02_bridgeCallsProducerAndWritesFrameViaSink() {
        val sink = RecordingSink()
        var producerCalls = 0
        val bridge = IdleBridge(sink, IdleBridgeConfig(throttleFrames = 1)) {
            producerCalls++
            // stub producer：固定参数帧（eyeBallX=0.1, headAngleY=5.0, bodySway=0.05；
            // 上游把 IdleSample 映射为参数帧后传入）
            mapOf(
                "ParamEyeBallX" to 0.1f,
                "ParamEyeBallY" to 0f,
                "ParamAngleX" to 0f,
                "ParamAngleY" to 5.0f,
                "ParamBodyAngleZ" to 0.05f,
            )
        }
        val triggered = bridge.onFrame(1L)
        assertTrue("throttle=1 首帧即写", triggered)
        assertEquals("producer 恰调 1 次", 1, producerCalls)
        // 参数帧经 sink 写出（eyeBallX / headAngleY / bodySway 至少一个被写）
        val names = sink.writes.flatMap { it.keys }.toSet()
        assertTrue("eyeBallX 通道被写", names.contains("ParamEyeBallX"))
        assertTrue("headAngle 通道被写", names.contains("ParamAngleY") || names.contains("ParamAngleX"))
        val headWrite = sink.writes.firstOrNull { "ParamAngleY" in it }?.get("ParamAngleY")
        if (headWrite != null) assertEquals("headAngleY 值 == 参数帧值", 5.0f, headWrite, 1e-3f)
    }

    // ═══════════════════════════════════════════════════════════
    // TB-03：throttle=1 每帧写；throttle=3 每 3 帧写
    // ═══════════════════════════════════════════════════════════

    @Test
    fun TB03_throttleOneEveryFrame_throttleThreeEveryThree() {
        val sink1 = RecordingSink()
        val b1 = IdleBridge(sink1, IdleBridgeConfig(throttleFrames = 1)) { mapOf("ParamEyeBallX" to 0.1f) }
        for (i in 1..4) b1.onFrame(i.toLong())
        assertEquals("throttle=1 → 4 帧写 4 次", 4L, b1.writeCount)

        val sink3 = RecordingSink()
        val b3 = IdleBridge(sink3, IdleBridgeConfig(throttleFrames = 3)) { mapOf("ParamEyeBallX" to 0.1f) }
        for (i in 1..9) b3.onFrame(i.toLong())
        assertEquals("throttle=3 → 9 帧写 3 次（帧 1/4/7）", 3L, b3.writeCount)
    }

    // ═══════════════════════════════════════════════════════════
    // TB-04：默认配置锁（throttleFrames=2 / minIntervalMs=30 防御位）
    // ═══════════════════════════════════════════════════════════

    @Test
    fun TB04_defaultConfig_locked() {
        assertEquals("默认节流帧==2", 2, IdleBridgeConfig().throttleFrames)
        assertEquals("最小时距==30ms", 30L, IdleBridgeConfig().minIntervalMs)
    }

    // ═══════════════════════════════════════════════════════════
    // TB-05：forceFlush 无节流强制写（生产 seam：首次调用强制写 → 期望值按生产语义）
    // ═══════════════════════════════════════════════════════════

    @Test
    fun TB05_forceFlush_unthrottledWrites() {
        val sink = RecordingSink()
        val frame = mapOf("ParamEyeBallX" to 0.1f)
        val bridge = IdleBridge(sink, IdleBridgeConfig(throttleFrames = 5)) { frame }
        // 帧 1 首次调用强制写；帧 2/3 距上次 1/2 帧 < 5 → 不写
        bridge.onFrame(1L)
        bridge.onFrame(2L)
        bridge.onFrame(3L)
        assertEquals("throttle=5：帧 1 首写后帧 2/3 不写（生产 seam 首次强制写）", 1L, bridge.writeCount)
        // forceFlush 强制收敛刷新（不受节流限制）
        val flushed = bridge.forceFlush()
        assertEquals("forceFlush 强制写一次", 2L, bridge.writeCount)
        assertEquals("forceFlush 返回本次写入帧 == producer 帧", frame, flushed)
    }

    // ═══════════════════════════════════════════════════════════
    // TB-06：IdleSample 字段面稳定（上游映射消费的 5 字段，防生产改字段破坏桥上游）
    // ═══════════════════════════════════════════════════════════

    @Test
    fun TB06_idleSampleExposesConsultedFields() {
        val sample = IdleSample(eyeBallX = 0f, eyeBallY = 0f, headAngleX = 0f, headAngleY = 0f, bodySway = 0f)
        assertEquals(0f, sample.eyeBallX, 0f)
        assertEquals(0f, sample.eyeBallY, 0f)
        assertEquals(0f, sample.headAngleX, 0f)
        assertEquals(0f, sample.headAngleY, 0f)
        assertEquals(0f, sample.bodySway, 0f)
    }
}
