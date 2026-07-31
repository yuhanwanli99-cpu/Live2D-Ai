package com.live2dai.plugin.proactive

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import androidx.work.WorkInfo
import androidx.work.WorkManager
import androidx.work.testing.WorkManagerTestInitHelper
import com.live2dai.pluginsdk.ChatContext
import com.live2dai.pluginsdk.ChatHook
import com.live2dai.pluginsdk.PersonaSnapshot
import com.live2dai.pluginsdk.PluginHost
import com.live2dai.pluginsdk.ToolDefinition
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * 插件注册流程测试：验证 [ProactivePlugin.onLoad] 确实注册了 tracking hook + WorkManager 周期任务，
 * [ProactivePlugin.onUnload] 取消任务并清空桥接（docs/plan-task-plugin-proactive.md §7）。
 */
@RunWith(RobolectricTestRunner::class)
class ProactivePluginTest {

    private lateinit var context: Context

    private class FakeHost : PluginHost {
        val hooks = mutableListOf<ChatHook>()
        val tools = mutableListOf<ToolDefinition>()
        override fun registerHook(hook: ChatHook) { hooks += hook }
        override fun registerTool(tool: ToolDefinition) { tools += tool }
        override fun sendAssistantMessage(text: String, emotion: String?) {}
        override fun getPersona(): PersonaSnapshot =
            PersonaSnapshot(name = "小喵", systemPrompt = "prompt", model = "model")
    }

    private class FakeGreetingClient : GreetingLlmClient {
        override suspend fun generateGreeting(personaName: String, systemPrompt: String, idleMinutes: Long): String? =
            "测试问候"
    }

    private fun config() = ProactivePluginConfig(
        context = context,
        greetingLlmClient = FakeGreetingClient(),
    )

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        // WorkManager 官方测试初始化：内存版 WorkManager（同步执行，入队/取消立即可查）
        WorkManagerTestInitHelper.initializeTestWorkManager(context)
        // 清掉上一个用例可能残留的同名任务，保证 KEEP 语义下断言确定性
        WorkManager.getInstance(context).cancelAllWork()
    }

    @After
    fun tearDown() {
        ProactiveHostBridge.host = null
        ProactiveHostBridge.greetingLlmClient = null
    }

    @Test
    fun onLoad_registersTrackingHook_andEnqueuesUniquePeriodicWork() {
        val host = FakeHost()
        val plugin = ProactivePlugin(config())

        plugin.onLoad(host)

        // 注册了一个 tracking hook，未注册 tool
        assertEquals(1, host.hooks.size)
        assertTrue("hook 应是 ProactiveTrackingHook", host.hooks.single() is ProactiveTrackingHook)
        assertTrue(host.tools.isEmpty())

        // 桥接已写入（Worker 执行时能拿到 host 与 LLM 客户端）
        assertTrue(ProactiveHostBridge.host === host)
        assertTrue(ProactiveHostBridge.greetingLlmClient is FakeGreetingClient)

        // WorkManager 唯一周期任务已入队（测试 WorkManager 用同步 Executor，get() 立即返回）
        val infos: List<WorkInfo> = WorkManager.getInstance(context)
            .getWorkInfosForUniqueWork(ProactivePlugin.UNIQUE_WORK_NAME)
            .get()
        assertEquals(1, infos.size)
        assertEquals(WorkInfo.State.ENQUEUED, infos.single().state)
    }

    @Test
    fun onLoad_hook_afterReceive_updatesLastInteraction() = runBlocking {
        val host = FakeHost()
        val plugin = ProactivePlugin(config())
        plugin.onLoad(host)

        host.hooks.single().afterReceive("喵~", null, ChatContext(recentMessages = emptyList(), personaName = "小喵"))

        // hook 内部用的是同一份 SharedPreferences，新开 tracker 读到的是同一份数据
        assertTrue("afterReceive 应刷新最后交互时间", LastInteractionTracker(context).lastInteractionAt() > 0L)
    }

    @Test
    fun onUnload_cancelsWork_andClearsBridge() {
        val host = FakeHost()
        val plugin = ProactivePlugin(config())
        plugin.onLoad(host)

        plugin.onUnload()

        val infos: List<WorkInfo> = WorkManager.getInstance(context)
            .getWorkInfosForUniqueWork(ProactivePlugin.UNIQUE_WORK_NAME)
            .get()
        assertEquals(1, infos.size)
        assertEquals("onUnload 应取消周期任务", WorkInfo.State.CANCELLED, infos.single().state)
        assertNull("onUnload 应清空桥接，防止泄漏 core 对象", ProactiveHostBridge.host)
        assertNull(ProactiveHostBridge.greetingLlmClient)
    }
}
