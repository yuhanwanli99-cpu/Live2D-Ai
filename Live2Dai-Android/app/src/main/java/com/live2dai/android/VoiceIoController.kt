package com.live2dai.android

import android.content.Context
import android.util.Log
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

/**
 * V1.1 模块1「语音闭环接线」统一协调器（TTS 说 + ASR 听）+ 模块4「LipSync」（TTS 播放驱动嘴型）。
 *
 * 职责（契约见 docs/plan-v11-plan.schema.json → functions.VoiceIoController.*）：
 * - [speak]：播报纯文本（已剥离 [emotion] 标签）。EdgeTtsService 内部三层降级链
 *   （Edge TTS WebSocket → 系统 TTS → 静默），任何失败均静默降级、不影响对话主链路。
 * - [stopSpeaking]：新消息发送前打断当前 TTS 播报（停止 MediaPlayer + Android TTS）。
 * - [listen]：启动 Android SpeechRecognizer 识别，返回识别文本；失败/超时/无匹配
 *   返回空字符串（UI 层静默降级为文本输入，保持 V1.0 纯文本模式可用）。
 * - [release]：Activity 销毁时释放 TTS 与 ASR 资源（幂等）。
 * - [attachLive2DView]：模块4 接线点，Live2D 视图创建后注入，驱动嘴型动画。
 *
 * 模块4（LipSync，plan §4.1.4 方案A 播放状态驱动）：
 * 本类在首次 [speak] 时向 [EdgeTtsService] 注册 [EdgeTtsService.onPlaybackStateChanged] 回调
 * （主线程执行）：播放开始 → 启动正弦波嘴型协程（每 100ms 更新一次 mouth）；
 * 播放结束/打断 → 取消协程并将嘴型归零。不绑定具体 TTS 引擎，只依赖“是否在播放”状态。
 *
 * 容错约束（plan §8.2/§8.3）：TTS/ASR 任何一个失败都不影响文本输入；
 * LipSync 异常 → catch 后 mouth 设为 0.0，不影响 TTS 播放。
 * 本类所有方法对外不抛异常。
 */
class VoiceIoController(
    context: Context,
    private val ttsService: EdgeTtsService = EdgeTtsService(context),
    private val asrService: VoiceInputService = VoiceInputService(context),
    /** 模块4（LipSync）：驱动嘴型的 Live2D 视图；由 [attachLive2DView] 在视图创建后接线。 */
    private var live2DView: Live2DView? = null,
    /** LipSync 动画协程作用域（主线程；测试注入 TestDispatcher 控制虚拟时间）。 */
    private val lipSyncScope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
) {

    companion object {
        private const val TAG = "VoiceIo"

        /** LipSync 取证日志 TAG（项目教训：Log.i 级别，国产 ROM 抑制 Log.d） */
        private const val LIP_SYNC_TAG = "LipSync"

        /** ASR 单次识别超时：SpeechRecognizer 在部分 ROM 上可能不回调，超时后降级为空结果 */
        private const val LISTEN_TIMEOUT_MS = 15_000L

        /** LipSync 嘴型更新周期（plan §4.1.4：每 100-150ms） */
        internal const val LIP_SYNC_INTERVAL_MS = 100L
    }

    /** 当前 LipSync 动画协程（同一时刻至多一个；新播放开始即取消旧协程重启） */
    private var lipSyncJob: Job? = null

    /**
     * 注册 TTS 播放状态回调（模块4 接线，契约 schema §5.4，回调在主线程）。
     *
     * 延迟到首次 speak() 而非构造时注册：空白文本等纯文本路径不应触碰 TTS 服务；
     * 重挂幂等（每次播报前确保回调在场，播放开始后才能驱动嘴型）。
     */
    private fun ensurePlaybackCallbackRegistered() {
        ttsService.onPlaybackStateChanged = { isPlaying -> handlePlaybackState(isPlaying) }
    }

    /**
     * TTS 播报纯文本（已剥离 [emotion] 标签）。
     *
     * 由 ChatService.runAfterReceive → ChatHook.afterReceive 触发（与表情切换同一时机）；
     * 空文本直接跳过；内部异常全部吞掉（契约 throws: []）。
     */
    suspend fun speak(text: String) {
        if (text.isBlank()) return
        logD("speak() 开始播报: ${text.take(40)}")
        // 模块4：播报前确保播放状态回调已注册（LipSync 驱动）
        ensurePlaybackCallbackRegistered()
        try {
            ttsService.speak(text)
            logD("speak() 播报完成")
        } catch (e: Exception) {
            // 容错：TTS 失败不影响对话链路（降级回 V1.0 纯文本模式）
            logW("speak() 失败，已静默降级: ${e.message}")
        }
    }

    /**
     * 停止当前 TTS 播放（新消息发送前打断 / 清空对话时调用）。
     *
     * 经 EdgeTtsService.stop() 触发 onPlaybackStateChanged(false) → LipSync 协程停止、嘴型归零。
     */
    fun stopSpeaking() {
        logD("stopSpeaking() 打断当前 TTS 播报")
        try {
            ttsService.stop()
        } catch (e: Exception) {
            logW("stopSpeaking() 异常: ${e.message}")
        }
    }

    /**
     * 模块4：接线 Live2D 视图（驱动嘴型的动画目标）。
     *
     * Live2DView 在 Compose AndroidView factory 中创建（晚于本控制器构造），因此延迟注入；
     * 视图销毁/解绑时传 null。嘴型动画协程在 [handlePlaybackState] 中按需启动。
     */
    fun attachLive2DView(view: Live2DView?) {
        live2DView = view
        logD("attachLive2DView() ${if (view != null) "已接线" else "已解绑"}")
    }

    /**
     * 模块4：TTS 播放状态回调（EdgeTtsService.onPlaybackStateChanged 触发，主线程执行）。
     *
     * - true=开始播放 → 启动正弦波嘴型动画协程（plan §4.1.4 方案A）
     * - false=播放结束/中断 → 取消协程，嘴型归零
     *
     * internal 便于单元测试直接驱动（真实路径由 [speak] 注册的回调触发）。
     */
    internal fun handlePlaybackState(isPlaying: Boolean) {
        if (isPlaying) {
            startLipSync()
        } else {
            stopLipSync()
        }
    }

    /** 启动 LipSync 正弦波嘴型动画协程（幂等：先取消旧协程再重启）。 */
    private fun startLipSync() {
        lipSyncJob?.cancel()
        lipSyncJob = null
        val view = live2DView
        if (view == null) {
            logLipSync("播放开始（Live2DView 未接线，跳过嘴型动画）")
            return
        }
        logLipSync("播放开始")
        lipSyncJob = lipSyncScope.launch {
            var frame = 0L
            try {
                while (isActive) {
                    val value = LipSyncMath.mouthValue(frame)
                    view.setLipSyncParam(value)
                    logLipSync("嘴型更新 frame=$frame mouth=${formatMouth(value)}")
                    frame++
                    delay(LIP_SYNC_INTERVAL_MS)
                }
            } catch (e: CancellationException) {
                // 正常取消（新播报打断 / stopSpeaking / release）——交给 finally 归零
                throw e
            } catch (e: Exception) {
                // 容错（plan §8.3）：LipSync 异常不影响 TTS 播放，嘴型归零
                logLipSync("动画异常，嘴型归零: ${e.message}")
            } finally {
                // 无论正常结束 / 打断 / 异常，保证嘴型归零
                try {
                    view.setLipSyncParam(0f)
                } catch (_: Throwable) {
                }
            }
        }
    }

    /** 停止 LipSync 协程并立即将嘴型归零。 */
    private fun stopLipSync() {
        logLipSync("播放结束")
        lipSyncJob?.cancel()
        lipSyncJob = null
        val view = live2DView ?: return
        try {
            view.setLipSyncParam(0f)
        } catch (_: Throwable) {
        }
    }

    private fun formatMouth(value: Float): String = "%.2f".format(value)

    /**
     * ASR 开始监听，返回识别文本。
     *
     * @return 识别文本；失败/超时/无匹配返回 ""（UI 无变化，降级为文本输入）
     */
    suspend fun listen(): String {
        logD("listen() 开始语音识别（超时 ${LISTEN_TIMEOUT_MS / 1000}s）")
        return try {
            val result = withTimeoutOrNull(LISTEN_TIMEOUT_MS) { asrService.listen() }
                ?: run {
                    logW("listen() 识别超时，降级为文本输入")
                    asrService.stopListening()
                    ""
                }
            logD("listen() 识别结果: '${result.take(40)}'")
            result
        } catch (e: Exception) {
            // ASR 失败（系统服务不可用等）→ 静默降级为文本输入
            logW("listen() 识别失败，降级为文本输入: ${e.message}")
            ""
        }
    }

    /**
     * 释放 TTS + ASR 资源 + LipSync 动画（Activity 销毁时调用；幂等）。
     */
    fun release() {
        logD("release() 释放语音资源")
        // 模块4：停止嘴型动画并解绑视图（避免悬挂引用）
        lipSyncJob?.cancel()
        lipSyncJob = null
        live2DView = null
        // 解除播放状态回调（随后的 ttsService.release() 不再驱动 LipSync）
        ttsService.onPlaybackStateChanged = null
        try {
            ttsService.release()
            asrService.stopListening()
        } catch (e: Exception) {
            logW("release() 异常: ${e.message}")
        }
    }

    // 日志输出带单元测试防护（本地 JVM 测试无 Android 运行时，直接调用 Log 会抛
    // "Method d in android.util.Log not mocked"）——与 EmotionController 同一模式。
    private fun logD(message: String) {
        try {
            Log.d(TAG, message)
        } catch (_: Throwable) {
        }
    }

    private fun logW(message: String) {
        try {
            Log.w(TAG, message)
        } catch (_: Throwable) {
        }
    }

    /** LipSync 取证日志：Log.i 级别（项目教训：国产 ROM 抑制 Log.d）。 */
    private fun logLipSync(message: String) {
        try {
            Log.i(LIP_SYNC_TAG, message)
        } catch (_: Throwable) {
        }
    }
}
