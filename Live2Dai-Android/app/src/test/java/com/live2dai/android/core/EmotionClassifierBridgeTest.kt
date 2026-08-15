package com.live2dai.android.core

import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 分类器→emotion 兜底语义测试（wave A：对齐生产 seam）。依据 = contract.refactor-0814
 * §2.1 / §2.2 / §2.6（分类失败异常/超时/空 → 回退 EmotionText.parse 内嵌标签；两者皆无 → neutral）。
 *
 * 生产 seam（已锁定，测试对齐生产）：
 *   class EmotionClassifierBridge(classifier: EmotionClassifier) {
 *       /** 返回 emotion ∈ 已知集。classify 空（null）→ 回退内嵌标签；再没有 → neutral。 */
 *       suspend fun emotionFor(text: String): String
 *   }
 *
 * 兜底语义（优先级）：分类器结果 > EmotionText.parse(text).emotion（内嵌标签）> neutral
 *
 * 对齐映射（旧 RED 形状 → 生产 seam）：
 *   - 旧"空结果（label=null）" → 新 seam classify 返回 null（fake produce 可空）
 *   - 旧"分类器抛异常桥兜底" → 生产 EmotionClassifier 实现契约"异常/超时 → null，绝不抛"
 *     （异常由 classifier 实现内吞，桥只见 null）；fake 遵守契约以 null 表达失败，回退断言不变
 *   - emotionFor(null) → 参数非空 String，null 输入不在 seam 域；空串 → neutral 断言保留
 *   - suspend 调用：测试方法体内 runBlocking { ... } 包裹，断言语义不变
 *
 * class 唯一；本测试只注 fake classifier 驱动确定性结果，不测分类器内部网络/超时实现。
 */
class EmotionClassifierBridgeTest {

    // 可注入 fake：lambda 返回结果 / null（失败），或抛异常（由生产契约保证实现不得穿越）
    private class FakeClassifier(
        private val produce: (String) -> EmotionClassifierResult?,
    ) : EmotionClassifier {
        override suspend fun classify(text: String): EmotionClassifierResult? = produce(text)
    }

    // ── 接 5.2 用例清单 ────────────────────────────────

    /** BC-01 分类成功 → 分类器标签优先（内嵌标签被忽略）。 */
    @Test
    fun emotionFor_classifierSucceeds_classifierLabelWins() {
        val bridge = EmotionClassifierBridge(
            FakeClassifier { EmotionClassifierResult("joy", 0.9f, "llm") }
        )
        assertEquals("joy", runBlocking { bridge.emotionFor("[sad] 正文") })
    }

    /** BC-02 分类失败-异常 → 回退 EmotionText.parse 内嵌标签；不抛异常。
     *  对齐：生产 EmotionClassifier 实现契约"异常/超时 → null，绝不抛"——异常在实现内吞，
     *  fake 以 null 表达失败（桥不 try/catch，见生产实现）；回退断言不变。 */
    @Test
    fun emotionFor_classifierThrows_fallsBackToInlineTag() {
        val bridge = EmotionClassifierBridge(
            FakeClassifier { null } // 生产契约：分类失败（异常/超时）→ null，绝不抛
        )
        assertEquals("sad", runBlocking { bridge.emotionFor("[sad] 正文") })
    }

    /** BC-03 分类失败-空结果（classify 返回 null）→ 回退内嵌标签。 */
    @Test
    fun emotionFor_classifierEmptyResult_fallsBackToInlineTag() {
        val bridge = EmotionClassifierBridge(
            FakeClassifier { null }
        )
        assertEquals("sad", runBlocking { bridge.emotionFor("[sad] 正文") })
    }

    /** BC-04 分类空结果 + 内嵌强度后缀 [joy_3] → 忽略后缀归 joy。 */
    @Test
    fun emotionFor_classifierEmpty_inlineStrengthSuffix_normalized() {
        val bridge = EmotionClassifierBridge(
            FakeClassifier { null }
        )
        assertEquals("joy", runBlocking { bridge.emotionFor("[joy_3] 好") })
    }

    /** BC-05 分类空结果 + 内嵌未知标签 → neutral（未知不参与判定）。 */
    @Test
    fun emotionFor_classifierEmpty_inlineUnknownTag_neutral() {
        val bridge = EmotionClassifierBridge(
            FakeClassifier { null }
        )
        assertEquals("neutral", runBlocking { bridge.emotionFor("[foo] 文本") })
    }

    /** BC-06 分类空结果 + 无内嵌标签纯文本 → neutral。 */
    @Test
    fun emotionFor_noClassiferNoInline_neutral() {
        val bridge = EmotionClassifierBridge(
            FakeClassifier { null }
        )
        assertEquals("neutral", runBlocking { bridge.emotionFor("无标签纯文本") })
    }

    /** BC-07 分类空结果 + 空输入 → neutral（不崩溃）。
     *  对齐：emotionFor 参数非空 String，null 输入不在生产 seam 域（生产 isBlank 空串 → neutral），
     *  删 null 调用；空串断言保留。 */
    @Test
    fun emotionFor_classifierEmptyEmptyInput_neutral_noThrow() {
        val bridge = EmotionClassifierBridge(
            FakeClassifier { null }
        )
        assertEquals("neutral", runBlocking { bridge.emotionFor("") })
    }
}
