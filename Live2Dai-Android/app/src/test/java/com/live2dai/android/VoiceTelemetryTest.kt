package com.live2dai.android

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * VoiceTelemetry / VoiceEventBus（V1.2 模块7「可观测性」链路遥测）单元测试。
 *
 * 覆盖（plan §5.7.3 / schema dataModels.VoiceTelemetryEvent）：
 * - 事件模型：9 种 VoiceEvent 可构造、字段完整
 * - log()：Log.i + 环形缓冲区（最近 100 条，时间序）
 * - clear()：测试隔离
 * - VoiceEventBus：log() 转发到共享流，实时订阅可收到
 *
 * 纯 JVM：VoiceTelemetry.log 内部 Log.i 有 try/catch 兜底（本地单测无
 * Android 运行时，直接调 Log 会抛 "Method i in android.util.Log not mocked"），
 * 与 VoiceIoController.logI 同一容错模式。
 */
class VoiceTelemetryTest {

    @Before
    fun setUp() {
        VoiceTelemetry.clear()
    }

    @After
    fun tearDown() {
        VoiceTelemetry.clear()
    }

    // ─────────────────────────────────────────────
    // 事件模型（schema dataModels.VoiceTelemetryEvent）
    // ─────────────────────────────────────────────

    @Test
    fun testEventModelAllTypesConstructible() {
        val ts = 1234L

        val ttsStarted: VoiceEvent = VoiceEvent.TtsStarted("cosyvoice_ws", 10, ts)
        val ttsSucceeded: VoiceEvent = VoiceEvent.TtsSucceeded("cosyvoice_ws", 500, ts)
        val ttsFailed: VoiceEvent = VoiceEvent.TtsFailed("cosyvoice_ws", "403", ts)
        val ttsFallback: VoiceEvent =
            VoiceEvent.TtsFallback("cosyvoice_ws", "sherpa_onnx_kokoro", "403", ts)
        val asrStarted: VoiceEvent = VoiceEvent.AsrStarted("sherpa_sensevoice", ts)
        val asrResult: VoiceEvent = VoiceEvent.AsrResult("sherpa_sensevoice", "你好", 300, ts)
        val asrResultNull: VoiceEvent = VoiceEvent.AsrResult("sherpa_sensevoice", null, 300, ts)
        val asrFailed: VoiceEvent = VoiceEvent.AsrFailed("sherpa_sensevoice", "timeout", ts)
        val dlStarted: VoiceEvent = VoiceEvent.ModelDownloadStarted("kokoro", ts)
        val dlCompleted: VoiceEvent =
            VoiceEvent.ModelDownloadCompleted("kokoro", 82L * 1024 * 1024, ts)

        assertEquals("cosyvoice_ws", (ttsStarted as VoiceEvent.TtsStarted).providerId)
        assertEquals(10, ttsStarted.textLength)
        assertEquals(ts, ttsStarted.timestamp)
        assertEquals(500, (ttsSucceeded as VoiceEvent.TtsSucceeded).durationMs)
        assertEquals("403", (ttsFailed as VoiceEvent.TtsFailed).reason)
        assertEquals("cosyvoice_ws", (ttsFallback as VoiceEvent.TtsFallback).fromProvider)
        assertEquals("sherpa_onnx_kokoro", ttsFallback.toProvider)
        assertEquals("sherpa_sensevoice", (asrStarted as VoiceEvent.AsrStarted).providerId)
        assertEquals("你好", (asrResult as VoiceEvent.AsrResult).text)
        assertNull("空结果事件 text 应为 null", (asrResultNull as VoiceEvent.AsrResult).text)
        assertEquals("timeout", (asrFailed as VoiceEvent.AsrFailed).reason)
        assertEquals("kokoro", (dlStarted as VoiceEvent.ModelDownloadStarted).modelId)
        assertEquals(82L * 1024 * 1024, (dlCompleted as VoiceEvent.ModelDownloadCompleted).bytes)
    }

    // ─────────────────────────────────────────────
    // log() → 环形缓冲区（最近 100 条可查询）
    // ─────────────────────────────────────────────

    @Test
    fun testLogAppendsEventToRingBuffer() {
        val event = VoiceEvent.TtsSucceeded("edge_tts", 42, System.currentTimeMillis())
        VoiceTelemetry.log(event)
        val recent = VoiceTelemetry.recentEvents()
        assertEquals(1, recent.size)
        assertEquals("环形缓冲区应包含刚记录的事件", event, recent.first())
    }

    @Test
    fun testRingBufferKeepsOnlyRecent100() {
        // 超出容量 → 丢弃最旧，只保留最近 100 条（plan §5.7.3：最近 100 条）
        for (i in 0 until 120) {
            VoiceTelemetry.log(VoiceEvent.TtsStarted("p$i", 1, i.toLong()))
        }
        val recent = VoiceTelemetry.recentEvents()
        assertEquals(100, recent.size)
        assertEquals("第 1 条应为第 20 个事件（旧事件被淘汰）", "p20", (recent.first() as VoiceEvent.TtsStarted).providerId)
        assertEquals("最后一条应为第 119 个事件", "p119", (recent.last() as VoiceEvent.TtsStarted).providerId)
    }

    @Test
    fun testClearResetsRingBuffer() {
        VoiceTelemetry.log(VoiceEvent.AsrStarted("sherpa_sensevoice", 1L))
        VoiceTelemetry.clear()
        assertTrue("clear() 后环形缓冲区应为空", VoiceTelemetry.recentEvents().isEmpty())
    }

    // ─────────────────────────────────────────────
    // VoiceEventBus（事件收集：实时订阅）
    // ─────────────────────────────────────────────

    @Test
    fun testVoiceEventBusDeliversLoggedEvents() = runBlocking {
        // 真实调度器验证 log() → 总线 → 订阅者的异步转发链路。
        // 不用 runTest 虚拟时间：backgroundScope + StandardTestDispatcher 下
        // SharedFlow 订阅时序不稳定（replay=0，订阅前发射的事件不补发）；
        // 这里用 Dispatchers.Default 订阅 + delay 让出 CPU 保证订阅先完成。
        val received = java.util.Collections.synchronizedList(mutableListOf<VoiceEvent>())
        val collector = launch(Dispatchers.Default) {
            VoiceEventBus.events.collect { received += it }
        }
        delay(50) // 等 collector 完成订阅（collect 订阅同步完成）

        val fallback = VoiceEvent.TtsFallback("cosyvoice_ws", "system_tts", "403", 99L)
        VoiceTelemetry.log(fallback)
        VoiceTelemetry.log(VoiceEvent.AsrFailed("sherpa_sensevoice", "timeout", 100L))
        delay(50) // 等转发（tryEmit 同步入队 + collector 协程被唤醒处理）
        collector.cancel()

        assertEquals("总线应收到 log() 转发的 2 个事件", 2, received.size)
        assertTrue("总线应包含 TtsFallback 事件", received.contains(fallback))
    }
}
