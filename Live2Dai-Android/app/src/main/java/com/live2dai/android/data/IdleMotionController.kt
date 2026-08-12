package com.live2dai.android.data

import java.util.Random
import kotlin.math.sin

// ============================================================
//  R13 角色表现力：程序化待机（IdleMotionController）
//
//  数值依据（.pi/spoq/search-report-expression.md §2.1，airi 眼神扫视 + Live2D 官方标准参数表）：
//  - 眼神扫视间隔 [400, 4400]ms；目标 X∈[-0.5,0.5] Y∈[-0.5,0.35]；lerp(cur,tgt,0.3) 阻尼落点
//  - 头部微动幅度 ≤15°（bai 钳制 ±30）；连续低频正弦，频率结构与"稀疏扫视"不同
//  - 身体晃动：更低频正弦（周期 > 头部），归一化幅度 [0.001, 0.3]
//
//  通道隔离（IM-07）：本控制器只输出眼神/头部/身体三通道，不碰眨眼（EyeLOpen/ROpen）
//  与呼吸（ParamBreath）——那两条分别归 EyeBlinkManager / BreathManager（既有通道）。
//
//  纯 JVM 类（无 native / android.* 依赖），可确定性单测：java.util.Random 固定种子 + 纯函数。
// ============================================================

/** 眼神阻尼（airi lerp(cur,tgt,0.3)） */
const val EYE_DAMPING: Float = 0.3f

/** 眼神扫视间隔下界（ms，airi EYE_SACCADE 400ms） */
const val SACCADE_MIN_MS: Float = 400f

/** 眼神扫视间隔上界（ms，airi EYE_SACCADE 4400ms） */
const val SACCADE_MAX_MS: Float = 4400f

/** 头部微动最大幅度（°）——airi focus 折算 + 官方 ±5-15 上界 */
const val HEAD_AMP_MAX_DEG: Float = 15f

/** bai 头部角度钳制（官方 ParamAngleX/Y ±30） */
const val BAI_ANGLE_CLAMP_DEG: Float = 30f

/** bai 眼球钳制（官方 ParamEyeBallX/Y ±1） */
const val BAI_EYEBALL_CLAMP: Float = 1f

/**
 * 程序化待机单帧输出样本（纯数据）。
 *
 * 字段不含眨眼/呼吸通道（IM-07：无 EyeLOpen / EYEOPEN / Breath）：
 * 眨眼归 EyeBlinkManager、呼吸归 BreathManager，本样本只承载眼神/头部/身体三通道。
 */
data class IdleSample(
    val eyeBallX: Float,
    val eyeBallY: Float,
    val headAngleX: Float,
    val headAngleY: Float,
    val bodySway: Float,
)

/**
 * 阻尼插值（airi lerp 语义）：current + (target - current) * damping。
 * 默认阻尼 = [EYE_DAMPING]（0.3）：dampedLerp(0,1)=0.3、dampedLerp(0.5,0)=0.35、dampedLerp(1,0)=0.7。
 */
fun dampedLerp(current: Float, target: Float, damping: Float = EYE_DAMPING): Float =
    current + (target - current) * damping

/**
 * 程序化待机配置。默认值 = search-report 数值（眼神 [400,4400]ms / ±0.5·0.35、
 * 头部 ±15°、身体低频正弦），可整体或逐项覆盖。
 */
data class IdleMotionConfig(
    val eyeDamping: Float = EYE_DAMPING,
    val saccadeMinMs: Int = 400,
    val saccadeMaxMs: Int = 4400,
    val eyeTargetXRange: ClosedFloatingPointRange<Float> = -0.5f..0.5f,
    val eyeTargetYRange: ClosedFloatingPointRange<Float> = -0.5f..0.35f,
    val headAmplitudeDeg: Float = HEAD_AMP_MAX_DEG,
    val headPeriodSeconds: Float = 3f,
    val bodyAmplitude: Float = 0.15f,
    val bodyPeriodSeconds: Float = 8f,
    val bodyPhaseOffsetRad: Float = 0.7853982f,
) {
    companion object {
        const val EYE_DAMPING: Float = 0.3f
        const val SACCADE_MIN_MS: Float = 400f
        const val SACCADE_MAX_MS: Float = 4400f
        const val HEAD_AMP_MAX_DEG: Float = 15f
        const val BAI_ANGLE_CLAMP_DEG: Float = 30f
        const val BAI_EYEBALL_CLAMP: Float = 1f
    }
}

/**
 * R13 程序化待机控制器（三通道纯状态类，JVM 可测，无 native / Android 依赖）：
 *
 * - **眼神漂移**：随机扫视（间隔 [400,4400]ms）+ [dampedLerp] 阻尼落点（0.3，非瞬切防抽搐）；
 *   扫视目标 X∈[-0.5,0.5] Y∈[-0.5,0.35]（airi focusTarget 折算）
 * - **头部微动**：连续低频正弦（幅度 ≤15°，bai 钳制 ±30；频率结构 ≠ 眼神稀疏扫视——逐帧连续 vs 稀疏跳变）
 * - **身体晃动**：更低频正弦（周期 > 头部，与头错相位），归一化幅度 [0.001, 0.3]
 *
 * 与既有 EyeBlinkManager / BreathManager 通道互不重叠（不碰 EyeLOpen/ROpen/ParamBreath）。
 * 时间推进与 AnimationPipeline.update 同一节奏（renderer 每 2 帧调用一次并累计 deltaTime 传入）。
 */
class IdleMotionController(
    val config: IdleMotionConfig = IdleMotionConfig(),
    val random: Random = Random(),
) {

    /** 便捷构造：仅固定随机源（测试确定性；T6 用 java.util.Random(seed)） */
    constructor(random: Random) : this(IdleMotionConfig(), random)

    companion object {
        const val EYE_DAMPING: Float = 0.3f
        const val SACCADE_MIN_MS: Float = 400f
        const val SACCADE_MAX_MS: Float = 4400f
        const val HEAD_AMP_MAX_DEG: Float = 15f
        const val BAI_ANGLE_CLAMP_DEG: Float = 30f
        const val BAI_EYEBALL_CLAMP: Float = 1f

        /** 2π（Float，头部/身体正弦角频率） */
        private const val TWO_PI: Float = 6.2831855f

        /** 头部 Y 周期 = X 周期 × 1.3（X/Y 错开更自然） */
        private const val HEAD_Y_PERIOD_RATIO: Float = 1.3f

        /** 头部 Y 幅度 = X 幅度 × 0.8（±12°，仍 ≤15°） */
        private const val HEAD_Y_AMPLITUDE_RATIO: Float = 0.8f

        /** 阻尼插值（与顶层 [dampedLerp] 同公式；兼容两种引用路径） */
        fun dampedLerp(current: Float, target: Float, damping: Float = EYE_DAMPING): Float =
            current + (target - current) * damping
    }

    // ── 眼神漂移状态 ──
    private var eyeCurrentX: Float = 0f
    private var eyeCurrentY: Float = 0f
    private var eyeTargetXVal: Float = 0f
    private var eyeTargetYVal: Float = 0f
    private var saccadeTimerMs: Float = 0f
    private var nextSaccadeIntervalMs: Float = 0f
    private var saccadeCountVal: Int = 0
    private val saccadeIntervalList: MutableList<Float> = mutableListOf()

    // ── 头部 / 身体状态 ──
    private var headTime: Float = 0f
    private var bodyTime: Float = 0f

    private var lastSample: IdleSample = IdleSample(0f, 0f, 0f, 0f, 0f)

    init {
        // t=0 初始扫视：角色从静止直接进入"有眼神"状态；此后按随机间隔 [400,4400]ms 扫视
        pickNewSaccadeTarget()
        scheduleNextSaccade()
    }

    /** 当前眼神目标 X（程序化层专属：情绪帧不含 EyeBall 键——眼神归本层，官方机制 §3.2） */
    val currentEyeTargetX: Float get() = eyeTargetXVal

    /** 当前眼神目标 Y */
    val currentEyeTargetY: Float get() = eyeTargetYVal

    /** 当前眼神目标 X（别名，与既有命名兼容） */
    val eyeTargetX: Float get() = eyeTargetXVal

    /** 当前眼神目标 Y（别名，与既有命名兼容） */
    val eyeTargetY: Float get() = eyeTargetYVal

    /** 已发生的扫视次数（含 t=0 初始扫视） */
    val saccadeCount: Int get() = saccadeCountVal

    /** 最近一次生成的扫视间隔（ms，∈ [saccadeMinMs, saccadeMaxMs)） */
    val lastSaccadeIntervalMs: Float get() = saccadeIntervalList.lastOrNull() ?: 0f

    /** 全部已生成扫视间隔（ms）快照 */
    val saccadeIntervals: List<Float> get() = saccadeIntervalList.toList()

    /** 最近一次 [update] 的输出样本 */
    val currentSample: IdleSample get() = lastSample

    /**
     * 每帧更新（deltaTime 秒），返回本帧输出样本。
     * 时间语义与 AnimationPipeline.update 一致（renderer 每 2 帧调用并累计 deltaTime 传入）。
     */
    fun update(deltaTime: Float): IdleSample {
        val dt = deltaTime.coerceAtLeast(0f)

        // ── 眼神漂移：随机扫视（间隔 [400,4400]ms）+ 阻尼落点（非瞬切）──
        saccadeTimerMs += dt * 1000f
        while (nextSaccadeIntervalMs > 0f && saccadeTimerMs >= nextSaccadeIntervalMs) {
            saccadeTimerMs -= nextSaccadeIntervalMs
            pickNewSaccadeTarget()
            scheduleNextSaccade()
        }
        eyeCurrentX = dampedLerp(eyeCurrentX, eyeTargetXVal, config.eyeDamping)
        eyeCurrentY = dampedLerp(eyeCurrentY, eyeTargetYVal, config.eyeDamping)

        // ── 头部微动：连续正弦（幅度 ≤15°，频率结构 ≠ 眼神稀疏扫视；X/Y 周期错开更自然）──
        headTime += dt
        val omegaX = TWO_PI / config.headPeriodSeconds
        val omegaY = TWO_PI / (config.headPeriodSeconds * HEAD_Y_PERIOD_RATIO)
        val headX = config.headAmplitudeDeg * sin(omegaX * headTime)
        val headY = config.headAmplitudeDeg * HEAD_Y_AMPLITUDE_RATIO * sin(omegaY * headTime)

        // ── 身体晃动：更低频正弦（周期 > 头部；与头错相位）──
        bodyTime += dt
        val bodyOmega = TWO_PI / config.bodyPeriodSeconds
        val bodySway = config.bodyAmplitude * sin(bodyOmega * bodyTime + config.bodyPhaseOffsetRad)

        lastSample = IdleSample(eyeCurrentX, eyeCurrentY, headX, headY, bodySway)
        return lastSample
    }

    private fun pickNewSaccadeTarget() {
        saccadeCountVal++
        val xRange = config.eyeTargetXRange
        val yRange = config.eyeTargetYRange
        eyeTargetXVal = xRange.start + random.nextFloat() * (xRange.endInclusive - xRange.start)
        eyeTargetYVal = yRange.start + random.nextFloat() * (yRange.endInclusive - yRange.start)
    }

    private fun scheduleNextSaccade() {
        val minMs = config.saccadeMinMs.coerceAtLeast(1)
        val maxMs = config.saccadeMaxMs.coerceAtLeast(minMs)
        val interval = random.nextFloat() * (maxMs - minMs).toFloat() + minMs.toFloat()
        saccadeIntervalList.add(interval)
        nextSaccadeIntervalMs = interval
    }
}
