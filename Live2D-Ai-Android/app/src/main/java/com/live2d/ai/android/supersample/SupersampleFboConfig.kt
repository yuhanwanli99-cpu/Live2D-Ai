package com.live2d.ai.android.supersample

import kotlin.math.roundToInt

/** 超采样 FBO 尺寸（宽×高）。 */
data class FboSize(val width: Int, val height: Int)

/**
 * 超采样 FBO 配置（seam，纯 JVM，无 GL）。
 *
 * 契约 REND2-A1/A2 + architecture-refactor-0814 F-4：1080×2400 表面 × SUPERSAMPLE_SCALE(4/3)
 * → FBO **1440×3200**（整数缩放，无取整误差）；FBO 分辨率 ≥1080p 下界；FBO 纵横比与表面一致
 * （同比例缩放，不拉伸）。
 */
object SupersampleFboConfig {

    /** 表面宽（iQOO 11 Pro 1080×2400@120Hz）。 */
    const val SURFACE_WIDTH = 1080

    /** 表面高。 */
    const val SURFACE_HEIGHT = 2400

    /** 超采样 FBO 宽（1080 × 4/3 = 1440，整数）。 */
    const val FBO_WIDTH = 1440

    /** 超采样 FBO 高（2400 × 4/3 = 3200，整数）。 */
    const val FBO_HEIGHT = 3200

    /** 超采样缩放系数（4/3）。 */
    const val SUPERSAMPLE_SCALE = 4.0 / 3.0

    /**
     * 计算超采样 FBO 尺寸：表面尺寸 × 缩放系数，四舍五入取整（宽高同系数 → 纵横比与表面一致）。
     *
     * 契约锚：`fboSize(1080, 2400)` = `(1440, 3200)`（整数缩放）。
     *
     * @throws IllegalArgumentException surfaceW/surfaceH 非正，或 scale 非正/非有限。
     */
    fun fboSize(surfaceW: Int, surfaceH: Int, scale: Float = 4f / 3f): Pair<Int, Int> {
        require(surfaceW > 0 && surfaceH > 0) { "surface 尺寸必须为正：${surfaceW}x${surfaceH}" }
        require(scale > 0f && scale.isFinite()) { "scale 必须为正有限值：$scale" }
        val width = (surfaceW * scale).roundToInt()
        val height = (surfaceH * scale).roundToInt()
        return width to height
    }

    /** [fboSize] 的 [FboSize] 数据类形态（宽高与 [fboSize] 完全一致）。 */
    fun fboSizeData(surfaceW: Int, surfaceH: Int, scale: Float = 4f / 3f): FboSize {
        val (w, h) = fboSize(surfaceW, surfaceH, scale)
        return FboSize(w, h)
    }

    /**
     * FBO ≥1080p 下界断言（谓词）：宽、高均 ≥ 1080。
     * 契约锚：FBO 1440×3200 通过；低于 1080p（如 720×1600）不通过。
     */
    fun isAtLeast1080p(width: Int, height: Int): Boolean = width >= 1080 && height >= 1080

    /**
     * FBO ≥1080p 下界断言（抛异常版）：低于 1080p 时抛 [IllegalArgumentException]。
     */
    fun requireAtLeast1080p(width: Int, height: Int) {
        require(isAtLeast1080p(width, height)) {
            "FBO ${width}x${height} 低于 1080p 下界（1080x1080），超采样路径不得使用"
        }
    }

    /**
     * FBO 不小于表面尺寸（逐维 ≥）：FBO 渲染目标不得小于输出表面（下采样语义下仍满足）。
     */
    fun isAtLeastSurface(width: Int, height: Int, surfaceW: Int = SURFACE_WIDTH, surfaceH: Int = SURFACE_HEIGHT): Boolean =
        width >= surfaceW && height >= surfaceH

    /**
     * 纵横比一致性：FBO 宽高比与表面宽高比相等（交叉相乘整数判定，无浮点误差）。
     * 契约锚：1440×3200 与 1080×2400 同比例（0.45），不拉伸。
     */
    fun preservesSurfaceAspect(
        width: Int,
        height: Int,
        surfaceW: Int = SURFACE_WIDTH,
        surfaceH: Int = SURFACE_HEIGHT
    ): Boolean = width.toLong() * surfaceH == height.toLong() * surfaceW
}
