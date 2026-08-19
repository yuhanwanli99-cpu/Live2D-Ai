package com.live2d.ai.android.render

import com.live2d.ai.android.data.MotionSynthesisGenerator
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * A-5=F-b 合成 motion3 生成器纯函数 RED 测试（表情参数帧 → 一次性合成运动）。
 *
 * 依据（唯一真相源）：
 *  - .pi/spoq/architecture-refactor-render-0814.md §5.2（A-5 裁定 F-b）：
 *      "EmotionController 帧 → 引擎内合成一次性 motion3.json（12 键、时长=fadeMs、Linear 段、FadeInTime=fadeMs）
 *       → CubismMotion::Create → CubismExpressionMotionManager 或 MotionManager 播放 → 官方 fade/权重/优先级机制全接管。
 *       合成器是自研残留（~100 行纯函数，JVM 可测）。"
 *  - .pi/spoq/contract-refactor-0814.md §3 RG1-A3 / §1.3 C-15（表情最小语义集）。
 *
 * 生产 seam 对齐（主代理下发，data/MotionSynthesisGenerator.kt 实际形状）：
 *   - `synthesizeEmotionMotion(paramFrame: Map<String,Float>, durationMs: Long, fadeMs: Long,
 *     sampleRateFps: Int): SynthesizedMotion3`，纯函数；
 *   - `SynthesizedMotion3{ curves: List<MotionCurveSegment>, sampleCount: Int,
 *     durationMs: Long, fadeMs: Long, loop: Boolean }`——纯数据载体（无 json/无 keyTimesMs 列表）；
 *   - `MotionCurveSegment(key, startValue, endValue)`：合成曲线 start==end==paramFrame 值（平台段，
 *     目标绝对值恒为 paramFrame 值）；
 *   - `sampleCount = durationMs * sampleRateFps / 1000`（整数除法，无 +1）；
 *   - fade 语义在 `segmentSnapshot(s, atIndex)` 采样时施加（包络 min(fadeIn, fadeOut, 1)）：
 *     首 fadeMs 内 0→目标值线性淡入、末 fadeMs 内目标值→0 线性淡出、中间平台恒为目标值；
 *     第 i 采样点时刻 t_i = i * durationMs / sampleCount（采样率恒定）；
 *   - 非法键 = 空串（跳过）；未知非空参数名透传（合法域校验不在本 seam）。
 *
 * 与原 RED 断言的不变量映射（对齐生产，语义不弱化）：
 *   ① 曲线数 = 入参参数键数（空串键除外）——保持；
 *   ② 「首键值=中性基线 0 / 末键值=目标值」→ 生产形态「曲线平台值 start==end==目标值；
 *      fade 包络：snapshot(0)==0（淡入首）、中间平台==目标值、末段 <平台（淡出）」——拉满语义保持；
 *   ③ 「相邻 keyTimes 差恒定」→ 生产形态「sampleCount = duration*fps/1000 + 平台边界与
 *      t_i = i·duration/n 一致（机械校验采样网格恒定）」——保持；
 *   ④ 时长一致（durationMs == 入参）——保持；
 *   ⑤ fade 元数据原样保留 + Loop=false——保持。
 * 删除（生产 seam 无对应物，报告已列）：SG07 json 序列化断言（SynthesizedMotion3 为纯数据载体，
 * json 生成不在本 seam）；SG08 原「NotARealParam 被跳过」口径（生产仅空串键为非法键）。
 */
class MotionSynthesisGeneratorTest {

    // bai 表情 6 键（既有 A3 fixture 的合法键域；键值取中性/目标示意）
    private val baiEmotionKeys = listOf(
        "ParamBrowLY", "ParamBrowRY", "ParamEyeLOpen", "ParamEyeROpen",
        "ParamMouthForm", "ParamMouthOpenY",
    )

    // ═══════════════════════════════════════════════════════════
    // SG-01：空参数帧 → 无异常、空曲线、时长一致
    // ═══════════════════════════════════════════════════════════

    @Test
    fun SG01_emptyFrame_noThrow_emptyCurves_durationConsistent() {
        val m = MotionSynthesisGenerator.synthesizeEmotionMotion(emptyMap(), durationMs = 1000L, fadeMs = 300L, sampleRateFps = 60)
        assertNotNull(m)
        assertEquals("空帧曲线数=0", 0, m.curves.size)
        assertEquals("时长一致（1000ms）", 1000L, m.durationMs)
        assertEquals("fade 元数据原样保留（300ms）", 300L, m.fadeMs)
        assertEquals("空帧采样数 = 1000*60/1000 = 60", 60, m.sampleCount)
        assertFalse("一次性运动非 Loop", m.loop)
    }

    // ═══════════════════════════════════════════════════════════
    // SG-02：单键 → 平台段（start==end==目标值）/ 时长 / fade / Loop 一次性
    //       + fade 包络采样：首采样=0（淡入起）、平台=目标值、末段衰减（淡出）
    // ═══════════════════════════════════════════════════════════

    @Test
    fun SG02_singleKey_platformSegment_duration_fade_loop() {
        val frame = mapOf("ParamMouthOpenY" to 0.8f)
        val m = MotionSynthesisGenerator.synthesizeEmotionMotion(frame, durationMs = 1000L, fadeMs = 300L, sampleRateFps = 60)
        assertEquals("单键 → 曲线数=1", 1, m.curves.size)
        val seg = m.curves[0]
        assertEquals("参数名保留", "ParamMouthOpenY", seg.key)
        // 平台段：start==end==目标值（F-b：曲线本身是平台段，目标绝对值恒定为 paramFrame 值）
        assertEquals("曲线起点 == 目标值 0.8", 0.8f, seg.startValue, 1e-6f)
        assertEquals("曲线终点 == 目标值 0.8", 0.8f, seg.endValue, 1e-6f)
        assertEquals("采样数 = 60fps×1s = 60", 60, m.sampleCount)
        assertEquals("时长==入参 1000ms", 1000L, m.durationMs)
        assertEquals("fade==入参 300ms", 300L, m.fadeMs)
        assertFalse("一次性 motion 非 Loop", m.loop)

        // fade 包络（segmentSnapshot）：首采样淡入起点=0；平台（t=500ms）== 目标值；末段淡出 < 平台
        val first = MotionSynthesisGenerator.segmentSnapshot(m, 0)["ParamMouthOpenY"]!!
        assertEquals("首采样（t=0）淡入起点 == 0（中性基线语义）", 0f, first, 1e-6f)
        val mid = MotionSynthesisGenerator.segmentSnapshot(m, 30)["ParamMouthOpenY"]!!
        assertEquals("平台采样（t=500ms ∈ [300,700]）== 目标值 0.8", 0.8f, mid, 1e-6f)
        val tail = MotionSynthesisGenerator.segmentSnapshot(m, 59)["ParamMouthOpenY"]!!
        assertTrue("末段采样（t=983ms > 700ms 淡出区）< 平台值", tail < mid - 1e-6f)
        assertTrue("末段采样 ≥ 0（淡出向 0 收敛）", tail >= 0f)
    }

    // ═══════════════════════════════════════════════════════════
    // SG-03：多键（bai 表情 6 键 × 强度档）→ 每键一条曲线
    // ═══════════════════════════════════════════════════════════

    @Test
    fun SG03_multiKey_eachKeyOneCurve() {
        val frame = baiEmotionKeys.associateWith { 0.6f } // 强度 0.6
        val m = MotionSynthesisGenerator.synthesizeEmotionMotion(frame, durationMs = 1000L, fadeMs = 300L, sampleRateFps = 60)
        assertEquals("曲线数=参数键数(${baiEmotionKeys.size})", baiEmotionKeys.size, m.curves.size)
        val segNames = m.curves.map { it.key }.toSet()
        assertEquals("覆盖全部入参键", baiEmotionKeys.toSet(), segNames)
        for (seg in m.curves) {
            assertEquals("曲线起点=目标强度 0.6", 0.6f, seg.startValue, 1e-6f)
            assertEquals("曲线终点=目标强度 0.6", 0.6f, seg.endValue, 1e-6f)
        }
    }

    // ═══════════════════════════════════════════════════════════
    // SG-04：采样率恒定（采样网格 = t_i = i·duration/sampleCount；平台边界机械校验）
    // ═══════════════════════════════════════════════════════════

    @Test
    fun SG04_uniformSampling_sampleCountAndGridBoundaries() {
        val frame = mapOf("ParamEyeLOpen" to 0.5f)
        // 2000ms × 30fps → sampleCount = 60；fade=500ms → 平台区 t ∈ [500,1500] → idx ∈ [15,45]
        val m = MotionSynthesisGenerator.synthesizeEmotionMotion(frame, durationMs = 2000L, fadeMs = 500L, sampleRateFps = 30)
        assertEquals("采样数 = 2000×30/1000 = 60（采样率恒定）", 60, m.sampleCount)

        val snap = { i: Int -> MotionSynthesisGenerator.segmentSnapshot(m, i)["ParamEyeLOpen"]!! }
        assertEquals("首采样（t=0）== 0（淡入起）", 0f, snap(0), 1e-6f)
        assertTrue("淡入区（t=466.7ms < 500）< 目标值", snap(14) < 0.5f - 1e-6f)
        assertEquals("平台首（t=500ms 恰为淡入终点）== 目标值 0.5", 0.5f, snap(15), 1e-6f)
        assertEquals("平台末（t=1500ms 恰为淡出起点）== 目标值 0.5", 0.5f, snap(45), 1e-6f)
        assertTrue("淡出区（t=1533.3ms > 1500）< 目标值", snap(46) < 0.5f - 1e-6f)
    }

    // ═══════════════════════════════════════════════════════════
    // SG-05：多情绪（joy/sad/angry）生成不互扰（无状态泄漏）
    // ═══════════════════════════════════════════════════════════

    @Test
    fun SG05_multipleEmotions_noCrossContamination_noStateLeak() {
        val joy = mapOf("ParamBrowLY" to 0.8f, "ParamMouthForm" to 0.5f)
        val sad = mapOf("ParamBrowLY" to -0.4f, "ParamMouthOpenY" to 0.1f)
        val angry = mapOf("ParamBrowLY" to 0.9f, "ParamBrowRY" to 0.9f)

        val mJoy = MotionSynthesisGenerator.synthesizeEmotionMotion(joy, durationMs = 800L, fadeMs = 300L, sampleRateFps = 60)
        val mSad = MotionSynthesisGenerator.synthesizeEmotionMotion(sad, durationMs = 800L, fadeMs = 300L, sampleRateFps = 60)
        val mAngry = MotionSynthesisGenerator.synthesizeEmotionMotion(angry, durationMs = 800L, fadeMs = 300L, sampleRateFps = 60)

        // 各自的曲线参数名集合互不串（无上一次残留键）
        val joyNames = mJoy.curves.map { it.key }.toSet()
        assertEquals("joy 只含 joy 键", setOf("ParamBrowLY", "ParamMouthForm"), joyNames)
        val joyBrow = mJoy.curves.first { it.key == "ParamBrowLY" }
        assertEquals("joy BrowLY 平台值=0.8", 0.8f, joyBrow.endValue, 1e-6f)

        val sadNames = mSad.curves.map { it.key }.toSet()
        assertEquals("sad 只含 sad 键", setOf("ParamBrowLY", "ParamMouthOpenY"), sadNames)
        val sadBrow = mSad.curves.first { it.key == "ParamBrowLY" }
        assertEquals("sad BrowLY 平台值=-0.4（负强度不污染 0.8）", -0.4f, sadBrow.endValue, 1e-6f)

        val angryNames = mAngry.curves.map { it.key }.toSet()
        assertEquals("angry 只含 angry 键", setOf("ParamBrowLY", "ParamBrowRY"), angryNames)
        val angryBrowR = mAngry.curves.first { it.key == "ParamBrowRY" }
        assertEquals("angry BrowRY 平台值=0.9", 0.9f, angryBrowR.endValue, 1e-6f)
    }

    // ═══════════════════════════════════════════════════════════
    // SG-06：时长/采样率/fade 参数化时效一致
    // ═══════════════════════════════════════════════════════════

    @Test
    fun SG06_parameterizedDurationFadeSampleRate_consistentOnOutput() {
        val frame = mapOf("ParamBrowRY" to 0.5f)
        val m = MotionSynthesisGenerator.synthesizeEmotionMotion(frame, durationMs = 2000L, fadeMs = 500L, sampleRateFps = 30)
        assertEquals("时长==2000ms", 2000L, m.durationMs)
        assertEquals("fade==500ms", 500L, m.fadeMs)
        // 30fps×2s = 60（生产 seam：无 +1）
        assertEquals("采样数=30×2=60", 60, m.sampleCount)
        assertEquals("单键 → 曲线数=1", 1, m.curves.size)
    }

    // ═══════════════════════════════════════════════════════════
    // SG-08：非法参数名（空串）被跳过（不入曲线，防污染官方 motion 曲线集）
    // ═══════════════════════════════════════════════════════════

    @Test
    fun SG08_invalidParameterKeys_skipped_neverBecomeCurves() {
        // 生产 seam 口径：非法键 = 空串（跳过）；未知非空参数名透传（合法域校验不在本 seam）
        val frame = mapOf("ParamMouthOpenY" to 0.5f, "" to 1.0f)
        val m = MotionSynthesisGenerator.synthesizeEmotionMotion(frame, durationMs = 1000L, fadeMs = 300L, sampleRateFps = 60)
        val segNames = m.curves.map { it.key }
        assertFalse("空串参数名不得进入曲线集", segNames.contains(""))
        assertEquals("合法键唯一入曲线", listOf("ParamMouthOpenY"), segNames)
    }

    // ═══════════════════════════════════════════════════════════
    // SG-10：segmentSnapshot 边界防御（空运动 / 越界索引钳制）
    // ═══════════════════════════════════════════════════════════

    @Test
    fun SG10_snapshot_emptyMotion_emptyMap_outOfRangeClamped() {
        val empty = MotionSynthesisGenerator.synthesizeEmotionMotion(emptyMap(), durationMs = 1000L, fadeMs = 300L, sampleRateFps = 60)
        assertEquals("空运动快照 == 空 Map", emptyMap<String, Float>(), MotionSynthesisGenerator.segmentSnapshot(empty, 0))

        val m = MotionSynthesisGenerator.synthesizeEmotionMotion(mapOf("ParamMouthOpenY" to 0.8f), durationMs = 1000L, fadeMs = 300L, sampleRateFps = 60)
        // 越界索引钳制到 [0, sampleCount-1]
        assertEquals("负索引钳制到 0", MotionSynthesisGenerator.segmentSnapshot(m, 0), MotionSynthesisGenerator.segmentSnapshot(m, -5))
        assertEquals("超界索引钳制到末采样", MotionSynthesisGenerator.segmentSnapshot(m, 59), MotionSynthesisGenerator.segmentSnapshot(m, 999))
    }
}
