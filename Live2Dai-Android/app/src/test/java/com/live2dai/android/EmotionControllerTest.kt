package com.live2dai.android

import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test
import org.mockito.kotlin.any
import org.mockito.kotlin.mock
import org.mockito.kotlin.never
import org.mockito.kotlin.verify

/**
 * Tests for EmotionController.
 *
 * Key behaviors from the implementation:
 * - parseAndApply() iterates all [emotion] tags but ONLY calls setExpression()
 *   ONCE at the end with the LAST valid emotion tag.
 * - setExpression is only called if expressionIndex != currentExpressionIndex.
 * - currentExpressionIndex starts at 0 (neutral).
 * - Tag regex removes only the [tag] marker, leaving surrounding whitespace.
 *
 * Emotion index mapping:
 *   neutral → 0, fear → 1, sadness → 2, anger → 3,
 *   disgust → 4, joy → 5, smirk → 6, surprise → 7
 */
class EmotionControllerTest {

    private lateinit var emotionController: EmotionController
    private lateinit var mockLive2DView: Live2DView

    @Before
    fun setUp() {
        mockLive2DView = mock<Live2DView>()
        // A3 双路径（contract-bai-params-v0.md AP-03/AP-28 + contract-R1-model §2.3）：本测试用 mock 强制
        // hasExpressions()=true 跑「索引路径」——bai 无 exp3，真实路径是 PARAM_FRAME；
        // EXPRESSION_INDEX 是保留的防御分支（行为零变化红线，不可达但保留），此处为回归门人工触发。
        // 必须明确 hasExpressions()=true，否则 mock 默认 boolean=false 会错误走参数路径（PARAM_FRAME）而非 setExpression。
        // 这是 A3 添加双路径后，随查询时机（构造或首次调用）与否，替代指数路径时均可正确。
        org.mockito.Mockito.`when`(mockLive2DView.hasExpressions()).thenReturn(true)
        emotionController = EmotionController(mockLive2DView)
    }

    @Test
    fun testJoyTagStrippedAndExpressionSet() {
        val input = "[joy] 今天心情真好！"
        val result = emotionController.parseAndApply(input)
        assertEquals("今天心情真好！", result)
        verify(mockLive2DView).setExpression(5) // joy → index 5
    }

    @Test
    fun testNeutralTagStrippedAndExpressionSet() {
        // First set to a non-neutral emotion so neutral triggers a change
        emotionController.parseAndApply("[joy] init")

        val input = "[neutral] 白也很开心！(=^･ω･^=)"
        val result = emotionController.parseAndApply(input)
        assertEquals("白也很开心！(=^･ω･^=)", result)
        verify(mockLive2DView).setExpression(0) // neutral → index 0
    }

    @Test
    fun testMultipleEmotionTagsLastWins() {
        val input = "[joy] 今天天气真好！[anger] 不许偷懒！[neutral] 白去倒茶~"
        val result = emotionController.parseAndApply(input)
        // Tags are stripped by regex, leaving whitespace where tags were
        assertEquals("今天天气真好！ 不许偷懒！ 白去倒茶~", result)
        // Only the last valid emotion tag triggers setExpression
        // neutral(0) == currentExpressionIndex(0) initially, so NOT called
        verify(mockLive2DView, never()).setExpression(any(), any())
    }

    @Test
    fun testNoEmotionTags() {
        val input = "这是一条没有表情标签的普通消息"
        val result = emotionController.parseAndApply(input)
        assertEquals("这是一条没有表情标签的普通消息", result)
        verify(mockLive2DView, never()).setExpression(any(), any())
    }

    @Test
    fun testUnknownEmotionTag() {
        val input = "[unknown] 这个标签不存在~"
        val result = emotionController.parseAndApply(input)
        assertEquals("这个标签不存在~", result)
        verify(mockLive2DView, never()).setExpression(any(), any())
    }

    @Test
    fun testMixedTextAndTags() {
        val input = "你好[joy]！今天天气真好[smirk]~ 白很高兴[neutral]！"
        val result = emotionController.parseAndApply(input)
        assertEquals("你好！今天天气真好~ 白很高兴！", result)
        // Only last valid emotion (neutral → 0) triggers setExpression
        // 0 == currentExpressionIndex(0) initially → NOT called
        verify(mockLive2DView, never()).setExpression(any(), any())
    }

    @Test
    fun testEmptyString() {
        val input = ""
        val result = emotionController.parseAndApply(input)
        assertEquals("", result)
        verify(mockLive2DView, never()).setExpression(any(), any())
    }

    @Test
    fun testOnlyEmotionTagsWithStateChange() {
        // First set to joy so the last tag (neutral) triggers a change
        emotionController.parseAndApply("[joy] init")

        val input = "[joy][anger][neutral]"
        val result = emotionController.parseAndApply(input)
        assertEquals("", result)
        // lastValidEmotion = neutral → 0 ≠ currentExpressionIndex (5) → calls setExpression(0)
        verify(mockLive2DView).setExpression(0)
    }

    @Test
    fun testStaticStripEmotionTags() {
        val text = "[joy] 你好~ [sadness] 不开心"
        val result = EmotionController.stripEmotionTags(text)
        // stripEmotionTags removes only [tag] markers, leaving surrounding whitespace
        assertEquals("你好~  不开心", result)
    }

    @Test
    fun testSetEmotionDirectly() {
        emotionController.setEmotion("anger")
        verify(mockLive2DView).setExpression(3) // anger → index 3
    }

    @Test
    fun testResetToNeutral() {
        emotionController.parseAndApply("[joy] test")
        emotionController.resetToNeutral()
        verify(mockLive2DView).setExpression(5) // joy
        verify(mockLive2DView).setExpression(0) // neutral from reset
    }

    @Test
    fun testGetCurrentExpressionIndex() {
        assertEquals(0, emotionController.getCurrentExpressionIndex())
        emotionController.parseAndApply("[surprise] 哇！")
        assertEquals(7, emotionController.getCurrentExpressionIndex())
    }

    @Test
    fun testNonBlankTextWithNoTagsReturnsSameText() {
        val input = "普通文本，没有标签"
        val result = emotionController.parseAndApply(input)
        assertEquals("普通文本，没有标签", result)
        verify(mockLive2DView, never()).setExpression(any(), any())
    }

    @Test
    fun testMultipleInvocationsChangeState() {
        emotionController.parseAndApply("[joy] 开心")
        verify(mockLive2DView).setExpression(5)

        emotionController.parseAndApply("[sadness] 难过")
        verify(mockLive2DView).setExpression(2)

        emotionController.parseAndApply("[surprise] 惊讶")
        verify(mockLive2DView).setExpression(7)
    }
}
