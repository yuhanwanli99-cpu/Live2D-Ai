package com.live2dai.android

import android.graphics.Bitmap
import android.util.Base64
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.*
import okhttp3.*
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.ByteArrayOutputStream

/**
 * Vision service using Zhipu GLM-4V for image understanding.
 * Called when the AI needs visual context (screen/camera).
 */
class VisionService(
    private val apiKey: String = BuildConfig.ZHIPU_API_KEY
        .ifEmpty { System.getenv("ZHIPU_API_KEY") ?: "" },
    private val baseUrl: String = "https://open.bigmodel.cn/api/paas/v4"
) {
    private val json = Json { ignoreUnknownKeys = true }
    private val client = OkHttpClient.Builder()
        .connectTimeout(30, java.util.concurrent.TimeUnit.SECONDS)
        .readTimeout(60, java.util.concurrent.TimeUnit.SECONDS)
        .build()
    private val mediaType = "application/json; charset=utf-8".toMediaType()

    /**
     * Describe an image using GLM-4V vision model.
     * @param bitmap The image to analyze
     * @param prompt Optional prompt for the image analysis
     * @return Description text from the vision model
     */
    suspend fun describeImage(bitmap: Bitmap, prompt: String = "请描述这张图片的内容"): String = withContext(Dispatchers.IO) {
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
                put("model", JsonPrimitive(PersonaConfig.getVisionModel()))
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
