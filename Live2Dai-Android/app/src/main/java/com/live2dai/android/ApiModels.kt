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

// ── 请求侧：工具定义（序列化为 LLM API 格式） ──

/**
 * 单个函数的 OpenAI 兼容描述，嵌入 [ToolDef] 发送给 LLM。
 * [parameters] 是 JSON Schema（参数定义由调用方构造）。
 */
@Serializable
data class FunctionSpec(
    val name: String,
    val description: String,
    val parameters: JsonObject,  // JSON Schema 参数定义
    val strict: Boolean = false  // DeepSeek strict mode（Beta，默认关闭）
)

/**
 * OpenAI 兼容的 tools[] 数组元素。
 */
@Serializable
data class ToolDef(
    val type: String = "function",
    val function: FunctionSpec
)

// ── 响应侧：工具调用结果 ──

/**
 * LLM 返回的 tool_call 中的 function 部分。
 * [arguments] 是 LLM 生成的 JSON 字符串（需自行解析）。
 */
@Serializable
data class ToolCallFunction(
    val name: String,
    val arguments: String  // JSON 字符串，由 LLM 生成
)

/**
 * LLM 返回的单个工具调用。
 */
@Serializable
data class ToolCall(
    val id: String,
    val type: String = "function",
    val function: ToolCallFunction
)

@Serializable
data class ChatMessage(
    val role: String = "assistant",  // "system", "user", "assistant", "tool"
    val content: String? = null,     // nullable: reasoning models omit content during chain-of-thought
    val tool_calls: List<ToolCall>? = null,   // assistant 消息可能携带工具调用
    val tool_call_id: String? = null          // tool 消息携带调用 ID（关联原 tool_call）
)

@Serializable
data class ChatRequest(
    val model: String = "deepseek-chat",
    val messages: List<ChatMessage>,
    val temperature: Double = 0.8,
    val max_tokens: Int = 300,
    val stream: Boolean = true,
    val tools: List<ToolDef>? = null,       // 工具列表（null = 不发送，无工具时行为与旧版一致）
    val tool_choice: String? = null         // "auto"/"none"/"required"（默认 null = tools 存在时 auto）
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
