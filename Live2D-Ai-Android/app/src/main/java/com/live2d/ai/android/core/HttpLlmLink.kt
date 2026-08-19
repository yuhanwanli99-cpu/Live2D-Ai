package com.live2d.ai.android.core

import com.live2d.ai.android.ChatMessage
import com.live2d.ai.android.ChatRequest
import com.live2d.ai.android.PersonaConfig
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.IOException

/**
 * [LlmLink] 的 HTTP 实现（2C-2，契约 §2.2）。
 *
 * - `send(text)`：**冷流**、单消费者、零重放（每次 collect 发起一次 HTTP 请求）；
 *   流式 SSE 增量逐条发射 [LlmEvent.Delta]，收尾发射 [LlmEvent.Completed]
 *   （emotion 经 [EmotionClassifierBridge] 独立分类调用得出——分类器优先，内嵌标签兜底
 *   由 resolveEmotion 承担；N.E.K.O. 方案）；
 *   HTTP 错误 / 网络错误 / 超时 → [LlmEvent.Failed]（中文文案 + quotaExhausted 标志）。
 * - `reset()`：清空对话上下文（保留 persona system 首位），后续请求不带旧轮。
 * - [rebuild]：运行时更新 provider/base_url/api_key/model/temperature（设置修改后无需
 *   重启即可对下一次请求生效，修复“设置写了但运行仍用旧值”的调用链断链）。
 * - persona 注入位置 = system 消息首位（[ChatHistoryBuilder.setSystem]）。
 */
class HttpLlmLink(
    baseUrl: String,
    apiKey: String,
    private val client: OkHttpClient,
    private val persona: PersonaConfig,
    /**
     * 情绪分类桥（N.E.K.O. 契约锚 6 接线）：Completed 的 emotion 来源；构造注入可替换 fake
     * （seam 要求）。默认 = [EmotionClassifierBridge] 包 [HttpEmotionClassifier]（同 baseUrl/
     * apiKey/client；单次分类调用超时 3s）。
     */
    private var bridge: EmotionClassifierBridge = EmotionClassifierBridge(
        HttpEmotionClassifier(baseUrl, apiKey, client),
    ),
) : LlmLink {

    private val json = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
        // 省略 null 字段（OpenAI 兼容 API 对缺失字段与 null 等效）
        explicitNulls = false
    }

    /** 运行时可变端点/模型快照（设置修改经 [rebuild] 更新，下一次请求即生效）。 */
    @Volatile
    private var config = LlmEndpointConfig(
        baseUrl = baseUrl,
        apiKey = apiKey,
        model = persona.getModel(),
        temperature = persona.getTemperature(),
    )

    private val historyBuilder = ChatHistoryBuilder()

    init {
        historyBuilder.setSystem(persona.getSystemPrompt())
    }

    /**
     * 运行时更新端点/模型配置（提供方在设置保存后调用；线程安全经 [@Volatile] 快照）。
     * model 传 null 时保留当前值；另一方缺失字段同理保留。
     */
    fun rebuild(newConfig: LlmEndpointConfig) {
        val current = config
        val effective = newConfig.copy(
            // null 字段保留当前值（调用方可只更新部分字段而不丢失其它）
            model = newConfig.model ?: current.model,
            temperature = newConfig.temperature ?: current.temperature,
        )
        config = effective
        // 情绪分类桥与主链路同端点/同 Key：运行时更换 Provider/base_url/api_key 后，
        // 分类调用也指向新端点，避免「主请求用新配置、情绪分类仍打旧端点」的错配/降级
        // （审计 M-1：HttpEmotionClassifier 构造期快照旧 baseUrl/apiKey）。
        bridge = EmotionClassifierBridge(
            HttpEmotionClassifier(effective.baseUrl, effective.apiKey, client),
        )
    }

    override fun send(text: String): Flow<LlmEvent> = flow {
        if (text.isBlank()) return@flow

        val messages = historyBuilder.withUser(text)
        val request = buildRequest(messages)

        try {
            val response = client.newCall(request).execute()
            response.use {
                if (!it.isSuccessful) {
                    val body = it.body?.string() ?: ""
                    val err = classifyError(it.code, body)
                    emit(LlmEvent.Failed(err.message, err.quotaExhausted))
                    return@flow
                }

                val source = it.body?.source()
                if (source == null) {
                    emit(LlmEvent.Failed("[错误] 未获取到回复内容", quotaExhausted = false))
                    return@flow
                }

                val decoder = SseChunkDecoder()
                val fullText = StringBuilder()
                var firstDeltaEmitted = false

                // 逐行读取 SSE（okio 行读取原子，不会切断多字节字符）
                while (!source.exhausted()) {
                    val line = source.readUtf8Line() ?: break
                    val events = decoder.feed(line + "\n")
                    for (event in events) {
                        when (event) {
                            is SseEvent.Delta -> {
                                fullText.append(event.fragment)
                                // 首段剥离表情标签（[joy] 前缀不显示在气泡）
                                val cleaned = if (!firstDeltaEmitted) {
                                    firstDeltaEmitted = true
                                    EmotionText.strip(event.fragment)
                                } else event.fragment
                                if (cleaned.isNotEmpty()) emit(LlmEvent.Delta(cleaned))
                            }
                            SseEvent.Done -> { /* [DONE]：终止，close() 无残余可冲刷 */ }
                        }
                    }
                    if (decoder.isDone) break
                }

                // 流关闭冲刷：末块无 [DONE] 时发出已累积增量
                decoder.close()?.let { event ->
                    if (event is SseEvent.Delta) {
                        fullText.append(event.fragment)
                        val cleaned = if (!firstDeltaEmitted) {
                            firstDeltaEmitted = true
                            EmotionText.strip(event.fragment)
                        } else event.fragment
                        if (cleaned.isNotEmpty()) emit(LlmEvent.Delta(cleaned))
                    }
                }

                val finalText = fullText.toString()
                if (finalText.isNotEmpty()) {
                    val cleanText = EmotionText.strip(finalText)
                    historyBuilder.addTurn(text, cleanText)
                    // 情绪来源改走分类器桥（N.E.K.O. 方案）：分类器优先，内嵌标签兜底由
                    // resolveEmotion 承担（bridge.emotionFor 内部经 EmotionText.parse 提取）
                    // 传 finalText（含标签原始文本）→ 分类器失败时内嵌 [tag] 兜底才可命中（主代理修正 deviation-1）
                    emit(LlmEvent.Completed(cleanText, bridge.emotionFor(finalText)))
                } else {
                    emit(LlmEvent.Failed("[错误] 未获取到回复内容", quotaExhausted = false))
                }
            }
        } catch (e: CancellationException) {
            throw e
        } catch (e: IOException) {
            val err = classifyNetworkError(e)
            emit(LlmEvent.Failed(err.message, err.quotaExhausted))
        } catch (e: Exception) {
            val err = classifyNetworkError(e)
            emit(LlmEvent.Failed(err.message, err.quotaExhausted))
        }
    }.flowOn(Dispatchers.IO)

    override fun reset() {
        historyBuilder.reset()
        historyBuilder.setSystem(persona.getSystemPrompt())
    }

    private fun buildRequest(messages: List<ChatMessage>): Request {
        val cfg = config
        val payload = ChatRequest(
            model = cfg.model ?: persona.getModel(),
            messages = messages,
            temperature = cfg.temperature?.toDouble() ?: persona.getTemperature(),
            max_tokens = persona.getMaxTokens(),
            stream = true,
        )
        val requestBody = json.encodeToString(payload).toRequestBody(JSON_MEDIA_TYPE)
        return Request.Builder()
            .url(endpoint(cfg.baseUrl))
            .addHeader("Accept", "text/event-stream")
            .apply {
                // 仅当 apiKey 非空时添加 Authorization header（Ollama 等本地模型无需 Key）
                if (cfg.apiKey.isNotEmpty()) {
                    addHeader("Authorization", "Bearer ${cfg.apiKey}")
                }
            }
            .post(requestBody)
            .build()
    }

    /** 由调用方传入已快照的 baseUrl，保证一次请求的 payload 与 URL 取自同一个 config 快照
     *  （审计 M-3：避免 rebuild 与 in-flight send 并发时 torn-config——payload 用旧值、URL 用新值）。 */
    private fun endpoint(baseUrl: String): String {
        val base = baseUrl.trimEnd('/')
        return if (base.endsWith("/chat/completions")) base else "$base/chat/completions"
    }

    private companion object {
        val JSON_MEDIA_TYPE = "application/json; charset=utf-8".toMediaType()
    }
}

/**
 * 运行时 LLM 端点/模型快照（[HttpLlmLink.rebuild] 的入参）。
 * 用于在设置页修改 Provider/base_url/api_key/model/temperature 后，
 * 无需重启即可让下一次请求使用新配置。
 */
data class LlmEndpointConfig(
    val baseUrl: String,
    val apiKey: String,
    val model: String?,
    val temperature: Float? = null,
)
