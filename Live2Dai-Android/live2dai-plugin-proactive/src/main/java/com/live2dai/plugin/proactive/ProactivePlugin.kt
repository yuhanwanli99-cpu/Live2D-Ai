package com.live2dai.plugin.proactive

import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.workDataOf
import com.live2dai.pluginsdk.Live2DaiPlugin
import com.live2dai.pluginsdk.PluginHost
import java.util.concurrent.TimeUnit

/**
 * 主动陪伴插件：让猫娘在用户长时间没理她时主动开口（docs/plan-task-plugin-proactive.md）。
 *
 * 能力：
 * - 定时问候：距上次对话超过 [ProactivePluginConfig.idleThresholdMinutes] 分钟 → 触发
 * - 时段问候：不在 [ProactivePluginConfig.quietHoursStart]~[quietHoursEnd] 静默时段内
 * - 触发文案：优先 LLM 即时生成（结合人设 + 当前时间），失败回退静态文案池
 *
 * [onLoad]：注册 [ProactiveTrackingHook]（每轮真实对话刷新最后交互时间）
 * + 排期 WorkManager 周期任务（[ProactiveCheckWorker]，15 分钟粒度——PeriodicWorkRequest 系统最小间隔）。
 * [onUnload]：取消周期任务并清空 [ProactiveHostBridge]，不留后台任务。
 */
class ProactivePlugin(private val config: ProactivePluginConfig) : Live2DaiPlugin {
    override val id: String = "proactive"
    override val version: String = "1.0.0"

    private lateinit var tracker: LastInteractionTracker

    override fun onLoad(host: PluginHost) {
        tracker = LastInteractionTracker(config.context)

        // R1 架构补丁：Worker 由系统调度创建拿不到 host 引用，通过进程内单例桥暴露
        ProactiveHostBridge.host = host
        ProactiveHostBridge.greetingLlmClient = config.greetingLlmClient

        host.registerHook(ProactiveTrackingHook(tracker))

        val request = PeriodicWorkRequestBuilder<ProactiveCheckWorker>(PERIODIC_INTERVAL_MINUTES, TimeUnit.MINUTES)
            .setInputData(
                workDataOf(
                    ProactiveCheckWorker.KEY_IDLE_THRESHOLD_MINUTES to config.idleThresholdMinutes,
                    ProactiveCheckWorker.KEY_COOLDOWN_MINUTES to config.cooldownMinutes,
                    ProactiveCheckWorker.KEY_QUIET_HOURS_START to config.quietHoursStart,
                    ProactiveCheckWorker.KEY_QUIET_HOURS_END to config.quietHoursEnd,
                    ProactiveCheckWorker.KEY_FALLBACK_GREETINGS to config.fallbackGreetings.toTypedArray(),
                )
            )
            .build()

        WorkManager.getInstance(config.context)
            .enqueueUniquePeriodicWork(UNIQUE_WORK_NAME, ExistingPeriodicWorkPolicy.KEEP, request)
    }

    override fun onUnload() {
        // 先清桥（Worker 可能仍在执行），再取消任务；顺序保证 Worker 拿不到已失效的 host
        ProactiveHostBridge.host = null
        ProactiveHostBridge.greetingLlmClient = null
        WorkManager.getInstance(config.context).cancelUniqueWork(UNIQUE_WORK_NAME)
    }

    companion object {
        /** 唯一周期任务名（onLoad 入队 / onUnload 取消都用它）。 */
        const val UNIQUE_WORK_NAME = "live2dai_proactive_check"

        /** WorkManager PeriodicWorkRequest 允许的最小间隔（Android 平台硬约束，不可更短）。 */
        const val PERIODIC_INTERVAL_MINUTES = 15L
    }
}
