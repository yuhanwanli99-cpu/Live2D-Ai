package com.live2dai.android

import android.util.Log
import com.live2dai.plugin.memory.SummaryLlmClient
import com.live2dai.pluginsdk.ChatTurn
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
 * [SummaryLlmClient] 的 core 实现（V1.1 模块3：长期记忆集成）。
 *
 * 复用用户已配置的 LLM Provider（构造时注入，见 [LLMProviderManager.getActiveProvider]），
 * 把近期几轮对话压缩成一条事实摘要，供记忆插件写入长期记忆库。
 *
 * 容错（架构约束 §8.2-2 + schema 契约 throws）：任何异常（网络超时 / HTTP 错误 /
 * 响应解析失败 / 空内容）都被捕获并静默返回 null——摘要失败只跳过本轮记忆写入，
 * 绝不影响对话核心链路。TAG 统一 "Memory"（Log.i 级别取证日志，见 V1.1 教训）。
 *
 * 契约见 docs/plan-v11-plan.schema.json → LlmSummaryClient.summarize。
 */
class LlmSummaryClient(
    private val provider: LLMProvider,
) : SummaryLlmClient {

    companion object {
        private const val TAG = "Memory"

        /** 摘要请求超时（秒）：远短于主对话链路（15s vs 120s），失败静默不拖累后台 */
        private const val TIMEOUT_SECONDS = 15L

        /** 送入摘要的最近对话轮数上限（控制 prompt 体积，~50 tokens 目标，plan §8.1） */
        private const val MAX_TURNS_IN_PROMPT = 6

        /** 摘要输出长度上限（一条事实摘要，~30 tokens 目标） */
        private const val MAX_OUTPUT_TOKENS = 100

        private const val MEDIA_TYPE = "application/json; charset=utf-8"

        /** 摘要系统提示词：提取「用户稳定信息」而非总结对话本身 */
        private const val SYSTEM_PROMPT =
            "你是一个对话记忆提取助手。从下面的对话中提取值得长期记住的用户稳定信息" +
                "（如用户的名字、喜好、家庭成员、习惯等事实），输出一条精简的中文事实摘要。" +
                "不要任何前缀、解释或标点修饰。如果没有值得长期记忆的信息，直接返回空。"
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

    override suspend fun summarize(recentTurns: List<ChatTurn>, personaName: String): String? =
        withContext(Dispatchers.IO) {
            try {
                doSummarize(recentTurns, personaName)
            } catch (e: Exception) {
                // 超时/API 错误/解析失败 → 静默跳过本轮摘要，不影响对话主链路
                logWarn("summarize 失败（${e.javaClass.simpleName}: ${e.message}），静默跳过")
                null
            }
        }

    /** 实际 HTTP 摘要调用（异常向上抛，由 [summarize] 统一捕获） */
    private fun doSummarize(recentTurns: List<ChatTurn>, personaName: String): String? {
        val turns = recentTurns.takeLast(MAX_TURNS_IN_PROMPT)
        if (turns.isEmpty()) return null

        val request = ChatRequest(
            model = provider.modelName,
            messages = listOf(
                ChatMessage(role = "system", content = SYSTEM_PROMPT),
                ChatMessage(role = "user", content = buildUserPrompt(turns, personaName)),
            ),
            temperature = 0.3,          // 低温度保证提取稳定
            max_tokens = MAX_OUTPUT_TOKENS,
            stream = false,             // 非流式：一条精简摘要无需 SSE
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

        logInfo("summarize 请求（${turns.size} 轮对话，model=${provider.modelName}）")
        client.newCall(httpRequest).execute().use { response ->
            if (!response.isSuccessful) {
                logWarn("summarize HTTP ${response.code}，静默跳过")
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
                logInfo("summarize 返回空内容：本轮无值得记忆的事实")
                return null
            }
            logInfo("summarize 成功：${content.take(40)}${if (content.length > 40) "…" else ""}")
            return content
        }
    }

    /** 把近期对话拼成摘要请求的用户消息（含人设名，让摘要贴合语气） */
    private fun buildUserPrompt(turns: List<ChatTurn>, personaName: String): String = buildString {
        appendLine("人设名：$personaName")
        appendLine("近期对话：")
        for (turn in turns) {
            val speaker = when (turn.role) {
                "user" -> "用户"
                "assistant" -> personaName.ifBlank { "助手" }
                else -> "系统"
            }
            appendLine("$speaker：${turn.content}")
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
