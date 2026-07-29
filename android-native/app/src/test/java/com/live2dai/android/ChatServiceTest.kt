package com.live2dai.android

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.runBlocking
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * Tests for ChatService — uses MockWebServer to intercept HTTP calls
 * to DeepSeek API and return mock SSE streaming responses.
 *
 * ChatMessage.role is required in JSON serialization (no default),
 * so SSE deltas must include "role":"assistant".
 *
 * Uses runBlocking because ChatService uses flowOn(Dispatchers.IO)
 * which conflicts with runTest's virtual time dispatcher.
 */
class ChatServiceTest {

    private lateinit var mockWebServer: MockWebServer
    private lateinit var chatService: ChatService

    @Before
    fun setUp() {
        mockWebServer = MockWebServer()
        mockWebServer.start()
        val baseUrl = mockWebServer.url("").toString().trimEnd('/')
        chatService = ChatService(apiKey = "test-key", baseUrl = baseUrl)
    }

    @After
    fun tearDown() {
        mockWebServer.shutdown()
    }

    @Test
    fun testSendMessageReturnsStreamingDeltas() = runBlocking(Dispatchers.IO) {
        // ChatMessage.role is required (no default), so include it in every delta
        val sseBody = buildString {
            appendLine("data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"喵\"}}]}")
            appendLine("data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"~\"}}]}")
            appendLine("data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\" 主人好\"}}]}")
            appendLine("data: [DONE]")
        }
        mockWebServer.enqueue(
            MockResponse()
                .setResponseCode(200)
                .setHeader("Content-Type", "text/event-stream")
                .setBody(sseBody)
        )

        val deltas = chatService.sendMessage("你好").toList()
        assertEquals(listOf("喵", "~", " 主人好"), deltas)
    }

    @Test
    fun testEmptyMessageEmitsNothing() = runBlocking {
        val deltas = chatService.sendMessage("").toList()
        assertEquals(0, deltas.size)
    }

    @Test
    fun testBlankMessageEmitsNothing() = runBlocking {
        val deltas = chatService.sendMessage("   ").toList()
        assertEquals(0, deltas.size)
    }

    @Test
    fun testApiErrorReturnsErrorMessage() = runBlocking(Dispatchers.IO) {
        mockWebServer.enqueue(
            MockResponse()
                .setResponseCode(401)
                .setBody("{\"error\":\"unauthorized\"}")
        )

        val result = chatService.sendMessage("test").toList()
        assertTrue("Should contain error message", result.first().contains("错误"))
    }

    @Test
    fun testResetConversationKeepsWorking() = runBlocking(Dispatchers.IO) {
        val sseBody = "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"回复1\"}}]}\ndata: [DONE]\n"
        mockWebServer.enqueue(
            MockResponse()
                .setResponseCode(200)
                .setHeader("Content-Type", "text/event-stream")
                .setBody(sseBody)
        )

        val response1 = chatService.sendMessage("消息1").toList()
        assertEquals(listOf("回复1"), response1)

        chatService.resetConversation()

        mockWebServer.enqueue(
            MockResponse()
                .setResponseCode(200)
                .setHeader("Content-Type", "text/event-stream")
                .setBody("data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"回复2\"}}]}\ndata: [DONE]\n")
        )

        val response2 = chatService.sendMessage("消息2").toList()
        assertEquals(listOf("回复2"), response2)
    }

    @Test
    fun testRequestContainsAuthorizationHeader() = runBlocking(Dispatchers.IO) {
        mockWebServer.enqueue(
            MockResponse()
                .setResponseCode(200)
                .setHeader("Content-Type", "text/event-stream")
                .setBody("data: [DONE]\n")
        )

        chatService.sendMessage("test").toList()

        val recordedRequest = mockWebServer.takeRequest()
        assertEquals("Bearer test-key", recordedRequest.getHeader("Authorization"))
        assertEquals("/chat/completions", recordedRequest.path)
    }

    @Test
    fun testRequestContainsExpectedJsonBody() = runBlocking(Dispatchers.IO) {
        val sseBody = "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"ok\"}}]}\ndata: [DONE]\n"
        mockWebServer.enqueue(
            MockResponse()
                .setResponseCode(200)
                .setHeader("Content-Type", "text/event-stream")
                .setBody(sseBody)
        )

        chatService.sendMessage("你好主人").toList()

        val request = mockWebServer.takeRequest()
        val bodyString = request.body.readUtf8()
        assertTrue("Request body should contain user message", bodyString.contains("你好主人"))
        assertTrue("Request body should contain model", bodyString.contains("deepseek-v4-flash"))
        assertTrue("Request body should have streaming enabled", bodyString.contains("\"stream\":true"))
    }
}
