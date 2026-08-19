package com.live2d.ai.android

import android.content.Context
import android.media.MediaPlayer
import android.util.Log
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONObject
import java.io.File
import java.io.FileOutputStream
import java.util.concurrent.TimeUnit

/**
 * 通义千问 Qwen TTS Provider（DashScope 非实时语音合成）。
 *
 * 调用 `qwen3-tts-flash` 生成 WAV，下载后通过 MediaPlayer 播放。
 * 与 CosyVoice 共用 DashScope API Key（settings.aliyunApiKey）。
 */
class QwenTtsProvider(
    private val context: Context,
    private val apiKey: String,
    private val voice: String = "Cherry",
    private val model: String = "qwen3-tts-flash",
    private val client: OkHttpClient = defaultClient(),
) : TtsProvider {

    companion object {
        private const val TAG = "QwenTts"
        const val PROVIDER_ID = "qwen_tts"
        private const val TTS_URL =
            "https://dashscope.aliyuncs.com/api/v1/services/aigc/multimodal-generation/generation"
        private const val JSON = "application/json; charset=utf-8"

        private fun defaultClient(): OkHttpClient = OkHttpClient.Builder()
            .connectTimeout(30, TimeUnit.SECONDS)
            .readTimeout(120, TimeUnit.SECONDS)
            .writeTimeout(60, TimeUnit.SECONDS)
            .build()
    }

    private val _health = MutableStateFlow(if (apiKey.isBlank()) TtsHealth.UNAVAILABLE else TtsHealth.OK)
    override val info = TtsProviderInfo(
        id = PROVIDER_ID,
        name = "通义千问 TTS",
        category = TtsProviderCategory.CLOUD_HTTP,
        requiresNetwork = true,
        health = _health,
        priority = 2,
        configurable = true,
    )

    private val _isSpeaking = MutableStateFlow(false)
    override val isSpeaking: StateFlow<Boolean> = _isSpeaking.asStateFlow()

    private val _audioFrameFlow = MutableSharedFlow<AudioFrame>(extraBufferCapacity = 16)
    override val audioFrameFlow: Flow<AudioFrame> = _audioFrameFlow.asSharedFlow()

    private var mediaPlayer: MediaPlayer? = null

    override suspend fun speak(text: String) {
        if (text.isBlank()) return
        if (apiKey.isBlank()) {
            _health.value = TtsHealth.UNAVAILABLE
            throw IllegalStateException("Qwen TTS API Key 未配置")
        }
        withContext(Dispatchers.IO) {
            try {
                val wavFile = synthesizeToFile(text)
                playFile(wavFile)
            } catch (ce: CancellationException) {
                throw ce
            } catch (e: Exception) {
                _health.value = TtsHealth.DEGRADED
                throw e
            }
        }
    }

    override fun stop() {
        try {
            mediaPlayer?.stop()
        } catch (_: Throwable) {
        }
        _isSpeaking.value = false
    }

    override fun release() {
        stop()
        try {
            mediaPlayer?.release()
        } catch (_: Throwable) {
        }
        mediaPlayer = null
    }

    private suspend fun synthesizeToFile(text: String): File {
        val payload = JSONObject()
            .put("model", model)
            .put(
                "input",
                JSONObject()
                    .put("text", text)
                    .put("voice", voice)
                    .put("language_type", "Auto")
            )
        val request = Request.Builder()
            .url(TTS_URL)
            .addHeader("Authorization", "Bearer $apiKey")
            .post(payload.toString().toRequestBody(JSON.toMediaType()))
            .build()
        client.newCall(request).execute().use { resp ->
            if (!resp.isSuccessful) {
                throw IllegalStateException("Qwen TTS HTTP ${resp.code}: ${resp.body?.string()?.take(200)}")
            }
            val body = resp.body?.string() ?: throw IllegalStateException("Qwen TTS 空响应")
            val audioUrl = JSONObject(body)
                .getJSONObject("output")
                .getJSONObject("audio")
                .getString("url")
            val audioResp = client.newCall(Request.Builder().url(audioUrl).build()).execute()
            if (!audioResp.isSuccessful) {
                throw IllegalStateException("Qwen TTS 音频下载失败: ${audioResp.code}")
            }
            val file = File(context.cacheDir, "qwen_tts_${System.currentTimeMillis()}.wav")
            audioResp.body?.byteStream()?.use { input ->
                FileOutputStream(file).use { output -> input.copyTo(output) }
            }
            return file
        }
    }

    private suspend fun playFile(file: File) {
        withContext(Dispatchers.Main.immediate) {
            try {
                mediaPlayer?.release()
            } catch (_: Throwable) {
            }
            val mp = MediaPlayer()
            mediaPlayer = mp
            mp.setDataSource(file.absolutePath)
            mp.setOnCompletionListener { _ ->
                _isSpeaking.value = false
            }
            mp.setOnErrorListener { _, _, _ ->
                _isSpeaking.value = false
                true
            }
            mp.prepare()
            _isSpeaking.value = true
            mp.start()
        }
    }
}
