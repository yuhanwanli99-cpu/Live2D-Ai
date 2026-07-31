package com.live2dai.plugin.vision

import com.live2dai.pluginsdk.ToolDefinition
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/**
 * "capture_screen" 工具：截取当前屏幕并理解屏幕内容（GLM-4V）。
 *
 * 参数 JSON Schema 见 [parametersSchema]。截图无安全风险，[requiresConfirmation] = false。
 * 注册后待 core 的 ChatService 接入 tool-calling loop（Wave 2）即可被 LLM 主动调用。
 */
class ScreenCaptureTool(
    private val client: GlmVisionClient,
    private val visionModel: String,
    private val screenshotProvider: ScreenshotProvider,
) : ToolDefinition {

    override val name = "capture_screen"
    override val description = "截取当前屏幕并理解屏幕内容。可用于查看用户正在看什么。"
    override val parametersSchema = """
        {
          "type": "object",
          "properties": {
            "prompt": {
              "type": "string",
              "description": "分析截图时的自定义提示词，默认简要描述屏幕内容"
            }
          },
          "required": []
        }
    """.trimIndent()
    override val requiresConfirmation = false    // 截图无安全风险，无需确认

    override suspend fun execute(argsJson: String): String {
        val prompt = parsePrompt(argsJson) ?: "请描述当前屏幕内容"
        if (client.apiKey.isBlank()) return """{"error": "未配置 Zhipu API Key"}"""
        val bitmap = screenshotProvider.capture()
            ?: return """{"error": "无法截取屏幕"}"""
        return client.describeImage(bitmap, visionModel, prompt)
    }

    /** 从 LLM 传入的参数 JSON 中解析可选的 prompt 字段；缺失/非法时返回 null（使用默认提示词） */
    private fun parsePrompt(argsJson: String): String? = try {
        val obj = Json.parseToJsonElement(argsJson).jsonObject
        obj["prompt"]?.jsonPrimitive?.contentOrNull?.takeIf { it.isNotBlank() }
    } catch (_: Exception) {
        null
    }
}
