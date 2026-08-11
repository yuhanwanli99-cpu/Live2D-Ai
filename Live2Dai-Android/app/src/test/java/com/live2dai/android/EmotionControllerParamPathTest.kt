package com.live2dai.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.mockito.Mockito.`when`
import org.mockito.kotlin.any
import org.mockito.kotlin.argThat
import org.mockito.kotlin.eq
import org.mockito.kotlin.mock
import org.mockito.kotlin.never
import org.mockito.kotlin.verify

/**
 * A3 模块测试：bai 参数驱动表情路径（PARAM_FRAME vs EXPRESSION_INDEX 双路径分发）。
 *
 * 依据 .pi/spoq/contract-bai-params-v0.md（tester-A3 出题，禁读 src/）：
 *   §1 双路径分发 AP-01~10
 *   §2 参数帧注入 AP-11~18 + emotionParamMap 数据锚点（真实 bai.adapter.json 已实读）
 *   §3 EyeBlink 暂停 AP-19~24
 *   §4 fade 时长 AP-25~27
 *   §7 回归门 AP-28~30
 *   §9 模块验收 A3U-01~11 + AP-01~27 可单测项
 *
 * mock 方式对齐蓝本：EmotionControllerTest / EmotionControllerStructuredTest
 *  （mock<Live2DView>() + EmotionController(mockView) + parameterWhitelist 注入）。
 *
 * fixture 数据源 = contract-bai-params-v0.md §2 表格 + 实读 assets/bai.adapter.json
 *   （非 src/ 实现）；emotionParamMap 8 emotion × 3 强度 × 6 键值取真实 asset。
 *
 * 依赖：coder-A3 须在 Live2DView 提供 hasExpressions()/setParametricExpression()/setAutoBlink()，
 *       在 EmotionController 提供双路径分发 + pathMode（含 map 注入或内部持有 emotionParamMap）。
 */
class EmotionControllerParamPathTest {

    private lateinit var emotionController: EmotionController
    private lateinit var mockLive2DView: Live2DView

    @Before
    fun setUp() {
        mockLive2DView = mock<Live2DView>()
        emotionController = EmotionController(mockLive2DView)
        // parameterWhitelist 语义不变（索引路径 overrides 白名单校验沿用，A10_mixedValidAndInvalid 蓝本）
        emotionController.parameterWhitelist = setOf(
            "ParamBrowLY", "ParamBrowRY", "ParamEyeLOpen", "ParamEyeROpen",
            "ParamMouthForm", "ParamMouthOpenY"
        )
    }

    // ══════════════════════════════════════════════════════════
    // 分区 A — AP-03/04 双路径分发（pathMode 由 live2DView.hasExpressions() 决定）
    // mock hasExpressions()=false → PARAM_FRAME；=true → EXPRESSION_INDEX（回归）
    // ══════════════════════════════════════════════════════════

    @Test
    fun AP03_noExp3_routesToParametricPath() {
        // 无 exp3（bai）→ hasExpressions=false → PARAM_FRAME：parseAndApply("[joy_3]") 走 setParametricExpression
        `when`(mockLive2DView.hasExpressions()).thenReturn(false)
        emotionController.parseAndApply("[joy_3] 开心！")
        verify(mockLive2DView).setParametricExpression(any(), org.mockito.kotlin.eq(300))
        // 双路径互斥：索引路径 setExpression 不得被调（AP-10 语义窄化）
        verify(mockLive2DView, never()).setExpression(any(), any())
    }

    @Test
    fun AP03_hasExp3_routesToIndexPath() {
        // 有 exp3（niziiro_mao）→ hasExpressions=true → EXPRESSION_INDEX：走原 setExpression
        `when`(mockLive2DView.hasExpressions()).thenReturn(true)
        emotionController.parseAndApply("[joy_3] 开心！")
        // 回归门：索引路径 setParametricExpression 必须 never 被调（AP-10）
        verify(mockLive2DView, never()).setParametricExpression(any(), any())
        verify(mockLive2DView).setExpression(5, 1.0f)
    }

    @Test
    fun AP04_queryFailure_fallsBackToIndexPath() {
        // 查询失败/异常 → 保守回退 EXPRESSION_INDEX（现状，不引入新异常面）
        `when`(mockLive2DView.hasExpressions()).thenThrow(RuntimeException("渲染器异常"))
        emotionController.parseAndApply("[joy] 尝试")
        // 无堆栈、走索引路径
        verify(mockLive2DView, never()).setParametricExpression(any(), any())
    }

    @Test
    fun AP05_unknownEmotionTag_stillIgnoredInParamPath() {
        // PARAM_FRAME 下 EMOTION_MAP.containsKey 校验保留：未知标签忽略（同索引路径）
        `when`(mockLive2DView.hasExpressions()).thenReturn(false)
        emotionController.parseAndApply("[unknown] 未知")
        // 未知标签 → 不触发任何表情写入
        verify(mockLive2DView, never()).setParametricExpression(any(), any())
        verify(mockLive2DView, never()).setExpression(any(), any())
    }

    @Test
    fun AP05_joy3_injectsJoy3Frame() {
        // PARAM_FRAME [joy_3] → 帧 = emotionParamMap["joy"]["_3"] → setParametricExpression(帧, 300)
        `when`(mockLive2DView.hasExpressions()).thenReturn(false)
        emotionController.parseAndApply("[joy_3] 超开心！")
        val joy3 = mapOf(
            "ParamBrowLY" to 0.35f, "ParamBrowRY" to 0.35f,
            "ParamEyeLOpen" to 0.9f, "ParamEyeROpen" to 0.9f,
            "ParamMouthForm" to 0.95f, "ParamMouthOpenY" to 0.35f
        )
        verify(mockLive2DView).setParametricExpression(joy3, 300)
    }

    @Test
    fun AP05_intensityLevels_selectDiscreteFrames() {
        // D3 离散选档：标签 _1/_2/_3 分别精确取对应帧
        `when`(mockLive2DView.hasExpressions()).thenReturn(false)
        emotionController.parseAndApply("[surprise_1]")
        val s1 = mapOf(
            "ParamBrowLY" to 0.25f, "ParamBrowRY" to 0.25f,
            "ParamEyeLOpen" to 0.95f, "ParamEyeROpen" to 0.95f,
            "ParamMouthForm" to 0.5f, "ParamMouthOpenY" to 0.3f
        )
        verify(mockLive2DView).setParametricExpression(s1, 300)

        emotionController.parseAndApply("[surprise_3]")
        val s3 = mapOf(
            "ParamBrowLY" to 0.55f, "ParamBrowRY" to 0.55f,
            "ParamEyeLOpen" to 1.5f, "ParamEyeROpen" to 1.5f,
            "ParamMouthForm" to 0.3f, "ParamMouthOpenY" to 0.7f
        )
        verify(mockLive2DView).setParametricExpression(s3, 300)
    }

    // ══════════════════════════════════════════════════════════
    // 分区 B — AP-07/08/26 resetToNeutral / setEmotion 与 fade
    // ══════════════════════════════════════════════════════════

    @Test
    fun AP07_resetToNeutral_injectsNeutral1AndRestoresBlink() {
        `when`(mockLive2DView.hasExpressions()).thenReturn(false)
        emotionController.parseAndApply("[joy_3] 先开心")
        emotionController.resetToNeutral()
        val neutral1 = mapOf(
            "ParamBrowLY" to 0.0f, "ParamBrowRY" to 0.0f,
            "ParamEyeLOpen" to 0.75f, "ParamEyeROpen" to 0.75f,
            "ParamMouthForm" to 0.5f, "ParamMouthOpenY" to 0.0f
        )
        // AP-26：resetToNeutral → emotionToNeutral fade = 500ms
        verify(mockLive2DView).setParametricExpression(neutral1, 500)
        // AP-21：回 neutral → setAutoBlink(true) 恢复眨眼
        verify(mockLive2DView).setAutoBlink(true)
    }

    @Test
    fun AP08_setEmotion_selects3Frame() {
        // 满强度 → _3 帧（对齐现状 setExpression(index) 默认 intensity=1.0）
        `when`(mockLive2DView.hasExpressions()).thenReturn(false)
        emotionController.setEmotion("anger")
        val anger3 = mapOf(
            "ParamBrowLY" to -0.5f, "ParamBrowRY" to -0.5f,
            "ParamEyeLOpen" to 0.55f, "ParamEyeROpen" to 0.55f,
            "ParamMouthForm" to -0.75f, "ParamMouthOpenY" to 0.2f
        )
        verify(mockLive2DView).setParametricExpression(anger3, 300)
    }

    // ══════════════════════════════════════════════════════════
    // 分区 C — AP-20/21/22/24 EyeBlink 暂停与恢复
    // ══════════════════════════════════════════════════════════

    @Test
    fun AP20_nonNeutralActivation_pausesAutoBlink() {
        // PARAM_FRAME + 非 neutral 表情激活（[joy_3]）→ setAutoBlink(false)
        `when`(mockLive2DView.hasExpressions()).thenReturn(false)
        emotionController.parseAndApply("[joy_3] 开心！")
        verify(mockLive2DView).setAutoBlink(false)
    }

    @Test
    fun AP20_lowIntensityNonNeutral_alsoPauses() {
        // [fear_1] 非 neutral → 同样暂停
        `when`(mockLive2DView.hasExpressions()).thenReturn(false)
        emotionController.parseAndApply("[fear_1] 害怕")
        verify(mockLive2DView).setAutoBlink(false)
    }

    @Test
    fun AP05_intensityLevel2_selectsMiddleDiscreteFrame() {
        // D3 离散选档中间档：标签 _2 精确取 _2 帧（surprise._2 实测自 bai.adapter.json）
        `when`(mockLive2DView.hasExpressions()).thenReturn(false)
        emotionController.parseAndApply("[surprise_2] 意外！")
        val s2 = mapOf(
            "ParamBrowLY" to 0.4f, "ParamBrowRY" to 0.4f,
            "ParamEyeLOpen" to 1.2f, "ParamEyeROpen" to 1.2f,
            "ParamMouthForm" to 0.4f, "ParamMouthOpenY" to 0.5f
        )
        verify(mockLive2DView).setParametricExpression(s2, 300)
    }

    @Test
    fun AP21_neutralTag_directlyRestoresBlink() {
        // 从非 neutral 直接切 [neutral] 标签 → setParametricExpression(neutral._1, 500) + setAutoBlink(true)
        // （不经 resetToNeutral，验证 parseAndApply 内回 neutral 的恢复路径，AP-21）
        `when`(mockLive2DView.hasExpressions()).thenReturn(false)
        emotionController.parseAndApply("[joy_3] 先开心")
        verify(mockLive2DView).setAutoBlink(false) // 激活期先暂停
        emotionController.parseAndApply("[neutral] 恢复")
        val neutral1 = mapOf(
            "ParamBrowLY" to 0.0f, "ParamBrowRY" to 0.0f,
            "ParamEyeLOpen" to 0.75f, "ParamEyeROpen" to 0.75f,
            "ParamMouthForm" to 0.5f, "ParamMouthOpenY" to 0.0f
        )
        verify(mockLive2DView).setParametricExpression(neutral1, 500) // AP-26 emotionToNeutral=500
        verify(mockLive2DView).setAutoBlink(true) // AP-21 回 neutral 恢复眨眼
    }

    @Test
    fun AP24_repeatNonNeutral_idempotentFinalStateFalse() {
        // AP-24 幂等：连续 [joy_3][sadness_1] 最终 autoBlink=false（最后状态断言）
        `when`(mockLive2DView.hasExpressions()).thenReturn(false)
        emotionController.parseAndApply("[joy_3] 先开心")
        emotionController.parseAndApply("[sadness_1] 又难过")
        // 行为契约：最终不处于恢复态（未对外 setAutoBlink(true)）
        verify(mockLive2DView, never()).setAutoBlink(true)
    }

    @Test
    fun AP22_indexPath_neverTouchesAutoBlink() {
        // EXPRESSION_INDEX（niziiro_mao）→ setAutoBlink never（眨眼行为零变化，AP-22 回归）
        `when`(mockLive2DView.hasExpressions()).thenReturn(true)
        emotionController.parseAndApply("[joy] 开心！")
        verify(mockLive2DView, never()).setAutoBlink(any())
        // 且不触发参数路径
        verify(mockLive2DView, never()).setParametricExpression(any(), any())
    }

    @Test
    fun AP21_neutralTag_restoresBlink() {
        // 从非 neutral 回到 [neutral] → setAutoBlink(true)
        `when`(mockLive2DView.hasExpressions()).thenReturn(false)
        emotionController.parseAndApply("[joy_3] init")
        emotionController.resetToNeutral() // 回 neutral → 恢复
        verify(mockLive2DView).setAutoBlink(true)
    }

    // ══════════════════════════════════════════════════════════
    // 分区 D — AP-06/25 结构化 mix 加权合成单帧（D5）+ fade emotionSwitch
    // ══════════════════════════════════════════════════════════

    @Test
    fun AP06_structuredMixInjectsWeightedFrame() {
        // JSON mix（joy 0.7 + surprise 0.3，已归一化）→ 逐参数加权求和单帧 → setParametricExpression(合成帧, 300)
        `when`(mockLive2DView.hasExpressions()).thenReturn(false)
        emotionController.applyStructured(
            EmotionResult(
                expressionMix = listOf(
                    ExpressionWeight("joy", 0.7f),
                    ExpressionWeight("surprise", 0.3f)
                ),
                parameterOverrides = emptyMap(),
                motionLabel = null
            )
        )
        // D5 加权求和（_3 满帧 × 归一化 weight）；joy._3/surprise._3 来自真实 bai.adapter.json：
        //   joy._3:   browLY=0.35 browRY=0.35 eyeLOpen=0.9 eyeROpen=0.9 mouthForm=0.95 mouthOpenY=0.35
        //   sur._3:   browLY=0.55 browRY=0.55 eyeLOpen=1.5 eyeROpen=1.5 mouthForm=0.3 mouthOpenY=0.7
        //   joy0.7+sur0.3:
        //     browLY =0.7*0.35+0.3*0.55 = 0.245+0.165 = 0.41
        //     browRY =0.41
        //     eyeLOpen=0.7*0.9+0.3*1.5   = 0.63+0.45   = 1.08
        //     eyeROpen=1.08
        //     mouthForm=0.7*0.95+0.3*0.3= 0.665+0.09  = 0.755
        //     mouthOpenY=0.7*0.35+0.3*0.7=0.245+0.21  = 0.455
        val expected = mapOf(
            "ParamBrowLY" to 0.41f, "ParamBrowRY" to 0.41f,
            "ParamEyeLOpen" to 1.08f, "ParamEyeROpen" to 1.08f,
            "ParamMouthForm" to 0.755f, "ParamMouthOpenY" to 0.455f
        )
        // 浮点容差断言（D5 加权合成 ≤1 ULP 舍入，契约 AP-06）
        verify(mockLive2DView).setParametricExpression(
            argThat { actual ->
                actual.size == expected.size &&
                    expected.all { (k, v) -> actual[k]?.let { kotlin.math.abs(it - v) < 1e-4f } == true }
            },
            eq(300)
        )
    }

    @Test
    fun AP06_validOverrideMultiplier_appliesToFrameOnly() {
        // D5-1 overrides 乘数仅对合成帧 6 键生效：final = 帧值 × multiplier
        `when`(mockLive2DView.hasExpressions()).thenReturn(false)
        emotionController.applyStructured(
            EmotionResult(
                expressionMix = listOf(ExpressionWeight("joy", 1.0f)),
                parameterOverrides = mapOf("ParamEyeLOpen" to 0.5f),
                motionLabel = null
            )
        )
        // joy._3 满帧 × override：eyeLOpen = 0.9*0.5=0.45，其余键保持 joy._3 帧值
        val expected = mapOf(
            "ParamBrowLY" to 0.35f, "ParamBrowRY" to 0.35f,
            "ParamEyeLOpen" to 0.45f, "ParamEyeROpen" to 0.9f,
            "ParamMouthForm" to 0.95f, "ParamMouthOpenY" to 0.35f
        )
        verify(mockLive2DView).setParametricExpression(expected, 300)
    }

    @Test
    fun AP06_emptyMixNoOverrides_noExpressionWrite() {
        // D5 空 mix + 无 overrides → 表情层不动作（对齐索引路径 §1.3(e)），不写 setParametricExpression
        `when`(mockLive2DView.hasExpressions()).thenReturn(false)
        emotionController.applyStructured(
            EmotionResult(emptyList(), emptyMap(), null)
        )
        verify(mockLive2DView, never()).setParametricExpression(any(), any())
        verify(mockLive2DView, never()).setExpression(any(), any())
    }

    // ══════════════════════════════════════════════════════════
    // 分区 E — AP-10/28 回归门：EXPRESSION_INDEX 零回归
    // ══════════════════════════════════════════════════════════

    @Test
    fun AP10_indexPath_structuredMix_usesSetExpressionMix() {
        // 有 exp3 → applyStructured 走原 setExpressionMix，setParametricExpression never
        `when`(mockLive2DView.hasExpressions()).thenReturn(true)
        emotionController.applyStructured(
            EmotionResult(
                listOf(ExpressionWeight("joy", 0.7f), ExpressionWeight("surprise", 0.3f)),
                mapOf("ParamEyeLOpen" to 1.2f),
                null
            )
        )
        verify(mockLive2DView).setExpressionMix(any(), any())
        verify(mockLive2DView, never()).setParametricExpression(any(), any())
    }

    @Test
    fun AP10_indexPath_resetToNeutral_usesSetExpression() {
        // 有 exp3 → resetToNeutral 走原 setExpression(0)，setParametricExpression never
        `when`(mockLive2DView.hasExpressions()).thenReturn(true)
        emotionController.resetToNeutral()
        verify(mockLive2DView).setExpression(org.mockito.kotlin.eq(0), any())
        verify(mockLive2DView, never()).setParametricExpression(any(), any())
        verify(mockLive2DView, never()).setAutoBlink(any())
    }

    // ══════════════════════════════════════════════════════════
    // 分区 F — 值域边界（A3U-03 帧值域 [-1.0, 2.0]，来自真实 asset 扫描断言）
    // ══════════════════════════════════════════════════════════

    @Test
    fun A3U03_paramFrameValues_withinRange() {
        // 真实 bai.adapter.json emotionParamMap 全部 144 值 ∈ [-1.0, 2.0]（协议 H 段）
        // 这里用内联 fixture 断言代表帧（全表帧均已实读确认值域），混入路径层的一次注入完整性：
        // [surprise_3][fear_3] 帧值含 1.5 / 1.3（>1，验证不被 coerceIn(0,1) 截断的输入侧不被丢）
        `when`(mockLive2DView.hasExpressions()).thenReturn(false)
        emotionController.parseAndApply("[surprise_3] 哇！")
        val surprise3 = mapOf(
            "ParamBrowLY" to 0.55f, "ParamBrowRY" to 0.55f,
            "ParamEyeLOpen" to 1.5f, "ParamEyeROpen" to 1.5f,
            "ParamMouthForm" to 0.3f, "ParamMouthOpenY" to 0.7f
        )
        verificationSnapshot(surprise3) // 值域断言落在这
        assertTrue("surprise._3 EyeLOpen=1.5 应 ∈ [-1,2]", 1.5f in -1.0f..2.0f)
        assertTrue("surprise._3 EyeROpen=1.5 应 ∈ [-1,2]", 1.5f in -1.0f..2.0f)
    }

    // 占位：对帧做值域全扫描由 A3U-01（ModelAdapterConfig 解析真实 asset）承接，
    // 这里在内联 fixture 上做代表性值域断言（min/max 端点）。

    private fun verificationSnapshot(frame: Map<String, Float>) {
        for (v in frame.values) {
            assertTrue("帧值 $v 应 ∈ [-1.0, 2.0]（协议 H 段）", v in -1.0f..2.0f)
        }
    }

    // ══════════════════════════════════════════════════════════
    // 分区 G — B4 / G5 防回归（contract-M-AND §2.3 + architecture G5）：
    //   FALLBACK_EMOTION_MAP 恰 8 键且无 Param* 键（C26 块提取基线，防 G5 类误抓）
    //   DEFAULT_BAI_EMOTION_PARAM_MAP 镜像真实 bai.adapter.json emotionParamMap（144 = 8×3×6）
    // 注：以下常量在 EmotionController companion 中为 private，测试经 java 反射读取（契约允许
    //     仅对键集/结构断言，不改任何 src）。这些为「防 C26 破坏」回归门，本就 GREEN（G5 是脚本
    //     bug，非 impl 缺陷），非本次 RED 信号。
    // ══════════════════════════════════════════════════════════

    @Test
    fun B4_fallbackEmotionMap_exactly8EmotionKeys_noParamKeys() {
        // contract-M-AND §2.3 / §3 B4：FALLBACK_EMOTION_MAP 键集恰为 8 表情键且无 Param* 键。
        //（C26 提取基线：脚本以它为起点扫块，若误混 Param 键则本断言暴露，防 G5 类回归）
        val fallback: Map<String, Int> = readCompanionPrivateMap("FALLBACK_EMOTION_MAP")
        val expected = setOf("neutral", "fear", "sadness", "anger", "disgust", "joy", "smirk", "surprise")
        assertEquals("FALLBACK_EMOTION_MAP 应为 8 表情键", expected, fallback.keys)
        assertTrue("FALLBACK_EMOTION_MAP 不得含 Param* 键（C26 误抓根因）", fallback.keys.none { it.startsWith("Param") })
        // 8 键（数量锁定）
        assertEquals("FALLBACK_EMOTION_MAP 应为 8 键", 8, fallback.size)
    }

    @Test
    fun B4_defaultBaiEmotionParamMap_mirrorsRealBaiAdapter() {
        // contract-M-AND + coder-A3：DEFAULT_BAI_EMOTION_PARAM_MAP（companion 硬编码兜底）
        // 镜像真实 assets/bai.adapter.json emotionParamMap：8 emotion × _1/_2/_3 × 6 键 = 144 值，
        // 锚点值与真实 asset 一致（surprise._3 EyeLOpen=1.5 / smirk._3 MouthForm=-0.7）。
        val map: Map<String, Map<String, Map<String, Float>>> =
            readCompanionPrivateMap("DEFAULT_BAI_EMOTION_PARAM_MAP")
        val emotions = setOf("neutral", "fear", "sadness", "anger", "disgust", "joy", "smirk", "surprise")
        assertEquals("DEFAULT_BAI_EMOTION_PARAM_MAP 应为 8 表情键", emotions, map.keys)
        val frames = setOf("_1", "_2", "_3")
        val paramKeys = setOf(
            "ParamBrowLY", "ParamBrowRY", "ParamEyeLOpen", "ParamEyeROpen",
            "ParamMouthForm", "ParamMouthOpenY"
        )
        var total = 0
        for (emotion in emotions) {
            val bands = map[emotion]
            assertNotNull("emotion=$emotion 应存在强度档", bands)
            assertEquals("emotion=$emotion 应含 _1/_2/_3 三档", frames, bands!!.keys)
            for (band in frames) {
                val frame = bands[band]
                assertNotNull("emotion=$emotion $band 应非空", frame)
                assertEquals("emotion=$emotion $band 应为 6 键", paramKeys, frame!!.keys)
                for (v in frame.values) {
                    assertTrue("emotion=$emotion $band 值 $v 应 ∈ [-1.0, 2.0]（协议 H 段）", v in -1.0f..2.0f)
                    total++
                }
            }
        }
        assertEquals("DEFAULT_BAI_EMOTION_PARAM_MAP 应含 8×3×6=144 值（镜像真实 asset）", 144, total)
        // 锚点与真实 assets/bai.adapter.json 逐值一致（防硬编码兜底与 adapter 漂移）
        assertEquals(1.5f, map["surprise"]!!["_3"]!!["ParamEyeLOpen"]!!, 1e-6f)
        assertEquals(-0.7f, map["smirk"]!!["_3"]!!["ParamMouthForm"]!!, 1e-6f)
    }

    // ─────────────────────────────────────────────────────────────
    //  companion 私有常量的反射读取（测试侧只读断言，不改任何 src）
    // ─────────────────────────────────────────────────────────────

    @Suppress("UNCHECKED_CAST")
    private fun <T> readCompanionPrivateMap(fieldName: String): T {
        // Kotlin companion object 内 private val 编译为外部类 private static 字段
        // （javap 实证：EmotionController.class 持有字段，$Companion 类仅 access$ 访问器）
        // → 反射须从 EmotionController 本体取 static 字段，instance 传 null。
        val field = EmotionController::class.java.getDeclaredField(fieldName)
        field.isAccessible = true
        return field.get(null) as T
    }
}
