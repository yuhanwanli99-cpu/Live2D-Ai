package com.live2d.ai.android.data

// ============================================================
//  R16 半身姿态联动：身体/头部姿态参数帧（BodyPoseFrames）
//
//  数据源 = .pi/spoq/T7b-bodypose-red.md §二 值表（tester 锁定 byte-exact；
//  8 情绪 × 3 强度档 × 6 键 = 144 值，全值 ∈ [-30,30]，neutral 全档全 0）。
//
//  轴语义（recon-techdebt-feasibility.md A.1，cdi3 实读轴结构）：
//  - ParamAngleX = 头点头/低头（pitch）；ParamAngleY = 头左右转（yaw）；ParamAngleZ = 头歪头（roll）
//  - ParamBodyAngleX = 身体前倾/后仰（pitch）；ParamBodyAngleY = 身体左右转体（yaw）；
//    ParamBodyAngleZ = 身体侧倾/摇摆（roll）
//  - 物理正确轴映射：低头→AX 负、前倾→BAX 负、后仰→BAX 正、抬→AX 正（T7b §五 裁定）
//
//  与 R13 ExpressionFrames 的关系（BPP-05 断言）：
//  - 本类 6 键（身体 3 + 头 3）∩ ExpressionFrames 6 键（Brow/Eye/Mouth）= ∅（零交集，不挤占表情面）
//  - EMOTIONS 与 R13 一致（bai 权威 8 键）
//
//  纯 JVM object（无 native / android.* 依赖），可确定性单测。
//  帧值为 Float（与 A3 代码面 emotionParamMap 的 Map<String, Float> 语义一致）。
// ============================================================

/**
 * R16 半身姿态参数帧数据源（8 情绪 × 3 强度 → ParamBodyAngleX/Y/Z + ParamAngleX/Y/Z）。
 */
object BodyPoseFrames {

    // ── 三轴合法值域常量（官方标准参数表；search-report-expression.md §2.1 锚定 [-30,30]）──

    /** 角度下限（官方标准参数表，ParamAngleX/Y/Z ∈ [-30,30]） */
    const val ANGLE_MIN = -30f

    /** 角度上限（官方标准参数表；BodyAngle 同官方角度范围） */
    const val ANGLE_MAX = 30f

    /** 8 情绪键（bai 权威键集，与 ExpressionFrames.EMOTIONS 一致） */
    val EMOTIONS: List<String> = listOf(
        "neutral", "fear", "sadness", "anger", "disgust", "joy", "smirk", "surprise"
    )

    /** 3 强度档（离散选档，契约 D3 同 R13：非连续插值） */
    val LEVELS: List<Int> = listOf(1, 2, 3)

    /** 身体 3 + 头 3 = 6 键白名单（与 ExpressionFrames 三轴 6 键零交集） */
    val POSE_KEYS: Set<String> = setOf(
        "ParamBodyAngleX", "ParamBodyAngleY", "ParamBodyAngleZ",
        "ParamAngleX", "ParamAngleY", "ParamAngleZ",
    )

    /** 六轴统一值域 = 官方 [-30,30] */
    val POSE_RANGES: Map<String, ClosedFloatingPointRange<Float>> = mapOf(
        "ParamBodyAngleX" to -30f..30f,
        "ParamBodyAngleY" to -30f..30f,
        "ParamBodyAngleZ" to -30f..30f,
        "ParamAngleX"     to -30f..30f,
        "ParamAngleY"     to -30f..30f,
        "ParamAngleZ"     to -30f..30f,
    )

    /**
     * 取 8 情绪 × 3 强度档姿态帧（6 键，T7b §二 值表 byte-exact）。
     *
     * @param emotion 情绪键（bai 权威键，如 "joy"）
     * @param level 强度档 1..3（离散选档，非连续插值）；缺省 → 3（满强度）
     * @return 6 键帧 Map<String,Float>；未知情绪/档位 → null
     */
    fun frameOf(emotion: String, level: Int = 3): Map<String, Float>? = FRAMES[emotion]?.get("_$level")

    /**
     * 校验姿态帧是否合法（BPP-03-②）：
     * 非空；全部键 ∈ [POSE_KEYS] 6 键白名单（未知面键 → false）；
     * 全部值 ∈ [-30,30]（越界 31/-31 → false）。
     */
    fun isValidBodyPoseFrame(frame: Map<String, Float>): Boolean {
        if (frame.isEmpty()) return false
        for ((key, value) in frame) {
            val range = POSE_RANGES[key] ?: return false
            if (value !in range) return false
        }
        return true
    }

    // ── T7b 值表（tester 锁定 byte-exact；144 值；键序 BAX=ParamBodyAngleX, BAY=ParamBodyAngleY,
    //    BAZ=ParamBodyAngleZ, AX=ParamAngleX, AY=ParamAngleY, AZ=ParamAngleZ）──
    private val FRAMES: Map<String, Map<String, Map<String, Float>>> = mapOf(
        "neutral" to mapOf(
            "_1" to mapOf("ParamBodyAngleX" to 0f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to 0f, "ParamAngleX" to 0f, "ParamAngleY" to 0f, "ParamAngleZ" to 0f),
            "_2" to mapOf("ParamBodyAngleX" to 0f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to 0f, "ParamAngleX" to 0f, "ParamAngleY" to 0f, "ParamAngleZ" to 0f),
            "_3" to mapOf("ParamBodyAngleX" to 0f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to 0f, "ParamAngleX" to 0f, "ParamAngleY" to 0f, "ParamAngleZ" to 0f),
        ),
        "joy" to mapOf(
            "_1" to mapOf("ParamBodyAngleX" to 2f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to 2f, "ParamAngleX" to 2f, "ParamAngleY" to 3f, "ParamAngleZ" to 1f),
            "_2" to mapOf("ParamBodyAngleX" to 3f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to 4f, "ParamAngleX" to 4f, "ParamAngleY" to 6f, "ParamAngleZ" to 2f),
            "_3" to mapOf("ParamBodyAngleX" to 5f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to 6f, "ParamAngleX" to 6f, "ParamAngleY" to 8f, "ParamAngleZ" to 3f),
        ),
        "sadness" to mapOf(
            "_1" to mapOf("ParamBodyAngleX" to -3f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to -1f, "ParamAngleX" to -4f, "ParamAngleY" to 0f, "ParamAngleZ" to 0f),
            "_2" to mapOf("ParamBodyAngleX" to -5f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to -2f, "ParamAngleX" to -8f, "ParamAngleY" to 0f, "ParamAngleZ" to 0f),
            "_3" to mapOf("ParamBodyAngleX" to -8f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to -3f, "ParamAngleX" to -12f, "ParamAngleY" to 0f, "ParamAngleZ" to 0f),
        ),
        "anger" to mapOf(
            "_1" to mapOf("ParamBodyAngleX" to -3f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to 0f, "ParamAngleX" to -2f, "ParamAngleY" to -1f, "ParamAngleZ" to 0f),
            "_2" to mapOf("ParamBodyAngleX" to -7f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to 0f, "ParamAngleX" to -4f, "ParamAngleY" to -2f, "ParamAngleZ" to 0f),
            "_3" to mapOf("ParamBodyAngleX" to -12f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to 0f, "ParamAngleX" to -6f, "ParamAngleY" to -3f, "ParamAngleZ" to 0f),
        ),
        "disgust" to mapOf(
            "_1" to mapOf("ParamBodyAngleX" to 6f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to 0f, "ParamAngleX" to 0f, "ParamAngleY" to -2f, "ParamAngleZ" to -1f),
            "_2" to mapOf("ParamBodyAngleX" to 10f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to 0f, "ParamAngleX" to 0f, "ParamAngleY" to -4f, "ParamAngleZ" to -2f),
            "_3" to mapOf("ParamBodyAngleX" to 14f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to 0f, "ParamAngleX" to 0f, "ParamAngleY" to -6f, "ParamAngleZ" to -3f),
        ),
        "fear" to mapOf(
            "_1" to mapOf("ParamBodyAngleX" to 5f, "ParamBodyAngleY" to -2f, "ParamBodyAngleZ" to 0f, "ParamAngleX" to -1f, "ParamAngleY" to 0f, "ParamAngleZ" to 0f),
            "_2" to mapOf("ParamBodyAngleX" to 9f, "ParamBodyAngleY" to -4f, "ParamBodyAngleZ" to 0f, "ParamAngleX" to -3f, "ParamAngleY" to 0f, "ParamAngleZ" to 0f),
            "_3" to mapOf("ParamBodyAngleX" to 13f, "ParamBodyAngleY" to -6f, "ParamBodyAngleZ" to 0f, "ParamAngleX" to -5f, "ParamAngleY" to 0f, "ParamAngleZ" to 0f),
        ),
        "smirk" to mapOf(
            "_1" to mapOf("ParamBodyAngleX" to 0f, "ParamBodyAngleY" to 2f, "ParamBodyAngleZ" to -2f, "ParamAngleX" to 1f, "ParamAngleY" to 0f, "ParamAngleZ" to 3f),
            "_2" to mapOf("ParamBodyAngleX" to 0f, "ParamBodyAngleY" to 4f, "ParamBodyAngleZ" to -4f, "ParamAngleX" to 2f, "ParamAngleY" to 0f, "ParamAngleZ" to 6f),
            "_3" to mapOf("ParamBodyAngleX" to 0f, "ParamBodyAngleY" to 6f, "ParamBodyAngleZ" to -6f, "ParamAngleX" to 3f, "ParamAngleY" to 0f, "ParamAngleZ" to 9f),
        ),
        "surprise" to mapOf(
            "_1" to mapOf("ParamBodyAngleX" to 8f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to 0f, "ParamAngleX" to 3f, "ParamAngleY" to 0f, "ParamAngleZ" to 0f),
            "_2" to mapOf("ParamBodyAngleX" to 13f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to 0f, "ParamAngleX" to 6f, "ParamAngleY" to 0f, "ParamAngleZ" to 0f),
            "_3" to mapOf("ParamBodyAngleX" to 18f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to 0f, "ParamAngleX" to 10f, "ParamAngleY" to 0f, "ParamAngleZ" to 0f),
        ),
    )
}
