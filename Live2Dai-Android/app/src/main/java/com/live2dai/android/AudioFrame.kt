package com.live2dai.android

/**
 * 一帧 PCM 音频数据（M4 音频事件流的最小单元）。
 *
 * @param samples FloatArray，-1.0..1.0，**单声道**（多声道必须由 Provider 侧降混为第一声道）
 * @param sampleRate 采样率（Hz），> 0；sherpa 通常 24000（UNC-3 实现时以 SherpaTtsAudio.sampleRate 实际值为准）
 * @param timestampMs 此帧在本次 speak 音频中的起始时间（ms），相对该次 speak 起点，从 0 递增
 */
data class AudioFrame(
    val samples: FloatArray,
    val sampleRate: Int,
    val timestampMs: Long,
)
