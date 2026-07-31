package com.live2dai.android

import android.app.NotificationManager
import android.app.PendingIntent
import android.app.RemoteInput
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.service.notification.NotificationListenerService
import android.service.notification.StatusBarNotification
import androidx.core.app.NotificationManagerCompat
import com.live2dai.plugin.agenttools.NotificationReplyBridge
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * 通知监听服务——快捷回复的底层能力（core 持有，插件只定义 [NotificationReplyBridge] 接口）。
 *
 * 快捷回复依赖 `NotificationListenerService`：需要用户在系统设置里单独授权"通知使用权"，
 * 且必须是一个真正注册的 Service 组件（`BIND_NOTIFICATION_LISTENER_SERVICE`）。
 * 这些是插件模块无法自行声明的（骨架约定 `AndroidManifest.xml` 保持空清单），由 core 持有更合理
 * （见 `docs/plan-task-plugin-agent-tools.md` §4.3）。
 *
 * 数据流：LLM 调 `send_notification_reply` → 确认 → [AndroidNotificationReplyBridge]
 * → 本服务取最近通知 → 找到带快捷回复（RemoteInput）动作的通知 → 用官方
 * [RemoteInput.addResultsToIntent] + [PendingIntent.send] 机制真实发送回复。
 */
class NotificationReplyListenerService : NotificationListenerService() {

    override fun onListenerConnected() {
        activeInstance = this
    }

    override fun onListenerDisconnected() {
        if (activeInstance === this) activeInstance = null
    }

    override fun onDestroy() {
        if (activeInstance === this) activeInstance = null
        super.onDestroy()
    }

    override fun onNotificationPosted(sbn: StatusBarNotification?) { /* 快捷回复按需读取，无需缓存 */ }
    override fun onNotificationRemoved(sbn: StatusBarNotification?) { /* 同上 */ }

    /**
     * 查找与 [appName] 匹配的最近一条带快捷回复入口的通知并真实发送 [replyText]。
     * 所有失败路径都以 JSON 错误字符串返回（不抛异常），便于 LLM 理解并引导用户。
     */
    internal fun replyToLatest(appName: String, replyText: String): String {
        val notifications = try {
            activeNotifications
        } catch (_: SecurityException) {
            return """{"error":"无法读取通知：请先开启通知访问权限"}"""
        } ?: emptyArray()

        val sbn = notifications.firstOrNull { matchesAppLabel(it, appName) }
            ?: return """{"error":"未找到「$appName」的可回复通知"}"""

        val action = sbn.notification.actions?.firstOrNull { !it.remoteInputs.isNullOrEmpty() }
            ?: return """{"error":"「$appName」的最近通知没有快捷回复入口"}"""

        // 官方快捷回复机制：把回复文本写入 action 的 RemoteInput 结果，
        // 通过 action 的 PendingIntent 发送给目标应用（与系统通知助手同机制，非灰手段）
        val remoteInputs = action.remoteInputs!!
        val primary = remoteInputs[0]
        val results = Bundle().apply { putCharSequence(primary.resultKey, replyText) }
        val sendIntent = Intent().apply { addFlags(Intent.FLAG_RECEIVER_FOREGROUND) }
        RemoteInput.addResultsToIntent(remoteInputs, sendIntent, results)

        return try {
            action.actionIntent.send(this, 0, sendIntent)
            """{"result":"已通过快捷回复发送给 $appName"}"""
        } catch (_: PendingIntent.CanceledException) {
            """{"error":"快捷回复发送失败：目标应用的回复入口已失效"}"""
        } catch (e: Exception) {
            """{"error":"快捷回复发送失败：${e.message}"}"""
        }
    }

    /** 按应用显示名称（或包名兜底）匹配通知，忽略大小写。 */
    private fun matchesAppLabel(sbn: StatusBarNotification, appName: String): Boolean {
        val target = appName.trim()
        if (target.isEmpty()) return false
        val label = try {
            packageManager.getApplicationLabel(
                packageManager.getApplicationInfo(sbn.packageName, 0)
            )?.toString()
        } catch (_: Exception) {
            null
        }
        if (label != null && (label.equals(target, ignoreCase = true) || label.contains(target, ignoreCase = true))) {
            return true
        }
        return sbn.packageName.equals(target, ignoreCase = true)
    }

    companion object {
        /** 当前已连接的系统通知监听实例；未授权/未绑定时为 null。 */
        @Volatile
        private var activeInstance: NotificationReplyListenerService? = null

        /** 供 [AndroidNotificationReplyBridge] 取当前实例。 */
        internal fun current(): NotificationReplyListenerService? = activeInstance
    }
}

/**
 * [NotificationReplyBridge] 的 core 实现——注入给 `live2dai-plugin-agent-tools`。
 *
 * 权限状态检测（R2 优雅降级）：
 * - 未授权"通知使用权" → 返回明确错误（LLM 可据此引导用户在系统设置中开启）；
 * - 已授权但服务未连接 → 返回明确错误；
 * - 以上通过 → 委托 [NotificationReplyListenerService.replyToLatest] 真实发送。
 */
class AndroidNotificationReplyBridge(private val context: Context) : NotificationReplyBridge {

    override suspend fun reply(appName: String, replyText: String): String =
        withContext(Dispatchers.IO) {
            if (!hasNotificationListenerAccess(context)) {
                return@withContext """{"error":"请先在系统设置中开启通知访问权限（通知使用权）"}"""
            }
            val service = NotificationReplyListenerService.current()
                ?: return@withContext """{"error":"通知监听服务未连接，请检查通知使用权设置"}"""
            service.replyToLatest(appName, replyText)
        }
}

/**
 * 检测当前 App 是否已获得"通知使用权"（NotificationListenerService 授权）。
 * API 30+ 用 [NotificationManager.isNotificationListenerAccessGranted]，
 * 旧版本用 [NotificationManagerCompat.getEnabledListenerPackages]（已废弃但 26-29 可用）。
 */
@Suppress("DEPRECATION")
internal fun hasNotificationListenerAccess(context: Context): Boolean {
    return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
        val nm = context.getSystemService(NotificationManager::class.java)
        nm.isNotificationListenerAccessGranted(
            ComponentName(context, NotificationReplyListenerService::class.java)
        )
    } else {
        NotificationManagerCompat.getEnabledListenerPackages(context).contains(context.packageName)
    }
}
