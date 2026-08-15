package com.live2dai.android.core

import com.live2dai.android.ChatMessage
import com.live2dai.android.ChatRequest
import com.live2dai.android.PersonaConfig
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
 *   （fullText 含 emotion 标签原文，emotion 仅提取首个命中标签）；
 *   HTTP 错误 / 网络错误 / 超时 → [LlmEvent.Failed]（中文文案 + quotaExhausted 标志）。
 * - `reset()`：清空对话上下文（保留 persona system 首位），后续请求不带旧轮。
 * - persona 注入位置 = system 消息首位（[ChatHistoryBuilder.setSystem]）。
 */
class HttpLlmLink(
    private val baseUrl: String,
    private val apiKey: String,
    private val client: OkHttpClient,
    private val persona: PersonaConfig,
) : LlmLink {

    private val json = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
        // 省略 null 字段（OpenAI 兼容 API 对缺失字段与 null 等效）
        explicitNulls = false
    }

    private val historyBuilder = ChatHistoryBuilder()

    init {
        historyBuilder.setSystem(persona.getSystemPrompt())
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
                    val emotion = emotionKeywords()
                        .firstOrNull { finalText.contains("[$it]") }
                    val cleanText = EmotionText.strip(finalText)
                    historyBuilder.addTurn(text, cleanText)
                    emit(LlmEvent.Completed(cleanText, emotion))
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
        val payload = ChatRequest(
            model = persona.getModel(),
            messages = messages,
            temperature = persona.getTemperature(),
            max_tokens = persona.getMaxTokens(),
            stream = true,
        )
        val requestBody = json.encodeToString(payload).toRequestBody(JSON_MEDIA_TYPE)
        return Request.Builder()
            .url(endpoint())
            .addHeader("Accept", "text/event-stream")
            .apply {
                // 仅当 apiKey 非空时添加 Authorization header（Ollama 等本地模型无需 Key）
                if (apiKey.isNotEmpty()) {
                    addHeader("Authorization", "Bearer $apiKey")
                }
            }
            .post(requestBody)
            .build()
    }

    private fun endpoint(): String {
        val base = baseUrl.trimEnd('/')
        return if (base.endsWith("/chat/completions")) base else "$base/chat/completions"
    }

    private companion object {
        val JSON_MEDIA_TYPE = "application/json; charset=utf-8".toMediaType()

        /**
         * 表情标签关键词（首个命中标签提取用）。
         *
         * 来源：Wave 5 前为 [EmotionController.EMOTION_KEYWORDS]（adapter emotionIndex 键集
         * 字母序；EmotionController 已随 Wave 5 删除）。现改为由 [EmotionText.SUPPORTED_EMOTIONS]
         * （契约 refactor-0814 2C-3 规范集）字母序派生——原 8 键（anger/disgust/fear/joy/neutral/
         * sadness/smirk/surprise）相对顺序与提取语义不变，扩展识别契约锚点短名 angry/sad。
         */
        private fun emotionKeywords(): List<String> = EmotionText.SUPPORTED_EMOTIONS.sorted()
    }
}
