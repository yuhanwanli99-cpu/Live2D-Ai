package com.live2d.ai.android.render

import com.live2d.ai.android.data.EmotionTagTable
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * M7 + C-15 情绪标签 → 参数帧映射表纯函数 RED 测试。
 *
 * 依据（唯一真相源，禁读 src/）：
 *  - .pi/spoq/contract-refactor-0814.md §1.3 C-15：「表情」最小语义 = 仅 [tag]；
 *    §2.2 ExpressionSink.apply(emotion: String?)/release()（release 幂等，release 后回 neutral）。
 *  - .pi/spoq/contract-refactor-0814.md §3 RG1-A3：表情桥 + M7 标签表处理。
 *  - 既有 core/EmotionTextTest.kt（Wave 2C-3）：SUPPORTED_EMOTIONS 含 angry/joy/sad/neutral；
 *    未知标签 → neutral 回退；强度后缀 [keyword_数字] 忽略取 keyword；大小写归一。
 *  - 参数键合法域 = bai 表情参数表（既有 A3 EmotionControllerParamPathTest fixture 的真实 adapter 8×3×6 键值域）。
 *
 * 契约锁定（C-15 最小语义集）：neutral / joy / sad / angry / surprise 五种均须有参数帧映射；
 * 未知 / null / 空白 → neutral；大小写归一；强度后缀忽略。
 *
 * 封装的待实现生产符号（data/EmotionTagTable.kt，不存在 → 编译期 RED，cil=coder 待实现）：
 *   object EmotionTagTable {
 *       val MINIMAL_SEMANTIC_EMOTIONS: Set<String>
 *       fun frameFor(emotion: String?, fallbackNeutral: Boolean = true): Map<String, Float>
 *       val validParameterKeys: Set<String>
 *       fun isKnownEmotion(emotion: String?): Boolean
 *   }
 */
class EmotionTagTableTest {

    private val C15_MINIMAL = setOf("neutral", "joy", "sad", "angry", "surprise")

    // ═══════════════════════════════════════════════════════════
    // TT-01：C-15 最小语义集完整性（5 种均识别、均有映射）
    // ═══════════════════════════════════════════════════════════

    @Test
    fun TT01_minimalSemanticSet_complete() {
        assertEquals("C-15 最小语义集 == {neutral,joy,sad,angry,surprise}", C15_MINIMAL, EmotionTagTable.MINIMAL_SEMANTIC_EMOTIONS)
        for (emotion in C15_MINIMAL) {
            val frame = EmotionTagTable.frameFor(emotion)
            assertNotNull("$emotion 须有参数帧映射（非 null）", frame)
            assertTrue("$emotion 映射帧非空（不落空）", frame.isNotEmpty())
        }
        assertTrue("neutral 是已知情绪", EmotionTagTable.isKnownEmotion("neutral"))
    }

    // ═══════════════════════════════════════════════════════════
    // TT-02：neutral 映射存在（可空帧或中性基线，非异常）
    // ═══════════════════════════════════════════════════════════

    @Test
    fun TT02_neutral_frameNonNull() {
        val frame = EmotionTagTable.frameFor("neutral")
        assertNotNull("neutral 帧非 null", frame)
        // neutral 可为中性基线（允许空帧——不锁具体值，只锁非 null + 无异常）
        assertTrue("neutral 求值不抛异常", true)
    }

    // ═══════════════════════════════════════════════════════════
    // TT-03：未知 / null / 空白 → neutral（回退）
    // ═══════════════════════════════════════════════════════════

    @Test
    fun TT03_unknownNullBlank_fallBackToNeutral() {
        assertFalse("未知情绪 isKnown=false", EmotionTagTable.isKnownEmotion("foo"))
        assertFalse("null isKnown=false", EmotionTagTable.isKnownEmotion(null))
        val unknownFrame = EmotionTagTable.frameFor("foo")
        assertNotNull("未知回退帧非 null", unknownFrame)
        val neutralFrame = EmotionTagTable.frameFor("neutral")
        assertNotNull(neutralFrame)
        assertEquals("未知情绪帧 == neutral 帧（回退语义）", neutralFrame, unknownFrame)
        // null / 空白也回退 neutral
        assertEquals("null == neutral 帧", neutralFrame, EmotionTagTable.frameFor(null))
        assertEquals("空串 == neutral 帧", neutralFrame, EmotionTagTable.frameFor(""))
        assertEquals("空白 == neutral 帧", neutralFrame, EmotionTagTable.frameFor("   "))
    }

    // ═══════════════════════════════════════════════════════════
    // TT-04：键值合法（帧键 ∈ validParameterKeys）
    // ═══════════════════════════════════════════════════════════

    @Test
    fun TT04_frameKeys_areValidParameterKeys() {
        assertTrue("validParameterKeys 非空（bai 参数表）", EmotionTagTable.validParameterKeys.isNotEmpty())
        for (emotion in C15_MINIMAL) {
            val frame = EmotionTagTable.frameFor(emotion)
            for ((key, value) in frame) {
                assertTrue("$emotion 帧键 $key 须 ∈ 合法参数键域", key in EmotionTagTable.validParameterKeys)
                assertTrue("值须为有限数值", value.isFinite())
                assertTrue("值须在 [0,1] 域（bai 表情参数归一化域）", value in 0f..1f)
            }
        }
    }

    // ═══════════════════════════════════════════════════════════
    // TT-05：大小写归一 + 强度后缀忽略
    // ═══════════════════════════════════════════════════════════

    @Test
    fun TT05_caseInsensitive_strengthSuffixIgnored() {
        assertTrue("JOY 大小写归一识别", EmotionTagTable.isKnownEmotion("JOY"))
        assertTrue("joy 基础识别", EmotionTagTable.isKnownEmotion("joy"))
        assertEquals("joy 帧 == JOY 帧（归一）", EmotionTagTable.frameFor("joy"), EmotionTagTable.frameFor("JOY"))
        // 强度后缀 [joy_3] 忽略数字，取基础 joy（对齐 EmotionText.strengthSuffix_ignored）
        assertTrue("joy_3 强度后缀识别为基础 joy", EmotionTagTable.isKnownEmotion("joy_3"))
        assertEquals("joy_3 帧 == joy 帧", EmotionTagTable.frameFor("joy"), EmotionTagTable.frameFor("joy_3"))
        assertFalse("未知带强度后缀仍未知", EmotionTagTable.isKnownEmotion("hype_3"))
    }
}
