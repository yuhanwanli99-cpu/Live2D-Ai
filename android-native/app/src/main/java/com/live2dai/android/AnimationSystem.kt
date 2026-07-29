package com.live2dai.android

import android.util.Log
import org.json.JSONArray
import org.json.JSONObject
import kotlin.math.cos
import kotlin.math.min
import kotlin.math.max
import kotlin.math.abs
import kotlin.random.Random

// ============================================================
//  曲线类型常量
// ============================================================

private const val CURVE_LINEAR = 0
private const val CURVE_BEZIER = 1
private const val CURVE_STEPPED = 2

// ============================================================
//  Motion 数据定义
// ============================================================

/**
 * motion3.json 中的单条曲线。
 * target 为 "Parameter" 时 id 是参数名，为 "Model" 时 id 是 "Opacity"。
 */
data class MotionCurve(
    val target: String,
    val id: String,
    val segments: FloatArray
) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is MotionCurve) return false
        return target == other.target && id == other.id && segments.contentEquals(other.segments)
    }

    override fun hashCode(): Int {
        var result = target.hashCode()
        result = 31 * result + id.hashCode()
        result = 31 * result + segments.contentHashCode()
        return result
    }
}

/**
 * 从 motion3.json 解析得到的完整动作数据。
 */
data class MotionData(
    val duration: Float,
    val fadeInTime: Float,
    val fadeOutTime: Float,
    val loop: Boolean,
    val curves: List<MotionCurve>
)

/**
 * 从 exp3.json 解析得到的表情参数。
 */
data class ExpressionParameter(
    val id: String,
    val value: Float,
    val blend: String
)

/**
 * 从 exp3.json 解析得到的完整表情数据。
 */
data class ExpressionData(
    val parameters: List<ExpressionParameter>
)

// ============================================================
//  Motion 优先级
// ============================================================

object MotionPriority {
    const val IDLE = 1
    const val NORMAL = 2
    const val FORCE = 3
}

// ============================================================
//  激活的 Motion 状态
// ============================================================

/**
 * 当前正在播放的 motion 状态。
 */
private class ActiveMotion(
    val data: MotionData,
    val priority: Int,
    /** 播放进度（秒） */
    var time: Float = 0f,
    /** fade-in 进度（秒） */
    var fadeTime: Float = 0f,
    /** 是否已完成 fade-in */
    var fadedIn: Boolean = false,
    /** 是否已经开始 fade-out */
    var fadingOut: Boolean = false,
    /** fade-out 进度（秒） */
    var fadeOutTime: Float = 0f
)

// ============================================================
//  MotionManager
// ============================================================

/**
 * Motion 播放管理器。
 *
 * 职责：
 * - 管理当前播放的 motion（含优先级控制）
 * - 每帧更新运动时间
 * - 评估曲线得到参数值
 * - 支持 fade-in/fade-out
 * - 支持循环播放
 */
class MotionManager(private val paramNameToIndex: Map<String, Int>) {

    private val tag = "MotionManager"

    /** 当前播放的 motion 栈（最多同时播放一个） */
    private var current: ActiveMotion? = null

    /** 等待播放的 motion（fade-out 当前后再播） */
    private var pending: ActiveMotion? = null

    // ============================================================
    //  公开 API
    // ============================================================

    /**
     * 开始播放 motion。
     *
     * @param motion 动作数据
     * @param priority 优先级 (1=IDLE, 2=NORMAL, 3=FORCE)
     * @return true 表示开始播放，false 表示被更高优先级阻止
     */
    fun startMotion(motion: MotionData, priority: Int): Boolean {
        val active = current
        if (active != null) {
            if (priority < active.priority) {
                // 被更高优先级阻止：如果已有 pending，保留更高的
                if (pending == null || priority > pending!!.priority) {
                    pending = ActiveMotion(motion, priority)
                }
                return false
            }
            if (priority == active.priority && priority != MotionPriority.FORCE) {
                // 同优先级不中断
                return false
            }
        }

        // 中断当前动作
        if (active != null && priority >= active.priority) {
            Log.i(tag, "Interrupting motion (new priority=$priority > old=${active.priority})")
        }

        current = ActiveMotion(
            data = motion,
            priority = priority,
            time = if (motion.loop) 0f else 0f,
            fadeTime = 0f,
            fadedIn = motion.fadeInTime <= 0f
        )
        pending = null
        return true
    }

    /**
     * 停止所有 motion。
     */
    fun stopAllMotions() {
        current = null
        pending = null
    }

    /**
     * 当前是否有 motion 正在播放。
     */
    fun isPlaying(): Boolean = current != null

    /**
     * 当前 motion 是否已播放完成（非循环动作）。
     */
    fun isFinished(): Boolean {
        val active = current ?: return true
        if (active.data.loop) return false
        return active.time >= active.data.duration && !active.fadingOut
    }

    /**
     * 每帧更新：推进时间，处理 fade-in/fade-out。
     *
     * @param deltaTime 帧时间差（秒）
     */
    fun update(deltaTime: Float) {
        val active = current ?: return

        // 计算 fade
        if (!active.fadedIn) {
            active.fadeTime += deltaTime
            if (active.fadeTime >= active.data.fadeInTime) {
                active.fadedIn = true
            }
        }

        // 更新播放时间
        if (active.data.loop) {
            // 循环：时间取模
            active.time = (active.time + deltaTime) % active.data.duration
        } else {
            active.time += deltaTime
            // 非循环：到达终点后开始 fade-out
            if (active.time >= active.data.duration && !active.fadingOut) {
                active.fadingOut = true
                active.fadeOutTime = 0f
            }
        }

        // 处理 fade-out
        if (active.fadingOut) {
            active.fadeOutTime += deltaTime
            if (active.fadeOutTime >= active.data.fadeOutTime) {
                // Fade-out 完成
                current = null

                // 检查待播 motion
                if (pending != null) {
                    val next = pending!!
                    current = ActiveMotion(
                        data = next.data,
                        priority = next.priority,
                        time = 0f,
                        fadeTime = 0f,
                        fadedIn = next.data.fadeInTime <= 0f
                    )
                    pending = null
                }
            }
        }
    }

    /**
     * 评估当前 motion 的曲线，将参数值写入 model。
     *
     * @param model 数据模型
     * @param defaults 参数默认值数组（用于复位被 motion 修改的参数）
     */
    fun evaluate(model: PurismModel, defaults: FloatArray?) {
        val active = current ?: return
        val motion = active.data

        for (curve in motion.curves) {
            when (curve.target) {
                "Parameter" -> {
                    val idx = paramNameToIndex[curve.id] ?: continue
                    val value = evaluateCurveAt(curve.segments, active.time)
                    if (value.isNaN()) continue

                    val minV = Live2DNative.getParameterMinValues(modelPtr(model))?.getOrElse(idx) { -1f } ?: -1f
                    val maxV = Live2DNative.getParameterMaxValues(modelPtr(model))?.getOrElse(idx) { 1f } ?: 1f
                    model.setParameterValue(idx, value.coerceIn(minV, maxV))
                }
                "Part" -> {
                    // Part 曲线：设置部件不透明度
                    val partIds = Live2DNative.getPartIds(modelPtr(model)) ?: continue
                    val partIdx = partIds.indexOfFirst { it == curve.id }
                    if (partIdx < 0) continue
                    val value = evaluateCurveAt(curve.segments, active.time)
                    if (value.isNaN()) continue
                    val partOpacities = Live2DNative.getPartOpacities(modelPtr(model)) ?: continue
                    if (partIdx < partOpacities.size) {
                        partOpacities[partIdx] = value.coerceIn(0f, 1f)
                    }
                }
                "Model" -> {
                    if (curve.id == "Opacity") {
                        // 当前简化实现：忽略 Model 透明度
                    }
                }
            }
        }
    }

    /**
     * 获取当前 fade 乘数（0~1）。
     * 取 fade-in 和 fade-out 的最小值。
     */
    private fun getFadeMultiplier(active: ActiveMotion): Float {
        var m = 1f
        if (!active.fadedIn && active.data.fadeInTime > 0f) {
            m = min(m, active.fadeTime / active.data.fadeInTime)
        }
        if (active.fadingOut && active.data.fadeOutTime > 0f) {
            m = min(m, 1f - active.fadeOutTime / active.data.fadeOutTime)
        }
        return m.coerceIn(0f, 1f)
    }

    companion object {
        /**
         * 获取 PurismModel 的 modelPtr（通过反射方式访问私有字段）。
         * 由于 PurismModel 的 modelPtr 是私有字段，这里通过 JNI 直接获取。
         */
        private fun modelPtr(model: PurismModel): Long = model.modelPtr

        /**
         * 在曲线 segments 中求值在时间 t 的参数值。
         * 与 Live2DRenderer.evaluateCurve 逻辑一致。
         */
        fun evaluateCurveAt(segments: FloatArray, time: Float): Float {
            if (segments.size < 2) return Float.NaN

            var prevT = segments[0]
            var prevV = segments[1]
            var i = 2

            while (i < segments.size) {
                val type = segments[i].toInt()
                i++

                when (type) {
                    CURVE_LINEAR -> {
                        if (i + 1 >= segments.size) break
                        val t1 = segments[i]
                        val v1 = segments[i + 1]
                        i += 2
                        if (time in min(prevT, t1)..max(prevT, t1)) {
                            val ratio = if (t1 != prevT) (time - prevT) / (t1 - prevT) else 0.0f
                            return prevV + (v1 - prevV) * ratio
                        }
                        prevT = t1
                        prevV = v1
                    }
                    CURVE_BEZIER -> {
                        // Bezier 段简化处理：用线性近似
                        if (i + 5 >= segments.size) break
                        val cx1 = segments[i]
                        val cy1 = segments[i + 1]
                        val cx2 = segments[i + 2]
                        val cy2 = segments[i + 3]
                        val t1 = segments[i + 4]
                        val v1 = segments[i + 5]
                        i += 6
                        if (time in min(prevT, t1)..max(prevT, t1)) {
                            val ratio = if (t1 != prevT) (time - prevT) / (t1 - prevT) else 0.0f
                            return prevV + (v1 - prevV) * ratio
                        }
                        prevT = t1
                        prevV = v1
                    }
                    CURVE_STEPPED -> {
                        if (i >= segments.size) break
                        val t1 = segments[i]
                        i++
                        if (time < t1) return prevV
                        prevT = t1
                    }
                }
            }
            return prevV
        }
    }
}

// ============================================================
//  EyeBlinkManager
// ============================================================

/**
 * 自动眨眼管理器。
 *
 * 模拟自然眨眼：
 * - 随机间隔（1.5~6 秒）
 * - 眨眼持续约 0.1~0.15 秒
 * - 参数平滑变化
 */
class EyeBlinkManager {

    private val tag = "EyeBlink"

    /** 下一次眨眼等待时间（秒） */
    private var nextBlinkTime: Float = 0f

    /** 眨眼计时器（秒） */
    private var blinkTimer: Float = 0f

    /** 眨眼状态：0=睁开, 0~1=闭合中, 1~0=睁开中 */
    private var blinkPhase: Float = 0f

    /** 眨眼闭合耗时（秒） */
    private val closingTime = 0.06f

    /** 眨眼睁开耗时（秒） */
    private val openingTime = 0.08f

    /** 完全闭合保持时间（秒） */
    private val closedHoldTime = 0.05f

    /** 最小间隔（秒） */
    private val minInterval = 1.5f

    /** 最大间隔（秒） */
    private val maxInterval = 6.0f

    /** 是否在眨眼过程中 */
    private var isBlinking = false

    /** 闭合后计时 */
    private var closedTimer: Float = 0f

    init {
        scheduleNextBlink()
    }

    /**
     * 每帧更新眨眼状态。
     *
     * @param deltaTime 帧时间差（秒）
     */
    fun update(deltaTime: Float) {
        if (isBlinking) {
            blinkTimer += deltaTime

            if (blinkPhase < 1f) {
                // 闭合阶段
                blinkPhase = (blinkTimer / closingTime).coerceIn(0f, 1f)
                if (blinkPhase >= 1f) {
                    // 完全闭合，开始保持
                    closedTimer = 0f
                }
            } else {
                // 保持闭合后，开始睁开
                closedTimer += deltaTime
                if (closedTimer >= closedHoldTime) {
                    val openElapsed = blinkTimer - closingTime - closedHoldTime
                    blinkPhase = 1f - (openElapsed / openingTime).coerceIn(0f, 1f)
                    if (blinkPhase <= 0f) {
                        // 眨眼结束
                        isBlinking = false
                        blinkPhase = 0f
                        blinkTimer = 0f
                        scheduleNextBlink()
                    }
                }
            }
        } else {
            blinkTimer += deltaTime
            if (blinkTimer >= nextBlinkTime) {
                // 开始眨眼
                isBlinking = true
                blinkTimer = 0f
                blinkPhase = 0f
            }
        }
    }

    /**
     * 获取左眼睁开度。
     * 1.0 = 完全睁开，0.0 = 完全闭合。
     */
    fun getLeftEyeOpen(): Float {
        return 1f - blinkPhase
    }

    /**
     * 获取右眼睁开度。
     * 1.0 = 完全睁开，0.0 = 完全闭合。
     */
    fun getRightEyeOpen(): Float {
        return 1f - blinkPhase
    }

    /**
     * 强制眨眼一次。
     */
    fun forceBlink() {
        if (!isBlinking) {
            isBlinking = true
            blinkTimer = 0f
            blinkPhase = 0f
        }
    }

    private fun scheduleNextBlink() {
        nextBlinkTime = Random.nextFloat() * (maxInterval - minInterval) + minInterval
    }
}

// ============================================================
//  BreathManager
// ============================================================

/**
 * 自动呼吸管理器。
 *
 * 使用正弦波在多个参数上叠加呼吸效果。
 * 参数频率/幅度来自参考实现 LAppModel 的常量。
 */
class BreathManager {

    private val tag = "Breath"

    /** 累计时间（秒） */
    private var totalTime: Float = 0f

    /** 呼吸参数定义 */
    private data class BreathParam(
        val id: String,
        val offset: Float,
        val peak: Float,
        val cycle: Float,
        val weight: Float
    )

    private val breathParams = listOf(
        BreathParam("ParamAngleX", 0.0f, 15.0f, 6.5345f, 0.5f),
        BreathParam("ParamAngleY", 0.0f, 8.0f, 3.5345f, 0.5f),
        BreathParam("ParamAngleZ", 0.0f, 10.0f, 5.5345f, 0.5f),
        BreathParam("ParamBodyAngleX", 0.0f, 4.0f, 15.5345f, 0.5f),
        BreathParam("ParamBreath", 0.5f, 0.5f, 3.2345f, 0.5f)
    )

    /**
     * 每帧更新呼吸状态。
     * 需要外部在 motion 更新后、model.update 前调用 apply。
     *
     * @param deltaTime 帧时间差（秒）
     * @param paramNameToIndex 参数名到索引的映射
     * @param model 数据模型
     */
    fun update(deltaTime: Float) {
        totalTime += deltaTime
    }

    /**
     * 将呼吸效果应用到模型参数。
     */
    fun apply(model: PurismModel, paramNameToIndex: Map<String, Int>) {
        for (bp in breathParams) {
            val idx = paramNameToIndex[bp.id] ?: continue
            val cycle = if (bp.cycle > 0f) bp.cycle else 1f

            // 正弦波呼吸：offset + peak * sin(2pi * time / cycle)
            val value = bp.offset + bp.peak * cos(totalTime * (2f * Math.PI.toFloat() / cycle))
            val weighted = value * bp.weight

            val current = Live2DNative.getParameterValues(modelPtr(model))?.getOrNull(idx) ?: continue
            val newValue = current + weighted

            val minV = Live2DNative.getParameterMinValues(modelPtr(model))?.getOrElse(idx) { -1f } ?: -1f
            val maxV = Live2DNative.getParameterMaxValues(modelPtr(model))?.getOrElse(idx) { 1f } ?: 1f
            model.setParameterValue(idx, newValue.coerceIn(minV, maxV))
        }
    }

    companion object {
        private fun modelPtr(model: PurismModel): Long = model.modelPtr
    }
}

// ============================================================
//  PhysicsManager
// ============================================================

/**
 * 物理模拟管理器。
 *
 * 解析 physics3.json 并应用物理效果。
 * 简化实现：仅支持最基本的物理参数叠加。
 */
class PhysicsManager {

    private val tag = "Physics"

    /** 是否已初始化 */
    private var initialized = false

    /** 物理参数设置列表 */
    private val settings = mutableListOf<PhysicsSetting>()

    /** 物理设置 */
    private data class PhysicsSetting(
        val id: String,
        val type: String,       // "Parameter" or "PartOpacity"
        val factor: Float,
        val inputId: String,    // 输入参数 ID
        val outputId: String    // 输出参数 ID
    )

    /**
     * 从 physics3.json 字节数据初始化。
     */
    fun initialize(jsonBytes: ByteArray) {
        try {
            val json = String(jsonBytes)
            val root = JSONObject(json)
            val meta = root.optJSONObject("Meta") ?: return
            val physicsSettings = root.optJSONArray("PhysicsSettings") ?: return

            settings.clear()
            for (i in 0 until physicsSettings.length()) {
                val ps = physicsSettings.getJSONObject(i)
                val id = ps.optString("Id", "")
                val input = ps.optJSONObject("Input") ?: continue
                val output = ps.optJSONObject("Output") ?: continue

                // 只支持最基本的 Weight 和 Type 属性
                val type = output.optString("Type", "Parameter")
                val inputId = input.optString("Source", "")
                val outputId = output.optString("Destination", "")
                val factor = output.optDouble("Scale", 1.0).toFloat()

                if (inputId.isNotEmpty() && outputId.isNotEmpty()) {
                    settings.add(PhysicsSetting(id, type, factor, inputId, outputId))
                }
            }

            initialized = settings.isNotEmpty()
            Log.i(tag, "Physics initialized with ${settings.size} settings")
        } catch (e: Exception) {
            Log.w(tag, "Failed to initialize physics: ${e.message}")
        }
    }

    /**
     * 是否已成功初始化。
     */
    fun isValid(): Boolean = initialized

    /**
     * 每帧应用物理效果。
     */
    fun evaluate(model: PurismModel, paramNameToIndex: Map<String, Int>, deltaTime: Float) {
        if (!initialized) return

        val paramValues = Live2DNative.getParameterValues(modelPtr(model)) ?: return

        for (setting in settings) {
            val inputIdx = paramNameToIndex[setting.inputId] ?: continue
            val outputIdx = paramNameToIndex[setting.outputId] ?: continue

            if (inputIdx >= paramValues.size || outputIdx >= paramValues.size) continue

            val inputValue = paramValues[inputIdx]
            // 简化的物理：输出 = 输入 * 因子
            val outputValue = inputValue * setting.factor

            val currentValue = paramValues.getOrNull(outputIdx) ?: continue
            val newValue = currentValue + outputValue

            val minV = Live2DNative.getParameterMinValues(modelPtr(model))?.getOrElse(outputIdx) { -1f } ?: -1f
            val maxV = Live2DNative.getParameterMaxValues(modelPtr(model))?.getOrElse(outputIdx) { 1f } ?: 1f
            model.setParameterValue(outputIdx, newValue.coerceIn(minV, maxV))
        }
    }

    companion object {
        private fun modelPtr(model: PurismModel): Long = model.modelPtr
    }
}

// ============================================================
//  PoseManager
// ============================================================

/**
 * Pose 管理器。
 *
 * 解析 pose3.json 并应用部件不透明度混合。
 * pose3.json 定义了一组部件组，组内部件的透明度变化会互相影响。
 */
class PoseManager {

    private val tag = "Pose"

    /** 是否已初始化 */
    private var initialized = false

    /** Fade 时间（秒） */
    private var fadeTime: Float = 0.5f

    /** 部件组列表：每组包含一组部件索引和它们的目标不透明度 */
    private val partGroups = mutableListOf<PartGroup>()

    private data class PartGroup(
        val partIds: List<String>,
        /** 当前活动部件的索引（在 partIds 中） */
        var activeIndex: Int = 0,
        /** 上一帧的活动索引（用于淡入淡出） */
        var previousIndex: Int = -1,
        /** 淡出计时器 */
        var fadeProgress: Float = 1f
    )

    /**
     * 从 pose3.json 字节数据初始化。
     */
    fun initialize(jsonBytes: ByteArray, partIds: Array<String>?) {
        try {
            val json = String(jsonBytes)
            val root = JSONObject(json)
            val groupsArray = root.optJSONArray("Groups")
            if (groupsArray == null) {
                Log.w(tag, "No Groups field in pose JSON")
                return
            }
            val idList = partIds?.toList() ?: emptyList()

            partGroups.clear()
            for (i in 0 until groupsArray.length()) {
                val group = groupsArray.getJSONArray(i)

                val partIdList = mutableListOf<String>()
                for (j in 0 until group.length()) {
                    val part = group.getJSONObject(j)
                    val id = part.getString("Id")
                    if (id in idList) {
                        partIdList.add(id)
                    }
                }

                if (partIdList.isNotEmpty()) {
                    partGroups.add(PartGroup(partIds = partIdList))
                }
            }

            initialized = partGroups.isNotEmpty()
            Log.i(tag, "Pose initialized with ${partGroups.size} groups")
        } catch (e: Exception) {
            Log.w(tag, "Failed to initialize pose: ${e.message}")
        }
    }

    /**
     * 是否已成功初始化。
     */
    fun isValid(): Boolean = initialized

    /**
     * 每帧更新部件不透明度。
     *
     * @param model 数据模型
     * @param partNameToIndex 部件名到索引的映射
     * @param deltaTime 帧时间差（秒）
     */
    fun updateParameters(model: PurismModel, partNameToIndex: Map<String, Int>, deltaTime: Float) {
        if (!initialized) return

        for (group in partGroups) {
            if (group.partIds.isEmpty()) continue

            // 组内第一个 part 设为可见（通过 setPartOpacity 写 native），其余隐藏
            val firstPartIdx = partNameToIndex[group.partIds[0]] ?: continue
            model.setPartOpacity(firstPartIdx, 1.0f)
            for (j in 1 until group.partIds.size) {
                val otherPartIdx = partNameToIndex[group.partIds[j]] ?: continue
                model.setPartOpacity(otherPartIdx, 0.0f)
            }
        }
    }
}

// ============================================================
//  AnimationPipeline — 整合所有动画组件
// ============================================================

/**
 * 完整的动画管线。
 *
 * 整合 MotionManager、EyeBlinkManager、BreathManager、
 * PhysicsManager、PoseManager 的统一入口。
 *
 * 每帧调用顺序（对应 Cubism SDK 的 update 流程）：
 * 1. loadParameters — 恢复参数快照
 * 2. updateMotion — 更新并评估 motion 曲线
 * 3. updateEyeBlink — 更新并应用眨眼
 * 4. updateExpression — 更新并应用表情
 * 5. updateBreath — 更新并应用呼吸
 * 6. updatePhysics — 更新并应用物理
 * 7. updatePose — 更新并应用 pose
 * 8. model.update — 计算新顶点
 */
class AnimationPipeline(
    private val model: PurismModel,
    private val paramNameToIndex: Map<String, Int>,
    private val partNameToIndex: Map<String, Int> = emptyMap()
) {
    private val tag = "AnimationPipeline"

    /** Motion 管理器 */
    val motionManager: MotionManager = MotionManager(paramNameToIndex)

    /** 眨眼管理器 */
    val eyeBlink: EyeBlinkManager = EyeBlinkManager()

    /** 呼吸管理器 */
    val breath: BreathManager = BreathManager()

    /** 物理管理器 */
    val physics: PhysicsManager = PhysicsManager()

    /** Pose 管理器 */
    val pose: PoseManager = PoseManager()

    /** 表情管理器 */
    val expressionManager: ExpressionManager = ExpressionManager(paramNameToIndex)

    /** 是否启用自动眨眼 */
    var autoBlinkEnabled: Boolean = true

    /** 是否启用自动呼吸 */
    var autoBreathEnabled: Boolean = true

    /** 是否已保存初始快照 */
    private var hasInitialSnapshot: Boolean = false

    /** 预设眨眼参数 ID */
    private var eyeLOpenIndex: Int = paramNameToIndex["PARAM_EYE_L_OPEN"] ?: -1
    private var eyeROpenIndex: Int = paramNameToIndex["PARAM_EYE_R_OPEN"] ?: -1
    private var eyeLOpenIndexAlt: Int = paramNameToIndex["ParamEyeLOpen"] ?: -1
    private var eyeROpenIndexAlt: Int = paramNameToIndex["ParamEyeROpen"] ?: -1

    /** 预设口型同步参数 ID */
    private var paramAIndex: Int = paramNameToIndex["ParamA"] ?: -1
    private var paramMouthOpenY: Int = paramNameToIndex["ParamMouthOpenY"] ?: -1

    // ============================================================
    //  表情管理
    // ============================================================

    /**
     * 表情管理 — 在表情叠加层管理当前表情。
     */
    inner class ExpressionManager(private val paramNameToIndex: Map<String, Int>) {

        /** 当前播放的表情数据 */
        var currentExpression: ExpressionData? = null

        /** 表情权重（0~1，用于淡入淡出） */
        var expressionWeight: Float = 0f

        /** 表情淡入时间（秒） */
        var expressionFadeTime: Float = 0.5f

        /** 表情淡入计时器 */
        private var expressionTimer: Float = 0f

        /**
         * 设置表情。
         */
        fun setExpression(expression: ExpressionData) {
            currentExpression = expression
            expressionTimer = 0f
            expressionWeight = 0f
        }

        /**
         * 清除表情。
         */
        fun clearExpression() {
            currentExpression = null
            expressionWeight = 0f
            expressionTimer = 0f
        }

        /**
         * 每帧更新表情。
         */
        fun update(deltaTime: Float) {
            val exp = currentExpression ?: return

            expressionTimer += deltaTime
            expressionWeight = (expressionTimer / expressionFadeTime).coerceIn(0f, 1f)
        }

        /**
         * 将表情应用到参数。
         */
        fun apply(model: PurismModel) {
            val exp = currentExpression ?: return
            if (expressionWeight <= 0f) return

            val defaults = Live2DNative.getParameterDefaultValues(modelPtr(model)) ?: return
            val paramValues = Live2DNative.getParameterValues(modelPtr(model)) ?: return
            val minValues = Live2DNative.getParameterMinValues(modelPtr(model))
            val maxValues = Live2DNative.getParameterMaxValues(modelPtr(model))

            for (ep in exp.parameters) {
                val idx = paramNameToIndex[ep.id] ?: continue
                if (idx >= paramValues.size) continue

                val current = paramValues[idx]
                val defaultValue = defaults.getOrElse(idx) { 0f }
                val minV = minValues?.getOrElse(idx) { -1f } ?: -1f
                val maxV = maxValues?.getOrElse(idx) { 1f } ?: 1f

                val newValue = when (ep.blend) {
                    "Add" -> current + ep.value * expressionWeight
                    "Multiply" -> current * (1f + ep.value * expressionWeight)
                    "Overwrite" -> defaultValue + (ep.value - defaultValue) * expressionWeight
                    else -> current + ep.value * expressionWeight
                }

                model.setParameterValue(idx, newValue.coerceIn(minV, maxV))
            }
        }

    }

    // ============================================================
    //  初始化
    // ============================================================

    /**
     * 创建初始参数快照。
     * 在第一次 loadParameters 前调用。
     */
    fun createInitialSnapshot() {
        model.loadParameters()
        hasInitialSnapshot = true
    }

    // ============================================================
    //  主更新循环
    // ============================================================

    /**
     * 每帧调用 — 执行完整的动画更新管线。
     *
     * @param deltaTime 帧间隔（秒）
     */
    fun update(deltaTime: Float) {
        // 1. 恢复参数快照到帧开始时的状态
        model.loadParameters()

        // 2. 更新 motion 时间
        motionManager.update(deltaTime)

        // 3. 应用 motion 曲线到参数
        val defaults = Live2DNative.getParameterDefaultValues(modelPtr(model))
        motionManager.evaluate(model, defaults)

        // 4. 更新表情
        expressionManager.update(deltaTime)
        expressionManager.apply(model)

        // 5. 更新眨眼
        eyeBlink.update(deltaTime)
        applyEyeBlink()

        // 6. 更新呼吸
        if (autoBreathEnabled) {
            breath.update(deltaTime)
            breath.apply(model, paramNameToIndex)
        }

        // 7. 更新物理
        if (physics.isValid()) {
            physics.evaluate(model, paramNameToIndex, deltaTime)
        }

        // 8. 保存参数快照（在 model.update 之前）
        model.saveParameters()

        // 9. model.update — 计算新顶点
        model.update()

        // 10. 更新 Pose — 必须在 model.update 之后！
        //     Cubium SDK 中，Pose 在 Update() 后执行，修改 part opacity
        if (pose.isValid()) {
            pose.updateParameters(model, partNameToIndex, deltaTime)
            // Pose 修改了 part opacity → 需要再跑一次 update 让 renderer 取到正确数据
            model.update()
        }
    }

    /**
     * 应用眨眼到眼睛参数。
     */
    private fun applyEyeBlink() {
        if (!autoBlinkEnabled) return

        val leftOpen = eyeBlink.getLeftEyeOpen()
        val rightOpen = eyeBlink.getRightEyeOpen()

        // 尝试标准眼参数名
        if (eyeLOpenIndex >= 0) {
            model.setParameterValue(eyeLOpenIndex, leftOpen)
        } else if (eyeLOpenIndexAlt >= 0) {
            model.setParameterValue(eyeLOpenIndexAlt, leftOpen)
        }

        if (eyeROpenIndex >= 0) {
            model.setParameterValue(eyeROpenIndex, rightOpen)
        } else if (eyeROpenIndexAlt >= 0) {
            model.setParameterValue(eyeROpenIndexAlt, rightOpen)
        }
    }

    /**
     * 设置口型同步值。
     */
    fun setLipSyncValue(value: Float) {
        val clamped = value.coerceIn(0f, 1f)
        if (paramAIndex >= 0) {
            model.setParameterValue(paramAIndex, clamped)
        }
        if (paramMouthOpenY >= 0) {
            model.setParameterValue(paramMouthOpenY, clamped)
        }
    }

    companion object {
        private fun modelPtr(model: PurismModel): Long = model.modelPtr
    }
}

// ============================================================
//  Motion 加载器
// ============================================================

/**
 * 从 model3.json 加载所有 motion 和 expression。
 */
class MotionLoader {

    companion object {
        private const val TAG = "MotionLoader"

        /**
         * 从 model3.json 的 FileReferences 加载所有 motion。
         *
         * @param settingJson model3.json 的 JSONObject
         * @param modelDir 模型目录（用于拼接完整路径）
         * @param loadFile 文件加载函数（接收相对路径，返回字节数组）
         * @return 按"(group)_(index)" 键值存储的 MotionData 映射
         */
        fun loadAllMotions(
            settingJson: JSONObject,
            modelDir: String,
            loadFile: (String) -> ByteArray?
        ): Map<String, MotionData> {
            val motions = mutableMapOf<String, MotionData>()

            try {
                val fileRefs = settingJson.getJSONObject("FileReferences")
                val allMotions = fileRefs.optJSONObject("Motions") ?: return motions

                val groups = allMotions.keys()
                while (groups.hasNext()) {
                    val group = groups.next()
                    val motionArray = allMotions.getJSONArray(group)

                    for (i in 0 until motionArray.length()) {
                        val motionObj = motionArray.getJSONObject(i)
                        val motionFile = motionObj.optString("File", "") ?: continue
                        if (motionFile.isEmpty()) continue

                        val path = "$modelDir/$motionFile"
                        val bytes = loadFile(path)
                        if (bytes == null) {
                            Log.w(TAG, "Failed to load motion: $path")
                            continue
                        }

                        val motionData = parseMotionJson(String(bytes), motionObj)
                        if (motionData != null) {
                            val key = "${group}_$i"
                            motions[key] = motionData
                            Log.i(TAG, "Loaded motion: $key (duration=${motionData.duration}s, loop=${motionData.loop}, ${motionData.curves.size} curves)")
                        }
                    }
                }
            } catch (e: Exception) {
                Log.w(TAG, "Failed to load motions: ${e.message}")
            }

            return motions
        }

        /**
         * 从 model3.json 的 FileReferences 加载所有表情。
         */
        fun loadAllExpressions(
            settingJson: JSONObject,
            modelDir: String,
            loadFile: (String) -> ByteArray?
        ): Map<String, ExpressionData> {
            val expressions = mutableMapOf<String, ExpressionData>()

            try {
                val fileRefs = settingJson.getJSONObject("FileReferences")
                val expArray = fileRefs.optJSONArray("Expressions") ?: return expressions

                for (i in 0 until expArray.length()) {
                    val expObj = expArray.getJSONObject(i)
                    val expFile = expObj.optString("File", "") ?: continue
                    val expName = expObj.optString("Name", "exp_$i") ?: "exp_$i"
                    if (expFile.isEmpty()) continue

                    val path = "$modelDir/$expFile"
                    val bytes = loadFile(path)
                    if (bytes == null) {
                        Log.w(TAG, "Failed to load expression: $path")
                        continue
                    }

                    val expressionData = parseExpressionJson(String(bytes))
                    if (expressionData != null) {
                        expressions[expName] = expressionData
                        Log.i(TAG, "Loaded expression: $expName (${expressionData.parameters.size} params)")
                    }
                }
            } catch (e: Exception) {
                Log.w(TAG, "Failed to load expressions: ${e.message}")
            }

            return expressions
        }

        /**
         * 解析 motion3.json 到 MotionData。
         */
        fun parseMotionJson(json: String, motionObj: JSONObject? = null): MotionData? {
            return try {
                val root = JSONObject(json)
                val meta = root.getJSONObject("Meta")

                val duration = meta.optDouble("Duration", 1.0).toFloat()
                val fadeInTime = meta.optDouble("FadeInTime", -1.0).toFloat()
                val fadeOutTime = meta.optDouble("FadeOutTime", -1.0).toFloat()
                val loop = meta.optBoolean("Loop", false)

                // 优先使用 model3.json 中的 fade 设置
                val finalFadeIn = if (motionObj != null && motionObj.has("FadeInTime")) {
                    motionObj.getDouble("FadeInTime").toFloat()
                } else if (fadeInTime >= 0f) {
                    fadeInTime
                } else {
                    0.5f // 默认 fade-in 0.5 秒
                }

                val finalFadeOut = if (motionObj != null && motionObj.has("FadeOutTime")) {
                    motionObj.getDouble("FadeOutTime").toFloat()
                } else if (fadeOutTime >= 0f) {
                    fadeOutTime
                } else {
                    0.5f // 默认 fade-out 0.5 秒
                }

                val curves = root.optJSONArray("Curves") ?: root.optJSONArray("Parameters")
                val curveList = mutableListOf<MotionCurve>()

                if (curves != null) {
                    for (ci in 0 until curves.length()) {
                        val curve = curves.getJSONObject(ci)
                        val target = curve.optString("Target", "Parameter")
                        val id = curve.getString("Id")
                        val segments = curve.getJSONArray("Segments")

                        val segArray = FloatArray(segments.length())
                        for (si in 0 until segments.length()) {
                            segArray[si] = segments.getDouble(si).toFloat()
                        }

                        curveList.add(MotionCurve(target, id, segArray))
                    }
                }

                MotionData(
                    duration = duration,
                    fadeInTime = finalFadeIn,
                    fadeOutTime = finalFadeOut,
                    loop = loop,
                    curves = curveList
                )
            } catch (e: Exception) {
                Log.w(TAG, "Failed to parse motion JSON: ${e.message}")
                null
            }
        }

        /**
         * 解析 exp3.json 到 ExpressionData。
         */
        fun parseExpressionJson(json: String): ExpressionData? {
            return try {
                val root = JSONObject(json)
                val params = root.getJSONArray("Parameters")

                val paramList = mutableListOf<ExpressionParameter>()
                for (i in 0 until params.length()) {
                    val p = params.getJSONObject(i)
                    val id = p.getString("Id")
                    val value = p.getDouble("Value").toFloat()
                    val blend = p.optString("Blend", "Add")
                    paramList.add(ExpressionParameter(id, value, blend))
                }

                ExpressionData(parameters = paramList)
            } catch (e: Exception) {
                Log.w(TAG, "Failed to parse expression JSON: ${e.message}")
                null
            }
        }
    }
}
