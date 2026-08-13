package com.live2dai.android

import android.content.Context
import android.os.SystemClock
import android.util.Log
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

/**
 * V1.1 模块1「语音闭环接线」统一协调器（TTS 说 + ASR 听）+ 模块4「LipSync」（TTS 播放驱动嘴型）
 * + V1.2 模块1「TtsProvider 注册表 + 降级链」（多引擎架构地基）。
 *
 * 职责（契约见 docs/plan-v12-plan.schema.json → functions.VoiceIoController.*）：
 * - [speak]：播报纯文本（已剥离 [emotion] 标签）。遍历 [TtsProviderRegistry.cascade()]
 *   降级链（Wave A：edge_tts → system_tts），按序尝试 [TtsProvider.speak]，成功即停；
 *   失败继续下一个；全失败静默降级（不影响对话主链路）。可指定 [providerId] 首选引擎。
 * - [stopSpeaking]：新消息发送前打断当前 TTS 播报（遍历降级链调用 stop()，
 *   确保切 Provider 后旧引擎资源释放）。
 * - [isSpeaking]：组合各 Provider 播放状态的 StateFlow（任一在播 = true）。
 * - [listen]：遍历 ASR Provider 降级链识别，返回识别文本；失败/超时/无匹配
 *   返回空字符串（UI 层静默降级为文本输入，保持 V1.0 纯文本模式可用）。
 * - 模块7 遥测（plan §5.7.3）：speak 时发射 TtsStarted → TtsSucceeded/TtsFailed
 *   （降级切换伴随 TtsFallback）；listen 时发射 AsrStarted → AsrResult/AsrFailed。
 *   事件经 [VoiceTelemetry.log] 写入 Log.i + 环形缓冲区 + [VoiceEventBus]。
 * - [release]：Activity 销毁时释放所有 Provider 与 ASR 资源（幂等）。
 * - [attachLive2DView]：模块4 接线点，Live2D 视图创建后注入，驱动嘴型动画。
 *
 * 模块4（LipSync，plan §4.1.4 方案A 播放状态驱动）：
 * V1.2 起 LipSync 由各 Provider 的 [TtsProvider.isSpeaking] StateFlow 驱动（替代 V1.1 的
 * EdgeTtsService.onPlaybackStateChanged 回调）：构造时收集组合流，任一 Provider
 * 播放开始 → 启动随机波动嘴型协程（级2-B：每 100ms 更新一次 mouth，随机波动 + 停顿模拟 + lerp 平滑）；
 * 播放结束/打断 → 取消协程并将嘴型归零。不绑定具体 TTS 引擎，只依赖“是否在播放”状态。
 *
 * 容错约束（plan §8.2/§8.3）：TTS/ASR 任何一个失败都不影响文本输入；
 * LipSync 异常 → catch 后 mouth 设为 0.0，不影响 TTS 播放。
 * 本类所有方法对外不抛异常（协程取消 [CancellationException] 除外——正常取消语义）。
 */
class VoiceIoController(
    context: Context,
    /** V1.2：TTS Provider 降级链（默认取注册表，测试可注入 mock）。 */
    private val providers: List<TtsProvider> = TtsProviderRegistry.cascade(),
    /** V1.2 模块4：ASR Provider 降级链（默认取注册表；测试可注入 mock）。 */
    private val asrProviders: List<AsrProvider> = AsrProviderRegistry.cascade(),
    /** 模块4（LipSync）：驱动嘴型的 Live2D 视图；由 [attachLive2DView] 在视图创建后接线。 */
    private var live2DView: Live2DView? = null,
    /** LipSync 动画协程作用域（主线程；测试注入 TestDispatcher 控制虚拟时间）。 */
    private val lipSyncScope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Main),
    /** M5：元音分析器；null = 禁用 viseme 驱动（走现有 RMS 随机降级），测试可注入 */
    private val visemeAnalyzer: VisemeAnalyzer? = null,
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

    /** ASR 降级链（构造时确定；注册表为空时兜底注入系统 ASR，保证 listen() 永远可用） */
    private val asrChain: List<AsrProvider>

    /** 当前 LipSync 动画协程（同一时刻至多一个；新播放开始即取消旧协程重启） */
    private var lipSyncJob: Job? = null

    /** viseme 分析收集协程（CPU 密集 → Dispatchers.Default；口型应用仍在 lipSyncScope/主线程） */
    private var visemeCollectJob: Job? = null

    /** M7 关键词兜底调度协程（contract-M7-android-l3.md §4；同 lipSyncScope 生命周期） */
    private var keywordMotionJob: Job? = null

    /** 播放状态观察协程（组合各 Provider isSpeaking → 更新 [isSpeaking] + 驱动 LipSync） */
    private var playbackObserverJob: Job? = null

    private val _isSpeaking = MutableStateFlow(false)

    /**
     * 组合播放状态流（契约 schema → functions.VoiceIoController.isSpeaking）：
     * 任一已注册 Provider 处于播放状态时 = true，全部空闲 = false。
     */
    val isSpeaking: StateFlow<Boolean> = _isSpeaking.asStateFlow()

    init {
        // V1.2 模块4：ASR 降级链兜底——注册表为空时（如未调用 registerDefaultAsrProviders）
        // 注册系统 ASR 并用于本实例，保证 listen() 永远有降级可用（plan §4.4.4
        // “ASR 注册点 = MainActivity 或 VoiceIoController init”）
        if (asrProviders.isEmpty()) {
            val system = SystemAsrProvider(context)
            AsrProviderRegistry.register(system)
            asrChain = listOf(system)
        } else {
            asrChain = asrProviders
        }
        // V1.2 模块1：观察各 Provider 播放状态（StateFlow 自动去重，仅状态切换时触发）。
        // 去重保护：仅当组合值实际变化才转发（combine 在任一上游发射时都会重算，
        // 组合值不变时不重复驱动 handlePlaybackState，避免打断正在运行的嘴型动画；
        // 初始 idle=false 与 _isSpeaking 初值一致，跳过）。
        if (providers.isNotEmpty()) {
            playbackObserverJob = lipSyncScope.launch {
                combine(providers.map { it.isSpeaking }) { states -> states.any { it } }
                    .collect { playing ->
                        if (playing != _isSpeaking.value) {
                            _isSpeaking.value = playing
                            handlePlaybackState(playing)
                        }
                    }
            }
        }
    }

    /**
     * TTS 播报纯文本（已剥离 [emotion] 标签）。
     *
     * task-fix-tts-sanitize：speak 是 TTS 播报统一入口（MainActivity 两条路径
     * ChatHook.afterReceive / onAssistantMessage 都调这里），第一行统一净化——
     * [EmotionController.sanitizeForTts] 剥离 [emotion] 标签 + 颜文字括号段
     * （(=^･ω･^=) 会被 TTS 读成"等于号"）+ markdown 符号，之后所有逻辑用净化后的文本。
     * 净化后为空文本 → 直接跳过（与空文本短路同语义）。
     *
     * 降级链（plan §3.1.7）：按序尝试 [providers]，成功即停；失败继续下一个；
     * 全失败静默降级（日志 Log.i 级别记录哪个 Provider 成功/失败）。
     *
     * @param providerId 指定首选 Provider ID（如 "system_tts"）；指定 Provider 失败后
     *                   仍按 priority 遍历其余 Provider 降级；null = 纯降级链遍历。
     *                   空文本直接跳过；内部异常全部吞掉（契约 throws: []）。
     * @param keywordMotionEnabled M7：是否允许关键词兜底触发 motion；false = 该次回复已由
     *                             motionLabel 驱动（contract-M7 §3.2 防线①）；默认 true。
     */
    suspend fun speak(text: String, providerId: String? = null, keywordMotionEnabled: Boolean = true) {
        if (text.isBlank()) return
        // 统一净化：剥离 [emotion] 标签 + 颜文字 + markdown 符号（只影响播报，不影响字幕/UI）
        val clean = EmotionController.sanitizeForTts(text)
        if (clean.isBlank()) {
            logD("speak() 净化后为空文本，跳过播报: ${text.take(40)}")
            return
        }
        logD("speak() 开始播报: ${clean.take(40)}")
        val chain = resolveChain(providerId)
        if (chain.isEmpty()) {
            logW("speak() 无可用 provider（providerId=$providerId），静默降级")
            return
        }
        // 模块7 遥测：首选 Provider 的合成开始事件（降级链首元素——无论最终哪个成功）
        VoiceTelemetry.log(
            VoiceEvent.TtsStarted(chain.first().info.id, clean.length, System.currentTimeMillis())
        )
        for ((index, provider) in chain.withIndex()) {
            val startedAt = System.currentTimeMillis()
            try {
                // M5: 在 speak() 前启动 viseme 分析收集（Dispatchers.Default 用于 CPU 密集的 MFCC/DTW）
                visemeAnalyzer?.reset()
                visemeCollectJob?.cancel()
                visemeCollectJob = visemeAnalyzer?.let { analyzer ->
                    lipSyncScope.launch(Dispatchers.Default) {
                        try {
                            provider.audioFrameFlow.collect { frame ->
                                analyzer.processFrame(frame)
                            }
                        } catch (_: CancellationException) {
                            // 正常取消
                        } catch (e: Exception) {
                            logW("viseme collect error: ${e.message}")
                        }
                    }
                }
                // M7：关键词兜底调度（motionLabel 驱动时调用方传 keywordMotionEnabled=false 整体禁用；
                // 无命中 → 零开销不启动协程）
                keywordMotionJob?.cancel()
                val keywordHits =
                    if (keywordMotionEnabled) KeywordMotionTable.find(clean) else emptyList()
                keywordMotionJob = if (keywordHits.isNotEmpty()) {
                    lipSyncScope.launch {
                        scheduleKeywordMotions(provider, clean, keywordHits)
                    }
                } else null
                provider.speak(clean)
                VoiceTelemetry.log(
                    VoiceEvent.TtsSucceeded(
                        provider.info.id,
                        System.currentTimeMillis() - startedAt,
                        System.currentTimeMillis(),
                    )
                )
                logI("speak() 由 ${provider.info.id} 播报完成")
                return
            } catch (e: CancellationException) {
                // 协程正常取消（stop() 打断 / 上层取消）——向上传播，不再尝试降级
                throw e
            } catch (e: Exception) {
                val reason = e.message ?: "unknown"
                // 模块7 遥测：当前 Provider 失败 + 降级切换（还有下一个时）
                VoiceTelemetry.log(
                    VoiceEvent.TtsFailed(provider.info.id, reason, System.currentTimeMillis())
                )
                val next = chain.getOrNull(index + 1)
                if (next != null) {
                    VoiceTelemetry.log(
                        VoiceEvent.TtsFallback(
                            provider.info.id, next.info.id, reason, System.currentTimeMillis()
                        )
                    )
                }
                // 降级链遍历日志（国产 ROM 抑制 Log.d 教训：用 Log.i 级别）
                logI("speak() ${provider.info.id} 失败，降级到下一个: $reason")
            } finally {
                visemeCollectJob?.cancel()
                visemeCollectJob = null
                keywordMotionJob?.cancel()
                keywordMotionJob = null
            }
        }
        logW("speak() 所有 provider 均失败，静默降级")
    }

    /**
     * M7 关键词兜底调度（contract-M7-android-l3.md §4.3）。
     *
     * 进度源 = provider.audioFrameFlow（sherpa 预分析模式：播放前全量 emit → totalMs 可得）：
     *   - 有帧 → 计算每个 hit 的触发时间 t(pos) = pos / clean.length × totalMs（匀速近似，无逐字时间戳）
     *     → 按 t 升序 delay 触发 startMotionByTag(hit.alias, NORMAL)。
     *   - 无进度源（空流/无帧）→ 降级：播放开始（provider.isSpeaking 变 true）触发首个命中一次。
     *
     * 静默降级：任何异常吞掉（不影响 TTS 播放，§4.4 不打断主链路）；Live2DView 未接线 → 跳过。
     */
    private suspend fun scheduleKeywordMotions(
        provider: TtsProvider,
        clean: String,
        hits: List<KeywordMotionTable.Hit>,
    ) {
        val view = live2DView ?: return
        try {
            var totalMs = 0L
            var frameCount = 0
            // 帧收集协程（与 viseme collect 同生命周期：speak 执行期间；CPU 密集 → Default）
            val collectJob = lipSyncScope.launch(Dispatchers.Default) {
                try {
                    provider.audioFrameFlow.collect { frame ->
                        val frameDur = if (frame.sampleRate > 0)
                            frame.samples.size * 1000L / frame.sampleRate else 0L
                        totalMs = frame.timestampMs + frameDur
                        frameCount++
                    }
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    logW("关键词帧收集异常: ${e.message}")
                }
            }
            // 等待播放开始（sherpa 预分析：帧已在播放前 emit 完 → totalMs 已定稿）
            provider.isSpeaking.first { it }
            if (frameCount > 0 && totalMs > 0) {
                // 进度模式：按匀速近似 t(pos) 调度触发
                val startMs = nowMillis()
                for (hit in hits) {
                    val tMs = hit.position.toLong() * totalMs / clean.length.coerceAtLeast(1)
                    val elapsed = nowMillis() - startMs
                    if (tMs > elapsed) delay(tMs - elapsed)
                    view.startMotionByTag(hit.alias, MotionPriority.NORMAL)
                }
            } else {
                // 降级：无进度源 → 播放开始触发首个命中一次
                logI("关键词兜底降级：无进度源（空流/无帧），播放开始触发首个命中一次")
                view.startMotionByTag(hits.first().alias, MotionPriority.NORMAL)
            }
            collectJob.cancel()
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            // 不打断主链路：调度异常静默吞掉（contract-M7 §4.4）
            logW("关键词兜底调度异常（不影响 TTS 播放）: ${e.message}")
        }
    }

    /**
     * 构造播报链：指定 providerId 时该 Provider 优先，其后按降级链顺序补其余 Provider；
     * 未指定时直接用降级链（[providers] 已按 priority 升序）。
     */
    private fun resolveChain(providerId: String?): List<TtsProvider> {
        if (providerId == null) return providers
        return providers.filter { it.info.id == providerId } +
            providers.filter { it.info.id != providerId }
    }

    /**
     * 停止当前 TTS 播放（新消息发送前打断 / 清空对话时调用）。
     *
     * 遍历降级链调用 [TtsProvider.stop]（幂等）：确保当前播放引擎停止，
     * 且切换 Provider 后旧引擎资源释放（plan §3.1.7）。
     * 各 Provider isSpeaking → false → LipSync 协程停止、嘴型归零。
     */
    fun stopSpeaking() {
        logD("stopSpeaking() 打断当前 TTS 播报")
        providers.forEach { provider ->
            try {
                provider.stop()
            } catch (e: Exception) {
                logW("stopSpeaking() ${provider.info.id} 异常: ${e.message}")
            }
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
        // C7d：LipSync 口型让位接线——组合播放状态注入渲染层（TTS 播放期间表情帧 MouthOpenY 让位给
        // LipSync 独占，防双写者交替写同一参数致嘴部抽搐）。view 为 null（解绑）→ 不注入，渲染层保持不剔除。
        view?.setLipSyncSpeakingProvider { isSpeaking.value }
        logD("attachLive2DView() ${if (view != null) "已接线" else "已解绑"}")
    }

    /**
     * 模块4：TTS 播放状态回调（组合 isSpeaking 流触发，主线程执行）。
     *
     * - true=开始播放 → 启动随机波动嘴型动画协程（级2-B：plan §4.1.4 方案A + 随机化）
     * - false=播放结束/中断 → 取消协程，嘴型归零
     *
     * internal 便于单元测试直接驱动（真实路径由 [init] 中的组合流收集触发）。
     */
    internal fun handlePlaybackState(isPlaying: Boolean) {
        if (isPlaying) {
            startLipSync()
        } else {
            stopLipSync()
        }
    }

    /** 启动 LipSync 动画协程（幂等：先取消旧协程再重启）。M5：优先 viseme 驱动，降级走 randomizedMouth。 */
    private fun startLipSync() {
        lipSyncJob?.cancel()
        lipSyncJob = null
        val view = live2DView
        if (view == null) {
            logLipSync("播放开始（Live2DView 未接线，跳过嘴型动画）")
            return
        }

        // M5 级1：尝试 viseme 驱动
        val analyzer = visemeAnalyzer
        if (analyzer != null) {
            val timeline = analyzer.getVisemeTimeline()
            if (timeline.isNotEmpty() && timeline.any { it.vowelLabel != "SIL" }) {
                logLipSync("播放开始（viseme 模式，${timeline.size} 帧）")
                lipSyncJob = lipSyncScope.launch { startVisemeLoop(view, timeline) }
                return
            }
        }
        // 级2：RMS 随机降级（现有行为保留）
        startRandomizedMouthLoop(view)
    }

    /** 级1：viseme 调度驱动（时间戳调度 + 帧间平滑） */
    private suspend fun startVisemeLoop(view: Live2DView, timeline: List<VisemeFrame>) {
        var last: VisemeFrame? = null
        val startMs = nowMillis()
        try {
            for (frame in timeline) {
                val elapsed = nowMillis() - startMs
                if (frame.timestampMs > elapsed) {
                    delay(frame.timestampMs - elapsed)
                }
                applyVisemeFrame(view, frame, last)
                last = frame
            }
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            logLipSync("viseme 动画异常，嘴型归零: ${e.message}")
        } finally {
            resetMouth(view)
        }
    }

    /** 应用一帧 viseme 五元音参数 */
    private fun applyVisemeFrame(view: Live2DView, frame: VisemeFrame, last: VisemeFrame?) {
        try {
            view.setParameter("ParamA", frame.paramA)
            view.setParameter("ParamI", frame.paramI)
            view.setParameter("ParamU", frame.paramU)
            view.setParameter("ParamE", frame.paramE)
            view.setParameter("ParamO", frame.paramO)
        } catch (_: Throwable) {
            // 单帧失败静默跳过，不影响后续帧
        }
    }

    /** viseme 模式结束/异常 → 五元音归零 + setLipSyncParam(0f) */
    private fun resetMouth(view: Live2DView) {
        try {
            view.setParameter("ParamA", 0f)
            view.setParameter("ParamI", 0f)
            view.setParameter("ParamU", 0f)
            view.setParameter("ParamE", 0f)
            view.setParameter("ParamO", 0f)
            view.setLipSyncParam(0f)
        } catch (_: Throwable) {}
    }

    /** 级2：RMS 随机波动嘴型（现有行为原样保留） */
    private fun startRandomizedMouthLoop(view: Live2DView) {
        logLipSync("播放开始（RMS 随机模式）")
        lipSyncJob = lipSyncScope.launch {
            var frame = 0L
            var currentMouth = 0f
            try {
                while (isActive) {
                    val target = LipSyncMath.randomizedMouth(frame, frame == 0L)
                    currentMouth = LipSyncMath.smoothMouth(currentMouth, target)
                    view.setLipSyncParam(currentMouth)
                    logLipSync(
                        "嘴型更新 frame=$frame mouth=${formatMouth(currentMouth)} " +
                            "target=${formatMouth(target)}"
                    )
                    frame++
                    delay(LIP_SYNC_INTERVAL_MS)
                }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                logLipSync("动画异常，嘴型归零: ${e.message}")
            } finally {
                try {
                    view.setLipSyncParam(0f)
                } catch (_: Throwable) {}
            }
        }
    }

    /** 停止 LipSync 协程并立即将嘴型归零（含五元音）。 */
    private fun stopLipSync() {
        logLipSync("播放结束")
        lipSyncJob?.cancel()
        lipSyncJob = null
        val view = live2DView ?: return
        try {
            view.setLipSyncParam(0f)
            // M5: 五元音归零
            resetMouth(view)
        } catch (_: Throwable) {
        }
    }

    private fun formatMouth(value: Float): String = "%.2f".format(value)

    /**
     * ASR 开始监听，返回识别文本（V1.2 模块4：遍历 ASR 降级链）。
     *
     * 降级链（plan §4.4.4）：sherpa_sensevoice → system_speech_recognizer → 静默 ""。
     * 遍历 [asrChain]：成功（非空文本）即返回；空结果/异常/超时 → 尝试下一个；
     * 全失败返回 ""（UI 无变化，降级为文本输入）。
     *
     * @param providerId 指定首选 ASR Provider ID（如 "sherpa_sensevoice"）；指定 Provider
     *                   失败/为空后仍按 priority 遍历其余 Provider 降级；null = 纯降级链遍历。
     *                   由引擎选择 UI（模块5）传入 SettingsRepository.asrProviderId。
     * @return 识别文本；失败/超时/无匹配返回 ""
     */
    suspend fun listen(providerId: String? = null): String {
        logD("listen() 开始语音识别（降级链: ${asrChain.map { it.info.id }}）")
        if (asrChain.isEmpty()) return ""
        val chain = resolveAsrChain(providerId)
        for (provider in chain) {
            val startedAt = System.currentTimeMillis()
            // 模块7 遥测：ASR 开始监听
            VoiceTelemetry.log(VoiceEvent.AsrStarted(provider.info.id, startedAt))
            try {
                val result = withTimeoutOrNull(LISTEN_TIMEOUT_MS) { provider.listen() }
                    ?: run {
                        logW("listen() ${provider.info.id} 超时，降级到下一个")
                        provider.stopListening()
                        // 模块7 遥测：超时按 ASR 失败处理（区别于空结果）
                        VoiceTelemetry.log(
                            VoiceEvent.AsrFailed(
                                provider.info.id,
                                "listen 超时（${LISTEN_TIMEOUT_MS}ms）",
                                System.currentTimeMillis(),
                            )
                        )
                        ""
                    }
                val durationMs = System.currentTimeMillis() - startedAt
                if (result.isNotEmpty()) {
                    // 模块7 遥测：识别成功（非空文本）
                    VoiceTelemetry.log(
                        VoiceEvent.AsrResult(provider.info.id, result, durationMs, System.currentTimeMillis())
                    )
                    logI("listen() 由 ${provider.info.id} 识别: '${result.take(40)}'")
                    return result
                }
                // 模块7 遥测：完成但无匹配（text=null，降级链继续）
                VoiceTelemetry.log(
                    VoiceEvent.AsrResult(provider.info.id, null, durationMs, System.currentTimeMillis())
                )
                logI("listen() ${provider.info.id} 返回空结果，降级到下一个")
            } catch (e: Exception) {
                // 模块7 遥测：ASR 失败（系统服务不可用等）
                VoiceTelemetry.log(
                    VoiceEvent.AsrFailed(provider.info.id, e.message ?: "unknown", System.currentTimeMillis())
                )
                // ASR 失败（系统服务不可用等）→ 静默降级到下一个 Provider
                logW("listen() ${provider.info.id} 识别失败，降级到下一个: ${e.message}")
            }
        }
        logW("listen() 所有 ASR provider 均失败/为空，静默返回空串")
        return ""
    }

    /**
     * 构造 ASR 识别链：指定 providerId 时该 Provider 优先，其后按降级链顺序补其余 Provider；
     * 未指定时直接用降级链（[asrChain] 已按 priority 升序）。与 TTS [resolveChain] 同构。
     */
    private fun resolveAsrChain(providerId: String?): List<AsrProvider> {
        if (providerId == null) return asrChain
        return asrChain.filter { it.info.id == providerId } +
            asrChain.filter { it.info.id != providerId }
    }

    /**
     * 释放 TTS + ASR 资源 + LipSync 动画（Activity 销毁时调用；幂等）。
     */
    fun release() {
        logD("release() 释放语音资源")
        // 停止播放状态观察（组合流收集）与嘴型动画，解绑视图（避免悬挂引用）
        playbackObserverJob?.cancel()
        playbackObserverJob = null
        lipSyncJob?.cancel()
        lipSyncJob = null
        visemeCollectJob?.cancel()
        visemeCollectJob = null
        keywordMotionJob?.cancel()
        keywordMotionJob = null
        live2DView = null
        // 释放所有 Provider（幂等）
        providers.forEach { provider ->
            try {
                provider.release()
            } catch (e: Exception) {
                logW("release() ${provider.info.id} 异常: ${e.message}")
            }
        }
        try {
            asrChain.forEach { it.stopListening() }
        } catch (e: Exception) {
            logW("release() ASR 异常: ${e.message}")
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

    /** 关键路径日志：Log.i 级别（项目教训：国产 ROM 抑制 Log.d，降级链决策日志必须可见）。 */
    private fun logI(message: String) {
        try {
            Log.i(TAG, message)
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

    /**
     * 时钟源（P0-3 降级防护）：真机优先 uptimeMillis（不受 suspend/resume 影响），
     * JVM 单元测试中 SystemClock 未 mock 会抛异常 → 降级 System.currentTimeMillis()。
     * 真机行为不变；测试环境自动降级。与 logD/logW 同一 try/catch 防护模式。
     */
    private fun nowMillis(): Long {
        return try {
            SystemClock.uptimeMillis()
        } catch (_: Throwable) {
            System.currentTimeMillis()
        }
    }
}
