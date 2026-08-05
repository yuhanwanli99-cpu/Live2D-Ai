package com.live2dai.android

import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import java.util.*
import kotlin.coroutines.resume

/**
 * Voice input service using Android's built-in SpeechRecognizer.
 * Returns recognized text as a string.
 *
 * For higher accuracy, consider also implementing a Groq Whisper fallback
 * that sends audio to a Windows proxy endpoint.
 */
class VoiceInputService(private val context: Context) {

    private var speechRecognizer: SpeechRecognizer? = null

    /**
     * Start listening and return recognized text.
     * Call from a coroutine context.
     */
    suspend fun listen(): String = withContext(Dispatchers.IO) {
        suspendCancellableCoroutine { cont ->
            try {
                speechRecognizer = SpeechRecognizer.createSpeechRecognizer(context)
                if (speechRecognizer == null) {
                    // 系统 SpeechRecognizer 创建失败（部分 ROM/权限异常）→ 立即降级为空结果，
                    // 避免协程永久挂起导致 UI 卡在"识别中"状态（plan §8.1 ASR 不可用→文本输入）
                    if (!cont.isCompleted) cont.resume("")
                    return@suspendCancellableCoroutine
                }
                val intent = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
                    putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
                    putExtra(RecognizerIntent.EXTRA_LANGUAGE, Locale.CHINESE)
                    putExtra(RecognizerIntent.EXTRA_MAX_RESULTS, 1)
                    putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, false)
                }

                speechRecognizer?.setRecognitionListener(object : RecognitionListener {
                    override fun onReadyForSpeech(params: Bundle?) {}
                    override fun onBeginningOfSpeech() {}
                    override fun onRmsChanged(rmsdB: Float) {}
                    override fun onBufferReceived(buffer: ByteArray?) {}
                    override fun onEndOfSpeech() {}
                    override fun onError(error: Int) {
                        val errorMsg = when (error) {
                            SpeechRecognizer.ERROR_NO_MATCH -> ""
                            SpeechRecognizer.ERROR_SPEECH_TIMEOUT -> ""
                            else -> ""
                        }
                        if (!cont.isCompleted) cont.resume(errorMsg)
                    }
                    override fun onResults(results: Bundle?) {
                        val matches = results?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
                        val text = matches?.firstOrNull() ?: ""
                        if (!cont.isCompleted) cont.resume(text)
                    }
                    override fun onPartialResults(partialResults: Bundle?) {}
                    override fun onEvent(eventType: Int, params: Bundle?) {}
                })

                speechRecognizer?.startListening(intent)
            } catch (e: Exception) {
                if (!cont.isCompleted) cont.resume("")
            }
        }
    }

    fun stopListening() {
        speechRecognizer?.stopListening()
        speechRecognizer?.destroy()
        speechRecognizer = null
    }
}
