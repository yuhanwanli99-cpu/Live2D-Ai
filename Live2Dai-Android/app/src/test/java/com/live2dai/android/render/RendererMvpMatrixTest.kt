package com.live2dai.android.render

import com.live2dai.android.data.CharacterFit
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Wave 2 · 2A-1 渲染替换 —— fit → MvpMatrix float[16] 对拍测试（RED）。
 *
 * 依据: architecture-refactor-render-0814.md M2「fit→MvpMatrix(float[16])（FitMath/ProjectionMath
 *       纯函数保留）」+ §4.1 薄壳「fit/override → engineSetMvpMatrix(float[16])」
 *       + contract-refactor-0814.md §2.5 `engineSetMvpMatrix(float[16])`。
 * 契约锚: M2「fit→MvpMatrix(float[16])」；§2.5 engineSetMvpMatrix(float[16])。
 *
 * 判定方式: 纯函数 seam `MvpMatrixMath.toMvpMatrix(fit, bounds)` → float[16]，机械断言
 *           **矩阵通用不变量**（与具体矩阵公式脱钩，防猜公式）：
 *             (d) 长度 == 16（engineSetMvpMatrix 前置）；
 *             (e) 无 NaN / 无 Inf；
 *             (f) 非零（至少一个对角元非零）——非退化 fit 必须产生非零矩阵；
 *             (g) 可逆（|det| > 0）——投影/仿射乘法可逆，不坍缩；
 *             (h) 对角元全部有限。
 *           seam `MvpMatrixMath` 未实现 → 编译期 unresolved reference = RED。
 *
 * 注: 契约/架构未锁定具体矩阵公式（column-major 序/仿射 vs 透视/轴翻转）——本测试只锁
 *     **公式无关的矩阵不变量**，不锁数值。coder 实现若违反任一通用不变量 → 冲突上报主代理。
 */
class RendererMvpMatrixTest {

    /** bai contain-fit 输出（契约 M1-A1/A3 锁定值：fitScale≈5.643、xOffset≈0.0003、yOffset≈-0.122）。 */
    private val baiFit = CharacterFit(scale = 5.643f, xOffset = 0.0003f, yOffset = -0.122f)

    /** 投影边界 [left, right, top, bottom]（契约 C-M1 A8，正交 + xOffset）。 */
    private val orthoBounds = floatArrayOf(-10f, 10f, 8.15f, -6f)

    // ==================== 双引号：断言长度（engineSetMvpMatrix 前置） ====================

    @Test
    fun mvpMatrix_hasLength16_forEngineSetMvpMatrix() {
        val mvp = MvpMatrixMath.toMvpMatrix(baiFit, orthoBounds)
        assertTrue(
            "engineSetMvpMatrix 需要 float[16]，实际大小 = ${mvp.size}",
            mvp.size == 16,
        )
    }

    @Test
    fun mvpMatrix_hasNoNaN_orInf() {
        val mvp = MvpMatrixMath.toMvpMatrix(baiFit, orthoBounds)
        for ((i, v) in mvp.withIndex()) {
            assertFalse("MvpMatrix[$i] 不得为 NaN，实际 $v", v.isNaN())
            assertFalse("MvpMatrix[$i] 不得为 Inf，实际 $v", v.isInfinite())
        }
    }

    @Test
    fun mvpMatrix_isNonZero_forNonDegenerateFit() {
        val mvp = MvpMatrixMath.toMvpMatrix(baiFit, orthoBounds)
        var diagonalNonZero = false
        for (i in 0 until 4) {
            if (mvp[i * 4 + i] != 0f) diagonalNonZero = true
        }
        assertTrue("非退化 fit 必须产生非零 MvpMatrix（至少一个对角元非零）", diagonalNonZero)
    }

    @Test
    fun mvpMatrix_isInvertible_determinantNonZero() {
        val mvp = MvpMatrixMath.toMvpMatrix(baiFit, orthoBounds)
        val det = determinant4(mvp)
        assertTrue("MvpMatrix 必须可逆（|det| = ${abs(det)} 须 > 1e-6，防投影/仿射坍缩）", abs(det) > 1e-6f)
    }

    @Test
    fun mvpMatrix_diagonalAllFinite() {
        val mvp = MvpMatrixMath.toMvpMatrix(baiFit, orthoBounds)
        for (i in 0 until 4) {
            val d = mvp[i * 4 + i]
            assertFalse("对角元 Mvp[${i * 4 + i}] 不得为 NaN", d.isNaN())
            assertFalse("对角元 Mvp[${i * 4 + i}] 不得为 Inf", d.isInfinite())
        }
    }

    // ==================== helper ====================

    /** 4x4 行列式（列主序展开，返回类型与布局无关；MvpMatrix 不变量判定用）。 */
    private fun determinant4(m: FloatArray): Float {
        val a0 = m[0]; val a1 = m[1]; val a2 = m[2]; val a3 = m[3]
        val b0 = m[4]; val b1 = m[5]; val b2 = m[6]; val b3 = m[7]
        val c0 = m[8]; val c1 = m[9]; val c2 = m[10]; val c3 = m[11]
        val d0 = m[12]; val d1 = m[13]; val d2 = m[14]; val d3 = m[15]
        val k1 = a0 * (b1 * (c2 * d3 - c3 * d2) - b2 * (c1 * d3 - c3 * d1) + b3 * (c1 * d2 - c2 * d1))
        val k2 = -a1 * (b0 * (c2 * d3 - c3 * d2) - b2 * (c0 * d3 - c3 * d0) + b3 * (c0 * d2 - c2 * d0))
        val k3 = a2 * (b0 * (c1 * d3 - c3 * d1) - b1 * (c0 * d3 - c3 * d0) + b3 * (c0 * d1 - c1 * d0))
        val k4 = -a3 * (b0 * (c1 * d2 - c2 * d1) - b1 * (c0 * d2 - c2 * d0) + b2 * (c0 * d1 - c1 * d0))
        return k1 + k2 + k3 + k4
    }

    private fun abs(v: Float): Float = if (v < 0f) -v else v
}
