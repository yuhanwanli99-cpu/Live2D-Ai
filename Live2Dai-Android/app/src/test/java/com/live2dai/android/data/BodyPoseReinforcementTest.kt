// =============================================================================
// RED 测试 — R17 动作层强化（情绪姿态幅度加大）新档位增量断言
// T8 测试设计师 | 依据: translation.md R17 项3 + T7b BodyPoseFrames 值表（不覆盖旧值）
//
// 【tester 裁定——强化方式】
//     新档位（reinforced tier），NOT 覆盖旧值。
//     理由: T7b BodyPoseFrames 已 GREEN（memory `t7c-contract-sync-lesson` 锁 6→12 键演进，
//           且 547/547 全绿）。覆盖旧值会回归既有测试，故 R17 强化以【新类】
//           BodyPoseReinforcement 叠加，提供情绪姿态【强化档位】满足幅度要求，
//           旧 BodyPoseFrames 值表字节级保留。
//     范围: 需求仅实锤 3 情绪强化（joy 身摆±8-12 / sad 低头-15 / surprise 后仰+15-20）。
//           其余 5 情绪 reinforcedFrameOf 回退到 BodyPoseFrames 基准值（不发明新值）。
//
// 【接口契约 — 给 coder】
// 新类:  package com.live2dai.android.data
//        object BodyPoseReinforcement   （纯函数，JVM 可测，禁 Android 依赖）
//
//  常量:
//     const val REINFORCE_LEVEL_SEPARATOR   = "_"      // 不带入 API，仅文档
//     val REINFORCED_EMOTIONS = setOf("joy","sadness","surprise")   // R17 元组实锤
//     val POSE_KEYS = listOf(
//         "ParamBodyAngleX","ParamBodyAngleY","ParamBodyAngleZ",   // 身体3
//         "ParamAngleX","ParamAngleY","ParamAngleZ"                // 头3
//     )
//
//  函数:
//     fun reinforcedFrameOf(emotion: String, level: Int = 3): Map<String,Float>?
//         - emotion ∈ {joy,sadness,surprise} → 返回强化档位 6 键帧（byte-exact 下表）
//         - emotion ∈ 其它（T7b 5 情绪）→ 返回 BodyPoseFrames.frameOf(emotion, level)
//                                              （回退基准，不强加新值）
//         - 未知 emotion / 非法 level → null
//     fun isReinforcedEmotion(emotion: String): Boolean
//
//  强化值表（契约 byte-exact 照抄，键序 BAX BAY BAZ AX AY AZ）:
//   | emotion | level | BAX | BAY | BAZ | AX  | AY  | AZ  | 语义 / 需求依据 |
//   | joy     | 1 | 4 | 0 |  8 | 4 | 5 | 2 | 身摆 BAZ=8（≥8）强化，头抬 |
//   | joy     | 2 | 5 | 0 | 10 | 6 | 8 | 3 | ↑ 摆幅10 |
//   | joy     | 3 | 6 | 0 | 12 | 8 |10 | 4 | ↑ 摆幅12（joy 身摆 ∈[8,12]°）✅ |
//   | sadness | 1 |-6 | 0 | -2 |-15| 0 | 0 | 头深低头 AX=-15 ✅ |
//   | sadness | 2 |-9 | 0 | -3 |-15| 0 | 0 | ↑ 身下沉加深 |
//   | sadness | 3 |-12| 0 | -4 |-15| 0 | 0 | ↑ |
//   | surprise| 1 |15 | 0 |  0 | 6 | 0 | 0 | 身后仰 BAX=15（∈[15,20]）✅ |
//   | surprise| 2 |18 | 0 |  0 | 8 | 0 | 0 | ↑ |
//   | surprise| 3 |20 | 0 |  0 |10 | 0 | 0 | ↑ 后仰 20（≤20）✅ |
//   （全值 ∈[-30,30]；BAY 全 0；neutral/其它回退 BodyPoseFrames）
//
//  轴语义沿用 T7b 物理正确轴: 低头/抬头=∠AngleX（头 pitch）、前倾/后仰=∠BodyAngleX、
//                            侧摆/侧倾=∠BodyAngleZ、点头/转体=∠AngleY。
//  任务字面轴名（AngleY 赋俯仰）与物理轴冲突已在 T7b 上报，本测试沿用物理正确轴。
// =============================================================================

package com.live2dai.android.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class BodyPoseReinforcementTest {

    private val BAX = "ParamBodyAngleX"
    private val BAY = "ParamBodyAngleY"
    private val BAZ = "ParamBodyAngleZ"
    private val AX = "ParamAngleX"
    private val AY = "ParamAngleY"
    private val AZ = "ParamAngleZ"

    private val reinforcedEmotions = setOf("joy", "sadness", "surprise")
    private val fallbackEmotions = setOf("neutral", "anger", "disgust", "fear", "smirk")

    // ---------- 1. 强化档位必须命中 R17 幅度要求 ----------

    @Test
    fun `R17-01 joy body sway amplitude 8 to 12 degrees on body-side axis`() {
        // 需求 joy 身体摆 ±8-12° → BAZ（身体侧摆）值 ∈[8,12]
        val f1 = BodyPoseReinforcement.reinforcedFrameOf("joy", 1)!!
        val f3 = BodyPoseReinforcement.reinforcedFrameOf("joy", 3)!!
        assertTrue("joy _1 BAZ=${f1[BAZ]} ∈[8,12]", f1[BAZ]!! >= 8f && f1[BAZ]!! <= 12f)
        assertTrue("joy _3 BAZ=${f3[BAZ]} ∈[8,12]", f3[BAZ]!! >= 8f && f3[BAZ]!! <= 12f)
        // 半身/头同向轻摆不致与身摆冲突（幅值为正）
        assertTrue(f1[BAX]!! > 0f)
        assertTrue(f1[AX]!! > 0f)
    }

    @Test
    fun `R17-02 sad head down exactly -15 degrees`() {
        // 需求 sad 低头 -15° → AX（头 pitch 低头）=-15（三个档位一致锁定深低头）
        for (lv in 1..3) {
            val f = BodyPoseReinforcement.reinforcedFrameOf("sadness", lv)!!
            assertEquals("sadness _${lv} AX 低头 -15", -15f, f[AX]!!, 0f)
        }
        // 身体随幅加深（身前倾下沉）
        val f1 = BodyPoseReinforcement.reinforcedFrameOf("sadness", 1)!!
        val f3 = BodyPoseReinforcement.reinforcedFrameOf("sadness", 3)!!
        assertTrue("sadness _3 BAX 幅>_1", abs(f3[BAX]!!) > abs(f1[BAX]!!))
    }

    @Test
    fun `R17-03 surprise lean back 15 to 20 degrees`() {
        // 需求 surprise 后仰 +15-20° → BAX（身体 pitch 后仰）∈[15,20]
        val f1 = BodyPoseReinforcement.reinforcedFrameOf("surprise", 1)!!
        val f3 = BodyPoseReinforcement.reinforcedFrameOf("surprise", 3)!!
        assertTrue("surprise _1 BAX=${f1[BAX]} ∈[15,20]", f1[BAX]!! >= 15f && f1[BAX]!! <= 20f)
        assertTrue("surprise _3 BAX=${f3[BAX]} ∈[15,20]", f3[BAX]!! >= 15f && f3[BAX]!! <= 20f)
    }

    @Test
    fun `R17-04 every reinforced value stays within official angle range -30 30`() {
        for (emo in bodyReinforcementEmotions()) {
            for (lv in 1..3) {
                val f = BodyPoseReinforcement.reinforcedFrameOf(emo, lv)!!
                for (k in BodyPoseReinforcement.POSE_KEYS) {
                    val v = f[k]!!
                    assertTrue("$emo/$lv $k=$v ∈[-30,30]", v >= -30f && v <= 30f)
                }
            }
        }
    }

    @Test
    fun `R17-05 reinforced frame has exactly the 6 pose keys`() {
        for (emo in bodyReinforcementEmotions()) {
            val f = BodyPoseReinforcement.reinforcedFrameOf(emo, 3)!!
            assertEquals("6 键", BodyPoseReinforcement.POSE_KEYS.toSet(), f.keys)
        }
    }

    // ---------- 2. 档位离散 / 单调 ----------
    @Test
    fun `R17-06 reinforce levels are discrete and monotonic non-decreasing in magnitude`() {
        for (emo in bodyReinforcementEmotions()) {
            val f1 = BodyPoseReinforcement.reinforcedFrameOf(emo, 1)!!
            val f3 = BodyPoseReinforcement.reinforcedFrameOf(emo, 3)!!
            assertTrue("$emo _1≠_3 离散", f1 != f3)
            for (k in BodyPoseReinforcement.POSE_KEYS) {
                assertTrue(
                    "$emo $k 档 3 幅≥档1",
                    abs(f3[k]!!) >= abs(f1[k]!!)
                )
            }
        }
    }

    // ---------- 3. 强化 vs 回退语义 ----------
    @Test
    fun `R17-07 reinforced frame strictly exceeds base on the strengthened axis`() {
        // joy BAZ 身摆强化>基准、sad AX 低头≥基准、surprise BAX 后仰>基准
        val baseJoy3 = BodyPoseFrames.frameOf("joy", 3)!![BAZ]!!
        val reinJoy3 = BodyPoseReinforcement.reinforcedFrameOf("joy", 3)!![BAZ]!!
        assertTrue("joy 摆幅强化 > 基准 $baseJoy3", reinJoy3 > baseJoy3)

        val baseSad1 = BodyPoseFrames.frameOf("sadness", 1)!![AX]!!
        val reinSad1 = BodyPoseReinforcement.reinforcedFrameOf("sadness", 1)!![AX]!!
        assertTrue("sad 低头强化 ≥ 基准 $baseSad1", abs(reinSad1) >= abs(baseSad1))

        val baseSur3 = BodyPoseFrames.frameOf("surprise", 3)!![BAX]!!
        val reinSur3 = BodyPoseReinforcement.reinforcedFrameOf("surprise", 3)!![BAX]!!
        assertTrue("surprise 后仰强化 > 基准 $baseSur3", reinSur3 > baseSur3)
    }

    @Test
    fun `R17-08 non-reinforced emotions fall back to base BodyPoseFrames values`() {
        // 需求仅实锤 3 情绪强化；其余 5 情绪回退 T7b 基准，不发明新值
        for (emo in fallbackEmotions) {
            for (lv in 1..3) {
                val base = BodyPoseFrames.frameOf(emo, lv)
                val rein = BodyPoseReinforcement.reinforcedFrameOf(emo, lv)
                assertNotNull("$emo/$lv 回退非空", rein)
                assertEquals("$emo/$lv 回退=基准", base, rein)
            }
        }
    }

    @Test
    fun `R17-09 unknown emotion or illegal level returns null`() {
        assertNull(BodyPoseReinforcement.reinforcedFrameOf("unknown_emotion"))
        assertNull(BodyPoseReinforcement.reinforcedFrameOf("blahblah", 3))
        assertNull(BodyPoseReinforcement.reinforcedFrameOf("joy", 0))
        assertNull(BodyPoseReinforcement.reinforcedFrameOf("joy", 4))
        assertFalse(BodyPoseReinforcement.isReinforcedEmotion("unknown_emotion"))
        assertTrue(BodyPoseReinforcement.isReinforcedEmotion("joy"))
        assertTrue(BodyPoseReinforcement.isReinforcedEmotion("sadness"))
        assertTrue(BodyPoseReinforcement.isReinforcedEmotion("surprise"))
    }

    @Test
    fun `R17-10 default level equals level 3`() {
        assertEquals(
            BodyPoseReinforcement.reinforcedFrameOf("joy"),
            BodyPoseReinforcement.reinforcedFrameOf("joy", 3)
        )
    }

    // helper: 被强化情绪集与 isReinforcedEmotion 一致（防止枚举漂移）
    private fun bodyReinforcementEmotions(): List<String> =
        BodyPoseReinforcement.REINFORCED_EMOTIONS.sorted()
}

private fun abs(f: Float): Float = Math.abs(f).toFloat()
