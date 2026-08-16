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
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/**
 * V1.1 模块1「语音闭环接线」统一协调器（TTS 说）+ 模块4「LipSync」（TTS 播放驱动嘴型）
 * + V1.2 模块1「TtsProvider 注册表 + 降级链」（多引擎架构地基）。
 *
 * 职责（契约见 docs/plans/plan-v12-plan.schema.json → functions.VoiceIoController.*）：
 * - [speak]：播报纯文本（已剥离 [emotion] 标签）。遍历 [TtsProviderRegistry.cascade()]
 *   降级链（K1：sherpa 单引擎），按序尝试 [TtsProvider.speak]，成功即停；
 *   失败继续下一个；全失败静默降级（不影响对话主链路）。可指定 [providerId] 首选引擎。
 * - [stopSpeaking]：新消息发送前打断当前 TTS 播报（遍历降级链调用 stop()，
 *   确保切 Provider 后旧引擎资源释放）。
 * - [isSpeaking]：组合各 Provider 播放状态的 StateFlow（任一在播 = true）。
 * - [release]：Activity 销毁时释放所有 Provider 资源（幂等）。
 * - [attachLive2DView]：模块4 接线点，Live2D 视图创建后注入，驱动嘴型动画。
 *
 * 模块4（LipSync，plan §4.1.4 方案A 播放状态驱动）：
 * V1.2 起 LipSync 由各 Provider 的 [TtsProvider.isSpeaking] StateFlow 驱动（替代 V1.1 的
 * EdgeTtsService.onPlaybackStateChanged 回调）：构造时收集组合流，任一 Provider
 * 播放开始 → 启动随机波动嘴型协程（级2-B：每 100ms 更新一次 mouth，随机波动 + 停顿模拟 + lerp 平滑）；
 * 播放结束/打断 → 取消协程并将嘴型归零。不绑定具体 TTS 引擎，只依赖“是否在播放”状态。
 *
 * 容错约束（plan §8.2/§8.3）：TTS 失败不影响文本输入；
 * LipSync 异常 → catch 后 mouth 设为 0.0，不影响 TTS 播放。
 * 本类所有方法对外不抛异常（协程取消 [CancellationException] 除外——正常取消语义）。
 */
class VoiceIoController(
    context: Context,
    /** V1.2：TTS Provider 降级链（默认取注册表，测试可注入 mock）。 */
    private val providers: List<TtsProvider> = TtsProviderRegistry.cascade(),
    /** 模块4（LipSync）：驱动嘴型的 Live2D 视图；由 [attachLive2DView] 在视图创建后接线。 */
    private var live2DView: Live2DView? = null,
    /** LipSync 动画协程作用域（M-AND-3：Default 后台线程——setLipSyncParam 已改为通道 submit，
     *  无需 Main 且不触碰 native；测试注入 TestDispatcher 控制虚拟时间）。 */
    private val lipSyncScope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Default),
    /** M5：元音分析器；null = 禁用 viseme 驱动（走现有 RMS 随机降级），测试可注入 */
    private val visemeAnalyzer: VisemeAnalyzer? = null,
) {

    companion object {
        private const val TAG = "VoiceIo"

        /** LipSync 取证日志 TAG（项目教训：Log.i 级别，国产 ROM 抑制 Log.d） */
        private const val LIP_SYNC_TAG = "LipSync"

        /** LipSync 嘴型更新周期（plan §4.1.4：每 100-150ms） */
        internal const val LIP_SYNC_INTERVAL_MS = 100L

        /**
         * M-AND-1 打回点修复（C1）：消费侧帧表容量上限（150ms 块数级）。
         * 覆盖最长句子音频（1024 块 ≈ 2.5 分钟 @150ms）；溢出丢最旧帧，不阻塞播放。
         */
        internal const val FRAME_TABLE_CAPACITY = 1024

        // ── TTS 播报净化正则（来源：EmotionController.kt 静态成员，Wave 5 迁入本类）──
        // [emotion] 标签形状：`[keyword]` / `[keyword_强度]`（与 EmotionText.TAG_REGEX 同源）
        private val EMOTION_TAG_REGEX = Regex("""\[(\w+?)(?:_(\d+))?\]""")
        // 颜文字括号段：括号内仅含符号（无中文/字母/数字）→ 剥离，如 (=^･ω･^=) (≧▽≦)
        // 括号内含中文（如（小声说））不匹配 → 保留（保守策略：不清正常中文括号）
        private val KAOMOJI_PAREN_REGEX = Regex("""[（(][^（）()\u4e00-\u9fa5A-Za-z0-9]+[）)]""")
        // 无括号简单颜文字：^_^ / >_< / =ω= 等三字符符号组合，前后不得紧跟字母/数字（避免误删）
        private val SIMPLE_KAOMOJI_REGEX =
            Regex("""(?<![A-Za-z0-9])[\^=<>;~][_·･ω∀≧≦][\^=<>;~](?![A-Za-z0-9])""")
        // markdown 加粗/斜体符号：** 或 __ （2+ 连续）→ 剥离符号、保留文字；单个 * / _ 不动
        private val MARKDOWN_SYMBOL_REGEX = Regex("""[*_]{2,}""")
    }

    /** 当前 LipSync 动画协程（同一时刻至多一个；新播放开始即取消旧协程重启） */
    private var lipSyncJob: Job? = null

    /** viseme 分析收集协程（CPU 密集 → Dispatchers.Default；口型应用仍在 lipSyncScope/主线程） */
    private var visemeCollectJob: Job? = null

    /** M7 关键词兜底调度协程（contract-M7-android-l3.md §4；同 lipSyncScope 生命周期） */
    private var keywordMotionJob: Job? = null

    /** 播放状态观察协程（组合各 Provider isSpeaking → 更新 [isSpeaking] + 驱动 LipSync） */
    private var playbackObserverJob: Job? = null

    /**
     * I2 speak 互斥（单走廊）：speak 串行执行——同一时刻至多一个 speak 链在途。
     * 前链退出（完成/失败/打断抛 CancellationException → withLock finally 释放）后
     * 新链才进入；打断后 isSpeaking 收敛 false（provider.stop() 同步置位）再起新链，
     * 单走廊不重叠（双播报免疫，契约 §2.1 I2）。
     */
    private val speakMutex = Mutex()

    private val _isSpeaking = MutableStateFlow(false)

    /** P1: 最近一次成功播报的 Provider id（供 UI/遥测显示）。 */
    private val _activeProviderId = MutableStateFlow<String?>(null)
    val activeProviderId: StateFlow<String?> = _activeProviderId.asStateFlow()

    /**
     * M-AND-1（3-B）：播放起始 wall-clock（重锚基准，AND1-A6）。
     * isSpeaking 前沿（真实播放开始）时记录；播放结束/打断复位 0。
     * 消费侧重锚：嘴型帧调度墙钟时刻 = [LipSyncClock.reanchor](frame.timestampMs, 本值)。
     */
    @Volatile
    private var playStartWallClockMs: Long = 0L

    /**
     * M-AND-2（1-B）：RMS 帧槽（最新一帧 PCM；collector 线程写、RMS 嘴型循环读）。
     * 帧流为空/未收到帧 → null → 嘴型静默（不张嘴，无随机 fallback）。
     * 打回点修复（C1）后为兜底：帧表按位置选帧无匹配时回退本槽（保持既有行为面）。
     */
    @Volatile
    private var latestAudio: AudioFrame? = null

    /**
     * M-AND-1 打回点修复（C1）：消费侧帧表缓冲（按重锚时钟选帧）。
     *
     * - collector 按 timestampMs 升序追加（契约 AND1-A2 严格非递减）；
     * - 嘴型循环每节拍按当前播放位置（重锚时钟 now − playStartWallClockMs）选
     *   max timestampMs ≤ positionMs 的帧 → 该帧 PCM 算 RMS → 嘴型；
     * - 容量上限 [FRAME_TABLE_CAPACITY]（150ms 块数级，覆盖最长句子音频）；
     *   溢出丢最旧帧 + Log.w 一次，不阻塞播放（audit R1 建议方向）。
     */
    private val frameTableLock = Any()

    private val frameTable = ArrayList<AudioFrame>(FRAME_TABLE_CAPACITY)

    /** 帧表溢出警告（本次 speak 会话内至多一次）。 */
    @Volatile
    private var frameTableOverflowWarned = false

    /** M-AND-2（1-B）：RMS 帧收集协程（speak 前启动；播放结束/失败时取消）。 */
    private var rmsFrameCollectJob: Job? = null

    /** M-AND-2（1-B）：当前成功播报的 Provider（供帧收集兜底重启）。 */
    private var activeProvider: TtsProvider? = null

    /**
     * 组合播放状态流（契约 schema → functions.VoiceIoController.isSpeaking）：
     * 任一已注册 Provider 处于播放状态时 = true，全部空闲 = false。
     */
    val isSpeaking: StateFlow<Boolean> = _isSpeaking.asStateFlow()

    init {
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
     * [sanitizeForTts]（EmotionController.sanitizeForTts 等价实现，Wave 5 迁入本类）
     * 剥离 [emotion] 标签 + 颜文字括号段（(=^･ω･^=) 会被 TTS 读成"等于号"）+ markdown 符号，
     * 之后所有逻辑用净化后的文本。
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
        val clean = sanitizeForTts(text)
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
        // I2 speak 互斥（单走廊）：串行执行——前一个 speak 链退出（完成/失败/打断抛
        // CancellationException → withLock finally 释放）后才允许新链进入；打断后
        // isSpeaking 收敛 false（provider.stop() 同步置位）再起新链，单走廊不重叠（双播报免疫）。
        speakMutex.withLock {
            val succeeded = speakThroughChain(chain, clean, keywordMotionEnabled)
            if (!succeeded) {
                logW("speak() 所有 provider 均失败，静默降级")
            }
        }
    }

    /**
     * I2 speak 互斥内部实现（仅 [speak] 在 [speakMutex] 持锁下调用，单走廊）：
     * 按降级链顺序尝试 [TtsProvider.speak]，成功即停返回 true；全失败返回 false。
     */
    private suspend fun speakThroughChain(
        chain: List<TtsProvider>,
        clean: String,
        keywordMotionEnabled: Boolean,
    ): Boolean {
        for ((index, provider) in chain.withIndex()) {
            var spokeSuccessfully = false
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
                // M-AND-2（1-B）：speak 前启动 RMS 帧收集（AND1-A7 时序契约：先 collect 后 speak；
                // 预解码 provider 播放前全量 emit，晚 collect 即丢帧；Default 与主线程并发订阅）
                startRmsFrameCollection(provider)
                provider.speak(clean)
                spokeSuccessfully = true
                _activeProviderId.value = provider.info.id
                logI("speak() 由 ${provider.info.id} 播报完成")
                return true
            } catch (e: CancellationException) {
                // 协程正常取消（stop() 打断 / 上层取消）——向上传播，不再尝试降级
                throw e
            } catch (e: Exception) {
                val reason = e.message ?: "unknown"
                // 降级链遍历日志（国产 ROM 抑制 Log.d 教训：用 Log.i 级别）
                logI("speak() ${provider.info.id} 失败，降级到下一个: $reason")
            } finally {
                visemeCollectJob?.cancel()
                visemeCollectJob = null
                keywordMotionJob?.cancel()
                keywordMotionJob = null
                if (!spokeSuccessfully) {
                    // 失败/取消路径：清 RMS 帧收集，避免残留帧污染下一个 provider 的嘴型
                    stopRmsFrameCollection()
                }
            }
        }
        return false
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
        // 打断幂等（2B-2 RED 锚）：同步停嘴型链——不等 isSpeaking 流异步收敛。
        // stop() 后任何迟到帧（provider 违约仍 emit）都不得写入嘴型参数：
        // 取消 RMS 循环协程 + 清帧表/帧槽 + 重锚基准复位 + 嘴型归零。
        stopLipSync()
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
     * - true=开始播放 → 记录播放起始墙钟（M-AND-1 重锚基准）→ 启动嘴型动画协程
     * - false=播放结束/中断 → 复位重锚基准 → 取消协程，嘴型归零
     *
     * internal 便于单元测试直接驱动（真实路径由 [init] 中的组合流收集触发）。
     */
    internal fun handlePlaybackState(isPlaying: Boolean) {
        if (isPlaying) {
            // M-AND-1（AND1-A6）：isSpeaking 前沿 = 播放起始墙钟（重锚基准，禁止以 nowMillis 起算）
            playStartWallClockMs = nowMillis()
            startLipSync()
        } else {
            playStartWallClockMs = 0L
            stopLipSync()
        }
    }

    /** M-AND-1（3-B）：当前播放起始墙钟（重锚基准）；未播放 = 0。供消费侧重锚调度读取。 */
    internal fun playbackStartWallClockMs(): Long = playStartWallClockMs

    /** 启动 LipSync 动画协程（幂等：先取消旧协程再重启）。M5：优先 viseme 驱动，降级走 RMS 幅度驱动（M-AND-2 选型 1-B）。 */
    private fun startLipSync() {
        lipSyncJob?.cancel()
        lipSyncJob = null
        val view = live2DView
        if (view == null) {
            logLipSync("播放开始（Live2DView 未接线，跳过嘴型动画）")
            return
        }

        // M-AND-2（1-B）兜底：帧收集未启动（如测试直接驱动 handlePlaybackState，或打断后
        // 再次播放）→ 从当前 activeProvider（无则降级链首个）补启，保证"再次 speak 后恢复正常"。
        if (rmsFrameCollectJob == null) {
            (activeProvider ?: providers.firstOrNull())?.let { startRmsFrameCollection(it) }
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
        // 级2（M-AND-2 选型 1-B）：RMS 幅度驱动（替换伪随机；帧流为空 → 静默不张嘴）
        startRmsMouthLoop(view)
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

    /**
     * 级2（M-AND-2 选型 1-B）：RMS 幅度驱动嘴型循环（真实时钟节拍，非帧号驱动）。
     *
     * - 数据源 = 帧表缓冲（speak 前启动的 audioFrameFlow 收集，AND1-A7 时序）；每节拍按
     *   重锚时钟当前播放位置选帧（打回点修复 C1：Edge/Cosy/sherpa 预发射全量帧不再恒取末帧——
     *   按 positionMs 选 max timestampMs ≤ positionMs 的帧）；帧表为空/无匹配 → 静默不张嘴
     *   （兜底 [latestAudio] 最新帧，保持既有行为面）；
     * - 每节拍 rmsNormalized → applyEnvelope（attack/release 包络 + 静音阈值门控）→
     *   步进钳制 ≤ 0.2（AND2-A7 帧间平滑）→ clamp [0,1]（AND2-A5）；
     * - 节拍由 delay 真实时钟调度 + nowMillis 测量（AND2-A6），不引用任何随机源（AND2-A2 确定性）。
     */
    private fun startRmsMouthLoop(view: Live2DView) {
        logLipSync("播放开始（RMS 幅度驱动模式）")
        lipSyncJob = lipSyncScope.launch {
            var currentMouth = 0f
            try {
                while (isActive) {
                    // 真实时钟节拍（AND2-A6：nowMillis 测量 + delay 调度，非帧号计数）
                    val tickMs = nowMillis()
                    // M-AND-1 打回点修复（C1）：按重锚时钟选帧——当前播放位置
                    // = now − playStartWallClockMs（AND1-A6 重锚基准，禁止以 nowMillis 起算）；
                    // 选 max timestampMs ≤ positionMs 的帧（帧表缓冲），无匹配兜底最新帧。
                    val positionMs = playbackPositionMs()
                    val frame = selectFrameForPosition(positionMs) ?: latestAudio
                    val rms = if (frame != null) LipSyncMath.rmsNormalized(frame.samples) else 0f
                    val target = LipSyncMath.applyEnvelope(rms, currentMouth, LIP_SYNC_INTERVAL_MS)
                    // AND2-A7 帧间平滑：相邻输出差 ≤ 0.2；AND2-A5 输出恒 ∈ [0,1]
                    val next = (currentMouth + (target - currentMouth)
                        .coerceIn(-LipSyncMath.MAX_FRAME_DELTA, LipSyncMath.MAX_FRAME_DELTA))
                        .coerceIn(0f, 1f)
                    currentMouth = next
                    view.setLipSyncParam(next)
                    logLipSync(
                        "嘴型更新 t=$tickMs pos=$positionMs frameTs=${frame?.timestampMs ?: -1} " +
                            "rms=${formatMouth(rms)} mouth=${formatMouth(next)}"
                    )
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

    /**
     * M-AND-2（1-B）：启动 RMS 帧收集（speak 前调用，AND1-A7「先 collect 后 speak」时序契约）。
     * Dispatchers.Default：与主线程 speak() 并发订阅，预解码 provider 播放前全量 emit 不丢帧。
     * 每次重启先清旧槽（新会话不残留上一 provider 帧）。
     */
    private fun startRmsFrameCollection(provider: TtsProvider) {
        rmsFrameCollectJob?.cancel()
        rmsFrameCollectJob = null
        latestAudio = null
        frameTableOverflowWarned = false
        synchronized(frameTableLock) { frameTable.clear() }
        activeProvider = provider
        rmsFrameCollectJob = lipSyncScope.launch(Dispatchers.Default) {
            try {
                provider.audioFrameFlow.collect { frame ->
                    latestAudio = frame
                    // 帧表缓冲（打回点修复 C1）：升序追加；溢出丢最旧 + Log.w 一次，不阻塞播放
                    synchronized(frameTableLock) {
                        if (frameTable.size >= FRAME_TABLE_CAPACITY) {
                            frameTable.removeAt(0)
                            if (!frameTableOverflowWarned) {
                                frameTableOverflowWarned = true
                                logW(
                                    "帧表溢出（容量 $FRAME_TABLE_CAPACITY 块 @150ms），" +
                                        "丢弃最旧帧（不影响播放）"
                                )
                            }
                        }
                        frameTable.add(frame)
                    }
                }
            } catch (_: CancellationException) {
                // 正常取消
            } catch (e: Exception) {
                logW("RMS 帧收集异常: ${e.message}")
            }
        }
    }

    /** M-AND-2（1-B）：停止 RMS 帧收集并清空帧槽/帧表（播放结束/打断/失败）。 */
    private fun stopRmsFrameCollection() {
        rmsFrameCollectJob?.cancel()
        rmsFrameCollectJob = null
        latestAudio = null
        synchronized(frameTableLock) { frameTable.clear() }
        activeProvider = null
    }

    /**
     * M-AND-1 打回点修复（C1）：当前播放位置（重锚时钟，AND1-A6）。
     *
     * = now − playStartWallClockMs（重锚基准：isSpeaking 前沿记录，播放结束复位 0）。
     * 与帧 timestampMs 同基准（均相对播放起点）→ 选帧无需额外换算；未播放（基准未记录）→ 0。
     * 禁止以 nowMillis() 直接起算（契约 AND1-A6）。
     */
    private fun playbackPositionMs(): Long {
        val start = playStartWallClockMs
        return if (start > 0L) (nowMillis() - start).coerceAtLeast(0L) else 0L
    }

    /**
     * M-AND-1 打回点修复（C1）：按当前播放位置选帧（消费侧重锚）。
     *
     * 帧表按 timestampMs 升序（契约 AND1-A2 严格非递减）→ 线性扫描取
     * max timestampMs ≤ positionMs 的帧；无匹配（位置早于首帧 ts，仅契约违约输入
     * 可能出现）→ null（调用方兜底 [latestAudio]）。
     */
    private fun selectFrameForPosition(positionMs: Long): AudioFrame? {
        synchronized(frameTableLock) {
            var selected: AudioFrame? = null
            for (frame in frameTable) {
                if (frame.timestampMs <= positionMs) {
                    selected = frame
                } else {
                    break
                }
            }
            return selected
        }
    }

    /** 停止 LipSync 协程并立即将嘴型归零（含五元音）。 */
    private fun stopLipSync() {
        logLipSync("播放结束")
        lipSyncJob?.cancel()
        lipSyncJob = null
        // 重锚基准复位（停止/打断即失效；handlePlaybackState(false) 与 stopSpeaking 两路共用）
        playStartWallClockMs = 0L
        stopRmsFrameCollection()
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
     * 释放 TTS 资源 + LipSync 动画（Activity 销毁时调用；幂等）。
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
        rmsFrameCollectJob?.cancel()
        rmsFrameCollectJob = null
        latestAudio = null
        synchronized(frameTableLock) { frameTable.clear() }
        activeProvider = null
        live2DView = null
        // 释放所有 Provider（幂等）
        providers.forEach { provider ->
            try {
                provider.release()
            } catch (e: Exception) {
                logW("release() ${provider.info.id} 异常: ${e.message}")
            }
        }
    }

    /**
     * TTS 播报前统一净化（EmotionController.sanitizeForTts 等价实现，Wave 5 迁入本类；
     * 原实现位于 EmotionController.kt，该文件已随 Wave 5 删除）。
     *
     * 在剥离 [emotion] 标签基础上，进一步剥离会被 TTS 朗读的符号类内容：
     * 颜文字括号段（如 (=^･ω･^=) 会被读成"等于号"）、无括号简单颜文字、markdown 加粗/斜体符号。
     * 保守原则：只清"符号类"内容，不得误删正常中文括号（（小声说）等括号内含中文 → 保留）。
     * 仅影响 TTS 播报文本，不影响 UI 显示与字幕。
     */
    private fun sanitizeForTts(text: String): String {
        return text
            .replace(EMOTION_TAG_REGEX, "")          // [joy] 等 [emotion] 标签
            .replace(KAOMOJI_PAREN_REGEX, "")       // 颜文字括号段 (=^･ω･^=)
            .replace(SIMPLE_KAOMOJI_REGEX, "")      // 无括号简单颜文字 ^_^ / >_<
            .replace(MARKDOWN_SYMBOL_REGEX, "")     // **xx** 符号剥离，保留文字
            .trim()
    }

    // 日志输出带单元测试防护（本地 JVM 测试无 Android 运行时，直接调用 Log 会抛
    // "Method d in android.util.Log not mocked"）。
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

/**
 * M-AND-1 播放时钟与重锚（选型 3-B）——播放时钟抽象 seam（contract-linkage-android.md §2）。
 *
 * - [reanchor]：重锚纯函数（AND1-A3/A6 统一语义，契约 v2.1）——返回调度墙钟时刻 =
 *   playStartWallClockMs + frameTsMs，clamp 下限 = playStartWallClockMs（不早于播放起点）。
 *   纯函数：无随机项、不读系统时钟（AND1-A6「注入固定时钟源，调度序列 = 重锚序列」）。
 * - [PlaybackClock]：按固定周期 [PLAYBACK_POLL_INTERVAL_MS] 轮询 [PlayerPositionSource]
 *   （真实实现 = MediaPlayer.getCurrentPosition）；onPrepared→onStart 恒 0、onStart 后非递减。
 * - [TestPlayerClock]：测试假体（fakePrepared / fakeStart / fakeAdvance 驱动，无需真实 MediaPlayer）。
 */
object LipSyncClock {

    /** 3-B：播放位置轮询周期（固定常数，∈ [50,100]ms；非抖动值，契约 AND1-A5 回归门）。 */
    const val PLAYBACK_POLL_INTERVAL_MS = 60L

    /**
     * 重锚纯函数（AND1-A3/A6，契约 v2.1 统一语义）：
     * 帧调度墙钟时刻 = playStartWallClockMs + frameTsMs，clamp 下限 = playStartWallClockMs
     * （帧时间戳恒 ≥ 0 时等价于加法；frameTsMs < 0 时防御性钳制到播放起点）。
     */
    fun reanchor(frameTsMs: Long, playStartWallClockMs: Long): Long =
        (playStartWallClockMs + frameTsMs).coerceAtLeast(playStartWallClockMs)

    /** 调度序列（AND1-A6）：帧列表 → 重锚调度墙钟序列（保序、保间距、严格递增）。 */
    fun scheduleTimes(frames: List<AudioFrame>, playStartWallClockMs: Long): List<Long> =
        frames.map { reanchor(it.timestampMs, playStartWallClockMs) }

    /** 播放位置源抽象：真实实现读 MediaPlayer.getCurrentPosition；测试用 [TestPlayerClock]。 */
    fun interface PlayerPositionSource {
        fun currentPositionMs(): Long
    }

    /**
     * 3-B 播放时钟：按 [PLAYBACK_POLL_INTERVAL_MS] 轮询 [PlayerPositionSource]。
     *
     * - [onPrepared]：位置恒 0（不启动轮询；prepared→start 间不抛异常）
     * - [onStart]：位置从 0 起，启动轮询；单调钳制保证非递减（AND1-A5）
     * - [onStop]：停止轮询并复位 0（播放完成/打断/释放）
     */
    class PlaybackClock(
        private val positionSource: PlayerPositionSource,
        private val scope: CoroutineScope,
    ) {
        private val _playbackPositionMs = MutableStateFlow(0L)
        val playbackPositionMs: StateFlow<Long> = _playbackPositionMs.asStateFlow()

        private var pollJob: Job? = null

        /** MediaPlayer.onPrepared：位置恒 0（停止轮询，等待 start）。 */
        fun onPrepared() {
            pollJob?.cancel()
            pollJob = null
            _playbackPositionMs.value = 0L
        }

        /** MediaPlayer.start() 后调用：开始轮询，位置非递减（单调钳制）。 */
        fun onStart() {
            _playbackPositionMs.value = 0L
            pollJob?.cancel()
            pollJob = scope.launch {
                while (isActive) {
                    val pos = try {
                        positionSource.currentPositionMs()
                    } catch (_: Throwable) {
                        0L
                    }
                    if (pos > _playbackPositionMs.value) {
                        _playbackPositionMs.value = pos
                    }
                    delay(PLAYBACK_POLL_INTERVAL_MS)
                }
            }
        }

        /** 播放完成 / 打断 / 释放：停止轮询并复位 0。 */
        fun onStop() {
            pollJob?.cancel()
            pollJob = null
            _playbackPositionMs.value = 0L
        }
    }

    /**
     * 测试假体：可控播放位置源（AND1-A5 seam；fakePrepared/fakeStart/fakeAdvance 驱动）。
     * 同时实现 [PlayerPositionSource]，可注入 [PlaybackClock] 验证轮询行为；
     * [playbackPositionMs] 镜像内部位置（prepared→start 恒 0、start 后非递减）。
     */
    class TestPlayerClock : PlayerPositionSource {
        private var positionMs = 0L
        private var playing = false

        private val _playbackPositionMs = MutableStateFlow(0L)

        /** 播放位置镜像（契约锚 playbackPositionMs：prepared→start 恒 0，start 后非递减）。 */
        val playbackPositionMs: StateFlow<Long> = _playbackPositionMs.asStateFlow()

        /** 模拟 MediaPlayer.onPrepared：位置归零、未播放（prepared→start 恒 0）。 */
        fun fakePrepared() {
            playing = false
            positionMs = 0L
            _playbackPositionMs.value = 0L
        }

        /** 模拟 MediaPlayer.start()：开始播放，位置从 0 起。 */
        fun fakeStart() {
            playing = true
            positionMs = 0L
            _playbackPositionMs.value = 0L
        }

        /** 推进播放位置（仅播放中有效；非递减）。 */
        fun fakeAdvance(ms: Long) {
            if (playing) {
                positionMs += ms
                if (positionMs > _playbackPositionMs.value) {
                    _playbackPositionMs.value = positionMs
                }
            }
        }

        /** 推进一个轮询周期。 */
        fun fakeAdvance() {
            fakeAdvance(PLAYBACK_POLL_INTERVAL_MS)
        }

        override fun currentPositionMs(): Long = positionMs
    }
}

/**
 * M-AND-1 重锚纯函数（top-level 便捷入口，同 [LipSyncClock.reanchor]）：
 * 帧调度墙钟时刻 = playStartWallClockMs + frameTsMs，clamp 下限 = playStartWallClockMs。
 */
internal fun reanchor(frameTsMs: Long, playStartWallClockMs: Long): Long =
    LipSyncClock.reanchor(frameTsMs, playStartWallClockMs)
