package com.live2dai.android

import android.util.Log
import com.live2dai.plugin.proactive.GreetingLlmClient
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.util.concurrent.TimeUnit

/**
 * [GreetingLlmClient] 的 core 实现（V1.2 模块6：主动陪伴）。
 *
 * 复用用户已配置的 LLM Provider（构造时注入，见 [LLMProviderManager.getActiveProvider]），
 * 与 [LlmSummaryClient] 完全同款调用方式：OpenAI 兼容 /chat/completions 非流式请求。
 * 生成一句贴合人设、带时间感、结合最近记忆的中文主动问候/话题（plan §5.6.3）。
 *
 * 容错（架构约束 §8.2-2 + schema 契约 throws: []）：任何异常（网络超时 / HTTP 错误 /
 * 响应解析失败 / 空内容）都被捕获并静默返回 null——主动问候失败只回退到静态文案池，
 * 绝不影响对话核心链路。日志 TAG "Proactive"（Log.i 级别取证日志，见 V1.1 教训）。
 *
 * 契约见 docs/plan-v12-plan.schema.json → GreetingLlmClient.generateGreeting。
 */
class ProactiveGreetingClient(
    private val provider: LLMProvider,
) : GreetingLlmClient {

    companion object {
        private const val TAG = "Proactive"

        /** 问候请求超时（秒）：与摘要同款短超时，失败静默不拖累后台检查 */
        private const val TIMEOUT_SECONDS = 15L

        /** 问候输出长度上限（一句自然的话，~60 tokens 目标） */
        private const val MAX_OUTPUT_TOKENS = 120

        private const val MEDIA_TYPE = "application/json; charset=utf-8"

        /**
         * 问候系统提示词：生成一句自然、简短、贴合人设与记忆的主动问候/话题，
         * 只输出这句话本身（LLM 输出会直接经 TTS 播报 + 通知栏展示）。
         */
        private const val SYSTEM_PROMPT =
            "你是一个主动的 AI 助手。请结合人设、距离上次聊天的时长" +
                "以及关于用户的记忆，生成一句自然、简短（不超过 40 字）的中文主动问候或话题。" +
                "要像真正在意用户一样，语气贴合人设。只输出这句话本身，" +
                "不要任何前缀、引号、表情标签或解释。"
    }

    @OptIn(ExperimentalSerializationApi::class)
    private val json = Json {
        ignoreUnknownKeys = true
        // 序列化时省略 null 字段（tools/tool_choice 不发送，保持请求体精简）
        explicitNulls = false
    }

    private val client = OkHttpClient.Builder()
        .connectTimeout(TIMEOUT_SECONDS, TimeUnit.SECONDS)
        .readTimeout(TIMEOUT_SECONDS, TimeUnit.SECONDS)
        .writeTimeout(TIMEOUT_SECONDS, TimeUnit.SECONDS)
        .build()

    private val mediaType = MEDIA_TYPE.toMediaType()

    override suspend fun generateGreeting(
        personaName: String,
        systemPrompt: String,
        idleMinutes: Long,
        memories: List<String>?,
    ): String? = withContext(Dispatchers.IO) {
        try {
            doGenerate(personaName, systemPrompt, idleMinutes, memories)
        } catch (e: Exception) {
            // 超时/API 错误/解析失败 → 静默返回 null，由插件回退静态文案池
            logWarn("generateGreeting 失败（${e.javaClass.simpleName}: ${e.message}），回退静态文案池")
            null
        }
    }

    /** 实际 HTTP 问候调用（异常向上抛，由 [generateGreeting] 统一捕获） */
    private fun doGenerate(
        personaName: String,
        systemPrompt: String,
        idleMinutes: Long,
        memories: List<String>?,
    ): String? {
        val request = ChatRequest(
            model = provider.modelName,
            messages = listOf(
                ChatMessage(role = "system", content = SYSTEM_PROMPT),
                ChatMessage(role = "user", content = buildUserPrompt(personaName, systemPrompt, idleMinutes, memories)),
            ),
            temperature = 0.8,       // 中等温度：自然口语化但不失控
            max_tokens = MAX_OUTPUT_TOKENS,
            stream = false,          // 非流式：一句话无需 SSE
        )

        val httpRequest = Request.Builder()
            .url("${provider.baseUrl}/chat/completions")
            .apply {
                // 仅当 apiKey 非空时添加 Authorization header（Ollama 等本地端无需鉴权）
                if (provider.apiKey.isNotEmpty()) {
                    addHeader("Authorization", "Bearer ${provider.apiKey}")
                }
            }
            .addHeader("Accept", "application/json")
            .post(json.encodeToString(request).toRequestBody(mediaType))
            .build()

        logInfo("generateGreeting 请求（idle=${idleMinutes}min, memories=${memories?.size ?: 0} 条, model=${provider.modelName}）")
        client.newCall(httpRequest).execute().use { response ->
            if (!response.isSuccessful) {
                logWarn("generateGreeting HTTP ${response.code}，静默回退静态文案池")
                return null
            }
            val body = response.body?.string().orEmpty()
            val chatResponse = json.decodeFromString<ChatResponse>(body)
            val content = chatResponse.choices
                ?.firstOrNull()
                ?.message
                ?.content
                ?.trim()
                .orEmpty()
            if (content.isEmpty()) {
                logWarn("generateGreeting 返回空内容，静默回退静态文案池")
                return null
            }
            logInfo("generateGreeting 成功：${content.take(40)}${if (content.length > 40) "…" else ""}")
            return content
        }
    }

    /** 把问候上下文（人设 + 时间 + 记忆）拼成用户消息。 */
    private fun buildUserPrompt(
        personaName: String,
        systemPrompt: String,
        idleMinutes: Long,
        memories: List<String>?,
    ): String = buildString {
        appendLine("人设名：${personaName.ifBlank { "凛" }}")
        if (systemPrompt.isNotBlank()) {
            appendLine("人设设定：$systemPrompt")
        }
        appendLine("距离上次聊天：$idleMinutes 分钟")
        appendLine("关于用户的记忆：")
        val memoryLines = memories.orEmpty()
        if (memoryLines.isEmpty()) {
            appendLine("（暂无）")
        } else {
            memoryLines.forEach { appendLine("- $it") }
        }
    }

    /**
     * 日志输出（带单元测试防护）：本地 JVM 单测（无 Android 运行时）中
     * android.util.Log 未被 mock，直接调用会抛 "Method i in android.util.Log not mocked"。
     */
    private fun logInfo(message: String) {
        try {
            Log.i(TAG, message)
        } catch (_: Throwable) {
            // 单元测试环境：静默忽略
        }
    }

    private fun logWarn(message: String) {
        try {
            Log.w(TAG, message)
        } catch (_: Throwable) {
            // 单元测试环境：静默忽略
        }
    }
}
