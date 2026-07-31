package com.live2dai.plugin.vision

import com.live2dai.pluginsdk.ChatContext
import com.live2dai.pluginsdk.ChatHook

/**
 * 对话钩子：每次用户发消息前截取屏幕并通过 GLM-4V 获取描述，注入为附加上下文。
 *
 * 截图不可用（[ScreenshotProvider.capture] 返回 null）、未配置 API Key 或描述失败时
 * 返回 null，core 的 ChatService 会静默跳过——对话流程不受影响。
 */
class VisionDescribeHook(
    private val client: GlmVisionClient,
    private val visionModel: String,
    private val screenshotProvider: ScreenshotProvider,
) : ChatHook {

    override suspend fun beforeSend(
        userMessage: String,
        context: ChatContext,
    ): String? {
        // 未配置 API Key 时不产生任何视觉能力（见 schema: apiKey 为空 → 插件加载但无视觉能力）
        if (client.apiKey.isBlank()) return null
        val bitmap = screenshotProvider.capture() ?: return null
        val description = client.describeImage(bitmap, visionModel, "简要描述当前屏幕内容，50字以内")
        return if (description.isNotEmpty()) {
            "[系统: 当前屏幕内容]\n屏幕截图描述: $description"
        } else null
    }

    override suspend fun afterReceive(assistantMessage: String, emotion: String?, context: ChatContext) {
        // 视觉插件无需处理 afterReceive
    }
}
