package com.live2dai.android.data

// ============================================================
//  R13 角色表现力：表情参数帧（ExpressionFrames）
//
//  数据源 = assets/bai.adapter.json emotionParamMap 真值（byte-exact 逐键镜像；
//  契约 A3U-01/03：8 情绪 × 3 强度档 × 6 键 = 144 值，实测 max=1.5 surprise._3 EyeLOpen、
//  min=-0.75 anger._3 MouthForm；smirk._3 MouthForm=-0.7 为 T6 锚点）。
//
//  官方表情机制语义（search-report-expression.md §3.2）：
//  - 情绪帧 = 参数差叠加（Overwrite 绝对目标值，经 ExpressionManager 按帧叠加应用）
//  - 眼神（ParamEyeBallX/Y）**不在**情绪帧内——眼神归程序化层 IdleMotionController
//  - MouthOpenY：neutral 帧 = 0（不挡口型）；非 neutral 帧按 bai 真值非零
//    （说话时口型让位由 LipSync「最近写入者胜」兜底，contract-bai-params-v0 §6.1）
//
//  纯 JVM object（无 native / android.* 依赖），可确定性单测。
//  帧值为 Float（与 A3 代码面 emotionParamMap 的 Map<String, Float> 语义一致）。
// ============================================================

/**
 * R13 表情参数帧数据源（bai emotionParamMap 真值镜像）。
 */
object ExpressionFrames {

    // ── 三轴合法值域常量（R13 契约文件头锁定值）──

    /** 眉高下限（官方标准参数表） */
    const val BROW_MIN = -1f

    /** 眉高上限（官方标准参数表） */
    const val BROW_MAX = 1f

    /** 眼开合下限 */
    const val EYE_OPEN_MIN = 0f

    /** 眼开合上限（官方 [0,1] 放宽 1.5：fear/surprise clamp 面） */
    const val EYE_OPEN_MAX = 1.5f

    /** 嘴形下限 */
    const val MOUTH_FORM_MIN = -1f

    /** 嘴形上限 */
    const val MOUTH_FORM_MAX = 1f

    /** 嘴开合上限（官方 MouthOpenY [0,1] 可到 2；bai 情绪帧实测 ≤0.7） */
    const val MOUTH_OPEN_MAX = 1.5f

    /** 8 情绪键（bai 权威键集：neutral/fear/sadness/anger/disgust/joy/smirk/surprise） */
    val EMOTIONS: List<String> = listOf(
        "neutral", "fear", "sadness", "anger", "disgust", "joy", "smirk", "surprise"
    )

    /** 3 强度档（离散选档，契约 D3：非连续插值） */
    val LEVELS: List<Int> = listOf(1, 2, 3)

    /** 三轴 6 键白名单（EF-01-②：每帧恰 6 键） */
    val AXIS_KEYS: Set<String> = setOf(
        "ParamBrowLY", "ParamBrowRY",
        "ParamEyeLOpen", "ParamEyeROpen",
        "ParamMouthForm", "ParamMouthOpenY"
    )

    /** 各轴合法值域（官方标准参数表 + 契约 A3U-03；EF-02-①） */
    val AXIS_RANGES: Map<String, ClosedFloatingPointRange<Float>> = mapOf(
        "ParamBrowLY" to BROW_MIN..BROW_MAX,
        "ParamBrowRY" to BROW_MIN..BROW_MAX,
        "ParamEyeLOpen" to EYE_OPEN_MIN..EYE_OPEN_MAX,
        "ParamEyeROpen" to EYE_OPEN_MIN..EYE_OPEN_MAX,
        "ParamMouthForm" to MOUTH_FORM_MIN..MOUTH_FORM_MAX,
        "ParamMouthOpenY" to EYE_OPEN_MIN..MOUTH_OPEN_MAX,
    )

    /**
     * 取 8 情绪 × 3 强度档参数帧（逐键 byte-exact 对齐 bai.adapter.json 真值）。
     *
     * @param emotion 情绪键（bai 权威键，如 "joy"）
     * @param level 强度档 1..3（离散选档，非连续插值）；缺省 → 3（满强度）
     * @return 6 键帧 Map<String,Float>；未知情绪/档位 → null
     */
    fun frameOf(emotion: String, level: Int = 3): Map<String, Float>? = FRAMES[emotion]?.get("_$level")

    /**
     * 从外部数据源（bai.adapter.json 的 emotionParamMap 结构）抽取 (emotion, level) 帧。
     *
     * @param emotionParamMap 数据源：外键=情绪键，中键=强度档（"_1"/"_2"/"_3"），内=6 键参数表
     * @param emotion 情绪键（bai 权威键，如 "joy"）
     * @param level 强度档 1..3（离散选档）
     * @return 6 键帧 Map<String,Float>；未知情绪/档位 → null
     */
    fun frameOf(
        emotionParamMap: Map<String, Map<String, Map<String, Float>>>,
        emotion: String,
        level: Int,
    ): Map<String, Float>? = emotionParamMap[emotion]?.get("_$level")

    /** [frameOf] 的档位字符串重载（"_3" 或 "3" 均可） */
    fun frameOf(emotion: String, level: String): Map<String, Float>? {
        val key = if (level.startsWith("_")) level else "_$level"
        return FRAMES[emotion]?.get(key)
    }

    /** [frameOf] 的档位浮点重载（1.0/2.0/3.0） */
    fun frameOf(emotion: String, level: Double): Map<String, Float>? {
        val intLevel = level.toInt()
        return if (intLevel in 1..3) FRAMES[emotion]?.get("_$intLevel") else null
    }

    /**
     * 校验参数帧是否合法（EF-02-③）：
     * 非空；全部键 ∈ 三轴 6 键白名单（含 EyeBall 键 → false——眼神归程序化层）；
     * 全部值在轴值域内（BrowLY/RY∈[-1,1]、EyeLOpen/ROpen∈[0,1.5]、MouthForm∈[-1,1]、MouthOpenY∈[0,1.5]）。
     */
    fun isValidEmotionFrame(frame: Map<String, Float>): Boolean {
        if (frame.isEmpty()) return false
        for ((key, value) in frame) {
            val range = AXIS_RANGES[key] ?: return false
            if (value !in range) return false
        }
        return true
    }

    /** [isValidEmotionFrame] 的情绪+档位形式（校验数据源对应帧） */
    fun isValidEmotionFrame(emotion: String, level: Int): Boolean =
        frameOf(emotion, level)?.let { isValidEmotionFrame(it) } ?: false

    // ── bai.adapter.json emotionParamMap 真值（byte-exact；144 值）──
    private val FRAMES: Map<String, Map<String, Map<String, Float>>> = mapOf(
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
}

// ── 顶层便捷函数（与 ExpressionFrames 成员同签名；测试按顶层函数引用）──

/** 取 8 情绪 × 3 强度档参数帧（委托 [ExpressionFrames.frameOf]；档位缺省 → 3） */
fun frameOf(emotion: String, level: Int = 3): Map<String, Float>? = ExpressionFrames.frameOf(emotion, level)

/** 从外部数据源抽取 (emotion, level) 帧（委托 [ExpressionFrames.frameOf]） */
fun frameOf(
    emotionParamMap: Map<String, Map<String, Map<String, Float>>>,
    emotion: String,
    level: Int,
): Map<String, Float>? = ExpressionFrames.frameOf(emotionParamMap, emotion, level)

/** 取参数帧（档位字符串形式，"_3" 或 "3"） */
fun frameOf(emotion: String, level: String): Map<String, Float>? = ExpressionFrames.frameOf(emotion, level)

/** 取参数帧（档位浮点形式，1.0/2.0/3.0） */
fun frameOf(emotion: String, level: Double): Map<String, Float>? = ExpressionFrames.frameOf(emotion, level)

/** 校验参数帧是否合法（委托 [ExpressionFrames.isValidEmotionFrame]） */
fun isValidEmotionFrame(frame: Map<String, Float>): Boolean = ExpressionFrames.isValidEmotionFrame(frame)

/** 校验参数帧是否合法（情绪+档位形式） */
fun isValidEmotionFrame(emotion: String, level: Int): Boolean = ExpressionFrames.isValidEmotionFrame(emotion, level)
