package com.live2dai.plugin.memory

import android.util.Log
import com.live2dai.pluginsdk.Live2DaiPlugin
import com.live2dai.pluginsdk.PluginHost

/**
 * 记忆插件：向 core 注册 [MemoryRecallHook]，提供长期记忆能力
 * （Room 事实存储 + 关键词检索注入），见 docs/plan-task-plugin-memory.md。
 *
 * 两层记忆分工（文档 §1.1）：
 * - 短期记忆（近期对话）：由 core 的 `ChatContext.recentMessages` 只读快照提供，插件不重复存储
 * - 长期记忆（事实摘要）：本插件负责——beforeSend 检索注入，afterReceive 周期性摘要写入
 *
 * 本 MVP 不注册 ToolDefinition（不做"主动查询记忆"工具），beforeSend 自动注入已覆盖主要场景。
 */
class MemoryPlugin(private val config: MemoryPluginConfig) : Live2DaiPlugin {
    override val id: String = "memory"
    override val version: String = "1.0.0"

    private var database: MemoryDatabase? = null

    override fun onLoad(host: PluginHost) {
        val db = MemoryDatabase.build(config.context, config.databaseName)
        database = db
        host.registerHook(
            MemoryRecallHook(
                dao = db.memoryDao(),
                summaryLlmClient = config.summaryLlmClient,
                triggerEveryNTurns = config.summaryTriggerEveryNTurns,
                maxFacts = config.maxFacts,
            )
        )
        logInfo(
            "MemoryPlugin 加载完成，hook 已注册 " +
                "(db=${config.databaseName}, triggerEvery=${config.summaryTriggerEveryNTurns}, maxFacts=${config.maxFacts})"
        )
    }

    override fun onUnload() {
        // 关闭 Room database 连接，释放文件句柄
        database?.close()
        database = null
        logInfo("MemoryPlugin 卸载完成（Room database 已关闭）")
    }

    /**
     * 日志输出（带单元测试防护）：本地 JVM 单测（无 Android 运行时）中
     * android.util.Log 未被 mock，直接调用会抛 "Method i in android.util.Log not mocked"。
     */
    private fun logInfo(message: String) {
        try {
            Log.i("Memory", message)
        } catch (_: Throwable) {
            // 单元测试环境：静默忽略
        }
    }
}
