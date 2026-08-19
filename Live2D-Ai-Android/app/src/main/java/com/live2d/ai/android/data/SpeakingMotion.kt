package com.live2d.ai.android.data

import kotlin.math.PI
import kotlin.math.sin

// ============================================================
//  R17 说话联动增强（SpeakingMotion）：头部点头 + 身体起伏
//
//  契约: .pi/spoq/T8-benchmark-red.md §三 + SpeakingMotionTest.kt 文件头
//  （tester 锁值 byte-exact，禁止改语义）
//
//  - 头部点头: 低频正弦 0.5-1Hz → 周期 1.5s ∈ [1,2]s；幅度 ±4°（需求 ±3-5°，取 4）
//  - 身体起伏: 周期 2.4s（≠ 呼吸周期 2.5s → 走「周期不同」错相位分支）；幅度 ±2°（需求 [0,3]°）
//  - 「与口型同步节奏」tester 裁定: 口型 ~100ms 快波动由 LipSync 独立负责，
//    本类只锁点头慢节拍周期 ∈ [1,2]s（不编造嘴型周期接口）
//
//  纯 JVM object（无 native / android.* 依赖），可确定性单测。
//  接入点: AnimationPipeline（说话激活 → 经 LiaisonArbiter 仲裁写 ParamAngleX / ParamBodyAngleY）。
// ============================================================

/**
 * R17 说话联动纯状态类（头部点头 + 身体起伏，双低频正弦）。
 */
object SpeakingMotion {

    /** 头部点头周期（s）——需求 0.5-1Hz → 周期 1-2s，取 1.5s（tester 锁值） */
    const val NOD_PERIOD_SECONDS: Float = 1.5f

    /** 头部点头幅度（°）——需求 ±3-5°，取 4（tester 锁值） */
    const val NOD_AMPLITUDE_HEAD_DEG: Float = 4f

    /** 身体起伏周期（s）——与呼吸(2.5s)错相位=周期不同；低频（tester 锁值） */
    const val BODY_RISE_PERIOD_SECONDS: Float = 2.4f

    /** 身体起伏幅度（°）——需求 [0,3]°，取 2（tester 锁值） */
    const val BODY_RISE_AMPLITUDE_DEG: Float = 2f

    /** 呼吸周期（s）——官方呼吸 0.3-0.5Hz → 周期 2-3.3s，取 2.5s（tester 锁值） */
    const val BREATH_PERIOD_SECONDS: Float = 2.5f

    /**
     * 头部点头偏移（°）: A·sin(2π·t / T) ∈ [-NOD_AMPLITUDE, NOD_AMPLITUDE] ⊆ [-5,5]。
     * t=0 / 半周期 → 0；t=1/4 周期 → 峰值 ≈ 幅度。
     */
    fun headNodOffset(timeSeconds: Float): Float {
        val t = timeSeconds.toDouble()
        return (NOD_AMPLITUDE_HEAD_DEG.toDouble() * sin(2.0 * PI * t / NOD_PERIOD_SECONDS.toDouble())).toFloat()
    }

    /**
     * 身体起伏偏移（°）: A·sin(2π·t / T) ∈ [-BODY_RISE_AMPLITUDE, BODY_RISE_AMPLITUDE] ⊆ [-3,3]。
     * 周期(2.4s) ≠ 呼吸周期(2.5s) → 长期相位漂移（错相位）。
     */
    fun bodyRiseOffset(timeSeconds: Float): Float {
        val t = timeSeconds.toDouble()
        return (BODY_RISE_AMPLITUDE_DEG.toDouble() * sin(2.0 * PI * t / BODY_RISE_PERIOD_SECONDS.toDouble())).toFloat()
    }
}
