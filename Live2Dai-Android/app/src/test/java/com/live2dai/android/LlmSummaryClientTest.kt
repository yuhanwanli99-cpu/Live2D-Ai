package com.live2dai.android

import com.live2dai.pluginsdk.ChatTurn
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
 * [LlmSummaryClient] 单元测试：验证「近期对话 → LLM 摘要」链路的 HTTP 行为与容错
 * （docs/plan-v11-plan.schema.json → LlmSummaryClient.summarize）。
 *
 * 契约断言：
 * - 正常响应：提取 message.content 返回（trim 后）
 * - HTTP 错误 / 响应解析失败 / 空内容：一律返回 null（静默跳过，不影响主链路）
 * - 请求侧：非流式（stream=false）、使用注入的 provider（baseUrl/modelName）
 */
class LlmSummaryClientTest {

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
        modelName = "test-summary-model",
        displayName = "test",
    )

    private fun sampleTurns() = listOf(
        ChatTurn("user", "我喜欢喝美式咖啡"),
        ChatTurn("assistant", "好的，记住了~"),
    )

    // ── 正常链路 ──

    @Test
    fun summarize_returnsContentFromLlmResponse() = runBlocking {
        server.enqueue(
            MockResponse().setResponseCode(200)
                .setBody("""{"choices":[{"message":{"content":"用户喜欢喝美式咖啡"}}]}""")
        )

        val result = LlmSummaryClient(provider()).summarize(sampleTurns(), "凛")

        assertEquals("用户喜欢喝美式咖啡", result)
        val body = server.takeRequest().body.readUtf8()
        assertTrue("请求应为非流式（工具循环同款慢路径）", body.contains("\"stream\":false"))
        assertTrue("请求应使用注入 provider 的模型名", body.contains("test-summary-model"))
    }

    @Test
    fun summarize_trimsWhitespaceAroundContent() = runBlocking {
        server.enqueue(
            MockResponse().setResponseCode(200)
                .setBody("""{"choices":[{"message":{"content":"  用户喜欢喝咖啡  "}}]}""")
        )

        val result = LlmSummaryClient(provider()).summarize(sampleTurns(), "凛")

        assertEquals("用户喜欢喝咖啡", result)
    }

    // ── 容错：静默返回 null ──

    @Test
    fun summarize_emptyTurns_returnsNullWithoutHttpCall() = runBlocking {
        val result = LlmSummaryClient(provider()).summarize(emptyList(), "凛")

        assertNull("无近期对话不应发起请求", result)
        assertEquals(0, server.requestCount)
    }

    @Test
    fun summarize_httpError_returnsNull() = runBlocking {
        server.enqueue(MockResponse().setResponseCode(500).setBody("internal error"))

        val result = LlmSummaryClient(provider()).summarize(sampleTurns(), "凛")

        assertNull("HTTP 错误应静默返回 null", result)
    }

    @Test
    fun summarize_emptyContent_returnsNull() = runBlocking {
        server.enqueue(
            MockResponse().setResponseCode(200)
                .setBody("""{"choices":[{"message":{"content":"  "}}]}""")
        )

        val result = LlmSummaryClient(provider()).summarize(sampleTurns(), "凛")

        assertNull("空内容表示本轮无值得记忆的事实，应返回 null", result)
    }

    @Test
    fun summarize_malformedJson_returnsNull() = runBlocking {
        server.enqueue(MockResponse().setResponseCode(200).setBody("not-json-at-all"))

        val result = LlmSummaryClient(provider()).summarize(sampleTurns(), "凛")

        assertNull("响应解析失败应静默返回 null", result)
    }

    @Test
    fun summarize_sendsBearerTokenWhenApiKeyPresent() = runBlocking {
        server.enqueue(
            MockResponse().setResponseCode(200)
                .setBody("""{"choices":[{"message":{"content":"用户喜欢咖啡"}}]}""")
        )
        val keyed = provider().copy(apiKey = "secret-key")

        LlmSummaryClient(keyed).summarize(sampleTurns(), "凛")

        val auth = server.takeRequest().getHeader("Authorization")
        assertEquals("Bearer secret-key", auth)
    }
}
