package com.live2dai.android

import com.live2dai.pluginsdk.ToolDefinition
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.runBlocking
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
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

    // ─────────────────────────────────────────────────────────────
    // 既有测试（回归：无工具注册时行为完全不变）
    // ─────────────────────────────────────────────────────────────

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

    // ─────────────────────────────────────────────────────────────
    // 工具调用循环测试（docs/plan-task-chat-tool-loop.md §7.2 不变量）
    // ─────────────────────────────────────────────────────────────

    /**
     * 可控的 FakeTool：记录 execute 调用次数与参数。
     */
    private class FakeTool(
        override val name: String,
        override val description: String = "测试工具 $name",
        override val parametersSchema: String = """{"type":"object","properties":{}}""",
        override val requiresConfirmation: Boolean = false,
        val onExecute: (String) -> String = { """{"result":"ok"}""" }
    ) : ToolDefinition {
        var executeCount = 0
        val executedArgs = mutableListOf<String>()

        override suspend fun execute(argsJson: String): String {
            executeCount++
            executedArgs.add(argsJson)
            return onExecute(argsJson)
        }
    }

    /**
     * 可控的 FakeGate：按预设结果响应 confirm，记录调用次数。
     */
    private class FakeGate(private val approved: Boolean) : ConfirmationGate {
        var confirmCount = 0

        override suspend fun confirm(tool: ToolDefinition, argsJson: String): Boolean {
            confirmCount++
            return approved
        }
    }

    /** 构造一个绑定了 MockWebServer 且带工具注册表的 ChatService。 */
    private fun chatServiceWith(
        tools: List<ToolDefinition>,
        gate: ConfirmationGate? = null
    ): ChatService {
        return ChatService(
            provider = LLMProvider(
                type = ProviderType.DEEPSEEK,
                baseUrl = mockWebServer.url("").toString().trimEnd('/'),
                apiKey = "test-key",
                modelName = "test-model",
                displayName = "test"
            ),
            hooks = emptyList(),
            tools = tools,
            confirmationGate = gate
        )
    }

    /** 非流式 tool_call 响应。args 为普通 JSON 对象文本，自动转义为 JSON 字符串值。 */
    private fun toolCallResponse(toolName: String, callId: String, args: String = "{}"): String {
        val escapedArgs = args.replace("\\", "\\\\").replace("\"", "\\\"")
        return """
            {"choices":[{"index":0,"message":{"role":"assistant","content":null,
              "tool_calls":[{"id":"$callId","type":"function","function":{"name":"$toolName","arguments":"$escapedArgs"}}]
            },"finish_reason":"tool_calls"}]}
        """.trimIndent()
    }

    /** 非流式纯文本响应。 */
    private fun textResponse(text: String): String {
        return """{"choices":[{"index":0,"message":{"role":"assistant","content":"$text"},"finish_reason":"stop"}]}"""
    }

    // T1（不变量 I1）：无工具注册 — 请求体不含 tools 字段，stream=true，行为完全不变
    @Test
    fun testNoToolsRegisteredRequestHasNoToolsField() = runBlocking(Dispatchers.IO) {
        mockWebServer.enqueue(
            MockResponse()
                .setResponseCode(200)
                .setHeader("Content-Type", "text/event-stream")
                .setBody("data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"ok\"}}]}\ndata: [DONE]\n")
        )

        chatService.sendMessage("test").toList()

        val request = mockWebServer.takeRequest()
        val bodyString = request.body.readUtf8()
        assertTrue(bodyString.contains("\"stream\":true"))
        assertFalse("无工具注册时请求体不应包含 tools 字段", bodyString.contains("\"tools\""))
    }

    // T2：有工具 — LLM 返回 tool_call — 正确执行并回填
    @Test
    fun testToolCallExecutesAndFeedsResultBackToLLM() = runBlocking(Dispatchers.IO) {
        val tool = FakeTool(name = "open_app")
        val service = chatServiceWith(listOf(tool))

        mockWebServer.enqueue(
            MockResponse().setResponseCode(200).setBody(
                toolCallResponse("open_app", callId = "call_abc", args = """{"app_name":"微信"}""")
            )
        )
        mockWebServer.enqueue(MockResponse().setResponseCode(200).setBody(textResponse("已经帮你打开微信了喵~")))

        val result = service.sendMessage("帮我打开微信").toList()

        // 最终 Flow 发射自然语言文本
        assertEquals(listOf("已经帮你打开微信了喵~"), result)
        // 工具被执行一次，参数正确传递
        assertEquals(1, tool.executeCount)
        assertEquals("""{"app_name":"微信"}""", tool.executedArgs.single())

        // 第一轮请求：携带 tools 字段 + 非流式
        val request1 = mockWebServer.takeRequest()
        val body1 = request1.body.readUtf8()
        assertTrue(body1.contains("\"tools\""))
        assertTrue(body1.contains("open_app"))
        assertTrue(body1.contains("\"stream\":false"))

        // 第二轮请求：回填了 assistant(tool_calls) + tool 结果消息
        val request2 = mockWebServer.takeRequest()
        val body2 = request2.body.readUtf8()
        assertTrue("回填消息应包含 tool_call_id", body2.contains("call_abc"))
        assertTrue("回填消息应包含 role=tool", body2.contains("\"role\":\"tool\""))
    }

    // T3（不变量 I2）：危险工具 — 用户拒绝 — execute 从未被调用
    @Test
    fun testDangerousToolRefusedByUserNeverExecutes() = runBlocking(Dispatchers.IO) {
        val tool = FakeTool(name = "send_reply", requiresConfirmation = true)
        val gate = FakeGate(approved = false)
        val service = chatServiceWith(listOf(tool), gate)

        mockWebServer.enqueue(
            MockResponse().setResponseCode(200).setBody(toolCallResponse("send_reply", callId = "call_x"))
        )
        mockWebServer.enqueue(MockResponse().setResponseCode(200).setBody(textResponse("好的，我不会替你发送")))

        val result = service.sendMessage("帮我回复").toList()

        assertEquals(0, tool.executeCount)
        assertEquals(1, gate.confirmCount)
        assertEquals(listOf("好的，我不会替你发送"), result)

        // LLM 第二轮收到「用户拒绝执行」的结果
        mockWebServer.takeRequest()
        val body2 = mockWebServer.takeRequest().body.readUtf8()
        assertTrue(body2.contains("用户拒绝执行"))
    }

    // T4：危险工具 — 用户同意 — 正常执行
    @Test
    fun testDangerousToolApprovedByUserExecutes() = runBlocking(Dispatchers.IO) {
        val tool = FakeTool(name = "send_reply", requiresConfirmation = true)
        val gate = FakeGate(approved = true)
        val service = chatServiceWith(listOf(tool), gate)

        mockWebServer.enqueue(
            MockResponse().setResponseCode(200).setBody(toolCallResponse("send_reply", callId = "call_y"))
        )
        mockWebServer.enqueue(MockResponse().setResponseCode(200).setBody(textResponse("已替你发送")))

        val result = service.sendMessage("帮我回复").toList()

        assertEquals(1, gate.confirmCount)
        assertEquals(1, tool.executeCount)
        assertEquals(listOf("已替你发送"), result)
    }

    // T5（不变量 I3）：循环上限 — 第 8 轮被截断，实际 HTTP 请求不超过 8 次
    @Test
    fun testToolLoopStopsAfterMaxRounds() = runBlocking(Dispatchers.IO) {
        val tool = FakeTool(name = "open_app")
        val service = chatServiceWith(listOf(tool))

        // 连续 8 个响应都返回 tool_calls（模拟 LLM 无限调工具）
        repeat(8) {
            mockWebServer.enqueue(
                MockResponse().setResponseCode(200).setBody(toolCallResponse("open_app", callId = "call_$it"))
            )
        }

        val result = service.sendMessage("test").toList()

        assertTrue("应发射循环上限错误信息", result.last().contains("超过最大轮次"))
        assertEquals("实际 HTTP 请求不应超过 8 次", 8, mockWebServer.requestCount)
        // 第 1-7 轮执行了工具，第 8 轮被截断
        assertEquals(7, tool.executeCount)
    }

    // T6（不变量 I4）：工具执行异常 — 不中断循环
    @Test
    fun testToolExceptionDoesNotBreakLoop() = runBlocking(Dispatchers.IO) {
        val toolA = FakeTool(name = "tool_a", onExecute = { throw RuntimeException("boom") })
        val toolB = FakeTool(name = "tool_b")
        val service = chatServiceWith(listOf(toolA, toolB))

        mockWebServer.enqueue(
            MockResponse().setResponseCode(200).setBody(
                """
                {"choices":[{"index":0,"message":{"role":"assistant","content":null,
                  "tool_calls":[
                    {"id":"call_a","type":"function","function":{"name":"tool_a","arguments":"{}"}},
                    {"id":"call_b","type":"function","function":{"name":"tool_b","arguments":"{}"}}
                  ]
                },"finish_reason":"tool_calls"}]}
                """.trimIndent()
            )
        )
        mockWebServer.enqueue(MockResponse().setResponseCode(200).setBody(textResponse("完成")))

        val result = service.sendMessage("test").toList()

        assertEquals(listOf("完成"), result)
        // A 抛异常但 B 仍被正常执行（异常隔离）
        assertEquals(1, toolA.executeCount)
        assertEquals(1, toolB.executeCount)

        // A 的异常信息作为 tool result 回填
        mockWebServer.takeRequest()
        val body2 = mockWebServer.takeRequest().body.readUtf8()
        assertTrue(body2.contains("工具执行异常"))
        assertTrue(body2.contains("boom"))
    }

    // T7（不变量 I5）：confirmationGate=null — 危险工具被拒绝（安全默认）
    @Test
    fun testNullGateRejectsDangerousToolByDefault() = runBlocking(Dispatchers.IO) {
        val tool = FakeTool(name = "danger", requiresConfirmation = true)
        val service = chatServiceWith(listOf(tool), gate = null)

        mockWebServer.enqueue(
            MockResponse().setResponseCode(200).setBody(toolCallResponse("danger", callId = "call_z"))
        )
        mockWebServer.enqueue(MockResponse().setResponseCode(200).setBody(textResponse("已拒绝")))

        val result = service.sendMessage("test").toList()

        assertEquals(0, tool.executeCount)
        assertEquals(listOf("已拒绝"), result)

        mockWebServer.takeRequest()
        val body2 = mockWebServer.takeRequest().body.readUtf8()
        assertTrue(body2.contains("用户拒绝执行"))
    }

    // T8：工具未注册 — 返回错误但不崩溃，循环继续
    @Test
    fun testUnregisteredToolReturnsErrorWithoutCrashing() = runBlocking(Dispatchers.IO) {
        val knownTool = FakeTool(name = "known_tool")
        val service = chatServiceWith(listOf(knownTool))

        mockWebServer.enqueue(
            MockResponse().setResponseCode(200).setBody(toolCallResponse("unknown_tool", callId = "call_u"))
        )
        mockWebServer.enqueue(MockResponse().setResponseCode(200).setBody(textResponse("继续")))

        val result = service.sendMessage("test").toList()

        assertEquals(listOf("继续"), result)
        assertEquals(0, knownTool.executeCount)

        mockWebServer.takeRequest()
        val body2 = mockWebServer.takeRequest().body.readUtf8()
        assertTrue(body2.contains("未找到工具"))
        assertTrue(body2.contains("unknown_tool"))
    }

    // T9：多轮工具调用 — LLM 需要多次调用工具才生成最终回复
    @Test
    fun testMultiRoundToolCalls() = runBlocking(Dispatchers.IO) {
        val toolA = FakeTool(name = "tool_a")
        val toolB = FakeTool(name = "tool_b")
        val service = chatServiceWith(listOf(toolA, toolB))

        mockWebServer.enqueue(
            MockResponse().setResponseCode(200).setBody(toolCallResponse("tool_a", callId = "call_a"))
        )
        mockWebServer.enqueue(
            MockResponse().setResponseCode(200).setBody(toolCallResponse("tool_b", callId = "call_b"))
        )
        mockWebServer.enqueue(MockResponse().setResponseCode(200).setBody(textResponse("最终回复")))

        val result = service.sendMessage("test").toList()

        assertEquals(listOf("最终回复"), result)
        assertEquals(1, toolA.executeCount)
        assertEquals(1, toolB.executeCount)
        assertEquals(3, mockWebServer.requestCount)
    }

    // T10（不变量 I1）：旧 ChatService(apiKey, baseUrl) 次构造函数 — 向后兼容
    @Test
    fun testSecondaryConstructorIsBackwardCompatible() = runBlocking(Dispatchers.IO) {
        mockWebServer.enqueue(
            MockResponse()
                .setResponseCode(200)
                .setHeader("Content-Type", "text/event-stream")
                .setBody("data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"ok\"}}]}\ndata: [DONE]\n")
        )

        val result = chatService.sendMessage("test").toList()
        assertEquals(listOf("ok"), result)

        val bodyString = mockWebServer.takeRequest().body.readUtf8()
        assertFalse("次构造函数不注册工具，请求体不应包含 tools 字段", bodyString.contains("\"tools\""))
    }

    // ─────────────────────────────────────────────────────────────
    // GLM 429（限 1 并发）友好重试提示（task_42f86a40c1ed 新增的提示文案）
    // ─────────────────────────────────────────────────────────────

    // 流式路径：无工具注册走 streamResponse()，HTTP 429 时应发射友好重试提示
    @Test
    fun testRateLimit429ShowsFriendlyRetryHint_streamingPath() = runBlocking(Dispatchers.IO) {
        mockWebServer.enqueue(
            MockResponse()
                .setResponseCode(429)
                .setBody("{\"error\":{\"code\":\"1301\",\"message\":\"用户请求过于频繁\"}}")
        )

        val result = chatService.sendMessage("test").toList()
        assertTrue("流式路径 429 应包含友好重试提示", result.isNotEmpty())
        assertTrue("应提示请求过于频繁", result.first().contains("请求过于频繁"))
        assertTrue("应提示稍后重试", result.first().contains("稍后重试"))
        assertTrue("应说明免费模型限 1 并发", result.first().contains("1 并发"))
    }

    // 非流式路径：有工具注册走 runToolLoop() → executeNonStreamingRequest()，
    // HTTP 429 时同样应发射友好重试提示（工具不应被执行）
    @Test
    fun testRateLimit429ShowsFriendlyRetryHint_nonStreamingPath() = runBlocking(Dispatchers.IO) {
        val tool = FakeTool(name = "open_app")
        val service = chatServiceWith(listOf(tool))

        mockWebServer.enqueue(
            MockResponse()
                .setResponseCode(429)
                .setBody("{\"error\":{\"code\":\"1301\",\"message\":\"用户请求过于频繁\"}}")
        )

        val result = service.sendMessage("帮我打开微信").toList()
        assertTrue("非流式路径 429 应包含友好重试提示", result.isNotEmpty())
        assertTrue("应提示请求过于频繁", result.first().contains("请求过于频繁"))
        assertTrue("应提示稍后重试", result.first().contains("稍后重试"))
        assertEquals("429 时工具不应被执行", 0, tool.executeCount)
    }

    // ─────────────────────────────────────────────────────────────
    // 共享免费 Key 额度耗尽/欠费（403 / quota 类错误）：不重试，引导换 Key
    // ─────────────────────────────────────────────────────────────

    // 流式路径：HTTP 403（智谱额度/余额不足类错误）→ 引导用户前往设置填写自己的 Key
    @Test
    fun testQuotaExhausted403ShowsFriendlyGuidance_streamingPath() = runBlocking(Dispatchers.IO) {
        mockWebServer.enqueue(
            MockResponse()
                .setResponseCode(403)
                .setBody("{\"error\":{\"code\":\"1002\",\"message\":\"账户余额不足\"}}")
        )

        val result = chatService.sendMessage("test").toList()
        assertTrue("403 应输出额度耗尽提示", result.isNotEmpty())
        val msg = result.first()
        assertTrue("应带额度耗尽标记", msg.contains(ChatService.QUOTA_EXHAUSTED_MARKER))
        assertTrue("应说明当前使用的是官方免费额度", msg.contains("免费额度"))
        assertTrue("应引导前往设置", msg.contains("设置"))
        assertTrue("应引导填写自己的 API Key", msg.contains("API Key"))
        assertFalse("额度耗尽不应按限流重试处理", msg.contains("稍后重试"))
    }

    // 非流式路径（工具循环）：403 → 引导换 Key，工具不应被执行
    @Test
    fun testQuotaExhausted403ShowsFriendlyGuidance_nonStreamingPath() = runBlocking(Dispatchers.IO) {
        val tool = FakeTool(name = "open_app")
        val service = chatServiceWith(listOf(tool))

        mockWebServer.enqueue(
            MockResponse()
                .setResponseCode(403)
                .setBody("{\"error\":{\"code\":\"1002\",\"message\":\"账户余额不足\"}}")
        )

        val result = service.sendMessage("帮我打开微信").toList()
        assertTrue("非流式路径 403 应输出额度耗尽提示", result.isNotEmpty())
        val msg = result.first()
        assertTrue("应带额度耗尽标记", msg.contains(ChatService.QUOTA_EXHAUSTED_MARKER))
        assertTrue("应引导填写自己的 API Key", msg.contains("API Key"))
        assertEquals("403 时工具不应被执行", 0, tool.executeCount)
    }

    // 非 403 但 body 命中额度耗尽关键词（如 400 + 余额不足）→ 同样识别为额度耗尽
    @Test
    fun testQuotaExhaustedByBodyKeyword_streamingPath() = runBlocking(Dispatchers.IO) {
        mockWebServer.enqueue(
            MockResponse()
                .setResponseCode(400)
                .setBody("{\"error\":{\"code\":\"2003\",\"message\":\"该模型的免费额度已用尽，请充值或更换模型\"}}")
        )

        val result = chatService.sendMessage("test").toList()
        assertTrue("body 命中额度关键词应输出额度耗尽提示", result.isNotEmpty())
        val msg = result.first()
        assertTrue("应带额度耗尽标记", msg.contains(ChatService.QUOTA_EXHAUSTED_MARKER))
        assertTrue("应引导填写自己的 API Key", msg.contains("API Key"))
    }

    // 其他错误（如 500）仍走通用错误文案，不带额度耗尽标记
    @Test
    fun testGeneric5xxStaysGenericError() = runBlocking(Dispatchers.IO) {
        mockWebServer.enqueue(
            MockResponse()
                .setResponseCode(500)
                .setBody("{\"error\":{\"code\":\"2000\",\"message\":\"服务器内部错误\"}}")
        )

        val result = chatService.sendMessage("test").toList()
        assertTrue("5xx 应输出通用错误文案", result.isNotEmpty())
        assertTrue("应包含错误前缀", result.first().contains("错误"))
        assertFalse("5xx 不应被识别为额度耗尽", result.first().contains(ChatService.QUOTA_EXHAUSTED_MARKER))
    }
}
