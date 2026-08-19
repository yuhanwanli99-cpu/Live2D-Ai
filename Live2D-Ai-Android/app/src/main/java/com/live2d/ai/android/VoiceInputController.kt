package com.live2d.ai.android

import android.content.Context
import android.content.Intent
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import android.os.Bundle
import android.speech.RecognitionListener
import android.speech.RecognizerIntent
import android.speech.SpeechRecognizer
import android.util.Log
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import java.util.concurrent.atomic.AtomicBoolean

/**
 * 语音输入控制器（P0 细化）。
 *
 * 支持两种引擎：
 * - system：Android 系统 [SpeechRecognizer]（默认，无需离线模型）
 * - sherpa：离线 sherpa-onnx SenseVoice（需下载模型，[SherpaAsrEngine]）
 *
 * 状态流：[VoiceInputState]。
 */
class VoiceInputController(
    private val context: Context,
    private val language: String = "zh-CN",
    private val sherpaEngine: SherpaAsrEngine? = null,
    private val sherpaSampleRate: Int = 16000,
) {
    private val tag = "VoiceInput"

    private val _state = MutableStateFlow<VoiceInputState>(VoiceInputState.Idle)
    val state: StateFlow<VoiceInputState> = _state.asStateFlow()

    private var recognizer: SpeechRecognizer? = null
    private var onResult: ((String) -> Unit)? = null

    // ── sherpa 离线录音状态 ──
    private var audioRecord: AudioRecord? = null
    private var recordingThread: Thread? = null
    private val recording = AtomicBoolean(false)
    private val pcmChunks = ArrayList<ShortArray>()

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
        if (sherpaEngine != null) {
            startOfflineRecording()
        } else {
            startSystemRecognition()
        }
    }

    fun stop() {
        if (sherpaEngine != null) {
            stopOfflineRecording()
        } else {
            recognizer?.stopListening()
        }
    }

    fun cancel() {
        recording.set(false)
        recordingThread?.join(500)
        recordingThread = null
        releaseAudioRecord()
        pcmChunks.clear()
        recognizer?.cancel()
        recognizer?.destroy()
        recognizer = null
        onResult = null
        _state.value = VoiceInputState.Idle
    }

    fun destroy() {
        cancel()
    }

    // ── 系统 SpeechRecognizer ──

    private fun startSystemRecognition() {
        try {
            val sr = SpeechRecognizer.createSpeechRecognizer(context.applicationContext)
            recognizer = sr
            sr.setRecognitionListener(systemListener)
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

    private val systemListener = object : RecognitionListener {
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

    // ── sherpa 离线录音识别 ──

    private fun startOfflineRecording() {
        val minBuf = AudioRecord.getMinBufferSize(
            sherpaSampleRate,
            AudioFormat.CHANNEL_IN_MONO,
            AudioFormat.ENCODING_PCM_16BIT
        )
        if (minBuf <= 0) {
            _state.value = VoiceInputState.Error("录音初始化失败")
            return
        }
        try {
            val record = AudioRecord(
                MediaRecorder.AudioSource.MIC,
                sherpaSampleRate,
                AudioFormat.CHANNEL_IN_MONO,
                AudioFormat.ENCODING_PCM_16BIT,
                minBuf.coerceAtLeast(4096) * 2
            )
            audioRecord = record
            pcmChunks.clear()
            recording.set(true)
            _state.value = VoiceInputState.Listening
            record.startRecording()
            recordingThread = Thread {
                val buf = ShortArray(minBuf.coerceAtLeast(4096) / 2)
                while (recording.get()) {
                    val read = try {
                        record.read(buf, 0, buf.size)
                    } catch (e: Exception) {
                        break
                    }
                    if (read > 0) {
                        synchronized(pcmChunks) {
                            pcmChunks.add(buf.copyOf(read))
                        }
                    }
                }
            }.apply { start() }
        } catch (e: Exception) {
            Log.w(tag, "AudioRecord start failed: ${e.message}")
            _state.value = VoiceInputState.Error(e.message ?: "无法启动离线录音")
            releaseAudioRecord()
        }
    }

    private fun stopOfflineRecording() {
        if (!recording.getAndSet(false)) return
        recordingThread?.join(1500)
        recordingThread = null
        releaseAudioRecord()

        val total = synchronized(pcmChunks) { pcmChunks.sumOf { it.size } }
        if (total == 0) {
            _state.value = VoiceInputState.Idle
            onResult = null
            return
        }
        val all = ShortArray(total)
        var offset = 0
        synchronized(pcmChunks) {
            for (chunk in pcmChunks) {
                chunk.copyInto(all, offset)
                offset += chunk.size
            }
            pcmChunks.clear()
        }
        val floats = FloatArray(all.size) { all[it] / 32768f }
        _state.value = VoiceInputState.Recognizing
        val text = sherpaEngine?.recognize(floats, sherpaSampleRate)?.trim().orEmpty()
        _state.value = VoiceInputState.Idle
        if (text.isNotEmpty()) {
            onResult?.invoke(text)
        }
        onResult = null
    }

    private fun releaseAudioRecord() {
        try {
            audioRecord?.stop()
        } catch (_: Throwable) {
        }
        try {
            audioRecord?.release()
        } catch (_: Throwable) {
        }
        audioRecord = null
    }
}
