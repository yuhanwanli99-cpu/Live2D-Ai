package com.live2dai.pluginsdk

/**
 * 对话主循环的事件钩子（对应 PiAgent Extension 的事件监听机制）。
 *
 * 多个插件的 hook 按**注册顺序串行执行**（不并行），避免多插件同时改写共享的
 * 对话上下文产生竞态。任何一个 hook 抛异常，core 应该捕获并跳过该 hook（不应让
 * 一个插件的 bug 打断整条对话链路），具体容错策略由 core 端的 hook 调度器实现。
 */
interface ChatHook {

    /**
     * 用户消息发出前调用，可用于注入上下文（如记忆摘要、屏幕截图描述）。
     *
     * @param userMessage 用户原始输入
     * @param context 当前对话上下文（只读快照）
     * @return 非空时，返回值会作为「附加上下文」拼接进本轮请求（不会替换用户原始输入本身），
     *         多个 hook 的返回值按注册顺序依次追加；返回 null 表示本 hook 本轮不注入任何内容。
     */
    suspend fun beforeSend(userMessage: String, context: ChatContext): String?

    /**
     * 收到 LLM 完整回复后调用（流式结束后，而非每个 delta 都调用一次）。
     * 典型用途：记忆插件在这里把本轮对话写入长期记忆。
     *
     * @param assistantMessage LLM 本轮完整回复文本（已剥离 `[emotion]` 标签）
     * @param emotion 本轮解析出的表情标签，可能为 null
     * @param context 当前对话上下文（只读快照），与 [beforeSend] 传入的是同一份快照，
     *                可供插件读取近期对话用于摘要生成等场景
     */
    suspend fun afterReceive(assistantMessage: String, emotion: String?, context: ChatContext)
}

/**
 * 传给 [ChatHook] 的只读对话上下文快照。
 *
 * 刻意只暴露最近若干条消息而非完整历史引用，防止插件意外修改 core 内部的
 * 对话状态（`ChatService` 内部的 `messages` 列表对插件不可见）。
 */
data class ChatContext(
    val recentMessages: List<ChatTurn>,
    val personaName: String,
)

data class ChatTurn(
    val role: String, // "user" | "assistant" | "system"
    val content: String,
)
