package com.live2dai.plugin.memory

import com.live2dai.pluginsdk.Live2DaiPlugin
import com.live2dai.pluginsdk.PluginHost

/**
 * 记忆插件骨架（对应 `docs/plan-task-plugin-architecture.md` 的 `live2dai-plugin-memory`）。
 *
 * 目标能力（`task-plugin-memory` 实现，本骨架不涉及）：
 * - `beforeSend`：从本地记忆存储中检索与当前对话相关的摘要，注入为附加上下文
 * - `afterReceive`：把本轮对话写入近期记忆，定期生成摘要写入事实记忆
 *
 * 骨架阶段 `onLoad` 为 no-op，不注册任何 hook/tool，因此当前**不接入** core
 * （`MainActivity.kt` 尚未装载本插件），不影响现有对话行为。
 */
class MemoryPlugin(private val config: MemoryPluginConfig) : Live2DaiPlugin {
    override val id: String = "memory"
    override val version: String = "0.1.0-skeleton"

    override fun onLoad(host: PluginHost) {
        // TODO(task-plugin-memory): host.registerHook(MemoryRecallHook(...))
        // TODO(task-plugin-memory): 视需要 host.registerTool(...)（如"记住这件事"工具）
    }

    override fun onUnload() {
        // TODO(task-plugin-memory): 关闭本地存储连接（如 Room database）
    }
}
