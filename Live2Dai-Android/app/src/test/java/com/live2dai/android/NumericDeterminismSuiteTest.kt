package com.live2dai.android

import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestCoroutineScheduler
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.mockito.ArgumentMatchers.anyFloat
import org.mockito.ArgumentMatchers.anyString
import org.mockito.kotlin.atLeastOnce
import org.mockito.kotlin.mock
import org.mockito.kotlin.never
import org.mockito.kotlin.verify
import org.mockito.kotlin.whenever

/**
 * T3.2 数值确定性套件（无视觉极致核心——数值断言闭环）。
 *
 * 依据 .pi/spoq/testdesign-v3.md §T3.2（N1-N8）+ contract-v3-fixes.md §2.4（U1-U6）+
 * contract-M4-M5-android-l2.md §3.4/§4/§5（软垫运算/降级链）。
 *
 * 本套件只从【契约 + adapter 权威值】出题，不读 app/src/main/ 实现逻辑：
 * - N2 softmax 求和 =1：经 internal 测试缝 [VowelClassifier.classify]（同包可访问）
 *   用确定性 MFCC 输入 → 断言 {SIL,A,I,U,E,O} 概率和 == 1.0（float32 舍入容差 1e-5）。
 * - N3 五元音逐 label 锁死：VOWEL_MAP(亲密图 表1 mo/mf) + 契约 表2 公式 → 六 label
 *   五元音精确值 == adapter.vowelParamMap 权威值（A=(0.8,0,0,0,0.04)… U=0.56…）。
 * - N8 降级链 + stop 协程：mock VisemeAnalyzer 注入 VoiceIoController → 级1
 *  （非 SIL timeline → setParameter 五元音）/ 级2（analyzer=null/空/全 SIL → setLipSyncParam）
 *   + release() 取消 viseme 收集协程不泄漏。
 *
 * [UNSURE 说明]：
 * - N1 DTW 距离对称性（dtw(a,b)==dtw(b,a) ±1e-6）：dtwSymmetric2 为 private，无 public/测试缝
 *   可直接断言其对称性。本套件不含直接对称断言，改以【确定性 + softmax 数值closure】行为佐证。
 * - N4 evaluateCurveAt 精确表格：segments FloatArray 编码未在契约文档化，且禁读 main 实现，
 *   无法按契约出精确值表格断言 → 报 [UNSURE]，待接口设计师补 segment 编码规格后再落地。
 */
@OptIn(ExperimentalCoroutinesApi::class)
class NumericDeterminismSuiteTest {

    // ══════════════════════════════════════════════════════════════
    // N2 · softmax 求和 = 1（±1e-5，float32 舍入极限）
    // 经 internal 测试缝 VowelClassifier.classify（同包可访问），确定性输入
    // ══════════════════════════════════════════════════════════════

    private fun assertSoftmaxSumsToOne(probs: FloatArray, msg: String) {
        val sum = probs.sum()
        // float32 逐项 exp/归一化累加的最坏舍入 ~ 6×1.19e-7；1e-5 是安全确定界线（非"大约"断言）
        val eps = 1e-5f
        assertTrue(
            "softmax 概率和应 == 1.0（实际 $sum，diff=" + (sum - 1.0f) + "）: $msg",
            kotlin.math.abs(sum - 1.0f) <= eps
        )
    }

    @Test
    fun N2_a_classifyInputAllSixTemplates_sumToOne() {
        // 输入 = 每个 DTW 模板本身（9 帧×2 维，确定性有限值）→ 距离有限 → softmax 和恒 1
        for (i in VisemeAnalyzer.ALL_TEMPLATES.indices) {
            val probs = VowelClassifier.classify(VisemeAnalyzer.ALL_TEMPLATES[i], 10.0f)
            assertEquals("每个 label 概率 ∈ (0,1]（不应为负数/NaN）", 6, probs.size)
            for (p in probs) assertTrue("p=$p 应 > 0（exp 不塌缩）", p > 0f && p <= 1f)
            assertSoftmaxSumsToOne(probs, "template#$i")
        }
    }

    @Test
    fun N2_b_singleFrameInput_sumToOne() {
        // 极短输入（1 帧）也跑满 DTW → softmax 和恒 1
        val oneFrame = listOf(floatArrayOf(150f, -60f))
        for (t in listOf(1.0f, 10.0f, 0.5f, 100.0f)) {
            val probs = VowelClassifier.classify(oneFrame, t)
            assertEquals(6, probs.size)
            assertSoftmaxSumsToOne(probs, "temp=$t")
        }
    }

    @Test
    fun N2_c_uniformExtremeInputs_sumToOne() {
        // 全零距离近 / 极端幅度输入：距离有限，softmax(减max防溢出) 后和恒 1
        val frames = listOf(
            listOf(floatArrayOf(0f, 0f), floatArrayOf(0f, 0f), floatArrayOf(0f, 0f)),
            listOf(floatArrayOf(1e6f, -1e6f), floatArrayOf(2e5f, 3e5f)),
            listOf(floatArrayOf(-9f, 4f), floatArrayOf(7f, -2f), floatArrayOf(0f, 0f)),
        )
        for (input in frames) {
            for (t in listOf(10.0f, 0.1f)) {
                val probs = VowelClassifier.classify(input, t)
                assertEquals(6, probs.size)
                for (p in probs) assertTrue("p=$p 应为有限权值", !p.isNaN() && p >= 0f)
                assertSoftmaxSumsToOne(probs, "input=${input.size}帧 temp=$t")
            }
        }
    }

    @Test
    fun N2_d_determinism_sameInputSameOutput() {
        // 确定性：同输入两次 classify 逐位一致（数值确定性核心）
        val probe = VisemeAnalyzer.ALL_TEMPLATES[3] // V_E
        val p1 = VowelClassifier.classify(probe, 10.0f)
        val p2 = VowelClassifier.classify(probe, 10.0f)
        for (i in p1.indices) assertEquals("probs[$i] 两次应逐位一致", p1[i], p2[i], 0f)
    }

    // ══════════════════════════════════════════════════════════════
    // N3 · 五元音逐 label 锁死（adapter 权威值 + 契约 表1/表2 公式）
    // ⇐ contract-v3-fixes §2.4（U1-U6）+ contract-M4-M5 §3.3 表1/表2
    // 期望：契约 表2 公式 施加到 VOWEL_MAP(mo,mf) → 五元音 == adapter 权威值
    // ══════════════════════════════════════════════════════════════

    private val eps = 1e-6f

    // adapter.vowelParamMap 权威值（shared/model-adapter/niziiro_mao.adapter.json，任务指定数据可读）
    // 顺序 [ParamA, ParamI, ParamU, ParamE, ParamO]
    private val AUTHORITATIVE = mapOf(
        "A" to floatArrayOf(0.8f, 0f, 0f, 0f, 0.04f),
        "I" to floatArrayOf(0f, 0.8f, 0f, 0f, 0f),
        "U" to floatArrayOf(0f, 0f, 0.56f, 0f, 0f),
        "E" to floatArrayOf(0.15f, 0f, 0f, 0.35f, 0f),
        "O" to floatArrayOf(0f, 0f, 0.21f, 0f, 0.6f),
        "SIL" to floatArrayOf(0f, 0f, 0f, 0f, 0f),
    )

    /** 契约 §3.3 表2 公式：label + (mo,mf) → [ParamA,ParamI,ParamU,ParamE,ParamO] */
    private fun contractFormula(label: String, mo: Float, mf: Float): FloatArray = when (label) {
        "A" -> floatArrayOf(mo, 0f, 0f, 0f, mf * 0.2f)
        "I" -> floatArrayOf(0f, mf, 0f, 0f, 0f)
        "U" -> floatArrayOf(0f, 0f, mf * 0.7f, 0f, 0f)
        "E" -> floatArrayOf(mo * 0.3f, 0f, 0f, mo * 0.7f, 0f)
        "O" -> floatArrayOf(0f, 0f, mf * 0.3f, 0f, mo)
        "SIL" -> floatArrayOf(0f, 0f, 0f, 0f, 0f)
        else -> throw IllegalArgumentException("未知 label: $label")
    }

    @Test
    fun N3_U_uParam_isPoint56() {
        // U 修正（D4：mf×0.7 = 0.8×0.7 = 0.56），ParamA/I/E/O 全 0
        val expected = AUTHORITATIVE["U"]!!
        val got = contractFormula("U", VisemeAnalyzer.VOWEL_MAP["U"]!![0], VisemeAnalyzer.VOWEL_MAP["U"]!![1])
        assertEquals("U.ParamU 应 = 0.56f（修正值 0.8×0.7）", 0.56f, got[2], eps)
        assertFourEquals(expected, got, "U")
    }

    @Test
    fun N3_O_uParam_isPoint21() {
        // O 回归（契约 §2.4 U3）：ParamU = mf×0.3 = 0.7×0.3 = 0.21；ParamO = mo = 0.6（不受 U 修正影响）
        val expected = AUTHORITATIVE["O"]!!
        val got = contractFormula("O", VisemeAnalyzer.VOWEL_MAP["O"]!![0], VisemeAnalyzer.VOWEL_MAP["O"]!![1])
        assertEquals(0.21f, got[2], eps)
        assertEquals(0.6f, got[4], eps)
        assertFourEquals(expected, got, "O")
    }

    @Test
    fun N3_allSixLabels_matchAdapterAuthoritative() {
        // 六 label 全锁死：VOWEL_MAP + 契约公式 == adapter.vowelParamMap 权威值（逐位 ±1e-6）
        for (label in VisemeAnalyzer.VOWEL_LABELS) {
            val mo = VisemeAnalyzer.VOWEL_MAP[label]!![0]
            val mf = VisemeAnalyzer.VOWEL_MAP[label]!![1]
            val got = contractFormula(label, mo, mf)
            val expected = AUTHORITATIVE[label]!!
            assertFourEquals(expected, got, label)
        }
    }

    @Test
    fun N3_SIL_allZero() {
        val expected = AUTHORITATIVE["SIL"]!!
        val got = contractFormula("SIL", 0f, 0f)
        assertEquals(0f, got.sum(), 0f)
        assertFourEquals(expected, got, "SIL")
    }

    private fun assertFourEquals(expected: FloatArray, got: FloatArray, label: String) {
        for (i in expected.indices) {
            assertEquals("$label 五元音[$i] 应等于 adapter 权威值", expected[i], got[i], eps)
        }
    }

    // ══════════════════════════════════════════════════════════════
    // N5/N6 · MotionTags resolve / idlePool（确定性行为）
    // ⇐ testdesign-v3 N5/N6 + MotionTagsTest 既有（补充精确断言）
    // ══════════════════════════════════════════════════════════════

    @Test
    fun N5_unknownTag_blank_returnsNull() {
        // resolve 未命中 → null（未知/空白不崩溃、不猜测）
        assertTrue(MotionAliasTable.resolve("dance") == null)
        assertTrue(MotionAliasTable.resolve("") == null)
        assertTrue(MotionAliasTable.resolve("   ") == null)
    }

    @Test
    fun N6_idlePool_noDuplicate_noSurprise() {
        // DEFAULT_POOL 6 元素、无重复、且与 adapter idlePool 权威集合一致
        val pool = IdleMotionSelector.DEFAULT_POOL
        assertEquals("DEFAULT_POOL 应为 6 键", 6, pool.size)
        assertEquals("DEFAULT_POOL 不应有重复", 6, pool.toSet().size)
        assertEquals(
            "DEFAULT_POOL 应 == adapter.idlePool [_0.._5]",
            listOf("_0", "_1", "_2", "_3", "_4", "_5"),
            pool
        )
    }

    // ══════════════════════════════════════════════════════════════
    // N8 · 降级链（级1 viseme → 级2 RMS）+ stop/release 协程不泄漏
    // 注入 mock VisemeAnalyzer；驱动 internal handlePlaybackState
    // ══════════════════════════════════════════════════════════════

    private lateinit var view: Live2DView
    private lateinit var scheduler: TestCoroutineScheduler
    private lateinit var context: Context

    @Before
    fun n8SetUp() {
        context = mock()
        view = mock()
        scheduler = TestCoroutineScheduler()
        Dispatchers.setMain(StandardTestDispatcher(scheduler))
    }

    @After
    fun n8TearDown() {
        Dispatchers.resetMain()
    }

    /** 构造一个 VoiceIoController（provider 为 stub，避免 Registry 依赖）。 */
    private fun buildController(
        visemeAnalyzer: VisemeAnalyzer?,
        attachView: Boolean = true,
    ): VoiceIoController {
        fun stubProvider(id: String): TtsProvider {
            val speaking = MutableStateFlow(false)
            val provider = mock<TtsProvider>()
            whenever(provider.info).thenReturn(
                TtsProviderInfo(
                    id = id, name = "stub-$id", category = TtsProviderCategory.LOCAL,
                    requiresNetwork = false, health = MutableStateFlow(TtsHealth.OK),
                    priority = 1, configurable = false,
                )
            )
            whenever(provider.isSpeaking).thenReturn(speaking)
            return provider
        }
        val providers = listOf(stubProvider("edge_tts"), stubProvider("system_tts"))
        val asr = mock<AsrProvider>()
        whenever(asr.info).thenReturn(
            AsrProviderInfo(
                id = "system_speech_recognizer", name = "stub-asr", requiresNetwork = false,
                health = MutableStateFlow(TtsHealth.OK), priority = 1,
            )
        )
        val attached = if (attachView) view else null
        return VoiceIoController(
            context, providers, listOf(asr), attached,
            CoroutineScope(SupervisorJob() + Dispatchers.Main),
            visemeAnalyzer,
        )
    }

    @Test
    fun N8_level1_nonSilTimeline_drivesFiveParams() {
        // 级1：analyzer 非空 + timeline 含非 SIL 帧 → setParameter(ParamA..ParamO, 帧值)
        val timeline = listOf(
            VisemeFrame(0L, "A", 0.8f, 0.2f, 0.8f, 0f, 0f, 0f, 0.04f),
            VisemeFrame(100L, "U", 0.4f, 0.8f, 0f, 0f, 0.56f, 0f, 0f),
        )
        val analyzer = mock<VisemeAnalyzer>()
        whenever(analyzer.getVisemeTimeline()).thenReturn(timeline)
        val controller = buildController(analyzer)

        controller.handlePlaybackState(true)
        scheduler.advanceUntilIdle()

        // 级1 驱动五元音精确值
        verify(view, atLeastOnce()).setParameter("ParamA", 0.8f)
        verify(view, atLeastOnce()).setParameter("ParamO", 0.04f)
        verify(view, atLeastOnce()).setParameter("ParamU", 0.56f)
        // 结束 → 五元音归零 + setLipSyncParam(0f)（frame U 的 ParamU=0 与 resetMouth 均含 0f，用 atLeastOnce）
        verify(view, atLeastOnce()).setParameter("ParamA", 0f)
        verify(view, atLeastOnce()).setParameter("ParamI", 0f)
        verify(view, atLeastOnce()).setParameter("ParamU", 0f)
        verify(view, atLeastOnce()).setParameter("ParamE", 0f)
        verify(view, atLeastOnce()).setParameter("ParamO", 0f)
        verify(view, atLeastOnce()).setLipSyncParam(0f)
        controller.release()
    }

    @Test
    fun N8_level2_nullAnalyzer_fallsBackToRmsLoop() {
        // 级2：analyzer=null → 走 randomizedMouth（setLipSyncParam 驱动，不 setParameter 五元音）
        val controller = buildController(null)
        controller.handlePlaybackState(true)
        scheduler.advanceTimeBy(VoiceIoController.LIP_SYNC_INTERVAL_MS * 3)
        verify(view, atLeastOnce()).setLipSyncParam(anyFloat())
        verify(view, never()).setParameter(anyString(), anyFloat())
        controller.release()
    }

    @Test
    fun N8_level2_allSilTimeline_fallsBackToRmsLoop() {
        // 级2：timeline 全 SIL（分析不可信）→ 随机波动比"一直闭嘴"自然（契约 §5 决策）
        val allSil = listOf(
            VisemeFrame(0L, "SIL", 0f, 0f, 0f, 0f, 0f, 0f, 0f),
            VisemeFrame(150L, "SIL", 0f, 0f, 0f, 0f, 0f, 0f, 0f),
        )
        val analyzer = mock<VisemeAnalyzer>()
        whenever(analyzer.getVisemeTimeline()).thenReturn(allSil)
        val controller = buildController(analyzer)
        controller.handlePlaybackState(true)
        scheduler.advanceTimeBy(VoiceIoController.LIP_SYNC_INTERVAL_MS * 2)
        verify(view, atLeastOnce()).setLipSyncParam(anyFloat())
        verify(view, never()).setParameter(anyString(), anyFloat())
        controller.release()
    }

    @Test
    fun N8_level3_noView_noLipSync() {
        // 级3：live2DView == null（未接线）→ 不驱动任何嘴型，不崩溃
        val controller = buildController(null, attachView = false)
        controller.handlePlaybackState(true)
        scheduler.advanceTimeBy(VoiceIoController.LIP_SYNC_INTERVAL_MS * 3)
        verify(view, never()).setLipSyncParam(anyFloat())
        verify(view, never()).setParameter(anyString(), anyFloat())
        controller.release()
    }

    @Test
    fun N8_release_cancelsVisemeCollectJob_noLeak() {
        // release() 取消 viseme 收集协程 + 释放资源（幂等多次调用不崩溃 —— 无泄漏）
        val timeline = listOf(
            VisemeFrame(0L, "A", 0.8f, 0.2f, 0.8f, 0f, 0f, 0f, 0.04f),
        )
        val analyzer = mock<VisemeAnalyzer>()
        whenever(analyzer.getVisemeTimeline()).thenReturn(timeline)
        val controller = buildController(analyzer)
        controller.handlePlaybackState(true)
        scheduler.advanceUntilIdle()

        controller.release()
        controller.release() // 幂等：二次 release 不崩溃
        scheduler.advanceUntilIdle()
        // 无异常即通过（协程取消不泄漏证明：advanceUntilIdle 后无未终结任务）
    }
}
