package com.live2dai.android

import com.live2dai.pluginsdk.ChatHook
import com.live2dai.pluginsdk.Live2DaiPlugin
import com.live2dai.pluginsdk.PersonaSnapshot
import com.live2dai.pluginsdk.PluginHost
import com.live2dai.pluginsdk.ToolDefinition
import java.util.concurrent.CopyOnWriteArrayList

/**
 * core 端唯一的 [PluginHost] 实现，同时充当插件注册表。
 *
 * 生命周期：由 `Application.onCreate()`（或 `MainActivity` 首次启动时）创建单例，
 * 调用 [loadPlugins] 装载所有插件，随后 [ChatService] 通过构造函数持有它，
 * 在对话主循环的既定时点调用 [hooks] / [tools]。
 *
 * 当前插件以「编译期 Gradle 依赖」的形式接入（见 docs/plan-task-plugin-architecture.md
 * 第 3.1 节），因此 [loadPlugins] 的插件列表在此硬编码传入；后续如果插件数量增多、
 * 需要用户在运行时自由启停，可以把这里换成基于 ServiceLoader / 反射扫描的动态发现，
 * 接口契约（[Live2DaiPlugin]/[PluginHost]）不需要变。
 */
class PluginManager : PluginHost {

    private val loadedPlugins = mutableListOf<Live2DaiPlugin>()

    // 使用线程安全容器：插件可能在非主线程的初始化协程里调用 registerHook/registerTool
    private val _hooks = CopyOnWriteArrayList<ChatHook>()
    private val _tools = CopyOnWriteArrayList<ToolDefinition>()

    /** 只读快照，供 [ChatService] 在对话主循环里按注册顺序遍历。 */
    val hooks: List<ChatHook> get() = _hooks
    val tools: List<ToolDefinition> get() = _tools

    /** 供主动陪伴类插件调用；由宿主（如 MainActivity/FloatingWindowService）注入实际实现。 */
    var onAssistantMessage: ((text: String, emotion: String?) -> Unit)? = null

    /**
     * 装载插件列表。
     *
     * TODO(task-extract-vision 等 Wave 1/2 任务)：随着插件迁出为独立仓库依赖，
     * 在此处把对应插件实例加入 [plugins] 参数即可，core 主循环无需改动。
     */
    fun loadPlugins(plugins: List<Live2DaiPlugin>) {
        plugins.forEach { plugin ->
            plugin.onLoad(this)
            loadedPlugins += plugin
        }
    }

    fun unloadAll() {
        loadedPlugins.forEach { it.onUnload() }
        loadedPlugins.clear()
        _hooks.clear()
        _tools.clear()
    }

    override fun registerHook(hook: ChatHook) {
        _hooks += hook
    }

    override fun registerTool(tool: ToolDefinition) {
        // 同名工具后注册者覆盖前者，避免多个插件注册同名工具时静默重复
        _tools.removeAll { it.name == tool.name }
        _tools += tool
    }

    override fun sendAssistantMessage(text: String, emotion: String?) {
        onAssistantMessage?.invoke(text, emotion)
    }

    override fun getPersona(): PersonaSnapshot = PersonaSnapshot(
        name = PersonaConfig.getPersonaName(),
        systemPrompt = PersonaConfig.getSystemPrompt(),
        model = PersonaConfig.getModel(),
    )
}
