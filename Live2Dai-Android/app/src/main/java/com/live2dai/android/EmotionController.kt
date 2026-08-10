package com.live2dai.android

import android.util.Log
import org.json.JSONException
import org.json.JSONObject

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

// ── M2 结构化表情结果（签名锁定自 contract-M1-shared-protocol.md §2.1，禁止改字段）──

/** 单表情权重项 */
data class ExpressionWeight(
    val name: String,      // EMOTION_MAP 8 key 之一（小写）
    val weight: Float      // [0.0, 1.0]
)

/** parseStructured 的结构化解析结果（null = 解析失败/校验失败，走降级路径） */
data class EmotionResult(
    val expressionMix: List<ExpressionWeight>,     // 可能为空（空 = 表情层不动作）
    val parameterOverrides: Map<String, Float>,    // key ∈ 当前模型 cdi3.json 参数 ID 白名单
    val motionLabel: String?                       // 别名表别名 或 null
)

class EmotionController(private val live2DView: Live2DView) {

    init {
        // 初始化取证日志：MainActivity 在 Live2DView 创建后实例化本控制器并接线，
        // logcat 出现本行即证明表情链路已接通（修复 T11 断线缺陷）。
        logInfo("EmotionController 初始化完成（Live2DView 已接线，等待 emotion 标签触发表情切换）")
    }

    companion object {
        private const val TAG = "EmotionController"

        // 表情关键词 → 表情索引映射表（每个表情单独一个索引）
        // 配置优先：adapter.emotionIndex（T1.2，与 PC model_dict emotionMap 同源）；
        // 缺失/解析失败 → 硬编码常量兜底（零行为变化，回归门）。
        private val FALLBACK_EMOTION_MAP: Map<String, Int> = mapOf(
            "neutral"  to 0,
            "fear"     to 1,
            "sadness"  to 2,
            "anger"    to 3,
            "disgust"  to 4,
            "joy"      to 5,
            "smirk"    to 6,
            "surprise" to 7
        )

        private val EMOTION_MAP: Map<String, Int>
            get() = ModelAdapterConfig.currentAdapter()?.emotionIndex ?: FALLBACK_EMOTION_MAP

        // 所有支持的表情关键词（用于构建提示词；与 EMOTION_MAP 同步动态读取）
        val EMOTION_KEYWORDS: List<String>
            get() = EMOTION_MAP.keys.sorted()

        // 正则表达式：[emotion] 标签，惰性匹配 + 可选强度后缀 [_数字]
        // [joy_3] → group1=joy, group2=3（强度）；[joy] → group1=joy, group2=null
        private val EMOTION_TAG_REGEX = Regex("""\[(\w+?)(?:_(\d+))?]""")

        // ── M2 结构化表情 JSON（emotion-protocol.md B 段）──

        // FENCE 块：```json / ``` 包裹的 JSON 对象（DOT_MATCHES_ALL 跨行；group1 = JSON 子串）
        private val JSON_FENCE_REGEX =
            Regex("""```(?:json)?\s*(\{.*?\})\s*```""", RegexOption.DOT_MATCHES_ALL)

        // NAKED：首个 { 到末个 } 的跨行子串
        private val JSON_NAKED_REGEX = Regex("""\{.*}""", RegexOption.DOT_MATCHES_ALL)

        // motionLabel 别名白名单（emotion-protocol.md F 段：niziiro_mao 别名表；
        // 未知别名 → 置 null，M1 §1 C4）
        private val MOTION_LABEL_ALIASES: Set<String> = setOf(
            "idle", "wave", "special_02", "special_03", "mtn_02", "mtn_03", "mtn_04",
            "flickup", "tap", "flick3"
        )

        // ── TTS 播报净化（task-fix-tts-sanitize）──
        // 用户真机实测：TTS 播报读到 "[joy] ... 等于号 ..."——[emotion] 标签（部分路径）
        // 与颜文字 (=^･ω･^=) 被朗读。sanitizeForTts 在 speak 统一入口兜底净化。

        // 颜文字括号段：括号内仅含符号（无中文/字母/数字）→ 剥离，如 (=^･ω･^=) (≧▽≦)
        // 括号内含中文（如（小声说））不匹配 → 保留（保守策略：不清正常中文括号）
        private val KAOMOJI_PAREN_REGEX = Regex("""[（(][^（）()\u4e00-\u9fa5A-Za-z0-9]+[）)]""")

        // 无括号简单颜文字：^_^ / >_< / =ω= 等三字符符号组合，前后不得紧跟字母/数字（避免误删）
        private val SIMPLE_KAOMOJI_REGEX =
            Regex("""(?<![A-Za-z0-9])[\^=<>;~][_·･ω∀≧≦][\^=<>;~](?![A-Za-z0-9])""")

        // markdown 加粗/斜体符号：** 或 __ （2+ 连续）→ 剥离符号、保留文字；单个 * / _ 不动
        private val MARKDOWN_SYMBOL_REGEX = Regex("""[*_]{2,}""")

        /**
         * 纯静态方法：TTS 播报前统一净化文本。
         *
         * 在 [stripEmotionTags]（剥离 [emotion] 标签）基础上，进一步剥离会被 TTS 朗读的
         * 符号类内容：颜文字括号段（如 (=^･ω･^=) 会被读成"等于号"）、无括号简单颜文字、
         * markdown 加粗/斜体符号。
         *
         * 保守原则（与 PC 端 tts_preprocessor.py 的 filter_brackets/filter_parentheses 不同）：
         * 只清"符号类"内容，**不得误删正常中文括号**（（小声说）等括号内含中文 → 保留）。
         * 仅影响 TTS 播报文本，不影响 UI 显示与字幕。
         */
        fun sanitizeForTts(text: String): String {
            return text
                .replace(EMOTION_TAG_REGEX, "")          // [joy] 等 [emotion] 标签
                .replace(KAOMOJI_PAREN_REGEX, "")       // 颜文字括号段 (=^･ω･^=)
                .replace(SIMPLE_KAOMOJI_REGEX, "")      // 无括号简单颜文字 ^_^ / >_<
                .replace(MARKDOWN_SYMBOL_REGEX, "")     // **xx** 符号剥离，保留文字
                .trim()
        }

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
    private var currentIntensity: Float = 1.0f

    // ── M2 结构化表情（JSON）状态 ──

    /**
     * 当前模型 cdi3.json 参数 ID 白名单（parameterOverrides key 校验源，emotion-protocol.md C3）。
     * 运行时模型加载/切换后同步（Live2DView.getParameterWhitelist()）；留空则 parseStructured
     * 自动回退查询 live2DView 的当前模型白名单。
     */
    var parameterWhitelist: Set<String> = emptySet()

    /**
     * motionLabel 触发接缝（M7 接线到 Live2DRenderer.startMotionByTag(tag)）。
     * M7 未接线时 = null → no-op，不抛异常（contract-M2-M3 §1.3(d)）。
     */
    var motionTrigger: ((String) -> Unit)? = null

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
     * @param text 可能包含表情标签的文本，如 "[joy] 今天心情真好！"
     * @return 移除了标签后的纯文本，如 "今天心情真好！"
     */
    fun parseAndApply(text: String): String {
        if (text.isBlank()) return text

        var lastValidEmotion: String? = null
        var lastIntensity: Int = 3  // 默认满强度

        // 查找所有 [emotion] 标签
        val matches = EMOTION_TAG_REGEX.findAll(text)
        for (match in matches) {
            val keyword = match.groupValues[1].lowercase()
            if (EMOTION_MAP.containsKey(keyword)) {
                lastValidEmotion = keyword
                lastIntensity = (match.groupValues[2].toIntOrNull() ?: 3).coerceIn(1, 3)
            }
        }

        // 如果找到有效表情，设置到 Live2D 模型
        if (lastValidEmotion != null) {
            val expressionIndex = EMOTION_MAP[lastValidEmotion] ?: 0
            val intensity = lastIntensity / 3.0f
            if (expressionIndex != currentExpressionIndex || intensity != currentIntensity) {
                currentExpressionIndex = expressionIndex
                currentIntensity = intensity
                live2DView.setExpression(expressionIndex, intensity)
                logInfo("parseAndApply: 标签[$lastValidEmotion] index=$expressionIndex intensity=${lastIntensity}")
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

    // ── M2 结构化表情 JSON 解析 / 应用 / 文本剥离 ──

    /**
     * 提取 JSON 对象子串：FENCE（```json/```）优先 → NAKED（首个 { 到末个 } 跨行子串）兜底。
     * 均无 → null。
     */
    private fun extractJsonObject(text: String): String? {
        val fenceMatch = JSON_FENCE_REGEX.find(text)
        if (fenceMatch != null) {
            return fenceMatch.groupValues[1]
        }
        return JSON_NAKED_REGEX.find(text)?.value
    }

    /**
     * 解析当前模型 cdi3.json 参数 ID 白名单：优先使用手动注入的 [parameterWhitelist]
     * （测试/无 renderer 场景），否则回退查询 live2DView 当前模型白名单。
     */
    private fun resolveParameterWhitelist(): Set<String> {
        if (parameterWhitelist.isNotEmpty()) return parameterWhitelist
        return runCatching { live2DView.getParameterWhitelist() }.getOrDefault(emptySet())
    }

    /**
     * 从 LLM 回复中解析结构化表情 JSON（emotion-protocol.md B 段 schema）。
     *
     * 提取顺序（markdown code fence 优先，防正文误匹配）：
     *   1. ```json ... ``` / ``` ... ``` fence 包裹的 JSON 对象
     *   2. 无 fence 时，取文本中第一个 '{' 到最后一个 '}' 的跨行子串（裸 JSON）
     *
     * 校验规则（与 PC extract_expression_mix 完全一致，双端同输入同输出）：
     *   - expressionMix.expressions[].name 须 ∈ EMOTION_MAP（8 key，大小写不敏感）
     *   - weight 非法（<0 或 >1 或非数字）→ 丢弃该项；全部丢弃 → expressionMix 为空（不降级）
     *   - 权重总和 > 1.0 → 按比例归一化
     *   - parameterOverrides key ∉ 当前模型 cdi3.json 白名单 → 丢弃该 key
     *   - motionLabel ∉ 别名表 → 置 null
     *   - 顶层 JSON 解析失败 / 无 JSON / 无合法字段 → 返回 null（调用方走关键词降级路径）
     *
     * @param text LLM 完整回复（可能含正文 + code fence + 旧 [keyword] 标签）
     * @return EmotionResult 或 null（null = 畸形/无 JSON，降级到 parseAndApply）
     */
    fun parseStructured(text: String): EmotionResult? {
        val jsonStr = extractJsonObject(text) ?: return null

        val root = try {
            JSONObject(jsonStr)
        } catch (e: JSONException) {
            return null
        }

        // C5 部分生效：至少一个字段存在且结构合法才算解析成功
        var anyValidField = false

        // ── expressionMix（C1/C2）──
        val mix = mutableListOf<ExpressionWeight>()
        val emObj = root.optJSONObject("expressionMix")
        if (emObj != null) {
            anyValidField = true
            val arr = emObj.optJSONArray("expressions")
            if (arr != null) {
                for (i in 0 until arr.length()) {
                    val item = try {
                        arr.getJSONObject(i)
                    } catch (e: JSONException) {
                        continue
                    }
                    val name = item.optString("name", "").lowercase().trim()
                    if (name !in EMOTION_MAP) continue
                    val weight = item.optDouble("weight", Double.NaN)
                    if (weight.isNaN() || weight < 0.0 || weight > 1.0) continue
                    mix.add(ExpressionWeight(name, weight.toFloat()))
                }
            }
        }
        // 权重和 > 1.0 → 按比例归一化（每项 / sum；双端比较容差 1e-4）
        if (mix.isNotEmpty()) {
            val sum = mix.sumOf { it.weight.toDouble() }
            if (sum > 1.0) {
                for (i in mix.indices) {
                    mix[i] = ExpressionWeight(mix[i].name, (mix[i].weight / sum).toFloat())
                }
            }
        }

        // ── parameterOverrides（C3：key 白名单 = 当前模型 cdi3.json；value = 乘数）──
        val overrides = mutableMapOf<String, Float>()
        val ovObj = root.optJSONObject("parameterOverrides")
        if (ovObj != null) {
            anyValidField = true
            val whitelist = resolveParameterWhitelist()
            val keys = ovObj.keys()
            while (keys.hasNext()) {
                val key = keys.next()
                if (whitelist.isNotEmpty() && key !in whitelist) continue
                val v = ovObj.optDouble(key, Double.NaN)
                if (v.isNaN()) continue
                overrides[key] = v.toFloat()
            }
        }

        // ── motionLabel（C4：别名表校验，未知 → null）──
        var motion: String? = null
        if (root.has("motionLabel")) {
            anyValidField = true
            val label = root.optString("motionLabel", "").lowercase().trim()
            if (label.isNotEmpty() && label in MOTION_LABEL_ALIASES) {
                motion = label
            }
        }

        if (!anyValidField) return null
        return EmotionResult(mix, overrides, motion)
    }

    /**
     * 应用结构化表情结果（contract-M2-M3 §1.3 锁定行为）。
     *
     * (a) expressionMix → name→EMOTION_MAP 索引 → Live2DView.setExpressionMix
     *     （索引→ExpressionData 解析与 ExpressionManager 加权混合在渲染侧完成；查不到的表情
     *     跳过，部分生效不降级）
     * (b) parameterOverrides → 随 setExpressionMix 传入 ExpressionManager（乘数语义，混合后叠加）
     * (d) motionLabel → motionTrigger 接缝（M7 接线前 no-op，不抛异常）
     * (e) 空 mix → 表情层不动作，但 overrides/motionLabel 仍生效
     */
    fun applyStructured(result: EmotionResult) {
        val indices = result.expressionMix.mapNotNull { ew ->
            val index = EMOTION_MAP[ew.name] ?: return@mapNotNull null
            index to ew.weight
        }
        if (result.motionLabel != null) {
            motionTrigger?.invoke(result.motionLabel)
        }
        live2DView.setExpressionMix(indices, result.parameterOverrides)
    }

    /**
     * 剥离文本中的结构化 JSON 块（双端一致规则，contract-M2-M3 §1.4）：
     * ① 含 FENCE（```json/```）块 → 剥离全部 FENCE 块；
     * ② 无 FENCE 且 NAKED JSON 解析成功 → 剥离该子串（首个 { 到末个 }）；
     * ③ 其余原样返回。
     * 保证 TTS 不朗读 JSON、UI 不显示 JSON。
     */
    fun stripStructuredJson(text: String): String {
        if (text.isEmpty()) return text
        // ① FENCE 块整体剥离
        if (JSON_FENCE_REGEX.containsMatchIn(text)) {
            return JSON_FENCE_REGEX.replace(text, "").trim()
        }
        // ② 无 FENCE：NAKED 解析成功 → 剥离该子串
        val nakedMatch = JSON_NAKED_REGEX.find(text)
        if (nakedMatch != null && parseStructured(text) != null) {
            return text.replace(nakedMatch.value, "").trim()
        }
        // ③ 其余原样返回
        return text
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
