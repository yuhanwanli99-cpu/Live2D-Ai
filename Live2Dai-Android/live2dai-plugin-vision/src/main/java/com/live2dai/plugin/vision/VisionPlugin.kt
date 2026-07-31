package com.live2dai.plugin.vision

import com.live2dai.pluginsdk.Live2DaiPlugin
import com.live2dai.pluginsdk.PluginHost

/**
 * 视觉插件：向 core 注册「截图描述」[ChatHook] 与「capture_screen」[ToolDefinition]。
 *
 * 双能力形态（见 docs/plan-task-extract-vision.md §1.2）：
 * - ChatHook（立即可用）：每次用户发消息前自动截图→描述→注入上下文
 * - ToolDefinition（预留）：LLM 主动调用 capture_screen 工具时执行（待 tool-calling loop 接入）
 *
 * 截图能力由宿主通过 [VisionPluginConfig.screenshotProvider] 注入，插件自身不持有。
 */
class VisionPlugin(private val config: VisionPluginConfig) : Live2DaiPlugin {
    override val id: String = "vision"
    override val version: String = "1.0.0"

    override fun onLoad(host: PluginHost) {
        // 注册 ChatHook（Wave 1 立即可用）
        host.registerHook(
            VisionDescribeHook(
                client = GlmVisionClient(config.apiKey, config.baseUrl),
                visionModel = config.visionModel,
                screenshotProvider = config.screenshotProvider,
            )
        )
        // 注册 ToolDefinition（Wave 2 tool-calling 启用后可用）
        host.registerTool(
            ScreenCaptureTool(
                client = GlmVisionClient(config.apiKey, config.baseUrl),
                visionModel = config.visionModel,
                screenshotProvider = config.screenshotProvider,
            )
        )
    }

    override fun onUnload() {
        // OkHttp client 由 GC 回收，无需显式关闭
    }
}
