package com.live2dai.android

import android.content.Context
import android.os.Build
import android.os.SystemClock
import android.speech.tts.TextToSpeech
import android.speech.tts.UtteranceProgressListener
import android.util.Log
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
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
class SystemTtsProvider(
    private val context: Context,
    /** M-AND-1 测试注入：直接提供 TextToSpeech 实例（视为已就绪；默认按 context 创建真实引擎）。 */
    private val ttsOverride: TextToSpeech? = null,
    /** M-AND-1 测试注入：时钟源（timestampMs = 词边界真实时间；默认 uptimeMillis，测试注入固定/递增时钟）。 */
    private val clock: () -> Long = { SystemClock.uptimeMillis() },
) : TtsProvider {

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
     * M-AND-1（2b-②）：词边界回调 → 音节包络帧流。
     * - UtteranceProgressListener 词边界回调期间 emit ≥ 1 帧（timestampMs = 词边界真实时间，幅度 = 音节包络 ∈ [0,1]）
     * - 合成完成（onDone）后帧流收口（不再 emit；500ms 内无新帧）
     * tryEmit 非阻塞（回调线程 = Binder 线程；缓冲满丢帧不阻塞引擎）。
     */
    private val _audioFrameFlow = MutableSharedFlow<AudioFrame>(replay = 0, extraBufferCapacity = 16)
    override val audioFrameFlow: Flow<AudioFrame> = _audioFrameFlow.asSharedFlow()

    private var androidTts: TextToSpeech? = null
    private var androidTtsReady = false

    /** M-AND-1（2b-②）：本次 speak 会话是否仍可 emit（onDone/onError 后关闭）。 */
    @Volatile
    private var framesOpen = false

    init {
        // M-AND-1：测试注入 ttsOverride 视为已就绪；否则初始化 Android 系统 TTS（降级链离线兜底）
        if (ttsOverride != null) {
            androidTts = ttsOverride
            androidTtsReady = true
        } else {
            androidTts = TextToSpeech(context) { status ->
                androidTtsReady = (status == TextToSpeech.SUCCESS)
                androidTts?.language = Locale.CHINESE
            }
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
        // M-AND-1（2b-②）：本次 speak 会话开启帧流；speak 起点墙钟（词边界真实时间的基准）
        framesOpen = true
        val speakStartClock = clock()
        // 先注册监听再 speak：避免 onStart 在监听器设置前触发而漏掉播放开始状态
        tts.setOnUtteranceProgressListener(object : UtteranceProgressListener() {
            override fun onStart(utteranceId: String?) {
                // 核心链路关键节点（plan §5.7.4：Log.i，国产 ROM 抑制 Log.d 教训）
                Log.i(TAG, "系统 TTS 开始发声（utteranceId=$utteranceId）")
                // 开始发声 → isSpeaking(true)（LipSync 驱动）
                _isSpeaking.value = true
                // M-AND-1（2b-②）：首帧 = 0（AudioFrame 契约）——首词音节包络帧
                emitEnvelopeFrame(LipSyncSystemTts.envelopeForWord(firstWord(text)), 0L)
            }
            override fun onRangeStart(utteranceId: String?, start: Int, end: Int, frame: Int) {
                // M-AND-1（2b-②）：词边界 → 帧（timestampMs = 词边界真实时间，幅度 = 音节包络）
                emitWordBoundaryFrame(text, start, end, speakStartClock)
            }
            override fun onDone(utteranceId: String?) {
                // 核心链路关键节点（plan §5.7.4：Log.i）
                Log.i(TAG, "系统 TTS 播报完成（utteranceId=$utteranceId）")
                // M-AND-1（2b-②）：合成完成 → 帧流收口（onDone 后 500ms 内无新帧）
                framesOpen = false
                // 播报完成 → isSpeaking(false)
                _isSpeaking.value = false
                if (!cont.isCompleted) cont.resume(Unit)
            }
            override fun onError(utteranceId: String?) {
                Log.w(TAG, "系统 TTS 播放错误")
                // M-AND-1（2b-②）：错误 → 帧流收口
                framesOpen = false
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

    /** M-AND-1（2b-②）：词边界回调 → 词文本音节包络帧（timestampMs = 词边界真实时间）。 */
    private fun emitWordBoundaryFrame(text: String, start: Int, end: Int, speakStartClock: Long) {
        if (!framesOpen) return
        val from = start.coerceIn(0, text.length)
        val to = end.coerceIn(from, text.length)
        val word = text.substring(from, to)
        val timestampMs = (clock() - speakStartClock).coerceAtLeast(0L)
        emitEnvelopeFrame(LipSyncSystemTts.envelopeForWord(word), timestampMs)
    }

    /** M-AND-1（2b-②）：emit 一帧包络（幅度 = envelope ∈ [0,1]，伪 PCM 全等采样）。 */
    private fun emitEnvelopeFrame(envelope: Float, timestampMs: Long) {
        if (!framesOpen) return
        val samples = FloatArray(LipSyncSystemTts.ENVELOPE_FRAME_SAMPLES) { envelope }
        _audioFrameFlow.tryEmit(
            AudioFrame(samples, LipSyncSystemTts.ENVELOPE_SAMPLE_RATE, timestampMs.coerceAtLeast(0L))
        )
    }

    /** 首词（onStart 首帧包络用）；无空白分隔时退回整段文本。 */
    private fun firstWord(text: String): String =
        text.trim().split(Regex("\\s+")).firstOrNull { it.isNotBlank() } ?: text
}

/**
 * M-AND-1 seam（2b-②）：词边界 → 音节包络（纯函数，AND1-A4 锚）。
 *
 * - [envelopeForWord]：音节数包络（CJK 每字 1 音节、拉丁元音组计数）→ ∈ [0,1]，单调递增、确定性
 * - [frameForWord]：词边界 → AudioFrame（幅度 = 音节包络，timestampMs = 词边界真实时间）
 * - 伪 PCM：帧采样全等于包络值（RMS 驱动下口型开合度 = 包络；无真实音频解码依赖）
 */
object LipSyncSystemTts {

    /** 包络帧采样率（Hz）：伪 PCM 仅承载幅度包络，任意 > 0 采样率即可。 */
    const val ENVELOPE_SAMPLE_RATE = 16000

    /** 包络帧采样数（10ms @ 16kHz）。 */
    const val ENVELOPE_FRAME_SAMPLES = 160

    /**
     * 音节包络值：syllables / (syllables + 2)，单调递增 ∈ [0,1)；空文本/无音节 → 0。
     * 音节计数：CJK 字符（0x4E00..0x9FFF）每字 1 音节；拉丁字母按元音组（a/e/i/o/u）计数。
     */
    fun envelopeForWord(word: String): Float {
        if (word.isBlank()) return 0f
        var syllables = 0
        var inVowelGroup = false
        for (ch in word) {
            val code = ch.code
            when {
                ch in 'a'..'z' || ch in 'A'..'Z' -> {
                    if (ch.lowercaseChar() in "aeiou") {
                        if (!inVowelGroup) {
                            syllables++
                            inVowelGroup = true
                        }
                    } else {
                        inVowelGroup = false
                    }
                }
                code in 0x4E00..0x9FFF -> syllables++
                else -> inVowelGroup = false
            }
        }
        if (syllables == 0) return 0f
        return (syllables.toFloat() / (syllables + 2).toFloat()).coerceIn(0f, 1f)
    }

    /** [envelopeForWord] 别名（音节包络）。 */
    fun syllableEnvelope(word: String): Float = envelopeForWord(word)

    /** [envelopeForWord] 别名（词包络）。 */
    fun envelope(word: String): Float = envelopeForWord(word)

    /**
     * 词边界 → AudioFrame（幅度 = 音节包络 ∈ [0,1]，timestampMs = 词边界真实时间，负值钳 0）。
     */
    fun frameForWord(
        word: String,
        timestampMs: Long,
        sampleRate: Int = ENVELOPE_SAMPLE_RATE,
    ): AudioFrame {
        val envelope = envelopeForWord(word)
        val samples = FloatArray(ENVELOPE_FRAME_SAMPLES) { envelope }
        return AudioFrame(samples, sampleRate, timestampMs.coerceAtLeast(0L))
    }

    /** [frameForWord] 别名（词边界帧）。 */
    fun wordBoundaryFrame(word: String, timestampMs: Long, sampleRate: Int = ENVELOPE_SAMPLE_RATE): AudioFrame =
        frameForWord(word, timestampMs, sampleRate)
}
