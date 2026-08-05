package com.live2dai.android

import android.util.Log

/**
 * 表情控制器
 *
 * 负责解析 LLM 回复中的 [emotion] 标签，将其映射为 Live2D 表情索引，
 * 并将标签从文本中剥离，返回纯文本供 TTS 使用。
 *
 * 表情映射表:
 *   neutral  (中性)  → index 0  (exp_01)
 *   fear     (恐惧)  → index 1  (exp_02)
 *   sadness  (悲伤)  → index 2  (exp_03)
 *   anger    (愤怒)  → index 3  (exp_04)
 *   disgust  (厌恶)  → index 4  (exp_05)
 *   joy      (高兴)  → index 5  (exp_06)
 *   smirk    (得意)  → index 6  (exp_07)
 *   surprise (惊讶)  → index 7  (exp_08)
 *
 * 共 8 个表情文件 (exp_01 ~ exp_08)，每个表情单独占用一个索引。
 */
class EmotionController(private val live2DView: Live2DView) {

    init {
        // 初始化取证日志：MainActivity 在 Live2DView 创建后实例化本控制器并接线，
        // logcat 出现本行即证明表情链路已接通（修复 T11 断线缺陷）。
        logInfo("EmotionController 初始化完成（Live2DView 已接线，等待 emotion 标签触发表情切换）")
    }

    companion object {
        private const val TAG = "EmotionController"

        // 表情关键词 → 表情索引映射表（每个表情单独一个索引）
        private val EMOTION_MAP = mapOf(
            "neutral"  to 0,
            "fear"     to 1,
            "sadness"  to 2,
            "anger"    to 3,
            "disgust"  to 4,
            "joy"      to 5,
            "smirk"    to 6,
            "surprise" to 7
        )

        // 所有支持的表情关键词（用于构建提示词）
        val EMOTION_KEYWORDS: List<String> = EMOTION_MAP.keys.sorted()

        // 正则表达式：[emotion] 标签
        private val EMOTION_TAG_REGEX = Regex("""\[(\w+)]""")

        /**
         * 纯静态方法：剥离文本中的 [emotion] 标签
         * 用在 UI 层流式显示时，避免用户看到原始标签文本
         */
        fun stripEmotionTags(text: String): String {
            return text.replace(EMOTION_TAG_REGEX, "").trim()
        }
    }

    /**
     * 上次设置的表情索引
     */
    private var currentExpressionIndex: Int = 0

    /**
     * 日志输出（带单元测试防护）。
     *
     * 本地 JVM 单元测试（无 Android 运行时）中 android.util.Log 未被 mock，直接调用会抛
     * "Method i in android.util.Log not mocked"，因此统一走这里包一层异常防护；
     * 真机上行为与直接调用一致（Log.i / Log.w）。
     */
    private fun logInfo(message: String) {
        try {
            Log.i(TAG, message)
        } catch (_: Throwable) {
            // 单元测试环境：静默忽略
        }
    }

    private fun logWarn(message: String) {
        try {
            Log.w(TAG, message)
        } catch (_: Throwable) {
            // 单元测试环境：静默忽略
        }
    }

    /**
     * 获取当前表情索引
     */
    fun getCurrentExpressionIndex(): Int = currentExpressionIndex

    /**
     * 解析文本中的 [emotion] 标签
     *
     * 1. 提取所有 [emotion] 标签（取最后一个有效标签）
     * 2. 映射为 Live2D 表情索引并设置
     * 3. 移除所有标签，返回纯文本
     *
     * @param text 可能包含表情标签的文本，如 "[joy] 主人今天心情真好喵~！"
     * @return 移除了标签后的纯文本，如 "主人今天心情真好喵~！"
     */
    fun parseAndApply(text: String): String {
        if (text.isBlank()) return text

        var lastValidEmotion: String? = null

        // 查找所有 [emotion] 标签
        val matches = EMOTION_TAG_REGEX.findAll(text)
        for (match in matches) {
            val keyword = match.groupValues[1].lowercase()
            if (EMOTION_MAP.containsKey(keyword)) {
                lastValidEmotion = keyword
            }
        }

        // 如果找到有效表情，设置到 Live2D 模型
        if (lastValidEmotion != null) {
            val expressionIndex = EMOTION_MAP[lastValidEmotion] ?: 0
            if (expressionIndex != currentExpressionIndex) {
                currentExpressionIndex = expressionIndex
                live2DView.setExpression(expressionIndex)
                logInfo("parseAndApply: 标签[$lastValidEmotion] -> expressionIndex=$expressionIndex")
            }
        }

        // 移除所有 [*] 标签，返回纯文本
        return text.replace(EMOTION_TAG_REGEX, "").trim()
    }

    /**
     * 手动设置表情
     * @param emotionName 表情名称（与 EMOTION_MAP 中的 key 对应）
     */
    fun setEmotion(emotionName: String) {
        val keyword = emotionName.lowercase().trim()
        val index = EMOTION_MAP[keyword] ?: run {
            logWarn("setEmotion: 未知表情关键词 '$emotionName'，忽略")
            return
        }
        if (index != currentExpressionIndex) {
            currentExpressionIndex = index
            live2DView.setExpression(index)
            logInfo("setEmotion: $keyword -> expressionIndex=$index")
        } else {
            logInfo("setEmotion: $keyword 已是当前表情(index=$index)，跳过")
        }
    }

    /**
     * 重置为中性表情
     */
    fun resetToNeutral() {
        if (currentExpressionIndex != 0) {
            currentExpressionIndex = 0
            live2DView.setExpression(0)
            logInfo("resetToNeutral: -> expressionIndex=0")
        }
    }
}
