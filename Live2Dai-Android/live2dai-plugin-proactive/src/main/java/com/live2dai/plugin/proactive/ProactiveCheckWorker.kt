package com.live2dai.plugin.proactive

import android.content.Context
import android.util.Log
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import java.util.Calendar

/**
 * 周期性后台检查任务：到达触发条件时让助手主动说一句话。
 *
 * 触发条件（**全部**满足才触发，见 docs/plan-task-plugin-proactive.md §2.2）：
 * 1. 不在静默时段（[KEY_QUIET_HOURS_START]~[KEY_QUIET_HOURS_END]，默认 23:00-07:00）
 * 2. 距上次真实对话超过 idle 阈值（[KEY_IDLE_THRESHOLD_MINUTES]，默认 180 分钟）
 * 3. 距上次主动问候超过冷却时间（[KEY_COOLDOWN_MINUTES]，默认 360 分钟）
 *
 * 决策所需的可序列化参数全部来自 [androidx.work.Data]（由 [ProactivePlugin.onLoad] 写入 inputData）；
 * 不可序列化的 [com.live2dai.pluginsdk.PluginHost] / [GreetingLlmClient] 通过 [ProactiveHostBridge] 获取。
 *
 * 测试确定性（design: 可注入 Clock）：通过 [nowMillis] 注入固定时间戳，
 * 配合预置的 SharedPreferences 时间戳即可覆盖所有决策分支，不依赖真实系统时钟。
 */
class ProactiveCheckWorker(
    context: Context,
    params: WorkerParameters,
) : CoroutineWorker(context, params) {

    /** 可注入时钟：生产环境为系统时钟，测试注入固定时间戳。 */
    internal var nowMillis: () -> Long = { System.currentTimeMillis() }

    override suspend fun doWork(): Result {
        val tracker = LastInteractionTracker(applicationContext)
        val now = nowMillis()
        val idleThresholdMinutes = inputData.getLong(KEY_IDLE_THRESHOLD_MINUTES, DEFAULT_IDLE_THRESHOLD_MINUTES)
        val cooldownMinutes = inputData.getLong(KEY_COOLDOWN_MINUTES, DEFAULT_COOLDOWN_MINUTES)
        val quietHoursStart = inputData.getInt(KEY_QUIET_HOURS_START, DEFAULT_QUIET_HOURS_START)
        val quietHoursEnd = inputData.getInt(KEY_QUIET_HOURS_END, DEFAULT_QUIET_HOURS_END)
        val fallbackGreetings = inputData.getStringArray(KEY_FALLBACK_GREETINGS)?.toList()
            ?: ProactivePluginConfig.DEFAULT_FALLBACK_GREETINGS

        // 条件 1：静默时段不打扰（睡眠时间）
        if (isWithinQuietHours(now, quietHoursStart, quietHoursEnd)) return Result.success()
        // 条件 2：用户最近有真实对话 → 不打断
        if (tracker.minutesSinceLastInteraction(now) < idleThresholdMinutes) return Result.success()
        // 条件 3：距上次主动消息太近 → 避免刷屏
        if (tracker.minutesSinceLastProactive(now) < cooldownMinutes) return Result.success()

        // 插件未加载（如 onUnload 清空桥与 Worker 执行存在竞态）：无可发送渠道，直接跳过
        val host = ProactiveHostBridge.host
        if (host == null) {
            Log.w(TAG, "ProactiveHostBridge.host 为空，跳过主动问候（插件可能已卸载）")
            return Result.success()
        }

        val idleMinutes = tracker.minutesSinceLastInteraction(now)
        val greeting = generateGreeting(idleMinutes)
            ?: fallbackGreetings.ifEmpty { ProactivePluginConfig.DEFAULT_FALLBACK_GREETINGS }.random()

        host.sendAssistantMessage(greeting)
        tracker.recordProactiveMessage(now)
        // 通知栏同步提示（App 在后台/息屏时用户也能看到）：权限缺失/禁用/失败一律静默
        val personaName = runCatching { host.getPersona().name }.getOrNull().orEmpty()
        ProactiveNotifier.notify(applicationContext, personaName, greeting)
        return Result.success()
    }

    /**
     * 优先走 LLM 即时生成（结合人设 + 当前时间 + 最近记忆，plan §5.6.3 记忆驱动话题）；
     * 客户端缺失、调用异常或返回 null 时返回 null，由调用方回退静态文案池。
     */
    private suspend fun generateGreeting(idleMinutes: Long): String? {
        val client = ProactiveHostBridge.greetingLlmClient ?: return null
        val persona = runCatching { ProactiveHostBridge.host?.getPersona() }.getOrNull()
        // 记忆提供者缺失 / 读取失败 → 空列表（LLM 仅按人设+时间生成，不影响链路）
        val memories = runCatching {
            ProactiveHostBridge.memoriesProvider?.invoke().orEmpty()
        }.getOrDefault(emptyList())
        return runCatching {
            client.generateGreeting(
                personaName = persona?.name.orEmpty(),
                systemPrompt = persona?.systemPrompt.orEmpty(),
                idleMinutes = idleMinutes,
                memories = memories,
            )
        }.getOrNull()
    }

    companion object {
        const val KEY_IDLE_THRESHOLD_MINUTES = "idle_threshold_minutes"
        const val KEY_COOLDOWN_MINUTES = "cooldown_minutes"
        const val KEY_QUIET_HOURS_START = "quiet_hours_start"
        const val KEY_QUIET_HOURS_END = "quiet_hours_end"
        const val KEY_FALLBACK_GREETINGS = "fallback_greetings"

        // 与 ProactivePluginConfig 的默认值保持一致（插件总是显式写入 inputData，这里只是兜底）
        const val DEFAULT_IDLE_THRESHOLD_MINUTES = ProactivePluginConfig.DEFAULT_IDLE_THRESHOLD_MINUTES
        const val DEFAULT_COOLDOWN_MINUTES = ProactivePluginConfig.DEFAULT_COOLDOWN_MINUTES
        const val DEFAULT_QUIET_HOURS_START = ProactivePluginConfig.DEFAULT_QUIET_HOURS_START
        const val DEFAULT_QUIET_HOURS_END = ProactivePluginConfig.DEFAULT_QUIET_HOURS_END

        private const val TAG = "Proactive"

        /**
         * 判断给定 24 小时制小时是否处于静默时段（纯函数，便于确定性单测）。
         *
         * 区间语义为 `[quietHoursStart, quietHoursEnd)`（闭-开）：
         * - 不跨天（start <= end）：如 9..17 → 9:00-16:59 静默，17:00 起不静默
         * - 跨天（start > end）：如 23..7 → 23:00-次日 6:59 静默，07:00 起不静默
         * - start == end：空区间，恒为 false（不静默）
         */
        fun isWithinQuietHours(hourOfDay: Int, quietHoursStart: Int, quietHoursEnd: Int): Boolean {
            require(hourOfDay in 0..23) { "hourOfDay 必须为 0..23，实际 $hourOfDay" }
            require(quietHoursStart in 0..23) { "quietHoursStart 必须为 0..23，实际 $quietHoursStart" }
            require(quietHoursEnd in 0..23) { "quietHoursEnd 必须为 0..23，实际 $quietHoursEnd" }
            return if (quietHoursStart <= quietHoursEnd) {
                hourOfDay in quietHoursStart until quietHoursEnd
            } else {
                hourOfDay >= quietHoursStart || hourOfDay < quietHoursEnd
            }
        }

        /** [isWithinQuietHours] 的毫秒时间戳重载：按本地时区取小时。 */
        fun isWithinQuietHours(nowMillis: Long, quietHoursStart: Int, quietHoursEnd: Int): Boolean {
            val hour = Calendar.getInstance().apply { timeInMillis = nowMillis }
                .get(Calendar.HOUR_OF_DAY)
            return isWithinQuietHours(hour, quietHoursStart, quietHoursEnd)
        }
    }
}
