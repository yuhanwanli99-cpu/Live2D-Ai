package com.live2dai.android

import kotlin.math.exp
import kotlin.math.sqrt

/**
 * LipSync 嘴型值纯函数（M-AND-2 选型 1-B：RMS 幅度驱动，替换 V1.1 伪随机波动）。
 *
 * 音频帧 → [rmsNormalized]（RMS 归一化 ∈ [0,1]）→ [applyEnvelope]（attack/release 包络 +
 * 静音阈值门控）→ 口型值 ∈ [0,1]。全程纯函数（无 Random、无 Android 依赖、无状态），
 * 同一输入恒同一输出（AND2-A2 确定性），可单测。
 *
 * - [SILENCE_THRESHOLD]：RMS < 0.02 → 口型目标归 0（静音闭嘴，AITuberFlow threshold=0.02，researched）
 * - attack τ=50ms / release τ=130ms 指数包络（kugutu attackMs=50/releaseMs=130，researched）：
 *   阶跃 50ms 达 1−e⁻¹≈0.63×目标、归零 130ms 衰减至 e⁻¹≈0.37×峰值（AND2-A4）
 * - 输出恒 clamp [0,1]（AND2-A5；VTS MouthOpen 输入 [0,1]）
 *
 * [smoothMouth] 与随机嘴型常量保留（历史契约兼容；生产 lip 路径不再引用）。
 */
internal object LipSyncMath {

    /** 说话中随机波动嘴型下限（历史常量，兼容保留；M-AND-2 起生产不再使用） */
    const val MIN_MOUTH: Float = 0.3f

    /** 说话中随机波动嘴型上限（历史常量，兼容保留） */
    const val MAX_MOUTH: Float = 0.7f

    /** 停顿闭口概率（历史常量，兼容保留） */
    const val PAUSE_PROBABILITY: Float = 0.08f

    /** 帧间平滑插值因子（历史常量，兼容保留） */
    const val SMOOTH_FACTOR: Float = 0.3f

    /**
     * M-AND-2（AND2-A3，researched）：静音阈值 = 0.02。
     * RMS < 本值 → 口型目标归 0（静音不张嘴）；RMS ≥ 本值 → 输出 > 0。
     * 来源: AITuberFlow plugins/lip-sync/node.ts threshold=0.02（search-report F4）。
     */
    const val SILENCE_THRESHOLD: Float = 0.02f

    /** M-AND-2（AND2-A4，researched）：attack 包络时间常数 = 50ms（kugutu attackMs=50）。 */
    internal const val ATTACK_MS: Float = 50f

    /** M-AND-2（AND2-A4，researched）：release 包络时间常数 = 130ms（kugutu releaseMs=130）。 */
    internal const val RELEASE_MS: Float = 130f

    /** M-AND-2（AND2-A7）：生产口型循环相邻输出最大步进 = 0.2（防跳变；经 applyEnvelope 后仍钳制）。 */
    internal const val MAX_FRAME_DELTA: Float = 0.2f

    /**
     * RMS 归一化（AND2-A1）：sqrt(mean(x²))，输出 ∈ [0,1]。
     *
     * - 空数组 → 0.0（防除零）；
     * - 全零样本 → 0.0；满幅（±1.0）样本 → 1.0；
     * - NaN/Inf 输入防御性归 0 → 输出恒无 NaN/Inf。
     *
     * 纯函数、确定性（AND2-A2）：同一输入恒同一输出。
     */
    fun rmsNormalized(samples: FloatArray): Float {
        if (samples.isEmpty()) return 0.0f
        var sum = 0.0
        for (s in samples) {
            val v = s.toDouble()
            sum += v * v
        }
        val rms = sqrt(sum / samples.size).toFloat()
        if (!rms.isFinite()) return 0.0f
        return rms.coerceIn(0.0f, 1.0f)
    }

    /**
     * attack/release 指数包络 + 静音阈值门控（AND2-A3/A4/A5，researched）。
     *
     * 指数趋近：out = prev + (target − prev) × (1 − e^(−dtMs/τ))，τ 取 attack=50ms（target ≥ prev）
     * 或 release=130ms（target < prev）：
     * - 阶跃输入（RMS 0→1.0）后 50ms 输出达 1−e⁻¹ ≈ 0.63×目标（AND2-A4 attack 窗 [40,60]ms）；
     * - 输入归 0 后 130ms 输出衰减至 e⁻¹ ≈ 0.37×峰值（AND2-A4 release 窗 [104,156]ms）。
     *
     * 门控（AND2-A3）：rms < [SILENCE_THRESHOLD] → 目标归 0（静音收敛闭嘴，非硬清零——
     * release 包络自然衰减，兼容 AND2-A4 释放曲线）。输出恒 clamp [0,1]（AND2-A5）。
     *
     * 纯函数、确定性（AND2-A2）：无状态、无随机，同一 (rms, prev, dtMs) 恒同一输出。
     */
    fun applyEnvelope(rms: Float, prev: Float, dtMs: Long): Float {
        val dt = dtMs.coerceAtLeast(0L).toFloat()
        // AND2-A3 门控：RMS 低于静音阈值 → 目标归 0（静音不张嘴）
        val target = if (rms < SILENCE_THRESHOLD) 0.0f else rms.coerceIn(0.0f, 1.0f)
        val tau = if (target >= prev) ATTACK_MS else RELEASE_MS
        val factor = 1.0f - exp(-dt / tau)
        val out = prev + (target - prev) * factor
        if (!out.isFinite()) return 0.0f
        return out.coerceIn(0.0f, 1.0f)
    }

    /**
     * 帧间平滑：current 向 target 线性插值（factor=0.3 默认），避免随机目标造成的突兀跳动。
     *
     * 历史契约兼容保留（M-AND-2 生产路径由 [applyEnvelope] + [MAX_FRAME_DELTA] 步进钳制替代）。
     */
    fun smoothMouth(current: Float, target: Float, factor: Float = SMOOTH_FACTOR): Float =
        current + (target - current) * factor
}
