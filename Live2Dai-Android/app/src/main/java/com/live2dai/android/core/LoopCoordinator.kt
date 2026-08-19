package com.live2dai.android.core

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.launch

/**
 * 核心环路唯一编排者（架构 C M1 LoopCore；契约 §2.2「视图接线」）。
 *
 * 职责：持有 [LoopStateMachine]，执行其副作用指令（LLM/TTS/表情三路接线 + 打断 + UI 回调），
 * 并提供双播报/冻结免疫的结构保证：
 *
 * - **I1 重入守卫**：[sendMessage] 在 SENDING（请求在途）时拒绝——至多一个 LLM 请求在途。
 * - **I6 接线幂等**：[attachView] 为**替换语义**（新引用覆盖旧引用，绝不追加；旧宿主解绑）——
 *   AndroidView factory 重跑 N 次 → 接线仍只有 1 份（双 hook 累积根因的反转）。
 * - **I7 fail-safe**：任一副作用指令执行抛异常 → 强制收敛回 IDLE，不崩溃。
 * - **I4/I5**：[LoopCommand.MouthZero]/[ExprRelease] 由状态机在每条 SPEAKING 出边发出，
 *   本类按序执行（SpeakFinished 的表情收敛按契约延迟 N 秒调度，N=裁定 C-2 默认 5）。
 *
 * 依赖注入形态（非全局单例；应用侧持有单例即可）。工厂见 [withDependencies]（伴生与 top-level 双入口）。
 * 默认 scope 用 Unconfined 使测试中非挂起 fake 同步执行（确定性）；生产侧应注入主线程 scope。
 */
class LoopCoordinator(
    // @Volatile：replaceLlm 由主线程更新、launchLlm（后台 scope）读取，
    // 跨线程可见性保证「下一次请求用新链接」（审计 A-5，与 config/state 一致）。
    @Volatile
    private var llmLink: LlmLink,
    private val expressionSink: ExpressionSink,
    private val speechSink: SpeechSink,
    private val stateMachine: LoopStateMachine = LoopStateMachine(),
    private val scope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined),
    private val expressionReleaseDelaySeconds: Long = 5,
) {

    /** 当前绑定的视图宿主（I6：恒为最后 attach 的一份；null = 未绑定/已解绑）。 */
    @Volatile
    var boundHost: LoopViewHost? = null
        private set

    /** UI 命令回调（M3 UI 壳接线点：用户气泡/流式增量/错误气泡）。null = 丢弃（2C-1 不实现 UI）。 */
    @Volatile
    var uiListener: ((LoopCommand) -> Unit)? = null

    /** 当前状态机状态（只读透传）。 */
    val state: LoopState get() = stateMachine.state

    /**
     * 视图接线（替换语义，I6）：新引用覆盖旧引用，绝不追加；旧宿主先解绑、新宿主再绑定。
     * 同一引用重复 attach 为 no-op（不重复 onAttach）。传 null 仅解绑。
     */
    fun attachView(newHost: LoopViewHost?) {
        val old = boundHost
        if (old === newHost) return
        old?.onDetach()
        boundHost = newHost
        newHost?.onAttach()
    }

    /**
     * 用户发送消息。重入守卫（I1）：SENDING（请求在途）时拒绝并返回 false，不触发第二个 llm.send。
     * SPEAKING 时合法（C-4 立即打断：stop → 嘴型归零 → 表情收敛 → 新请求）。
     */
    /**
     * 运行时替换 LLM 链接（设置页修改 Provider/base_url/api_key/model 后调用，
     * 下一次请求即用新链接，无需重启 Activity）。
     */
    fun replaceLlm(newLink: LlmLink) {
        llmLink = newLink
    }

    fun sendMessage(text: String): Boolean {
        // 空文本/纯空白：不进入 SENDING，避免状态机卡死（HttpLlmLink.send 对 blank 直接返回，
        // 状态机无超时回退 → 永久 SENDING）。修复 F2.6。
        if (text.isBlank()) return false
        if (stateMachine.state == LoopState.SENDING) return false
        val transition = stateMachine.transition(LoopEvent.SubmitText(text))
        // 兜底：状态机拒绝（态不变、零指令）时同样不执行
        if (transition.nextState == stateMachine.state && transition.commands.isEmpty()) return false
        execute(transition.commands)
        return true
    }

    /** 通用事件入口：事件 → 状态机转换 → 按序执行副作用指令（分发序 = 转移表命令顺序）。 */
    fun onEvent(event: LoopEvent) {
        val transition = stateMachine.transition(event)
        if (event is LoopEvent.SpeakFinished) {
            // I5：SpeakFinished 的表情收敛按契约「延迟 N 秒」调度（mouth.zero 立即执行）
            execute(transition.commands.filterNot { it is ExprRelease })
            scheduleExpressionRelease()
        } else {
            execute(transition.commands)
        }
    }

    /** [onEvent] 别名（历史命名兼容）。 */
    fun dispatch(event: LoopEvent) = onEvent(event)

    private fun execute(commands: List<LoopCommand>) {
        for (command in commands) {
            try {
                when (command) {
                    is LoopCommand.LlmSend -> launchLlm(command.text)
                    is LoopCommand.LlmReset -> llmLink.reset()
                    is LoopCommand.UiAddUserBubble -> uiListener?.invoke(command)
                    is UiAppendStreamBubble -> uiListener?.invoke(command)
                    is LoopCommand.UiShowError -> uiListener?.invoke(command)
                    is LoopCommand.SpeechStop -> speechSink.stop()
                    is LoopCommand.SpeechSpeak -> launchSpeak(command.text)
                    is LoopCommand.MouthZero -> boundHost?.zeroMouth()
                    is LoopCommand.ExprApply -> expressionSink.apply(command.emotion)
                    is ExprRelease -> expressionSink.release()
                }
            } catch (ce: CancellationException) {
                throw ce
            } catch (t: Throwable) {
                failSafe(t)
            }
        }
    }

    /** 启动 LLM 冷流收集：事件转发回状态机（单消费者——结构上无双播消费点，I3）。 */
    private fun launchLlm(text: String) {
        scope.launch(start = CoroutineStart.UNDISPATCHED) {
            try {
                llmLink.send(text).collectLlmEvents()
            } catch (ce: CancellationException) {
                throw ce
            } catch (t: Throwable) {
                failSafe(t)
            }
        }
    }

    private suspend fun Flow<LlmEvent>.collectLlmEvents() {
        collect { llmEvent ->
            when (llmEvent) {
                is LlmEvent.Delta -> onEvent(LoopEvent.LlmDelta(llmEvent.fragment))
                is LlmEvent.Completed -> onEvent(LoopEvent.LlmCompleted(llmEvent.fullText, llmEvent.emotion))
                is LlmEvent.Failed -> onEvent(LoopEvent.LlmFailed(llmEvent.message, llmEvent.quotaExhausted))
            }
        }
    }

    /** 执行 speak（suspend）；UNDISPATCHED 使非挂起 fake 同步完成（测试确定性）。 */
    private fun launchSpeak(text: String) {
        scope.launch(start = CoroutineStart.UNDISPATCHED) {
            try {
                speechSink.speak(text)
            } catch (ce: CancellationException) {
                throw ce
            } catch (t: Throwable) {
                failSafe(t)
            }
        }
    }

    /** SpeakFinished 表情收敛：延迟 N 秒执行（N≤0 时同步，便于测试注入 0）。 */
    private fun scheduleExpressionRelease() {
        if (expressionReleaseDelaySeconds <= 0) {
            try {
                expressionSink.release()
            } catch (ce: CancellationException) {
                throw ce
            } catch (t: Throwable) {
                failSafe(t)
            }
            return
        }
        scope.launch(start = CoroutineStart.UNDISPATCHED) {
            try {
                delay(expressionReleaseDelaySeconds * 1000L)
                expressionSink.release()
            } catch (ce: CancellationException) {
                throw ce
            } catch (t: Throwable) {
                failSafe(t)
            }
        }
    }

    /** I7：异常收敛回 IDLE，不崩溃。 */
    private fun failSafe(t: Throwable) {
        stateMachine.forceIdle()
        println("[LoopCoordinator] fail-safe: converged to IDLE, cause=${t.message}")
    }

    companion object {
        /**
         * 工厂（位置/命名参数形态）：三路接缝 + 可选 stateMachine/scope/收敛延迟。
         */
        fun withDependencies(
            llmLink: LlmLink,
            expressionSink: ExpressionSink,
            speechSink: SpeechSink,
            stateMachine: LoopStateMachine = LoopStateMachine(),
            scope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined),
            expressionReleaseDelaySeconds: Long = 5,
        ): LoopCoordinator = LoopCoordinator(
            llmLink = llmLink,
            expressionSink = expressionSink,
            speechSink = speechSink,
            stateMachine = stateMachine,
            scope = scope,
            expressionReleaseDelaySeconds = expressionReleaseDelaySeconds,
        )

        /** 工厂（DSL 形态）：`withDependencies { llmLink = ...; expressionSink = ...; speechSink = ... }`。 */
        fun withDependencies(block: LoopDependencies.() -> Unit): LoopCoordinator {
            val deps = LoopDependencies().apply(block)
            return LoopCoordinator(
                llmLink = requireNotNull(deps.llmLink) { "withDependencies: llmLink 必填" },
                expressionSink = requireNotNull(deps.expressionSink) { "withDependencies: expressionSink 必填" },
                speechSink = requireNotNull(deps.speechSink) { "withDependencies: speechSink 必填" },
                stateMachine = deps.stateMachine ?: LoopStateMachine(),
                scope = deps.scope ?: CoroutineScope(SupervisorJob() + Dispatchers.Unconfined),
                expressionReleaseDelaySeconds = deps.expressionReleaseDelaySeconds,
            )
        }
    }
}

/**
 * [withDependencies] DSL 依赖容器（属性别名覆盖 llm/llmLink、expr/expressionSink、speech/speechSink 命名）。
 */
class LoopDependencies {
    var llmLink: LlmLink? = null
    var llm: LlmLink?
        get() = llmLink
        set(value) {
            llmLink = value
        }

    var expressionSink: ExpressionSink? = null
    var expr: ExpressionSink?
        get() = expressionSink
        set(value) {
            expressionSink = value
        }

    var speechSink: SpeechSink? = null
    var speech: SpeechSink?
        get() = speechSink
        set(value) {
            speechSink = value
        }

    var stateMachine: LoopStateMachine? = null

    var scope: CoroutineScope? = null

    /** SpeakFinished 表情收敛延迟秒数（契约 C-2，默认 5）。 */
    var expressionReleaseDelaySeconds: Long = 5

    var convergeDelaySeconds: Long
        get() = expressionReleaseDelaySeconds
        set(value) {
            expressionReleaseDelaySeconds = value
        }
}

/** top-level 工厂（DSL 形态，同包 bare 调用）。 */
fun withDependencies(block: LoopDependencies.() -> Unit): LoopCoordinator =
    LoopCoordinator.withDependencies(block)
