package com.live2dai.plugin.memory

/**
 * 插件构造参数（从 core 注入）。骨架阶段先留空占位，
 * `task-plugin-memory` 落地时预计需要补充：
 * - 本地存储句柄（如 Room `MemoryDatabase` 或数据库文件路径）
 * - 摘要触发策略参数（如每 N 轮生成一次摘要）
 */
data class MemoryPluginConfig(
    val placeholder: Unit = Unit
)
