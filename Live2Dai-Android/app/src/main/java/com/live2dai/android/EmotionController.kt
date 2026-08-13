package com.live2dai.android

import android.util.Log
import com.live2dai.android.data.BodyPoseFrames
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

class EmotionController(
    private val live2DView: Live2DView,
    /**
     * A3 参数帧数据注入点（contract-bai-params-v0 L31：map 注入**或**内部持有 emotionParamMap）。
     *
     * 非 null → 优先于 ModelAdapterConfig.currentAdapter() 读取（测试 JVM 环境 currentAdapter() 为
     * null 时的注入通道）；null → 回退 adapter.emotionParamMap → 硬编码 bai 默认帧兜底
     * （[DEFAULT_BAI_EMOTION_PARAM_MAP]）。默认 null 保持既有单参构造（ChatViewModel 等）兼容。
     */
    private val emotionParamMapOverride: Map<String, Map<String, Map<String, Float>>>? = null,
) {

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

        /**
         * A3 参数帧硬编码兜底（contract-bai-params-v0 §2 数据锚点，镜像 assets/bai.adapter.json
         * emotionParamMap，144 值：8 emotion × 3 强度档 × 6 键，值域 [-1.0, 2.0] 实测 min=-0.75 max=1.5）。
         *
         * 对齐 [FALLBACK_EMOTION_MAP] 兜底模式：adapter 未加载（测试 JVM）/解析失败场景不丢参数路径；
         * 运行时可加载的 adapter（bai 单源）优先于本兜底（见 [EmotionController.emotionParamMap]）。
         * 无 exp3（bai）→ 无 emotionParamMap 时走 PARAM_FRAME 路径，兜底帧生效（AP-28 零回归）。
         */
        private val DEFAULT_BAI_EMOTION_PARAM_MAP: Map<String, Map<String, Map<String, Float>>> = mapOf(
            "neutral" to mapOf(
                "_1" to mapOf("ParamBrowLY" to 0.0f, "ParamBrowRY" to 0.0f, "ParamEyeLOpen" to 0.75f, "ParamEyeROpen" to 0.75f, "ParamMouthForm" to 0.5f, "ParamMouthOpenY" to 0.0f),
                "_2" to mapOf("ParamBrowLY" to 0.0f, "ParamBrowRY" to 0.0f, "ParamEyeLOpen" to 0.75f, "ParamEyeROpen" to 0.75f, "ParamMouthForm" to 0.55f, "ParamMouthOpenY" to 0.0f),
                "_3" to mapOf("ParamBrowLY" to 0.0f, "ParamBrowRY" to 0.0f, "ParamEyeLOpen" to 0.8f, "ParamEyeROpen" to 0.8f, "ParamMouthForm" to 0.6f, "ParamMouthOpenY" to 0.0f),
            ),
            "fear" to mapOf(
                "_1" to mapOf("ParamBrowLY" to 0.2f, "ParamBrowRY" to 0.2f, "ParamEyeLOpen" to 0.85f, "ParamEyeROpen" to 0.85f, "ParamMouthForm" to 0.3f, "ParamMouthOpenY" to 0.2f),
                "_2" to mapOf("ParamBrowLY" to 0.35f, "ParamBrowRY" to 0.35f, "ParamEyeLOpen" to 1.0f, "ParamEyeROpen" to 1.0f, "ParamMouthForm" to 0.2f, "ParamMouthOpenY" to 0.4f),
                "_3" to mapOf("ParamBrowLY" to 0.55f, "ParamBrowRY" to 0.55f, "ParamEyeLOpen" to 1.3f, "ParamEyeROpen" to 1.3f, "ParamMouthForm" to 0.15f, "ParamMouthOpenY" to 0.65f),
            ),
            "sadness" to mapOf(
                "_1" to mapOf("ParamBrowLY" to 0.15f, "ParamBrowRY" to 0.15f, "ParamEyeLOpen" to 0.55f, "ParamEyeROpen" to 0.55f, "ParamMouthForm" to -0.2f, "ParamMouthOpenY" to 0.1f),
                "_2" to mapOf("ParamBrowLY" to 0.25f, "ParamBrowRY" to 0.25f, "ParamEyeLOpen" to 0.5f, "ParamEyeROpen" to 0.5f, "ParamMouthForm" to -0.4f, "ParamMouthOpenY" to 0.15f),
                "_3" to mapOf("ParamBrowLY" to 0.35f, "ParamBrowRY" to 0.35f, "ParamEyeLOpen" to 0.45f, "ParamEyeROpen" to 0.45f, "ParamMouthForm" to -0.55f, "ParamMouthOpenY" to 0.25f),
            ),
            "anger" to mapOf(
                "_1" to mapOf("ParamBrowLY" to -0.2f, "ParamBrowRY" to -0.2f, "ParamEyeLOpen" to 0.65f, "ParamEyeROpen" to 0.65f, "ParamMouthForm" to -0.3f, "ParamMouthOpenY" to 0.05f),
                "_2" to mapOf("ParamBrowLY" to -0.35f, "ParamBrowRY" to -0.35f, "ParamEyeLOpen" to 0.6f, "ParamEyeROpen" to 0.6f, "ParamMouthForm" to -0.55f, "ParamMouthOpenY" to 0.1f),
                "_3" to mapOf("ParamBrowLY" to -0.5f, "ParamBrowRY" to -0.5f, "ParamEyeLOpen" to 0.55f, "ParamEyeROpen" to 0.55f, "ParamMouthForm" to -0.75f, "ParamMouthOpenY" to 0.2f),
            ),
            "disgust" to mapOf(
                "_1" to mapOf("ParamBrowLY" to -0.15f, "ParamBrowRY" to -0.15f, "ParamEyeLOpen" to 0.6f, "ParamEyeROpen" to 0.6f, "ParamMouthForm" to -0.3f, "ParamMouthOpenY" to 0.05f),
                "_2" to mapOf("ParamBrowLY" to -0.25f, "ParamBrowRY" to -0.25f, "ParamEyeLOpen" to 0.55f, "ParamEyeROpen" to 0.55f, "ParamMouthForm" to -0.5f, "ParamMouthOpenY" to 0.1f),
                "_3" to mapOf("ParamBrowLY" to -0.35f, "ParamBrowRY" to -0.35f, "ParamEyeLOpen" to 0.5f, "ParamEyeROpen" to 0.5f, "ParamMouthForm" to -0.65f, "ParamMouthOpenY" to 0.15f),
            ),
            "joy" to mapOf(
                "_1" to mapOf("ParamBrowLY" to 0.15f, "ParamBrowRY" to 0.15f, "ParamEyeLOpen" to 0.8f, "ParamEyeROpen" to 0.8f, "ParamMouthForm" to 0.7f, "ParamMouthOpenY" to 0.1f),
                "_2" to mapOf("ParamBrowLY" to 0.25f, "ParamBrowRY" to 0.25f, "ParamEyeLOpen" to 0.85f, "ParamEyeROpen" to 0.85f, "ParamMouthForm" to 0.85f, "ParamMouthOpenY" to 0.2f),
                "_3" to mapOf("ParamBrowLY" to 0.35f, "ParamBrowRY" to 0.35f, "ParamEyeLOpen" to 0.9f, "ParamEyeROpen" to 0.9f, "ParamMouthForm" to 0.95f, "ParamMouthOpenY" to 0.35f),
            ),
            "smirk" to mapOf(
                "_1" to mapOf("ParamBrowLY" to 0.1f, "ParamBrowRY" to 0.1f, "ParamEyeLOpen" to 0.7f, "ParamEyeROpen" to 0.7f, "ParamMouthForm" to -0.3f, "ParamMouthOpenY" to 0.0f),
                "_2" to mapOf("ParamBrowLY" to 0.2f, "ParamBrowRY" to 0.2f, "ParamEyeLOpen" to 0.65f, "ParamEyeROpen" to 0.65f, "ParamMouthForm" to -0.5f, "ParamMouthOpenY" to 0.0f),
                "_3" to mapOf("ParamBrowLY" to 0.3f, "ParamBrowRY" to 0.3f, "ParamEyeLOpen" to 0.6f, "ParamEyeROpen" to 0.6f, "ParamMouthForm" to -0.7f, "ParamMouthOpenY" to 0.05f),
            ),
            "surprise" to mapOf(
                "_1" to mapOf("ParamBrowLY" to 0.25f, "ParamBrowRY" to 0.25f, "ParamEyeLOpen" to 0.95f, "ParamEyeROpen" to 0.95f, "ParamMouthForm" to 0.5f, "ParamMouthOpenY" to 0.3f),
                "_2" to mapOf("ParamBrowLY" to 0.4f, "ParamBrowRY" to 0.4f, "ParamEyeLOpen" to 1.2f, "ParamEyeROpen" to 1.2f, "ParamMouthForm" to 0.4f, "ParamMouthOpenY" to 0.5f),
                "_3" to mapOf("ParamBrowLY" to 0.55f, "ParamBrowRY" to 0.55f, "ParamEyeLOpen" to 1.5f, "ParamEyeROpen" to 1.5f, "ParamMouthForm" to 0.3f, "ParamMouthOpenY" to 0.7f),
            ),
        )

        private val EMOTION_MAP: Map<String, Int>
            get() = ModelAdapterConfig.currentAdapter()?.emotionIndex ?: FALLBACK_EMOTION_MAP

        // 所有支持的表情关键词（用于构建提示词；与 EMOTION_MAP 同步动态读取）
        val EMOTION_KEYWORDS: List<String>
            get() = EMOTION_MAP.keys.sorted()

        // 正则表达式：[emotion] 标签，惰性匹配 + 可选强度后缀 [_数字]
        // [joy_3] → group1=joy, group2=3（强度）；[joy] → group1=joy, group2=null
        private val EMOTION_TAG_REGEX = Regex("""\[(\w+?)(?:_(\d+))?\]""")

        // ── M2 结构化表情 JSON（emotion-protocol.md B 段）──

        // FENCE 块：```json / ``` 包裹的 JSON 对象（DOT_MATCHES_ALL 跨行；group1 = JSON 子串）
        private val JSON_FENCE_REGEX =
            Regex("""```(?:json)?\s*(\{.*?\})\s*```""", RegexOption.DOT_MATCHES_ALL)

        // NAKED：首个 { 到末个 } 的跨行子串（\} 必须转义：Android ICU Pattern 对未转义 } 报错，JVM 宽容）
        private val JSON_NAKED_REGEX = Regex("""\{.*\}""", RegexOption.DOT_MATCHES_ALL)

        // motionLabel 别名白名单（emotion-protocol.md F 段；bai 无 motion → 空集，别名表保留为通用防御）
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

    // ── A3 参数驱动表情路径（contract-bai-params-v0）──

    /** 双路径分发模式：有 exp3 → 原索引路径（零行为变化）；无 exp3（bai）→ 参数帧路径 */
    private enum class PathMode { EXPRESSION_INDEX, PARAM_FRAME }

    /** 已记录的 pathMode（仅模式变化时打日志，A3S-01/06 判据源） */
    private var loggedPathMode: PathMode? = null

    /**
     * 双路径分发判据（AP-03/04）：渲染层无 exp3（bai）→ PARAM_FRAME；
     * 无 exp3（bai）→ PARAM_FRAME（现状行为）；
     * hasExpressions 查询异常 → 保守回退 EXPRESSION_INDEX（不引入新异常面）。
     */
    private fun pathMode(): PathMode {
        val mode = try {
            if (live2DView.hasExpressions()) PathMode.EXPRESSION_INDEX else PathMode.PARAM_FRAME
        } catch (e: Throwable) {
            logWarn("pathMode: hasExpressions 查询异常，保守回退 EXPRESSION_INDEX")
            PathMode.EXPRESSION_INDEX
        }
        if (loggedPathMode != mode) {
            loggedPathMode = mode
            logInfo("pathMode=$mode")
        }
        return mode
    }

    /**
     * A3 参数帧数据源（契约 L31：map 注入**或**内部持有，注入优先）：
     * 构造注入 emotionParamMapOverride > adapter.emotionParamMap（运行时可加载，bai 单源）>
     * 硬编码 bai 默认帧兜底（[DEFAULT_BAI_EMOTION_PARAM_MAP]）。
     * 兜底对齐 [FALLBACK_EMOTION_MAP] 模式：adapter 未加载（测试 JVM）/解析失败场景不丢参数路径；
     * 无 emotionParamMap 且走 EXPRESSION_INDEX 路径，兜底帧不生效（AP-28 零回归）。
     */
    private val emotionParamMap: Map<String, Map<String, Map<String, Float>>>
        get() = emotionParamMapOverride
            ?: ModelAdapterConfig.currentAdapter()?.emotionParamMap?.takeIf { it.isNotEmpty() }
            ?: DEFAULT_BAI_EMOTION_PARAM_MAP

    /** A3 fade：emotionSwitch（协议 H 段 300ms；adapter.fadeMs 单源，缺失 → 默认 300） */
    private fun emotionSwitchFadeMs(): Int =
        ModelAdapterConfig.currentAdapter()?.fadeMs?.emotionSwitch ?: 300

    /** A3 fade：emotionToNeutral（协议 H 段 500ms；adapter.fadeMs 单源，缺失 → 默认 500） */
    private fun emotionToNeutralFadeMs(): Int =
        ModelAdapterConfig.currentAdapter()?.fadeMs?.emotionToNeutral ?: 500

    /**
     * A3 参数路径表情应用（AP-05/08/20/21/25/26，D2-A/D3/D4 定案）。
     *
     * - 非 neutral 表情 → 对应强度档帧（_1/_2/_3 离散选档，D3）+ fadeMs.emotionSwitch（300ms）+ 暂停自动眨眼
     * - neutral（回 neutral）→ neutral._1 帧 + fadeMs.emotionToNeutral（500ms）+ 恢复自动眨眼
     */
    private fun applyParamFrameEmotion(emotion: String, intensity: Int, isNeutralTransition: Boolean) {
        val frame = if (isNeutralTransition) {
            emotionParamMap["neutral"]?.get("_1")
        } else {
            emotionParamMap[emotion]?.get("_${intensity}")
        }
        if (frame == null) {
            logWarn("applyParamFrameEmotion: 参数路径无帧 emotion=$emotion 档位=$intensity，忽略")
            return
        }
        // R16 半身姿态联动（task-C7b）：姿态帧（BodyAngle 3 + Angle 3 = 6 键）并入表情帧 → 12 键单帧、
        // 一次 setParametricExpression。选 a（合并）而非 b（独立二次调用）的依据：
        // ExpressionManager.setExpression 互斥替换（AnimationSystem.kt）——二次调用会把表情帧降为淡出对象，
        // fade 结束后被丢弃（表情面丢失），故必须单帧合并。键集零交集（T7b BPP-05）→ map 合并无覆盖；
        // 姿态键 ∈ bai moc3 白名单（model-facts-bai §三 实读），renderer 按 paramNameToIndex 过滤，
        // 缺失键仅跳过（不抛）。neutral 过渡 → neutral._1 姿态帧（全 0，无姿态基准，idle 不判激活）。
        val poseFrame = if (isNeutralTransition) {
            BodyPoseFrames.frameOf("neutral", 1)
        } else {
            BodyPoseFrames.frameOf(emotion, intensity)
        }
        val merged = if (poseFrame != null) frame + poseFrame else frame
        val fadeMs = if (isNeutralTransition) emotionToNeutralFadeMs() else emotionSwitchFadeMs()
        live2DView.setParametricExpression(merged, fadeMs)
        // D2-A（contract AP-20/21）：非 neutral 表情激活期间暂停自动眨眼（false）；回 neutral 恢复（true）。
        // 布尔赋值幂等（AP-24 允许重复赋值无副作用）。注意语义：enabled=true = 眨眼开启，故回 neutral → true。
        live2DView.setAutoBlink(isNeutralTransition)
        logInfo("applyParamFrameEmotion: emotion=$emotion 帧=${merged.size}键（表情${frame.size}+姿态${poseFrame?.size ?: 0}）fade=${fadeMs}ms autoBlink=$isNeutralTransition")
    }

    /**
     * A3 D5 加权合成（contract-bai-params-v0 §0）：多表情 mix → 逐参数加权求和单帧。
     * 每项取 emotionParamMap[emotion]["_3"] 满帧 × 归一化 weight；缺帧项跳过（部分生效，同索引路径）；
     * 全部缺帧 → null（调用方表情层不动作）。
     */
    private fun synthesizeParamFrame(
        mix: List<ExpressionWeight>,
        paramMap: Map<String, Map<String, Map<String, Float>>>
    ): Map<String, Float>? {
        // Double 中间累积（Float 帧值 × Float weight 提升为 Double 求和，末次转回 Float）：
        // 避免 float32 逐项舍入误差累积（中间结果更接近协议 D5 十进制算例，最终 float32 位型与
        // float32 直接累积一致；个别 key 与十进制算例相差 ≤1 ULP，属二进制浮点固有舍入）。
        val acc = mutableMapOf<String, Double>()
        for (ew in mix) {
            val frame = paramMap[ew.name]?.get("_3") ?: continue
            for ((key, value) in frame) {
                acc[key] = (acc[key] ?: 0.0) + value.toDouble() * ew.weight.toDouble()
            }
        }
        if (acc.isEmpty()) return null
        return acc.mapValues { it.value.toFloat() }
    }

    /**
     * A3 D5-1 overrides 乘数域：final[p] = 合成帧[p] × multiplier[p] 逐 key；
     * 仅对合成帧键生效，帧外 key 丢弃（记日志）。
     */
    private fun applyOverrideMultipliers(
        frame: Map<String, Float>,
        overrides: Map<String, Float>
    ): Map<String, Float> {
        if (overrides.isEmpty()) return frame
        val result = frame.toMutableMap()
        var dropped = 0
        for ((key, multiplier) in overrides) {
            val current = result[key]
            if (current != null) {
                result[key] = current * multiplier
            } else {
                dropped++
                logWarn("applyStructured: overrides key '$key' 不在合成帧内，丢弃（D5-1）")
            }
        }
        if (dropped > 0) {
            logInfo("applyStructured: overrides 丢弃 $dropped 个帧外 key")
        }
        return result
    }

    /**
     * A3 参数路径 applyStructured（AP-06，D5/D5-1 定案）：
     * - 非空 mix → 加权合成单帧（_3 满帧 × 归一化 weight）→ overrides 乘数 → setParametricExpression(帧, 300)
     * - 空 mix + overrides → neutral._1 基底 × overrides → setParametricExpression(帧, 300)
     * - 空 mix 无 overrides → 表情层不动作（不清除状态）
     * - autoBlink：mix 含任一非 neutral → 暂停眨眼；否则恢复
     */
    private fun applyStructuredParamFrame(result: EmotionResult) {
        val mix = result.expressionMix
        val overrides = result.parameterOverrides
        val paramMap = emotionParamMap

        if (mix.isEmpty()) {
            if (overrides.isEmpty()) {
                logInfo("applyStructured: 空 mix 无 overrides，参数路径表情层不动作")
                return
            }
            val base = paramMap["neutral"]?.get("_1")
            if (base == null) {
                logWarn("applyStructured: 参数路径 neutral._1 帧缺失，表情层不动作")
                return
            }
            val frame = applyOverrideMultipliers(base, overrides)
            live2DView.setParametricExpression(frame, emotionSwitchFadeMs())
            live2DView.setAutoBlink(true)
            logInfo("applyStructured: 空 mix+overrides → neutral 基底帧（${frame.size} 键）autoBlink 恢复")
            return
        }

        val synthesized = synthesizeParamFrame(mix, paramMap)
        if (synthesized == null) {
            logWarn("applyStructured: 参数路径合成帧为空（emotionParamMap 缺帧），表情层不动作")
            return
        }
        val frame = if (overrides.isEmpty()) synthesized else applyOverrideMultipliers(synthesized, overrides)
        live2DView.setParametricExpression(frame, emotionSwitchFadeMs())
        val anyNonNeutral = mix.any { it.name != "neutral" }
        live2DView.setAutoBlink(!anyNonNeutral)
        logInfo("applyStructured: 合成帧 ${frame.size} 键（mix=${mix.size}）autoBlink=${!anyNonNeutral}")
    }

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
            // A3 双路径分发（contract-bai-params-v0 §1）：无 exp3（bai）→ 参数帧路径；有 exp3 → 原索引路径（零行为变化）
            if (pathMode() == PathMode.PARAM_FRAME) {
                applyParamFrameEmotion(lastValidEmotion, lastIntensity, lastValidEmotion == "neutral")
            } else {
                val expressionIndex = EMOTION_MAP[lastValidEmotion] ?: 0
                val intensity = lastIntensity / 3.0f
                if (expressionIndex != currentExpressionIndex || intensity != currentIntensity) {
                    currentExpressionIndex = expressionIndex
                    currentIntensity = intensity
                    live2DView.setExpression(expressionIndex, intensity)
                    logInfo("parseAndApply: 标签[$lastValidEmotion] index=$expressionIndex intensity=${lastIntensity}")
                }
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
        // A3 双路径分发：无 exp3（bai）→ 参数帧路径（AP-08：满强度 → _3 帧）；有 exp3 → 原索引路径（零行为变化）
        if (pathMode() == PathMode.PARAM_FRAME) {
            applyParamFrameEmotion(keyword, 3, keyword == "neutral")
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
        // A3 双路径分发：无 exp3（bai）→ 参数帧路径（D5 加权合成）；有 exp3 → 原索引路径（零行为变化）
        if (pathMode() == PathMode.PARAM_FRAME) {
            applyStructuredParamFrame(result)
            return
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
        // A3 双路径分发：无 exp3（bai）→ 参数帧路径（AP-07/26）；有 exp3 → 原索引路径（零行为变化）
        if (pathMode() == PathMode.PARAM_FRAME) {
            // neutral._1 帧 + fadeMs.emotionToNeutral（500ms）+ autoBlink 恢复 true
            val frame = emotionParamMap["neutral"]?.get("_1")
            live2DView.setAutoBlink(true)
            if (frame != null) {
                live2DView.setParametricExpression(frame, emotionToNeutralFadeMs())
                logInfo("resetToNeutral: 参数路径 neutral._1 fade=${emotionToNeutralFadeMs()}ms autoBlink 恢复")
            } else {
                logWarn("resetToNeutral: 参数路径 neutral._1 帧缺失，仅恢复 autoBlink")
            }
            return
        }
        // AP-28：resetToNeutral → setExpression(0)（无条件；重复调用幂等，同现状视觉效果）
        currentExpressionIndex = 0
        live2DView.setExpression(0)
        logInfo("resetToNeutral: -> expressionIndex=0")
    }
}
