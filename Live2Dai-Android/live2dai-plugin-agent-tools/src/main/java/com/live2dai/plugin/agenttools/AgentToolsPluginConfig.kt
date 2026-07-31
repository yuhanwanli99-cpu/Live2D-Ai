package com.live2dai.plugin.agenttools

/**
 * 插件构造参数（从 core 注入）。骨架阶段先留空占位，
 * `task-plugin-agent-tools` 落地时预计需要补充：
 * - 用户确认 UI 回调接口（如 `suspend fun confirm(actionDescription: String): Boolean`）
 * - 允许操作的白名单（App 包名/坐标区域等，出于安全考虑）
 */
data class AgentToolsPluginConfig(
    val placeholder: Unit = Unit
)
