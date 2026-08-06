package com.live2dai.android

import android.content.Context
import android.util.Log
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * 系统语音识别 Provider（V1.2 模块4，plan §4.4.4：VoiceInputService → SystemAsrProvider 包装）。
 *
 * 保留现有 [VoiceInputService]（Android SpeechRecognizer）作为 ASR 降级链 Tier 2
 * （id="system_speech_recognizer"，priority=1）。内部委托 [VoiceInputService]，
 * 行为与 V1.1 完全一致：失败/超时/无匹配返回 ""（降级为文本输入）。
 *
 * 契约偏差说明：schema providersV12.system_speech_recognizer 标注 requiresNetwork=false
 * （Gboard 离线包部分离线），沿用 schema 值。
 */
class SystemAsrProvider(private val context: Context) : AsrProvider {

    companion object {
        private const val TAG = "SystemAsr"

        /** Provider 唯一标识（契约 schema providersV12.system_speech_recognizer） */
        const val PROVIDER_ID = "system_speech_recognizer"
    }

    private val _health = MutableStateFlow(TtsHealth.OK)

    override val info = AsrProviderInfo(
        id = PROVIDER_ID,
        name = "系统语音识别",
        // schema：部分离线（Gboard 离线包），取 false
        requiresNetwork = false,
        health = _health,
        // ASR 降级链 Tier 2（plan §4.4.4）：sherpa_sensevoice(0) 失败后的系统兜底
        priority = 1,
    )

    private val delegate = VoiceInputService(context)

    override suspend fun listen(): String {
        val result = delegate.listen()
        if (result.isNotEmpty()) {
            _health.value = TtsHealth.OK
        }
        return result
    }

    override fun stopListening() {
        try {
            delegate.stopListening()
        } catch (e: Exception) {
            Log.w(TAG, "stopListening 异常: ${e.message}")
        }
    }

    override fun release() {
        stopListening()
    }
}
