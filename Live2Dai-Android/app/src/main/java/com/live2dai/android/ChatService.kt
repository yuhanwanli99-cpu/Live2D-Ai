package com.live2dai.android

import com.live2dai.pluginsdk.ChatContext
import com.live2dai.pluginsdk.ChatHook
import com.live2dai.pluginsdk.ChatTurn
import com.live2dai.pluginsdk.ToolDefinition
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject
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
 *
 * 工具调用循环（OpenAI 兼容 function-calling）：
 * - 无工具注册（[tools] 为空）→ 快路径：SSE 流式，行为与旧版完全一致
 * - 有工具注册 → 慢路径：非流式请求（stream=false）携带 tools，收到 tool_calls 后
 *   查表执行（requiresConfirmation=true 的工具先过 [ConfirmationGate] 确认），
 *   结果回填为 tool 消息继续请求，最多 [MAX_TOOL_ROUNDS] 轮
 */
class ChatService(
    /**
     * LLM Provider — 可以是 DeepSeek 云端或本地 LLM (Ollama/vLLM)。
     * 通过 LLMProviderManager 获取或由调用方手动构造。
     */
    private val provider: LLMProvider = LLMProvider.deepSeek(),
    /**
     * 插件注册的对话钩子（来自 [PluginManager.hooks]）。默认空列表，
     * 因此不接插件时行为与重构前完全一致——这是保持向后兼容的关键。
     */
    private val hooks: List<ChatHook> = emptyList(),
    /**
     * 插件注册的工具表（来自 [PluginManager.tools] 活引用）。默认空列表，
     * 为空时发送的请求体不含 tools 字段、保持流式，行为与旧版完全一致。
     */
    private val tools: List<ToolDefinition> = emptyList(),
    /**
     * 危险工具确认拦截门（core 内部接口，由 MainActivity 注入 Compose 实现）。
     * null = 拒绝所有 requiresConfirmation=true 的工具（安全默认）。
     */
    private val confirmationGate: ConfirmationGate? = null
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
        ),
        hooks = emptyList()
    )

    @OptIn(ExperimentalSerializationApi::class)
    private val json = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true // Include default values
        // 序列化时省略 null 字段：无工具时请求体不含 "tools"（OpenAI 兼容 API 对
        // 缺失字段与 null 等效，但显式 null 会污染部分严格实现），同时避免
        // tool_calls/content 为 null 时发送无意义字段
        explicitNulls = false
    }
    private val client = OkHttpClient.Builder()
        .connectTimeout(60, java.util.concurrent.TimeUnit.SECONDS)
        .readTimeout(120, java.util.concurrent.TimeUnit.SECONDS)
        .writeTimeout(60, java.util.concurrent.TimeUnit.SECONDS)
        .build()

    private val mediaType = "application/json; charset=utf-8".toMediaType()

    // Conversation history
    private val messages = mutableListOf<ChatMessage>()

    companion object {
        // 对话历史最大消息对数（防止 token 超限）
        // 每对 (user + assistant) 约占用 200-500 tokens
        // deepseek-v4-flash 上下文窗口 ≈ 128K，保留 100 对也足够安全
        private const val MAX_MESSAGE_PAIRS = 50

        // 单次 sendMessage() 中最大工具调用轮次（防止 tool-calling 死循环）
        private const val MAX_TOOL_ROUNDS = 8

        // ── HTTP 错误分类与友好提示 ───────────────────────────────────────

        /** 限流（429）提示：可稍后重试（免费模型个人账号限 1 并发） */
        const val RATE_LIMIT_MESSAGE = "[提示] 请求过于频繁，请稍后重试（免费模型限 1 并发）"

        /**
         * 共享免费 Key 额度耗尽/欠费类错误的前缀标记（UI 层据此识别并给出
         * 「前往设置填写自己的 API Key」入口；展示时该前缀会被剥离）。
         * 这类错误通常不应重试，应引导用户切换到自己的 Key。
         */
        const val QUOTA_EXHAUSTED_MARKER = "[QUOTA_EXHAUSTED]"

        /** 额度耗尽/欠费类错误的友好中文提示（不含重试语义） */
        const val QUOTA_EXHAUSTED_MESSAGE =
            "当前使用的是官方免费额度（GLM-4.7-Flash），免费额度已用尽或服务繁忙。" +
                "可前往「设置 → API 设置」填写你自己的智谱 API Key 继续使用。"

        // 额度耗尽/欠费类错误的识别关键词（智谱错误响应 body 中可能出现的标记）。
        // 除 HTTP 403 外，命中这些关键词也视为额度耗尽，避免因错误码差异漏判。
        private val QUOTA_KEYWORDS = listOf(
            "quota", "balance", "insufficient", "exhausted", "overdue", "frozen",
            "余额", "额度", "欠费", "冻结", "充值", "已用尽",
        )

        /**
         * 把一次非成功 HTTP 响应分类为可展示的友好/错误文案。
         *
         * - 429  → 限流：稍后重试
         * - 403 或 body 命中额度耗尽关键词 → 共享 Key 额度耗尽：引导用户换自己的 Key（不重试）
         * - 其他 → 通用错误文案
         */
        fun httpErrorMessage(code: Int, body: String): String = when {
            code == 429 -> RATE_LIMIT_MESSAGE
            code == 403 || QUOTA_KEYWORDS.any { body.contains(it, ignoreCase = true) } ->
                "$QUOTA_EXHAUSTED_MARKER$QUOTA_EXHAUSTED_MESSAGE"
            else -> "[错误] API请求失败: $code - $body"
        }
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
     *
     * 有工具注册时走非流式 tool-calling 循环（整段文本一次发射），
     * 无工具注册时保持 SSE 流式逐 delta 发射（向后兼容）。
     */
    fun sendMessage(userText: String): Flow<String> = flow {
        if (userText.isBlank()) return@flow

        // ── Step 1: beforeSend hooks：按注册顺序串行执行，非空返回值依次追加为附加上下文 ──
        // 单个 hook 抛异常不应打断整条对话链路，因此逐个 try/catch，跳过失败的 hook。
        val extraContext = StringBuilder()
        val chatContext = ChatContext(
            recentMessages = messages.takeLast(10).map { ChatTurn(it.role, it.content ?: "") },
            personaName = PersonaConfig.getPersonaName()
        )
        for (hook in hooks) {
            try {
                hook.beforeSend(userText, chatContext)?.let { extraContext.appendLine(it) }
            } catch (_: Exception) {
                // 插件钩子失败不应影响核心对话流程，静默跳过
            }
        }
        val effectiveUserText = if (extraContext.isEmpty()) {
            userText
        } else {
            "$userText\n\n[插件附加上上下文]\n$extraContext"
        }

        messages.add(ChatMessage(role = "user", content = effectiveUserText))

        // ── Step 2: 快/慢路径选择 ──
        // tools 为空 → 快路径（stream=true，SSE 流式，与旧版完全一致）
        // tools 非空 → 慢路径（stream=false，非流式 tool-calling 循环）
        val requestedTools = buildToolsForRequest()
        if (requestedTools == null) {
            // 快路径：流式请求，逐 delta 发射；完整回复触发 afterReceive hooks
            streamResponse { emit(it) }?.let { finalText ->
                runAfterReceive(finalText, chatContext)
            }
        } else {
            // 慢路径：工具调用循环
            runToolLoop(requestedTools, chatContext) { emit(it) }
        }
    }.flowOn(Dispatchers.IO)

    /**
     * 快路径：无工具注册时的 SSE 流式请求（行为与旧版完全一致）。
     *
     * 逐 delta 发射文本片段，返回完整回复文本；API 错误或空回复返回 null。
     */
    private suspend fun streamResponse(
        onDelta: suspend (String) -> Unit
    ): String? {
        // 请求模型取 provider.modelName：DeepSeek 场景与 PersonaConfig.getModel() 一致
        // （deepSeek() 工厂以人设模型名构造），智谱场景则为 glm-4.7-flash。
        val modelName = provider.modelName
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
            onDelta(httpErrorMessage(response.code, errorBody))
            return null
        }

        // 在 API 调用成功后、读取响应前修剪对话历史
        // 避免单次对话累积过多消息导致 token 超限
        trimConversationHistory()

        val body = response.body ?: return null
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
                        onDelta(delta)
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

        return if (assistantContent.isNotEmpty()) assistantContent.toString() else null
    }

    /**
     * 慢路径：有工具注册时的 tool-calling 循环（非流式请求）。
     *
     * 状态机（详见 docs/plan-task-chat-tool-loop.md §7.1）：
     * 构建请求 → HTTP POST → 解析响应 → 检测 tool_calls
     *   ├─ 有 tool_calls（round < MAX_TOOL_ROUNDS）→ 执行工具 → 回填结果 → 继续循环
     *   ├─ 有 tool_calls（round >= MAX_TOOL_ROUNDS）→ 错误退出（循环上限）
     *   └─ 无 tool_calls → 提取文本 → 退出循环 → 发射最终内容
     */
    private suspend fun runToolLoop(
        requestedTools: List<ToolDef>,
        chatContext: ChatContext,
        onDelta: suspend (String) -> Unit
    ) {
        // 请求模型取 provider.modelName（与 streamResponse 一致）
        val modelName = provider.modelName
        var finalContent: String? = null
        var round = 0

        while (round < MAX_TOOL_ROUNDS) {
            round++

            // ── 构建请求（tools 固定，messages 累积） ──
            val request = ChatRequest(
                model = modelName,
                messages = messages,
                temperature = PersonaConfig.getTemperature(),
                max_tokens = PersonaConfig.getMaxTokens(),
                stream = false,      // 工具循环一律非流式（简化 tool_calls 解析）
                tools = requestedTools
                // tool_choice 不设置 → 默认 auto
            )

            // ── HTTP POST ──
            val httpResponse = executeNonStreamingRequest(request)
            if (!httpResponse.isSuccessful) {
                val errorBody = httpResponse.body?.string() ?: "未知错误"
                onDelta(httpErrorMessage(httpResponse.code, errorBody))
                return
            }

            // ── 解析完整响应 ──
            val body = httpResponse.body?.string() ?: ""
            httpResponse.close()
            val chatResponse = try {
                json.decodeFromString<ChatResponse>(body)
            } catch (e: Exception) {
                onDelta("[错误] 响应解析失败: ${e.message}")
                return
            }
            val choice = chatResponse.choices?.firstOrNull() ?: continue
            val message = choice.message ?: continue

            // ── 检测 tool_calls ──
            val toolCalls = message.tool_calls
            if (toolCalls.isNullOrEmpty()) {
                // 无工具调用：退出循环，获取文本内容
                finalContent = message.content ?: ""
                messages.add(message)
                break
            }

            // 第 MAX_TOOL_ROUNDS 轮仍返回 tool_calls → 循环上限，防止死循环
            if (round >= MAX_TOOL_ROUNDS) {
                onDelta("[错误] 工具调用超过最大轮次，已终止")
                return
            }

            // ── 有工具调用：先记录 assistant 消息（含 tool_calls） ──
            messages.add(message)

            // ── 逐个串行执行工具（含确认拦截），结果回填为 tool 消息 ──
            for (toolCall in toolCalls) {
                val toolName = toolCall.function.name
                val argsJson = toolCall.function.arguments
                val result = executeToolSafely(toolName, argsJson)
                messages.add(
                    ChatMessage(
                        role = "tool",
                        tool_call_id = toolCall.id,
                        content = result
                    )
                )
            }
            // 循环继续：messages 中已有 assistant(tool_calls) + tool results，
            // LLM 看到这些消息后会决定是继续调工具还是生成最终文本
        }

        // 循环因轮次耗尽而退出且未拿到文本 → 错误终止
        if (round >= MAX_TOOL_ROUNDS && finalContent == null) {
            onDelta("[错误] 工具调用超过最大轮次，已终止")
            return
        }

        // ── 发射最终内容（慢路径一次性发射整段文本） ──
        if (finalContent != null) {
            onDelta(finalContent)
            // ── afterReceive hooks：本轮对话已完整结束 ──
            runAfterReceive(finalContent, chatContext)
        }

        // ── 修剪对话历史（工具相关消息同样参与计数，自然裁剪） ──
        trimConversationHistory()
    }

    /**
     * 查表执行单个工具调用，返回回填给 LLM 的结果文本。
     *
     * 安全与容错：
     * - 未注册的工具 → 返回 "未找到工具" 错误 JSON（不崩溃）
     * - requiresConfirmation=true → 必须经 [confirmationGate] 确认：
     *   gate 为 null 或用户拒绝 → 不执行，返回 "用户拒绝执行"（安全默认，不可绕过）
     * - execute() 抛异常 → 异常信息作为工具结果回填，不中断循环
     */
    private suspend fun executeToolSafely(toolName: String, argsJson: String): String {
        val toolDef = tools.find { it.name == toolName }
        if (toolDef == null) {
            return """{"error":"未找到工具: $toolName"}"""
        }

        val approved = if (toolDef.requiresConfirmation) {
            val gate = confirmationGate
            if (gate == null) {
                false  // 安全默认：未注入确认门 → 拒绝所有危险操作（不变量 I5）
            } else {
                try {
                    gate.confirm(toolDef, argsJson)
                } catch (_: Exception) {
                    false  // 确认回调异常按拒绝处理（fail-safe）
                }
            }
        } else {
            true
        }

        if (!approved) {
            return """{"error":"用户拒绝执行 $toolName"}"""
        }

        return try {
            toolDef.execute(argsJson)
        } catch (e: Exception) {
            """{"error":"工具执行异常: ${e.message}"}"""
        }
    }

    /**
     * 将 [tools] 转换为 LLM API 所需的 tools 格式。
     * parametersSchema 为 JSON Schema 字符串，解析为 JsonObject 后嵌入 FunctionSpec。
     *
     * @return null 表示 tools 为空（请求体不发送 tools 字段，走快路径）
     */
    private fun buildToolsForRequest(): List<ToolDef>? {
        if (tools.isEmpty()) return null
        return tools.map { tool ->
            val parameters = try {
                json.parseToJsonElement(tool.parametersSchema).jsonObject
            } catch (_: Exception) {
                // 单个插件 schema 非法不应让整个请求失败，降级为空 Schema
                buildJsonObject { }
            }
            ToolDef(
                function = FunctionSpec(
                    name = tool.name,
                    description = tool.description,
                    parameters = parameters
                )
            )
        }
    }

    /**
     * 执行非流式 HTTP 请求，返回完整响应。
     * 用于工具调用循环（需要解析完整 tool_calls JSON）。
     */
    private fun executeNonStreamingRequest(request: ChatRequest): Response {
        val requestBody = json.encodeToString(request).toRequestBody(mediaType)
        val httpRequest = Request.Builder()
            .url("${provider.baseUrl}/chat/completions")
            .apply {
                if (provider.apiKey.isNotEmpty()) {
                    addHeader("Authorization", "Bearer ${provider.apiKey}")
                }
            }
            .addHeader("Accept", "application/json")
            .post(requestBody)
            .build()
        return client.newCall(httpRequest).execute()
    }

    /**
     * 触发 afterReceive hooks（已剥离表情标签）。
     * 单个 hook 抛异常不打断主链路（逐 try/catch 静默跳过）。
     */
    private suspend fun runAfterReceive(rawText: String, chatContext: ChatContext) {
        val emotion = EmotionController.EMOTION_KEYWORDS.firstOrNull { keyword ->
            rawText.contains("[$keyword]")
        }
        val strippedText = EmotionController.stripEmotionTags(rawText)
        for (hook in hooks) {
            try {
                hook.afterReceive(strippedText, emotion, chatContext)
            } catch (_: Exception) {
                // 插件钩子失败不应影响核心对话流程，静默跳过
            }
        }
    }

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
