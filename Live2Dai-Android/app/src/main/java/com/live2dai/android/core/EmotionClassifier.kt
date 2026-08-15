package com.live2dai.android.core

import com.live2dai.android.ChatMessage
import com.live2dai.android.ChatRequest
import com.live2dai.android.ChatResponse
import com.live2dai.android.data.EmotionTagTable
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withTimeoutOrNull
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import okhttp3.Call
import okhttp3.Callback
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
import java.io.IOException

/**
 * 情绪分类结果（N.E.K.O. 契约锚 1）。
 *
 * @property label 情绪标签（小写；∈ [EmotionTagTable] 已知集，见 [EmotionTagTable.isKnownEmotion]）。
 * @property confidence 置信度，构造时钳制到 [0,1]（越界 coerceIn 归一；NaN 防御为 0f）。
 * @property source 来源标记：如 "llm"（独立分类调用）/ "inline"（内嵌标签）/ "neutral"。
 */
data class EmotionClassifierResult constructor(
    val label: String,
    val confidence: Float,
    val source: String,
) {
    companion object {
        /**
         * 工厂：confidence 越界钳制到 [0,1]（契约锚 1 "confidence ∈[0,1] 越界钳制"）。
         * 构造语法与普通 data class 一致（`EmotionClassifierResult("joy", 1.5f, "llm")`）。
         */
        operator fun invoke(label: String, confidence: Float, source: String): EmotionClassifierResult =
            EmotionClassifierResult(
                label = label,
                confidence = if (confidence.isNaN()) 0f else confidence.coerceIn(0f, 1f),
                source = source,
            )
    }
}

/**
 * 情绪分类器接缝（N.E.K.O. 契约锚 2）：独立 LLM 分类调用 → 单一情绪标签。
 *
 * 实现契约：
 *  - 成功 → 单一已知标签（label ∈ [EmotionTagTable] 已知集，经 [EmotionTagTable.isKnownEmotion]
 *    校验；未知 → null）；
 *  - 异常 / 超时 → null（**绝不抛**，仅协程取消的 [CancellationException] 原样上抛）。
 *
 * 可注入 fake（seam 要求）：HttpLlmLink / EmotionClassifierBridge 经构造注入本接口。
 */
interface EmotionClassifier {

    /** 对 [text] 做情绪分类；失败 / 未知 / 超时 → null（绝不抛异常）。 */
    suspend fun classify(text: String): EmotionClassifierResult?
}

/**
 * 情绪解析纯函数（N.E.K.O. 契约锚 3）。
 *
 * 优先级：**分类器标签 > 内嵌标签 > "neutral"**；
 * 返回值恒 ∈ [EmotionTagTable] 已知集（集外防御 → neutral）；
 * 大小写归一（trim + 小写 + 去强度后缀 `_数字`）。
 *
 * @param classifierResult 分类器结果（可 null：分类失败/未启用/未知标签）。
 * @param inlineEmotion 内嵌标签（[EmotionText.parse] 提取的 emotion；可 null）。
 */
fun resolveEmotion(classifierResult: EmotionClassifierResult?, inlineEmotion: String?): String {
    val classifierLabel = normalizeEmotion(classifierResult?.label)
    if (classifierLabel != null && EmotionTagTable.isKnownEmotion(classifierLabel)) {
        return classifierLabel
    }
    val inline = normalizeEmotion(inlineEmotion)
    if (inline != null && EmotionTagTable.isKnownEmotion(inline)) {
        return inline
    }
    return "neutral"
}

/**
 * 情绪分类桥（N.E.K.O. 契约锚 4）：分类器结果 + 内嵌标签 → 单一情绪标签。
 *
 * [emotionFor]：调 [classifier.classify] → [resolveEmotion]（内嵌标签经
 * [EmotionText.parse] 提取）；空输入不崩溃，返回 "neutral"。
 */
class EmotionClassifierBridge(
    private val classifier: EmotionClassifier,
) {

    /** 分类器优先，内嵌标签兜底；空输入返回 "neutral"；恒 ∈ 已知集。 */
    suspend fun emotionFor(text: String): String {
        if (text.isBlank()) return "neutral"
        val result = classifier.classify(text)
        return resolveEmotion(result, EmotionText.parse(text).emotion)
    }
}

/**
 * 情绪分类器 HTTP 实现（N.E.K.O. 契约锚 5）：独立 LLM 分类调用 → 单一情绪标签。
 *
 * - 与 [HttpLlmLink] 同 baseUrl / apiKey / client（构造注入，可复用同一 [OkHttpClient]）；
 * - 非流式小请求：system prompt 要求把文本情绪归为 joy/sad/angry/surprise/neutral 之一、
 *   只输出一个词；temperature=0、max_tokens=16（确定性单标签输出）；
 * - 成功 → [EmotionClassifierResult]（label ∈ [EmotionTagTable] 已知集，经
 *   [EmotionTagTable.isKnownEmotion] 校验；未知 → null）；
 * - HTTP 错误 / 网络错误 / 解析失败 / 超时 → null（绝不抛）；
 * - **单次调用超时 3s**：[withTimeoutOrNull] + 可取消 enqueue
 *   （[suspendCancellableCoroutine] + invokeOnCancellation cancel 底层 [Call]，超时可真中断）。
 */
class HttpEmotionClassifier(
    private val baseUrl: String,
    private val apiKey: String,
    private val client: OkHttpClient,
    private val model: String = "deepseek-chat",
) : EmotionClassifier {

    private val json = Json {
        ignoreUnknownKeys = true
    }

    override suspend fun classify(text: String): EmotionClassifierResult? {
        if (text.isBlank()) return null
        return withTimeoutOrNull(CLASSIFY_TIMEOUT_MS) {
            classifyInternal(text)
        }
    }

    /** 单次调用体：任何失败 → null（超时由外层 [withTimeoutOrNull] 负责）。 */
    private suspend fun classifyInternal(text: String): EmotionClassifierResult? {
        try {
            val response = awaitResponse(buildRequest(text)) ?: return null
            response.use {
                if (!it.isSuccessful) return null
                val body = it.body?.string() ?: return null
                return parseLabel(body)
            }
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            return null
        }
    }

    /** 可取消的 OkHttp 调用（enqueue + suspendCancellableCoroutine；取消时 cancel 底层 Call）。 */
    private suspend fun awaitResponse(request: Request): Response? =
        suspendCancellableCoroutine { cont ->
            val call = client.newCall(request)
            cont.invokeOnCancellation { call.cancel() }
            call.enqueue(object : Callback {
                override fun onFailure(call: Call, e: IOException) {
                    // 失败（含被取消）：取消已由 withTimeoutOrNull 收尾 → 无需再恢复
                    cont.resume(null, onCancellation = null)
                }

                override fun onResponse(call: Call, response: Response) {
                    // 若续体已被取消（超时先到），onCancellation 关闭 response 防泄漏
                    cont.resume(response, onCancellation = { _ -> response.close() })
                }
            })
        }

    /** 解析响应 JSON → 归一化标签；未知标签 / 解析失败 → null。 */
    private fun parseLabel(body: String): EmotionClassifierResult? {
        val response = try {
            json.decodeFromString<ChatResponse>(body)
        } catch (e: Exception) {
            return null
        }
        val content = response.choices?.firstOrNull()?.message?.content
        val label = content?.let { normalizeLabel(it) } ?: return null
        if (!EmotionTagTable.isKnownEmotion(label)) return null
        return EmotionClassifierResult(label = label, confidence = 1.0f, source = "llm")
    }

    /** LLM 输出标签规范化：剥离包裹符（[]/引号/反引号）与句读 → trim → 小写 → 去强度后缀。 */
    private fun normalizeLabel(raw: String): String? {
        var t = raw.trim()
        t = t.removePrefix("[").removeSuffix("]")
        t = t.removePrefix("\"").removeSuffix("\"")
        t = t.removePrefix("'").removeSuffix("'")
        t = t.removePrefix("`").removeSuffix("`")
        t = t.trim('.', '。', '!', '！', '?', '？')
        return normalizeEmotion(t)
    }

    private fun buildRequest(text: String): Request {
        val payload = ChatRequest(
            model = model,
            messages = listOf(
                ChatMessage(role = "system", content = CLASSIFY_SYSTEM_PROMPT),
                ChatMessage(role = "user", content = text),
            ),
            temperature = 0.0,
            max_tokens = 16,
            stream = false,
        )
        val requestBody = json.encodeToString(payload).toRequestBody(JSON_MEDIA_TYPE)
        return Request.Builder()
            .url(endpoint())
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
        /** 单次调用超时（契约锚 5）：3 秒。 */
        const val CLASSIFY_TIMEOUT_MS = 3_000L

        /** 分类 prompt（契约锚 5 文案）：五基元之一、只输出一个词。 */
        const val CLASSIFY_SYSTEM_PROMPT =
            "把下面这句话的情绪归为 joy/sad/angry/surprise/neutral 之一，只输出一个词"

        val JSON_MEDIA_TYPE = "application/json; charset=utf-8".toMediaType()
    }
}

/** 标签规范化（[resolveEmotion] / [HttpEmotionClassifier] 共用）：trim + 小写 + 去强度后缀；空白 → null。 */
private fun normalizeEmotion(raw: String?): String? {
    if (raw.isNullOrBlank()) return null
    return raw.trim().lowercase().replace(INTENSITY_SUFFIX_REGEX, "").ifEmpty { null }
}

/** 强度后缀形状：`_数字`（joy_3 → joy；与 EmotionText 标签形状同源）。 */
private val INTENSITY_SUFFIX_REGEX = Regex("""_\d+$""")
