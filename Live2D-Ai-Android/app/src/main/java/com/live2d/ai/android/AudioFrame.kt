package com.live2d.ai.android

import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow

/**
 * 一帧 PCM 音频数据（M4 音频事件流的最小单元）。
 *
 * @param samples FloatArray，-1.0..1.0，**单声道**（多声道必须由 Provider 侧降混为第一声道）
 * @param sampleRate 采样率（Hz），> 0；sherpa 通常 24000（UNC-3 实现时以 SherpaTtsAudio.sampleRate 实际值为准）
 * @param timestampMs 此帧在本次 speak 音频中的起始时间（ms），相对该次 speak 起点，从 0 递增（首帧 = 0）
 *
 * M-AND-1（contract-linkage-android.md §2）契约字段：`pcm` = PCM float 单声道采样（[samples] 别名）、
 * `timestampMs` 相对 `speak()` 起点、首帧 = 0。
 */
data class AudioFrame(
    val samples: FloatArray,
    val sampleRate: Int,
    val timestampMs: Long,
) {
    /** M-AND-1 契约字段别名：PCM float 单声道采样（与 [samples] 同一数组）。 */
    val pcm: FloatArray get() = samples

    /** M-AND-1 契约便捷构造：AudioFrame(pcm, timestampMs)。sampleRate 取 24000（sherpa/Edge/Cosy 标准采样率）。 */
    constructor(pcm: FloatArray, timestampMs: Long) : this(pcm, 24000, timestampMs)
}

/**
 * M-AND-1 seam：150ms 固定分块纯函数（选型 2a-① 确定性契约锚）。
 *
 * - 首帧 timestampMs = 0（相对 speak 起点）
 * - 相邻帧差 = [FRAME_MS]（150ms ± 1ms，固定分块确定性）
 * - 同输入双跑输出完全一致（纯函数：无随机项、不读系统时钟）
 *
 * Edge/CosyVoice（2a-①）与 sherpa（2c-① 现状 150ms 分块）共用同一分块语义。
 */
object LipSyncFrameChunker {

    /** 固定分块窗口（ms），与 VisemeAnalyzer 默认分析窗口对齐（contract-M4-M5 §2.3.1）。 */
    const val FRAME_MS: Long = 150L

    /**
     * 将 PCM 采样按 [FRAME_MS] 分块为 [AudioFrame] 序列。
     *
     * 块长 = sampleRate × FRAME_MS / 1000（末块截断）；timestampMs = 块索引 × FRAME_MS。
     * 空采样 / sampleRate ≤ 0 → 空列表（无 NaN/除零）。
     */
    fun chunk(samples: FloatArray, sampleRate: Int): List<AudioFrame> =
        chunk(samples, sampleRate, FRAME_MS)

    /** 按自定义窗口分块（契约断言固定 150ms，测试可注入其它窗口；Int 版便捷重载）。 */
    fun chunk(samples: FloatArray, sampleRate: Int, chunkMs: Long): List<AudioFrame> {
        if (samples.isEmpty() || sampleRate <= 0 || chunkMs <= 0) return emptyList()
        val chunkSize = (sampleRate.toLong() * chunkMs / 1000L).toInt().coerceAtLeast(1)
        if (chunkSize <= 0) return emptyList()
        val frames = ArrayList<AudioFrame>((samples.size + chunkSize - 1) / chunkSize)
        var offset = 0
        var index = 0L
        while (offset < samples.size) {
            val len = minOf(chunkSize, samples.size - offset)
            frames.add(AudioFrame(samples.copyOfRange(offset, offset + len), sampleRate, index * chunkMs))
            offset += len
            index++
        }
        return frames
    }

    /** 按自定义窗口分块（Int ms 便捷重载，委托 [chunk] Long 版）。 */
    fun chunk(samples: FloatArray, sampleRate: Int, chunkMs: Int): List<AudioFrame> =
        chunk(samples, sampleRate, chunkMs.toLong())

    /** 分块结果转为帧流（时序契约：消费者在 speak() 前启动 collect；speak 后 1s 内首帧）。 */
    fun toFlow(samples: FloatArray, sampleRate: Int): Flow<AudioFrame> = flow {
        chunk(samples, sampleRate).forEach { emit(it) }
    }
}
