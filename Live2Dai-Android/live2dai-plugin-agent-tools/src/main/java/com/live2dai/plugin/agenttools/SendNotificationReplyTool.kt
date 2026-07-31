package com.live2dai.plugin.agenttools

import com.live2dai.pluginsdk.ToolDefinition
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/**
 * "send_notification_reply" 工具：代替用户快捷回复某个应用的最近一条通知消息。
 *
 * ⚠️ 危险操作：[requiresConfirmation] = true（硬约束）——会代替用户发出真实消息，
 * 执行前必须经 core 的确认拦截（`ConfirmationGate`）用户确认 UI 拦截，
 * 用户拒绝时 [execute] 不会被调用（见 `docs/plan-task-plugin-agent-tools.md` §2.1）。
 *
 * 快捷回复能力由 core 注入的 [NotificationReplyBridge] 提供：
 * - 未注入（null）→ 返回清晰错误（功能未启用），不崩溃；
 * - 已注入但用户未授权"通知使用权" → 由桥接实现内部检测并返回引导性错误（R2 优雅降级）。
 */
class SendNotificationReplyTool(
    private val notificationBridge: NotificationReplyBridge?,
) : ToolDefinition {

    override val name = "send_notification_reply"
    override val description = "代替用户快捷回复某个应用的最近一条通知消息（会真实发送给聊天对象，执行前需用户确认）"
    override val parametersSchema = """
        {
          "type": "object",
          "properties": {
            "app_name": {
              "type": "string",
              "description": "目标应用名称，如「微信」"
            },
            "reply_text": {
              "type": "string",
              "description": "回复内容"
            }
          },
          "required": ["app_name", "reply_text"]
        }
    """.trimIndent()
    override val requiresConfirmation = true  // 硬约束：真实发送消息前必须经用户确认

    override suspend fun execute(argsJson: String): String {
        val (appName, replyText) = parseSendNotificationReplyArgs(argsJson)
            ?: return """{"error":"参数不合法：需要 app_name 和 reply_text 两个字符串参数"}"""
        val bridge = notificationBridge
            ?: return """{"error":"通知快捷回复功能未启用（core 未注入 NotificationReplyBridge）"}"""
        return bridge.reply(appName, replyText)
    }
}

/**
 * 解析 [SendNotificationReplyTool.execute] 参数中的 `app_name` 与 `reply_text`
 * （两个都必须是字符串且非空）。非法 JSON / 缺失 / 类型错误时返回 null。
 */
internal fun parseSendNotificationReplyArgs(argsJson: String): Pair<String, String>? {
    return try {
        val obj = Json.parseToJsonElement(argsJson).jsonObject
        val appName = obj["app_name"]?.jsonPrimitive?.takeIf { it.isString }?.content?.trim()?.takeIf { it.isNotEmpty() }
            ?: return null
        val replyText = obj["reply_text"]?.jsonPrimitive?.takeIf { it.isString }?.content?.takeIf { it.isNotEmpty() }
            ?: return null
        appName to replyText
    } catch (_: Exception) {
        null
    }
}
