package com.live2dai.plugin.agenttools

import androidx.test.core.app.ApplicationProvider
import com.live2dai.pluginsdk.ChatHook
import com.live2dai.pluginsdk.PersonaSnapshot
import com.live2dai.pluginsdk.PluginHost
import com.live2dai.pluginsdk.ToolDefinition
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * 插件注册流程测试：验证 [AgentToolsPlugin.onLoad] 向 host 注册三个工具，
 * 且危险工具（send_notification_reply）正确标记 `requiresConfirmation = true`
 * （确认拦截是 core 的职责，插件侧必须诚实上报标记，否则确认流程不会触发）。
 */
@RunWith(RobolectricTestRunner::class)
class AgentToolsPluginTest {

    private class FakeHost : PluginHost {
        val hooks = mutableListOf<ChatHook>()
        val tools = mutableListOf<ToolDefinition>()
        override fun registerHook(hook: ChatHook) { hooks += hook }
        override fun registerTool(tool: ToolDefinition) { tools += tool }
        override fun sendAssistantMessage(text: String, emotion: String?) {}
        override fun getPersona(): PersonaSnapshot =
            PersonaSnapshot(name = "白", systemPrompt = "prompt", model = "model")
    }

    private val testContext get() = ApplicationProvider.getApplicationContext<android.content.Context>()

    private fun plugin(
        notificationReplyBridge: NotificationReplyBridge? = NotificationReplyBridge { _, _ -> """{"result":"ok"}""" },
    ) = AgentToolsPlugin(
        AgentToolsPluginConfig(
            context = testContext,
            notificationReplyBridge = notificationReplyBridge,
        )
    )

    @Test
    fun onLoad_registersThreeTools() {
        val host = FakeHost()

        plugin().onLoad(host)

        assertEquals(3, host.tools.size)
        assertEquals(
            setOf("open_app", "send_notification_reply", "set_alarm"),
            host.tools.map { it.name }.toSet()
        )
        assertTrue(host.hooks.isEmpty())  // 本插件不注册 hook
    }

    @Test
    fun onLoad_dangerousToolIsFlaggedForConfirmation() {
        val host = FakeHost()

        plugin().onLoad(host)

        val replyTool = host.tools.first { it.name == "send_notification_reply" }
        assertTrue("快捷回复真实发送消息，必须 requiresConfirmation=true", replyTool.requiresConfirmation)
        assertFalse(host.tools.first { it.name == "open_app" }.requiresConfirmation)
        assertFalse(host.tools.first { it.name == "set_alarm" }.requiresConfirmation)
    }

    @Test
    fun onLoad_registersAllToolsEvenWithoutBridge() {
        // 未注入桥接：三个工具仍全部注册（send_notification_reply 执行时优雅降级，R2）
        val host = FakeHost()

        plugin(notificationReplyBridge = null).onLoad(host)

        assertEquals(3, host.tools.size)
        assertTrue(host.tools.any { it.name == "send_notification_reply" })
    }

    @Test
    fun onUnload_doesNotThrow() {
        val host = FakeHost()
        val p = plugin()
        p.onLoad(host)

        p.onUnload()  // 不应抛异常（三个工具均无状态）
    }
}
