package com.live2d.ai.android.render

/**
 * 动画更新通道（seam，纯 JVM）。
 * 与官方 CubismUpdateScheduler 通道一一对应。
 * 注：本 enum 声明顺序只决定 [UpdaterRegistry.scan] 的产出顺序；
 * 官方 executionOrder（vendored ICubismUpdater：EyeBlink200/Expression300/Look400/
 * Breath500/Physics600/LipSync700/Pose800）由 C++ 侧 SortUpdatableList 重排，勿据本 enum 序推断官方序。
 */
enum class UpdaterChannel {
    MOTION, EXPRESSION, EYE_BLINK, BREATH, PHYSICS, POSE, LIP_SYNC
}

/** 模型动画能力描述（7 布尔，来自模型资产盘点：motion3/exp3/pose3/cdi3 参数）。 */
data class ModelAnimationCapabilities(
    val hasMotionAssets: Boolean,
    val hasExpressionAssets: Boolean,
    val hasPoseAssets: Boolean,
    val hasEyeBlinkParams: Boolean,
    val hasBreathParams: Boolean,
    val hasPhysicsAssets: Boolean,
    val hasLipSyncParams: Boolean,
)

/** 一条应注册记录：通道 + 官方 updater 标识（对应 C++ 端注册类名）。 */
data class RegisteredUpdater(
    val channel: UpdaterChannel,
    val updaterId: String,
)

/**
 * 官方 updater 注册扫描器（seam，纯 JVM）。
 *
 * 激活矩阵（bai 铁律 + A-4 裁定）：
 * - MOTION / EXPRESSION / POSE：无资产 → 不注册（bai 无 motion3/exp3/pose3 → no-op）；
 * - EYE_BLINK：恒注册（bai ParamEyeLOpen/ROpen 存在 → 激活）；
 * - BREATH：恒注册，但视觉 no-op（bai 无 ParamBreath，行为零变化，注册即可）；
 * - PHYSICS：按 physicsEnabled（A-4=D1 true 注册 / D2 false 不注册）；
 * - LIP_SYNC：恒注册（bai ParamMouthOpenY/ParamA 存在 → 激活）。
 */
object UpdaterRegistry {

    private const val ID_MOTION = "CubismMotionUpdater"
    private const val ID_EXPRESSION = "CubismExpressionUpdater"
    private const val ID_EYE_BLINK = "CubismEyeBlinkUpdater"
    private const val ID_BREATH = "CubismBreathUpdater"
    private const val ID_PHYSICS = "CubismPhysicsUpdater"
    private const val ID_POSE = "CubismPoseUpdater"
    private const val ID_LIP_SYNC = "CubismLipSyncUpdater"

    /** 按激活矩阵扫描，返回应注册的 updater 列表（通道声明序）。 */
    fun scan(
        caps: ModelAnimationCapabilities,
        physicsEnabled: Boolean = true,
    ): List<RegisteredUpdater> {
        val result = mutableListOf<RegisteredUpdater>()

        // MOTION：无资产 → 不注册（bai 铁律）
        if (caps.hasMotionAssets) {
            result += RegisteredUpdater(UpdaterChannel.MOTION, ID_MOTION)
        }
        // EXPRESSION：无资产 → 不注册（bai 铁律）
        if (caps.hasExpressionAssets) {
            result += RegisteredUpdater(UpdaterChannel.EXPRESSION, ID_EXPRESSION)
        }
        // EYE_BLINK：恒注册
        result += RegisteredUpdater(UpdaterChannel.EYE_BLINK, ID_EYE_BLINK)
        // BREATH：恒注册，但视觉 no-op（bai 无 ParamBreath，行为零变化）
        result += RegisteredUpdater(UpdaterChannel.BREATH, ID_BREATH)
        // PHYSICS：按 A-4 物理开关（D1=true 注册 / D2=false 不注册）
        if (physicsEnabled) {
            result += RegisteredUpdater(UpdaterChannel.PHYSICS, ID_PHYSICS)
        }
        // POSE：无资产 → 不注册（bai 铁律）
        if (caps.hasPoseAssets) {
            result += RegisteredUpdater(UpdaterChannel.POSE, ID_POSE)
        }
        // LIP_SYNC：恒注册
        result += RegisteredUpdater(UpdaterChannel.LIP_SYNC, ID_LIP_SYNC)

        return result
    }
}
