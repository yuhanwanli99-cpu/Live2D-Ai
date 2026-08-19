package com.live2d.ai.android

/**
 * 语音输入状态（P0）。
 */
sealed interface VoiceInputState {
    data object Idle : VoiceInputState
    data object Listening : VoiceInputState
    data object Recognizing : VoiceInputState
    data class Error(val message: String) : VoiceInputState
}
