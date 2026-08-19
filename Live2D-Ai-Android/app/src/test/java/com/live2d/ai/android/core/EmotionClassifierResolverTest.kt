package com.live2d.ai.android.core

import com.live2d.ai.android.data.EmotionTagTable
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 分类器→emotion 优先级纯函数测试（wave A：对齐生产 seam）。依据 = contract.refactor-0814
 * §2.1 LlmCompleted.emotion 优先级（分类器结果 > 内嵌标签 > neutral）+ §2.6（失败天然归 null）。
 *
 * 生产 seam（已锁定，纯函数，测试对齐生产）：
 *   fun resolveEmotion(classifierResult: EmotionClassifierResult?, inlineEmotion: String?): String
 * 优先级：
 *   - classifier 非 null 且 label ∈ 已知集 → 返回 label（内嵌不参与）
 *   - classifier 为 null 或 label 集外（失败/无效）→ 返回 inlineEmotion（须已知；neutral 亦可）
 *   - inlineEmotion 空/未知 → neutral
 * 永不返回 null；恒 ∈ MINIMAL_SEMANTIC_EMOTIONS。
 *
 * 对齐映射（旧 RED 形状 → 生产 seam）：
 *   - 旧"label 为 null 的空结果" → 新 seam 无字段级 null；无效结果由 classifierResult=null
 *     （RS-02）或 label 集外（生产 isKnownEmotion 校验回退；RS-03/RS-06）表达
 *   - source 枚举 → "llm"/"neutral" 字符串字面量
 */
class EmotionClassifierResolverTest {

    private fun res(label: String) = EmotionClassifierResult(label, 0.7f, "llm")

    /** RS-01 分类器有 label → 分类器优先（内嵌"更弱"被忽略）。 */
    @Test
    fun resolve_classifierLabelWins_overInline() {
        assertEquals("joy", resolveEmotion(res("joy"), "sad"))
    }

    /** RS-02 classifier 参数为 null → 用内嵌标签。 */
    @Test
    fun resolve_classifierNull_usesInline() {
        assertEquals("sad", resolveEmotion(null, "sad"))
    }

    /** RS-03 classifier label 无效（集外 = 失败/空结果）→ 用内嵌标签。
     *  对齐：旧 seam 字段级 label=null 在新 seam 不存在；无效结果由集外 label 表达
     *  （生产 isKnownEmotion 校验回退内嵌），断言语义"无效结果 → 内嵌"不变。 */
    @Test
    fun resolve_classifierLabelNull_usesInline() {
        assertEquals("sad", resolveEmotion(res("ecstatic"), "sad"))
    }

    /** RS-04 classifier null + inline neutral → neutral。 */
    @Test
    fun resolve_nullClassifierNeutralInline_neutral() {
        assertEquals("neutral", resolveEmotion(null, "neutral"))
    }

    /** RS-05 全无（classifier null + inline null/空）→ neutral，不崩溃、不返 null。 */
    @Test
    fun resolve_nothing_neutral_noNull() {
        assertEquals("neutral", resolveEmotion(null, null))
        assertEquals("neutral", resolveEmotion(null, ""))
    }

    /** RS-06 返回值恒 ∈ 已知集（C-15），永不集外/null。 */
    @Test
    fun resolve_outputAlwaysInKnownSet() {
        val known = EmotionTagTable.MINIMAL_SEMANTIC_EMOTIONS
        val outputs = listOf(
            resolveEmotion(res("joy"), "sad"),
            resolveEmotion(null, "sad"),
            resolveEmotion(res("ecstatic"), "angry"),
            resolveEmotion(null, null),
            resolveEmotion(null, "neutral"),
        )
        for (out in outputs) {
            assertTrue("resolve 返回值 $out 须 ∈ 已知集，且非 null", out in known)
        }
    }
}
