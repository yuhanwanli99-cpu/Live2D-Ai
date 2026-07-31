package com.live2dai.plugin.proactive

import com.live2dai.pluginsdk.ChatHook
import com.live2dai.pluginsdk.PersonaSnapshot
import com.live2dai.pluginsdk.PluginHost
import com.live2dai.pluginsdk.ToolDefinition
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 骨架阶段的最小验证：只保证插件生命周期方法可安全调用，且不会意外调用
 * `sendAssistantMessage`（骨架不应产生任何主动消息）。
 * `task-plugin-proactive` 落地真实功能时应在此基础上补充 WorkManager 调度验证。
 */
class ProactivePluginTest {

    private class FakeHost : PluginHost {
        val hooks = mutableListOf<ChatHook>()
        val tools = mutableListOf<ToolDefinition>()
        var sentCount = 0
        override fun registerHook(hook: ChatHook) { hooks += hook }
        override fun registerTool(tool: ToolDefinition) { tools += tool }
        override fun sendAssistantMessage(text: String, emotion: String?) { sentCount++ }
        override fun getPersona(): PersonaSnapshot =
            PersonaSnapshot(name = "小喵", systemPrompt = "prompt", model = "model")
    }

    @Test
    fun onLoad_and_onUnload_doNotThrow_andDoNotSendMessage() {
        val host = FakeHost()
        val plugin = ProactivePlugin(ProactivePluginConfig())

        plugin.onLoad(host)
        plugin.onUnload()

        assertTrue(host.hooks.isEmpty())
        assertTrue(host.tools.isEmpty())
        assertTrue("骨架阶段不应主动发消息", host.sentCount == 0)
    }
}
