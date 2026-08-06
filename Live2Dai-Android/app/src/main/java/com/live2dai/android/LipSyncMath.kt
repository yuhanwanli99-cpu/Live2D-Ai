package com.live2dai.android

import kotlin.random.Random

/**
 * LipSync 嘴型值纯函数（级2-B：音频响度驱动的随机波动 + 停顿模拟，替代 V1.1 正弦公式）。
 *
 * TTS 播放期间（isPlaying=true）嘴型按随机波动 [0.3, 0.7] 每帧微变，约 [PAUSE_PROBABILITY]
 * 概率插入 0.0（停顿闭口），更接近真实说话节奏；帧号作为随机种子 → 同帧同值
 * （确定性纯函数，可单测）。播放结束由调用方（[VoiceIoController]）置 0.0（闭嘴）。
 * 帧间突变由 [smoothMouth] 线性插值平滑，避免突兀跳动。
 * 抽离为纯函数（无 Android 依赖）便于单元测试。
 */
internal object LipSyncMath {

    /** 说话中随机波动嘴型下限（0.3 + 0 * 0.4） */
    const val MIN_MOUTH: Float = 0.3f

    /** 说话中随机波动嘴型上限（0.3 + 1 * 0.4） */
    const val MAX_MOUTH: Float = 0.7f

    /** 停顿闭口概率（任务要求 5%-10%，取中值 8% 模拟说话中的自然停顿） */
    const val PAUSE_PROBABILITY: Float = 0.08f

    /** 帧间平滑插值因子（每帧向目标靠近 30%，避免突兀跳动） */
    const val SMOOTH_FACTOR: Float = 0.3f

    /**
     * 按帧序号计算嘴型开合度：随机波动 + 停顿模拟。
     *
     * - 首帧（[isFirstFrame]=true）返回 0.0：嘴型从闭嘴起步，配合 [smoothMouth] 平滑张开；
     * - 其余帧：约 [PAUSE_PROBABILITY] 概率返回 0.0（停顿闭口），否则
     *   [MIN_MOUTH] + Random(frame).nextFloat() * ([MAX_MOUTH] - [MIN_MOUTH])
     *   ∈ [MIN_MOUTH, MAX_MOUTH)（说话中随机波动）；
     * - 帧号作随机种子：同帧同值（确定性），不同帧伪随机变化（每 100ms 一帧，视觉上自然波动）。
     *
     * @param frame 从 0 递增的动画帧号（每 [VoiceIoController.LIP_SYNC_INTERVAL_MS] 加 1，
     *              默认 100ms/帧）
     * @param isFirstFrame 是否本次播放的首帧（true → 返回 0.0，嘴型从闭嘴起步）
     * @return 嘴型开合度：0.0=闭嘴/停顿，[MIN_MOUTH, MAX_MOUTH]=说话中波动（0=闭嘴, 1=最大张口）
     */
    fun randomizedMouth(frame: Long, isFirstFrame: Boolean): Float {
        if (isFirstFrame) return 0.0f
        val rng = Random(frame)
        return if (rng.nextFloat() < PAUSE_PROBABILITY) {
            // 停顿闭口：偶尔归零，模拟说话中的自然停顿
            0.0f
        } else {
            MIN_MOUTH + rng.nextFloat() * (MAX_MOUTH - MIN_MOUTH)
        }
    }

    /**
     * 帧间平滑：current 向 target 线性插值（factor=0.3 默认），避免随机目标造成的突兀跳动。
     *
     * 结果恒在 [current, target] 之间（factor ∈ [0,1]），多次插值单调收敛到 target，不振荡。
     */
    fun smoothMouth(current: Float, target: Float, factor: Float = SMOOTH_FACTOR): Float =
        current + (target - current) * factor
}
