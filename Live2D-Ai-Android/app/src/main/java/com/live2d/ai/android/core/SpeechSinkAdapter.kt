package com.live2d.ai.android.core

import com.live2d.ai.android.VoiceIoController
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.launch
import java.util.concurrent.atomic.AtomicBoolean

/**
 * [SpeechSink] 的 [VoiceIoController] 适配器（Wave 4 · 4-1 绑定规格 1；C 片 core/ 内，
 * 只引用 B 片类型 [VoiceIoController]，不依赖 A 片渲染类型）。
 *
 * - [speak]：转发 [VoiceIoController.speak]（suspend 互斥，至多一个合成在途——I2 的 B 侧保障）；
 *   非取消异常 → 发射 [speakFailure]（供 LoopWire SpeakFailed 泵）；VoiceIoController
 *   全失败静默降级（不抛）时本句从未出声（isSpeaking 未升起）→ 同样视为失败发射信号，
 *   避免状态机卡死在 SPEAKING（I7 fail-safe 的播报侧兜底）。
 * - [stop]：转发 [VoiceIoController.stopSpeaking]（幂等打断，遍历降级链 stop + 嘴型归零）。
 * - [isSpeaking]：直通 [VoiceIoController.isSpeaking]（StateFlow 组合流：任一 Provider 在播 = true）。
 */
class SpeechSinkAdapter(
    private val voiceIo: VoiceIoController,
) : SpeechSink {

    /**
     * 首选 TTS 引擎 ID（对应 SettingsRepository.ttsProviderId）。由装配方在设置保存后更新；
     * [speak] 转发给 [VoiceIoController.speak]，使引擎选择真正生效（F2.3）。null = 纯降级链。
     */
    var ttsProviderId: String? = null

    private val _speakFailure = MutableSharedFlow<Unit>(extraBufferCapacity = 1)

    /** 播报失败信号：speak 抛非取消异常 / 全失败静默未出声时发射一次（供 SpeakFailed 泵）。 */
    val speakFailure: SharedFlow<Unit> = _speakFailure.asSharedFlow()

    override suspend fun speak(cleanText: String) {
        // AtomicBoolean：observer 在后台线程写、本方法在主线程读，保证跨线程可见。
        // （审计 A-4：原裸 var 无内存可见性，成功出声也可能被误判为未出声。）
        val spoke = AtomicBoolean(false)
        var failed = false
        // 观察本句是否实际出声（isSpeaking 前沿）：并发收集，speak 返回时取消。
        val observer = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined).launch {
            voiceIo.isSpeaking.collect { if (it) spoke.set(true) }
        }
        try {
            voiceIo.speak(cleanText, providerId = ttsProviderId)
        } catch (ce: CancellationException) {
            throw ce // 打断/取消：正常语义，不算失败（打断路径由状态机 SpeechStop 处理）
        } catch (t: Throwable) {
            failed = true
        } finally {
            observer.cancel()
        }
        // 失败信号：speak 抛非取消异常，或全失败静默降级（本句从未出声）→ 播报失败
        if (failed || !spoke.get()) {
            _speakFailure.tryEmit(Unit)
        }
    }

    override fun stop() {
        voiceIo.stopSpeaking()
    }

    override val isSpeaking: StateFlow<Boolean> = voiceIo.isSpeaking
}
