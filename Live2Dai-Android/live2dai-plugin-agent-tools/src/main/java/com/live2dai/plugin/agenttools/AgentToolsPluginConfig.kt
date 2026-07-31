package com.live2dai.plugin.agenttools

import android.content.Context

/**
 * Agent 工具执行插件构造参数（从 core 注入）。
 *
 * 相比 memory/proactive，本插件不需要注入 LLM 客户端——它纯粹是"执行动作"，不生成文本。
 * 需要注入的是 Android 运行环境依赖：
 * - [context]：PackageManager / Intent 都需要 Context（由 core 传 applicationContext）
 * - [notificationReplyBridge]：通知快捷回复能力（core 的 NotificationListenerService 桥接，
 *   见 `docs/plan-task-plugin-agent-tools.md` §4.3）。为 null 时
 *   [SendNotificationReplyTool] 仍会注册，但执行时返回明确错误（优雅降级，R2）。
 */
data class AgentToolsPluginConfig(
    /** Android Context（applicationContext），供 open_app / set_alarm 使用 */
    val context: Context,
    /** 通知快捷回复桥接实现（core 注入）；null = 快捷回复功能未启用 */
    val notificationReplyBridge: NotificationReplyBridge? = null,
)
