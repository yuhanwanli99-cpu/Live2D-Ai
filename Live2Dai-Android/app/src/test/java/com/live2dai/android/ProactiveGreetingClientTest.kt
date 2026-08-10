package com.live2dai.android

import kotlinx.coroutines.runBlocking
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * [ProactiveGreetingClient] 单元测试：验证「人设 + 时间 + 最近记忆 → LLM 主动问候」链路的
 * HTTP 行为与容错（docs/plan-v12-plan.schema.json → GreetingLlmClient.generateGreeting）。
 *
 * 契约断言：
 * - 正常响应：提取 message.content 返回（trim 后）
 * - 请求侧：非流式（stream=false）、使用注入 provider（baseUrl/modelName）、
 *   记忆列表出现在请求体（记忆驱动话题，plan §5.6.3）
 * - 容错：HTTP 错误 / 响应解析失败 / 空内容 / 无记忆：一律返回 null 或正常生成（静默，不影响主链路）
 */
class ProactiveGreetingClientTest {

    private lateinit var server: MockWebServer

    @Before
    fun setUp() {
        server = MockWebServer()
        server.start()
    }

    @After
    fun tearDown() {
        server.shutdown()
    }

    private fun provider() = LLMProvider(
        type = ProviderType.LOCAL,
        baseUrl = server.url("/").toString().trimEnd('/'),
        apiKey = "",
        modelName = "test-greeting-model",
        displayName = "test",
    )

    private val memories = listOf("用户喜欢喝美式咖啡", "用户养了一只叫豆豆的猫")

    // ── 正常链路 ──

    @Test
    fun generateGreeting_returnsContentFromLlmResponse() = runBlocking {
        server.enqueue(
            MockResponse().setResponseCode(200)
                .setBody("""{"choices":[{"message":{"content":"好久没聊了，豆豆最近怎么样呀？"}}]}""")
        )

        val result = ProactiveGreetingClient(provider())
            .generateGreeting("凛", "你是凛，一位18岁的天才少女。", 45L, memories)

        assertEquals("好久没聊了，豆豆最近怎么样呀？", result)
        val body = server.takeRequest().body.readUtf8()
        assertTrue("请求应为非流式", body.contains("\"stream\":false"))
        assertTrue("请求应使用注入 provider 的模型名", body.contains("test-greeting-model"))
        assertTrue("请求应包含空闲分钟数（时间感）", body.contains("45"))
        assertTrue("请求应包含最近记忆（记忆驱动话题）", body.contains("用户养了一只叫豆豆的猫"))
    }

    @Test
    fun generateGreeting_trimsWhitespaceAroundContent() = runBlocking {
        server.enqueue(
            MockResponse().setResponseCode(200)
                .setBody("""{"choices":[{"message":{"content":"  该喝水啦~  "}}]}""")
        )

        val result = ProactiveGreetingClient(provider())
            .generateGreeting("凛", "prompt", 10L, null)

        assertEquals("该喝水啦~", result)
    }

    // ── 容错：静默返回 null ──

    @Test
    fun generateGreeting_httpError_returnsNull() = runBlocking {
        server.enqueue(MockResponse().setResponseCode(500).setBody("internal error"))

        val result = ProactiveGreetingClient(provider())
            .generateGreeting("凛", "prompt", 10L, memories)

        assertNull("HTTP 错误应静默返回 null（插件回退静态文案池）", result)
    }

    @Test
    fun generateGreeting_emptyContent_returnsNull() = runBlocking {
        server.enqueue(
            MockResponse().setResponseCode(200)
                .setBody("""{"choices":[{"message":{"content":"  "}}]}""")
        )

        val result = ProactiveGreetingClient(provider())
            .generateGreeting("凛", "prompt", 10L, memories)

        assertNull("空内容应静默返回 null", result)
    }

    @Test
    fun generateGreeting_malformedJson_returnsNull() = runBlocking {
        server.enqueue(MockResponse().setResponseCode(200).setBody("not-json-at-all"))

        val result = ProactiveGreetingClient(provider())
            .generateGreeting("凛", "prompt", 10L, memories)

        assertNull("响应解析失败应静默返回 null", result)
    }

    @Test
    fun generateGreeting_noMemories_stillGenerates() = runBlocking {
        server.enqueue(
            MockResponse().setResponseCode(200)
                .setBody("""{"choices":[{"message":{"content":"今天也要元气满满哦"}}]}""")
        )

        val result = ProactiveGreetingClient(provider())
            .generateGreeting("凛", "prompt", 5L, null)

        assertEquals("无记忆时仍应能按人设+时间生成", "今天也要元气满满哦", result)
        val body = server.takeRequest().body.readUtf8()
        assertTrue("无记忆时请求体应标注暂无", body.contains("（暂无）"))
    }

    @Test
    fun generateGreeting_sendsBearerTokenWhenApiKeyPresent() = runBlocking {
        server.enqueue(
            MockResponse().setResponseCode(200)
                .setBody("""{"choices":[{"message":{"content":"该喝水啦"}}]}""")
        )
        val keyed = provider().copy(apiKey = "secret-key")

        ProactiveGreetingClient(keyed).generateGreeting("凛", "prompt", 10L, memories)

        val auth = server.takeRequest().getHeader("Authorization")
        assertEquals("Bearer secret-key", auth)
    }
}
