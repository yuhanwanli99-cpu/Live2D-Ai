package com.live2d.ai.android

import android.Manifest
import android.content.pm.PackageManager
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.CompositionLocalProvider
import androidx.core.content.ContextCompat
import com.live2d.ai.android.core.HttpLlmLink
import com.live2d.ai.android.core.LlmEndpointConfig
import com.live2d.ai.android.core.LlmLink
import com.live2d.ai.android.core.LoopCommand
import com.live2d.ai.android.core.LoopCoordinator
import com.live2d.ai.android.core.LoopEvent
import com.live2d.ai.android.core.LoopState
import com.live2d.ai.android.core.LoopViewHost
import com.live2d.ai.android.core.LoopWire
import com.live2d.ai.android.core.SpeechSinkAdapter
import com.live2d.ai.android.core.TimedExpressionSink
import com.live2d.ai.android.core.VisualExpressionSink
import com.live2d.ai.android.core.UiAppendStreamBubble
import com.live2d.ai.android.data.SettingsRepository
import com.live2d.ai.android.ui.LocalLoopWire
import com.live2d.ai.android.ui.LoopWireBindings
import com.live2d.ai.android.ui.UiBubble
import com.live2d.ai.android.ui.theme.Live2DAiTheme
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import okhttp3.OkHttpClient
import java.util.concurrent.TimeUnit

/**
 * Wave 4 · 4-1 三片真实绑定：核心环路（聊天→LLM→回复文本→TTS 出声+嘴型帧→Live2D 动画）
 * 端到端接线（契约 refactor-0814 §2.1-§2.3）。
 *
 * 注册链（onCreate）：
 * 1. TTS Provider 注册（registerDefaultTtsProviders，须在 VoiceIoController 构造前）；
 * 2. PersonaConfig / LLMProviderManager 就绪（HttpLlmLink 构造依赖 persona system prompt）；
 * 3. 构造 VoiceIoController → SpeechSinkAdapter（B 片适配）→ HttpLlmLink（baseUrl/apiKey
 *    取 LLMProviderManager.getActiveProvider() 现有取值点）→ TimedExpressionSink；
 * 4. LoopWire.assemble 组装 LoopCoordinator（withDependencies + attachView，I6 替换语义）；
 * 5. 泵：LLM 事件泵（I3 单消费者）+ SpeakFinished/SpeakFailed 泵（isSpeaking 下降沿）；
 * 6. 消费面经 CompositionLocal（[LocalLoopWire]）提供给 MainScreen。
 *
 * 生命周期：视图绑定在 AndroidView factory（onResume 后）经 onLive2DViewWired 建立
 * （host.zeroMouth 目标 + VoiceIo LipSync 绑定）；onPause 发送 Reset 事件（stop + 收敛 + llm.reset）。
 *
 * 旧编排已删除（Wave 5）：旧 Chat 链类文件已随删除清单移除；
 * 本类直接组装新链。
 */
class MainActivity : ComponentActivity() {

    companion object {
        private const val TAG = "MainActivity"
    }

    /** 泵/协程作用域（Activity 级；onDestroy cancel）。 */
    private val appScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    // ── 核心环路（Wave 4 三片真实绑定）──
    private lateinit var voiceIo: VoiceIoController
    private lateinit var voiceInput: VoiceInputController
    private lateinit var speechSink: SpeechSinkAdapter
    private lateinit var llmLink: LlmLink
    private lateinit var coordinator: LoopCoordinator

    // ── 消费面（MainScreen 经 LocalLoopWire 收集；气泡追加序 + 状态机状态）──
    private val _bubbles = MutableStateFlow<List<UiBubble>>(emptyList())
    private val _loopState = MutableStateFlow<LoopState>(LoopState.IDLE)

    /**
     * LoopViewHost 适配器（绑定规格 3）：onAttach/onDetach 日志；zeroMouth →
     * Live2DView.setLipSyncParam(0f)（I4 落点，视图由 onLive2DViewWired 注入）；
     * onState 记录日志。
     */
    private val host = object : LoopViewHost {
        @Volatile
        var view: Live2DView? = null

        override fun onAttach() {
            Log.i(TAG, "LoopHost 已绑定（attachView 替换语义，I6）")
        }

        override fun onDetach() {
            Log.i(TAG, "LoopHost 已解绑")
        }

        override fun zeroMouth() {
            view?.setLipSyncParam(0f)
        }

        override fun onState(state: LoopState) {
            Log.i(TAG, "LoopState=$state")
        }
    }

    /**
     * 状态观察：coordinator 不转发状态机 onState 到宿主（2C-1 既有缺口），且
     * [LoopStateMachine] 为 final 不可继承——以轮询 [LoopCoordinator.state] 驱动消费面
     * [loopState]（UI isLoading 源）+ 宿主 onState 日志。仅状态变化时转发。
     */
    private fun observeLoopState() {
        appScope.launch {
            while (isActive) {
                val current = coordinator.state
                if (current != _loopState.value) {
                    _loopState.value = current
                    host.onState(current)
                }
                delay(100)
            }
        }
    }

    private val requestRecordAudioPermission =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
            if (!granted) {
                Log.w(TAG, "RECORD_AUDIO permission denied")
            }
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // V1.2 模块1：注册默认 TTS Provider（须在 VoiceIoController 构造前；同名覆盖幂等）
        registerDefaultTtsProviders(applicationContext)
        // Persona/Provider 就绪（旧链入口已删除；新链在此显式加载）
        PersonaConfig.load(applicationContext)
        LLMProviderManager.init(applicationContext)

        voiceIo = VoiceIoController(applicationContext)
        voiceInput = VoiceInputController(
            applicationContext,
            sherpaEngine = createSherpaAsrEngine(),
        )
        speechSink = SpeechSinkAdapter(voiceIo)
        speechSink.ttsProviderId = SettingsRepository(this).ttsProviderId
        llmLink = buildHttpLlmLink()
        coordinator = LoopWire.assemble(
            llmLink = llmLink,
            // Wave 4.1 修复 2：视觉表情落点——TimedExpressionSink（状态机语义）之上叠
            // VisualExpressionSink（apply/release 转发到 Live2DView.setParametricExpression）；
            // viewProvider 懒取 host.view（onLive2DViewWired 后可用，之前为 null → 视觉静默）。
            expressionSink = VisualExpressionSink(TimedExpressionSink()) { host.view },
            speechSink = speechSink,
            host = host,
            scope = appScope,
        )
        coordinator.uiListener = { command -> handleUiCommand(command) }

        // ── 泵（I3 单消费者 LLM 泵 + SpeakFinished/SpeakFailed 泵）+ 状态观察 ──
        LoopWire.pump(coordinator, llmLink, appScope)
        LoopWire.pumpSpeakFinished(coordinator, speechSink, appScope)
        observeLoopState()

        setContent {
            Live2DAiTheme {
                CompositionLocalProvider(
                    LocalLoopWire provides LoopWireBindings(
                        onSubmit = { text -> coordinator.sendMessage(text) },
                        bubbles = _bubbles.asStateFlow(),
                        loopState = _loopState.asStateFlow(),
                        voiceState = voiceInput.state,
                        isSpeaking = voiceIo.isSpeaking,
                        activeTtsProvider = voiceIo.activeProviderId,
                        onMicClick = {
                            if (!SettingsRepository(this@MainActivity).asrEnabled) return@LoopWireBindings
                            if (ContextCompat.checkSelfPermission(
                                    this@MainActivity,
                                    Manifest.permission.RECORD_AUDIO
                                ) != PackageManager.PERMISSION_GRANTED
                            ) {
                                requestRecordAudioPermission.launch(Manifest.permission.RECORD_AUDIO)
                            } else {
                                voiceInput.toggle { text ->
                                    coordinator.sendMessage(text)
                                }
                            }
                        },
                        onLive2DViewWired = { view ->
                            // 视图绑定（onResume 后 AndroidView factory 中建立）：
                            // host.zeroMouth 目标 + VoiceIo LipSync 绑定（B→A 嘴型直连）
                            host.view = view
                            voiceIo.attachLive2DView(view)
                        },
                    )
                ) {
                    Live2DAiApp(onRuntimeSettingsChanged = { applyRuntimeSettings() })
                }
            }
        }
    }

    override fun onPause() {
        super.onPause()
        // onPause 时 Reset：speech.stop + 收敛（mouth.zero + expr.release）+ llm.reset（通配 Reset）
        coordinator.onEvent(LoopEvent.Reset)
    }

    override fun onDestroy() {
        super.onDestroy()
        appScope.cancel()
        voiceInput.destroy()
        voiceIo.release()
    }

    /** 按设置创建 sherpa 离线 ASR 引擎；未启用/未下载/不可用 → null（自动回退系统识别）。 */
    private fun createSherpaAsrEngine(): SherpaAsrEngine? {
        if (SettingsRepository(this).asrEngine != "sherpa") return null
        SherpaOnnxEngineRegistry.tryBootstrap()
        val factory = SherpaOnnxEngineRegistry.factoryOrNull() ?: return null
        val manager = ModelDownloadManager(applicationContext)
        val spec = ModelDownloadManager.SENSEVOICE_ASR
        if (!manager.isDownloaded(spec)) {
            Log.w(TAG, "SenseVoice ASR 模型未下载，使用系统语音识别")
            return null
        }
        return factory.createAsr(manager.modelDir(spec))
    }

    /** HttpLlmLink 构造：baseUrl/apiKey 取现有取值点（LLMProviderManager.getActiveProvider）。 */
    private fun buildHttpLlmLink(): HttpLlmLink {
        val provider = LLMProviderManager.getActiveProvider()
        val repo = SettingsRepository(this)
        val client = OkHttpClient.Builder()
            .connectTimeout(60, TimeUnit.SECONDS)
            .readTimeout(120, TimeUnit.SECONDS)
            .writeTimeout(60, TimeUnit.SECONDS)
            .build()
        return HttpLlmLink(
            baseUrl = provider.baseUrl,
            apiKey = provider.apiKey,
            client = client,
            persona = PersonaConfig,
        ).apply {
            // 出生即用「当前生效聊天模型/温度」（设置页选择，而非仅 persona.yaml）
            rebuild(
                LlmEndpointConfig(
                    baseUrl = provider.baseUrl,
                    apiKey = provider.apiKey,
                    model = repo.model,
                    temperature = repo.temperature,
                )
            )
        }
    }

    /**
     * 应用当前设置到运行中实例（设置页保存后经 Live2DAiApp 回调调用，无需重启）：
     * - LLM：从当前 Provider + 设置页聊天模型/温度重建链接并替换 LoopCoordinator 的引用；
     * - TTS：从注册表刷新 Provider 列表（新 Key/新引擎可见），并更新首选引擎 id。
     */
    private fun applyRuntimeSettings() {
        val repo = SettingsRepository(this)
        val provider = LLMProviderManager.getActiveProvider()
        if (llmLink is HttpLlmLink) {
            (llmLink as HttpLlmLink).rebuild(
                LlmEndpointConfig(
                    baseUrl = provider.baseUrl,
                    apiKey = provider.apiKey,
                    model = repo.model,
                    temperature = repo.temperature,
                )
            )
        } else {
            // 兜底：非 HttpLlmLink（测试注入的 fake）→ 重建实例并替换引用
            llmLink = buildHttpLlmLink()
        }
        if (::coordinator.isInitialized) {
            coordinator.replaceLlm(llmLink)
        }
        voiceIo.refreshProviders()
        speechSink.ttsProviderId = repo.ttsProviderId
    }

    /** UI 命令消费：气泡追加序（用户气泡 / 流式增量 / 错误气泡），尾插保序。 */
    private fun handleUiCommand(command: LoopCommand) {
        when (command) {
            is LoopCommand.UiAddUserBubble -> appendBubble(UiBubble.User(command.text))
            is UiAppendStreamBubble -> appendStreamBubble(command.fragment)
            is LoopCommand.UiShowError -> appendBubble(UiBubble.Error(command.message))
            else -> Unit // 非 UI 命令（Llm/LlmReset/SpeechStop/SpeechSpeak/MouthZero/Expr*）不消费
        }
    }

    private fun appendBubble(bubble: UiBubble) {
        _bubbles.value = _bubbles.value + bubble
    }

    /** 流式增量：末位为助手气泡则追加，否则新建（SENDING 首个 Delta 前无助手气泡）。 */
    private fun appendStreamBubble(fragment: String) {
        val current = _bubbles.value
        val last = current.lastOrNull()
        _bubbles.value = if (last is UiBubble.Assistant) {
            current.dropLast(1) + last.copy(text = last.text + fragment)
        } else {
            current + UiBubble.Assistant(fragment)
        }
    }
}
