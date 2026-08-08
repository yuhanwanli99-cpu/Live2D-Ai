package com.live2dai.android

import android.content.Context
import android.os.Build
import android.speech.tts.TextToSpeech
import android.speech.tts.UtteranceProgressListener
import android.util.Log
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.emptyFlow
import kotlinx.coroutines.suspendCancellableCoroutine
import java.util.Locale
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

/**
 * Android 系统 TTS Provider（V1.2 模块1，plan §3.1.8）。
 *
 * 从原 [EdgeTtsService] 剥离的 Android TextToSpeech 逻辑，作为独立 Provider 注册
 * （id="system_tts"，降级链 Tier 2：edge_tts 失败后的离线兜底）。
 *
 * 契约：speak 失败（引擎未就绪 / 播放错误）向上抛异常，由降级链处理；
 * 播放开始/完成通过 [isSpeaking] StateFlow 暴露（LipSync/UI 观察）。
 */
class SystemTtsProvider(private val context: Context) : TtsProvider {

    companion object {
        private const val TAG = "TtsProvider"

        /** Provider 唯一标识（契约 schema providersV12.system_tts） */
        const val PROVIDER_ID = "system_tts"
    }

    private val _health = MutableStateFlow(TtsHealth.OK)

    override val info = TtsProviderInfo(
        id = PROVIDER_ID,
        name = "系统 TTS",
        category = TtsProviderCategory.LOCAL,
        requiresNetwork = false,
        health = _health,
        // plan §3.1.6：Android 降级链 Tier 3 (priority=2)，Wave A 起即生效
        priority = 2,
        configurable = false,
    )

    private val _isSpeaking = MutableStateFlow(false)
    override val isSpeaking: StateFlow<Boolean> = _isSpeaking.asStateFlow()

    /**
     * M4：音频帧流（v1.0 空流）。
     *
     * Android TextToSpeech API 不暴露 PCM 音频，无解码头
     * （contract-M4-M5 §2.2 空流策略）。
     */
    override val audioFrameFlow: Flow<AudioFrame> = emptyFlow()

    private var androidTts: TextToSpeech? = null
    private var androidTtsReady = false

    init {
        // 初始化 Android 系统 TTS（降级链离线兜底）
        androidTts = TextToSpeech(context) { status ->
            androidTtsReady = (status == TextToSpeech.SUCCESS)
            androidTts?.language = Locale.CHINESE
        }
    }

    /**
     * 使用 Android 系统 TTS 播放文本（TtsProvider 契约）。
     *
     * 引擎未初始化（未就绪 / 已 release）时抛 [IllegalStateException] → 降级链继续；
     * 播放中途错误（onError）抛 [RuntimeException] → 降级链继续（若还有下一个 Provider）。
     */
    override suspend fun speak(text: String) {
        try {
            speakAndroid(text)
            _health.value = TtsHealth.OK
        } catch (e: Exception) {
            _health.value = TtsHealth.DEGRADED
            throw e
        }
    }

    /**
     * 停止当前播放（打断；幂等）。
     */
    override fun stop() {
        androidTts?.stop()
        // StateFlow 自动去重：未在播放时置 false 是 no-op
        _isSpeaking.value = false
    }

    /**
     * 释放所有资源（幂等）。
     */
    override fun release() {
        androidTts?.shutdown()
        androidTts = null
        _isSpeaking.value = false
    }

    private suspend fun speakAndroid(text: String) = suspendCancellableCoroutine<Unit> { cont ->
        val tts = androidTts
        if (tts == null || !androidTtsReady) {
            Log.w(TAG, "系统 TTS 未就绪，抛异常触发降级")
            cont.resumeWithException(IllegalStateException("系统 TTS 未就绪"))
            return@suspendCancellableCoroutine
        }
        // 先注册监听再 speak：避免 onStart 在监听器设置前触发而漏掉播放开始状态
        tts.setOnUtteranceProgressListener(object : UtteranceProgressListener() {
            override fun onStart(utteranceId: String?) {
                // 核心链路关键节点（plan §5.7.4：Log.i，国产 ROM 抑制 Log.d 教训）
                Log.i(TAG, "系统 TTS 开始发声（utteranceId=$utteranceId）")
                // 开始发声 → isSpeaking(true)（LipSync 驱动）
                _isSpeaking.value = true
            }
            override fun onDone(utteranceId: String?) {
                // 核心链路关键节点（plan §5.7.4：Log.i）
                Log.i(TAG, "系统 TTS 播报完成（utteranceId=$utteranceId）")
                // 播报完成 → isSpeaking(false)
                _isSpeaking.value = false
                if (!cont.isCompleted) cont.resume(Unit)
            }
            override fun onError(utteranceId: String?) {
                Log.w(TAG, "系统 TTS 播放错误")
                // 播报中断 → isSpeaking(false)
                _isSpeaking.value = false
                if (!cont.isCompleted) {
                    cont.resumeWithException(RuntimeException("系统 TTS 播放错误"))
                }
            }
        })
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
            tts.speak(text, TextToSpeech.QUEUE_FLUSH, null, "utterance_id")
        } else {
            @Suppress("DEPRECATION")
            tts.speak(text, TextToSpeech.QUEUE_FLUSH, null)
        }
    }
}
