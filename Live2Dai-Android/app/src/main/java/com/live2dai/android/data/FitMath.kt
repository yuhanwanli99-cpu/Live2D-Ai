package com.live2dai.android.data

import kotlin.math.min

/**
 * 角色 bbox contain-fit 纯函数（contract-C-M1 §2.1，可 JVM 单测，无 native 依赖）。
 *
 * - [characterBBox]：对参与 drawable 的顶点数组（xy 交错）求 min/max 聚合。
 *   参与规则（isVisible && vertexCount>0 && textureIndex>=0）由调用方过滤后传入即可，
 *   函数只做 min/max 聚合；退化（空列表 / 任一范围非有限 / 宽或高 ≤ 0）→ null。
 *   Y-flip 对 min/max 不变。
 * - [computeCharacterFit]：contain-fit 纯函数（架构 §3.2/§3.3，全部量同空间 = 模型单位）；
 *   退化（bbox 非法 / view ≤ 0 / canvas ≤ 0 / padding < 0）→ CharacterFit(1f, 0f, 0f)（= 现状，零行为变化）。
 *
 * 注：bbox 与 view 必须同空间；renderer 负责按运行时 k=canvasWidth/canvasVertexW（=1/ppu）
 * 把顶点空间换算到模型单位（ppu 不变式，架构 §3.2）。
 */

/** 角色包围盒（模型单位，与 computeProjection 输入同空间） */
data class BBox(val minX: Float, val maxX: Float, val minY: Float, val maxY: Float)

/**
 * 角色 bbox 聚合：对参与 drawable 的顶点数组（xy 交错）求 min/max。
 * 退化（空列表 / 任一范围非有限 / 宽或高 ≤ 0）→ null。Y-flip 对 min/max 不变。
 */
fun characterBBox(vertexArrays: List<FloatArray>): BBox? {
    if (vertexArrays.isEmpty()) return null
    var minX = Float.POSITIVE_INFINITY
    var maxX = Float.NEGATIVE_INFINITY
    var minY = Float.POSITIVE_INFINITY
    var maxY = Float.NEGATIVE_INFINITY
    for (vertices in vertexArrays) {
        var i = 0
        while (i + 1 < vertices.size) {
            val x = vertices[i]
            val y = vertices[i + 1]
            if (x < minX) minX = x
            if (x > maxX) maxX = x
            if (y < minY) minY = y
            if (y > maxY) maxY = y
            i += 2
        }
    }
    if (!minX.isFinite() || !maxX.isFinite() || !minY.isFinite() || !maxY.isFinite()) return null
    if (maxX - minX <= 0f || maxY - minY <= 0f) return null
    return BBox(minX, maxX, minY, maxY)
}

/** contain-fit 结果（模型单位）：scale + 水平/垂直偏移（= bbox 中心）。 */
data class CharacterFit(val scale: Float, val xOffset: Float, val yOffset: Float)

/**
 * contain-fit 纯函数（架构 §3.2/§3.3，全部量同空间 = 模型单位）：
 *   bboxW = maxX - minX; bboxH = maxY - minY
 *   cx = (minX + maxX) / 2; cy = (minY + maxY) / 2
 *   fitScale = min(viewWidth / bboxW, viewHeight / bboxH) * (1 - padding)   // padding ∈ [0,1)，默认 0.05f
 *   fitXOffset = cx; fitYOffset = cy
 * 退化（bbox 宽或高 ≤ 0 / 任一非有限 / viewWidth≤0 / viewHeight≤0 / canvasWidth≤0 /
 * canvasHeight≤0 / padding<0）→ CharacterFit(1f, 0f, 0f)（= 现状，零行为变化）。
 */
fun computeCharacterFit(
    bbox: BBox,
    viewWidth: Float,
    viewHeight: Float,
    canvasWidth: Float,
    canvasHeight: Float,
    padding: Float = 0.05f
): CharacterFit {
    val bboxW = bbox.maxX - bbox.minX
    val bboxH = bbox.maxY - bbox.minY
    if (bboxW <= 0f || bboxH <= 0f ||
        !bboxW.isFinite() || !bboxH.isFinite() ||
        viewWidth <= 0f || viewHeight <= 0f ||
        canvasWidth <= 0f || canvasHeight <= 0f ||
        padding < 0f
    ) {
        return CharacterFit(1f, 0f, 0f)
    }
    val cx = (bbox.minX + bbox.maxX) / 2f
    val cy = (bbox.minY + bbox.maxY) / 2f
    val fitScale = min(viewWidth / bboxW, viewHeight / bboxH) * (1f - padding)
    return CharacterFit(fitScale, cx, cy)
}
