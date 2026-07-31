package com.live2dai.plugin.agenttools

import com.live2dai.pluginsdk.Live2DaiPlugin
import com.live2dai.pluginsdk.PluginHost

/**
 * Agent 工具执行插件——注册三个可操作手机的 [com.live2dai.pluginsdk.ToolDefinition]：
 *
 * | 工具 | 功能 | requiresConfirmation |
 * | ---- | ---- | -------------------- |
 * | `open_app` | 按应用名打开 App | false（仅切前台，无数据风险） |
 * | `send_notification_reply` | 快捷回复通知消息（真实发送） | **true**（必须经 core 确认 UI） |
 * | `set_alarm` | 设置系统闹钟 | false（系统页天然二次确认） |
 *
 * 确认机制是 core 的职责（`ConfirmationGate`，ChatService 统一拦截），插件只负责
 * 把 `requiresConfirmation=true` 标记在 [com.live2dai.pluginsdk.ToolDefinition] 上
 * （硬约束：标记缺失则确认流程不会触发，见 `docs/plan-task-plugin-agent-tools.md` §2.1）。
 */
class AgentToolsPlugin(private val config: AgentToolsPluginConfig) : Live2DaiPlugin {
    override val id: String = "agent-tools"
    override val version: String = "1.0.0"

    override fun onLoad(host: PluginHost) {
        host.registerTool(OpenAppTool(config.context))
        host.registerTool(SendNotificationReplyTool(config.notificationReplyBridge))
        host.registerTool(SetAlarmTool(config.context))
    }

    override fun onUnload() {
        // 三个工具均为无状态实现，无资源需要释放
    }
}
