package com.live2dai.plugin.proactive

import android.util.Log
import androidx.work.Data
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.ExistingWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.workDataOf
import com.live2dai.pluginsdk.Live2DaiPlugin
import com.live2dai.pluginsdk.PluginHost
import java.util.concurrent.TimeUnit

/**
 * 主动陪伴插件：让助手在用户长时间没理她时主动开口（docs/plan-task-plugin-proactive.md）。
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
        ProactiveHostBridge.memoriesProvider = config.memoriesProvider

        // 通知渠道（API 26+ 必须；幂等）——Android 13+ 权限由 core 请求，插件只负责发送时检查
        ProactiveNotifier.ensureChannel(config.context)

        host.registerHook(ProactiveTrackingHook(tracker))

        val inputData = buildInputData()

        // WorkManager 周期任务：间隔受系统 15 分钟最小硬约束，显式钳制避免非法值抛异常
        val intervalMinutes = config.workManagerIntervalMinutes.coerceAtLeast(MIN_PERIODIC_INTERVAL_MINUTES)
        if (intervalMinutes != config.workManagerIntervalMinutes) {
            logWarn(
                "workManagerIntervalMinutes=${config.workManagerIntervalMinutes} 低于系统最小间隔 " +
                    "$MIN_PERIODIC_INTERVAL_MINUTES 分钟，已钳制"
            )
        }
        val request = PeriodicWorkRequestBuilder<ProactiveCheckWorker>(intervalMinutes, TimeUnit.MINUTES)
            .setInputData(inputData)
            .build()
        WorkManager.getInstance(config.context)
            .enqueueUniquePeriodicWork(UNIQUE_WORK_NAME, ExistingPeriodicWorkPolicy.KEEP, request)

        // Debug 短周期开关（E2E 教训，plan §5.6.4）：额外排一次立即检查，
        // 「冷启动 → 空闲已超阈值」无需等一个完整周期即可触发，方便验收
        if (config.debugMode) {
            val immediateCheck = OneTimeWorkRequestBuilder<ProactiveCheckWorker>()
                .setInputData(inputData)
                .build()
            WorkManager.getInstance(config.context)
                .enqueueUniqueWork(
                    UNIQUE_IMMEDIATE_CHECK_NAME,
                    ExistingWorkPolicy.REPLACE,
                    immediateCheck,
                )
        }
        logInfo(
            "ProactivePlugin 加载完成：周期=${intervalMinutes}min 空闲阈值=" +
                "${config.idleThresholdMinutes}min 冷却=${config.cooldownMinutes}min debug=${config.debugMode}"
        )
    }

    override fun onUnload() {
        // 先清桥（Worker 可能仍在执行），再取消任务；顺序保证 Worker 拿不到已失效的 host
        ProactiveHostBridge.host = null
        ProactiveHostBridge.greetingLlmClient = null
        ProactiveHostBridge.memoriesProvider = null
        WorkManager.getInstance(config.context).cancelUniqueWork(UNIQUE_WORK_NAME)
        WorkManager.getInstance(config.context).cancelUniqueWork(UNIQUE_IMMEDIATE_CHECK_NAME)
        logInfo("ProactivePlugin 卸载完成（周期/立即检查任务已取消，桥已清空）")
    }

    /** 序列化进 WorkRequest inputData 的可序列化决策参数（周期任务与立即检查共用）。 */
    private fun buildInputData(): Data = workDataOf(
        ProactiveCheckWorker.KEY_IDLE_THRESHOLD_MINUTES to config.idleThresholdMinutes,
        ProactiveCheckWorker.KEY_COOLDOWN_MINUTES to config.cooldownMinutes,
        ProactiveCheckWorker.KEY_QUIET_HOURS_START to config.quietHoursStart,
        ProactiveCheckWorker.KEY_QUIET_HOURS_END to config.quietHoursEnd,
        ProactiveCheckWorker.KEY_FALLBACK_GREETINGS to config.fallbackGreetings.toTypedArray(),
    )

    /** 日志输出带单元测试防护（本地 JVM 测试无 Android 运行时，直接调用 Log 会抛异常）。 */
    private fun logInfo(message: String) {
        try {
            Log.i(TAG, message)
        } catch (_: Throwable) {
        }
    }

    private fun logWarn(message: String) {
        try {
            Log.w(TAG, message)
        } catch (_: Throwable) {
        }
    }

    companion object {
        private const val TAG = "Proactive"

        /** 唯一周期任务名（onLoad 入队 / onUnload 取消都用它）。 */
        const val UNIQUE_WORK_NAME = "live2dai_proactive_check"

        /** Debug 立即检查任务名（与周期任务独立，避免 REPLACE 误伤周期排期）。 */
        const val UNIQUE_IMMEDIATE_CHECK_NAME = "live2dai_proactive_immediate_check"

        /** WorkManager PeriodicWorkRequest 允许的最小间隔（Android 平台硬约束，不可更短）。 */
        const val MIN_PERIODIC_INTERVAL_MINUTES = 15L
    }
}
