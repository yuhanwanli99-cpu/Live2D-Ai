package com.live2d.ai.android.core

import kotlinx.coroutines.flow.Flow

/**
 * LLM 链路接缝（契约 refactor-0814 §2.2 锁定：`send(text): Flow<LlmEvent>` 冷流、单消费者；`reset()` 清上下文）。
 *
 * 实现归 2C-2（ChatService 裁剪）；本文件仅声明接口签名。
 */
interface LlmLink {
    /**
     * 发起一次 LLM 请求并返回冷事件流（单消费者）。
     * 语义：无自动重试；429/额度错误分类 + 中文文案；历史修剪 50 对；persona 注入。
     */
    fun send(text: String): Flow<LlmEvent>

    /** 清空上下文。 */
    fun reset()
}

/**
 * LLM 链路事件（由 [LoopCoordinator] 映射为 [LoopEvent] 喂入状态机）。
 */
sealed interface LlmEvent {
    /** 流式增量。 */
    data class Delta(val fragment: String) : LlmEvent

    /** 回复完成（fullText 含 emotion 标签原文，剥离/净化归 EmotionText 层）。 */
    data class Completed(val fullText: String, val emotion: String?) : LlmEvent

    /** 失败（message 为中文友好文案；quotaExhausted 供额度提示）。 */
    data class Failed(val message: String, val quotaExhausted: Boolean) : LlmEvent
}
