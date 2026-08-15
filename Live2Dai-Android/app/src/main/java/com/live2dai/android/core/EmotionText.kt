package com.live2dai.android.core

/**
 * 表情标签解析结果（契约 refactor-0814 2C-3 锚点：emotion + cleanText 两字段）。
 *
 * @property emotion 提取到的表情关键词（小写，∈ [EmotionText.SUPPORTED_EMOTIONS]）；
 *                   无有效标签时为 "neutral"。
 * @property cleanText 剥离所有 [emotion] 标签后的纯文本（trim）。
 */
data class EmotionResult(
    val emotion: String,
    val cleanText: String,
)

/**
 * 表情标签纯函数层（契约 refactor-0814 2C-3；EmotionController 纯函数拆出）。
 *
 * 职责：从 LLM 回复文本中提取 [emotion] 标签（[joy]/[sad]/[angry]/[neutral] 等）、
 * 剥离标签、空白/大小写规范化。纯静态、无状态、无 Android 依赖（JVM 可测）。
 *
 * 规则（与 EmotionController.parseAndApply/stripEmotionTags 同源）：
 *  - 标签形状 `[keyword]` 或 `[keyword_数字]`（强度后缀忽略，仅取 keyword）；
 *  - 多个有效标签取**最后一个**（EmotionController "取最后一个有效标签" 同语义）；
 *  - 未知标签忽略（不参与情绪判定，但同 [strip] 一并从文本剥离）；
 *  - 无有效标签 → emotion = "neutral"。
 */
object EmotionText {

    /**
     * 支持的标签关键词集合：
     * 契约锚点短名（joy/sad/angry/surprise/neutral）+ EmotionController 现有情绪集全集
     * （fear/sadness/anger/disgust/smirk，见 [EmotionController.FALLBACK_EMOTION_MAP]）。
     */
    val SUPPORTED_EMOTIONS: Set<String> = setOf(
        // 契约锚点（refactor-0814 2C-3 §1）
        "joy", "sad", "angry", "surprise", "neutral",
        // EmotionController 现有情绪集（"等"）
        "fear", "sadness", "anger", "disgust", "smirk",
    )

    /** 标签形状：`[keyword]` / `[keyword_强度]`（与 EmotionController.EMOTION_TAG_REGEX 同源）。 */
    private val TAG_REGEX = Regex("""\[(\w+?)(?:_(\d+))?\]""")

    /**
     * 解析文本中的 [emotion] 标签。
     *
     * @param text 可能含标签的文本（null/空白 → neutral + 原样 trim）。
     * @return [EmotionResult]：最后一个有效标签关键词（无则 "neutral"）+ 剥离全部标签后的纯文本。
     */
    fun parse(text: String?): EmotionResult {
        val t = text ?: ""
        if (t.isBlank()) return EmotionResult("neutral", t.trim())
        var lastValid: String? = null
        for (match in TAG_REGEX.findAll(t)) {
            val keyword = match.groupValues[1].lowercase()
            if (keyword in SUPPORTED_EMOTIONS) lastValid = keyword
        }
        return EmotionResult(lastValid ?: "neutral", strip(t))
    }

    /**
     * 剥离文本中全部 [emotion] 形状标签，返回纯文本（trim）。
     * 未知标签同样被剥离（对齐 EmotionController.stripEmotionTags：标签形状即剥离，
     * 不判已知性），"去标签后的纯文本"语义。
     */
    fun strip(text: String?): String {
        if (text == null) return ""
        return text.replace(TAG_REGEX, "").trim()
    }

    /**
     * 空白/大小写规范化：剥离标签 → trim → 小写 → 连续空白折叠为单空格。
     * 供 TTS/UI 展示前统一净化（如 "[JOY] Hello World " → "hello world"）。
     */
    fun sanitize(text: String?): String {
        if (text == null) return ""
        return strip(text).lowercase().replace(WHITESPACE_REGEX, " ").trim()
    }

    private val WHITESPACE_REGEX = Regex("""\s+""")
}
