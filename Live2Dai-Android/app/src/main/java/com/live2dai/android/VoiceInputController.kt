package com.live2dai.android

import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import android.util.Log
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * 语音输入控制器（P0 最小闭环）。
 *
 * 基于 Android 系统 [SpeechRecognizer]（API 26+），不引入额外依赖。
 * 识别结果通过回调交给核心环路（LoopCoordinator.sendMessage）。
 *
 * 状态流：[VoiceInputState]。
 */
class VoiceInputController(
    private val context: Context,
    private val language: String = "zh-CN",
) {
    private val tag = "VoiceInput"

    private val _state = MutableStateFlow<VoiceInputState>(VoiceInputState.Idle)
    val state: StateFlow<VoiceInputState> = _state.asStateFlow()

    private var recognizer: SpeechRecognizer? = null
    private var onResult: ((String) -> Unit)? = null

    val isListening: Boolean get() = _state.value == VoiceInputState.Listening

    /** 开始/停止切换；识别结果经 [onResult] 回调。 */
    fun toggle(onResult: (String) -> Unit) {
        if (isListening) {
            stop()
        } else {
            start(onResult)
        }
    }

    fun start(onResult: (String) -> Unit) {
        if (_state.value == VoiceInputState.Listening) return
        this.onResult = onResult
        try {
            val sr = SpeechRecognizer.createSpeechRecognizer(context.applicationContext)
            recognizer = sr
            sr.setRecognitionListener(listener)
            val intent = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
                putExtra(
                    RecognizerIntent.EXTRA_LANGUAGE_MODEL,
                    RecognizerIntent.LANGUAGE_MODEL_FREE_FORM
                )
                putExtra(RecognizerIntent.EXTRA_LANGUAGE, language)
                putExtra(RecognizerIntent.EXTRA_LANGUAGE_PREFERENCE, language)
                putExtra(RecognizerIntent.EXTRA_PARTIAL_RESULTS, true)
                putExtra(RecognizerIntent.EXTRA_MAX_RESULTS, 3)
            }
            _state.value = VoiceInputState.Listening
            sr.startListening(intent)
        } catch (e: Exception) {
            Log.w(tag, "startListening failed: ${e.message}")
            _state.value = VoiceInputState.Error(e.message ?: "无法启动语音识别")
            recognizer?.destroy()
            recognizer = null
        }
    }

    fun stop() {
        recognizer?.stopListening()
        // 状态由回调推进到 Recognizing/Idle；这里不强制置 Idle，避免吞掉结果。
    }

    fun cancel() {
        recognizer?.cancel()
        recognizer?.destroy()
        recognizer = null
        onResult = null
        _state.value = VoiceInputState.Idle
    }

    fun destroy() {
        recognizer?.destroy()
        recognizer = null
        onResult = null
    }

    private val listener = object : RecognitionListener {
        override fun onReadyForSpeech(params: Bundle?) {
            _state.value = VoiceInputState.Listening
        }

        override fun onBeginningOfSpeech() {
            _state.value = VoiceInputState.Listening
        }

        override fun onRmsChanged(rmsdB: Float) = Unit

        override fun onBufferReceived(buffer: ByteArray?) = Unit

        override fun onEndOfSpeech() {
            _state.value = VoiceInputState.Recognizing
        }

        override fun onError(error: Int) {
            val message = when (error) {
                SpeechRecognizer.ERROR_NO_MATCH -> "没有听清，请再说一次"
                SpeechRecognizer.ERROR_SPEECH_TIMEOUT -> "没有说话，已停止"
                SpeechRecognizer.ERROR_RECOGNIZER_BUSY -> "语音识别忙，请稍后再试"
                SpeechRecognizer.ERROR_INSUFFICIENT_PERMISSIONS -> "缺少录音权限"
                else -> "语音识别失败（$error）"
            }
            Log.w(tag, "onError: $error $message")
            _state.value = VoiceInputState.Error(message)
            recognizer?.destroy()
            recognizer = null
        }

        override fun onResults(results: Bundle?) {
            val text = results
                ?.getStringArrayList(SpeechRecognizer.RESULTS_RECOGNITION)
                ?.firstOrNull()
                ?.trim()
                .orEmpty()
            recognizer?.destroy()
            recognizer = null
            _state.value = VoiceInputState.Idle
            if (text.isNotEmpty()) {
                onResult?.invoke(text)
            }
            onResult = null
        }

        override fun onPartialResults(partialResults: Bundle?) = Unit

        override fun onEvent(eventType: Int, params: Bundle?) = Unit
    }
}
