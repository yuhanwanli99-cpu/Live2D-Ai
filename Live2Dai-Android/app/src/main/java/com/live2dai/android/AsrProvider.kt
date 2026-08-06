package com.live2dai.android

import kotlinx.coroutines.flow.StateFlow

/**
 * ASR Provider 元数据（V1.2 模块4，契约见 docs/plan-v12-plan.schema.json →
 * dataModels.AsrProviderInfo）。
 *
 * [priority] 为降级链优先级（数字越小越优先，0 最高），由 [AsrProviderRegistry.cascade]
 * 升序排列。与 [TtsProviderInfo] 同构，但 ASR 链路更简单（listen→文本），
 * 不需要 isSpeaking 播放状态。
 */
data class AsrProviderInfo(
    /** 唯一标识（lowercase_snake_case，如 "sherpa_sensevoice" / "system_speech_recognizer"） */
    val id: String,
    /** 人类可读名称（如 "SenseVoice 离线 ASR" / "系统语音识别"） */
    val name: String,
    /** 是否需要网络（false = 离线可用；系统 SpeechRecognizer 部分离线，按 schema 取 false） */
    val requiresNetwork: Boolean,
    /** 引擎健康状态（OK / DEGRADED / UNAVAILABLE；UNAVAILABLE 时降级链自动跳过） */
    val health: StateFlow<TtsHealth>,
    /** 降级链优先级（0 = 最高，数字越小越优先） */
    val priority: Int,
)

/**
 * ASR Provider 接口 — 所有语音识别引擎的统一合约（V1.2 模块4）。
 *
 * 契约见 docs/plan-v12-plan.schema.json → functions.AsrProvider.*：
 * - [listen] 挂起到识别结束；失败/超时/无匹配返回空字符串 ""（调用方/降级链继续尝试下一个）。
 * - [stopListening] 停止监听并释放麦克风，幂等。
 * - [release] 释放引擎资源（SpeechRecognizer / sherpa 引擎），幂等。
 *
 * 实现要求（plan §4.4.3）：内部异常不得外抛给调用方——通过返回空串暴露失败，
 * 由降级链（sherpa_sensevoice → system_speech_recognizer → 静默 ""）继续。
 */
interface AsrProvider {
    /** Provider 元数据（id / name / requiresNetwork / health / priority） */
    val info: AsrProviderInfo

    /**
     * 开始监听并返回识别文本。
     *
     * @return 识别的文本；失败/超时/无匹配返回空字符串 ""
     */
    suspend fun listen(): String

    /** 停止监听（打断当前识别）；幂等。 */
    fun stopListening()

    /** 释放引擎资源；幂等。 */
    fun release()
}
