package com.live2d.ai.android.core

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * emotion 文本标签纯函数测试（对齐生产 `core/EmotionText.kt`，生产锁定不可改）。
 *
 * 生产语义（本测试逐条锁定，不得反向要求生产改）：
 *  - parse(text): EmotionResult —— 提取标签：
 *      * 标签形状 `[keyword]` / `[keyword_数字]`（强度后缀忽略，仅取 keyword）；
 *      * 多个有效标签取**最后一个**（last-wins）；
 *      * 大小写不敏感（keyword lowercased）；
 *      * 未知标签不参与情绪判定（无任何有效标签 → emotion="neutral"），
 *        但 cleanText 中所有标签形状（含未知）全部剥离；
 *      * null/空白 → EmotionResult("neutral", trim 后文本)。
 *  - strip(text): 剥离全部 `[...]` 形状标签 + trim；null → ""；空白文本 → ""。
 *  - sanitize(text): strip → 小写 → 连续空白折叠为单空格（再 trim）。
 *  - SUPPORTED_EMOTIONS 含 angry（angry 是已知标签）。
 *
 * seam（生产 object，同包）：
 *   object EmotionText {
 *       val SUPPORTED_EMOTIONS: Set<String>
 *       fun parse(text: String?): EmotionResult   // emotion + cleanText
 *       fun strip(text: String?): String
 *       fun sanitize(text: String?): String
 *   }
 */
class EmotionTextTest {

    // ═══════════════════════════════════════════════
    // parse：EmotionResult(emotion, cleanText)；最后一个有效标签 last-wins
    // ═══════════════════════════════════════════════

    /** "[joy] 你好呀" → emotion=joy，cleanText=你好呀。 */
    @Test
    fun parse_extractsKnownTag_joy() {
        val r = EmotionText.parse("[joy] 你好呀")
        assertEquals("joy", r.emotion)
        assertEquals("你好呀", r.cleanText)
    }

    /** "[sad] 有点难过" → emotion=sad，cleanText=有点难过。 */
    @Test
    fun parse_extractsKnownTag_sad() {
        val r = EmotionText.parse("[sad] 有点难过")
        assertEquals("sad", r.emotion)
        assertEquals("有点难过", r.cleanText)
    }

    /** "[neutral] 平淡回复" → emotion=neutral，cleanText=平淡回复。 */
    @Test
    fun parse_extractsKnownTag_neutral() {
        val r = EmotionText.parse("[neutral] 平淡回复")
        assertEquals("neutral", r.emotion)
        assertEquals("平淡回复", r.cleanText)
    }

    /** "[angry] ..." → angry 是已知标签（∈ SUPPORTED_EMOTIONS），必须识别为 angry。 */
    @Test
    fun parse_extractsKnownTag_angry() {
        val r = EmotionText.parse("[angry] 你小心点")
        assertEquals("angry", r.emotion)
        assertEquals("你小心点", r.cleanText)
    }

    /** 强度后缀 [keyword_数字] 忽略数字，仅取 keyword。 */
    @Test
    fun parse_strengthSuffix_ignored() {
        assertEquals("joy", EmotionText.parse("[joy_3] 好").emotion)
        assertEquals("好", EmotionText.parse("[joy_3] 好").cleanText)
        assertEquals("angry", EmotionText.parse("[angry_10] 哼").emotion)
    }

    /** 无标签纯文本 → neutral（回退），cleanText 原样。 */
    @Test
    fun parse_noTag_returnsNeutral() {
        val r = EmotionText.parse("今天天气很好")
        assertEquals("neutral", r.emotion)
        assertEquals("今天天气很好", r.cleanText)
    }

    /** 空/null 输入 → neutral + 空串。 */
    @Test
    fun parse_emptyOrNull_returnsNeutral() {
        assertEquals("neutral", EmotionText.parse("").emotion)
        assertEquals("", EmotionText.parse("").cleanText)
        assertEquals("neutral", EmotionText.parse(null).emotion)
        assertEquals("", EmotionText.parse(null).cleanText)
    }

    /** 仅空白输入 → neutral + trim 后空串。 */
    @Test
    fun parse_blank_returnsNeutral() {
        val r = EmotionText.parse("   ")
        assertEquals("neutral", r.emotion)
        assertEquals("", r.cleanText)
    }

    /** 未知标签（[foo] ∉ SUPPORTED_EMOTIONS）→ 不参与判定 → neutral，但 cleanText 已剥离。 */
    @Test
    fun parse_unknownTag_ignoredReturnsNeutral() {
        val r = EmotionText.parse("[foo] 你小心点")
        assertEquals("neutral", r.emotion)
        assertEquals("你小心点", r.cleanText)
    }

    /** 未知标签不遮蔽后续已知标签：忽略 [foo]，命中 [joy]。 */
    @Test
    fun parse_unknownTagSkipped_findsNextKnownTag() {
        val r = EmotionText.parse("[foo] 但是[joy] 很开心")
        assertEquals("joy", r.emotion)
        assertEquals("但是 很开心", r.cleanText)
    }

    /** 多个已知标签并存 → 取最后一个（last-wins：[sad] 覆盖 [joy]）。 */
    @Test
    fun parse_multipleKnownTags_takesLast() {
        val r = EmotionText.parse("[joy] 哈哈哈 [sad] 后面再说")
        assertEquals("sad", r.emotion)
        assertEquals("哈哈哈  后面再说", r.cleanText)
    }

    /** 嵌套括号场景：正则扁平匹配（不递归），"[joy [sad]]" 中命中内层 [sad]。 */
    @Test
    fun parse_nestedTag_findsInnerTag() {
        val r = EmotionText.parse("[joy [sad]] 古怪")
        assertEquals("sad", r.emotion)
        assertEquals("[joy ] 古怪", r.cleanText)
    }

    /** 标签大小写变体 → 小写归一后识别。 */
    @Test
    fun parse_tagCaseInsensitive() {
        assertEquals("joy", EmotionText.parse("[JOY] 大声").emotion)
        assertEquals("大声", EmotionText.parse("[JOY] 大声").cleanText)
        assertEquals("sad", EmotionText.parse("[Sad] 低沉").emotion)
        assertEquals("低沉", EmotionText.parse("[Sad] 低沉").cleanText)
        assertEquals("neutral", EmotionText.parse("[NeUtRaL] 平淡").emotion)
        assertEquals("平淡", EmotionText.parse("[NeUtRaL] 平淡").cleanText)
    }

    /** 标签前带前导空白 → 仍识别，cleanText 被 trim。 */
    @Test
    fun parse_leadingWhitespaceBeforeTag_stillRecognized() {
        val r = EmotionText.parse("   [joy] 你好")
        assertEquals("joy", r.emotion)
        assertEquals("你好", r.cleanText)
    }

    // ═══════════════════════════════════════════════
    // strip：剥离全部 [tag] 形状（含未知）+ trim
    // ═══════════════════════════════════════════════

    /** "[joy] 今日开心" → "今日开心"。 */
    @Test
    fun strip_removesKnownTag() {
        assertEquals("今日开心", EmotionText.strip("[joy] 今日开心"))
    }

    /** 未知标签同样剥离（标签形状即剥离，不判已知性）。 */
    @Test
    fun strip_removesUnknownTag() {
        assertEquals("你小心点", EmotionText.strip("[foo] 你小心点"))
    }

    /** 文本中段/尾部标签也剥离。 */
    @Test
    fun strip_removesTagInMiddle() {
        assertEquals("今天", EmotionText.strip("今天[joy]"))
    }

    /** 多个标签全部剥离。 */
    @Test
    fun strip_removesAllTags() {
        assertEquals("甲乙丙", EmotionText.strip("[joy]甲[neutral]乙[sad]丙"))
    }

    /** 无标签 → 原样（不动）。 */
    @Test
    fun strip_noTag_unchanged() {
        assertEquals("纯文本内容", EmotionText.strip("纯文本内容"))
    }

    /** 空/null → 空串（不抛异常）。 */
    @Test
    fun strip_emptyOrNull_returnsEmpty() {
        assertEquals("", EmotionText.strip(""))
        assertEquals("", EmotionText.strip(null))
    }

    /** 空白文本：无标签可剥，trim 后 → 空串（生产 strip 含 trim）。 */
    @Test
    fun strip_blank_returnsEmpty() {
        assertEquals("", EmotionText.strip("   "))
    }

    // ═══════════════════════════════════════════════
    // sanitize：strip → 小写 → 连续空白折叠为单空格
    // ═══════════════════════════════════════════════

    /** 大小写归一 + 剥离标签 + 去首尾空白。 */
    @Test
    fun sanitize_normalizesCaseStripTrim() {
        assertEquals("hello world", EmotionText.sanitize("[JOY] Hello World "))
    }

    /** 中文内容 sanitize 后保留中文 + 剥离标签。 */
    @Test
    fun sanitize_chineseText_preservesContent() {
        assertEquals("你好世界", EmotionText.sanitize("[sad] 你好世界"))
    }

    /** sanitize 空/null → 空串（不抛异常、永不 null）。 */
    @Test
    fun sanitize_emptyOrNull_returnsEmpty() {
        assertEquals("", EmotionText.sanitize(""))
        assertEquals("", EmotionText.sanitize(null))
    }

    /** sanitize 纯标签 → 空串（标签剥离后无残留内容，不产出 null）。 */
    @Test
    fun sanitize_onlyTag_returnsEmptyString() {
        assertEquals("", EmotionText.sanitize("[joy]"))
    }

    /** 连续空白折叠为单空格（\s+ → " "，再 trim）。 */
    @Test
    fun sanitize_collapsesInteriorWhitespace() {
        assertEquals("你好 世界", EmotionText.sanitize("  你好             世界  "))
    }
}
