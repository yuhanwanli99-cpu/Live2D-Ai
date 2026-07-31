package com.live2dai.plugin.agenttools

import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * [SendNotificationReplyTool] 测试：
 * - `requiresConfirmation == true`（验收标准 #5：危险操作必须标记确认，编译期属性断言）
 * - 参数校验：缺失/非法参数返回清晰错误而非崩溃（验收标准 #2）
 * - 桥接委托：参数正确传给 [NotificationReplyBridge]，其结果原样返回
 * - 优雅降级：桥接未注入（null）时返回明确错误（R2）
 */
class SendNotificationReplyToolTest {

    // ── 元数据（验收标准 #5） ──

    @Test
    fun requiresConfirmation_isTrue() {
        val tool = SendNotificationReplyTool(null)
        assertTrue("代替用户真实发送消息，必须标记 requiresConfirmation=true", tool.requiresConfirmation)
    }

    @Test
    fun metadata_nameAndSchema() {
        val tool = SendNotificationReplyTool(null)
        assertEquals("send_notification_reply", tool.name)

        val schema = Json.parseToJsonElement(tool.parametersSchema).jsonObject
        val required = schema["required"]?.jsonArray?.map { it.jsonPrimitive.content }
        assertEquals(listOf("app_name", "reply_text"), required)
        assertTrue(schema.containsKey("properties"))
    }

    // ── 参数校验（验收标准 #2：非法参数返回错误，不崩溃） ──

    @Test
    fun execute_returnsErrorForInvalidArgs_withoutInvokingBridge() = runBlocking {
        var bridgeCalled = false
        val tool = SendNotificationReplyTool(
            NotificationReplyBridge { _, _ ->
                bridgeCalled = true
                """{"result":"ok"}"""
            }
        )

        assertTrue(tool.execute("{}").contains("参数不合法"))
        assertTrue(tool.execute("not-json").contains("参数不合法"))
        assertTrue(tool.execute("""{"app_name":"微信"}""").contains("参数不合法"))
        assertTrue(tool.execute("""{"app_name":"","reply_text":"好的"}""").contains("参数不合法"))
        assertTrue(tool.execute("""{"app_name":"微信","reply_text":""}""").contains("参数不合法"))
        assertTrue(tool.execute("""{"app_name":123,"reply_text":"好的"}""").contains("参数不合法"))
        assertTrue("参数非法时不应调用桥接", !bridgeCalled)
    }

    @Test
    fun execute_returnsErrorWhenBridgeNotInjected() = runBlocking {
        val tool = SendNotificationReplyTool(null)

        val result = tool.execute("""{"app_name":"微信","reply_text":"好的"}""")

        assertTrue("未注入桥接时应优雅降级", result.contains("未启用"))
    }

    // ── 桥接委托 ──

    @Test
    fun execute_delegatesArgsToBridgeAndReturnsResult() = runBlocking {
        var capturedApp: String? = null
        var capturedReply: String? = null
        val tool = SendNotificationReplyTool(
            NotificationReplyBridge { app, reply ->
                capturedApp = app
                capturedReply = reply
                """{"result":"已发送"}"""
            }
        )

        val result = tool.execute("""{"app_name":"微信","reply_text":"马上到"}""")

        assertEquals("微信", capturedApp)
        assertEquals("马上到", capturedReply)
        assertEquals("""{"result":"已发送"}""", result)
    }

    @Test
    fun execute_passesBridgeErrorBackAsIs() = runBlocking {
        val tool = SendNotificationReplyTool(
            NotificationReplyBridge { _, _ -> """{"error":"请先在系统设置中开启通知访问权限"}""" }
        )

        val result = tool.execute("""{"app_name":"微信","reply_text":"好的"}""")

        assertTrue("桥接的引导性错误应原样返回（R2）", result.contains("通知访问权限"))
    }

    // ── 纯逻辑：参数解析 ──

    @Test
    fun parseSendNotificationReplyArgs_valid() {
        assertEquals(
            "微信" to "好的",
            parseSendNotificationReplyArgs("""{"app_name":"微信","reply_text":"好的"}""")
        )
        assertEquals(
            "微信" to "好的",
            parseSendNotificationReplyArgs("""{"app_name":" 微信 ","reply_text":"好的"}""")
        )
    }

    @Test
    fun parseSendNotificationReplyArgs_invalid() {
        assertNull(parseSendNotificationReplyArgs("not-json"))
        assertNull(parseSendNotificationReplyArgs("{}"))
        assertNull(parseSendNotificationReplyArgs("""{"app_name":"微信"}"""))
        assertNull(parseSendNotificationReplyArgs("""{"reply_text":"好的"}"""))
        assertNull(parseSendNotificationReplyArgs("""{"app_name":"","reply_text":"好的"}"""))
        assertNull(parseSendNotificationReplyArgs("""{"app_name":"微信","reply_text":""}"""))
        assertNull(parseSendNotificationReplyArgs("""{"app_name":123,"reply_text":"好的"}"""))
    }
}
