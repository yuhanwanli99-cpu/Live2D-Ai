package com.live2dai.plugin.vision

import android.graphics.Bitmap
import android.util.Base64
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonObject
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.ByteArrayOutputStream
import java.util.concurrent.TimeUnit

/**
 * GLM-4V 视觉模型 HTTP 客户端（从 core 的 `VisionService.kt` 迁移）。
 *
 * 变化点（相对原 VisionService）：
 * - `apiKey` 从 `BuildConfig`/环境变量读取改为构造注入（由 core 传入）
 * - `visionModel` 不再从 `PersonaConfig.getVisionModel()` 读取，改为 [describeImage] 参数
 * - 其余逻辑（Base64 编码、HTTP POST、JSON 解析）与原实现保持一致
 */
class GlmVisionClient(
    /** Zhipu API Key；为空时 [describeImage] 直接返回空串，不发请求 */
    val apiKey: String,
    private val baseUrl: String = "https://open.bigmodel.cn/api/paas/v4",
) {
    private val json = Json { ignoreUnknownKeys = true }
    private val client = OkHttpClient.Builder()
        .connectTimeout(30, TimeUnit.SECONDS)
        .readTimeout(60, TimeUnit.SECONDS)
        .build()
    private val mediaType = "application/json; charset=utf-8".toMediaType()

    /**
     * Describe an image using GLM-4V vision model.
     *
     * @param bitmap The image to analyze
     * @param model 视觉模型名称，如 "glm-4.6v"
     * @param prompt Optional prompt for the image analysis
     * @return Description text from the vision model; empty string on failure
     *         or when no apiKey is configured.
     */
    suspend fun describeImage(
        bitmap: Bitmap,
        model: String,
        prompt: String = "请描述这张图片的内容",
    ): String = withContext(Dispatchers.IO) {
        if (apiKey.isBlank()) return@withContext ""
        try {
            val base64 = bitmapToBase64(bitmap)

            val contentArray = buildJsonArray {
                add(buildJsonObject { put("type", JsonPrimitive("text")); put("text", JsonPrimitive(prompt)) })
                add(buildJsonObject { put("type", JsonPrimitive("image_url")); putJsonObject("image_url") { put("url", JsonPrimitive("data:image/jpeg;base64,$base64")) } })
            }

            val messagesArray = buildJsonArray {
                add(buildJsonObject {
                    put("role", JsonPrimitive("user"))
                    put("content", contentArray)
                })
            }

            val requestBody = buildJsonObject {
                put("model", JsonPrimitive(model))
                put("messages", messagesArray)
                put("stream", JsonPrimitive(false))
                put("max_tokens", JsonPrimitive(500))
            }.toString().toRequestBody(mediaType)

            val request = Request.Builder()
                .url("$baseUrl/chat/completions")
                .addHeader("Authorization", "Bearer $apiKey")
                .post(requestBody)
                .build()

            val response = client.newCall(request).execute()
            if (!response.isSuccessful) return@withContext ""

            val responseBody = response.body?.string() ?: return@withContext ""
            val parsed = json.decodeFromString<JsonObject>(responseBody)
            parsed["choices"]?.jsonArray?.firstOrNull()
                ?.jsonObject?.let { choice ->
                    // Handle both streaming and non-streaming response formats
                    val msg = choice["message"]?.jsonObject
                    msg?.let {
                        it["content"]?.jsonPrimitive?.contentOrNull
                    } ?: choice["delta"]?.jsonObject?.let {
                        it["content"]?.jsonPrimitive?.contentOrNull
                    }
                } ?: ""
        } catch (e: Exception) {
            e.printStackTrace()
            ""
        }
    }

    private fun bitmapToBase64(bitmap: Bitmap): String {
        val stream = ByteArrayOutputStream()
        bitmap.compress(Bitmap.CompressFormat.JPEG, 85, stream)
        val bytes = stream.toByteArray()
        return Base64.encodeToString(bytes, Base64.NO_WRAP)
    }
}
