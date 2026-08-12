package com.live2dai.plugin.memory

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.live2dai.pluginsdk.ChatHook
import com.live2dai.pluginsdk.PersonaSnapshot
import com.live2dai.pluginsdk.PluginHost
import com.live2dai.pluginsdk.ToolDefinition
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * 插件注册流程测试：验证 [MemoryPlugin.onLoad] 向 host 注册 1 个 [ChatHook]
 * （骨架阶段的"零注册"断言已替换为真实注册数量断言，见 docs/plan-task-plugin-memory.md §5 #13）。
 */
@RunWith(RobolectricTestRunner::class)
class MemoryPluginTest {

    private class FakeHost : PluginHost {
        val hooks = mutableListOf<ChatHook>()
        val tools = mutableListOf<ToolDefinition>()
        override fun registerHook(hook: ChatHook) { hooks += hook }
        override fun registerTool(tool: ToolDefinition) { tools += tool }
        override fun sendAssistantMessage(text: String, emotion: String?) {}
        override fun getPersona(): PersonaSnapshot =
            PersonaSnapshot(name = "白", systemPrompt = "prompt", model = "model")
    }

    @Test
    fun onLoad_registersOneHook_andOnUnloadDoesNotThrow() {
        val host = FakeHost()
        val plugin = MemoryPlugin(
            MemoryPluginConfig(
                context = ApplicationProvider.getApplicationContext<Context>(),
                summaryLlmClient = SummaryLlmClient { _, _ -> null },
            )
        )

        plugin.onLoad(host)
        plugin.onUnload()

        assertEquals("应注册 1 个记忆 hook", 1, host.hooks.size)
        assertTrue("MVP 不注册工具", host.tools.isEmpty())
        assertTrue("注册的 hook 应为 MemoryRecallHook", host.hooks.single() is MemoryRecallHook)
    }

    @Test
    fun onUnload_withoutOnLoad_doesNotThrow() {
        val plugin = MemoryPlugin(
            MemoryPluginConfig(
                context = ApplicationProvider.getApplicationContext<Context>(),
                summaryLlmClient = SummaryLlmClient { _, _ -> null },
            )
        )

        plugin.onUnload() // 未加载时卸载不应抛异常
    }
}
