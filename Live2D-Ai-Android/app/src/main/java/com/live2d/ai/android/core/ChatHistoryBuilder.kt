package com.live2d.ai.android.core

import com.live2d.ai.android.ChatMessage

/** 对话历史最大轮数（对）。契约 §2.2（refactor-0814）：历史修剪 50 对。 */
const val HISTORY_MAX_PAIRS = 50

/**
 * 对话历史构建器（修剪算法，2C-2 锚 5）。
 *
 * 不变式：
 * - system（persona 注入）恒在首位；
 * - 超出 [maxPairs] 对时丢弃最旧对话对，保留最近 N 对；
 * - 保序（takeLast 语义，顺序不变）；
 * - 短历史不误剪（未超限时不裁剪）。
 */
class ChatHistoryBuilder(private val maxPairs: Int = HISTORY_MAX_PAIRS) {

    private val messages = mutableListOf<ChatMessage>()

    /**
     * 设置/更新 system 消息（persona 注入，恒在首位）。
     * 无 system 时插入首位；已有 system 时原位替换。
     */
    fun setSystem(prompt: String): ChatHistoryBuilder {
        val system = ChatMessage(role = "system", content = prompt)
        val index = messages.indexOfFirst { it.role == "system" }
        if (index >= 0) {
            messages[index] = system
        } else {
            messages.add(0, system)
        }
        trim()
        return this
    }

    /** 当前完整消息列表（system 首位 + 修剪后的对话对）。 */
    fun build(): List<ChatMessage> = messages.toList()

    /** 追加本轮 user 消息后返回请求用消息列表（system + 历史 + 当前 user，不入库）。 */
    fun withUser(text: String): List<ChatMessage> = buildList {
        addAll(messages)
        add(ChatMessage(role = "user", content = text))
    }

    /** 回复完成后成对落库（user + assistant），自动修剪，返回修剪后的完整列表。 */
    fun addTurn(userText: String, assistantText: String): List<ChatMessage> {
        messages.add(ChatMessage(role = "user", content = userText))
        messages.add(ChatMessage(role = "assistant", content = assistantText))
        trim()
        return build()
    }

    /** 清空对话（保留 system，供 [LlmLink.reset] 复用）。 */
    fun reset() {
        val system = messages.firstOrNull { it.role == "system" }
        messages.clear()
        if (system != null) messages.add(system)
    }

    /**
     * 修剪算法：system 恒在首位；非 system 消息最多保留 [maxPairs] 对
     * （2 × [maxPairs] 条），超出时丢弃最旧、保留最近（保序）；短历史不裁剪。
     */
    private fun trim() {
        val systemIndex = messages.indexOfFirst { it.role == "system" }
        val system = if (systemIndex >= 0) messages[systemIndex] else null
        val dialog = if (systemIndex >= 0) messages.drop(systemIndex + 1) else messages.toList()
        val maxDialogMessages = maxPairs * 2
        if (dialog.size <= maxDialogMessages) return
        val kept = dialog.takeLast(maxDialogMessages)
        messages.clear()
        if (system != null) messages.add(system)
        messages.addAll(kept)
    }
}
