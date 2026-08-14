package com.live2dai.android.core

import kotlinx.coroutines.flow.StateFlow

/**
 * TTS 播报接缝（契约 refactor-0814 §2.2 锁定：`speak` suspend 互斥 / `stop()` 幂等 / `isSpeaking: StateFlow<Boolean>`）。
 *
 * 实现归 B 片；本文件仅声明接口签名。
 * 约束：至多一个合成在途（I2 的 B 侧保障）；打断后 isSpeaking 收敛 false 且帧源停发。
 */
interface SpeechSink {
    /** 播报一段净化后文本；挂起到播放完成/失败；互斥（至多一个在途）。 */
    suspend fun speak(cleanText: String)

    /** 立即打断（幂等，可随时调用）。 */
    fun stop()

    /** 真实播放状态：start=播放开始→true；完成/错误/stop→false。 */
    val isSpeaking: StateFlow<Boolean>
}
