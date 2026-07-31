package com.live2dai.plugin.memory

import android.content.Context

/**
 * 插件构造参数（从 core 注入），见 docs/plan-task-plugin-memory.md §3.2。
 */
data class MemoryPluginConfig(
    /** 用于构建 Room database（core 传入 applicationContext） */
    val context: Context,
    /** LLM 摘要能力（core 注入实现，插件不自己接 LLM 客户端） */
    val summaryLlmClient: SummaryLlmClient,
    /** 每 N 轮对话触发一次摘要生成（默认 10） */
    val summaryTriggerEveryNTurns: Int = 10,
    /** Room 数据库文件名 */
    val databaseName: String = "live2dai_memory.db",
)
