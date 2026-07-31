package com.live2dai.plugin.proactive

import android.content.Context

/**
 * 记录「最后对话时间」与「最后一次主动消息时间」的轻量持久化（SharedPreferences）。
 *
 * 为什么不用 Room：这里只存两个时间戳，数据极简，Room 是过度设计（见 docs/plan-task-plugin-proactive.md §3.3）。
 *
 * 所有读取方法都参数化 `nowMillis`，由调用方注入当前时间——
 * 生产环境传 [System.currentTimeMillis()]，测试传固定时间戳，避免时间相关的 flaky test（R4）。
 */
class LastInteractionTracker(context: Context) {

    private val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)

    /** 记录一轮真实对话的结束时间（由 [ProactiveTrackingHook.afterReceive] 调用）。 */
    fun recordInteraction(atMillis: Long = System.currentTimeMillis()) {
        prefs.edit().putLong(KEY_LAST_INTERACTION, atMillis).apply()
    }

    /** 记录一次主动问候的发送时间（由 [ProactiveCheckWorker] 触发成功后调用）。 */
    fun recordProactiveMessage(atMillis: Long = System.currentTimeMillis()) {
        prefs.edit().putLong(KEY_LAST_PROACTIVE, atMillis).apply()
    }

    fun lastInteractionAt(): Long = prefs.getLong(KEY_LAST_INTERACTION, 0L)

    fun lastProactiveAt(): Long = prefs.getLong(KEY_LAST_PROACTIVE, 0L)

    /**
     * 距上次真实对话已过去的分钟数。
     *
     * 从未有过对话记录时返回 [Long.MAX_VALUE]——"上次对话"可视为无限久远之前，
     * 此时只要不处于静默时段/冷却期就允许主动问候（首次使用 App 的"初见面打招呼"场景）。
     */
    fun minutesSinceLastInteraction(nowMillis: Long = System.currentTimeMillis()): Long =
        minutesSince(KEY_LAST_INTERACTION, nowMillis)

    /** 距上次主动问候已过去的分钟数，语义同 [minutesSinceLastInteraction]。 */
    fun minutesSinceLastProactive(nowMillis: Long = System.currentTimeMillis()): Long =
        minutesSince(KEY_LAST_PROACTIVE, nowMillis)

    private fun minutesSince(key: String, nowMillis: Long): Long {
        val last = prefs.getLong(key, 0L)
        if (last <= 0L) return Long.MAX_VALUE
        // 时钟回拨等导致的时间戳在未来 → 钳制为 0，避免负数分钟
        return (nowMillis - last).coerceAtLeast(0L) / MILLIS_PER_MINUTE
    }

    internal companion object {
        const val PREFS_NAME = "live2dai_proactive"
        const val KEY_LAST_INTERACTION = "last_interaction_at"
        const val KEY_LAST_PROACTIVE = "last_proactive_at"
        const val MILLIS_PER_MINUTE = 60_000L
    }
}
