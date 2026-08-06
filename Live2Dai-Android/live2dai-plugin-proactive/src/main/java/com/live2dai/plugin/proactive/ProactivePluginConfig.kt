package com.live2dai.plugin.proactive

import android.content.Context

/**
 * 主动陪伴插件构造参数（由 core 注入）。
 *
 * @param context applicationContext——WorkManager 排期需要 Context（见 docs/plan-task-plugin-proactive.md §2）
 * @param greetingLlmClient 问候文案生成器（LLM 即时生成，贴合人设；失败回退 [fallbackGreetings]）
 * @param memoriesProvider 最近长期记忆提供者（V1.2 记忆驱动话题，plan §5.6.3）：触发前由 Worker 调用，
 *                         返回最近若干条记忆事实作为话题上下文；读取失败静默降级为空列表
 * @param idleThresholdMinutes 距上次真实对话超过该分钟数才允许主动问候（默认 180 = 3 小时）
 * @param cooldownMinutes 距上次主动问候的最小冷却时间，避免频繁刷屏（默认 360 = 6 小时）
 * @param quietHoursStart 静默时段起始小时（24 小时制，默认 23 = 23:00）
 * @param quietHoursEnd 静默时段结束小时（默认 7 = 07:00；与 start 组成跨天区间 [23:00, 07:00)）
 * @param fallbackGreetings LLM 不可用/调用失败时的静态文案池（随机取一条兜底）
 * @param workManagerIntervalMinutes 周期检查间隔（分钟）。默认 15 = WorkManager PeriodicWorkRequest
 *                                   系统最小间隔（硬约束，不可更短）；release 生产用 60（每小时）
 * @param debugMode Debug 短周期开关（E2E 教训，plan §5.6.4）：true 时额外排一次立即检查，
 *                  「冷启动 → 空闲超阈值」无需等一个完整周期即可触发，方便验收；由 core 按 BuildConfig.DEBUG 传入
 */
data class ProactivePluginConfig(
    val context: Context,
    val greetingLlmClient: GreetingLlmClient,
    val memoriesProvider: suspend () -> List<String> = { emptyList() },
    val idleThresholdMinutes: Long = DEFAULT_IDLE_THRESHOLD_MINUTES,
    val cooldownMinutes: Long = DEFAULT_COOLDOWN_MINUTES,
    val quietHoursStart: Int = DEFAULT_QUIET_HOURS_START,
    val quietHoursEnd: Int = DEFAULT_QUIET_HOURS_END,
    val fallbackGreetings: List<String> = DEFAULT_FALLBACK_GREETINGS,
    val workManagerIntervalMinutes: Long = DEFAULT_WORK_MANAGER_INTERVAL_MINUTES,
    val debugMode: Boolean = false,
) {
    companion object {
        const val DEFAULT_IDLE_THRESHOLD_MINUTES = 180L
        const val DEFAULT_COOLDOWN_MINUTES = 360L
        const val DEFAULT_QUIET_HOURS_START = 23
        const val DEFAULT_QUIET_HOURS_END = 7

        /** WorkManager PeriodicWorkRequest 系统最小间隔（分钟）——硬约束，不可更短 */
        const val DEFAULT_WORK_MANAGER_INTERVAL_MINUTES = 15L

        val DEFAULT_FALLBACK_GREETINGS = listOf(
            "在忙什么呀？好久没理我了~",
            "主人，该喝水啦！",
            "今天也要元气满满哦！",
        )
    }
}
