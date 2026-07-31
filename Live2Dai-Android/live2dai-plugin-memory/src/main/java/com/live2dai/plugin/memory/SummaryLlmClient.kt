package com.live2dai.plugin.memory

import com.live2dai.pluginsdk.ChatTurn

/**
 * 摘要生成客户端——插件不自己实现 LLM HTTP 客户端，由 core 注入实现
 * （内部复用现有 `LLMProvider.kt` 的请求逻辑），与 vision 插件的
 * [ScreenshotProvider][com.live2dai.plugin.vision.ScreenshotProvider] 接缝同理：
 * 插件只定义「需要什么能力」的接口，具体实现留给 core，保持插件与厂商 API 解耦。
 *
 * 见 docs/plan-task-plugin-memory.md §2.1 关键设计决策。
 */
fun interface SummaryLlmClient {

    /**
     * 把近期几轮对话浓缩成一条事实摘要。
     *
     * @param recentTurns 近期对话（来自 core 的 `ChatContext.recentMessages` 只读快照）
     * @param personaName 人设名（可让摘要贴合人设语气，如"小喵"）
     * @return 事实摘要文本；返回 null / 空白表示本轮无值得记忆的内容（跳过写入）
     */
    suspend fun summarize(recentTurns: List<ChatTurn>, personaName: String): String?
}
