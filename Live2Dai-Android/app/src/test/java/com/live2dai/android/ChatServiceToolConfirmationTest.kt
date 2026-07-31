package com.live2dai.android

import com.live2dai.pluginsdk.ToolDefinition
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
 * 验收阻塞项测试（docs/plan-task-plugin-agent-tools.md §7 #3 + R1）：
 * **确认机制无法绕过**——`requiresConfirmation = true` 的工具，在用户拒绝时
 * `execute()` 从未被调用（而不是"调用了但假装失败"），且 LLM 收到"用户拒绝执行"的结果。
 *
 * 覆盖不变量（docs/plan-task-chat-tool-loop.md §7.2）：
 * - I2：gate 返回 false → tool.execute() 从未被调用
 * - I5：gate 为 null → 危险工具被拒绝（安全默认）
 * - 额外：gate 抛异常 → 按拒绝处理（fail-safe），execute 同样不被调用
 *
 * 注：ChatServiceTest 的 T3/T4/T7 也覆盖了部分同构场景；本文件作为独立验收阻塞项，
 * 聚焦确认拦截的不可绕过性，保证该硬约束有专门的验收入口。
 */
class ChatServiceToolConfirmationTest {

    private lateinit var mockWebServer: MockWebServer

    @Before
    fun setUp() {
        mockWebServer = MockWebServer()
        mockWebServer.start()
    }

    @After
    fun tearDown() {
        mockWebServer.shutdown()
    }

    /** 可控的危险工具：记录 execute 调用次数。 */
    private class FakeDangerousTool(
        override val name: String = "send_notification_reply",
    ) : ToolDefinition {
        override val description = "测试危险工具"
        override val parametersSchema = """{"type":"object","properties":{}}"""
        override val requiresConfirmation = true
        var executeCount = 0
        override suspend fun execute(argsJson: String): String {
            executeCount++
            return """{"result":"executed"}"""
        }
    }

    /** 可控的确认门。 */
    private class FakeGate(
        private val approved: Boolean = true,
        private val throwOnConfirm: Boolean = false,
    ) : ConfirmationGate {
        var confirmCount = 0
        var lastToolName: String? = null
        var lastArgs: String? = null

        override suspend fun confirm(tool: ToolDefinition, argsJson: String): Boolean {
            confirmCount++
            lastToolName = tool.name
            lastArgs = argsJson
            if (throwOnConfirm) throw IllegalStateException("gate boom")
            return approved
        }
    }

    private fun serviceWith(tools: List<ToolDefinition>, gate: ConfirmationGate?): ChatService =
        ChatService(
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

    private fun toolCallResponse(toolName: String, callId: String, args: String = "{}"): String {
        val escapedArgs = args.replace("\\", "\\\\").replace("\"", "\\\"")
        return """
            {"choices":[{"index":0,"message":{"role":"assistant","content":null,
              "tool_calls":[{"id":"$callId","type":"function","function":{"name":"$toolName","arguments":"$escapedArgs"}}]
            },"finish_reason":"tool_calls"}]}
        """.trimIndent()
    }

    private fun textResponse(text: String): String =
        """{"choices":[{"index":0,"message":{"role":"assistant","content":"$text"},"finish_reason":"stop"}]}"""

    // ── 阻塞项：用户拒绝 → execute 从未被调用 + LLM 收到拒绝结果 ──

    @Test
    fun userRefuses_dangerousToolNeverExecutes_andLLMReceivesRefusal() = runBlocking(Dispatchers.IO) {
        val tool = FakeDangerousTool()
        val gate = FakeGate(approved = false)
        val service = serviceWith(listOf(tool), gate)

        mockWebServer.enqueue(
            MockResponse().setResponseCode(200).setBody(toolCallResponse("send_notification_reply", "call_1"))
        )
        mockWebServer.enqueue(MockResponse().setResponseCode(200).setBody(textResponse("好的，我不会替你发送")))

        val result = service.sendMessage("帮我回复微信").toList()

        // 硬约束：execute 从未被调用
        assertEquals("用户拒绝时 execute() 必须从未被调用", 0, tool.executeCount)
        // 确认门确实被询问了一次
        assertEquals(1, gate.confirmCount)
        assertEquals("send_notification_reply", gate.lastToolName)
        // 最终回复正常
        assertEquals(listOf("好的，我不会替你发送"), result)

        // LLM 第二轮收到的 tool 结果必须包含"用户拒绝执行"
        mockWebServer.takeRequest()
        val body2 = mockWebServer.takeRequest().body.readUtf8()
        assertTrue("LLM 应收到拒绝结果而非执行结果", body2.contains("用户拒绝执行"))
        assertTrue("不应出现真实执行结果", !body2.contains("executed"))
    }

    // ── 用户同意 → 正常执行 ──

    @Test
    fun userApproves_dangerousToolExecutes() = runBlocking(Dispatchers.IO) {
        val tool = FakeDangerousTool()
        val gate = FakeGate(approved = true)
        val service = serviceWith(listOf(tool), gate)

        mockWebServer.enqueue(
            MockResponse().setResponseCode(200).setBody(toolCallResponse("send_notification_reply", "call_2"))
        )
        mockWebServer.enqueue(MockResponse().setResponseCode(200).setBody(textResponse("已替你发送")))

        val result = service.sendMessage("帮我回复微信").toList()

        assertEquals(1, gate.confirmCount)
        assertEquals("用户同意后应执行一次", 1, tool.executeCount)
        assertEquals(listOf("已替你发送"), result)

        // LLM 收到的是真实执行结果
        mockWebServer.takeRequest()
        val body2 = mockWebServer.takeRequest().body.readUtf8()
        assertTrue(body2.contains("executed"))
    }

    // ── 安全默认：gate 为 null → 危险工具一律拒绝 ──

    @Test
    fun nullGate_rejectsDangerousTool_safeDefault() = runBlocking(Dispatchers.IO) {
        val tool = FakeDangerousTool()
        val service = serviceWith(listOf(tool), gate = null)

        mockWebServer.enqueue(
            MockResponse().setResponseCode(200).setBody(toolCallResponse("send_notification_reply", "call_3"))
        )
        mockWebServer.enqueue(MockResponse().setResponseCode(200).setBody(textResponse("已拒绝")))

        val result = service.sendMessage("帮我回复微信").toList()

        assertEquals("未注入确认门时危险工具不得执行", 0, tool.executeCount)
        assertEquals(listOf("已拒绝"), result)

        mockWebServer.takeRequest()
        val body2 = mockWebServer.takeRequest().body.readUtf8()
        assertTrue(body2.contains("用户拒绝执行"))
    }

    // ── fail-safe：确认门抛异常 → 按拒绝处理，execute 不被调用 ──

    @Test
    fun gateThrows_treatedAsRejection_executeNeverCalled() = runBlocking(Dispatchers.IO) {
        val tool = FakeDangerousTool()
        val gate = FakeGate(approved = true, throwOnConfirm = true)
        val service = serviceWith(listOf(tool), gate)

        mockWebServer.enqueue(
            MockResponse().setResponseCode(200).setBody(toolCallResponse("send_notification_reply", "call_4"))
        )
        mockWebServer.enqueue(MockResponse().setResponseCode(200).setBody(textResponse("确认失败，已终止")))

        val result = service.sendMessage("帮我回复微信").toList()

        assertEquals("确认回调异常必须按拒绝处理（fail-safe）", 0, tool.executeCount)
        assertEquals(listOf("确认失败，已终止"), result)

        mockWebServer.takeRequest()
        val body2 = mockWebServer.takeRequest().body.readUtf8()
        assertTrue(body2.contains("用户拒绝执行"))
    }

    // ── 确认门能拿到工具名与调用参数（UI 展示所需） ──

    @Test
    fun gateReceivesToolNameAndArgsForUiDisplay() = runBlocking(Dispatchers.IO) {
        val tool = FakeDangerousTool()
        val gate = FakeGate(approved = false)
        val service = serviceWith(listOf(tool), gate)

        mockWebServer.enqueue(
            MockResponse().setResponseCode(200).setBody(
                toolCallResponse("send_notification_reply", "call_5", args = """{"app_name":"微信","reply_text":"马上到"}""")
            )
        )
        mockWebServer.enqueue(MockResponse().setResponseCode(200).setBody(textResponse("好")))

        service.sendMessage("帮我回复微信").toList()

        assertEquals(1, gate.confirmCount)
        assertEquals("send_notification_reply", gate.lastToolName)
        assertEquals("""{"app_name":"微信","reply_text":"马上到"}""", gate.lastArgs)
    }
}
