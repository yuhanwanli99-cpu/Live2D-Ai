package com.live2dai.android

import android.util.Log
import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow

/**
 * TTS/ASR 链路遥测事件（V1.2 模块7「可观测性」，plan §5.7.3）。
 *
 * 契约见 docs/plan-v12-plan.schema.json → dataModels.VoiceTelemetryEvent：
 * - TTS 链路：TtsStarted → TtsSucceeded / TtsFailed（降级切换时伴随 TtsFallback）
 * - ASR 链路：AsrStarted → AsrResult / AsrFailed
 * - 模型下载（可选）：ModelDownloadStarted / ModelDownloadCompleted
 *
 * 所有事件携带 [timestamp]（epoch millis），供日志 dump / 调试面板排序展示。
 * 事件由 [VoiceTelemetry.log] 统一落盘（Log.i + 环形缓冲区 + [VoiceEventBus]），
 * 调用方（VoiceIoController 埋点 / Provider）只负责在正确时机构造事件。
 */
sealed class VoiceEvent {

    /** TTS 合成开始（首选 Provider 确定后、首次 speak 调用前发射）。 */
    data class TtsStarted(
        val providerId: String,
        val textLength: Int,
        val timestamp: Long,
    ) : VoiceEvent()

    /** TTS 合成+播放成功（speak 返回时发射；durationMs = speak 挂起时长）。 */
    data class TtsSucceeded(
        val providerId: String,
        val durationMs: Long,
        val timestamp: Long,
    ) : VoiceEvent()

    /** TTS Provider 失败（speak 抛异常时发射；reason = 异常消息）。 */
    data class TtsFailed(
        val providerId: String,
        val reason: String,
        val timestamp: Long,
    ) : VoiceEvent()

    /** TTS 降级切换（当前 Provider 失败、准备尝试下一个时发射）。 */
    data class TtsFallback(
        val fromProvider: String,
        val toProvider: String,
        val reason: String,
        val timestamp: Long,
    ) : VoiceEvent()

    /** ASR 开始监听（listen 进入降级链中某 Provider 前发射）。 */
    data class AsrStarted(
        val providerId: String,
        val timestamp: Long,
    ) : VoiceEvent()

    /**
     * ASR 识别结果：text=null 表示完成但无匹配/空结果（降级链继续尝试下一个）；
     * text 非空 = 成功识别（调用方立即返回该结果）。
     */
    data class AsrResult(
        val providerId: String,
        val text: String?,
        val durationMs: Long,
        val timestamp: Long,
    ) : VoiceEvent()

    /** ASR 失败（异常 / 超时）。 */
    data class AsrFailed(
        val providerId: String,
        val reason: String,
        val timestamp: Long,
    ) : VoiceEvent()

    /** 模型下载开始（可选埋点，供模型下载链路观察）。 */
    data class ModelDownloadStarted(
        val modelId: String,
        val timestamp: Long,
    ) : VoiceEvent()

    /** 模型下载完成（可选埋点；bytes = 模型字节数）。 */
    data class ModelDownloadCompleted(
        val modelId: String,
        val bytes: Long,
        val timestamp: Long,
    ) : VoiceEvent()
}

/**
 * 遥测事件总线（plan §5.7.3「事件收集」）——所有经 [VoiceTelemetry.log] 的事件
 * 同步转发到此共享流，供调试面板 / 未来可观测 UI 订阅实时事件流。
 *
 * 缓冲策略：extraBufferCapacity=[VoiceTelemetry.RING_SIZE] + DROP_OLDEST
 * （与环形缓冲区一致；消费者慢于生产者时不阻塞主链路——plan §8.3「遥测不影响主链路」）。
 * 不设置 replay：历史事件通过 [VoiceTelemetry.recentEvents] 查询，总线只做实时订阅。
 */
object VoiceEventBus {

    private val _events = MutableSharedFlow<VoiceEvent>(
        extraBufferCapacity = VoiceTelemetry.RING_SIZE,
        onBufferOverflow = BufferOverflow.DROP_OLDEST,
    )

    /** 实时事件流（不重放历史；订阅即开始接收新事件）。 */
    val events: SharedFlow<VoiceEvent> = _events.asSharedFlow()

    /**
     * 转发事件到共享流。tryEmit 不挂起；无订阅者时事件被缓冲后丢弃
     * （遥测非关键路径，失败/满缓冲静默，不影响主链路）。
     */
    fun emit(event: VoiceEvent) {
        try {
            _events.tryEmit(event)
        } catch (_: Throwable) {
        }
    }
}

/**
 * 链路遥测入口（契约 schema → functions.VoiceTelemetry.log）。
 *
 * 每次 [log] 做三件事：
 * 1. **Log.i 写入**（国产 ROM 抑制 Log.d 教训——plan §5.7.4 日志规范，
 *    核心链路关键节点必须 Log.i 级别可取证）
 * 2. **环形缓冲区追加**（最近 [RING_SIZE] 条，供调试面板 dump / 单测断言）
 * 3. **转发 [VoiceEventBus]**（实时订阅）
 *
 * 容错（plan §8.3「可观测性」回退策略）：Log / 缓冲失败全部吞掉，
 * 遥测绝不影响 TTS/ASR 主链路。
 */
object VoiceTelemetry {

    private const val TAG = "VoiceTelemetry"

    /** 环形缓冲区容量（schema 契约：最近 100 条）。 */
    const val RING_SIZE = 100

    /** 最近事件环形缓冲区（线程安全：speak/listen 可能跨线程调用）。 */
    private val ring = ArrayDeque<VoiceEvent>()
    private val lock = Any()

    /**
     * 记录一条遥测事件：Log.i + 环形缓冲区 + 事件总线（幂等、线程安全）。
     *
     * 全部副作用内部 try/catch：JVM 单测无 Android 运行时（Log 抛
     * "not mocked"）、缓冲异常——一律静默，保证主链路不受影响。
     */
    fun log(event: VoiceEvent) {
        try {
            Log.i(TAG, event.toString())
        } catch (_: Throwable) {
            // JVM 单测无 Android 运行时 / 日志系统抑制——遥测非关键路径
        }
        synchronized(lock) {
            ring.addLast(event)
            while (ring.size > RING_SIZE) ring.removeFirst()
        }
        VoiceEventBus.emit(event)
    }

    /** 最近 [RING_SIZE] 条遥测事件快照（时间序；调试面板 dump / 单测断言）。 */
    fun recentEvents(): List<VoiceEvent> = synchronized(lock) { ring.toList() }

    /** 清空环形缓冲区（测试隔离用；不影响事件总线——SharedFlow 不可回退）。 */
    fun clear() = synchronized(lock) { ring.clear() }
}
