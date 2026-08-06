package com.live2dai.android

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * EmotionController.sanitizeForTts 单元测试（task-fix-tts-sanitize）。
 *
 * 缺陷背景（用户真机实测）：TTS 播报读到 "[joy] ... 等于号 ..."——
 * 1. [emotion] 标签（部分路径）被朗读
 * 2. 颜文字 (=^･ω･^=) 被朗读成"等于号"（stripEmotionTags 只剥离 [xxx]，不剥离颜文字）
 *
 * sanitizeForTts 契约（纯函数）：
 * - 剥离 [emotion] 标签（复用 EMOTION_TAG_REGEX）
 * - 剥离"括号内仅含符号"的颜文字段，如 (=^･ω･^=) (≧▽≦)
 * - 剥离无括号简单颜文字（^_^ / >_< / =ω= 等）
 * - 剥离 markdown 加粗/斜体符号（** / __）
 * - **不得误删正常中文括号**（（小声说）等括号内含中文 → 保留）
 */
class EmotionControllerSanitizeTest {

    // ─────────────────────────────────────────────
    // [emotion] 标签剥离
    // ─────────────────────────────────────────────

    @Test
    fun stripsEmotionTag() {
        assertEquals("主人好", EmotionController.sanitizeForTts("[joy] 主人好"))
    }

    // ─────────────────────────────────────────────
    // 颜文字括号段剥离
    // ─────────────────────────────────────────────

    @Test
    fun stripsKaomojiParenSegment() {
        // 括号内仅含符号（无中文/字母/数字）→ 剥离
        assertEquals("", EmotionController.sanitizeForTts("(=^･ω･^=)"))
    }

    @Test
    fun stripsFullWidthKaomojiParenSegment() {
        // 全角括号颜文字
        assertEquals("", EmotionController.sanitizeForTts("(≧▽≦)"))
    }

    @Test
    fun keepsNormalChineseParentheses() {
        // 括号内含中文 → 不得误删（核心防误删用例）
        assertEquals("（小声说）", EmotionController.sanitizeForTts("（小声说）"))
    }

    // ─────────────────────────────────────────────
    // 组合场景（标签 + 颜文字）
    // ─────────────────────────────────────────────

    @Test
    fun stripsTagAndKaomojiTogether() {
        assertEquals("今天开心", EmotionController.sanitizeForTts("[joy] 今天开心 (=^･ω･^=)"))
    }

    // ─────────────────────────────────────────────
    // markdown 符号剥离
    // ─────────────────────────────────────────────

    @Test
    fun stripsMarkdownBoldSymbols() {
        // **xx** 中符号剥离，保留文字
        assertEquals("加粗文字", EmotionController.sanitizeForTts("**加粗**文字"))
    }

    @Test
    fun stripsMarkdownBoldSymbolsChinese() {
        assertEquals("今天真开心", EmotionController.sanitizeForTts("**今天**真开心"))
    }

    // ─────────────────────────────────────────────
    // 无括号简单颜文字
    // ─────────────────────────────────────────────

    @Test
    fun stripsSimpleKaomoji() {
        // ^_^ / >_< / =ω= 等无括号颜文字（直接删除，不留空格）
        assertEquals("心情很好", EmotionController.sanitizeForTts("心情^_^很好"))
        assertEquals("", EmotionController.sanitizeForTts(">_<"))
    }

    // ─────────────────────────────────────────────
    // 不变性：正常内容不受影响
    // ─────────────────────────────────────────────

    @Test
    fun keepsPureChineseTextUnchanged() {
        assertEquals("今天天气真好呀", EmotionController.sanitizeForTts("今天天气真好呀"))
    }

    @Test
    fun keepsMixedChineseAndPunctuation() {
        // 中文 + 正常标点（含半角/全角括号、句号）→ 原样保留
        assertEquals("喵呜（好开心）！", EmotionController.sanitizeForTts("喵呜（好开心）！"))
    }

    // ─────────────────────────────────────────────
    // 边界：空输入 / 空白
    // ─────────────────────────────────────────────

    @Test
    fun emptyInputReturnsEmpty() {
        assertEquals("", EmotionController.sanitizeForTts(""))
    }

    @Test
    fun blankInputReturnsEmpty() {
        assertEquals("", EmotionController.sanitizeForTts("   "))
    }
}
