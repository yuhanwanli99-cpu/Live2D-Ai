package com.live2dai.android.data

import kotlin.math.min

// ============================================================
//  Wave 3 · 3-2 表情桥：合成一次性 motion3（F-b 裁定）
//
//  职责：把「参数帧」（Map<参数名, 目标值>）合成为一段可播放的一次性表情运动，
//  供下游渲染管线按采样索引取帧（A-5/F-b：曲线采样/时长/fade 语义）。
//
//  设计语义（KDoc 即契约）：
//  - 每条曲线 = 单段 `MotionCurveSegment(key, startValue, endValue)`，其中
//    startValue == endValue == paramFrame[key]（首键值=末键值=paramFrame 值；
//    fade 段首=末 —— 曲线本身是"平台段"，目标绝对值恒定为 paramFrame 值）。
//  - 播放包络（fade 语义）在 [segmentSnapshot] 采样时施加：
//      首 fadeMs 内 0 → paramFrame（线性淡入）；
//      中间保持 paramFrame（平台）；
//      末 fadeMs 内 paramFrame → 0（线性淡出）。
//    包络 = min(fadeIn, fadeOut, 1)，durationMs < 2*fadeMs 时自然退化为三角波。
//  - 采样率恒定：第 i 个采样点时刻 t_i = i * durationMs / sampleCount（ms）。
//  - 时长一致：durationMs / fadeMs 原样保留在 SynthesizedMotion3 元数据。
//
//  纯 JVM（无 android.* / native 依赖），可确定性单测。
// ============================================================

/** 合成运动配置（元数据载体；生成函数按逐参数签名调用，本类供调用方统一定义参数组）。 */
data class MotionSynthesisConfig(
    val durationMs: Long,
    val fadeMs: Long,
    val sampleRateFps: Int,
)

/**
 * 单参数运动曲线段。
 *
 * @param key Live2D 参数名（如 ParamMouthOpenY）。
 * @param startValue 曲线段起点值。
 * @param endValue 曲线段终点值。
 */
data class MotionCurveSegment(
    val key: String,
    val startValue: Float,
    val endValue: Float,
)

/**
 * 合成的一次性表情运动（motion3 语义的纯数据载体）。
 *
 * @param curves 曲线段列表（每个 paramFrame 键一条，顺序 = 输入帧迭代顺序）。
 * @param sampleCount 总采样点数 = durationMs * sampleRateFps / 1000。
 * @param durationMs 运动总时长（ms，原样保留输入）。
 * @param fadeMs 淡入/淡出时长（ms，原样保留输入；见 [MotionSynthesisGenerator.segmentSnapshot] 包络语义）。
 * @param loop 是否循环播放（本生成器恒为 false：一次性运动；供下游按需改写）。
 */
data class SynthesizedMotion3(
    val curves: List<MotionCurveSegment>,
    val sampleCount: Int,
    val durationMs: Long,
    val fadeMs: Long,
    val loop: Boolean = false,
)

/**
 * 表情参数帧 → 一次性运动合成器（纯函数，无状态，无 Android 依赖）。
 *
 * 典型用法：把 EmotionTagTable 给出的表情参数帧（或任意目标参数帧）合成为
 * 带淡入/淡出包络的运动，渲染侧每帧按 index 取采样点（[segmentSnapshot]）。
 */
object MotionSynthesisGenerator {

    /**
     * 合成表情运动。
     *
     * @param paramFrame 目标参数帧：key = Live2D 参数名，value = 目标值。
     *                   空串 key 视为非法键跳过；空帧 → 空曲线列表。
     * @param durationMs 运动总时长（ms）。
     * @param fadeMs 淡入/淡出时长（ms；≤0 表示无 fade，全程恒定目标值）。
     * @param sampleRateFps 采样率（帧/秒；≤0 时按 1 防御）。
     * @return 合成运动：曲线数 = 有效键数；每条曲线 startValue == endValue == paramFrame[key]；
     *         sampleCount = durationMs * sampleRateFps / 1000（整数除法）。
     */
    fun synthesizeEmotionMotion(
        paramFrame: Map<String, Float>,
        durationMs: Long,
        fadeMs: Long,
        sampleRateFps: Int,
    ): SynthesizedMotion3 {
        val duration = durationMs.coerceAtLeast(0L)
        val fade = fadeMs.coerceAtLeast(0L)
        val fps = sampleRateFps.coerceAtLeast(1)

        val curves = paramFrame.entries
            .filter { it.key.isNotEmpty() } // 非法键（空串）跳过
            .map { (key, value) -> MotionCurveSegment(key, value, value) }

        val sampleCount = (duration * fps / 1000L).toInt()
        return SynthesizedMotion3(
            curves = curves,
            sampleCount = sampleCount,
            durationMs = duration,
            fadeMs = fade,
            loop = false,
        )
    }

    /**
     * 按采样索引取运动快照（取采样点值）。
     *
     * 采样语义：
     *  - 第 i 个采样点时刻 t_i = i * durationMs / sampleCount（ms），采样率恒定；
     *  - 曲线值 = 段内线性插值（[MotionCurveSegment.startValue] → [endValue]）；
     *    合成曲线 start==end，故平台段恒为 paramFrame 值；
     *  - fade 包络（fadeMs > 0 时）：t < fadeMs → 线性淡入 0→base；
     *    t > durationMs - fadeMs → 线性淡出 base→0；中间保持 base。
     *    包络 = min(t/fadeMs, (durationMs - t)/fadeMs, 1)，重叠（durationMs < 2*fadeMs）时自然三角化。
     *
     * @param s 合成运动（或任意手搓 [SynthesizedMotion3]，start≠end 时按线性插值处理）。
     * @param atIndex 采样索引；越界自动钳制到 [0, sampleCount-1]。
     * @return key → 采样值；空运动/空曲线 → 空 Map。
     */
    fun segmentSnapshot(s: SynthesizedMotion3, atIndex: Int): Map<String, Float> {
        if (s.curves.isEmpty()) return emptyMap()
        val n = s.sampleCount
        val idx = if (n <= 0) 0 else atIndex.coerceIn(0, n - 1)
        val duration = s.durationMs.coerceAtLeast(0L)
        val fade = s.fadeMs.coerceAtLeast(0L)

        val tMs: Double = if (n <= 0 || duration <= 0L) 0.0 else idx.toDouble() * duration / n

        return s.curves.associate { curve ->
            val progress = if (duration <= 0L) 0.0 else (tMs / duration).coerceIn(0.0, 1.0)
            val base = curve.startValue + (curve.endValue - curve.startValue) * progress.toFloat()
            val value = if (fade <= 0L || duration <= 0L) {
                base
            } else {
                val fadeIn = tMs / fade
                val fadeOut = (duration - tMs) / fade
                base * min(fadeIn, min(fadeOut, 1.0)).toFloat()
            }
            curve.key to value
        }
    }
}
