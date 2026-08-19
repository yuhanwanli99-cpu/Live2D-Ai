package com.live2d.ai.android

import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Wave 2 · 2B-1（sherpa-onnx 本地 TTS 主路径 + Kokoro-zh 模型 spec）—— AudioFrame 帧契约穷举测试。
 *
 * 依据：contract-refactor-0814.md §2.3/§3.2（帧契约）+ §3.3 时序契约 + 架构 architecture-refactor-tts-0814.md §3.2。
 * 契约锚：AudioFrame(samples, sampleRate, timestampMs)：
 *   - samples: FloatArray，单声道 PCM float，样本 ∈ [-1,1]
 *   - sampleRate: Int > 0；sherpa/Kokoro 预期 24000（U3 待实测回填）
 *   - timestampMs: Long，相对本次 speak 起点，首帧=0、150ms 步进、严格非递减
 *   - 消费方须在 speak() 前启动 collect（replay=0）；speak 后 1s 内收首帧
 *
 * 本文件按 §2 帧契约穷举，覆盖 MAnd1 已有分块测试之外未穷举的面：
 * 帧内容/采样数与帧长一致、严格非递减全序列、残留尾帧、多采样率、全量重构。
 *
 * ── RED 状态如实标注 ──
 * LipSyncFrameChunker.chunk(samples, sampleRate) 已在 M-AND-1 波次落地（生产 seam 存在），
 * 故本文件的纯数据分块断言在现有实现上**大多应为 GREEN**（回归锁），非本次 RED 主面。
 * 本次 RED 主面 = Sherpa2b1SpeakStateTest（provider 引擎注入 seam 尚不存在），见 wave2-2b1-red.md。
 */
class Sherpa2b1AudioFrameContractTest {

    // ─────────────────────────────────────────────
    // §3.2-F1: 首帧 timestampMs == 0（相对 speak 起点）
    // ─────────────────────────────────────────────

    @Test
    fun first_frame_timestamp_zero_for_all_sample_rates() = runTest {
        for (sr in listOf(8000, 16000, 22050, 24000, 44100)) {
            val frames = LipSyncFrameChunker.chunk(allQuietSamples(sr, 300), sr)
            assertTrue("采样率 $sr 应至少产出 1 帧", frames.isNotEmpty())
            assertEquals("采样率 $sr 首帧 timestampMs 应 = 0", 0L, frames.first().timestampMs)
        }
    }

    // ─────────────────────────────────────────────
    // §3.2-F2: 150ms 步进 + 严格非递减（全序列，非仅相邻差）
    // ─────────────────────────────────────────────

    @Test
    fun timestamp_sequence_is_strictly_non_decreasing_150ms_step() = runTest {
        val sr = 24000
        val frames = LipSyncFrameChunker.chunk(allQuietSamples(sr, 1200), sr)
        assertTrue("1.2s 输入应产出多帧", frames.size >= 6)
        var prev = Long.MIN_VALUE
        for ((i, f) in frames.withIndex()) {
            assertTrue("帧[$i] timestampMs 应 ≥ 上一帧（严格非递减），实际 ${f.timestampMs} < $prev", f.timestampMs >= prev)
            prev = f.timestampMs
            if (i > 0) {
                val diff = f.timestampMs - frames[i - 1].timestampMs
                assertTrue(
                    "帧[$i] 与上一帧差应 = 150ms ±1（分块误差），实际 $diff",
                    diff in 149L..151L
                )
            }
        }
    }

    // ─────────────────────────────────────────────
    // §3.2-F3: 帧内容与采样数一致（全量重构 == 输入；帧采样数 = 帧长×采样率）
    // ─────────────────────────────────────────────

    @Test
    fun frame_content_reconstructs_input_exactly() = runTest {
        val sr = 24000
        val input = allQuietSamples(sr, 1000)
        val frames = LipSyncFrameChunker.chunk(input, sr)

        // 全量重构 == 输入（逐 sample 一致）
        val reconstructed = FloatArray(input.size)
        var off = 0
        for (f in frames) {
            f.samples.copyInto(reconstructed, off)
            off += f.samples.size
        }
        assertEquals("重构长度应等于输入样本数", input.size, off)
        // 全量逐样本一致
        for (i in input.indices) {
            assertEquals("sample[$i] 重构", input[i], reconstructed[i], 1e-6f)
        }
    }

    @Test
    fun frame_sample_count_matches_frame_duration() = runTest {
        val sr = 24000
        val frames = LipSyncFrameChunker.chunk(allQuietSamples(sr, 1000), sr)
        val fullFrameSamples = sr * 150 / 1000 // 3600
        for ((i, f) in frames.withIndex()) {
            if (i < frames.size - 1 || f.samples.size == fullFrameSamples) {
                assertEquals(
                    "帧[$i] 采样数应 = 150ms×采样率 ($fullFrameSamples)，实际 ${f.samples.size}",
                    fullFrameSamples, f.samples.size
                )
            }
        }
        // 采样数 × 时间戳不冲突：最后一帧不超帧长（残留尾帧 ≤ 150ms 采样数）
        assertTrue("任一帧采样数应 ≤ 150ms 采样数 $fullFrameSamples", frames.all { it.samples.size <= fullFrameSamples })
    }

    // ─────────────────────────────────────────────
    // §3.2-F4: 残留尾帧（非整 150ms 输入）不丢失、长度正确
    // ─────────────────────────────────────────────

    @Test
    fun residual_tail_frame_preserves_remainder() = runTest {
        val sr = 16000
        val fullFrame = sr * 150 / 1000 // 2400
        // 4200 样本 = 1 整帧(2400) + 残帧(1800)
        val input = FloatArray(4200) { 0.25f }
        val frames = LipSyncFrameChunker.chunk(input, sr)
        assertEquals("1 整帧+1 残帧应产出 2 帧", 2, frames.size)
        assertEquals("整帧长", fullFrame, frames[0].samples.size)
        assertEquals("残帧采样数 = 输入余量", 4200 - fullFrame, frames[1].samples.size)
        // 残帧时间戳 = 整帧数×150ms = 150
        assertEquals("残帧 timestampMs 应 = 150（int(整帧×150ms)）", 150L, frames[1].timestampMs)
    }

    // ─────────────────────────────────────────────
    // §3.2-F5: sampleRate 字段 > 0 且透传输入采样率
    // ─────────────────────────────────────────────

    @Test
    fun sample_rate_field_positive_and_propagated() = runTest {
        for (sr in listOf(8000, 24000)) {
            val f = LipSyncFrameChunker.chunk(allQuietSamples(sr, 150), sr).first()
            assertTrue("sampleRate > 0", f.sampleRate > 0)
            assertEquals("sampleRate 应透传输入采样率", sr, f.sampleRate)
        }
    }

    // ─────────────────────────────────────────────
    // §3.2-F6: frames 交换率半空输入（静音）保持合法帧
    // ─────────────────────────────────────────────

    @Test
    fun sample_values_in_legal_range_and_finite() = runTest {
        val frames = LipSyncFrameChunker.chunk(allQuietSamples(24000, 600), 24000)
        for (f in frames) {
            for (s in f.samples) {
                assertTrue("样本应 ∈ [-1,1]，实际 $s", s in -1.0f..1.0f)
                assertTrue("样本不得为 NaN/Infinity", !s.isNaN() && !s.isInfinite())
            }
        }
    }

    // ─────────────────────────────────────────────
    // §3.2-F7: 确定性（同输入同产出）
    // ─────────────────────────────────────────────

    @Test
    fun chunking_is_deterministic() = runTest {
        val input = allQuietSamples(24000, 700)
        val key = { f: List<AudioFrame> -> f.map { Triple(it.samples.asList(), it.sampleRate, it.timestampMs) } }
        assertEquals("同输入双跑帧序列应完全一致", key(LipSyncFrameChunker.chunk(input, 24000)), key(LipSyncFrameChunker.chunk(input, 24000)))
    }

    // ─────────────────────────────────────────────
    // fixture
    // ─────────────────────────────────────────────

    private fun allQuietSamples(sampleRate: Int, ms: Int): FloatArray =
        FloatArray(sampleRate * ms / 1000) { 0f }
}
