package com.live2d.ai.android.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * R13 表情参数帧纯函数断言（程序化表情强化：8 情绪 × 3 强度 → 三轴参数值）。
 *
 * 依据（唯一真相源）：
 *  - .pi/spoq/search-report-expression.md §3（官方表情机制 / 参数差叠加语义 / 眼神不动情绪帧 / MouthOpenY 表情帧=0 不挡口型）
 *  - .pi/spoq/model-facts-bai.md §四（bai 只有 BrowLY/RY 眉高、EyeLOpen/ROpen 眼开合、MouthForm/MouthOpenY 嘴——无 EyeBall 于情绪表）
 *  - bai.adapter.json emotionParamMap（= .pi/spoq/contract-bai-params-v0.md A3U-01/A3U-03 的唯一数据锚点，**非 src/ 实现**）
 *  - .pi/spoq/contract-bai-params-v0.md §2（A3U-01 结构 8×3×6=144 值；A3U-03 值域实测 max=1.5 / min=-0.7）
 *
 * 封装的待实现生产符号（**均不存在，编译期 RED 属预期**，cil=coder 待实现）：
 *   object ExpressionFrames {
 *       const val BROW_MIN = -1f; BROW_MAX = 1f           // 官方标准参数表 [-1,1]
 *       const val EYE_OPEN_MIN = 0f; EYE_OPEN_MAX = 1.5f   // 官方眼开合 [0,1] 可放宽 1.5（fear/surprise clamp 面）
 *       const val MOUTH_FORM_MIN = -1f; MOUTH_FORM_MAX = 1f
 *       const val MOUTH_OPEN_MAX = 1.5f                    // 官方 MouthOpenY [0,1] 可到 2；bai 情绪帧实测 ≤0.7
 *   }
 *   /** 按 8 情绪 × 3 强度抽出单帧（6 键参数表）。 */
 *   fun frameOf(emotionParamMap: Map<String, Map<String, Map<String, Float>>>,
 *               emotion: String, level: Int): Map<String, Float>?
 *   /** 三轴值域校验：眉 [-1,1]、眼 [0,1.5]、嘴形 [-1,1]、嘴开合 [0,1.5]；越界或含 EyeBall 键 → false。 */
 *   fun isValidEmotionFrame(frame: Map<String, Float>): Boolean
 *
 * 设计说明（禁读 src/ 实现）：
 *   - 本测试锁定的数据源为 bai.adapter.json 的 emotionParamMap（144 值）——R13 模块须按此表实现 frameOf 映射，
 *     **不是** 我凭空编造。测试以下列真值做「结构性 + 值域 + 语义」锁定，而非锁死每个具体 float（那属 A3 契约，非本模块）。
 *   - 三轴类别划分、眼神不动情绪帧 —— 直接来自 search-report §3.2 官方机制与 model-facts-bai §四（bai 情绪表本就无 EyeBall 键）。
 *
 * [CONFLICT / UNSURE 标注]：
 *   - 任务项 2 要求「MouthOpenY 表情帧 = 0 不挡口型」。但 **bai 真实 emotionParamMap 中 7/8 情绪的 MouthOpenY 非零**
 *     （joy._3=0.35 / surprise._3=0.7 / anger._3=0.2 …），仅 neutral 与 smirk._1/._2 = 0.0。A3 契约（contract-bai-params-v0 §2/§6.1）
 *     已按该数据落地（fade + setParametricExpression 注入 6 键含 MouthOpenY；口型让位由「最近写入者胜」LipSync 兜底）。
 *     → 若测试断言「**所有** 情绪帧 MouthOpenY=0」，将与已锁定/已绿的真值数据冲突。本测试据实锁定：
 *       (a) neutral 帧 MouthOpenY == 0.0（真值成立）
 *       (b) 帧值域 MouthOpenY ∈ [0, 1.5]（真值 0.0..0.7 成立）
 *       (c) 「情绪帧不允许 EyeBall 键（眼神不动）」成立（真值情绪表无 EyeBall）。
 *     「全部情绪帧 MouthOpenY=0」为调研推荐语义，与 bai 数据/既有 A3 冲突，**如实上报主代理，不强行锁定**。
 */
class ExpressionFrameTest {

    /** 真值数据锚点：bai.adapter.json emotionParamMap（8 情绪 × 3 强度 × 6 键），非 src 实现。 */
    private val emotionParamMap: Map<String, Map<String, Map<String, Float>>> = mapOf(
        "neutral" to mapOf(
            "_1" to f(0.0f, 0.0f, 0.75f, 0.75f, 0.5f, 0.0f),
            "_2" to f(0.0f, 0.0f, 0.75f, 0.75f, 0.55f, 0.0f),
            "_3" to f(0.0f, 0.0f, 0.8f, 0.8f, 0.6f, 0.0f),
        ),
        "joy" to mapOf(
            "_1" to f(0.15f, 0.15f, 0.8f, 0.8f, 0.7f, 0.1f),
            "_2" to f(0.25f, 0.25f, 0.85f, 0.85f, 0.85f, 0.2f),
            "_3" to f(0.35f, 0.35f, 0.9f, 0.9f, 0.95f, 0.35f),
        ),
        "anger" to mapOf(
            "_1" to f(-0.2f, -0.2f, 0.65f, 0.65f, -0.3f, 0.05f),
            "_2" to f(-0.35f, -0.35f, 0.6f, 0.6f, -0.55f, 0.1f),
            "_3" to f(-0.5f, -0.5f, 0.55f, 0.55f, -0.75f, 0.2f),
        ),
        "sadness" to mapOf(
            "_1" to f(0.15f, 0.15f, 0.55f, 0.55f, -0.2f, 0.1f),
            "_2" to f(0.25f, 0.25f, 0.5f, 0.5f, -0.4f, 0.15f),
            "_3" to f(0.35f, 0.35f, 0.45f, 0.45f, -0.55f, 0.25f),
        ),
        "fear" to mapOf(
            "_1" to f(0.2f, 0.2f, 0.85f, 0.85f, 0.3f, 0.2f),
            "_2" to f(0.35f, 0.35f, 1.0f, 1.0f, 0.2f, 0.4f),
            "_3" to f(0.55f, 0.55f, 1.3f, 1.3f, 0.15f, 0.65f),
        ),
        "surprise" to mapOf(
            "_1" to f(0.25f, 0.25f, 0.95f, 0.95f, 0.5f, 0.3f),
            "_2" to f(0.4f, 0.4f, 1.2f, 1.2f, 0.4f, 0.5f),
            "_3" to f(0.55f, 0.55f, 1.5f, 1.5f, 0.3f, 0.7f),
        ),
        "smirk" to mapOf(
            "_1" to f(0.1f, 0.1f, 0.7f, 0.7f, -0.3f, 0.0f),
            "_2" to f(0.2f, 0.2f, 0.65f, 0.65f, -0.5f, 0.0f),
            "_3" to f(0.3f, 0.3f, 0.6f, 0.6f, -0.7f, 0.05f),
        ),
        "disgust" to mapOf(
            "_1" to f(-0.15f, -0.15f, 0.6f, 0.6f, -0.3f, 0.05f),
            "_2" to f(-0.25f, -0.25f, 0.55f, 0.55f, -0.5f, 0.1f),
            "_3" to f(-0.35f, -0.35f, 0.5f, 0.5f, -0.65f, 0.15f),
        ),
    )

    private fun f(
        browLY: Float, browRY: Float, eyeLOpen: Float, eyeROpen: Float,
        mouthForm: Float, mouthOpenY: Float,
    ): Map<String, Float> = mapOf(
        "ParamBrowLY" to browLY,
        "ParamBrowRY" to browRY,
        "ParamEyeLOpen" to eyeLOpen,
        "ParamEyeROpen" to eyeROpen,
        "ParamMouthForm" to mouthForm,
        "ParamMouthOpenY" to mouthOpenY,
    )

    // ═══════════════════════════════════════════════════════════
    // EF-01：frameOf 结构完整性 —— 8 情绪 × 3 强度 × 6 键（契约 A3U-01=144 值）
    // ═══════════════════════════════════════════════════════════

    /** EF-01-①：8 情绪全枚举且各含 _1/_2/_3。 */
    @Test
    fun frameOf_allEightEmotions_threeLevelsPresent() {
        val expectedEmotions = setOf("neutral", "fear", "sadness", "anger", "disgust", "joy", "smirk", "surprise")
        assertEquals("情绪全集 = bai emotionParamMap 的 8 key", expectedEmotions, emotionParamMap.keys)
        for (emo in expectedEmotions) {
            assertEquals("情绪 $emo 应有 3 强度档", setOf("_1", "_2", "_3"), emotionParamMap[emo]!!.keys)
        }
    }

    /** EF-01-②：frameOf 抽取任意 (情绪,强度) 返回 6 键完整帧（A3U-01：每帧 6 键）。 */
    @Test
    fun frameOf_extractsSixKeyFrame_forEveryEmotionLevel() {
        for (emo in emotionParamMap.keys) {
            for (lvl in 1..3) {
                val frame = frameOf(emotionParamMap, emo, lvl)
                assertNotNull("frameOf($emo,$lvl) 应非空", frame)
                assertEquals("frameOf($emo,$lvl) 应恰含 6 键", 6, frame!!.size)
                assertEquals(
                    "frameOf($emo,$lvl) 键集应为 6 键标准参数",
                    setOf("ParamBrowLY", "ParamBrowRY", "ParamEyeLOpen", "ParamEyeROpen",
                        "ParamMouthForm", "ParamMouthOpenY"),
                    frame.keys,
                )
            }
        }
    }

    // ═══════════════════════════════════════════════════════════
    // EF-02：三轴值域锁定（BrowLY/RY ∈ [-1,1]、EyeLOpen/ROpen ∈ [0,1.5]、MouthForm ∈ [-1,1]、MouthOpenY ∈ [0,1.5]）
    // ═══════════════════════════════════════════════════════════

    /** EF-02-①：所有 144 帧值均在官方三轴范围（Brow 眉高 / 眼开 / 嘴形+开合）。 */
    @Test
    fun allFrameValues_withinAxisRanges() {
        for (emo in emotionParamMap.keys) {
            for (frame in emotionParamMap[emo]!!.values) {
                for ((key, v) in frame) {
                    when (key) {
                        "ParamBrowLY", "ParamBrowRY" ->
                            assertTrue("$key 应在 [-1,1]，实际 $v", v in -1f..1f)
                        "ParamEyeLOpen", "ParamEyeROpen" ->
                            assertTrue("$key 应在 [0,1.5]，实际 $v", v in 0f..1.5f)
                        "ParamMouthForm" ->
                            assertTrue("$key 应在 [-1,1]，实际 $v", v in -1f..1f)
                        "ParamMouthOpenY" ->
                            assertTrue("$key 应在 [0,1.5]，实际 $v", v in 0f..1.5f)
                    }
                }
            }
        }
    }

    /** EF-02-②：值域边界锚点命中（契约 A3U-03 实读：max=1.5(surprise._3 EyeLOpen)、min=-0.7(smirk._3 MouthForm)）。 */
    @Test
    fun axisRangeGlobalExtremes_matchContractAnchors() {
        val surprise3 = frameOf(emotionParamMap, "surprise", 3)!!
        assertEquals("surprise._3 EyeLOpen 应为帧值域上界 1.5（A3U-03 实读 max）", 1.5f, surprise3["ParamEyeLOpen"]!!, 0f)
        assertEquals("surprise._3 EyeROpen 应为 1.5", 1.5f, surprise3["ParamEyeROpen"]!!, 0f)

        val smirk3 = frameOf(emotionParamMap, "smirk", 3)!!
        assertEquals("smirk._3 MouthForm 应达下界 -0.7（A3U-03 实读 min）", -0.7f, smirk3["ParamMouthForm"]!!, 0f)
    }

    /** EF-02-③：isValidEmotionFrame 对越界/含 EyeBall 键帧返回 false（官方标准参数表边界）。 */
    @Test
    fun isValidEmotionFrame_rejectsOutOfRange_andEyeBallKeys() {
        assertTrue("合法 neutral._1 帧应通过校验", isValidEmotionFrame(frameOf(emotionParamMap, "neutral", 1)!!))
        // 眼开合越界（<0 或 >1.5）→ 拒绝
        assertFalse("眼开合 EyeLOpen=-0.1 越界应拒", isValidEmotionFrame(f("ParamEyeLOpen" to -0.1f)))
        assertFalse("眼开合 EyeLOpen=2.1 越界应拒", isValidEmotionFrame(f("ParamEyeLOpen" to 2.1f)))
        // 眉高越界
        assertFalse("眉 BrowLY=1.2 越界应拒", isValidEmotionFrame(f("ParamBrowLY" to 1.2f)))
        // 嘴形越界
        assertFalse("嘴形 MouthForm=-1.3 越界应拒", isValidEmotionFrame(f("ParamMouthForm" to -1.3f)))
        // 眼神键混入情绪帧（官方「眼神不动情绪帧」违反）→ 拒
        assertFalse(
            "情绪帧含 EyeBallX 键（眼神不应在情绪帧）应拒",
            isValidEmotionFrame(frameOf(emotionParamMap, "joy", 3)!! + ("ParamEyeBallX" to 0.2f)),
        )
    }

    // ═══════════════════════════════════════════════════════════
    // EF-03：官方机制语义锁定
    // ═══════════════════════════════════════════════════════════

    /** EF-03-①：情绪帧不含 EyeBallX/Y（眼神漂移走程序化待机层，官方 §3.2 + airi 一致；model-facts-bai 情绪表本就无 EyeBall）。 */
    @Test
    fun emotionFrames_excludeEyeBallKeys_eyeDriftIsProceduralLayer() {
        assertTrue(
            "bai emotionParamMap 中任一帧不应含 EyeBall 键（眼神不动情绪帧）",
            emotionParamMap.values.none { emo ->
                emo.values.any { frame -> frame.keys.any { it.contains("EyeBall") } }
            },
        )
    }

    /** EF-03-②：neutral 三档 MouthOpenY == 0.0（neutral 基准不开口，口型让位 LipSync）。此项真值成立；非 neutral 见 [CONFLICT] 标注。 */
    @Test
    fun neutralFrames_mouthOpenYIsZero() {
        for (lvl in 1..3) {
            val frame = frameOf(emotionParamMap, "neutral", lvl)!!
            assertEquals("neutral._$lvl MouthOpenY 应为 0（neutral 不开口）", 0.0f, frame["ParamMouthOpenY"]!!, 0f)
        }
    }

    /** EF-03-③：参数差叠加语义锚点 —— 合法帧抽取后各键值与数据源逐键一致（frameOf 是「按表抽取」，不篡改/不插值）。 */
    @Test
    fun frameOf_returnsDataSourceKeyValues_byteExact() {
        val joy3 = frameOf(emotionParamMap, "joy", 3)!!
        assertEquals("joy._3 BrowLY 应=数据源 0.35", 0.35f, joy3["ParamBrowLY"]!!, 1e-6f)
        assertEquals("joy._3 MouthForm 应=数据源 0.95", 0.95f, joy3["ParamMouthForm"]!!, 1e-6f)
    }

    /** EF-03-④：强度档语义 —— _1/_2/_3 为离散档，档值从数据源逐档取（非连续插值），任意两档应可区分（抽样 joy/surprise）。 */
    @Test
    fun intensityLevels_areDistinctDataSourceLevels() {
        for (emo in listOf("joy", "surprise", "anger")) {
            val f1 = frameOf(emotionParamMap, emo, 1)!!
            val f2 = frameOf(emotionParamMap, emo, 2)!!
            val f3 = frameOf(emotionParamMap, emo, 3)!!
            assertTrue(
                "$emo._1 与 $emo._3 应存在可区分参数的档差（离散档非恒等）",
                (f1.keys + f2.keys + f3.keys).any { key -> f1[key] != f3[key] },
            )
        }
    }

    // ═══════════════════════════════════════════════════════════
    // EF-04：[CONFLICT] MouthOpenY 表达式帧语义如实锁定（仅锁真值成立处）
    // ═══════════════════════════════════════════════════════════

    /** EF-04：非 neutral 情绪帧 MouthOpenY 在真值数据中非零 —— 该事实与任务项 2「所有帧 MouthOpenY=0」冲突，如实记录（测试锁真值）。 */
    @Test
    fun nonNeutralEmotionFrames_doHaveMouthOpenY_inRealData_sourceOfTruth() {
        val surprise3 = frameOf(emotionParamMap, "surprise", 3)!!
        assertEquals("surprise._3 MouthOpenY 真值 = 0.7（>0，证非 neutral 帧 MouthOpenY 非零）", 0.7f, surprise3["ParamMouthOpenY"]!!, 0f)
        val joy3 = frameOf(emotionParamMap, "joy", 3)!!
        assertEquals("joy._3 MouthOpenY 真值 = 0.35（>0）", 0.35f, joy3["ParamMouthOpenY"]!!, 0f)
    }

    // ═══════════════════════════════════════════════════════════
    // 测试用辅助（占位，供 coder 对齐；生产符号见文件头契约）
    // 这些引用在待实现类存在前编译期 RED（属预期）。实际实现交给 cil。
    // ═══════════════════════════════════════════════════════════

    /** 占位：对单键越界帧构造（用于 isValidEmotionFrame 否定分支）。 */
    private fun f(vararg kv: Pair<String, Float>): Map<String, Float> {
        val base = mapOf(
            "ParamBrowLY" to 0f, "ParamBrowRY" to 0f,
            "ParamEyeLOpen" to 0.75f, "ParamEyeROpen" to 0.75f,
            "ParamMouthForm" to 0.5f, "ParamMouthOpenY" to 0f,
        )
        return base + kv
    }
}
