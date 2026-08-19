package com.live2d.ai.android.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 角色 bbox contain-fit 纯函数断言（契约 C-M1 §2.1/§3，M1-A1~A7）。
 *
 * 契约锁定的 contain-fit 公式（同空间=模型单位）：
 *     bboxW = maxX-minX；bboxH = maxY-minY
 *     cx = (minX+maxX)/2；cy = (minY+maxY)/2
 *     fitScale = min(viewWidth/bboxW, viewHeight/bboxH) * (1 - padding)   // padding 默认 0.05f
 *     fitXOffset = cx；fitYOffset = cy
 *     退化（bbox==null / viewWidth≤0 / viewHeight≤0 / canvasWidth≤0 /
 *           canvasHeight≤0 / padding<0）→ CharacterFit(1f, 0f, 0f)（= 现状，零行为变化）
 *
 * 生产符号（RED 待实现）：`data/FitMath.characterBBox(vertexArrays)` 与
 * `data/FitMath.computeCharacterFit(bbox, viewWidth, viewHeight, canvasWidth, canvasHeight, padding)`。
 * 所有数值断言以契约 §3 锁定为准（bai：0.0830×0.2525 / 1.0×1.5 → 5.643±0.01；
 * 竖屏 1.0×2.1667 → 8.15±0.01；R1-model D2 后唯一模型为 bai，不再含 niziiro）。
 */
class FitMathTest {

    // ============================================================
    // A1：contain-fit scale = 受限维占比 × (1-padding)，bai 高受限
    // ============================================================
    @Test
    fun fitScale_containFit_minRatio() {
        // bai：bbox 0.0830(w) × 0.2525(h)，中心在原点的模型单位；view 1.0 × 1.5；padding 0.05
        // min(1.0/0.0830, 1.5/0.2525) = min(12.048, 5.941) = 5.941 → ×0.95 = 5.644 ≈ 5.643
        val bbox = BBox(-0.0415f, 0.0415f, -0.12625f, 0.12625f) // 0.0830 × 0.2525，居中
        val fit = computeCharacterFit(bbox, 1.0f, 1.5f, 1.0f, 1.5f)

        assertEquals("bai contain-fit scale 应为 5.643 ± 0.01", 5.643f, fit.scale, 0.01f)

        // 高度受限分支命中：height 占比 < width 占比
        val heightRatio = minRatioOf(bbox, 1.0f, 1.5f).heightRatio
        val widthRatio = minRatioOf(bbox, 1.0f, 1.5f).widthRatio
        assertTrue(
            "高度受限分支应命中（height 占比 ${heightRatio} > width 占比 $widthRatio）",
            heightRatio > widthRatio
        )
    }

    // ============================================================
    // A2：contain-fit 恒等式 —— 受限维占比 == (1-padding) == 0.95
    // ============================================================
    @Test
    fun fitScale_widthLimited_vs_heightLimited() {
        // 例1：宽度受限。bbox 2×1，view 1×1 → min(1/2, 1/1)=0.5 → scale=0.5×0.95=0.475
        val widthLimited = computeCharacterFit(BBox(0f, 2f, 0f, 1f), 1f, 1f, 1f, 1f)
        // (bboxW × scale) / viewWidth == 0.95 ± 0.01
        val widthLimitedDimRatio = (2f * widthLimited.scale) / 1f
        assertEquals("宽限例受限维(宽)占比应为 0.95 ± 0.01", 0.95f, widthLimitedDimRatio, 0.01f)

        // 例2：高度受限。bbox 1×2，view 1×1 → min(1, 0.5)=0.5 → scale=0.5×0.95=0.475
        val heightLimited = computeCharacterFit(BBox(0f, 1f, 0f, 2f), 1f, 1f, 1f, 1f)
        val heightLimitedDimRatio = (2f * heightLimited.scale) / 1f
        assertEquals("高限例受限维(高)占比应为 0.95 ± 0.01", 0.95f, heightLimitedDimRatio, 0.01f)

        // 受限维判定：宽限例宽主导、高限例高主导（互不误判）
        assertTrue("宽限例中，宽占比应 > 高占比（宽受限）", widthTrimRatio(2f, 1f) > heightTrimRatio(2f, 1f))
        assertTrue("高限例中，高占比应 > 宽占比（高受限）", heightTrimRatio(1f, 2f) > widthTrimRatio(1f, 2f))
    }

    // ============================================================
    // A3：offset = bbox 中心（cx/cy），契约锁定值
    // ============================================================
    @Test
    fun offsets_bboxCenter() {
        // ---- bai（唯一模型，R1-model D2）----
        // px bbox x∈[-164.8,167.3] y∈[-994.3,16.0]，ppu=4000 → 模型单位 min/max：
        // cx = (-164.8+167.3)/2/4000 = 0.0003125 ≈ 0.0003
        // cy = (-994.3+16.0)/2/4000 = -0.1222875 ≈ -0.122
        val baiBbox = BBox(-164.8f / 4000f, 167.3f / 4000f, -994.3f / 4000f, 16.0f / 4000f)
        val baiFit = computeCharacterFit(baiBbox, 1.0f, 1.5f, 1.0f, 1.5f)
        assertEquals("bai xOffset(=cx) 应为 0.0003 ± 0.001", 0.0003f, baiFit.xOffset, 0.001f)
        assertEquals("bai yOffset(=cy) 应为 -0.122 ± 0.001", -0.122f, baiFit.yOffset, 0.001f)
    }

    // ============================================================
    // A4：竖屏 viewport 下仍 contain-fit 不裁切（1080×2340 → 1.0×2.1667）
    // ============================================================
    @Test
    fun portraitViewport() {
        // 竖屏：viewWidth=1.0, viewHeight=2.1667（2340/1080），bai bbox 0.0830×0.2525 居中
        // min(1/0.0830, 2.1667/0.2525) = min(12.048, 8.581) = 8.581 → ×0.95 = 8.152 ≈ 8.15
        val bbox = BBox(-0.0415f, 0.0415f, -0.12625f, 0.12625f)
        val fit = computeCharacterFit(bbox, 1.0f, 2.1667f, 1.0f, 2.1667f)

        assertEquals("竖屏 contain-fit scale 应为 8.15 ± 0.01", 8.15f, fit.scale, 0.01f)

        // 竖屏不裁切：受限维(高度)占比 == 0.95 ± 0.01
        val limitedDimRatio = (bbox.maxY - bbox.minY) * fit.scale / 2.1667f
        assertEquals("竖屏受限维(高)占比应为 0.95 ± 0.01", 0.95f, limitedDimRatio, 0.01f)
    }

    // ============================================================
    // A5：退化输入 → 不启用 fit（scale=1/xOffset=0/yOffset=0 = 现状）
    // ============================================================
    @Test
    fun degenerateBbox_noFit() {
        // ---- characterBBox：空集/单点/零宽/NaN/Inf → null ----
        assertNull("空列表应为退化 → null", characterBBox(emptyList()))
        assertNull("单点(宽高均 0)应为退化 → null", characterBBox(listOf(floatArrayOf(1f, 2f))))
        // 零宽：多个顶点但 x 全相同 → bboxW=0 → 退化
        assertNull(
            "零宽 bbox 应为退化 → null",
            characterBBox(listOf(floatArrayOf(0f, 0f, 0f, 2f)))
        )
        // NaN / Inf：任一坐标非有限 → 退化
        assertNull(
            "含 NaN 顶点应为退化 → null",
            characterBBox(listOf(floatArrayOf(Float.NaN, 0f, 1f, 2f)))
        )
        assertNull(
            "含 Inf 顶点应为退化 → null",
            characterBBox(listOf(floatArrayOf(Float.POSITIVE_INFINITY, 0f, 1f, 2f)))
        )

        // ---- computeCharacterFit：view≤0 / canvas≤0 / padding<0 → CharacterFit(1,0,0) ----
        val validBbox = BBox(-0.0415f, 0.0415f, -0.12625f, 0.12625f)
        val degenerateViewWidth = computeCharacterFit(validBbox, 0f, 1.5f, 1f, 1.5f)
        assertFit_identity("viewWidth=0", degenerateViewWidth)
        val degenerateViewHeight = computeCharacterFit(validBbox, 1f, 0f, 1f, 1f)
        assertFit_identity("viewHeight=0", degenerateViewHeight)
        val degenerateCanvasWidth = computeCharacterFit(validBbox, 1f, 1.5f, 0f, 1.5f)
        assertFit_identity("canvasWidth=0", degenerateCanvasWidth)
        val degenerateCanvasHeight = computeCharacterFit(validBbox, 1f, 1.5f, 1f, 0f)
        assertFit_identity("canvasHeight=0", degenerateCanvasHeight)
        val degeneratePadding = computeCharacterFit(validBbox, 1f, 1.5f, 1f, 1.5f, -0.1f)
        assertFit_identity("padding<0", degeneratePadding)

        // （ppu≤0 守卫发生在 renderer 集成层，未在本纯函数签名暴露 —— 见覆盖注记，非本层可测）
    }

    // ============================================================
    // A6：bbox 聚合 min/max；Y-flip 后范围不变；单点/空退化
    // ============================================================
    @Test
    fun bboxAggregation_minMax() {
        // 2 个 drawable 顶点数组，求全局 min/max（xy 交错）
        val drawableA = floatArrayOf(1f, 5f, 3f, -2f)    // pts: (1,5),(3,-2)
        val drawableB = floatArrayOf(-7f, 8f, 4f, 6f)    // pts: (-7,8),(4,6)
        val bbox = characterBBox(listOf(drawableA, drawableB))

        assertEquals("全局 minX 应为 -7", -7f, bbox!!.minX, 1e-4f)
        assertEquals("全局 maxX 应为 4", 4f, bbox.maxX, 1e-4f)
        assertEquals("全局 minY 应为 -2", -2f, bbox.minY, 1e-4f)
        assertEquals("全局 maxY 应为 8", 8f, bbox.maxY, 1e-4f)
    }

    @Test
    fun bboxAggregation_yFlipPreservesRange() {
        // Y-flip（y 取反）后 min/max 交换，但范围 (maxY-minY) 不变
        val drawableA = floatArrayOf(1f, 5f, 3f, -2f)
        val drawableB = floatArrayOf(-7f, 8f, 4f, 6f)
        val original = characterBBox(listOf(drawableA, drawableB))!!

        val flippedA = floatArrayOf(1f, -5f, 3f, 2f)
        val flippedB = floatArrayOf(-7f, -8f, 4f, -6f)
        val flipped = characterBBox(listOf(flippedA, flippedB))!!

        val originalSpan = original.maxY - original.minY
        val flippedSpan = flipped.maxY - flipped.minY
        assertEquals("Y-flip 后高度范围应不变", originalSpan, flippedSpan, 1e-4f)
        assertEquals("Y-flip 后 minY 应等于原 maxY 的负值", -original.maxY, flipped.minY, 1e-4f)
        assertEquals("Y-flip 后 maxY 应等于原 minY 的负值", -original.minY, flipped.maxY, 1e-4f)

        // 宽度不受 Y-flip 影响
        assertEquals("Y-flip 对 minX 不变", original.minX, flipped.minX, 1e-4f)
        assertEquals("Y-flip 对 maxX 不变", original.maxX, flipped.maxX, 1e-4f)
    }

    // ============================================================
    // A7：ppu 不变式 —— 同一模型在 ppu=1 与 ppu=4000 换算输入下 fitScale 相同
    // ============================================================
    @Test
    fun ppuInvariance() {
        // ppu=4000：顶点 px → 模型单位（÷ppu），view = 画布模型单位 1.0×1.5
        val modelBbox = BBox(0f, 332f / 4000f, 0f, 1010f / 4000f)
        val modelFit = computeCharacterFit(modelBbox, 1.0f, 1.5f, 1.0f, 1.5f)

        // ppu=1：顶点与画布同尺度（px 视作模型单位），view = 画布 px 4000×6000
        val pxBbox = BBox(0f, 332f, 0f, 1010f)
        val pxFit = computeCharacterFit(pxBbox, 4000f, 6000f, 4000f, 6000f)

        // 两世界 fitScale 必须一致（ratio 同构，架构 §3.2）
        assertEquals("ppu=4000 与 ppu=1 的 fitScale 应一致（ppu 不变式）", pxFit.scale, modelFit.scale, 1e-4f)
        // 且命中契约锁定值 5.643（min(1/0.0830, 1.5/0.2525)×0.95）
        assertEquals("模型 fitScale 应为 5.643 ± 0.01", 5.643f, modelFit.scale, 0.01f)
    }

    // ============================================================
    // 辅助
    // ============================================================

    private fun assertFit_identity(label: String, fit: CharacterFit) {
        assertEquals("[$label] scale 应 => 1f（= 现状）", 1f, fit.scale, 1e-6f)
        assertEquals("[$label] xOffset 应 => 0f（= 现状）", 0f, fit.xOffset, 1e-6f)
        assertEquals("[$label] yOffset 应 => 0f（= 现状）", 0f, fit.yOffset, 1e-6f)
    }

    // t: bbox 原始 (bboxW, bboxH)；返回受限维(宽或高)在 view 为 1×1 下的占比
    private fun widthTrimRatio(bboxW: Float, bboxH: Float): Float = bboxW / 1f
    private fun heightTrimRatio(bboxW: Float, bboxH: Float): Float = bboxH / 1f

    private class DimRatios(val widthRatio: Float, val heightRatio: Float)

    // 计算 bbox 相对 view 的宽/高占比（未含 padding）
    private fun minRatioOf(bbox: BBox, viewWidth: Float, viewHeight: Float): DimRatios {
        val bboxW = bbox.maxX - bbox.minX
        val bboxH = bbox.maxY - bbox.minY
        return DimRatios(bboxW / viewWidth, bboxH / viewHeight)
    }
}
