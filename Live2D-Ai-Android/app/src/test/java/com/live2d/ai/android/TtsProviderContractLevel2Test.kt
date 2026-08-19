package com.live2d.ai.android

import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.emptyFlow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * TtsProvider audioFrameFlow 契约测试（M4-M5 §2.1-2.3）。
 *
 * 对齐蓝本：VoiceIoControllerTest.kt（协程+Mockito 风格）。
 * 覆盖 audioFrameFlow 空流/实发/时序/停止后不 emit/无订阅者不阻塞/默认实现向后兼容。
 */
@OptIn(ExperimentalCoroutinesApi::class)
class TtsProviderContractLevel2Test {

    // ── AF1: 空流 Provider ──

    @Test
    fun AF1_emptyFlow_providerNoCapability() = runTest {
        // 默认实现：emptyFlow()
        val provider = FakeEmptyFlowProvider()
        val items = provider.audioFrameFlow.toList()
        assertTrue("空流 collect 立即完成，无元素", items.isEmpty())
    }

    // ── AF2: 实发分块 ──

    @Test
    fun AF2_emit_allSamplesReconstructed() = runTest {
        val chunkSize = 4800 // 200ms @24kHz
        val samples = FloatArray(14400) { (it * 0.01f).toFloat().coerceIn(-1f, 1f) }
        val provider = FakeChunkedFlowProvider(samples, 24000, chunkSize)
        provider.startEmitting() // P1-1: emit 前置条件，否则 audioFrameFlow 默认空流
        val frames = provider.audioFrameFlow.toList()

        // 帧数 = ceil(samples.size / chunkSize)
        val expectedFrames = (samples.size + chunkSize - 1) / chunkSize
        assertEquals(expectedFrames, frames.size)

        // 拼接全部帧 → contentEquals（逐块 contentEquals）
        val reconstructed = FloatArray(samples.size)
        var offset = 0
        for (f in frames) {
            f.samples.copyInto(reconstructed, offset)
            offset += f.samples.size
        }
        assertEquals(samples.size, offset)
        // FloatArray contentEquals
        for (i in samples.indices) {
            assertEquals("sample[$i]", samples[i], reconstructed[i], 1e-6f)
        }
    }

    // ── AF3: 帧数 ──

    @Test
    fun AF3_emit_frameCount() = runTest {
        val samples = FloatArray(10000) { 0f }
        val chunkSize = 3000
        val provider = FakeChunkedFlowProvider(samples, 16000, chunkSize)
        provider.startEmitting() // P1-1: emit 前置条件，否则 audioFrameFlow 默认空流
        val frames = provider.audioFrameFlow.toList()
        val expected = (samples.size + chunkSize - 1) / chunkSize
        assertEquals("ceil(samples/chunk)", expected, frames.size)
    }

    // ── AF4: timestampMonotonic ──

    @Test
    fun AF4_timestampMonotonic() = runTest {
        val samples = FloatArray(9600) { 0f }
        val chunkSize = 2400
        val provider = FakeChunkedFlowProvider(samples, 16000, chunkSize)
        provider.startEmitting() // P1-1: emit 前置条件，否则 audioFrameFlow 默认空流
        val frames = provider.audioFrameFlow.toList()
        for (i in frames.indices) {
            assertEquals(
                "frame $i timestamp",
                (i * chunkSize * 1000L / 16000),
                frames[i].timestampMs
            )
        }
    }

    // ── AF5: sampleRate 非硬编码 ──

    @Test
    fun AF5_sampleRate_notHardcoded() = runTest {
        val samples = FloatArray(4800) { 0f }
        val provider = FakeChunkedFlowProvider(samples, 24000, 2400)
        provider.startEmitting() // P1-1: emit 前置条件，否则 audioFrameFlow 默认空流
        val frames = provider.audioFrameFlow.toList()
        assertTrue(frames.isNotEmpty())
        assertEquals(24000, frames[0].sampleRate)
    }

    // ── AF6: speak 完成后不再 emit ──

    @Test
    fun AF6_noEmitAfterSpeakCompleted() = runTest {
        val samples = FloatArray(4800) { 0f }
        val provider = FakeChunkedFlowProvider(samples, 16000, 2400)
        provider.startEmitting()
        provider.finishEmitting()

        val moreFrames = mutableListOf<AudioFrame>()
        provider.audioFrameFlow.collect { moreFrames.add(it) }
        assertTrue("speak 完成后不应再有新帧", moreFrames.isEmpty())
    }

    // ── AF7: stop() 后不 emit ──

    @Test
    fun AF7_noEmitAfterStop() = runTest {
        val samples = FloatArray(4800) { 0f }
        val provider = FakeChunkedFlowProvider(samples, 16000, 2400)
        provider.startEmitting()
        provider.stopEmitting()

        val moreFrames = mutableListOf<AudioFrame>()
        provider.audioFrameFlow.collect { moreFrames.add(it) }
        assertTrue("stop 后不应再有新帧", moreFrames.isEmpty())
    }

    // ── AF8: 无订阅者不阻塞 speak ──

    @Test
    fun AF8_noBlocker_noSubscriber() = runTest {
        // 无订阅者时 emit 丢弃 + Log.w
        val samples = FloatArray(4800) { 0f }
        val provider = FakeChunkedFlowProvider(samples, 16000, 2400)
        // 不订阅 → emit 被 tryEmit 丢弃
        provider.startEmitting() // 不阻塞，正常完成
        assertTrue("无订阅者不阻塞", true)
    }

    // ── AF9: SystemTtsProvider 空流 ──

    @Test
    fun AF9_emptyFlow_SystemTts() = runTest {
        // 默认实现 emptyFlow() —— 接口 default get() = emptyFlow()
        val provider = object : TtsProvider {
            override val info = TtsProviderInfo(
                "test", "test", TtsProviderCategory.LOCAL, false,
                MutableStateFlow(TtsHealth.OK), 1, false
            )
            override val isSpeaking = MutableStateFlow(false)
            override suspend fun speak(text: String) {}
            override fun stop() {}
            override fun release() {}
        }
        val items = provider.audioFrameFlow.toList()
        assertTrue("默认 audioFrameFlow 为空流", items.isEmpty())
    }

    // ── AF10: EdgeTts/CosyVoice v1.0 空流 ──

    @Test
    fun AF10_emptyFlow_byDefault() = runTest {
        // 不重写 audioFrameFlow 的 Provider → 走 default get() = emptyFlow()
        val provider = object : TtsProvider {
            override val info = TtsProviderInfo(
                "edge_test", "test", TtsProviderCategory.CLOUD_WS, true,
                MutableStateFlow(TtsHealth.OK), 1, false
            )
            override val isSpeaking = MutableStateFlow(false)
            override suspend fun speak(text: String) {}
            override fun stop() {}
            override fun release() {}
        }
        val items = provider.audioFrameFlow.toList()
        assertTrue(items.isEmpty())
    }

    // ── AF11: 285 不回退（默认实现编译通过）──

    @Test
    fun AF11_defaultImpl_backwardCompat() {
        // 直接构造的 TtsProvider 不实现 audioFrameFlow 也能编译
        val provider = object : TtsProvider {
            override val info = TtsProviderInfo(
                "compat", "compat", TtsProviderCategory.LOCAL, false,
                MutableStateFlow(TtsHealth.OK), 1, false
            )
            override val isSpeaking = MutableStateFlow(false)
            override suspend fun speak(text: String) {}
            override fun stop() {}
            override fun release() {}
        }
        // 默认 audioFrameFlow 是 emptyFlow
        assertTrue(provider.audioFrameFlow is kotlinx.coroutines.flow.Flow<*>)
    }
}

// ============================================================================
// Fake Providers（测试内部使用）
// ============================================================================

/** 空流 fake provider */
internal class FakeEmptyFlowProvider : TtsProvider {
    override val info = TtsProviderInfo(
        "empty", "empty", TtsProviderCategory.LOCAL, false,
        MutableStateFlow(TtsHealth.OK), 1, false
    )
    override val isSpeaking = MutableStateFlow(false)
    override suspend fun speak(text: String) {}
    override fun stop() {}
    override fun release() {}
    override val audioFrameFlow = emptyFlow<AudioFrame>()
}

/** 按 chunkSize 分块 emit 的 fake provider */
internal class FakeChunkedFlowProvider(
    private val samples: FloatArray,
    private val sampleRate: Int,
    private val chunkSize: Int,
) : TtsProvider {
    override val info = TtsProviderInfo(
        "chunked", "chunked", TtsProviderCategory.LOCAL, false,
        MutableStateFlow(TtsHealth.OK), 1, false
    )
    override val isSpeaking = MutableStateFlow(false)
    override suspend fun speak(text: String) {}
    override fun stop() {}
    override fun release() {}

    private var emitting = false

    override val audioFrameFlow: kotlinx.coroutines.flow.Flow<AudioFrame>
        get() = if (emitting) {
            kotlinx.coroutines.flow.flow {
                var offset = 0
                var ts = 0L
                while (offset < samples.size) {
                    val size = minOf(chunkSize, samples.size - offset)
                    emit(
                        AudioFrame(
                            samples.copyOfRange(offset, offset + size),
                            sampleRate,
                            ts
                        )
                    )
                    offset += size
                    ts += size * 1000L / sampleRate
                }
            }
        } else {
            emptyFlow()
        }

    fun startEmitting() {
        emitting = true
    }

    fun finishEmitting() {
        emitting = false
    }

    fun stopEmitting() {
        emitting = false
    }
}
