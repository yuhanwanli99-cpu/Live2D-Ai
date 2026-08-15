package com.live2dai.android

import android.util.Log
import com.live2dai.android.data.BodyPoseFrames
import com.live2dai.android.data.IdleMotionConfig
import com.live2dai.android.data.IdleMotionController
import com.live2dai.android.data.IdleSample
import com.live2dai.android.data.LiaisonArbiter
import com.live2dai.android.data.SpeakingMotion
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

/** C7d：LipSync 口型让位目标参数——表情层与 LipSync 双写者冲突根因（TTS 播放期间表情层不写此参数，口型归 LipSync 独占） */
private const val MOUTH_OPEN_Y_ID = "ParamMouthOpenY"

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
     * 当前播放 motion 的优先级（无播放 → null）。
     * 只读 getter（contract-M7-android-l3.md §1.2）：供 idle 轮换 / 触发回落判断使用，
     * 不改动 MotionManager 纯 motion 栈引擎的任何构造/签名/行为。
     */
    val currentPriority: Int?
        get() = current?.priority

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

    /** 呼吸参数定义（硬编码默认值，可由 meta 覆盖） */
    private data class BreathParam(
        val id: String,
        val index: Int = -1,
        val offset: Float,
        val peak: Float,
        val cycle: Float,
        val weight: Float
    ) {
        /** 此参数是否可通过索引在模型中定位 */
        fun isValid(): Boolean = index >= 0
    }

    /** 默认呼吸参数（用于模型无可用的 cdi3.json 时） */
    // spoq-fix：呼吸只驱动胸部（ParamBreath）——移除 ParamAngleX/Y/Z（头部旋转）与
    // ParamBodyAngleX：它们在 bai 等模型上表现为头部正弦摆动（用户实锤异常）。
    private val defaultBreathParams = listOf(
        BreathParam("ParamBreath", offset = 0.5f, peak = 0.5f, cycle = 3.2345f, weight = 0.5f)
    )

    /** 当前使用的呼吸参数（默认值，可由 initialize(meta) 覆盖） */
    private var breathParams: List<BreathParam> = defaultBreathParams

    /**
     * 是否已通过 meta 初始化（设为 true 后，参数索引直接从 meta 获取，不再按名称查找）。
     */
    private var metaInitialized = false

    /**
     * 用模型元配置初始化呼吸参数。
     *
     * 替换默认的硬编码呼吸参数列表，使用从 cdi3.json 自动发现的参数。
     * 每个参数的 offset/peak/cycle/weight 仍使用 Cubism SDK 参考实现的默认值，
     * 但参数索引从 meta 获取（确保不遗漏模型中实际存在的参数）。
     *
     * @param meta 模型元配置
     */
    fun initialize(meta: ModelMetaConfig) {
        if (meta.semantic.breathParams.isEmpty()) {
            Log.i(tag, "No breath params in meta — keeping defaults")
            return
        }

        // 将 meta 的 BreathParamDef 转换为内部 BreathParam
        val resolved = meta.semantic.breathParams.map { def ->
            BreathParam(
                id = def.id,
                index = def.index,
                offset = def.offset,
                peak = def.peak,
                cycle = def.cycle,
                weight = def.weight
            )
        }

        breathParams = resolved
        metaInitialized = true

        Log.i(tag, "BreathManager initialized from meta: ${resolved.size} params")
        for (bp in resolved) {
            Log.v(tag, "  BreathParam: ${bp.id}(idx=${bp.index}) " +
                    "offset=${bp.offset} peak=${bp.peak} cycle=${bp.cycle} weight=${bp.weight}")
        }
    }

    /**
     * 每帧更新呼吸状态。
     * 需要外部在 motion 更新后、model.update 前调用 apply。
     *
     * @param deltaTime 帧时间差（秒）
     */
    fun update(deltaTime: Float) {
        totalTime += deltaTime
    }

    /**
     * 将呼吸效果应用到模型参数。
     *
     * 如果已通过 meta 初始化，优先使用 meta 提供的参数索引（比按名称查找更快更可靠）。
     * 否则使用默认的硬编码参数列表，按名称查找索引。
     */
    fun apply(model: PurismModel, paramNameToIndex: Map<String, Int>) {
        for (bp in breathParams) {
            val idx: Int
            if (metaInitialized) {
                if (!bp.isValid()) continue
                idx = bp.index
            } else {
                idx = paramNameToIndex[bp.id] ?: continue
            }

            val cycle = if (bp.cycle > 0f) bp.cycle else 1f

            // 正弦波呼吸：offset + peak * cos(2pi * time / cycle)
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

    /** 驱动参数映射：part group 索引 → 驱动参数索引 */
    private val driverParamMap = mutableMapOf<Int, Int>()

    /** 驱动参数阈值映射 */
    private val driverThresholdMap = mutableMapOf<Int, Float>()

    /**
     * 从 pose3.json 字节数据初始化。
     *
     * @param jsonBytes pose3.json 字节数据
     * @param partIds MOC3 的所有 part ID 列表
     * @param meta 模型元配置（可选，用于驱动参数关联）
     */
    fun initialize(jsonBytes: ByteArray, partIds: Array<String>?, meta: ModelMetaConfig? = null) {
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
            driverParamMap.clear()
            driverThresholdMap.clear()

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

                    // 如果 meta 提供了该组的驱动参数，则记录
                    if (meta?.poseMeta != null && i < meta.poseMeta.groups.size) {
                        val poseGroupMeta = meta.poseMeta.groups[i]
                        if (poseGroupMeta.driverParamIdx >= 0) {
                            driverParamMap[i] = poseGroupMeta.driverParamIdx
                            driverThresholdMap[i] = poseGroupMeta.driverThreshold
                        }
                    }
                }
            }

            initialized = partGroups.isNotEmpty()

            if (initialized && driverParamMap.isNotEmpty()) {
                Log.i(tag, "Pose initialized with ${partGroups.size} groups, ${driverParamMap.size} with driver params")
            } else {
                Log.i(tag, "Pose initialized with ${partGroups.size} groups")
            }
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
     * 使用驱动参数选择可见 part（如果有）：
     * - 参数值 < 阈值 → partIds[0] 可见
     * - 参数值 ≥ 阈值 → partIds[1] 可见
     * - 无驱动参数 → partIds[0] 始终可见（默认行为）
     *
     * @param model 数据模型
     * @param partNameToIndex 部件名到索引的映射
     * @param deltaTime 帧时间差（秒）
     */
    fun updateParameters(model: PurismModel, partNameToIndex: Map<String, Int>, deltaTime: Float) {
        if (!initialized) return

        val paramValues = Live2DNative.getParameterValues(modelPtr(model))

        for (groupIdx in partGroups.indices) {
            val group = partGroups[groupIdx]
            if (group.partIds.isEmpty()) continue

            // 确定当前应显示哪个 part
            val activeIdx: Int = if (driverParamMap.containsKey(groupIdx) && paramValues != null) {
                val driverIdx = driverParamMap[groupIdx]!!
                val threshold = driverThresholdMap[groupIdx] ?: 0.5f

                if (driverIdx < paramValues.size) {
                    val driverValue = paramValues[driverIdx]
                    val rawIdx = if (driverValue < threshold) 0 else 1
                    if (rawIdx >= group.partIds.size) 0 else rawIdx
                } else {
                    0
                }
            } else {
                0
            }

            for (j in group.partIds.indices) {
                val partIdx = partNameToIndex[group.partIds[j]] ?: continue
                if (j == activeIdx) {
                    model.setPartOpacity(partIdx, 1.0f)
                } else {
                    model.setPartOpacity(partIdx, 0.0f)
                }
            }
        }
    }

    companion object {
        private fun modelPtr(model: PurismModel): Long = model.modelPtr
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
    private val meta: ModelMetaConfig? = null,
    private val partNameToIndex: Map<String, Int> = emptyMap()
) {
    private val tag = "AnimationPipeline"

    /** Motion 管理器 */
    val motionManager: MotionManager = MotionManager(paramNameToIndex)

    /** 眨眼管理器 */
    val eyeBlink: EyeBlinkManager = EyeBlinkManager()

    /** 呼吸管理器 */
    val breath: BreathManager = BreathManager()

    /**
     * R13 程序化待机控制器（眼神漂移/头部微动/身体晃动；与眨眼/呼吸并列的参数层通道）。
     * 间歇轻晃调优（用户反馈③）：头部幅度 15°→5°、周期 3s→7s、身体 0.15→0.06、
     * 周期 8s→9s；运动 5s / 静止 10s 交替、首次启动先静 3s——观感对齐 N.E.K.O.
     * 预设动作间歇播放（静止期眼珠通道照常）。
     */
    val idleMotion: IdleMotionController = IdleMotionController(
        IdleMotionConfig(
            headAmplitudeDeg = 5f,
            headPeriodSeconds = 7f,
            bodyAmplitude = 0.06f,
            bodyPeriodSeconds = 9f,
            motionDurationSeconds = 5f,
            restDurationSeconds = 10f,
            initialRestSeconds = 3f,
        )
    )

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

    /** 是否启用程序化待机（R13：眼神漂移/头部微动/身体晃动） */
    var autoIdleMotionEnabled: Boolean = true

    // ── C6-fix2：表现力参数写入节流 ──
    // 背景：R13 IdleMotionController 在每次管线 update 都写眼神/头部/身体 5 个参数
    // （眼神 lerp 持续推进）→ native 参数每 update 变化 → purism 参数级脏标记短路
    // 命中率 96% → 0%（每 2 渲染帧一次全量重算 ~24ms）→ 帧率 111 → 61fps（机器实证
    // [C-Perf7-fix] light=0% full=120）。
    // 方案：每 6 渲染帧（≈50ms @120Hz，即 3 次管线 update）才应用一次 IdleMotion 输出，
    // 中间帧跳过写入——参数保持上次值，渲染连续。眼神阻尼 lerp / 头部低频正弦是慢变信号，
    // 50ms 步进视觉平滑；写入降频后脏标记每 6 帧才全量一次 → 帧率回到 90-111fps。
    /** 表现力参数写入节流阈值（渲染帧数 @120Hz；50ms 步进对慢变眼神/头部视觉平滑） */
    private val IDLE_MOTION_THROTTLE_FRAMES = 6

    /** 管线 update 对应的渲染帧数（renderer 每 MODEL_UPDATE_INTERVAL_FRAMES=2 渲染帧调用一次，C-Perf4） */
    private val RENDER_FRAMES_PER_PIPELINE_UPDATE = 2

    /** 节流计数器（渲染帧单位；累计 ≥ IDLE_MOTION_THROTTLE_FRAMES 才写入一次） */
    private var idleMotionThrottleCounter = 0

    // ── C8b：表情 fade 写入节流 + 物理更新降频 ──
    // 背景（机器证据）：120Hz 预算 8.3ms/帧；表情 fade 期间每帧写 12 键参数 → 每次管线 update
    // 都触发 native 全量重算（full 帧 15ms 超预算 1.8x）；物理（若有效）同样每次 update 写参数。
    // 方案：fade 插值每 2 次管线 update 写一次（fade 300-600ms → 步进 ≈33ms，视觉平滑）；
    // 物理每 4 渲染帧评估一次（慢变信号，视觉无损）。稳态（fade 完成/权重饱和）不节流：
    // 写入值恒定 → native 脏检查判 light，零开销。
    /** 表情 fade 写入间隔（管线 update 次数；每 2 次写一次 = 4 渲染帧 @120Hz） */
    private val EXPRESSION_FADE_WRITE_INTERVAL = 2

    /** fade 节流计数器（管线 update 单位） */
    private var expressionFadeWriteCounter = 0

    /** 物理评估间隔（渲染帧数；每 4 渲染帧一次 = 每 2 次管线 update 一次） */
    private val PHYSICS_UPDATE_INTERVAL_FRAMES = 4

    /** 物理节流计数器（渲染帧单位） */
    private var physicsUpdateCounter = 0

    // ── R17 说话联动（SpeakingMotion）状态 ──

    /**
     * R17 说话联动激活源（VoiceIoController 组合 isSpeaking：任一 TtsProvider 在播 = true）。
     * 由 Live2DRenderer 在管线创建时注入（参照 C7d lipSyncMouthOpenYYield 注入方式，同一组合流）；
     * null → 不激活（说话联动关闭，行为与 R17 前一致）。
     */
    @Volatile
    var speakingActiveProvider: (() -> Boolean)? = null

    /** R17 说话联动正弦相位累计时间（s）；说话停止 → 归零（下次从头相位起，无跳变） */
    private var speakingMotionTime: Float = 0f

    /** R17 说话联动写入节流计数器（渲染帧单位；与 idle 共用同一节流窗口语义，防脏标记短路失效） */
    private var speakingMotionThrottleCounter = 0

    /** R17 说话联动打点计数（真机取证日志降频：每 60 次写入一行） */
    private var speakingMotionLogCounter = 0

    /** 是否已保存初始快照 */
    private var hasInitialSnapshot: Boolean = false

    /**
     * 眨眼参数索引（使用 meta 语义索引优先，硬编码名称兜底）。
     *
     * 发现优先级：
     * 1. ModelMetaConfig.semantic.eyeLeftOpenIdx
     * 2. 直接查找 "PARAM_EYE_L_OPEN"（旧版 Cubism 3.x 命名）
     * 3. 查找 "ParamEyeLOpen"（Cubism 4.x/5.x 命名）
     */
    private var eyeLOpenIndex: Int = resolveEyeIndex(side = 'L')
    private var eyeROpenIndex: Int = resolveEyeIndex(side = 'R')

    /**
     * 口型同步参数索引（使用 meta 语义索引优先，硬编码名称兜底）。
     *
     * mouthOpenIndices 是 List<Int>；取第 0 个为 ParamA 类参数，
     * 取第 1 个为 ParamMouthOpenY 类参数。
     */
    private var paramAIndex: Int = resolveMouthIndex(0)
    private var paramMouthOpenY: Int = resolveMouthIndex(1)

    // ============================================================
    //  表情管理
    // ============================================================

    /**
     * 表情管理 — 在表情叠加层管理当前表情。
     */
    inner class ExpressionManager(private val paramNameToIndex: Map<String, Int>) {

        /** 当前播放的表情数据 */
        var currentExpression: ExpressionData? = null

        /** 正在淡出的旧表情数据（交叉淡变） */
        var previousExpression: ExpressionData? = null

        /** 表情权重（0~1，用于淡入淡出） */
        var expressionWeight: Float = 0f

        /** 旧表情淡出起始权重（切换瞬间当前表情的显示权重，避免中途切换弹跳） */
        var previousExpressionStartWeight: Float = 0f

        /** 表情淡入时间（秒） */
        var expressionFadeTime: Float = 0.5f

        /**
         * C7d LipSync 口型让位：非 null 且返回 true 时，本管理器跳过 ParamMouthOpenY 写入
         * （当前/淡出表情与参数覆盖全部让位——口型归 LipSync 独占，防双写者交替写同一参数致嘴部抽搐）。
         * 由 Live2DRenderer 在管线创建时注入（读取 VoiceIoController 组合 isSpeaking 流）。
         */
        @Volatile
        var lipSyncMouthOpenYYield: (() -> Boolean)? = null

        /** 表情淡入计时器 */
        private var expressionTimer: Float = 0f

        /**
         * 旧表情当前淡出权重（1→0，仅当存在旧表情且未完成淡出时 > 0）。
         *
         * 公式：起始权重 × (1 - timer / fadeTime)，与淡入共用同一计时器，
         * 保证新旧表情权重之和始终等于起始权重（切换瞬间）。
         */
        val previousExpressionWeight: Float
            get() {
                if (previousExpression == null) return 0f
                val t = (expressionTimer / expressionFadeTime).coerceIn(0f, 1f)
                return previousExpressionStartWeight * (1f - t)
            }

        /**
         * C8b：fade 是否进行中（timer 未达 fadeTime 且有活跃表情/旧表情/混合/覆盖）。
         * 供管线 fade 写入节流判定——fade 期间参数逐帧变化（每次写触发 native 全量），
         * 稳态（timer ≥ fadeTime）后写入值恒定 → 不需节流（native 脏检查判 light）。
         */
        val isFadeInProgress: Boolean
            get() {
                val hasActive = currentExpression != null ||
                    previousExpression != null ||
                    expressionMix.isNotEmpty() ||
                    parameterOverrides.isNotEmpty()
                return hasActive && expressionTimer < expressionFadeTime
            }

        /** 当前表情的目标强度（0-1），级3 情感强度。默认 1.0=满权重。 */
        var currentIntensity: Float = 1.0f

        /** 结构化混合表情列表：(ExpressionData, 权重)，数组顺序 = 应用顺序（contract-M2-M3 §1.3(a)） */
        var expressionMix: List<Pair<ExpressionData, Float>> = emptyList()

        /** 结构化参数覆盖乘数：key = 参数 ID，value = 乘数；在 mix 全部应用之后逐 key 叠加（§1.3(b)） */
        var parameterOverrides: Map<String, Float> = emptyMap()

        /**
         * 设置表情（与旧表情交叉淡变，支持情感强度）。
         * @param intensity 表情目标强度 0.33-1.0，默认 1.0（满权重）。控制淡入后的最终权重上限。
         */
        fun setExpression(expression: ExpressionData, intensity: Float = 1.0f) {
            currentIntensity = intensity.coerceIn(0f, 1f)
            // 当前表情仍在显示（权重 > 0）且与目标不同 → 保留为淡出对象
            previousExpression = currentExpression?.takeIf { it !== expression && expressionWeight > 0f }
            previousExpressionStartWeight = expressionWeight
            currentExpression = expression
            // 互斥（contract-M2-M3 §1.3(a)3）：单表情路径整体替换结构化混合状态
            expressionMix = emptyList()
            parameterOverrides = emptyMap()
            expressionTimer = 0f
            expressionWeight = 0f
        }

        /**
         * 清除表情。
         */
        fun clearExpression() {
            currentExpression = null
            previousExpression = null
            expressionMix = emptyList()
            parameterOverrides = emptyMap()
            expressionWeight = 0f
            previousExpressionStartWeight = 0f
            expressionTimer = 0f
        }

        /**
         * 设置结构化表情混合（contract-M2-M3 §1.3）。
         *
         * - 非空 mix → 整体替换单表情状态（互斥）；mix 每项按数组顺序逐帧应用，
         *   effectiveWeight = item.weight × expressionWeight（与单表情同淡入机制，0.5s）。
         * - 空 mix → 表情层不动作（不清除当前表情状态）；但 overrides 仍生效（§1.3(e) 部分生效）。
         * - overrides 乘数语义：final = current × (1 + (multiplier - 1) × expressionWeight)。
         */
        fun setExpressionMix(mix: List<Pair<ExpressionData, Float>>, overrides: Map<String, Float>) {
            parameterOverrides = overrides
            if (mix.isNotEmpty()) {
                previousExpression = currentExpression?.takeIf { expressionWeight > 0f }
                previousExpressionStartWeight = expressionWeight
                currentExpression = null
                currentIntensity = 1.0f
                expressionMix = mix
                expressionTimer = 0f
                expressionWeight = 0f
            } else if (currentExpression == null && expressionMix.isEmpty() && previousExpression == null) {
                // 空 mix 且无任何表情状态：启动淡入计时，让 overrides 随淡入进度逐步生效
                expressionTimer = 0f
                expressionWeight = 0f
            }
        }

        /**
         * 每帧更新表情。
         */
        fun update(deltaTime: Float) {
            val hasExpression = currentExpression != null || expressionMix.isNotEmpty()
            if (!hasExpression && parameterOverrides.isEmpty()) return

            expressionTimer += deltaTime
            expressionWeight = (expressionTimer / expressionFadeTime).coerceIn(0f, 1f) * currentIntensity

            // 交叉淡变结束 → 丢弃旧表情
            if (expressionTimer >= expressionFadeTime) {
                previousExpression = null
                previousExpressionStartWeight = 0f
            }
        }

        /**
         * 将表情应用到参数（同时应用新旧表情，分别乘各自的权重；
         * 结构化混合按数组顺序逐项应用 + 覆盖叠加）。
         */
        fun apply(model: PurismModel) {
            val hasSingle = currentExpression != null
            val hasMix = expressionMix.isNotEmpty()
            val hasOverrides = parameterOverrides.isNotEmpty()
            if (!hasSingle && !hasMix && !hasOverrides) return
            if (expressionWeight <= 0f && previousExpression == null) return

            val defaults = model.getParameterDefaultValues() ?: return
            val paramValues = model.getParameterValues() ?: return
            val minValues = model.getParameterMinValues()
            val maxValues = model.getParameterMaxValues()

            // 先应用旧表情（淡出），再应用新表情（淡入）
            previousExpression?.let { prev ->
                val prevWeight = previousExpressionWeight
                if (prevWeight > 0f) {
                    applyExpression(prev, prevWeight, paramValues, defaults, minValues, maxValues, model)
                }
            }
            if (expressionWeight > 0f) {
                currentExpression?.let { exp ->
                    applyExpression(exp, expressionWeight, paramValues, defaults, minValues, maxValues, model)
                }
                // 结构化混合：按数组顺序逐项应用（顺序敏感，防参数竞争；Add 累加 / Multiply 连乘 /
                // Overwrite 后应用者胜，§1.3(a)5）
                for ((exp, w) in expressionMix) {
                    if (w <= 0f) continue
                    applyExpression(exp, w * expressionWeight, paramValues, defaults, minValues, maxValues, model)
                }
                // 参数覆盖：混合全部应用后，逐 key：final = current × effective(multiplier)，§1.3(b)
                applyOverrides(parameterOverrides, expressionWeight, paramValues, minValues, maxValues, model)
            }
        }

        /**
         * 按指定权重把单个表情应用到参数。
         */
        private fun applyExpression(
            expression: ExpressionData,
            weight: Float,
            paramValues: FloatArray,
            defaults: FloatArray,
            minValues: FloatArray?,
            maxValues: FloatArray?,
            model: PurismModel
        ) {
            for (ep in expression.parameters) {
                val idx = paramNameToIndex[ep.id] ?: continue
                if (idx >= paramValues.size) continue
                // C7d：LipSync 播放期间 MouthOpenY 让位（口型归 LipSync 独占，防双写者交替写致嘴部抽搐）
                if (ep.id == MOUTH_OPEN_Y_ID && lipSyncMouthOpenYYield?.invoke() == true) continue

                val current = paramValues[idx]
                val defaultValue = defaults.getOrElse(idx) { 0f }
                val minV = minValues?.getOrElse(idx) { -1f } ?: -1f
                val maxV = maxValues?.getOrElse(idx) { 1f } ?: 1f

                val newValue = when (ep.blend) {
                    "Add" -> current + ep.value * weight
                    "Multiply" -> current * (1f + ep.value * weight)
                    "Overwrite" -> defaultValue + (ep.value - defaultValue) * weight
                    else -> current + ep.value * weight
                }

                val clamped = newValue.coerceIn(minV, maxV)
                model.setParameterValue(idx, clamped)
                // 回写快照：同一帧内后续表达式/覆盖读到累加后的值
                // （Add 累加 / Multiply 连乘 / Overwrite 后应用者胜，contract-M2-M3 §1.3(a)5）
                if (idx < paramValues.size) paramValues[idx] = clamped
            }
        }

        /**
         * 应用参数覆盖乘数（contract-M2-M3 §1.3(b)）。
         * 在 expressionMix 全部应用之后逐 key：final = current × (1 + (multiplier - 1) × weight)，
         * clamp 到参数合法范围（无合法范围信息按 [0,1]）。
         */
        private fun applyOverrides(
            overrides: Map<String, Float>,
            weight: Float,
            paramValues: FloatArray,
            minValues: FloatArray?,
            maxValues: FloatArray?,
            model: PurismModel
        ) {
            for ((paramId, multiplier) in overrides) {
                val idx = paramNameToIndex[paramId] ?: continue
                if (idx >= paramValues.size) continue
                // C7d：LipSync 播放期间 MouthOpenY 让位（参数覆盖同样不写口型）
                if (paramId == MOUTH_OPEN_Y_ID && lipSyncMouthOpenYYield?.invoke() == true) continue

                val current = paramValues[idx]
                // 乘数语义：随淡入进度逐步生效（expressionWeight=0 → 乘数 1.0 → 不变）
                val effective = 1f + (multiplier - 1f) * weight
                val minV = minValues?.getOrElse(idx) { 0f } ?: 0f
                val maxV = maxValues?.getOrElse(idx) { 1f } ?: 1f

                val clamped = (current * effective).coerceIn(minV, maxV)
                model.setParameterValue(idx, clamped)
                if (idx < paramValues.size) paramValues[idx] = clamped
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
        // C8b：fade 插值节流——fade 进行中每 EXPRESSION_FADE_WRITE_INTERVAL 次管线 update
        // 才 apply 一次（fade 期间参数逐帧变化 → 每帧写都触发 native 全量重算；间隔写后
        // 全量频率减半，步进 ≈33ms 视觉平滑）。fade 完成后写入值恒定 → 全量自然停止。
        // ExpressionManager 类零改动（update 计时/权重推进语义不变），节流仅发生在管线落点。
        if (expressionManager.isFadeInProgress) {
            expressionFadeWriteCounter++
            if (expressionFadeWriteCounter % EXPRESSION_FADE_WRITE_INTERVAL == 0) {
                expressionFadeWriteCounter = 0
                expressionManager.apply(model)
            }
        } else {
            expressionFadeWriteCounter = 0
            expressionManager.apply(model)
        }

        // 5. 更新眨眼
        eyeBlink.update(deltaTime)
        applyEyeBlink()

        // 6. 更新呼吸
        if (autoBreathEnabled) {
            breath.update(deltaTime)
            breath.apply(model, paramNameToIndex)
        }

        // 6.5 R13 程序化待机（眼神漂移/头部微动/身体晃动）——与眨眼/呼吸并列的参数层通道；
        //     有 motion 播放（如 niziiro_mao 的 idle group）时让位（待机不压制动作，search-report §2.1）
        //     C6-fix2：表现力参数写入节流——每 IDLE_MOTION_THROTTLE_FRAMES 渲染帧
        //     （≈50ms @120Hz = 3 次管线 update）才 update+apply 一次；中间帧跳过写入，
        //     参数保持上次值（渲染连续），使 native 脏标记短路恢复命中（机器实证：light 0%→63%，
        //     fps 61→87.5 avg，见 .pi/spoq/C6-fix2-throttle.md）。
        //     IdleMotionController 行为零改动（纯 JVM 可测类保持逐帧 update 语义，测试不受影响）；
        //     节流只发生在管线落点。
        //     R17 统一仲裁（LiaisonArbiter）：BODY_HEAD 通道候选 = emotion(情绪姿态帧) > speaking(说话点头/起伏) > idle；
        //     idle 头部/身体写入是否放行由仲裁裁决（情绪激活 LA-07 / 说话激活 LA-06 时让位），
        //     眼神通道不参赛（arbiter 仅 BODY_HEAD/MOUTH 两通道）——保留既有无条件眼神写入。
        val liaison = arbitrateLiaison()

        if (autoIdleMotionEnabled && !motionManager.isPlaying()) {
            idleMotionThrottleCounter += RENDER_FRAMES_PER_PIPELINE_UPDATE
            if (idleMotionThrottleCounter >= IDLE_MOTION_THROTTLE_FRAMES) {
                idleMotionThrottleCounter = 0
                applyIdleMotion(idleMotion.update(deltaTime), liaison)
            }
        }

        // 6.5.1 R17 说话联动（头部点头 + 身体起伏）——BODY_HEAD 通道经仲裁器：
        //     说话时 idle 头/身让位（LA-06）；情绪激活时说话点头/起伏让位但口型仍驱动（LA-08，
        //     口型由 LipSync 独占，见 C7d）。与 idle 共用节流窗口语义（低频正弦慢变信号，
        //     每 IDLE_MOTION_THROTTLE_FRAMES 渲染帧写入一次，防 native 脏标记短路失效掉帧）。
        val speakingNow = speakingActiveProvider?.invoke() == true
        if (!speakingNow) {
            // 说话停止 → 相位归零（下次从头相位起）；节流计数复位
            speakingMotionTime = 0f
            speakingMotionThrottleCounter = 0
        }
        if (speakingNow && !motionManager.isPlaying()) {
            speakingMotionThrottleCounter += RENDER_FRAMES_PER_PIPELINE_UPDATE
            if (speakingMotionThrottleCounter >= IDLE_MOTION_THROTTLE_FRAMES) {
                speakingMotionThrottleCounter = 0
                applySpeakingMotion(deltaTime, liaison)
            }
        }

        // 7. 更新物理（C8b：每 PHYSICS_UPDATE_INTERVAL_FRAMES=4 渲染帧评估一次——
        //     慢变信号视觉无损，全量触发频率减半。bai 当前物理为 inactive（physics3.json
        //     Input 为数组，简化解析器取不到 → 本段为空操作，零行为变化；对其他模型是
        //     安全的降频守卫）
        if (physics.isValid()) {
            physicsUpdateCounter += RENDER_FRAMES_PER_PIPELINE_UPDATE
            if (physicsUpdateCounter >= PHYSICS_UPDATE_INTERVAL_FRAMES) {
                physicsUpdateCounter = 0
                physics.evaluate(model, paramNameToIndex, deltaTime)
            }
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
     * 使用已解析的 eyeLOpenIndex / eyeROpenIndex（从 meta 或硬编码兜底）。
     */
    private fun applyEyeBlink() {
        if (!autoBlinkEnabled) return

        val leftOpen = eyeBlink.getLeftEyeOpen()
        val rightOpen = eyeBlink.getRightEyeOpen()

        if (eyeLOpenIndex >= 0) {
            model.setParameterValue(eyeLOpenIndex, leftOpen)
        }
        if (eyeROpenIndex >= 0) {
            model.setParameterValue(eyeROpenIndex, rightOpen)
        }
    }

    /**
     * R16 半身姿态联动：当前/淡出中的表情是否含非零姿态帧键（情绪姿态激活判定）。
     *
     * 姿态帧键 = BodyPoseFrames.POSE_KEYS（ParamBodyAngleX/Y/Z + ParamAngleX/Y/Z）。
     * 值非零判定：neutral 姿态帧全 0（无姿态基准）→ 不判激活，idle 正常；
     * 非 neutral 七情绪帧均含非零键 → 判激活。表情面键（Brow/Eye/Mouth）不在此列。
     * 淡出中的旧表情（previousExpression，如姿态→neutral 交叉淡变）也计入——
     * 避免淡变期间 idle 写回与残存姿态打架。
     */
    private fun emotionalPoseActive(): Boolean {
        fun hasPose(exp: ExpressionData?): Boolean =
            exp?.parameters?.any { it.id in BodyPoseFrames.POSE_KEYS && it.value != 0f } == true
        return hasPose(expressionManager.currentExpression) || hasPose(expressionManager.previousExpression)
    }

    /**
     * R13 程序化待机落点：把 IdleSample 应用到模型参数（眼神/头部/身体）。
     * 三通道与既有通道互不重叠：EyeBall（眼神）不碰眨眼 EyeLOpen/ROpen；Angle/BodyAngle 不碰呼吸 ParamBreath。
     * 全部按 native min/max 钳制（bai：角度 ±30、眼球 ±1）。
     *
     * R17 统一仲裁（task-C8 / T8）：BODY_HEAD 通道由 [LiaisonArbiter] 裁决
     * （emotion(姿态帧) > speaking(说话) > idle），本落点只在仲裁判 IDLE 驱动时写头部/身体：
     * - 情绪姿态帧激活（LA-07）→ idle 头部角度 + 身体晃动整体让位（不写 = 姿态帧绝对值保留，
     *   防每 ~50ms 打架）——R16 既有行为收敛进仲裁器；
     * - 说话激活（LA-06）→ idle 头部/身体同样让位（说话点头/起伏接管，经 SpeakingMotion 落点）。
     * 眼神通道保留（姿态帧无 EyeBall 键、说话联动无 EyeBall 键，零交集）。
     * deviation 记录（R16 遗留，语义不变）：任务字面“bodySway 幅度降 0（Angles 保留）”
     * ——落点实现为 bodySway 与头部 Angles 一并让位，理由：姿态帧含 ParamAngleX/Y 且 Overwrite 绝对值
     * （sadness AX=-12 等），保留 idle 头写会每 ~50ms 覆盖情绪头位使 R16 轴语义不可见；
     * IdleMotionController 类本身零改动（Angles 持续计算，测试不受影响），
     * 让位仅发生在管线落点，与 R13 “motion 播放时 idle 让位”同语义。
     */
    private fun applyIdleMotion(
        sample: IdleSample,
        arbitration: LiaisonArbiter.ArbitrationResult,
    ) {
        val minValues = Live2DNative.getParameterMinValues(modelPtr(model))
        val maxValues = Live2DNative.getParameterMaxValues(modelPtr(model))
        fun applyParam(id: String, value: Float) {
            val idx = paramNameToIndex[id] ?: return
            val minV = minValues?.getOrElse(idx) { -1f } ?: -1f
            val maxV = maxValues?.getOrElse(idx) { 1f } ?: 1f
            model.setParameterValue(idx, value.coerceIn(minV, maxV))
        }
        applyParam("ParamEyeBallX", sample.eyeBallX)
        applyParam("ParamEyeBallY", sample.eyeBallY)
        // R17：BODY_HEAD 通道仲裁（emotion > speaking > idle）——非 IDLE 驱动则头部/身体让位
        // （情绪姿态帧 / 说话联动接管）；眼神通道不参赛，恒写（保留既有行为）
        if (arbitration.drives[LiaisonArbiter.Channel.BODY_HEAD] != LiaisonArbiter.Source.IDLE) return
        applyParam("ParamAngleX", sample.headAngleX)
        applyParam("ParamAngleY", sample.headAngleY)
        // 身体晃动：归一化 bodySway（±0.15）→ 角度（×±30 钳制；低频，周期 > 头部）
        applyParam("ParamBodyAngleX", sample.bodySway * IdleMotionController.BAI_ANGLE_CLAMP_DEG)
    }

    /**
     * R17 统一联动仲裁（[LiaisonArbiter] 单入口）：每帧收集 4 源激活状态 → 仲裁。
     * - emotion: 情绪姿态帧激活（R16 [emotionalPoseActive]，含淡出中的旧表情）
     * - speaking: TTS 播放中（VoiceIoController 组合 isSpeaking，经 [speakingActiveProvider] 注入）
     * - expression: 表情层有活跃表达式（含淡出中的旧表情与结构化 mix；驱动 MOUTH 通道）
     * - idle: 程序化待机可用（autoIdleMotionEnabled 且无 motion 播放）
     * 表情/姿态应用入口（EmotionController → Live2DRenderer.setParametricExpression → ExpressionManager）
     * 以 expressionManager 状态在此登记激活——仲裁器每帧读取，无需跨组件额外状态。
     */
    private fun arbitrateLiaison(): LiaisonArbiter.ArbitrationResult {
        val expressionActive = expressionManager.currentExpression != null ||
            expressionManager.previousExpression != null ||
            expressionManager.expressionMix.isNotEmpty()
        return LiaisonArbiter.arbitrate(
            LiaisonArbiter.Sources(
                emotionActive = emotionalPoseActive(),
                speakingActive = speakingActiveProvider?.invoke() == true,
                expressionActive = expressionActive,
                idleActive = autoIdleMotionEnabled && !motionManager.isPlaying(),
            )
        )
    }

    /**
     * R17 说话联动落点（[SpeakingMotion] 低频正弦 → 模型参数）：
     * - 点头 → ParamAngleX（头 pitch，低频 0.5-1Hz，±4°）
     * - 起伏 → ParamBodyAngleY（身体，周期 2.4s 与呼吸 2.5s 错相位，±2°）
     * 仲裁：BODY_HEAD 通道说话让位（emotion 激活，LA-08）→ 相位冻结（正弦值连续，恢复后续写不跳变）。
     * 说话停止 → 相位由调用方归零（[speakingMotionTime]）。全部按 native min/max 钳制。
     */
    private fun applySpeakingMotion(
        deltaTime: Float,
        arbitration: LiaisonArbiter.ArbitrationResult,
    ) {
        // 仲裁：emotion 激活 → speaking 在 BODY_HEAD 通道让位（LA-08；口型仍由 LipSync 独占驱动）
        if (arbitration.drives[LiaisonArbiter.Channel.BODY_HEAD] != LiaisonArbiter.Source.SPEAKING) return
        speakingMotionTime += deltaTime
        val nod = SpeakingMotion.headNodOffset(speakingMotionTime)
        val rise = SpeakingMotion.bodyRiseOffset(speakingMotionTime)
        applyClampedParam("ParamAngleX", nod)
        applyClampedParam("ParamBodyAngleY", rise)
        // R17 取证打点（真机 logcat：低频正弦相位可观测；降频每 60 次写入一行 ≈3s）
        speakingMotionLogCounter++
        if (speakingMotionLogCounter % 60 == 1) {
            Log.i(tag, "[SpeakingMotion] t=${String.format("%.2f", speakingMotionTime)}s " +
                "nod=${String.format("%.2f", nod)}deg rise=${String.format("%.2f", rise)}deg")
        }
    }

    /** 按 native min/max 钳制写入单参数（缺失参数静默跳过） */
    private fun applyClampedParam(id: String, value: Float) {
        val idx = paramNameToIndex[id] ?: return
        val minV = Live2DNative.getParameterMinValues(modelPtr(model))?.getOrElse(idx) { -1f } ?: -1f
        val maxV = Live2DNative.getParameterMaxValues(modelPtr(model))?.getOrElse(idx) { 1f } ?: 1f
        model.setParameterValue(idx, value.coerceIn(minV, maxV))
    }

    /**
     * 设置口型同步值。
     * 同时设置所有可用的口型参数（如 ParamA + ParamMouthOpenY）。
     *
     * M-AND-3（AND-L5，选型 4-A）：本函数为「native 写唯一落点」之一，**仅经
     * LipSyncParamChannel drain 的 onApply 调用（GL 线程）**——Live2DRenderer.onDrawFrame
     * 帧首 drain 消费 Main 嘴型协程 submit 的 pending 后回调本函数（AND3-A2/A4）。
     * 不再允许 Main 线程直调（AND3-A1：调用链上无 Main → model.setParameterValue 直写）。
     */
    fun setLipSyncValue(value: Float) {
        val clamped = value.coerceIn(0f, 1f)
        if (paramAIndex >= 0) {
            model.setParameterValue(paramAIndex, clamped)
        }
        if (paramMouthOpenY >= 0) {
            model.setParameterValue(paramMouthOpenY, clamped)
        }

        // 如果还有额外口型参数（meta 提供了多于 2 个），也一并设置
        if (meta != null) {
            for (i in 2 until meta.semantic.mouthOpenIndices.size) {
                val idx = meta.semantic.mouthOpenIndices[i]
                if (idx >= 0) {
                    model.setParameterValue(idx, clamped)
                }
            }
        }
    }

    /**
     * 从 meta 语义索引或硬编码名称解析眼睛参数索引。
     */
    private fun resolveEyeIndex(side: Char): Int {
        // 一级：meta 语义索引
        if (meta != null) {
            val idx = if (side == 'L') meta.semantic.eyeLeftOpenIdx
                      else meta.semantic.eyeRightOpenIdx
            if (idx >= 0) return idx
        }

        // 二级：硬编码名称查找（旧模型向后兼容）
        val primaryName = "PARAM_EYE_${side}_OPEN"
        val altName = "ParamEye${side}Open"
        return paramNameToIndex[primaryName] ?: paramNameToIndex[altName] ?: -1
    }

    /**
     * 从 meta 语义索引或硬编码名称解析口型参数索引。
     *
     * @param indexInList 在 mouthOpenIndices 列表中的位置
     */
    private fun resolveMouthIndex(indexInList: Int): Int {
        // 一级：meta 语义索引
        if (meta != null) {
            if (indexInList < meta.semantic.mouthOpenIndices.size) {
                return meta.semantic.mouthOpenIndices[indexInList]
            }
        }

        // 二级：硬编码名称查找（旧模型向后兼容）
        return when (indexInList) {
            0 -> paramNameToIndex["ParamA"] ?: -1
            1 -> paramNameToIndex["ParamMouthOpenY"] ?: -1
            else -> -1
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
