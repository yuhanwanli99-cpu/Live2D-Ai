package com.live2d.ai.android

import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.emptyFlow

/**
 * Provider 分类（V1.2 模块1，对标 N.E.K.O. tts_client/workers/ 三分类）。
 *
 * 契约见 docs/plans/plan-v12-plan.schema.json → dataModels.TtsProviderCategory：
 * - CLOUD_WS:    WebSocket 流式双工（CosyVoice DashScope Realtime / Edge TTS）
 * - CLOUD_HTTP:  HTTP 按句请求（阿里云 REST 等）
 * - LOCAL:       本地推理，零网络依赖（sherpa-onnx / 系统 TTS）
 */
enum class TtsProviderCategory {
    /** WebSocket 流式双工 — 首音频延迟最低 */
    CLOUD_WS,

    /** HTTP 按句请求 — 实现最简单 */
    CLOUD_HTTP,

    /** 本地推理 — 零网络依赖 */
    LOCAL,
}

/**
 * Provider 元数据（注册表条目，契约见 schema → dataModels.TtsProviderInfo）。
 *
 * [priority] 为降级链优先级（数字越小越优先，0 最高），由 [TtsProviderRegistry.cascade] 升序排列。
 */
data class TtsProviderInfo(
    /** 唯一标识（lowercase_snake_case，如 "cosyvoice_ws" / "edge_tts" / "system_tts"） */
    val id: String,
    /** 人类可读名称（如 "Edge TTS" / "系统 TTS"） */
    val name: String,
    /** 分类（CLOUD_WS | CLOUD_HTTP | LOCAL） */
    val category: TtsProviderCategory,
    /** 是否需要网络（false = 离线可用） */
    val requiresNetwork: Boolean,
    /** 引擎健康状态（OK / DEGRADED / UNAVAILABLE，供可观测性） */
    val health: StateFlow<TtsHealth>,
    /** 降级链优先级（0 = 最高，数字越小越优先） */
    val priority: Int,
    /** 是否有用户可配置参数（API Key 等） */
    val configurable: Boolean,
)

/**
 * TTS Provider 接口 — 所有 TTS 引擎的统一合约（V1.2 模块1，架构地基）。
 *
 * 契约见 docs/plans/plan-v12-plan.schema.json → functions.TtsProvider.*：
 * - [speak] 是挂起函数，挂起到播放完成或失败；失败必须抛异常（由调用方/降级链处理），
 *   被 [stop] 打断时抛出 [kotlinx.coroutines.CancellationException]（协程正常取消）。
 * - [stop] 立即停止当前播放（新消息打断），幂等。
 * - [isSpeaking] 供 LipSync / UI / 可观测性观察播放状态。
 * - [release] 释放引擎资源（音频播放器、WebSocket 连接、Android TTS 等），幂等。
 *
 * 实现要求（plan §3.1.3）：speak 内部不得吞掉失败（否则降级链无法感知）；
 * 每次 speak 成功/失败应更新 [TtsProviderInfo.health] 供可观测性使用。
 */
interface TtsProvider {
    /** Provider 元数据（id / name / category / priority 等） */
    val info: TtsProviderInfo

    /** 播放状态流 — true=正在播放，false=空闲（LipSync/UI 观察） */
    val isSpeaking: StateFlow<Boolean>

    /**
     * 播放位置时钟（M-AND-1 选型 3-B）：MediaPlayer.getCurrentPosition 轮询重锚。
     *
     * - `onPrepared→start` 期间恒 0 且不抛异常；`start()` 后非递减
     * - 轮询周期 = [LipSyncClock.PLAYBACK_POLL_INTERVAL_MS]（固定常数 ∈ [50,100]ms，非抖动值）
     * - 消费侧重锚：嘴型帧调度墙钟时刻 = `reanchor(frameTsMs, playStartWallClockMs)`
     *   = playStartWallClockMs + frameTsMs（clamp 下限 = playStartWallClockMs，不早于播放起点）
     *
     * 默认实现：恒 0（无 MediaPlayer 的 Provider / 测试假件；与 TtsProvider 接口向后兼容）。
     */
    val playbackPositionMs: StateFlow<Long>
        get() = MutableStateFlow(0L)

    /**
     * 合成并播放文本。挂起直到播放完成、失败或被打断。
     *
     * @param text 已剥离 [emotion] 标签的纯文本
     * @throws Exception 播放失败（由上层降级链捕获并尝试下一个 Provider）
     * @throws kotlinx.coroutines.CancellationException 被 [stop] 打断时（协程正常取消）
     */
    suspend fun speak(text: String)

    /** 立即停止当前播放（打断）。幂等。 */
    fun stop()

    /** 释放引擎资源（音频播放器、WebSocket 连接等）。幂等。 */
    fun release()

    /**
     * 音频帧流（M-AND-1：全 Provider 非空，嘴型驱动源统一）。
     *
     * - 有能力暴露 PCM 的 Provider（sherpa 2c-① / Edge / CosyVoice 2a-①）→ speak() 执行期间 emit AudioFrame
     *   （150ms 固定分块：首帧 timestampMs=0、相邻帧差=150ms±1ms、同 fixture 双跑确定性一致）
     * - System TTS（2b-②）→ UtteranceProgressListener 词边界回调期间 emit 音节包络帧
     *   （timestampMs=词边界真实时间、幅度∈[0,1]；onDone 后 500ms 内流收口）
     * - 消费者（VoiceIoController）在调用 speak() **之前**启动 collect（时序契约 contract-M4-M5 §2.3 / AND1-A7：
     *   speak 后 1s 内收到首帧）；流式 Provider 用 tryEmit 非阻塞（无订阅者/缓冲满 → 丢块 + Log.w，不影响播放）
     *
     * 默认实现：空流（向后兼容——不实现该成员的 Provider/测试假件也能编译）。
     */
    val audioFrameFlow: Flow<AudioFrame>
        get() = emptyFlow()
}
