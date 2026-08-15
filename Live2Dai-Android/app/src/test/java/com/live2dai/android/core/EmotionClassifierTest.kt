package com.live2dai.android.core

import com.live2dai.android.data.EmotionTagTable
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 分类器 seam 测试（wave A：对齐生产 seam）。依据 = contract.refactor-0814 §1.3 C-15 +
 * §2.6 失败语义 + search-report-neko-ai-linkage R01/R02（N.E.K.O. 独立情绪 LLM 输出单一情绪标签）。
 *
 * 生产 seam（已锁定，测试对齐生产）：
 *   interface EmotionClassifier { suspend fun classify(text: String): EmotionClassifierResult? }
 *   data class EmotionClassifierResult(label: String, confidence: Float, source: String)  // source ∈ {"llm","inline","neutral"}
 *
 * 对齐映射（旧 RED 形状 → 生产 seam）：
 *   - EmotionClassifierSource.LLM_CLASSIFIER → "llm"；INLINE_TAG_FALLBACK → "inline"；NEUTRAL_FALLBACK → "neutral"
 *   - 旧 label/confidence 可空（字段级 null）→ 生产非空；"空结果"由 classify 返回 null 表达（结果级 null）
 *   - 大小写归一职责在生产 normalizeEmotion / 顶层 resolveEmotion（data class 直接构造不归一）
 *   - confidence 越界钳制（契约锚 1）在 companion 工厂 invoke；构造器公开后直接构造不钳制（二者并存）
 *   - suspend 调用：测试方法体内 runBlocking { ... } 包裹，断言语义不变
 *
 * 本测试只测"接口 + 结果类型"的可测 seam 约定（fake 注入），不测分类器内部网络。
 */
class EmotionClassifierTest {

    // ── 可注入 fake：lambda 返回指定结果或 null（使结果确定可测）────────────────
    private class FakeClassifier(
        private val produce: (String) -> EmotionClassifierResult?,
    ) : EmotionClassifier {
        override suspend fun classify(text: String): EmotionClassifierResult? = produce(text)
    }

    /** EC-01 分类成功返回单一已知标签（∈ MINIMAL_SEMANTIC_EMOTIONS），confidence ∈ [0,1]，source="llm"。 */
    @Test
    fun classify_success_returnsKnownSingleLabel() {
        val c = FakeClassifier { EmotionClassifierResult("joy", 0.9f, "llm") }
        val r = runBlocking { c.classify("很开心") }
        assertNotNull("分类成功结果不得为 null", r)
        val result = r!!
        assertNotNull("分类成功 label 不得为 null（类型系统保证）", result.label)
        assertEquals("joy", result.label)
        assertTrue("label 必须 ∈ C-15 已知集", result.label in EmotionTagTable.MINIMAL_SEMANTIC_EMOTIONS)
        assertNotNull("confidence 不得为 null（类型系统保证）", result.confidence)
        assertTrue("confidence ∈ [0,1]", result.confidence in 0f..1f)
        assertEquals("llm", result.source)
    }

    /** EC-02 未知文本（分类器无判定）→ classify 返回 null（下游 resolve 归 neutral），不抛异常。
     *  对齐：旧 seam 字段级 null（label/confidence=null）→ 新 seam 结果级 null；下游归 neutral 语义经 resolveEmotion 闭环保留。 */
    @Test
    fun classify_unknownText_returnsNullLabel_noThrow() {
        val c = FakeClassifier { null }
        val r = runBlocking { c.classify("无意义 8921") }
        assertNull("未知文本分类结果须为 null（新 seam：字段级 null → 结果级 null）", r)
        assertEquals("下游 resolve 归 neutral（§2.6 失败不崩溃）", "neutral", resolveEmotion(r, null))
    }

    /** EC-03 已知集内每一标签皆可被分类产出（分类器不得产出集外标签的闭环）。 */
    @Test
    fun classify_eachKnownEmotion_inKnownSet() {
        val known = EmotionTagTable.MINIMAL_SEMANTIC_EMOTIONS // {neutral, joy, sad, angry, surprise}
        assertEquals("C-15 最小语义集", setOf("neutral", "joy", "sad", "angry", "surprise"), known)
        for (emotion in known) {
            val c = FakeClassifier { EmotionClassifierResult(emotion, 0.5f, "llm") }
            val r = runBlocking { c.classify("一些文本 $emotion") }
            assertNotNull("分类结果不得为 null", r)
            val result = r!!
            assertEquals("分类产出标签应等于注入值", emotion, result.label)
            assertTrue("$emotion ∈ 已知集", result.label in known)
        }
    }

    /** EC-04 分类器产出大小写变体时归一为小写已知标签。
     *  对齐：生产 data class 直接构造不归一；归一职责在生产 normalizeEmotion（经顶层 resolveEmotion）。
     *  断言语义不变：大小写变体 → 小写已知标签。 */
    @Test
    fun classify_caseVariant_normalizesToLowerKnownLabel() {
        val c = FakeClassifier { EmotionClassifierResult("JOY", 0.8f, "llm") }
        val r = runBlocking { c.classify("大声") }
        assertNotNull("分类结果不得为 null", r)
        val normalized = resolveEmotion(r, null)
        assertEquals("大小写变体归一为小写已知标签", "joy", normalized)
        assertTrue(normalized in EmotionTagTable.MINIMAL_SEMANTIC_EMOTIONS)
    }

    /** EC-05 confidence 越界（>1 / <0）被 clamp 到 [0,1]（§2.6 不崩溃、不产出非法值）。
     *  对齐：构造器公开后直接构造不钳制；钳制契约锚 1 在 companion 工厂（invoke）。
     *  显式调用工厂保留钳制断言（断言语义不变）。 */
    @Test
    fun classify_confidenceOutOfRange_clampedOrGuaranteedValid() {
        val c = FakeClassifier { EmotionClassifierResult.invoke("sad", 1.5f, "llm") }
        val r = runBlocking { c.classify("低落") }
        assertNotNull("分类结果不得为 null", r)
        val result = r!!
        assertNotNull("confidence 不得为 null（类型系统保证）", result.confidence)
        assertTrue("confidence 被 clamp 回 [0,1]", result.confidence in 0f..1f)
    }
}
