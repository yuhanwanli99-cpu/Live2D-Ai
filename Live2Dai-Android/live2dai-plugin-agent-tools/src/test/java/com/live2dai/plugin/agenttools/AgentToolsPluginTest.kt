package com.live2dai.plugin.agenttools

import com.live2dai.pluginsdk.ChatHook
import com.live2dai.pluginsdk.PersonaSnapshot
import com.live2dai.pluginsdk.PluginHost
import com.live2dai.pluginsdk.ToolDefinition
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 骨架阶段的最小验证：只保证插件生命周期方法可安全调用。
 * `task-plugin-agent-tools` 落地真实功能时，必须补充针对
 * `requiresConfirmation = true` 的危险工具的确认流程测试（不可绕过确认直接执行）。
 */
class AgentToolsPluginTest {

    private class FakeHost : PluginHost {
        val hooks = mutableListOf<ChatHook>()
        val tools = mutableListOf<ToolDefinition>()
        override fun registerHook(hook: ChatHook) { hooks += hook }
        override fun registerTool(tool: ToolDefinition) { tools += tool }
        override fun sendAssistantMessage(text: String, emotion: String?) {}
        override fun getPersona(): PersonaSnapshot =
            PersonaSnapshot(name = "小喵", systemPrompt = "prompt", model = "model")
    }

    @Test
    fun onLoad_and_onUnload_doNotThrow() {
        val host = FakeHost()
        val plugin = AgentToolsPlugin(AgentToolsPluginConfig())

        plugin.onLoad(host)
        plugin.onUnload()

        assertTrue(host.hooks.isEmpty())
        assertTrue(host.tools.isEmpty())
    }
}
