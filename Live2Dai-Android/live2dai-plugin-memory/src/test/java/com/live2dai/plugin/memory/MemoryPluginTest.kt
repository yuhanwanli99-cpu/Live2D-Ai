package com.live2dai.plugin.memory

import com.live2dai.pluginsdk.ChatHook
import com.live2dai.pluginsdk.PersonaSnapshot
import com.live2dai.pluginsdk.PluginHost
import com.live2dai.pluginsdk.ToolDefinition
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 骨架阶段的最小验证：只保证插件生命周期方法可安全调用。
 * `task-plugin-memory` 落地真实功能时，应在此基础上补充 hook/tool 注册数量、
 * 记忆检索/写入行为的断言（参考 `live2dai-plugin-vision` 的 `VisionPluginTest`）。
 */
class MemoryPluginTest {

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
        val plugin = MemoryPlugin(MemoryPluginConfig())

        plugin.onLoad(host)
        plugin.onUnload()

        // 骨架阶段 onLoad 不注册任何 hook/tool；功能任务落地后应改为断言具体数量
        assertTrue(host.hooks.isEmpty())
        assertTrue(host.tools.isEmpty())
    }
}
