package com.live2dai.android

import com.live2dai.pluginsdk.ToolDefinition

/**
 * 危险工具执行前的确认拦截门。
 *
 * 这是 core 内部接口（不放入 plugin-sdk），原因：
 * 1. 弹确认框需要 Activity/Compose UI 上下文，属于 core 的 UI 层职责
 * 2. 插件不应持有 Activity 引用（内存泄漏风险，与 ScreenshotProvider 设计原则一致）
 * 3. 确认逻辑在 core 统一生效，保证所有 requiresConfirmation=true 的工具都被拦截
 *
 * 实现者：MainActivity（Compose Dialog）或单元测试（mock 直接返回 true/false）。
 * 未注入 gate（null）时，ChatService 会拒绝所有 requiresConfirmation=true 的工具
 * （安全默认，参见 plan-task-chat-tool-loop.md 不变量 I5）。
 */
fun interface ConfirmationGate {

    /**
     * 请求用户确认执行危险工具。
     *
     * @param tool 待确认的工具定义（可展示 name/description 给用户）
     * @param argsJson LLM 生成的调用参数（可解析后展示关键字段给用户，如"回复内容: xxx"）
     * @return true 表示用户同意执行，false 表示拒绝
     */
    suspend fun confirm(tool: ToolDefinition, argsJson: String): Boolean
}
