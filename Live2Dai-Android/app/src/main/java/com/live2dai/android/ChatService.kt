package com.live2dai.android

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import okhttp3.*
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.BufferedReader
import java.io.InputStreamReader

/**
 * Streaming chat service that calls DeepSeek API directly (no gateway).
 * Uses SSE (Server-Sent Events) for streaming response.
 * Reads persona from [PersonaConfig] for system prompt injection and model name.
 *
 * DeepSeek Chat: non-reasoning model for fast VTuber chat.
 */
class ChatService(
    /**
     * LLM Provider — 可以是 DeepSeek 云端或本地 LLM (Ollama/vLLM)。
     * 通过 LLMProviderManager 获取或由调用方手动构造。
     */
    private val provider: LLMProvider = LLMProvider.deepSeek()
) {

    /**
     * 兼容旧代码的次级构造函数。
     * 用于简化单元测试和已有调用点迁移。
     */
    constructor(apiKey: String, baseUrl: String) : this(
        LLMProvider(
            type = ProviderType.DEEPSEEK,
            baseUrl = baseUrl,
            apiKey = apiKey,
            modelName = PersonaConfig.getModel(),
            displayName = "DeepSeek"
        )
    )
    private val json = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true // Include default values
    }
    private val client = OkHttpClient.Builder()
        .connectTimeout(60, java.util.concurrent.TimeUnit.SECONDS)
        .readTimeout(120, java.util.concurrent.TimeUnit.SECONDS)
        .writeTimeout(60, java.util.concurrent.TimeUnit.SECONDS)
        .build()

    private val mediaType = "application/json; charset=utf-8".toMediaType()

    // Conversation history
    private val messages = mutableListOf<ChatMessage>()

    // 对话历史最大消息对数（防止 token 超限）
    // 每对 (user + assistant) 约占用 200-500 tokens
    // deepseek-v4-flash 上下文窗口 ≈ 128K，保留 100 对也足够安全
    companion object {
        private const val MAX_MESSAGE_PAIRS = 50
    }

    init {
        // Seed with system prompt from persona
        val systemPrompt = PersonaConfig.getSystemPrompt()
        if (systemPrompt.isNotEmpty()) {
            // 追加表情控制提示词，让 LLM 知道可用表情标签
            val emotionInstruction = buildString {
                appendLine("")
                appendLine("## 表情控制")
                appendLine("你可以在回复中使用表情标签来表达情绪，标签不会在语音中读出。")
                appendLine("可用表情关键词: [neutral], [joy], [anger], [sadness], [fear], [surprise], [smirk]")
                appendLine("使用方法: \"[joy] 主人今天心情真好喵~！\"")
                appendLine("注意: 表情关键词放在句首，不会在语音中读出")
            }
            messages.add(ChatMessage(role = "system", content = systemPrompt + emotionInstruction))
        }
    }

    /**
     * Send a user message and receive streaming response as a Flow of text deltas.
     * Direct SSE call to DeepSeek API — no intermediate gateway.
     */
    fun sendMessage(userText: String): Flow<String> = flow {
        if (userText.isBlank()) return@flow

        messages.add(ChatMessage(role = "user", content = userText))

        val modelName = PersonaConfig.getModel()
        val request = ChatRequest(
            model = modelName,
            messages = messages,
            temperature = PersonaConfig.getTemperature(),
            max_tokens = PersonaConfig.getMaxTokens(),
            stream = true
        )

        val requestBody = json.encodeToString(request)
            .toRequestBody(mediaType)

        val httpRequest = Request.Builder()
            .url("${provider.baseUrl}/chat/completions")
            .apply {
                // 仅当 apiKey 非空时添加 Authorization header
                // Ollama 不需要 API Key，此时不传递 Bearer token
                if (provider.apiKey.isNotEmpty()) {
                    addHeader("Authorization", "Bearer ${provider.apiKey}")
                }
            }
            .addHeader("Accept", "text/event-stream")
            .post(requestBody)
            .build()

        val response = client.newCall(httpRequest).execute()
        if (!response.isSuccessful) {
            val errorBody = response.body?.string() ?: "未知错误"
            emit("[错误] API请求失败: ${response.code} - $errorBody")
            return@flow
        }

        // 在 API 调用成功后、读取响应前修剪对话历史
        // 避免单次对话累积过多消息导致 token 超限
        trimConversationHistory()

        val body = response.body ?: return@flow
        val reader = BufferedReader(InputStreamReader(body.byteStream(), "UTF-8"))
        var line: String?
        val assistantContent = StringBuilder()

        while (reader.readLine().also { line = it } != null) {
            val currentLine = line ?: continue

            // SSE format: "data: {...}"
            if (currentLine.startsWith("data: ")) {
                val dataStr = currentLine.removePrefix("data: ").trim()

                // Check for stream end
                if (dataStr == "[DONE]") break

                try {
                    val chunk = json.decodeFromString<ChatResponse>(dataStr)
                    val delta = chunk.choices?.firstOrNull()?.delta?.content ?: ""
                    if (delta.isNotEmpty()) {
                        assistantContent.append(delta)
                        emit(delta)
                    }
                } catch (_: Exception) {
                    // Skip malformed chunks (e.g., rate limit info, keep-alive)
                }
            }
        }

        reader.close()

        // Store assistant response in conversation history
        if (assistantContent.isNotEmpty()) {
            messages.add(ChatMessage(role = "assistant", content = assistantContent.toString()))
        }

        // 再次修剪，确保每次请求后不超过限制
        trimConversationHistory()
    }.flowOn(Dispatchers.IO)

    /**
     * 修剪对话历史：保留 system prompt，然后保留最近的 N 轮对话（user + assistant 对）
     *
     * 在每次 API 调用成功后执行，防止单次对话累积过多消息导致 token 超限。
     * 删除策略：优先丢弃最旧的对话对。
     */
    private fun trimConversationHistory() {
        // 找到 system prompt 的位置
        val systemIndex = messages.indexOfFirst { it.role == "system" }
        val systemMessage = if (systemIndex >= 0) messages[systemIndex] else null

        // 非 system 消息（即 user + assistant 对话对）
        val dialogMessages = if (systemIndex >= 0) {
            messages.drop(systemIndex + 1)
        } else {
            messages.toList()
        }

        // 最多保留 MAX_MESSAGE_PAIRS 对（2 倍条消息）
        val maxDialogMessages = MAX_MESSAGE_PAIRS * 2
        if (dialogMessages.size <= maxDialogMessages) return

        // 裁剪对话：保留最近的 maxDialogMessages 条
        val trimmedDialog = dialogMessages.takeLast(maxDialogMessages)

        messages.clear()
        if (systemMessage != null) {
            messages.add(systemMessage)
        }
        messages.addAll(trimmedDialog)
    }

    /**
     * Reset conversation history but keep the system prompt.
     */
    fun resetConversation() {
        val systemPrompt = PersonaConfig.getSystemPrompt()
        messages.clear()
        if (systemPrompt.isNotEmpty()) {
            val emotionInstruction = buildString {
                appendLine("")
                appendLine("## 表情控制")
                appendLine("你可以在回复中使用表情标签来表达情绪，标签不会在语音中读出。")
                appendLine("可用表情关键词: [neutral], [joy], [anger], [sadness], [fear], [surprise], [smirk]")
                appendLine("使用方法: \"[joy] 主人今天心情真好喵~！\"")
                appendLine("注意: 表情关键词放在句首，不会在语音中读出")
            }
            messages.add(ChatMessage(role = "system", content = systemPrompt + emotionInstruction))
        }
    }
}
