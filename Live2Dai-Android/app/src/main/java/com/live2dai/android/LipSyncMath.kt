package com.live2dai.android

import kotlin.math.sin

/**
 * LipSync 嘴型值纯函数（V1.1 模块4 方案A：播放状态驱动，plan §4.1.4）。
 *
 * TTS 播放期间 mouth 参数按正弦波动（0.65 ± 0.25 → [0.4, 0.9]），
 * 播放结束由调用方（[VoiceIoController]）置 0.0（闭嘴）。
 * 抽离为纯函数（无 Android 依赖）便于单元测试。
 */
internal object LipSyncMath {

    /** 嘴型开合度理论下限（0.65 - 0.25） */
    const val MIN_MOUTH: Float = 0.4f

    /** 嘴型开合度理论上限（0.65 + 0.25） */
    const val MAX_MOUTH: Float = 0.9f

    /**
     * 按帧序号计算嘴型开合度：0.65 + 0.25 * sin(frame * 0.5)。
     *
     * @param frame 从 0 递增的动画帧号（每 [VoiceIoController.LIP_SYNC_INTERVAL_MS] 加 1，
     *              默认 100ms/帧 → 波动周期约 4π 帧 ≈ 1.26s）
     * @return 嘴型开合度 [0.4, 0.9]（0=闭嘴, 1=最大张口）
     */
    fun mouthValue(frame: Long): Float = 0.65f + 0.25f * sin(frame * 0.5f)
}
