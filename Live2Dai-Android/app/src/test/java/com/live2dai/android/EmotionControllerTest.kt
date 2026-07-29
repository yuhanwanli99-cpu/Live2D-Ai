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
        emotionController = EmotionController(mockLive2DView)
    }

    @Test
    fun testJoyTagStrippedAndExpressionSet() {
        val input = "[joy] 主人今天心情真好喵~！"
        val result = emotionController.parseAndApply(input)
        assertEquals("主人今天心情真好喵~！", result)
        verify(mockLive2DView).setExpression(5) // joy → index 5
    }

    @Test
    fun testNeutralTagStrippedAndExpressionSet() {
        // First set to a non-neutral emotion so neutral triggers a change
        emotionController.parseAndApply("[joy] init")

        val input = "[neutral] 小喵也很开心喵！(=^･ω･^=)"
        val result = emotionController.parseAndApply(input)
        assertEquals("小喵也很开心喵！(=^･ω･^=)", result)
        verify(mockLive2DView).setExpression(0) // neutral → index 0
    }

    @Test
    fun testMultipleEmotionTagsLastWins() {
        val input = "[joy] 主人好呀喵~！[anger] 不许偷懒喵！[neutral] 小喵去倒茶~"
        val result = emotionController.parseAndApply(input)
        // Tags are stripped by regex, leaving whitespace where tags were
        assertEquals("主人好呀喵~！ 不许偷懒喵！ 小喵去倒茶~", result)
        // Only the last valid emotion tag triggers setExpression
        // neutral(0) == currentExpressionIndex(0) initially, so NOT called
        verify(mockLive2DView, never()).setExpression(any())
    }

    @Test
    fun testNoEmotionTags() {
        val input = "这是一条没有表情标签的普通消息"
        val result = emotionController.parseAndApply(input)
        assertEquals("这是一条没有表情标签的普通消息", result)
        verify(mockLive2DView, never()).setExpression(any())
    }

    @Test
    fun testUnknownEmotionTag() {
        val input = "[unknown] 这个标签不存在喵~"
        val result = emotionController.parseAndApply(input)
        assertEquals("这个标签不存在喵~", result)
        verify(mockLive2DView, never()).setExpression(any())
    }

    @Test
    fun testMixedTextAndTags() {
        val input = "主人好[joy]！今天天气真好[smirk]~ 小喵很高兴[neutral]！"
        val result = emotionController.parseAndApply(input)
        assertEquals("主人好！今天天气真好~ 小喵很高兴！", result)
        // Only last valid emotion (neutral → 0) triggers setExpression
        // 0 == currentExpressionIndex(0) initially → NOT called
        verify(mockLive2DView, never()).setExpression(any())
    }

    @Test
    fun testEmptyString() {
        val input = ""
        val result = emotionController.parseAndApply(input)
        assertEquals("", result)
        verify(mockLive2DView, never()).setExpression(any())
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
        val text = "[joy] 你好喵~ [sadness] 不开心"
        val result = EmotionController.stripEmotionTags(text)
        // stripEmotionTags removes only [tag] markers, leaving surrounding whitespace
        assertEquals("你好喵~  不开心", result)
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
        verify(mockLive2DView, never()).setExpression(any())
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
