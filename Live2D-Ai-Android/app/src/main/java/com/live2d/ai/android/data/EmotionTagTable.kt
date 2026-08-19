package com.live2d.ai.android.data

import com.live2d.ai.android.core.EmotionText

// ============================================================
//  Wave 3 · 3-2 M7：情绪标签 → 参数帧映射表
//
//  职责：把文本情绪标签（裸关键词，如 "joy_3"；由 EmotionText 剥离方括号后传入）
//  解析为 bai 参数驱动路径可用的参数帧，
//  与 EmotionText（core 层）同源语义：大小写不敏感 + 强度后缀 [_数字] 忽略。
//
//  键域（合法域 = bai emotionParamMap 6 键，contract-bai-params-v0 §2 A3U-01）：
//    ParamBrowLY / ParamBrowRY / ParamEyeLOpen / ParamEyeROpen /
//    ParamMouthForm / ParamMouthOpenY
//  帧产出键 = 6 键中除眼部键外的 4 键：ParamEyeLOpen/ParamEyeROpen **不入帧**——
//  眼→眨眼专属（R13/R16 旧仲裁语义：表情不碰眼），防表情 Overwrite 持续覆盖眨眼写入
//  （ExpressionManager fade 后权重不衰减、恒全量写，眼部键入帧会把眼皮钉在非 1.0 值上）。
//  值域：全部帧值 ∈ [0,1]（契约 3-2 约束；归一化显示强度，0=通道关闭/中性，
//  1=通道满强度；模型原生带符号域（如角度 ±30）的换算由下游渲染层负责）。
//
//  纯 JVM（无 android.* / native 依赖），可确定性单测。
// ============================================================

/**
 * 情绪标签 → 参数帧映射表（纯静态）。
 */
object EmotionTagTable {

    /**
     * 最小语义情绪集（契约锚点）：neutral/joy/sad/angry/surprise 五基元。
     * 均为 [EmotionText.SUPPORTED_EMOTIONS] 子集。
     */
    val MINIMAL_SEMANTIC_EMOTIONS: Set<String> = setOf("neutral", "joy", "sad", "angry", "surprise")

    /**
     * bai emotionParamMap 6 键合法域（contract-bai-params-v0 §2 A3U-01）。
     * 注意：[frameFor] 帧产出键为其子集且**不含眼部键**（ParamEyeLOpen/ParamEyeROpen——
     * 眼→眨眼专属，表情不碰眼，R13/R16 仲裁）。
     */
    val validParameterKeys: Set<String> = setOf(
        "ParamBrowLY", "ParamBrowRY", "ParamEyeLOpen", "ParamEyeROpen",
        "ParamMouthForm", "ParamMouthOpenY",
    )

    /** 强度后缀形状：`_数字`（如 joy_3 → joy；与 EmotionText 标签形状同源）。 */
    private val INTENSITY_SUFFIX_REGEX = Regex("""_\d+$""")

    /**
     * 情绪帧表（8 情绪 × 4 键；值 ∈ [0,1]）。
     *
     * **不含眼部键**（ParamEyeLOpen/ParamEyeROpen）：眼→眨眼专属（R13/R16 旧仲裁语义，
     * 表情不碰眼）——表情帧若带眼部键会经 ExpressionManager Overwrite 每帧覆盖眨眼写入
     * （fade 后权重不衰减、恒全量写），致眨眼异常（眼皮被钉在非 1.0 值上、双写者打架）。
     * 眨眼归 EyeBlinkManager 独占（AnimationSystem.applyEyeBlink）。
     *
     * 帧值设计（观感依据，KDoc 即契约）：
     *  - neutral  ：放松基准——眉/嘴形 0.5 中性、嘴闭合 0；
     *  - joy      ：眉抬高 0.8、大嘴笑形 0.9 + 开口 0.7；
     *  - sad      ：眉下垂 0.2、嘴下弯 0.15、微开口 0.1；
     *  - angry    ：眉压低 0.1、嘴紧抿 0.2、微开口 0.35；
     *  - surprise ：眉高抬 0.95、嘴张大 1.0（jaw drop）；
     *  - fear     ：眉高抬 0.85、嘴微张 0.6、嘴形紧 0.3；
     *  - disgust  ：眉压低 0.25、嘴下弯 0.1、微开口 0.2；
     *  - smirk    ：不对称坏笑——左眉 0.65/右眉 0.45、嘴形偏笑 0.75、开口 0.3。
     */
    private val FRAMES: Map<String, Map<String, Float>> = mapOf(
        "neutral" to mapOf(
            "ParamBrowLY" to 0.5f,
            "ParamBrowRY" to 0.5f,
            "ParamMouthForm" to 0.5f,
            "ParamMouthOpenY" to 0.0f,
        ),
        "joy" to mapOf(
            "ParamBrowLY" to 0.8f,
            "ParamBrowRY" to 0.8f,
            "ParamMouthForm" to 0.9f,
            "ParamMouthOpenY" to 0.7f,
        ),
        "sad" to mapOf(
            "ParamBrowLY" to 0.2f,
            "ParamBrowRY" to 0.2f,
            "ParamMouthForm" to 0.15f,
            "ParamMouthOpenY" to 0.1f,
        ),
        "angry" to mapOf(
            "ParamBrowLY" to 0.1f,
            "ParamBrowRY" to 0.1f,
            "ParamMouthForm" to 0.2f,
            "ParamMouthOpenY" to 0.35f,
        ),
        "surprise" to mapOf(
            "ParamBrowLY" to 0.95f,
            "ParamBrowRY" to 0.95f,
            "ParamMouthForm" to 0.5f,
            "ParamMouthOpenY" to 1.0f,
        ),
        "fear" to mapOf(
            "ParamBrowLY" to 0.85f,
            "ParamBrowRY" to 0.85f,
            "ParamMouthForm" to 0.3f,
            "ParamMouthOpenY" to 0.6f,
        ),
        // 语义别名：sadness ≡ sad（EmotionController 既有情绪集映射）
        "sadness" to mapOf(
            "ParamBrowLY" to 0.2f,
            "ParamBrowRY" to 0.2f,
            "ParamMouthForm" to 0.15f,
            "ParamMouthOpenY" to 0.1f,
        ),
        // 语义别名：anger ≡ angry
        "anger" to mapOf(
            "ParamBrowLY" to 0.1f,
            "ParamBrowRY" to 0.1f,
            "ParamMouthForm" to 0.2f,
            "ParamMouthOpenY" to 0.35f,
        ),
        "disgust" to mapOf(
            "ParamBrowLY" to 0.25f,
            "ParamBrowRY" to 0.25f,
            "ParamMouthForm" to 0.1f,
            "ParamMouthOpenY" to 0.2f,
        ),
        "smirk" to mapOf(
            "ParamBrowLY" to 0.65f,
            "ParamBrowRY" to 0.45f,
            "ParamMouthForm" to 0.75f,
            "ParamMouthOpenY" to 0.3f,
        ),
    )

    /**
     * 标签是否已知情绪：大小写不敏感 + 强度后缀 `_数字` 忽略
     * （与 [EmotionText] 同源语义：裸关键词 `joy_3` / `JOY` / `Joy_2` 均命中；
     *  注意本表不做方括号剥离——`[joy]` 形输入回退未知（审计 R3 文档修正）；
     *  null / 空白 / 未知词 → false）。
     */
    fun isKnownEmotion(tag: String?): Boolean {
        val base = normalizeTag(tag) ?: return false
        return base in EmotionText.SUPPORTED_EMOTIONS
    }

    /**
     * 情绪标签 → 参数帧。
     *
     * @param tag 情绪标签（如 "joy" / "JOY_3" / "[joy_3]" 原样标签均可；null / 空白 → 未知）。
     * @param fallbackNeutral 未知情绪时是否回退 neutral 帧：
     *                        true → neutral 帧；false → 空 Map（调用方自行决定不动作）。
     * @return 已知情绪 → 对应参数帧（键 ∈ bai 6 键合法域且**不含眼部键**——眼归眨眼专属；
     *         值 ∈ [0,1]）；未知情绪 → 按 [fallbackNeutral] 回退 neutral 帧或空 Map。
     */
    fun frameFor(tag: String?, fallbackNeutral: Boolean = true): Map<String, Float> {
        val base = normalizeTag(tag)
        val frame = base?.let { FRAMES[it] }
        return when {
            frame != null -> frame
            fallbackNeutral -> FRAMES.getValue("neutral")
            else -> emptyMap()
        }
    }

    /** 规范化：trim + 小写 + 去强度后缀（`_数字`）；空白 → null。 */
    private fun normalizeTag(tag: String?): String? {
        if (tag.isNullOrBlank()) return null
        val t = tag.trim().lowercase()
        if (t.isEmpty()) return null
        return t.replace(INTENSITY_SUFFIX_REGEX, "")
    }
}
