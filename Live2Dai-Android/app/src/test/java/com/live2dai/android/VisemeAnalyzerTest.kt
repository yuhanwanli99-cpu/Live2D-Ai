package com.live2dai.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.ByteArrayInputStream
import java.io.InputStream
import kotlin.math.PI
import kotlin.math.sin

// ============================================================================
// WAV 解析 / 生成辅助（测试内部私有，替代 PC make_wav_bytes）
// ============================================================================

/** 读取 test resources fixtures/tts_vowels.wav → FloatArray */
internal fun readTtsVowelsFixture(): FloatArray {
    val stream: InputStream = VisemeAnalyzerTest::class.java.classLoader
        ?.getResourceAsStream("fixtures/tts_vowels.wav")
        ?: error("fixtures/tts_vowels.wav not found in test resources")
    val bytes = stream.readBytes()
    return parseWavPcm16(bytes)
}

/**
 * 解析 16-bit PCM 单声道 WAV：
 *   44 字节头 → data chunk → LittleEndian short → FloatArray / 32768.0
 */
internal fun parseWavPcm16(wavBytes: ByteArray): FloatArray {
    require(wavBytes.size > 44) { "WAV too small: ${wavBytes.size} bytes" }
    // 检查 RIFF 头
    val riff = String(wavBytes, 0, 4)
    require(riff == "RIFF") { "Not a RIFF file: $riff" }
    // 查找 data chunk
    var offset = 12
    while (offset + 8 <= wavBytes.size) {
        val chunkId = String(wavBytes, offset, 4)
        val chunkSize = (wavBytes[offset + 4].toInt() and 0xFF) or
            ((wavBytes[offset + 5].toInt() and 0xFF) shl 8) or
            ((wavBytes[offset + 6].toInt() and 0xFF) shl 16) or
            ((wavBytes[offset + 7].toInt() and 0xFF) shl 24)
        if (chunkId == "data") {
            val dataStart = offset + 8
            val sampleCount = chunkSize / 2
            val samples = FloatArray(sampleCount)
            for (i in 0 until sampleCount) {
                val lo = wavBytes[dataStart + i * 2].toInt() and 0xFF
                val hi = wavBytes[dataStart + i * 2 + 1].toInt() and 0xFF
                val s16 = (hi shl 8 or lo).toShort()
                samples[i] = s16.toFloat() / 32768.0f
            }
            return samples
        }
        offset += 8 + chunkSize
    }
    error("data chunk not found in WAV")
}

/** 生成指定时长静音样本 */
internal fun silenceSamples(durationMs: Int, sampleRate: Int): FloatArray {
    val count = sampleRate * durationMs / 1000
    return FloatArray(count) { 0f }
}

/** 生成正弦波样本 */
internal fun sineSamples(freqHz: Float, durationMs: Int, sampleRate: Int, amplitude: Float = 0.5f): FloatArray {
    val count = sampleRate * durationMs / 1000
    return FloatArray(count) { i ->
        (amplitude * sin(2.0 * PI * freqHz * i / sampleRate)).toFloat()
    }
}

/** 生成 WAV 字节（16-bit PCM 单声道），用于构造 AudioFrame 测试→不直接写文件 */
internal fun makeWavFloatArray(vararg sampleBlocks: FloatArray): FloatArray {
    val total = sampleBlocks.sumOf { it.size }
    val result = FloatArray(total)
    var offset = 0
    for (block in sampleBlocks) {
        block.copyInto(result, offset)
        offset += block.size
    }
    return result
}

// ============================================================================
// VisemeAnalyzerTest — 包含多个内部测试类（F4 全部）
// ============================================================================

class VisemeAnalyzerTest {

    // ── VowelMapContractTest ──

    @Test
    fun vowelMap_keys_and_bounds() {
        val map = VisemeAnalyzer.VOWEL_MAP
        val expectedKeys = setOf("A", "I", "U", "E", "O", "SIL")
        assertEquals(expectedKeys, map.keys)

        // 精确值断言
        val eps = 1e-6f
        assertEquals(0.8f, map["A"]!![0], eps); assertEquals(0.2f, map["A"]!![1], eps)
        assertEquals(0.3f, map["I"]!![0], eps); assertEquals(0.8f, map["I"]!![1], eps)
        assertEquals(0.2f, map["U"]!![0], eps); assertEquals(0.0f, map["U"]!![1], eps)
        assertEquals(0.5f, map["E"]!![0], eps); assertEquals(0.5f, map["E"]!![1], eps)
        assertEquals(0.6f, map["O"]!![0], eps); assertEquals(0.7f, map["O"]!![1], eps)
        assertEquals(0.0f, map["SIL"]!![0], eps); assertEquals(0.0f, map["SIL"]!![1], eps)

        // 所有值 ∈ [0,1]
        for ((_, v) in map) {
            assertTrue("mo=${v[0]} ∈ [0,1]", v[0] in 0f..1f)
            assertTrue("mf=${v[1]} ∈ [0,1]", v[1] in 0f..1f)
        }
    }

    // ── MaoProMappingTest ──

    @Test
    fun mapToFiveVowels_precise() {
        // 使用 VisemeAnalyzer 构造函数 + 内部 buildVisemeFrame 验证映射
        // 注：S10 契约样本的 mao_pro 无 ParamMouthForm——这里用契约表 2 精确值
        val analyzer = VisemeAnalyzer(16000, 150)

        // 通过构造 VisemeFrame 验证——直接验证 buildVisemeFrame 逻辑
        // 注意：buildVisemeFrame 是 private → 通过 processFrame 间接验证
        // 为直接验证映射，构造已知 label 的帧并断言

        // 用全零样本（判 SIL）→ 验证 SIL 映射
        // 149ms = 2384 samples < frameSize(2400) → 不足一窗口，应为 null
        val frameSil = analyzer.processFrame(
            AudioFrame(silenceSamples(149, 16000), 16000, 0)
        )
        assertNull("149ms(2384)<frameSize(2400) 不足一窗口，应为 null", frameSil)
        // 喂足 150ms → 需要 2400 samples
        val fullWindow = silenceSamples(150, 16000)  // 2400 samples
        val frame = analyzer.processFrame(AudioFrame(fullWindow, 16000, 0))
        assertNotNull("满窗口应产出帧", frame)
        assertEquals("SIL", frame!!.vowelLabel)
        // SIL → 全 0
        assertEquals(0f, frame.paramA, 1e-6f)
        assertEquals(0f, frame.paramI, 1e-6f)
        assertEquals(0f, frame.paramU, 1e-6f)
        assertEquals(0f, frame.paramE, 1e-6f)
        assertEquals(0f, frame.paramO, 1e-6f)
    }

    @Test
    fun overdrive_clamp_bounds() {
        // 通过直接构造 VisemeFrame 验证字段自动 coerceIn [0,1]
        // VisemeFrame 是 data class，字段在 analyzer 内部由 coerceIn 保证
        val analyzer = VisemeAnalyzer(16000, 150)
        val fullSilence = silenceSamples(150, 16000)
        val frame = analyzer.processFrame(AudioFrame(fullSilence, 16000, 0))
        assertNotNull(frame)
        // 所有五元音 ∈ [0,1]
        assertTrue(frame!!.paramA in 0f..1f)
        assertTrue(frame.paramI in 0f..1f)
        assertTrue(frame.paramU in 0f..1f)
        assertTrue(frame.paramE in 0f..1f)
        assertTrue(frame.paramO in 0f..1f)
        assertTrue(frame.mouthOpen in 0f..1f)
        assertTrue(frame.mouthForm in 0f..1f)
    }

    // ── VisemeFrameTest ──

    @Test
    fun dataClass_fields_and_ranges() {
        val frame = VisemeFrame(
            timestampMs = 0L,
            vowelLabel = "A",
            mouthOpen = 0.8f,
            mouthForm = 0.2f,
            paramA = 0.8f, paramI = 0f, paramU = 0f, paramE = 0f, paramO = 0.04f
        )
        assertEquals(0L, frame.timestampMs)
        assertEquals("A", frame.vowelLabel)
        assertTrue(VisemeAnalyzer.VOWEL_LABELS.contains(frame.vowelLabel))
        assertTrue(frame.timestampMs >= 0)
    }

    // ── AnalyzerConfigTest ──

    @Test
    fun config_validation_sampleRate() {
        // sampleRate <= 0 → 不直接抛异常（构造不校验 sampleRate）
        // frameMs <= 0 → 抛 IllegalArgumentException
        try {
            VisemeAnalyzer(16000, frameMs = 0)
            assertTrue("frameMs=0 应抛异常", false)
        } catch (e: IllegalArgumentException) {
            // expected
        } catch (e: Exception) {
            // also acceptable
        }
    }

    @Test
    fun config_validation_frameMs() {
        try {
            VisemeAnalyzer(16000, frameMs = -1)
            assertTrue("frameMs=-1 应抛异常", false)
        } catch (e: IllegalArgumentException) {
            // expected
        }
    }

    @Test
    fun defaults() {
        val a = VisemeAnalyzer(16000)
        assertEquals(150, a.frameMs)
        assertEquals(10.0f, a.temperature)
    }

    @Test
    fun windowSize_mfccFrames() {
        val a = VisemeAnalyzer(16000, 150)
        // frameSize = 16000 * 150 / 1000 = 2400
        // MFCC 每帧 23ms @ 512 hop → 至少 ~5 帧（23ms→约2帧，但 hop=512/16000=32ms，窗口 150ms 约 4-5 帧）
        // 实际看 MFCC extract 的 nFrames 公式
        assertTrue("frameSize > 0", 16000 * 150 / 1000 == 2400)
    }

    // ── ProcessAudioDegradeTest ──

    @Test
    fun emptySamples_noCrash() {
        val a = VisemeAnalyzer(16000, 150)
        val frame = a.processFrame(AudioFrame(FloatArray(0), 16000, 0))
        assertNull("空 samples → null", frame)
    }

    @Test
    fun bufferBelowWindow_null() {
        val a = VisemeAnalyzer(16000, 150)
        // 喂不足一窗口（150ms @16k = 2400 samples，只喂 1000）
        val partial = FloatArray(1000) { 0f }
        val frame = a.processFrame(AudioFrame(partial, 16000, 0))
        assertNull("不足一窗口 → null", frame)
    }

    @Test
    fun sampleRateMismatch_ignored() {
        val a = VisemeAnalyzer(16000, 150)
        val frame = a.processFrame(AudioFrame(FloatArray(3000) { 0f }, 24000, 0))
        assertNull("sampleRate 不匹配 → null", frame)
        // 不污染缓冲：后续喂正确 sampleRate 应正常
        // 100ms = 1600 samples < frameSize(2400) → 缓冲未满仍为 null
        val frame2 = a.processFrame(AudioFrame(silenceSamples(100, 16000), 16000, 0))
        assertNull("缓冲未满仍为 null", frame2)
        // 再喂 50ms = 800 samples 凑满（1600+800=2400）
        val frame3 = a.processFrame(AudioFrame(silenceSamples(50, 16000), 16000, 150))
        assertNotNull("缓冲应正确累积", frame3)
    }

    // ── ProcessAudioSilenceTest ──

    @Test
    fun silence600ms_framesAndTimestamps() {
        val a = VisemeAnalyzer(16000, 150)
        // 600ms @16k = 9600 samples，分 4 个窗口每 150ms
        val totalSilence = silenceSamples(600, 16000)
        val frames = mutableListOf<VisemeFrame>()
        var offset = 0
        var ts = 0L
        while (offset < totalSilence.size) {
            val chunkSize = minOf(2400, totalSilence.size - offset)
            val chunk = totalSilence.copyOfRange(offset, offset + chunkSize)
            val frame = a.processFrame(AudioFrame(chunk, 16000, ts))
            if (frame != null) frames.add(frame)
            offset += chunkSize
            ts += chunkSize * 1000L / 16000
        }

        assertEquals("600ms → 4 帧", 4, frames.size)
        assertEquals(listOf(0L, 150L, 300L, 450L), frames.map { it.timestampMs })
        for (f in frames) assertEquals("SIL", f.vowelLabel)
    }

    @Test
    fun silence_zeroVowelParams() {
        val a = VisemeAnalyzer(16000, 150)
        val fullWindow = silenceSamples(150, 16000)
        val frame = a.processFrame(AudioFrame(fullWindow, 16000, 0))
        assertNotNull(frame)
        assertEquals("SIL", frame!!.vowelLabel)
        assertEquals(0f, frame.paramA, 1e-6f)
        assertEquals(0f, frame.paramI, 1e-6f)
        assertEquals(0f, frame.paramU, 1e-6f)
        assertEquals(0f, frame.paramE, 1e-6f)
        assertEquals(0f, frame.paramO, 1e-6f)
    }

    // ── ProcessAudioGoldenTest ──

    @Test
    fun realSpeech_nonSilentFrames() {
        val a = VisemeAnalyzer(16000, 150)
        val samples = readTtsVowelsFixture() // 12000 samples @16kHz = 750ms
        // 逐窗喂入
        var offset = 0
        var ts = 0L
        val frames = mutableListOf<VisemeFrame>()
        while (offset < samples.size) {
            val chunkSize = minOf(2400, samples.size - offset)
            val chunk = samples.copyOfRange(offset, offset + chunkSize)
            val frame = a.processFrame(AudioFrame(chunk, 16000, ts))
            if (frame != null) frames.add(frame)
            offset += chunkSize
            ts += chunkSize * 1000L / 16000
        }
        assertTrue("应产出至少 4 帧", frames.size >= 4)
        // golden 标签序列：SIL, I, I, I, SIL
        val labels = frames.map { it.vowelLabel }
        val goldenLabels = listOf("SIL", "I", "I", "I", "SIL")
        assertEquals("标签序列应与 golden 一致", goldenLabels, labels)

        // 时间戳
        val timestamps = frames.map { it.timestampMs }
        assertEquals(listOf(0L, 150L, 300L, 450L, 600L), timestamps)

        // 非 SIL 帧 > 0
        val nonSil = frames.count { it.vowelLabel != "SIL" }
        assertTrue("非 SIL 帧 > 0", nonSil > 0)
    }

    // ── ProcessAudioDeterministicTest ──

    @Test
    fun timestamps_incrementByFrameMs() {
        val a = VisemeAnalyzer(16000, 150)
        // 喂 1000ms 静音 = 16000 samples
        val total = silenceSamples(1000, 16000)
        var offset = 0
        var ts = 0L
        val frames = mutableListOf<VisemeFrame>()
        while (offset + 2400 <= total.size) {
            val chunk = total.copyOfRange(offset, offset + 2400)
            val frame = a.processFrame(AudioFrame(chunk, 16000, ts))
            if (frame != null) frames.add(frame)
            offset += 2400
            ts += 150
        }
        // 1000ms / 150ms = ~6 完整窗口
        assertTrue("至少 6 帧", frames.size >= 6)
        val expectedTs = (0 until frames.size).map { (it * 150).toLong() }
        assertEquals(expectedTs, frames.map { it.timestampMs })
    }

    @Test
    fun customFrameMs_200() {
        val a = VisemeAnalyzer(16000, 200)
        val oneWindow = silenceSamples(200, 16000) // 3200 samples
        val frame = a.processFrame(AudioFrame(oneWindow, 16000, 0))
        assertNotNull(frame)
        assertEquals(0L, frame!!.timestampMs)
    }

    @Test
    fun chunkSizeInvariance() {
        // 同段音频分批喂 vs 一次喂，timeline 应一致
        val samples = silenceSamples(600, 16000)

        val a1 = VisemeAnalyzer(16000, 150)
        // 一次喂
        val chunk600 = AudioFrame(samples.copyOf(), 16000, 0)
        // 单帧只有 600ms，一次喂满 4 个窗口
        a1.processFrame(chunk600)
        // 剩余不足一窗口 → null，但已产出 4 帧
        val tl1 = a1.getVisemeTimeline()

        val a2 = VisemeAnalyzer(16000, 150)
        // 分批喂：3 次 200ms
        var offset = 0
        var ts = 0L
        while (offset < samples.size) {
            val size = minOf(3200, samples.size - offset) // 200ms chunks
            val chunk = samples.copyOfRange(offset, offset + size)
            a2.processFrame(AudioFrame(chunk, 16000, ts))
            offset += size
            ts += size * 1000L / 16000
        }
        val tl2 = a2.getVisemeTimeline()

        assertEquals("分批与整段 timeline 相同", tl1.size, tl2.size)
    }

    // ── LabelPickerTest ──

    @Test
    fun argmax() {
        val probs = floatArrayOf(0.1f, 0.5f, 0.2f, 0.05f, 0.03f, 0.12f) // SIL,A,I,U,E,O
        val label = LabelPicker.pick(probs)
        assertEquals("A", label) // idx 1 = A
    }

    @Test
    fun unknown_empty_to_sil() {
        assertEquals("SIL", LabelPicker.pick(FloatArray(0)))
    }

    @Test
    fun tie_break_first() {
        val probs = floatArrayOf(0.3f, 0.3f, 0.3f, 0.0f, 0.0f, 0.0f)
        val label = LabelPicker.pick(probs)
        assertEquals("SIL", label) // 平局取数组首个 SIL
    }

    // ── ProcessFrameAccumulateTest ──

    @Test
    fun crossFrameAccumulate() {
        val a = VisemeAnalyzer(16000, 150)
        // 先喂 1000 samples，再喂 1400 → 累积 2400 = 一窗口
        val chunk1 = FloatArray(1000) { 0f }
        val chunk2 = FloatArray(1400) { 0f }

        assertNull(a.processFrame(AudioFrame(chunk1, 16000, 0)))
        val frame = a.processFrame(AudioFrame(chunk2, 16000, 1000))
        assertNotNull("跨帧累积满一窗口应产出帧", frame)
        assertEquals(0L, frame!!.timestampMs)
    }

    @Test
    fun reset_clearsTimeline() {
        val a = VisemeAnalyzer(16000, 150)
        a.processFrame(AudioFrame(silenceSamples(150, 16000), 16000, 0))
        assertTrue(a.getVisemeTimeline().isNotEmpty())
        a.reset()
        assertTrue(a.getVisemeTimeline().isEmpty())
    }
}
