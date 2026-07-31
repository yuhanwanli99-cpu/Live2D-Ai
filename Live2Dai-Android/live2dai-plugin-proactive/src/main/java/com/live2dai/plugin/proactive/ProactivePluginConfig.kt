package com.live2dai.plugin.proactive

import android.content.Context

/**
 * 主动陪伴插件构造参数（由 core 注入）。
 *
 * @param context applicationContext——WorkManager 排期需要 Context（见 docs/plan-task-plugin-proactive.md §2）
 * @param greetingLlmClient 问候文案生成器（LLM 即时生成，贴合人设；失败回退 [fallbackGreetings]）
 * @param idleThresholdMinutes 距上次真实对话超过该分钟数才允许主动问候（默认 180 = 3 小时）
 * @param cooldownMinutes 距上次主动问候的最小冷却时间，避免频繁刷屏（默认 360 = 6 小时）
 * @param quietHoursStart 静默时段起始小时（24 小时制，默认 23 = 23:00）
 * @param quietHoursEnd 静默时段结束小时（默认 7 = 07:00；与 start 组成跨天区间 [23:00, 07:00)）
 * @param fallbackGreetings LLM 不可用/调用失败时的静态文案池（随机取一条兜底）
 */
data class ProactivePluginConfig(
    val context: Context,
    val greetingLlmClient: GreetingLlmClient,
    val idleThresholdMinutes: Long = DEFAULT_IDLE_THRESHOLD_MINUTES,
    val cooldownMinutes: Long = DEFAULT_COOLDOWN_MINUTES,
    val quietHoursStart: Int = DEFAULT_QUIET_HOURS_START,
    val quietHoursEnd: Int = DEFAULT_QUIET_HOURS_END,
    val fallbackGreetings: List<String> = DEFAULT_FALLBACK_GREETINGS,
) {
    companion object {
        const val DEFAULT_IDLE_THRESHOLD_MINUTES = 180L
        const val DEFAULT_COOLDOWN_MINUTES = 360L
        const val DEFAULT_QUIET_HOURS_START = 23
        const val DEFAULT_QUIET_HOURS_END = 7
        val DEFAULT_FALLBACK_GREETINGS = listOf(
            "在忙什么呀？好久没理我了~",
            "主人，该喝水啦！",
            "今天也要元气满满哦！",
        )
    }
}
