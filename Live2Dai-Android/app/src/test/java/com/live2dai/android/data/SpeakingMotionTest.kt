// =============================================================================
// RED 测试 — R17 说话联动增强（头部点头 + 身体起伏）
// T8 测试设计师 | 依据: .pi/spoq/translation.md R17 + search-report-expression.md
//
// 【接口契约 — 给 coder】
// 新类:  package com.live2dai.android.data
//        object SpeakingMotion   （纯函数/纯静态，JVM 可测，禁 Android 依赖）
//
//  常量（byte-exact 照抄）:
//     const val NOD_PERIOD_SECONDS: Float      = 1.5f    // 头点头周期 ∈[1,2]s（低频 0.5-1Hz）✅
//     const val NOD_AMPLITUDE_HEAD_DEG: Float  = 4f      // 头点头幅度（峰值 ∈[3,5]°）✅
//     const val BODY_RISE_PERIOD_SECONDS: Float = 2.4f   // 身体起伏周期（与呼吸错相位=周期不同）✅
//     const val BODY_RISE_AMPLITUDE_DEG: Float = 2f      // 身体起伏幅度 ∈[0,3]°（BodyAngle 度）✅
//     const val BREATH_PERIOD_SECONDS: Float   = 2.5f    // 呼吸周期（Cubism 0.3-0.5Hz → 2-3.3s，取 2.5s）
//
//  函数:
//     fun headNodOffset(timeSeconds: Float): Float
//        = NOD_AMPLITUDE_HEAD_DEG * sin(2π * timeSeconds / NOD_PERIOD_SECONDS)  // 秒级相位
//        ∈ [-NOD_AMPLITUDE, NOD_AMPLITUDE] ⊆ [-5,5]
//     fun bodyRiseOffset(timeSeconds: Float): Float
//        = BODY_RISE_AMPLITUDE_DEG * sin(2π * timeSeconds / BODY_RISE_PERIOD_SECONDS)  // 低频
//        ∈ [-BODY_RISE_AMPLITUDE, BODY_RISE_AMPLITUDE] ⊆ [-3,3]
//
//  错相位语义（tester 裁定）:
//     - 需求「频率与呼吸错相位（相位差≥60° 或 周期不同）」——判定走 OR 的「周期不同」分支：
//       身体周期 BODY_RISE_PERIOD_SECONDS(2.4s) ≠ 呼吸周期 BREATH_PERIOD_SECONDS(2.5s)，
//       两正弦周期互质→长期相位漂移、任意时点相位差随时间变化，机械断言锁定「周期不同」。
//     - 「与口型同步节奏」（tester 裁定）:
//       口型波动 ~100ms（快波动）VS 头点头 1-2s（慢节拍）——量级不同，故只锁
//       点头周期 ∈ [1,2]s（慢拍），口型快波动由 LipSync 独立负责（不在本类断言周期量级）。
// =============================================================================

package com.live2dai.android.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.sin

class SpeakingMotionTest {

    // ---------- 1. 头部点头：频率 0.5-1Hz（周期 1-2s）----------
    @Test
    fun `SM-01 nod period is 1 to 2 seconds (low freq 0-5-1-0Hz)`() {
        assertTrue(
            "点头周期 ${SpeakingMotion.NOD_PERIOD_SECONDS} 须 ∈[1,2]s（低频 0.5-1Hz）",
            SpeakingMotion.NOD_PERIOD_SECONDS >= 1.0f && SpeakingMotion.NOD_PERIOD_SECONDS <= 2.0f
        )
    }

    @Test
    fun `SM-02 nod amplitude peak within 3 to 5 degrees and bounded by 5`() {
        val a = SpeakingMotion.NOD_AMPLITUDE_HEAD_DEG
        assertTrue(
            "点头幅度峰值 ${a} 须 ∈[3,5]°（需求 ±3-5°）",
            a >= 3.0f && a <= 5.0f
        )
        // 正弦输出幅度受限于峰值且上界 5
        val quarter = SpeakingMotion.NOD_PERIOD_SECONDS / 4f
        val peakOut = SpeakingMotion.headNodOffset(quarter)
        assertTrue("峰值输出≈峰值幅度", abs(peakOut - a) < 1e-3f)
        assertTrue("输出上界 ≤5", abs(peakOut) <= 5.0f)
    }

    @Test
    fun `SM-03 nod output is sinusoidal and bounded everywhere`() {
        val period = SpeakingMotion.NOD_PERIOD_SECONDS
        val amp = SpeakingMotion.NOD_AMPLITUDE_HEAD_DEG
        // t=0 → 0；t=period/2 → 过零；任意 t |out|≤amp
        assertEquals(0f, SpeakingMotion.headNodOffset(0f), 1e-3f)
        assertEquals(0f, SpeakingMotion.headNodOffset(period / 2f), 1e-3f)
        // 期望解析式：out = A*sin(2π t / period)
        val t = 0.37f
        val expected = (amp * sin(2.0 * PI * t / period)).toFloat()
        assertEquals(expected, SpeakingMotion.headNodOffset(t), 1e-3f)
        // 全域钳制
        val nPts = 800
        for (k in 0..nPts) {
            val tt = k.toFloat() * period / nPts
            assertTrue("|nod(t)| ≤ $amp", abs(SpeakingMotion.headNodOffset(tt)) <= amp + 1e-3f)
            assertTrue("|nod(t)| ≤ 5（需求上界）", abs(SpeakingMotion.headNodOffset(tt)) <= 5.0f)
        }
    }

    // ---------- 2. 身体起伏：幅度 [0,3]°、低频、与呼吸错相位 ----------
    @Test
    fun `SM-04 body rise amplitude within 0 to 3 degrees and bounded`() {
        val a = SpeakingMotion.BODY_RISE_AMPLITUDE_DEG
        assertTrue("身体起伏幅度 ${a} 须 ∈[0,3]°（BodyAngle 度）", a >= 0.0f && a <= 3.0f)
        val quarter = SpeakingMotion.BODY_RISE_PERIOD_SECONDS / 4f
        assertTrue("峰值输出≈幅度", abs(SpeakingMotion.bodyRiseOffset(quarter) - a) < 1e-3f)
        assertTrue("输出上界 ≤3", abs(SpeakingMotion.bodyRiseOffset(quarter)) <= 3.0f)
        // 全域钳制
        val nPts = 600
        for (k in 0..nPts) {
            val tt = k.toFloat() * SpeakingMotion.BODY_RISE_PERIOD_SECONDS / nPts
            assertTrue("|bodyRise(t)| ≤ $a", abs(SpeakingMotion.bodyRiseOffset(tt)) <= a + 1e-3f)
        }
    }

    @Test
    fun `SM-05 body rise is out of phase with breath (different period)`() {
        // 需求「频率与呼吸错相位（相位差≥60° 或 周期不同）」→ 走「周期不同」分支
        assertTrue(
            "身体周期 ${SpeakingMotion.BODY_RISE_PERIOD_SECONDS} ≠ 呼吸周期 ${SpeakingMotion.BREATH_PERIOD_SECONDS} → 错相位",
            abs(SpeakingMotion.BODY_RISE_PERIOD_SECONDS - SpeakingMotion.BREATH_PERIOD_SECONDS) > 1e-4f
        )
    }

    @Test
    fun `SM-06 body rise is low frequency (period 2-3-3s range like idle breath)`() {
        // 身体低频：周期落在官方呼吸同级量级（2-3.3s）附近
        val p = SpeakingMotion.BODY_RISE_PERIOD_SECONDS
        assertTrue("身体起伏低频周期 $p ∈[1.5,3.5]s", p >= 1.5f && p <= 3.5f)
    }

    // ---------- 3. 与口型同步节奏（tester 裁定：慢节拍 1-2s vs 快波动 ~100ms）----------
    @Test
    fun `SM-07 nod is slow beat while lip fluctuation is fast magnitude`() {
        // 口型 ~100ms 波动 vs 点头 1-2s——量级不同。点头周期已被 SM-01 锁 ∈[1,2]s。
        // 此处补充量级对比锚点（仅锁定点头慢拍，不强行要求嘴型同周期）
        val nodPeriod = SpeakingMotion.NOD_PERIOD_SECONDS
        assertTrue("点头慢拍周期 ${nodPeriod}s ≥ 1s", nodPeriod >= 1.0f)
        // 声明性对照：口型快波动量级 ~0.1s（LipSync 独立方言，本类不负责任何嘴型函数）
        assertTrue("慢拍周期 ≫ 快波动量级(0.1s)", nodPeriod >= 0.1f * 10f)
    }
}
