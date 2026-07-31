package com.live2dai.plugin.agenttools

import android.content.Context
import android.content.Intent
import android.provider.AlarmClock
import com.live2dai.pluginsdk.ToolDefinition
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/**
 * "set_alarm" 工具：通过系统 [AlarmClock.ACTION_SET_ALARM] Intent 设置闹钟/提醒。
 *
 * 风险等级：低——系统会弹出闹钟设置确认页，天然有系统级二次确认，
 * 因此 [requiresConfirmation] = false（无需插件层再确认一次）。
 * 若设备上没有可处理闹钟设置的 App（resolveActivity 为空），返回明确错误而非崩溃。
 */
class SetAlarmTool(private val context: Context) : ToolDefinition {

    override val name = "set_alarm"
    override val description = "设置一个系统闹钟（会跳转系统闹钟确认页，用户可在系统页确认或取消）"
    override val parametersSchema = """
        {
          "type": "object",
          "properties": {
            "hour": {
              "type": "integer",
              "description": "小时 0-23"
            },
            "minute": {
              "type": "integer",
              "description": "分钟 0-59"
            },
            "label": {
              "type": "string",
              "description": "闹钟备注，可选"
            }
          },
          "required": ["hour", "minute"]
        }
    """.trimIndent()
    override val requiresConfirmation = false  // 系统闹钟页天然二次确认，无需插件层再确认

    override suspend fun execute(argsJson: String): String {
        val (hour, minute, label) = parseSetAlarmArgs(argsJson)
            ?: return """{"error":"参数不合法：需要 hour(0-23) 和 minute(0-59)"}"""
        val intent = buildSetAlarmIntent(hour, minute, label)
        if (intent.resolveActivity(context.packageManager) == null) {
            return """{"error":"设备上没有可处理闹钟设置的应用"}"""
        }
        context.startActivity(intent)
        val time = "${hour.toString().padStart(2, '0')}:${minute.toString().padStart(2, '0')}"
        return """{"result":"已跳转到闹钟设置 $time"}"""
    }
}

/**
 * 解析 [SetAlarmTool.execute] 参数：hour(0-23) / minute(0-59) 必须为整数且在范围内，
 * label 可选（字符串，可为 null）。非法 JSON / 缺失 / 越界时返回 null。
 */
internal fun parseSetAlarmArgs(argsJson: String): Triple<Int, Int, String?>? {
    return try {
        val obj = Json.parseToJsonElement(argsJson).jsonObject
        val hour = obj["hour"]?.jsonPrimitive?.content?.toIntOrNull() ?: return null
        val minute = obj["minute"]?.jsonPrimitive?.content?.toIntOrNull() ?: return null
        if (hour !in 0..23 || minute !in 0..59) return null
        val label = obj["label"]?.jsonPrimitive?.takeIf { it.isString }?.content?.trim()?.takeIf { it.isNotEmpty() }
        Triple(hour, minute, label)
    } catch (_: Exception) {
        null
    }
}

/** 构造系统闹钟 Intent（ACTION_SET_ALARM + hour/minute/message），供 execute 与测试复用。 */
internal fun buildSetAlarmIntent(hour: Int, minute: Int, label: String?): Intent =
    Intent(AlarmClock.ACTION_SET_ALARM).apply {
        putExtra(AlarmClock.EXTRA_HOUR, hour)
        putExtra(AlarmClock.EXTRA_MINUTES, minute)
        label?.let { putExtra(AlarmClock.EXTRA_MESSAGE, it) }
        addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    }
