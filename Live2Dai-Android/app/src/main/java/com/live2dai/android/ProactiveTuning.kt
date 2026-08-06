package com.live2dai.android

/**
 * 主动陪伴 Debug/Release 参数开关（V1.2 模块6 Debug 短周期开关，docs/plan-v12-plan.md §5.6.4）。
 *
 * E2E 教训：长周期功能（如 release 的 1 小时周期 / 30 分钟空闲阈值）在验收时几乎无法等待，
 * debug 构建必须内置缩短版周期/阈值。core（MainActivity）按 [BuildConfig.DEBUG] 选择
 * [forDebug] 返回的参数组注入 [com.live2dai.plugin.proactive.ProactivePluginConfig]。
 *
 * 注意：WorkManager PeriodicWorkRequest 的最小间隔是 **15 分钟**（Android 平台硬约束，
 * 更短会抛 IllegalArgumentException），因此 debug 的"短周期"落到 15 分钟下限，
 * 真正的加速来自空闲阈值（5 分钟 vs 30 分钟）与冷却（2 分钟 vs 6 小时），
 * 再配合插件内 debugMode 的「立即检查」——冷启动后无需等完整周期即可触发。
 */
data class ProactiveTiming(
    /** WorkManager 周期检查间隔（分钟）；< 15 会被插件钳制到 15 */
    val workManagerIntervalMinutes: Long,
    /** 距上次真实对话超过该分钟数才允许主动问候 */
    val idleThresholdMinutes: Long,
    /** 距上次主动问候的最小冷却分钟数 */
    val cooldownMinutes: Long,
)

object ProactiveTuning {

    /** Debug 构建（E2E 验收）：周期钳制到 WorkManager 最小 15 分钟，空闲阈值 5 分钟、冷却 2 分钟 */
    val DEBUG = ProactiveTiming(
        workManagerIntervalMinutes = 15L,
        idleThresholdMinutes = 5L,
        cooldownMinutes = 2L,
    )

    /** Release 构建（生产）：周期 1 小时、空闲阈值 30 分钟、冷却 6 小时 */
    val RELEASE = ProactiveTiming(
        workManagerIntervalMinutes = 60L,
        idleThresholdMinutes = 30L,
        cooldownMinutes = 360L,
    )

    /** 按构建类型选择参数组（isDebug = BuildConfig.DEBUG）。 */
    fun forDebug(isDebug: Boolean): ProactiveTiming = if (isDebug) DEBUG else RELEASE
}
