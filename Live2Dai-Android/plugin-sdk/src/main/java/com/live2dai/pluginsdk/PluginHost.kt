package com.live2dai.pluginsdk

/**
 * core 暴露给插件的能力集合（宿主接口）。
 *
 * 插件只能通过这个接口和 core 交互——不允许反射拿 ChatService 内部状态，
 * 这是保持 core 精简、插件解耦的硬约束。
 */
interface PluginHost {

    /** 注册一个对话生命周期钩子，参见 [ChatHook]。同一插件可注册多个 hook。 */
    fun registerHook(hook: ChatHook)

    /** 注册一个可被 LLM 调用的工具，参见 [ToolDefinition]。 */
    fun registerTool(tool: ToolDefinition)

    /**
     * 让猫娘主动说一句话（不经过用户输入触发）。
     * 用于 `live2dai-plugin-proactive` 这类主动陪伴插件。
     *
     * @param text 要说的话（会经过 TTS 播放）
     * @param emotion 可选的表情标签（如 "joy"/"anger"），为空则保持当前表情
     */
    fun sendAssistantMessage(text: String, emotion: String? = null)

    /** 读取当前人设配置（只读），插件可用它做个性化（如记忆摘要要贴合人设语气）。 */
    fun getPersona(): PersonaSnapshot
}

/**
 * [PluginHost.getPersona] 返回的只读人设快照。
 * 字段与 `shared/persona.yaml` 对应，避免插件直接依赖 core 的 `PersonaConfig` 实现细节。
 */
data class PersonaSnapshot(
    val name: String,
    val systemPrompt: String,
    val model: String,
)
