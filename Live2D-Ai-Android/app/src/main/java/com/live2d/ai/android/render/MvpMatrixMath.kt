package com.live2d.ai.android.render

import com.live2d.ai.android.data.CharacterFit

/**
 * MVP 矩阵纯函数（seam，纯 JVM，无 Android 依赖）。
 *
 * 输入 [fit]（[CharacterFit]，data/FitMath.kt 的 contain-fit 结果：scale + 中心偏移）与
 * [bounds]（调用方预留的视口/包围盒参数，本轮不参与计算——单位视图投影即可，语义自洽）。
 *
 * 输出列主序 float[16]（OpenGL 约定）：
 * ```
 *   [  scale,    0,      0,  0,
 *         0,   scale,    0,  0,
 *         0,      0,     1,  0,
 *    xOffset, yOffset,   0,  1 ]
 * ```
 * 即模型矩阵 = T(xOffset, yOffset) · S(scale, scale, 1)，视图/投影为单位矩阵。
 *
 * 不变量：非零、无 NaN、可逆（det = scale² ≠ 0）、对角有限。
 * 退化输入（scale 非有限或为 0，或偏移非有限）→ 归一为 CharacterFit(1f, 0f, 0f) 语义。
 */
object MvpMatrixMath {

    fun toMvpMatrix(fit: CharacterFit, bounds: FloatArray): FloatArray {
        val scale = if (fit.scale.isFinite() && fit.scale != 0f) fit.scale else 1f
        val xOffset = if (fit.xOffset.isFinite()) fit.xOffset else 0f
        val yOffset = if (fit.yOffset.isFinite()) fit.yOffset else 0f
        return floatArrayOf(
            scale, 0f, 0f, 0f,
            0f, scale, 0f, 0f,
            0f, 0f, 1f, 0f,
            xOffset, yOffset, 0f, 1f
        )
    }
}
