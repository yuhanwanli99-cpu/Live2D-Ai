package com.live2dai.plugin.proactive

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat

/**
 * 主动陪伴通知（docs/plan-v12-plan.md §5.6 + 任务要求「通知栏通知 + 点击回 app」）。
 *
 * 主动问候发送成功后弹一条通知栏消息：App 在后台/息屏时用户也能看到助手的主动陪伴；
 * 点击通知回到 App 主界面（用 launcher intent，不依赖 core 的具体 Activity 类，插件保持解耦）。
 *
 * 容错约定（plan §8.2/§8.3）：通知链路任何失败都**静默**——Android 13+ 未授予
 * POST_NOTIFICATIONS、系统通知被禁用、PendingIntent 构造失败等一律不抛异常、
 * 不打断对话链路（主动陪伴只是锦上添花，通知发不出去不影响说话）。
 * 日志统一 TAG "Proactive"，Log.i 级别（国产 ROM 抑制 Log.d 教训）。
 */
object ProactiveNotifier {

    private const val TAG = "Proactive"

    /** 通知渠道 ID（API 26+ 必须；minSdk 26，恒可用） */
    private const val CHANNEL_ID = "live2dai_proactive"

    private const val CHANNEL_NAME = "主动陪伴"

    /** 主动陪伴通知固定 ID（同一时刻至多一条，新通知覆盖旧的） */
    private const val NOTIFICATION_ID = 1001

    /** 创建通知渠道（幂等：重复调用安全）。[ProactivePlugin.onLoad] 时调用一次。 */
    fun ensureChannel(context: Context) {
        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                val manager = context.getSystemService(NotificationManager::class.java) ?: return
                manager.createNotificationChannel(
                    NotificationChannel(
                        CHANNEL_ID,
                        CHANNEL_NAME,
                        NotificationManager.IMPORTANCE_DEFAULT,
                    )
                )
            }
        } catch (e: Exception) {
            logWarn("创建通知渠道失败（静默）: ${e.message}")
        }
    }

    /**
     * 发送主动陪伴通知（静默容错，永不抛异常）。
     *
     * @param title 通知标题（如人设名）
     * @param text 主动问候/话题正文
     */
    fun notify(context: Context, title: String, text: String) {
        try {
            // Android 13+（API 33）：POST_NOTIFICATIONS 是运行时权限，未授予时静默跳过
            if (Build.VERSION.SDK_INT >= 33 &&
                ContextCompat.checkSelfPermission(
                    context, Manifest.permission.POST_NOTIFICATIONS
                ) != PackageManager.PERMISSION_GRANTED
            ) {
                logInfo("无 POST_NOTIFICATIONS 权限，跳过主动陪伴通知（不影响 TTS 播报）")
                return
            }
            val manager = NotificationManagerCompat.from(context)
            if (!manager.areNotificationsEnabled()) {
                logInfo("系统通知已禁用，跳过主动陪伴通知")
                return
            }

            // 点击回 app：launcher intent（不依赖具体 Activity 类，插件与 core 解耦）
            val launchIntent = context.packageManager
                .getLaunchIntentForPackage(context.packageName)
            val contentIntent = PendingIntent.getActivity(
                context,
                0,
                launchIntent,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )

            val notification = NotificationCompat.Builder(context, CHANNEL_ID)
                .setSmallIcon(R.drawable.ic_proactive_notification)
                .setContentTitle(title.ifBlank { "Live2Dai" })
                .setContentText(text)
                .setContentIntent(contentIntent)
                .setAutoCancel(true)
                .setPriority(NotificationCompat.PRIORITY_DEFAULT)
                .build()
            manager.notify(NOTIFICATION_ID, notification)
            logInfo("已发送主动陪伴通知：${text.take(30)}")
        } catch (e: Exception) {
            logWarn("发送主动陪伴通知失败（静默）: ${e.message}")
        }
    }

    private fun logInfo(message: String) {
        try {
            Log.i(TAG, message)
        } catch (_: Throwable) {
            // 单元测试环境（无 Android 运行时）：静默忽略
        }
    }

    private fun logWarn(message: String) {
        try {
            Log.w(TAG, message)
        } catch (_: Throwable) {
        }
    }
}
