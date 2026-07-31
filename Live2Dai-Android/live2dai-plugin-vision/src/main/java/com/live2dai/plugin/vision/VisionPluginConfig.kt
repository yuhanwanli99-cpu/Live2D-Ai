package com.live2dai.plugin.vision

import android.graphics.Bitmap

/**
 * 截图提供者——插件不自己持有截图能力，由宿主（core）注入实现。
 *
 * 截图需要 MediaProjection / AccessibilityService 等系统权限，属于宿主 UI 层
 * 的责任范围；插件作为纯逻辑层只消费截图结果。`capture()` 返回 null 时，
 * 插件会静默跳过本轮注入，对话流程不受影响（见 docs/plan-task-extract-vision.md §2.3）。
 */
fun interface ScreenshotProvider {
    suspend fun capture(): Bitmap?
}

/**
 * 插件构造参数（从 core 注入），见 docs/plan-task-extract-vision.schema.json。
 *
 * `apiKey` 为空时插件正常加载但不产生任何视觉能力（hook 静默跳过、工具返回错误）。
 */
data class VisionPluginConfig(
    /** Zhipu (智谱) API Key，用于调用 GLM-4V 视觉模型 */
    val apiKey: String,
    /** 视觉模型名称（如 glm-4.6v），由 core 的 `PersonaConfig.getVisionModel()` 提供 */
    val visionModel: String,
    /** 截图能力提供者，由 core 注入（如 `AndroidScreenshotProvider`） */
    val screenshotProvider: ScreenshotProvider,
    /** GLM API 基地址，通常无需修改 */
    val baseUrl: String = "https://open.bigmodel.cn/api/paas/v4",
)
