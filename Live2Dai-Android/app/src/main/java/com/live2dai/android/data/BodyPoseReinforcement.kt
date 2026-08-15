package com.live2dai.android.data

// ============================================================
//  R17 动作层强化（BodyPoseReinforcement）：情绪姿态幅度加大（新档位）
//
//  契约: .pi/spoq/T8-benchmark-red.md §三 + BodyPoseReinforcementTest.kt 文件头
//
//  T20 裁定（tester）: 【新档位（reinforced tier），NOT 覆盖旧值】——
//  T7b BodyPoseFrames 已 GREEN（547/547），覆盖会回归既有测试；R17 强化以本独立类叠加，
//  旧 BodyPoseFrames 值表字节级保留。范围: 仅 joy/sadness/surprise 3 情绪强化
//  （joy 身摆 ±8-12° / sad 低头 -15° / surprise 后仰 +15-20°），
//  其余 5 情绪 + neutral 回退 BodyPoseFrames 基准值（不发明新值）。
//
//  轴语义沿用 T7b 物理正确轴（recon-techdebt-feasibility.md A.1）:
//  低头/抬头=ParamAngleX（头 pitch）、前倾/后仰=ParamBodyAngleX、侧摆/侧倾=ParamBodyAngleZ、
//  点头/转体=ParamAngleY。任务字面轴名冲突已在 T7b 上报，本类沿用物理正确轴。
//
//  强化值表 byte-exact（T8 §二，键序 BAX BAY BAZ AX AY AZ）:
//  | joy     | 1 |  4| 0|  8| 4| 5| 2 | 身摆 BAZ=8（∈[8,12]）|
//  | joy     | 2 |  5| 0| 10| 6| 8| 3 | ↑ 摆幅10 |
//  | joy     | 3 |  6| 0| 12| 8|10| 4 | ↑ 摆幅12（≤12）|
//  | sadness | 1 | -6| 0| -2|-15| 0| 0 | 头深低头 AX=-15 |
//  | sadness | 2 | -9| 0| -3|-15| 0| 0 | ↑ 身下沉加深 |
//  | sadness | 3 |-12| 0| -4|-15| 0| 0 | ↑ |
//  | surprise| 1 | 15| 0|  0| 6| 0| 0 | 身后仰 BAX=15（∈[15,20]）|
//  | surprise| 2 | 18| 0|  0| 8| 0| 0 | ↑ |
//  | surprise| 3 | 20| 0|  0|10| 0| 0 | ↑ 后仰 20（≤20）|
//  （全值 ∈ [-30,30]；BAY 全 0；neutral/其它回退 BodyPoseFrames）
//
//  纯 JVM object（无 native / android.* 依赖），可确定性单测。
//  接入点: EmotionController.applyParamFrameEmotion（姿态帧改走本类强化档位）。
// ============================================================

/**
 * R17 情绪姿态强化档位（joy/sadness/surprise 幅度加大；其余回退 BodyPoseFrames 基准）。
 */
object BodyPoseReinforcement {

    /** R17 实锤强化情绪元组（3 元组，tester 锁定） */
    val REINFORCED_EMOTIONS: Set<String> = setOf("joy", "sadness", "surprise")

    /** 6 键（身体 3 + 头 3），键序与 BodyPoseFrames 一致（BAX BAY BAZ AX AY AZ） */
    val POSE_KEYS: List<String> = listOf(
        "ParamBodyAngleX", "ParamBodyAngleY", "ParamBodyAngleZ",
        "ParamAngleX", "ParamAngleY", "ParamAngleZ",
    )

    /** 情绪是否为强化情绪（精确匹配 3 元组） */
    fun isReinforcedEmotion(emotion: String): Boolean = emotion in REINFORCED_EMOTIONS

    /**
     * 取强化档位姿态帧（6 键）:
     * - emotion ∈ {joy,sadness,surprise} 且 level ∈ 1..3 → 强化值表（byte-exact）
     * - 其它情绪（T7b 5 情绪 + neutral）→ [BodyPoseFrames.frameOf] 基准（回退，不强加新值）
     * - 未知情绪 / 非法档位（level ∉ 1..3，如 0/4）→ null
     */
    fun reinforcedFrameOf(emotion: String, level: Int = 3): Map<String, Float>? {
        if (level !in 1..3) return null
        REINFORCED_FRAMES[emotion]?.get("_$level")?.let { return it }
        if (emotion in REINFORCED_EMOTIONS) return null
        return BodyPoseFrames.frameOf(emotion, level)
    }

    // ── T8 §二 强化值表（tester 锁定 byte-exact；键序 BAX BAY BAZ AX AY AZ）──
    private val REINFORCED_FRAMES: Map<String, Map<String, Map<String, Float>>> = mapOf(
        "joy" to mapOf(
            "_1" to mapOf("ParamBodyAngleX" to 4f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to 8f, "ParamAngleX" to 4f, "ParamAngleY" to 5f, "ParamAngleZ" to 2f),
            "_2" to mapOf("ParamBodyAngleX" to 5f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to 10f, "ParamAngleX" to 6f, "ParamAngleY" to 8f, "ParamAngleZ" to 3f),
            "_3" to mapOf("ParamBodyAngleX" to 6f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to 12f, "ParamAngleX" to 8f, "ParamAngleY" to 10f, "ParamAngleZ" to 4f),
        ),
        "sadness" to mapOf(
            "_1" to mapOf("ParamBodyAngleX" to -6f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to -2f, "ParamAngleX" to -15f, "ParamAngleY" to 0f, "ParamAngleZ" to 0f),
            "_2" to mapOf("ParamBodyAngleX" to -9f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to -3f, "ParamAngleX" to -15f, "ParamAngleY" to 0f, "ParamAngleZ" to 0f),
            "_3" to mapOf("ParamBodyAngleX" to -12f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to -4f, "ParamAngleX" to -15f, "ParamAngleY" to 0f, "ParamAngleZ" to 0f),
        ),
        "surprise" to mapOf(
            "_1" to mapOf("ParamBodyAngleX" to 15f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to 0f, "ParamAngleX" to 6f, "ParamAngleY" to 0f, "ParamAngleZ" to 0f),
            "_2" to mapOf("ParamBodyAngleX" to 18f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to 0f, "ParamAngleX" to 8f, "ParamAngleY" to 0f, "ParamAngleZ" to 0f),
            "_3" to mapOf("ParamBodyAngleX" to 20f, "ParamBodyAngleY" to 0f, "ParamBodyAngleZ" to 0f, "ParamAngleX" to 10f, "ParamAngleY" to 0f, "ParamAngleZ" to 0f),
        ),
    )
}
