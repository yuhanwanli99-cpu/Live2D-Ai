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
            PersonaSnapshot(name = "凛", systemPrompt = "prompt", model = "model")
    }

    private class FakeGreetingClient : GreetingLlmClient {
        override suspend fun generateGreeting(
            personaName: String,
            systemPrompt: String,
            idleMinutes: Long,
            memories: List<String>?,
        ): String? = "测试问候"
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
        ProactiveHostBridge.memoriesProvider = null
    }

    /**
     * 周期任务在测试 WorkManager（SynchronousExecutor）中可能已被驱动执行
     * （ENQUEUED → RUNNING → ENQUEUED），状态断言需容忍 ENQUEUED/RUNNING——
     * 断言的核心是「任务已入队且未被取消/失败」。
     */
    private fun assertScheduledOrRunning(info: WorkInfo) {
        assertTrue(
            "周期任务应已入队并处于调度中，实际 state=${info.state}",
            info.state == WorkInfo.State.ENQUEUED || info.state == WorkInfo.State.RUNNING,
        )
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

        // 桥接已写入（Worker 执行时能拿到 host、LLM 客户端与记忆源）
        assertTrue(ProactiveHostBridge.host === host)
        assertTrue(ProactiveHostBridge.greetingLlmClient is FakeGreetingClient)
        assertTrue("记忆源应写入桥接", ProactiveHostBridge.memoriesProvider != null)

        // WorkManager 唯一周期任务已入队（测试 WorkManager 用同步 Executor，get() 立即返回）
        val infos: List<WorkInfo> = WorkManager.getInstance(context)
            .getWorkInfosForUniqueWork(ProactivePlugin.UNIQUE_WORK_NAME)
            .get()
        assertEquals(1, infos.size)
        assertScheduledOrRunning(infos.single())
    }

    @Test
    fun onLoad_hook_afterReceive_updatesLastInteraction() = runBlocking {
        val host = FakeHost()
        val plugin = ProactivePlugin(config())
        plugin.onLoad(host)

        host.hooks.single().afterReceive("喵~", null, ChatContext(recentMessages = emptyList(), personaName = "凛"))

        // hook 内部用的是同一份 SharedPreferences，新开 tracker 读到的是同一份数据
        assertTrue("afterReceive 应刷新最后交互时间", LastInteractionTracker(context).lastInteractionAt() > 0L)
    }

    @Test
    fun onLoad_hook_beforeSend_updatesLastInteraction() = runBlocking {
        val host = FakeHost()
        val plugin = ProactivePlugin(config())
        plugin.onLoad(host)

        host.hooks.single().beforeSend("你好", ChatContext(recentMessages = emptyList(), personaName = "凛"))

        assertTrue(
            "beforeSend 应刷新最后交互时间（用户发起消息即活跃，任务要求）",
            LastInteractionTracker(context).lastInteractionAt() > 0L,
        )
    }

    @Test
    fun onLoad_debugMode_enqueuesImmediateCheck() {
        val host = FakeHost()
        val plugin = ProactivePlugin(config().copy(debugMode = true))

        plugin.onLoad(host)

        // Debug 短周期开关：额外排一次立即检查（无需等一个完整周期即可触发）
        val infos: List<WorkInfo> = WorkManager.getInstance(context)
            .getWorkInfosForUniqueWork(ProactivePlugin.UNIQUE_IMMEDIATE_CHECK_NAME)
            .get()
        assertEquals("debugMode 应入队立即检查任务", 1, infos.size)
        val state = infos.single().state
        assertTrue(
            "立即检查应处于调度中，实际 state=$state",
            state == WorkInfo.State.ENQUEUED || state == WorkInfo.State.RUNNING || state == WorkInfo.State.SUCCEEDED,
        )
    }

    @Test
    fun onLoad_releaseMode_doesNotEnqueueImmediateCheck() {
        val host = FakeHost()
        val plugin = ProactivePlugin(config()) // debugMode = false

        plugin.onLoad(host)

        val infos: List<WorkInfo> = WorkManager.getInstance(context)
            .getWorkInfosForUniqueWork(ProactivePlugin.UNIQUE_IMMEDIATE_CHECK_NAME)
            .get()
        assertTrue("release 模式不应排立即检查任务", infos.isEmpty())
    }

    @Test
    fun onLoad_clampsWorkManagerIntervalBelowMinimum() {
        // WorkManager 周期间隔硬约束 15 分钟：传 5 分钟不应抛 IllegalArgumentException
        val host = FakeHost()
        val plugin = ProactivePlugin(
            config().copy(workManagerIntervalMinutes = 5L)
        )

        plugin.onLoad(host)

        val infos: List<WorkInfo> = WorkManager.getInstance(context)
            .getWorkInfosForUniqueWork(ProactivePlugin.UNIQUE_WORK_NAME)
            .get()
        assertEquals(1, infos.size)
        assertScheduledOrRunning(infos.single())
    }

    @Test
    fun onLoad_schedulesPeriodicWork_withConfigValues() {
        // WorkInfo 不暴露 inputData（work-runtime 2.9.1 仅有 outputData），
        // 配置 → inputData → Worker 决策 的链路由 ProactiveCheckWorkerTest 覆盖
        // （buildWorker 显式注入 inputData 验证决策）；此处验证：带自定义参数 onLoad 不抛异常且任务入队。
        val host = FakeHost()
        val plugin = ProactivePlugin(
            config().copy(
                idleThresholdMinutes = 5L,
                cooldownMinutes = 2L,
                quietHoursStart = 22,
                quietHoursEnd = 8,
                fallbackGreetings = listOf("a", "b", "c"),
            )
        )

        plugin.onLoad(host)

        val infos: List<WorkInfo> = WorkManager.getInstance(context)
            .getWorkInfosForUniqueWork(ProactivePlugin.UNIQUE_WORK_NAME)
            .get()
        assertEquals(1, infos.size)
        assertScheduledOrRunning(infos.single())
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
        assertNull("onUnload 应清空记忆源", ProactiveHostBridge.memoriesProvider)
    }

    @Test
    fun onUnload_debugMode_cancelsImmediateCheck() {
        val host = FakeHost()
        val plugin = ProactivePlugin(config().copy(debugMode = true))
        plugin.onLoad(host)

        plugin.onUnload()

        val infos: List<WorkInfo> = WorkManager.getInstance(context)
            .getWorkInfosForUniqueWork(ProactivePlugin.UNIQUE_IMMEDIATE_CHECK_NAME)
            .get()
        assertEquals(1, infos.size)
        val state = infos.single().state
        assertTrue(
            "onUnload 应取消立即检查任务，实际 state=$state",
            state == WorkInfo.State.CANCELLED || state == WorkInfo.State.SUCCEEDED,
        )
    }
}
