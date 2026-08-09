package com.live2dai.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.mockito.kotlin.any
import org.mockito.kotlin.mock
import org.mockito.kotlin.never
import org.mockito.kotlin.verify

/**
 * EmotionController 结构化表情 JSON 解析测试（M2 parseStructured / applyStructured / stripStructuredJson）。
 *
 * 对齐蓝本：EmotionControllerTest.kt（Mockito mock Live2DView + EmotionController(mockView) 构造风格）。
 * 覆盖契约：
 *   M1 §7.1 A1-A13（parseStructured 解析/校验）
 *   M2 §3 S1-S10（双端一致性样本 Android 侧）
 *   M2 §1.4 stripStructuredJson
 *   M2 §1.3(d) motionTrigger no-op
 */
class EmotionControllerStructuredTest {

    private lateinit var emotionController: EmotionController
    private lateinit var mockLive2DView: Live2DView

    @Before
    fun setUp() {
        mockLive2DView = mock<Live2DView>()
        emotionController = EmotionController(mockLive2DView)
        // 注入参数白名单以便测试 parameterOverrides 校验（测试环境无 renderer）
        emotionController.parameterWhitelist = setOf("ParamEyeLOpen", "ParamMouthForm")
    }

    // ══════════════════════════════════════════════
    // 分区 A — M1 §7.1 A1-A13：parseStructured 解析/校验
    // ══════════════════════════════════════════════

    @Test
    fun A1_fenceJson_parses() {
        val input = "```json\n{\"expressionMix\":{\"expressions\":[{\"name\":\"joy\",\"weight\":0.7}]}}\n```"
        val result = emotionController.parseStructured(input)
        assertNotNull("fence 包裹的合法 JSON 应解析成功", result)
        assertEquals(1, result!!.expressionMix.size)
        assertEquals("joy", result.expressionMix[0].name)
        assertEquals(0.7f, result.expressionMix[0].weight)
        assertTrue(result.parameterOverrides.isEmpty())
        assertNull(result.motionLabel)
    }

    @Test
    fun A2_fenceWithoutJsonMarker() {
        val input = "```\n{\"expressionMix\":{\"expressions\":[{\"name\":\"joy\",\"weight\":0.7}]}}\n```"
        val result = emotionController.parseStructured(input)
        assertNotNull("fence 无 json 标记也应解析成功", result)
        assertEquals(1, result!!.expressionMix.size)
        assertEquals("joy", result.expressionMix[0].name)
    }

    @Test
    fun A3_nakedJson_withPrefix() {
        val input = "好的！{\"expressionMix\":{\"expressions\":[{\"name\":\"joy\",\"weight\":0.7}]}}"
        val result = emotionController.parseStructured(input)
        assertNotNull("正文前缀不影响裸 JSON 解析", result)
        assertEquals(1, result!!.expressionMix.size)
        assertEquals("joy", result.expressionMix[0].name)
    }

    @Test
    fun A4_malformedJson() {
        val input = "{\"expressionMix\": {"
        val result = emotionController.parseStructured(input)
        assertNull("畸形 JSON 应返回 null", result)
    }

    @Test
    fun A5_noJson() {
        val input = "[joy] 你好喵~！"
        val result = emotionController.parseStructured(input)
        assertNull("无 JSON 应返回 null", result)
    }

    @Test
    fun A6_unknownName() {
        val input = "{\"expressionMix\":{\"expressions\":[{\"name\":\"love\",\"weight\":1.0}]}}"
        val result = emotionController.parseStructured(input)
        assertNotNull("name 不在 8 key → 丢弃该项但不降级（非 null）", result)
        assertTrue(result!!.expressionMix.isEmpty())
    }

    @Test
    fun A7_weightOutOfRange() {
        val input = "{\"expressionMix\":{\"expressions\":[{\"name\":\"joy\",\"weight\":1.5}]}}"
        val result = emotionController.parseStructured(input)
        assertNotNull("weight > 1 → 丢弃该项但不降级（非 null）", result)
        assertTrue(result!!.expressionMix.isEmpty())
    }

    @Test
    fun A8_weightSumOverOne() {
        val input = "{\"expressionMix\":{\"expressions\":[{\"name\":\"joy\",\"weight\":0.7},{\"name\":\"surprise\",\"weight\":0.6}]}}"
        val result = emotionController.parseStructured(input)
        assertNotNull(result)
        assertEquals(2, result!!.expressionMix.size)
        assertEquals("joy", result.expressionMix[0].name)
        assertEquals("surprise", result.expressionMix[1].name)
        // 归一化：0.7/1.3=0.53846..., 0.6/1.3=0.46153...
        assertEquals(0.53846f, result.expressionMix[0].weight, 1e-3f)
        assertEquals(0.46154f, result.expressionMix[1].weight, 1e-3f)
        // 和=1.0（容差 1e-3 Float32）
        val sum = result.expressionMix.sumOf { it.weight.toDouble() }
        assertEquals(1.0, sum, 1e-3)
    }

    @Test
    fun A9_paramOverride_validKey() {
        val input = "{\"parameterOverrides\":{\"ParamEyeLOpen\":1.2}}"
        val result = emotionController.parseStructured(input)
        assertNotNull(result)
        assertTrue(result!!.expressionMix.isEmpty())
        assertEquals(mapOf("ParamEyeLOpen" to 1.2f), result.parameterOverrides)
    }

    @Test
    fun A10_paramOverride_invalidKey() {
        val input = "{\"parameterOverrides\":{\"NotAParam\":1.0}}"
        val result = emotionController.parseStructured(input)
        assertNotNull("非法 key → 丢弃该 key；仍有合法字段 → 非 null", result)
        assertTrue(result!!.parameterOverrides.isEmpty())
    }

    @Test
    fun A11_motionLabel_unknown() {
        val input = "{\"motionLabel\":\"dance\"}"
        val result = emotionController.parseStructured(input)
        assertNotNull("motionLabel 不在别名表 → 置 null；仍有合法字段", result)
        assertNull(result!!.motionLabel)
    }

    @Test
    fun A12_allFields() {
        val input = "{\"expressionMix\":{\"expressions\":[{\"name\":\"joy\",\"weight\":0.7},{\"name\":\"surprise\",\"weight\":0.3}]},\"parameterOverrides\":{\"ParamEyeLOpen\":1.2},\"motionLabel\":\"wave\"}"
        val result = emotionController.parseStructured(input)
        assertNotNull(result)
        assertEquals(2, result!!.expressionMix.size)
        assertEquals("joy", result.expressionMix[0].name)
        assertEquals(0.7f, result.expressionMix[0].weight)
        assertEquals("surprise", result.expressionMix[1].name)
        assertEquals(0.3f, result.expressionMix[1].weight)
        assertEquals(mapOf("ParamEyeLOpen" to 1.2f), result.parameterOverrides)
        assertEquals("wave", result.motionLabel)
    }

    @Test
    fun A13_emptyMix_validOverride() {
        val input = "{\"expressionMix\":{\"expressions\":[]},\"parameterOverrides\":{\"ParamMouthForm\":0.8}}"
        val result = emotionController.parseStructured(input)
        assertNotNull(result)
        assertTrue(result!!.expressionMix.isEmpty())
        assertEquals(mapOf("ParamMouthForm" to 0.8f), result.parameterOverrides)
        assertNull(result.motionLabel)
    }

    // ══════════════════════════════════════════════
    // 分区 B — M2 §3 S1-S10：双端一致性样本
    // ══════════════════════════════════════════════

    @Test
    fun S1_fence_full() {
        val input = "```json\n{\"expressionMix\":{\"expressions\":[{\"name\":\"joy\",\"weight\":0.7},{\"name\":\"surprise\",\"weight\":0.3}]},\"parameterOverrides\":{\"ParamEyeLOpen\":1.2},\"motionLabel\":\"wave\"}\n```\n主人回来啦喵~！"
        val result = emotionController.parseStructured(input)
        assertNotNull(result)
        assertEquals(2, result!!.expressionMix.size)
        assertEquals(mapOf("ParamEyeLOpen" to 1.2f), result.parameterOverrides)
        assertEquals("wave", result.motionLabel)
    }

    @Test
    fun S2_english_minimal() {
        val input = "I'm so happy! {\"expressionMix\":{\"expressions\":[{\"name\":\"joy\",\"weight\":1.0}]}}"
        val result = emotionController.parseStructured(input)
        assertNotNull(result)
        assertEquals(1, result!!.expressionMix.size)
        assertEquals("joy", result.expressionMix[0].name)
        assertEquals(1.0f, result.expressionMix[0].weight)
        assertTrue(result.parameterOverrides.isEmpty())
        assertNull(result.motionLabel)
    }

    @Test
    fun S3_fence_noMarker() {
        val input = "```\n{\"expressionMix\":{\"expressions\":[{\"name\":\"sadness\",\"weight\":0.8}]}}\n```"
        val result = emotionController.parseStructured(input)
        assertNotNull(result)
        assertEquals(1, result!!.expressionMix.size)
        assertEquals("sadness", result.expressionMix[0].name)
        assertEquals(0.8f, result.expressionMix[0].weight)
    }

    @Test
    fun S4_mixedKeywordAndJson() {
        val input = "[joy] 你好喵~！{\"expressionMix\":{\"expressions\":[{\"name\":\"anger\",\"weight\":0.6}]}}"
        val result = emotionController.parseStructured(input)
        assertNotNull("JSON 优先，应成功解析", result)
        assertEquals(1, result!!.expressionMix.size)
        assertEquals("anger", result.expressionMix[0].name)
    }

    @Test
    fun S5_pureKeyword() {
        val input = "[joy] 主人回来啦喵~！"
        val result = emotionController.parseStructured(input)
        assertNull("纯 [keyword] 无 JSON → null → 走降级路径", result)
    }

    @Test
    fun S6_malformed() {
        val input = "{\"expressionMix\": {\"expressions\": ["
        val result = emotionController.parseStructured(input)
        assertNull(result)
    }

    @Test
    fun S7_unknownPlusOutOfRange() {
        val input = "{\"expressionMix\":{\"expressions\":[{\"name\":\"love\",\"weight\":1.0},{\"name\":\"joy\",\"weight\":1.5}]}}"
        val result = emotionController.parseStructured(input)
        assertNotNull(result)
        assertTrue(result!!.expressionMix.isEmpty())
    }

    @Test
    fun S8_normalize() {
        val input = "{\"expressionMix\":{\"expressions\":[{\"name\":\"joy\",\"weight\":0.7},{\"name\":\"surprise\",\"weight\":0.6}]}}"
        val result = emotionController.parseStructured(input)
        assertNotNull(result)
        assertEquals(2, result!!.expressionMix.size)
        assertEquals(0.53846f, result.expressionMix[0].weight, 1e-4f)
        assertEquals(0.46154f, result.expressionMix[1].weight, 1e-4f)
    }

    @Test
    fun S9_onlyInvalidOverride() {
        val input = "{\"parameterOverrides\":{\"NotAParam\":1.0}}"
        val result = emotionController.parseStructured(input)
        assertNotNull(result)
        assertTrue(result!!.expressionMix.isEmpty())
        assertTrue(result.parameterOverrides.isEmpty())
    }

    @Test
    fun S10_emptyMix_partial() {
        val input = "{\"expressionMix\":{\"expressions\":[]},\"parameterOverrides\":{\"ParamMouthForm\":0.8},\"motionLabel\":\"dance\"}"
        val result = emotionController.parseStructured(input)
        assertNotNull(result)
        assertTrue(result!!.expressionMix.isEmpty())
        assertEquals(mapOf("ParamMouthForm" to 0.8f), result.parameterOverrides)
        assertNull("dance 不在别名表 → motionLabel=null", result.motionLabel)
    }

    // ══════════════════════════════════════════════
    // 分区 C — stripStructuredJson（M2 §1.4）
    // ══════════════════════════════════════════════

    @Test
    fun C1_stripFence() {
        val input = "```json\n{\"expressionMix\":{\"expressions\":[{\"name\":\"joy\",\"weight\":0.7}]}}\n```\n主人回来啦喵~！"
        val cleaned = emotionController.stripStructuredJson(input)
        assertFalse("输出不含 expressionMix", cleaned.contains("expressionMix"))
        assertFalse("输出不含 parameterOverrides", cleaned.contains("parameterOverrides"))
        assertFalse("输出不含 motionLabel", cleaned.contains("motionLabel"))
        assertTrue("正文保留", cleaned.contains("主人回来啦喵"))
    }

    @Test
    fun C2_stripNaked() {
        val input = "I'm so happy! {\"expressionMix\":{\"expressions\":[{\"name\":\"joy\",\"weight\":1.0}]}}"
        val cleaned = emotionController.stripStructuredJson(input)
        assertTrue("输出以 I'm so happy 开头", cleaned.startsWith("I'm so happy!"))
        assertFalse("不含 JSON 字段", cleaned.contains("expressionMix"))
        assertFalse("不含花括号", cleaned.contains("{"))
    }

    @Test
    fun C3_stripNone() {
        val input = "[joy] 主人回来啦喵~！"
        val cleaned = emotionController.stripStructuredJson(input)
        assertEquals("无 JSON → 原样返回", input, cleaned)
    }

    // ══════════════════════════════════════════════
    // 分区 D — motionTrigger 接缝 no-op（M2 §1.3(d)）
    // ══════════════════════════════════════════════

    @Test
    fun D1_motionTriggerNull_noop() {
        // motionTrigger 未接线（null）→ applyStructured 不抛异常
        val result = EmotionResult(
            expressionMix = listOf(ExpressionWeight("joy", 1.0f)),
            parameterOverrides = emptyMap(),
            motionLabel = "wave"
        )
        // motionTrigger 默认 null，调用 applyStructured 应不抛异常
        emotionController.applyStructured(result)
        // 验证 setExpressionMix 被调用（表情部分正常）
        verify(mockLive2DView).setExpressionMix(any(), any())
    }

    @Test
    fun A10_mixedValidAndInvalid_overrides() {
        val input = "{\"parameterOverrides\":{\"NotAParam\":1.0,\"ParamEyeLOpen\":0.5}}"
        val result = emotionController.parseStructured(input)
        assertNotNull(result)
        // NotAParam 丢弃，ParamEyeLOpen 保留
        assertEquals(mapOf("ParamEyeLOpen" to 0.5f), result!!.parameterOverrides)
    }
}
