package com.live2dai.plugin.proactive

import com.live2dai.pluginsdk.Live2DaiPlugin
import com.live2dai.pluginsdk.PluginHost

/**
 * 主动陪伴插件骨架（对应 `docs/plan-task-plugin-architecture.md` 的 `live2dai-plugin-proactive`）。
 *
 * 目标能力（`task-plugin-proactive` 实现，本骨架不涉及）：
 * - 用 AndroidX `WorkManager` 注册定时/事件触发器（见 `docs/plan-task-plugin-architecture.md` §6）
 * - 触发时调用 `PluginHost.sendAssistantMessage()` 让猫娘主动说话
 *
 * 骨架阶段 `onLoad` 为 no-op，不注册任何调度任务，因此当前**不接入** core
 * （`MainActivity.kt` 尚未装载本插件），不影响现有对话行为、不产生任何后台任务。
 */
class ProactivePlugin(private val config: ProactivePluginConfig) : Live2DaiPlugin {
    override val id: String = "proactive"
    override val version: String = "0.1.0-skeleton"

    override fun onLoad(host: PluginHost) {
        // TODO(task-plugin-proactive): 注册 WorkManager 周期任务，触发时调用
        //   host.sendAssistantMessage(text = "...", emotion = "...")
    }

    override fun onUnload() {
        // TODO(task-plugin-proactive): 取消已注册的 WorkManager 任务
    }
}
