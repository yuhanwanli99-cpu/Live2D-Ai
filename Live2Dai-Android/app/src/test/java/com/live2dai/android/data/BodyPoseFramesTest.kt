package com.live2dai.android.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * R16 半身姿态联动：身体/头部姿态参数帧（BodyPoseFrames）纯函数断言（8 情绪 × 3 强度 → ParamBodyAngleX/Y/Z + ParamAngleX/Y/Z）。
 *
 * 依据（唯一真相源，禁读 src/ 实现）：
 *  - .pi/spoq/model-facts-bai.md §三（bai cdi3 128 参数：头部/身体 = ParamAngleX/Y/Z + ParamBodyAngleX/Y/Z 实读存在）
 *  - .pi/spoq/recon-techdebt-feasibility.md A.1（轴结构语义：AngleX=头点头/低头、AngleY=头左右转、AngleZ=头歪头；
 *    BodyAngleX=身体前倾/后仰、BodyAngleY=身体左右转体、BodyAngleZ=身体侧倾/摇摆）
 *  - .pi/spoq/search-report-expression.md §2.1/§3（官方标准参数范围 ParamAngleX/Y/Z ∈ [-30,30]；airi 头 focus ±5-15° 阻尼；
 *    身体/重心 BodyAngle 低频微动参照）
 *  - ExpressionFrames（R13 已绿 object，同包）：封装风格同构（object + frameOf(emotion, level) + isValid*Frame）；
 *    6 键 = ParamBrowLY/RY + EyeLOpen/ROpen + MouthForm/MouthOpenY —— 与 BodyPoseFrames 零交集（断言 BPP-05）
 *
 * 封装的待实现生产符号（**均不存在，编译期 RED 属预期**；cil=coder 待实现，按下文契约照抄实现）：
 *   object BodyPoseFrames {
 *       // 三轴合法值域常量（官方标准参数表；搜索报告 §2.1 锚定 [-30,30]）
 *       const val ANGLE_MIN = -30f
 *       const val ANGLE_MAX = 30f
 *       // 8 情绪键（bai 权威键集，与 ExpressionFrames.EMOTIONS 一致）
 *       val EMOTIONS: List<String> = listOf("neutral", "fear", "sadness", "anger", "disgust", "joy", "smirk", "surprise")
 *       // 3 强度档（离散选档，契约 D3 同 R13）
 *       val LEVELS: List<Int> = listOf(1, 2, 3)
 *       // 身体 3 + 头 3 = 6 键白名单（与 ExpressionFrames 三轴 6 键零交集）
 *       val POSE_KEYS: Set<String> = setOf(
 *           "ParamBodyAngleX", "ParamBodyAngleY", "ParamBodyAngleZ",
 *           "ParamAngleX", "ParamAngleY", "ParamAngleZ",
 *       )
 *       // 六轴统一值域 = 官方 [-30,30]
 *       val POSE_RANGES: Map<String, ClosedFloatingPointRange<Float>> = mapOf(
 *           "ParamBodyAngleX" to -30f..30f,
 *           "ParamBodyAngleY" to -30f..30f,
 *           "ParamBodyAngleZ" to -30f..30f,
 *           "ParamAngleX"     to -30f..30f,
 *           "ParamAngleY"     to -30f..30f,
 *           "ParamAngleZ"     to -30f..30f,
 *       )
 *       // 取 8 情绪 × 3 强度档姿态帧（6 键）；未知情绪/档位 → null；缺省档位 → 3
 *       fun frameOf(emotion: String, level: Int = 3): Map<String, Float>?
 *       // 6 键姿态帧校验：非空；键集 ⊆ POSE_KEYS；全部值 ∈ [-30,30]
 *       fun isValidBodyPoseFrame(frame: Map<String, Float>): Boolean
 *   }
 *
 * 轴语义契约（给 coder 照抄值表 + 防乱改语义；与 recon A.1 轴结构对齐，负面=负向动作）：
 *   - ParamBodyAngleX 身体前倾/后仰（负=前倾低头，正=后仰）；ParamBodyAngleY 身体左右转体；ParamBodyAngleZ 身体侧倾/摇摆
 *   - ParamAngleX 头点头/低头（负=低头，正=抬）；ParamAngleY 头左右转；ParamAngleZ 头歪头
 *   - joy     = 轻身摆(BAX 2/3/5) + 头微抬(AX 2/4/6) + 轻转(AY 3/6/8) + 侧摆(BAZ 2/4/6)
 *   - sad     = 头深深低头(AX -4/-8/-12) + 身体前倾下沉(BAX -3/-5/-8) + 身微垂(BAZ -1/-2/-3)
 *   - anger   = 身体前倾压迫(BAX -3/-7/-12) + 头压低前伸(AX -2/-4/-6) + 怒视侧转(AY -1/-2/-3)
 *   - surprise= 身体后仰(BAX +8/+13/+18) + 头抬(AX +3/+6/+10)（后仰幅度随档加大，任务语义 后仰 +8~+18 / 抬 +3~+10）
 *   - disgust = 身体后仰躲避(BAX +6/+10/+14) + 头别过侧(AY -2/-4/-6) + 歪头(AZ -1/-2/-3)
 *   - fear    = 身体后缩(BAX +5/+9/+13) + 转体躲闪(BAY -2/-4/-6) + 头压低(AX -1/-3/-5)
 *   - smirk   = 得意坏笑头歪(AZ +3/+6/+9) + 身体侧斜(BAZ -2/-4/-6) + 微转体(BAY +2/+4/+6) + 头微抬(AX +1/+2/+3)
 *
 * [UNSURE 标注 —— 轴名 vs 物理语义冲突，如实上报主代理，本测试按物理正确轴落表]：
 *   任务项 3 的字面措辞将「低头/微抬/前倾/后仰」赋给了 AngleY / BodyAngleY（头/身体的**左右转**轴）：
 *     - joy     ：任务写「AngleY 微抬（+2~+8）」
 *     - sad     ：任务写「AngleY 低头（-12~-4）+ BodyAngleY 下沉（负值）」
 *     - angry   ：任务写「BodyAngleY 前倾（负值 -12~-3）」
 *     - surprise：任务写「BodyAngleY 后仰（+8~+18）+ AngleY 抬（+3~+10）」
 *   但 recon-techdebt-feasibility.md A.1（由 cdi3.json 实读轴结构）明确：低头/抬头是 AngleX（头 pitch）、
 *   前倾/后仰是 BodyAngleX（身体 pitch）；AngleY/BodyAngleY 是左右转（yaw）——语义不指向垂直运动。
 *   → 冲突：任务希望「低头/前倾/后仰」表达的身体姿态，若按字面落在 yaw 轴，画面会是左右转而非俯仰。
 *     **本测试按物理正确轴锁定值表（低头→AX 负、前倾→BAX 负、后仰→BAX 正、抬→AX 正），
 *     数值量级沿用任务所给范围（如 sad AX -12~-4、surprise BAX +8~+18），并在报告中如实上报该轴映射取舍，
 *     等主代理确认是否需改轴名。** 值表（BPP-07）为 byte-exact 契约，coder 照抄；
 *     若主代理裁定改轴，仅需改 POSE_KEYS 与值表键名，断言结构不变。
 *
 * 值表 = 契约一部份（tester 出值 coder 照抄，防 coder 乱改语义）。所有值 ∈ [-30,30]；neutral 全档全 0。
 */
class BodyPoseFramesTest {

    /** 值表契约（tester 锁定；mirror 为实现对照源，均 6 键、全值 ∈ [-30,30]、neutral 全 0） */
    private val valueTable: Map<String, Map<Int, Map<String, Float>>> = mapOf(
        "neutral" to mapOf(
            1 to p(0f, 0f, 0f, 0f, 0f, 0f),
            2 to p(0f, 0f, 0f, 0f, 0f, 0f),
            3 to p(0f, 0f, 0f, 0f, 0f, 0f),
        ),
        "joy" to mapOf(
            1 to p(2f, 0f, 2f, 2f, 3f, 1f),
            2 to p(3f, 0f, 4f, 4f, 6f, 2f),
            3 to p(5f, 0f, 6f, 6f, 8f, 3f),
        ),
        "sadness" to mapOf(
            1 to p(-3f, 0f, -1f, -4f, 0f, 0f),
            2 to p(-5f, 0f, -2f, -8f, 0f, 0f),
            3 to p(-8f, 0f, -3f, -12f, 0f, 0f),
        ),
        "anger" to mapOf(
            1 to p(-3f, 0f, 0f, -2f, -1f, 0f),
            2 to p(-7f, 0f, 0f, -4f, -2f, 0f),
            3 to p(-12f, 0f, 0f, -6f, -3f, 0f),
        ),
        "disgust" to mapOf(
            1 to p(6f, 0f, 0f, 0f, -2f, -1f),
            2 to p(10f, 0f, 0f, 0f, -4f, -2f),
            3 to p(14f, 0f, 0f, 0f, -6f, -3f),
        ),
        "fear" to mapOf(
            1 to p(5f, -2f, 0f, -1f, 0f, 0f),
            2 to p(9f, -4f, 0f, -3f, 0f, 0f),
            3 to p(13f, -6f, 0f, -5f, 0f, 0f),
        ),
        "smirk" to mapOf(
            1 to p(0f, 2f, -2f, 1f, 0f, 3f),
            2 to p(0f, 4f, -4f, 2f, 0f, 6f),
            3 to p(0f, 6f, -6f, 3f, 0f, 9f),
        ),
        "surprise" to mapOf(
            1 to p(8f, 0f, 0f, 3f, 0f, 0f),
            2 to p(13f, 0f, 0f, 6f, 0f, 0f),
            3 to p(18f, 0f, 0f, 10f, 0f, 0f),
        ),
    )

    /** 6 键构造器（顺序：BodyAngleX/Y/Z, AngleX/Y/Z） */
    private fun p(
        bodyAngleX: Float, bodyAngleY: Float, bodyAngleZ: Float,
        angleX: Float, angleY: Float, angleZ: Float,
    ): Map<String, Float> = mapOf(
        "ParamBodyAngleX" to bodyAngleX,
        "ParamBodyAngleY" to bodyAngleY,
        "ParamBodyAngleZ" to bodyAngleZ,
        "ParamAngleX" to angleX,
        "ParamAngleY" to angleY,
        "ParamAngleZ" to angleZ,
    )

    /** 单键覆写帧构造器（用于 isValidBodyPoseFrame 否定分支，默认全 0 基底） */
    private fun over(vararg kv: Pair<String, Float>): Map<String, Float> = p(0f, 0f, 0f, 0f, 0f, 0f) + kv

    /** R13 已绿 6 键（用于零交集断言 BPP-05） */
    private val expressionAxisKeys = setOf(
        "ParamBrowLY", "ParamBrowRY",
        "ParamEyeLOpen", "ParamEyeROpen",
        "ParamMouthForm", "ParamMouthOpenY",
    )

    // ═══════════════════════════════════════════════════════════
    // BPP-01：frameOf 结构完整性 —— 8 情绪 × 3 强度 × 6 键 = 144 值
    // ═══════════════════════════════════════════════════════════

    /** BPP-01-①：8 情绪全枚举，各情绪 3 档都能取到帧（与 ExpressionFrames.EMOTIONS 同集会）。 */
    @Test
    fun frameOf_allEightEmotions_threeLevelsAvailable() {
        val expected = setOf("neutral", "fear", "sadness", "anger", "disgust", "joy", "smirk", "surprise")
        assertEquals("情绪全集应为 8 键（bai 权威键集）", expected, valueTable.keys)
        for (emo in expected) {
            for (lvl in 1..3) {
                assertNotNull("frameOf($emo,$lvl) 应非空（8 情绪 × 3 档完整）", BodyPoseFrames.frameOf(emo, lvl))
            }
        }
    }

    /** BPP-01-②：frameOf 每帧恰含 6 键 = 身体 3 + 头 3（BodyAngleX/Y/Z + AngleX/Y/Z）。 */
    @Test
    fun frameOf_extractsSixKeyPoseFrame_forEveryEmotionLevel() {
        val poseKeys = setOf(
            "ParamBodyAngleX", "ParamBodyAngleY", "ParamBodyAngleZ",
            "ParamAngleX", "ParamAngleY", "ParamAngleZ",
        )
        for (emo in valueTable.keys) {
            for (lvl in 1..3) {
                val frame = BodyPoseFrames.frameOf(emo, lvl)
                assertNotNull("frameOf($emo,$lvl) 应非空", frame)
                assertEquals("frameOf($emo,$lvl) 应恰含 6 键", 6, frame!!.size)
                assertEquals("frameOf($emo,$lvl) 键集应为身体 3 + 头 3", poseKeys, frame.keys)
            }
        }
    }

    // ═══════════════════════════════════════════════════════════
    // BPP-02：byte-exact 值表镜像（tester 契约值，coder 照抄）+ 未知键行为
    // ═══════════════════════════════════════════════════════════

    /** BPP-02-①：全部 8×3 帧与值表逐键精确一致（144 值 = 契约）。 */
    @Test
    fun frameOf_matchesValueTable_byteExact_allFrames() {
        for ((emo, levels) in valueTable) {
            for ((lvl, expected) in levels) {
                val frame = BodyPoseFrames.frameOf(emo, lvl)!!
                assertEquals("frameOf($emo,$lvl) 应逐键精确=契约值表", expected, frame)
            }
        }
    }

    /** BPP-02-②：缺省档位 → 满强度 _3（与 ExpressionFrames.frameOf(emo)=_3 语义同构）。 */
    @Test
    fun frameOf_defaultLevel_isLevel3() {
        for (emo in valueTable.keys) {
            assertEquals("frameOf($emo) 缺省 → _3", BodyPoseFrames.frameOf(emo, 3), BodyPoseFrames.frameOf(emo))
        }
    }

    /** BPP-02-③：未知情绪/非法档位 → null（防呆，与 ExpressionFrames 同构）。 */
    @Test
    fun frameOf_unknownEmotionOrLevel_returnsNull() {
        assertNull("未知情绪 → null", BodyPoseFrames.frameOf("confused", 1))
        assertNull("档位 0 越界 → null", BodyPoseFrames.frameOf("joy", 0))
        assertNull("档位 4 越界 → null", BodyPoseFrames.frameOf("joy", 4))
    }

    // ═══════════════════════════════════════════════════════════
    // BPP-03：值域断言 —— 全部值 ∈ [-30,30]（官方参数范围）
    // ═══════════════════════════════════════════════════════════

    /** BPP-03-①：所有 144 值都在官方 [-30,30] 内（搜索报告 §2.1 官方角度范围）。 */
    @Test
    fun allPoseValues_withinOfficialAngleRange() {
        for (emo in valueTable.keys) {
            for (lvl in 1..3) {
                val frame = BodyPoseFrames.frameOf(emo, lvl)!!
                for ((key, v) in frame) {
                    assertTrue("$key 应在官方 [-30,30]，实际 $v", v in -30f..30f)
                }
            }
        }
    }

    /** BPP-03-②：isValidBodyPoseFrame 校验 —— 合法帧通过；越界/未知键/空帧拒绝。 */
    @Test
    fun isValidBodyPoseFrame_rejectsOutOfRange_andUnknownKey_andEmpty() {
        assertTrue("合法 joy._3 帧应通过", BodyPoseFrames.isValidBodyPoseFrame(BodyPoseFrames.frameOf("joy", 3)!!))
        assertTrue("校验不应依赖具体情绪（disgust._2 也应过）", BodyPoseFrames.isValidBodyPoseFrame(BodyPoseFrames.frameOf("disgust", 2)!!))

        assertFalse("BodyAngleX=31 越界应拒", BodyPoseFrames.isValidBodyPoseFrame(over("ParamBodyAngleX" to 31f)))
        assertFalse("AngleY=-31 越界应拒", BodyPoseFrames.isValidBodyPoseFrame(over("ParamAngleY" to -31f)))
        assertFalse("含表情面键（MouthForm）应拒——只接受身体/头 6 键", BodyPoseFrames.isValidBodyPoseFrame(over("ParamMouthForm" to 0.5f)))
        assertFalse("空帧应拒", BodyPoseFrames.isValidBodyPoseFrame(emptyMap()))
    }

    // ═══════════════════════════════════════════════════════════
    // BPP-04：neutral 全档 = 全 0（无姿态）
    // ═══════════════════════════════════════════════════════════

    /** BPP-04：neutral _1/_2/_3 六键全为 0（无姿态底噪基准，任务项 2）。 */
    @Test
    fun neutralFrames_allAxesZero() {
        for (lvl in 1..3) {
            val frame = BodyPoseFrames.frameOf("neutral", lvl)!!
            for ((key, v) in frame) {
                assertEquals("neutral._$lvl $key 应为 0（中性无姿态）", 0.0f, v, 0f)
            }
        }
    }

    // ═══════════════════════════════════════════════════════════
    // BPP-05：与表情帧 6 键零交集
    // ═══════════════════════════════════════════════════════════

    /** BPP-05：BodyPoseFrames 6 键（BodyAngleX/Y/Z + AngleX/Y/Z）与 ExpressionFrames 6 键（眉/眼/嘴）零交集。 */
    @Test
    fun bodyPoseAxisKeys_disjointFromExpressionAxisKeys() {
        val bodyPoseKeys = setOf(
            "ParamBodyAngleX", "ParamBodyAngleY", "ParamBodyAngleZ",
            "ParamAngleX", "ParamAngleY", "ParamAngleZ",
        )
        assertTrue(
            "身体/头姿态键与表情面键应零交集（缺身体姿态短板 = 补足，不挤占表情键）",
            bodyPoseKeys.intersect(expressionAxisKeys).isEmpty(),
        )
        // 与已绿 R13 共享语义交集：情绪键集一致（同 bai 权威 8 键）
        assertEquals(
            "BodyPoseFrames 情绪键集应与 ExpressionFrames.EMOTIONS 一致",
            ExpressionFrames.EMOTIONS, BodyPoseFrames.EMOTIONS,
        )
    }

    // ═══════════════════════════════════════════════════════════
    // BPP-06：强度档离散语义 —— _1/_2/_3 可区分（抽样）
    // ═══════════════════════════════════════════════════════════

    /** BPP-06：非 neutral 情绪 _1 与 _3 应有可区分参数差（离散档非恒等，任取一轴）。 */
    @Test
    fun intensityLevels_areDistinct_forNonNeutral() {
        for (emo in listOf("joy", "sadness", "anger", "surprise", "disgust", "fear", "smirk")) {
            val f1 = BodyPoseFrames.frameOf(emo, 1)!!
            val f3 = BodyPoseFrames.frameOf(emo, 3)!!
            assertTrue(
                "$emo._1 与 $emo._3 应存在档差（离散档）",
                f1.keys.any { key -> f1[key] != f3[key] },
            )
            // 强度单调性：主转动轴 _3 幅值 ≥ _1（避免档位倒挂）
            assertTrue(
                "$emo 强度应随档位不衰减（任意键 _3 幅值 ≥ _1）",
                f1.keys.all { key -> Math.abs(f3[key]!!) >= Math.abs(f1[key]!!) },
            )
        }
    }

    // ═══════════════════════════════════════════════════════════
    // BPP-07：语义方向锁定（值表契约锚点示例，byte-exact；防 coder 乱改语义）
    // ═══════════════════════════════════════════════════════════

    /** BPP-07：抽样语义锚点 —— sadness 头低头(AX 负大)、anger 身体前倾(BAX 负)、surprise 后仰(BAX 正)+头抬(AX 正)、smirk 歪头(AZ 正)。 */
    @Test
    fun semanticAnchors_matchValueTable() {
        val sad3 = BodyPoseFrames.frameOf("sadness", 3)!!
        assertEquals("sadness._3 低头: AngleX 应=-12", -12f, sad3["ParamAngleX"]!!, 0f)
        assertTrue("sadness._3 AngleX 应为负（低头，<-4）", sad3["ParamAngleX"]!! <= -4f)

        val anger3 = BodyPoseFrames.frameOf("anger", 3)!!
        assertEquals("anger._3 前倾: BodyAngleX 应=-12", -12f, anger3["ParamBodyAngleX"]!!, 0f)
        assertTrue("anger._3 BodyAngleX 应为负（前倾压迫）", anger3["ParamBodyAngleX"]!! <= -3f)

        val surprise3 = BodyPoseFrames.frameOf("surprise", 3)!!
        assertEquals("surprise._3 后仰: BodyAngleX 应=18", 18f, surprise3["ParamBodyAngleX"]!!, 0f)
        assertTrue("surprise._3 BodyAngleX 应∈[8,18]（后仰）", surprise3["ParamBodyAngleX"]!! in 8f..18f)
        assertTrue("surprise._3 头抬: AngleX 应>0", surprise3["ParamAngleX"]!! > 0f)

        val smirk3 = BodyPoseFrames.frameOf("smirk", 3)!!
        assertTrue("smirk._3 歪头: AngleZ 应>0", smirk3["ParamAngleZ"]!! > 0f)

        val joy3 = BodyPoseFrames.frameOf("joy", 3)!!
        assertTrue("joy._3 BodyAngleX 应|·|≤10（微摆）", Math.abs(joy3["ParamBodyAngleX"]!!) <= 10f)
        assertTrue("joy._3 头抬(AngleX) 应>0", joy3["ParamAngleX"]!! > 0f)
    }

    // ═══════════════════════════════════════════════════════════
    // 测试辅助（占位；生产符号见文件头契约，编译期 RED 属预期）
    // ═══════════════════════════════════════════════════════════
}
