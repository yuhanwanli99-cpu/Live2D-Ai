package com.live2dai.android.data

import org.junit.Assert.assertArrayEquals
import org.junit.Test

/**
 * 半身显示投影纯函数断言（契约 v3-rename §2.1/§2.3，R6；+契约 C-M1 §2.2/§3 M1-A8 xOffset 扩展）。
 *
 * 契约锁定的 orthoM 投影公式：
 *     halfW = viewWidth / 2；halfH = viewHeight / 2
 *     halfW' = halfW / scale；halfH' = halfH / scale
 *     left=-halfW'，right=halfW'，bottom=-halfH' + yOffset，top=halfH' + yOffset
 *
 * C-M1 A4/A8 扩展：新增可选 `xOffset: Float = 0f`（additive，默认 0 → 既有 6 测试逐位不变）
 *     left = -halfW' + xOffset；right = halfW' + xOffset（水平平移）
 *     top/bottom 不变（垂直不受 xOffset 影响）
 *
 * 生产符号已实现：`data/ProjectionMath.computeProjection(scale, yOffset, viewWidth,
 * viewHeight, canvasWidth, canvasHeight[, xOffset = 0f])`（返回 FloatArray[4]，顺序 = [left, right,
 * top, bottom]；渲染端 Live2DRenderer.onSurfaceChanged 委托调用）→ 既有 6 测试为 GREEN 回归，
 * xOffset 相关为 RED（待实现扩展）。
 */
class ProjectionMathTest {

    // §2.1 锁定的投影边界纯计算（测试内部按公式复算期望值；断言第 3 条直接引用生产符号）
    private fun lockedBounds(
        scale: Float,
        yOffset: Float,
        viewWidth: Float,
        viewHeight: Float,
    ): FloatArray {
        val halfW = viewWidth / 2f
        val halfH = viewHeight / 2f
        val halfWPrime = halfW / scale
        val halfHPrime = halfH / scale
        // [left, right, top, bottom]；left=-halfW'，right=halfW'，top=halfH'+yOffset，bottom=-halfH'+yOffset
        return floatArrayOf(
            -halfWPrime,
            halfWPrime,
            halfHPrime + yOffset,
            -halfHPrime + yOffset,
        )
    }

    private fun assertProjection(scale: Float, yOffset: Float, view: Float, expected: FloatArray) {
        // 用同宽高的画布：canvasWidth/Height 传 viewWidth/Height（映射平凡化）
        val actual = computeProjection(scale, yOffset, view, view, view, view)
        assertArrayEquals(
            "computeProjection($scale, $yOffset, $view) 应命中契约锁定矩阵边界",
            expected,
            actual,
            1e-5f
        )
    }

    @Test
    fun defaultScaleNoOffset_returnsCurrentOrtho_zeroRegression() {
        // §2.3：computeProjection(1f, 0f, ...) == 现状 orthoM 输出（全模型，零回归）
        assertProjection(1f, 0f, 100f, lockedBounds(1f, 0f, 100f, 100f))
    }

    @Test
    fun scaleOnePointFour_narrowsBoundsCentered() {
        // §2.3：scale>1 → 边界收窄（halfW/halfH 缩小），中心仍在原点
        assertProjection(1.4f, 0f, 100f, lockedBounds(1.4f, 0f, 100f, 100f))
    }

    @Test
    fun yOffset_positive_raisesTop_andLowersBottom_bothShiftedUp() {
        // §2.1：yOffset>0 把上下边界同向上移 → 模型视口下移，下半身出画布被裁切（上半身特写）
        assertProjection(1f, 5f, 100f, lockedBounds(1f, 5f, 100f, 100f))
    }

    @Test
    fun scaleAndOffsetCombined_correctBounds() {
        // 组合场景：放大 + 上移，边界同锁公式
        assertProjection(1.4f, 8f, 120f, lockedBounds(1.4f, 8f, 120f, 120f))
    }

    @Test
    fun scaleBelowOne_widensBounds() {
        // scale<1（缩小）→ 边界拓宽
        assertProjection(0.5f, 0f, 100f, lockedBounds(0.5f, 0f, 100f, 100f))
    }

    @Test
    fun zeroOffsetAndScale_keepsCenter_TopBottomSymmetryAboutRaisedOffset() {
        // yOffset>0 时 top 与 bottom 关于 yOffset 对称：top + bottom = 2*yOffset
        val scale = 1.4f
        val yOffset = 8f
        val view = 120f
        val actual = computeProjection(scale, yOffset, view, view, view, view)
        val symmetry = actual[2] + actual[3] - 2 * yOffset
        org.junit.Assert.assertTrue("top + bottom 应 = 2*yOffset，偏差 $symmetry", abs(symmetry) < 1e-4f)
        // 水平中心应保持 0：left + right = 0
        val center = actual[1] + actual[0]
        org.junit.Assert.assertTrue("水平中心应保持 0，偏差 $center", abs(center) < 1e-4f)
    }

    // =====================================================================
    // C-M1 M1-A8：xOffset 扩展（additive，默认 0 不破坏既有测试；xOffset≠0 水平平移）
    // =====================================================================

    @Test
    fun xOffsetDefaultZero_returnsCurrentOrtho_explicitZeroRegression() {
        // xOffset 默认 0f 必须是"不传"与"显式传 0f"逐位一致，且都等于现状（K5 公式）
        val scale = 1f
        val yOffset = 0f
        val view = 100f
        val viaDefault = computeProjection(scale, yOffset, view, view, view, view)
        val viaExplicitZero = computeProjection(scale, yOffset, view, view, view, view, 0f)
        assertArrayEquals(
            "xOffset 默认 0f 应与显式 xOffset=0f 逐位一致",
            viaDefault,
            viaExplicitZero,
            1e-5f
        )
        // 且等于契约锁定现状矩阵
        assertArrayEquals(
            "xOffset=0 路径仍应命中 K5 现状正交矩阵",
            lockedBounds(scale, yOffset, view, view),
            viaDefault,
            1e-5f
        )
    }

    @Test
    fun xOffset_five_shiftsLeftRight_keepsVertical_centerAtFive() {
        // xOffset=5f：left/right 各 +5；top/bottom 不变；水平中心 = 5f
        val scale = 1.4f
        val yOffset = 8f
        val view = 120f

        val baseline = computeProjection(scale, yOffset, view, view, view, view)   // xOffset 默认 0
        val shifted = computeProjection(scale, yOffset, view, view, view, view, 5f)

        // left / right 各 +5
        org.junit.Assert.assertEquals("left 应 +5", baseline[0] + 5f, shifted[0], 1e-4f)
        org.junit.Assert.assertEquals("right 应 +5", baseline[1] + 5f, shifted[1], 1e-4f)
        // top / bottom 不变
        assertEqualsF("top 应保持不变", baseline[2], shifted[2], 1e-4f)
        assertEqualsF("bottom 应保持不变", baseline[3], shifted[3], 1e-4f)
        // 水平中心 = xOffset = 5f
        org.junit.Assert.assertEquals("水平中心应 = xOffset = 5f", 5f, (shifted[0] + shifted[1]) / 2f, 1e-4f)
    }

    @Test
    fun xOffsetCombineScaleAndYOffset_correctBounds() {
        // xOffset 与 scale/yOffset 正交叠加：单边界仍由 lockedBounds + 水平平移精确表达
        val scale = 1.4f
        val yOffset = 8f
        val view = 120f
        val xOffset = 5f
        val expected = lockedBounds(scale, yOffset, view, view)
        expected[0] += xOffset
        expected[1] += xOffset
        val actual = computeProjection(scale, yOffset, view, view, view, view, xOffset)
        assertArrayEquals(
            "xOffset 与 scale/yOffset 叠加边界应精确 = 平移后的锁定矩阵",
            expected,
            actual,
            1e-4f
        )
    }

    private fun assertEqualsF(msg: String, expected: Float, actual: Float, delta: Float) {
        org.junit.Assert.assertEquals(msg, expected, actual, delta)
    }

    private fun abs(v: Float): Float = if (v < 0f) -v else v
}
