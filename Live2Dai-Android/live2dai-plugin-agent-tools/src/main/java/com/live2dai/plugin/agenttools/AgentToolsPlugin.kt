package com.live2dai.plugin.agenttools

import com.live2dai.pluginsdk.Live2DaiPlugin
import com.live2dai.pluginsdk.PluginHost

/**
 * Agent 工具执行插件骨架（对应 `docs/plan-task-plugin-architecture.md` 的 `live2dai-plugin-agent-tools`）。
 *
 * 目标能力（`task-plugin-agent-tools` 实现，本骨架不涉及）：
 * - 注册可操作手机的 `ToolDefinition`（如打开 App、点击坐标）
 * - 危险操作 `requiresConfirmation = true`，执行前必须经用户确认 UI（硬约束，不可绕过）
 *
 * 骨架阶段 `onLoad` 为 no-op，不注册任何工具，因此当前**不接入** core
 * （`MainActivity.kt` 尚未装载本插件），不产生任何设备操作能力。
 */
class AgentToolsPlugin(private val config: AgentToolsPluginConfig) : Live2DaiPlugin {
    override val id: String = "agent-tools"
    override val version: String = "0.1.0-skeleton"

    override fun onLoad(host: PluginHost) {
        // TODO(task-plugin-agent-tools): host.registerTool(OpenAppTool(...))
        // TODO(task-plugin-agent-tools): 危险工具必须设置 requiresConfirmation = true，
        //   并确保 core 侧在 execute() 前弹出确认 UI（详见架构设计 §3.3 中间件拦截点）
    }

    override fun onUnload() {
        // TODO(task-plugin-agent-tools): 释放资源（如有）
    }
}
