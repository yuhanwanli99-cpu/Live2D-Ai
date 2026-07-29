package com.live2dai.android

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/**
 * Data models for DeepSeek Chat API (OpenAI-compatible).
 * Used for non-streaming and SSE streaming requests.
 */

@Serializable
data class ChatMessage(
    val role: String = "assistant",  // "system", "user", "assistant"
    val content: String? = null       // nullable: reasoning models omit content during chain-of-thought
)

@Serializable
data class ChatRequest(
    val model: String = "deepseek-chat",
    val messages: List<ChatMessage>,
    val temperature: Double = 0.8,
    val max_tokens: Int = 300,
    val stream: Boolean = true
)

@Serializable
data class ChatChoice(
    val index: Int = 0,
    val delta: ChatMessage? = null,
    val message: ChatMessage? = null,
    val finish_reason: String? = null
)

@Serializable
data class ChatResponse(
    val id: String? = null,
    @kotlinx.serialization.SerialName("object")
    val object_type: String? = null,
    val created: Long? = null,
    val model: String? = null,
    val choices: List<ChatChoice>? = null
)

/**
 * Parsed SSE chunk from DeepSeek streaming response.
 * Lines look like: data: {"choices":[{"delta":{"content":"喵~"}}]}
 */
data class SSEDelta(
    val content: String = ""
)
