package com.live2dai.android.data

import java.util.Random
import kotlin.math.sin

// ============================================================
//  R13 角色表现力：程序化待机（IdleMotionController）
//
//  数值依据（.pi/spoq/search-report-expression.md §2.1，airi 眼神扫视 + Live2D 官方标准参数表）：
//  - 眼神扫视间隔 [400, 4400]ms；目标 X∈[-0.5,0.5] Y∈[-0.5,0.35]；lerp(cur,tgt,0.3) 阻尼落点
//  - 头部微动幅度 ≤5°（bai 钳制 ±30）；间歇低频正弦——运动期/静止期交替（N.E.K.O. 预设动作间歇播放观感）
//  - 身体晃动：更低频正弦（周期 > 头部），归一化幅度 [0.001, 0.3]（默认 0.06）
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
 * 间歇轻晃窗口判定（纯函数）：自启动累计时间（ms）→ 当前是否处于"运动期"。
 *
 * 语义（N.E.K.O. 预设动作间歇播放观感）：
 * - `elapsedMs < initialRestSeconds*1000` → 静止（首次启动先静默，避免开屏就晃）；
 * - 之后以 `motionDurationSeconds + restDurationSeconds` 为周期循环，周期内前
 *   `motionDurationSeconds` 为运动期、其余为静止期；
 * - 退化：`motionDurationSeconds <= 0` → 恒静止；`restDurationSeconds <= 0` →
 *   跳过初始静默后恒运动。
 *
 * 与 [IdleMotionController.update] 内部同一判定（elapsedMs 由控制器逐帧累计 deltaTime），
 * 抽为纯函数便于直接单测（无状态、确定性）。
 */
fun inMotionWindow(elapsedMs: Float, config: IdleMotionConfig): Boolean {
    val elapsed = elapsedMs.coerceAtLeast(0f)
    val initialRestMs = config.initialRestSeconds.coerceAtLeast(0f) * 1000f
    if (elapsed < initialRestMs) return false
    val motionMs = config.motionDurationSeconds.coerceAtLeast(0f) * 1000f
    val restMs = config.restDurationSeconds.coerceAtLeast(0f) * 1000f
    val cycleMs = motionMs + restMs
    if (cycleMs <= 0f) return motionMs > 0f
    return (elapsed - initialRestMs) % cycleMs < motionMs
}

/**
 * 程序化待机配置。默认值 = 间歇轻晃调优数值（眼神 [400,4400]ms / ±0.5·0.35、
 * 头部 ±5° 周期 7s、身体 0.06 周期 9s；运动 5s / 静止 10s 交替，首次启动先静 3s），
 * 可整体或逐项覆盖。
 */
data class IdleMotionConfig(
    val eyeDamping: Float = EYE_DAMPING,
    val saccadeMinMs: Int = 400,
    val saccadeMaxMs: Int = 4400,
    val eyeTargetXRange: ClosedFloatingPointRange<Float> = -0.5f..0.5f,
    val eyeTargetYRange: ClosedFloatingPointRange<Float> = -0.5f..0.35f,
    val headAmplitudeDeg: Float = 5f,
    val headPeriodSeconds: Float = 7f,
    val bodyAmplitude: Float = 0.06f,
    val bodyPeriodSeconds: Float = 9f,
    val bodyPhaseOffsetRad: Float = 0.7853982f,
    /** 间歇轻晃：运动期时长（s）——静止 10s 中动 5s，N.E.K.O. 预设动作间歇播放观感 */
    val motionDurationSeconds: Float = 5f,
    /** 间歇轻晃：静止期时长（s）——期间头部/身体输出幅度 0（眼珠通道不受影响） */
    val restDurationSeconds: Float = 10f,
    /** 间歇轻晃：首次启动先静默时长（s）——避免开屏就晃（先静 2-3s 再进入首个运动期） */
    val initialRestSeconds: Float = 3f,
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
 * - **头部微动**：间歇低频正弦（幅度 ≤5°，bai 钳制 ±30；运动期/静止期交替——N.E.K.O. 预设动作间歇播放观感）
 * - **身体晃动**：更低频正弦（周期 > 头部，与头错相位），归一化幅度 0.06（默认；∈ [0.001, 0.3]）
 * - **间歇轻晃窗口**：运动 5s / 静止 10s 交替（[IdleMotionConfig.motionDurationSeconds] /
 *   [IdleMotionConfig.restDurationSeconds]）；静止期头部/身体输出 0（眼珠通道不受影响，
 *   眼神扫视照常）；首次启动先静默 [IdleMotionConfig.initialRestSeconds]=3s 再进入首个运动期
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

        /** 间歇轻晃窗口判定（与顶层 [inMotionWindow] 同公式；兼容两种引用路径） */
        fun inMotionWindow(elapsedMs: Float, config: IdleMotionConfig): Boolean {
            val elapsed = elapsedMs.coerceAtLeast(0f)
            val initialRestMs = config.initialRestSeconds.coerceAtLeast(0f) * 1000f
            if (elapsed < initialRestMs) return false
            val motionMs = config.motionDurationSeconds.coerceAtLeast(0f) * 1000f
            val restMs = config.restDurationSeconds.coerceAtLeast(0f) * 1000f
            val cycleMs = motionMs + restMs
            if (cycleMs <= 0f) return motionMs > 0f
            return (elapsed - initialRestMs) % cycleMs < motionMs
        }
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

    // ── 间歇轻晃窗口状态（运动期/静止期交替，N.E.K.O. 预设动作间歇播放观感）──
    /** 自启动累计时间（ms）：窗口判定时钟，静止期照常推进（与墙钟一致，确定性可测） */
    private var elapsedMs: Float = 0f

    /** 上一帧是否处于运动期：进入沿检测——运动期起始相位归零（每段自静止中性位起晃，无跳变） */
    private var wasInMotion: Boolean = false

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
        elapsedMs += dt * 1000f

        // ── 眼神漂移：随机扫视（间隔 [400,4400]ms）+ 阻尼落点（非瞬切）──
        //    间歇轻晃窗口不影响眼珠通道：静止期眼神扫视照常（任务要求"保持眼珠通道不受影响"）
        saccadeTimerMs += dt * 1000f
        while (nextSaccadeIntervalMs > 0f && saccadeTimerMs >= nextSaccadeIntervalMs) {
            saccadeTimerMs -= nextSaccadeIntervalMs
            pickNewSaccadeTarget()
            scheduleNextSaccade()
        }
        eyeCurrentX = dampedLerp(eyeCurrentX, eyeTargetXVal, config.eyeDamping)
        eyeCurrentY = dampedLerp(eyeCurrentY, eyeTargetYVal, config.eyeDamping)

        // ── 间歇轻晃窗口（运动期/静止期交替；判定纯函数 inMotionWindow）──
        //    运动期进入沿：头部/身体相位归零——静止期参数输出 0（已回落中性位），
        //    相位归零保证起晃瞬间无跳变（每段动作自中性起、止于中性，N.E.K.O. 观感）。
        //    静止期：相位冻结（headTime/bodyTime 不推进）、头部/身体输出幅度 0。
        val inMotion = inMotionWindow(elapsedMs, config)
        if (inMotion && !wasInMotion) {
            headTime = 0f
            bodyTime = 0f
        }
        wasInMotion = inMotion

        // ── 头部微动：间歇低频正弦（幅度 ≤5°、周期 7s；X/Y 周期错开更自然）──
        if (inMotion) headTime += dt
        val omegaX = TWO_PI / config.headPeriodSeconds
        val omegaY = TWO_PI / (config.headPeriodSeconds * HEAD_Y_PERIOD_RATIO)
        val headX = if (inMotion) config.headAmplitudeDeg * sin(omegaX * headTime) else 0f
        val headY = if (inMotion) config.headAmplitudeDeg * HEAD_Y_AMPLITUDE_RATIO * sin(omegaY * headTime) else 0f

        // ── 身体晃动：更低频正弦（周期 > 头部；与头错相位；静止期输出 0）──
        if (inMotion) bodyTime += dt
        val bodyOmega = TWO_PI / config.bodyPeriodSeconds
        val bodySway = if (inMotion) config.bodyAmplitude * sin(bodyOmega * bodyTime + config.bodyPhaseOffsetRad) else 0f

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
