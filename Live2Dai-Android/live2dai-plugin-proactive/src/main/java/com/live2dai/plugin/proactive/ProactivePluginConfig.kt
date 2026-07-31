package com.live2dai.plugin.proactive

/**
 * 插件构造参数（从 core 注入）。骨架阶段先留空占位，
 * `task-plugin-proactive` 落地时预计需要补充：
 * - 触发间隔/时段配置（如"每 2 小时一次，仅 9:00-23:00"）
 * - 触发内容来源（如接入记忆插件摘要、社交媒体热搜等，视 Wave 3 规划）
 */
data class ProactivePluginConfig(
    val placeholder: Unit = Unit
)
