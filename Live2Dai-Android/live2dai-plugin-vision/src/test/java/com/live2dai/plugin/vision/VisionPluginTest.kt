package com.live2dai.plugin.vision

import android.graphics.Bitmap
import com.live2dai.pluginsdk.ChatContext
import com.live2dai.pluginsdk.ChatHook
import com.live2dai.pluginsdk.PersonaSnapshot
import com.live2dai.pluginsdk.PluginHost
import com.live2dai.pluginsdk.ToolDefinition
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * 插件注册流程测试：验证 [VisionPlugin.onLoad] 向 host 注册 ChatHook + ToolDefinition，
 * 以及 hook / tool 在截图不可用、未配置 API Key 等情况下的降级行为。
 */
@RunWith(RobolectricTestRunner::class)
class VisionPluginTest {

    /** 内存版 PluginHost 测试替身 */
    private class FakeHost : PluginHost {
        val hooks = mutableListOf<ChatHook>()
        val tools = mutableListOf<ToolDefinition>()
        val sentMessages = mutableListOf<Pair<String, String?>>()

        override fun registerHook(hook: ChatHook) { hooks += hook }
        override fun registerTool(tool: ToolDefinition) { tools += tool }
        override fun sendAssistantMessage(text: String, emotion: String?) {
            sentMessages += text to emotion
        }
        override fun getPersona(): PersonaSnapshot =
            PersonaSnapshot(name = "白", systemPrompt = "prompt", model = "model")
    }

    private fun sampleBitmap(): Bitmap = Bitmap.createBitmap(4, 4, Bitmap.Config.ARGB_8888)

    private fun chatContext() = ChatContext(recentMessages = emptyList(), personaName = "白")

    // ── 插件注册 ──

    @Test
    fun onLoad_registersOneHookAndCaptureScreenTool() {
        val host = FakeHost()
        val plugin = VisionPlugin(
            VisionPluginConfig(
                apiKey = "test-key",
                visionModel = "glm-4.6v",
                screenshotProvider = ScreenshotProvider { null },
            )
        )

        plugin.onLoad(host)

        assertEquals(1, host.hooks.size)
        assertEquals(1, host.tools.size)
        assertEquals("capture_screen", host.tools.single().name)
    }

    @Test
    fun onUnload_doesNotThrow() {
        val host = FakeHost()
        val plugin = VisionPlugin(
            VisionPluginConfig(
                apiKey = "test-key",
                visionModel = "glm-4.6v",
                screenshotProvider = ScreenshotProvider { null },
            )
        )
        plugin.onLoad(host)
        plugin.onUnload() // 不应抛异常
    }

    // ── ChatHook 行为 ──

    @Test
    fun beforeSend_returnsNullWhenScreenshotUnavailable() = runBlocking {
        val host = FakeHost()
        val plugin = VisionPlugin(
            VisionPluginConfig(
                apiKey = "test-key",
                visionModel = "glm-4.6v",
                screenshotProvider = ScreenshotProvider { null },
            )
        )
        plugin.onLoad(host)

        val result = host.hooks.single().beforeSend("你好", chatContext())
        assertEquals(null, result)
    }

    @Test
    fun beforeSend_returnsNullWithoutCapturingWhenApiKeyBlank() = runBlocking {
        var captureCalled = false
        val host = FakeHost()
        val plugin = VisionPlugin(
            VisionPluginConfig(
                apiKey = "",
                visionModel = "glm-4.6v",
                screenshotProvider = ScreenshotProvider {
                    captureCalled = true
                    sampleBitmap()
                },
            )
        )
        plugin.onLoad(host)

        val result = host.hooks.single().beforeSend("你好", chatContext())
        assertEquals(null, result)
        assertFalse("未配置 API Key 时不应触发截图", captureCalled)
    }

    @Test
    fun beforeSend_injectsScreenDescription() = runBlocking(Dispatchers.IO) {
        val server = MockWebServer()
        server.start()
        server.enqueue(
            MockResponse()
                .setResponseCode(200)
                .setBody("""{"choices":[{"message":{"content":"屏幕上显示聊天界面"}}]}""")
        )
        val client = GlmVisionClient(apiKey = "test-key", baseUrl = server.url("/").toString().trimEnd('/'))
        val hook = VisionDescribeHook(
            client = client,
            visionModel = "glm-4.6v",
            screenshotProvider = ScreenshotProvider { sampleBitmap() },
        )

        val result = hook.beforeSend("你好", chatContext())
        server.shutdown()

        assertEquals("[系统: 当前屏幕内容]\n屏幕截图描述: 屏幕上显示聊天界面", result)
    }

    // ── ToolDefinition 行为 ──

    @Test
    fun captureScreenTool_returnsDescription() = runBlocking(Dispatchers.IO) {
        val server = MockWebServer()
        server.start()
        server.enqueue(
            MockResponse()
                .setResponseCode(200)
                .setBody("""{"choices":[{"message":{"content":"屏幕上显示一个时钟"}}]}""")
        )
        val client = GlmVisionClient(apiKey = "test-key", baseUrl = server.url("/").toString().trimEnd('/'))
        val tool = ScreenCaptureTool(
            client = client,
            visionModel = "glm-4.6v",
            screenshotProvider = ScreenshotProvider { sampleBitmap() },
        )

        val result = tool.execute("""{"prompt": "看看屏幕上的时间"}""")
        server.shutdown()

        assertEquals("屏幕上显示一个时钟", result)
    }

    @Test
    fun captureScreenTool_returnsErrorWhenScreenshotUnavailable() = runBlocking {
        val client = GlmVisionClient(apiKey = "test-key", baseUrl = "http://localhost:1")
        val tool = ScreenCaptureTool(
            client = client,
            visionModel = "glm-4.6v",
            screenshotProvider = ScreenshotProvider { null },
        )

        val result = tool.execute("{}")

        assertTrue("Should report screenshot failure", result.contains("无法截取屏幕"))
    }

    @Test
    fun captureScreenTool_returnsErrorWhenApiKeyBlank() = runBlocking {
        val client = GlmVisionClient(apiKey = "", baseUrl = "http://localhost:1")
        val tool = ScreenCaptureTool(
            client = client,
            visionModel = "glm-4.6v",
            screenshotProvider = ScreenshotProvider { sampleBitmap() },
        )

        val result = tool.execute("{}")

        assertTrue("Should report missing api key", result.contains("未配置 Zhipu API Key"))
    }

    @Test
    fun captureScreenTool_toleratesInvalidArgsJson() = runBlocking {
        // 非法参数 JSON → 使用默认提示词；截图不可用 → 返回截图错误（而非崩溃）
        val client = GlmVisionClient(apiKey = "test-key", baseUrl = "http://localhost:1")
        val tool = ScreenCaptureTool(
            client = client,
            visionModel = "glm-4.6v",
            screenshotProvider = ScreenshotProvider { null },
        )

        val result = tool.execute("not-json")

        assertTrue(result.contains("无法截取屏幕"))
    }
}
