package com.live2dai.android.core

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.emptyFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.merge
import kotlinx.coroutines.launch

/**
 * Wave 4 · 4-1 核心环路装配器（三片真实绑定）。
 *
 * 把 C 片状态机/协调器与 A 片（表情/视图宿主）、B 片（TTS 播报）接成可运行的
 * TTS+AI+Live2D 核心环路（契约 refactor-0814 §2.1-§2.3 落点）：
 *
 * - [assemble]：依赖装配（[LoopCoordinator.withDependencies]）+ 视图绑定
 *   （[LoopCoordinator.attachView]，I6 替换语义——新引用覆盖旧引用，绝不追加）；
 * - [mapLlmEvent]：[LlmEvent] → [LoopEvent] 纯映射（LLM 事件泵的映射层）；
 * - [pump]：LLM 事件泵（I3 单消费者：请求冷流至多一条收集链）；
 * - [pumpSpeakFinished]：SpeakFinished/SpeakFailed 泵——isSpeaking 下降沿
 *   （true→false 且曾 speaking）→ `SpeakFinished`；speak 失败信号 → `SpeakFailed`；
 * - 表情收敛调度：coordinator 内置（SpeakFinished 出边 ExprRelease 延迟 N=5s 执行；
 *   SpeakFailed 出边 ExprRelease 立即执行——状态机出边 + [LoopCoordinator] 默认延迟，
 *   装配注入 [TimedExpressionSink] 提供查询时判定收敛，isReleased）。
 */
object LoopWire {

    /**
     * 装配（规格形态）：withDependencies（表情收敛延迟默认 5s）+ attachView(host)。
     * 单例使用：MainActivity 在 onCreate 组装一次；attachView 为替换语义（I6）。
     */
    fun assemble(
        llmLink: LlmLink,
        expressionSink: ExpressionSink,
        speechSink: SpeechSink,
        host: LoopViewHost,
    ): LoopCoordinator = LoopCoordinator.withDependencies(
        llmLink = llmLink,
        expressionSink = expressionSink,
        speechSink = speechSink,
    ).also { it.attachView(host) }

    /**
     * 装配（扩展形态）：额外注入 [stateMachine]（onState 状态观察，UI isLoading 驱动源）、
     * [scope]（协程作用域）与 [expressionReleaseDelaySeconds]（表情收敛延迟，契约 C-2 默认 5）。
     */
    fun assemble(
        llmLink: LlmLink,
        expressionSink: ExpressionSink,
        speechSink: SpeechSink,
        host: LoopViewHost,
        stateMachine: LoopStateMachine = LoopStateMachine(),
        scope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined),
        expressionReleaseDelaySeconds: Long = 5,
    ): LoopCoordinator = LoopCoordinator.withDependencies(
        llmLink = llmLink,
        expressionSink = expressionSink,
        speechSink = speechSink,
        stateMachine = stateMachine,
        scope = scope,
        expressionReleaseDelaySeconds = expressionReleaseDelaySeconds,
    ).also { it.attachView(host) }

    /** [LlmEvent] → [LoopEvent] 纯映射（LLM 事件泵映射层；与 LoopCoordinator 内部映射同语义）。 */
    fun mapLlmEvent(e: LlmEvent): LoopEvent = when (e) {
        is LlmEvent.Delta -> LoopEvent.LlmDelta(e.fragment)
        is LlmEvent.Completed -> LoopEvent.LlmCompleted(e.fullText, e.emotion)
        is LlmEvent.Failed -> LoopEvent.LlmFailed(e.message, e.quotaExhausted)
    }

    /**
     * LLM 事件泵（装配形态，I3 单消费者）：collect `llmLink.send(text)` 冷流 →
     * [mapLlmEvent] 映射 → [LoopCoordinator.onEvent] 转发。
     *
     * 生产主链说明：请求经 [LoopCoordinator.sendMessage] 进入（SubmitText 转移 → LlmSend 副作用），
     * coordinator 内部 `launchLlm` 已是该请求冷流的**唯一收集者**（I3 结构保证，无双播消费点）。
     * 本泵为该单消费者语义的显式接线形态——装配层启动常驻泵协程，与内部管线
     * 二选一使用（标准装配用内部管线，本形态不重复收集）。
     */
    fun pump(coordinator: LoopCoordinator, llmLink: LlmLink, scope: CoroutineScope): Job =
        scope.launch(start = CoroutineStart.UNDISPATCHED) {
            // I3 单消费者：请求冷流收集归 coordinator 内部 launchLlm
            //（sendMessage → LlmSend 副作用 → llmLink.send(text).collectLlmEvents()）。
            // 显式驱动形态（外部收集请求冷流）见 [pump]（Flow 形态）。
        }

    /**
     * LLM 事件泵（显式驱动形态，I3 单消费者）：收集一次请求冷流
     * （`llmLink.send(text)` 产物；冷流、单消费者——本协程是该流的唯一收集者）
     * → [mapLlmEvent] → [LoopCoordinator.onEvent]。
     */
    fun pump(coordinator: LoopCoordinator, request: Flow<LlmEvent>, scope: CoroutineScope): Job =
        scope.launch(start = CoroutineStart.UNDISPATCHED) {
            request.collect { coordinator.onEvent(mapLlmEvent(it)) }
        }

    /**
     * SpeakFinished/SpeakFailed 泵：collect `speechSink.isSpeaking` 下降沿
     * （true→false 且曾 speaking）→ `coordinator.onEvent(SpeakFinished)`；
     * speechSink 为 [SpeechSinkAdapter] 时其 [SpeechSinkAdapter.speakFailure] 信号
     * （speak 抛异常 / 全失败静默未出声）→ `coordinator.onEvent(SpeakFailed)`。
     */
    fun pumpSpeakFinished(
        coordinator: LoopCoordinator,
        speechSink: SpeechSink,
        scope: CoroutineScope,
    ): Job = pumpSpeakFinished(
        coordinator,
        speechSink,
        (speechSink as? SpeechSinkAdapter)?.speakFailure,
        scope,
    )

    /**
     * SpeakFinished/SpeakFailed 泵（显式失败流形态）：[speakFailure] 为 null → 仅 SpeakFinished 泵。
     * 供测试注入显式失败信号；生产经 [SpeechSinkAdapter.speakFailure] 接入。
     */
    fun pumpSpeakFinished(
        coordinator: LoopCoordinator,
        speechSink: SpeechSink,
        speakFailure: Flow<Unit>?,
        scope: CoroutineScope,
    ): Job = scope.launch(start = CoroutineStart.UNDISPATCHED) {
        var wasSpeaking = false
        val speakingSignals: Flow<SpeakPumpSignal> =
            speechSink.isSpeaking.map { SpeakPumpSignal.Speaking(it) }
        val failureSignals: Flow<SpeakPumpSignal> =
            speakFailure?.map { SpeakPumpSignal.Failed } ?: emptyFlow()
        merge(speakingSignals, failureSignals).collect { signal ->
            when (signal) {
                is SpeakPumpSignal.Speaking -> {
                    // 下降沿（true→false 且曾 speaking）→ 播报自然结束
                    if (wasSpeaking && !signal.value) {
                        coordinator.onEvent(LoopEvent.SpeakFinished)
                    }
                    wasSpeaking = signal.value
                }
                SpeakPumpSignal.Failed -> coordinator.onEvent(LoopEvent.SpeakFailed)
            }
        }
    }

    /** SpeakFinished 泵内部信号（isSpeaking 状态 / 失败事件）。 */
    private sealed interface SpeakPumpSignal {
        data class Speaking(val value: Boolean) : SpeakPumpSignal
        data object Failed : SpeakPumpSignal
    }
}
