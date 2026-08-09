package com.live2dai.android

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.live2dai.android.data.SettingsRepository
import com.live2dai.android.ui.DisplayMessage
import com.live2dai.plugin.memory.MemoryPlugin
import com.live2dai.plugin.memory.MemoryPluginConfig
import com.live2dai.plugin.proactive.ProactivePlugin
import com.live2dai.plugin.proactive.ProactivePluginConfig
import com.live2dai.plugin.vision.VisionPlugin
import com.live2dai.plugin.vision.VisionPluginConfig
import com.live2dai.pluginsdk.ChatContext
import com.live2dai.pluginsdk.ChatHook
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.IOException
import java.net.UnknownHostException

/**
 * 聊天界面 ViewModel — 从 MainActivity.kt 的 MainChatScreen Composable 中提取的
 * 全部业务逻辑。
 *
 * 职责：
 * - 管理聊天状态：displayMessages、inputText、isLoading、isListening
 * - 管理对话框状态：showClearConfirm、showOnboarding、showFreeNotice
 * - 初始化与存活期管理：PluginManager、ToolConfirmationGate、ChatService、VoiceIoController
 * - 发送消息 / 流式 delta 处理 / 聊天历史持久化
 * - ASR 语音输入（委托 voiceIo.listen）
 * - TTS 打断（voiceIo.stopSpeaking）
 * - 清空对话（LLM 上下文重置 + 消息清空 + 本地历史删除）
 * - 首次引导 / 免费额度提示关闭
 * - Live2D 视图接线：EmotionController、ChatHook（afterReceive + onAssistantMessage）、LipSync
 *
 * 生命周期：由 Compose UI 通过 viewModel() 获取；onCleared 时释放 voiceIo 资源。
 */
class ChatViewModel(application: Application) : AndroidViewModel(application) {

    // ── Application / Settings ──

    private val app = getApplication<Application>()
    private val repo = SettingsRepository(app)

    // ── 服务（公开属性供 UI 层引用） ──

    /** 插件管理器：注册与查询 ChatHook / ToolDefinition */
    val pluginManager = PluginManager()

    /** 危险工具确认门：ChatService 挂起等待 → Compose UI 弹框 → respond() 恢复 */
    val confirmationGate = ToolConfirmationGate()

    /** 语音闭环协调器：TTS 说 + ASR 听 + LipSync 嘴型驱动 */
    val voiceIo = VoiceIoController(app)

    /** LLM 流式聊天服务 */
    private val chatService: ChatService

    // ── Live2D 视图 / 表情（onLive2DViewCreated 接线后持有） ──

    private var live2DView: Live2DView? = null
    private var emotionController: EmotionController? = null

    // ── 聊天状态（StateFlow，UI 层 collectAsStateWithLifecycle 收集） ──

    /** 当前会话的显示消息列表（LazyColumn 数据源） */
    val displayMessages = MutableStateFlow<List<DisplayMessage>>(emptyList())

    /** 输入框文本 */
    val inputText = MutableStateFlow("")

    /** LLM 流式请求进行中 */
    val isLoading = MutableStateFlow(false)

    /** ASR 语音识别进行中 */
    val isListening = MutableStateFlow(false)

    // ── 对话框状态（StateFlow，UI 层 collectAsStateWithLifecycle 收集） ──

    /** 清空对话确认对话框 */
    val showClearConfirm = MutableStateFlow(false)

    /** 首次启动引导对话框 */
    val showOnboarding = MutableStateFlow(false)

    /** 免费共享额度提示 banner */
    val showFreeNotice = MutableStateFlow(false)

    // ── 初始化 ──

    init {
        // LLM Provider 管理器初始化（须在 ChatService 构造前）
        LLMProviderManager.init(app)

        // ChatService 持有 pluginManager.hooks/tools 活引用，后注册的 hook 立即可见
        chatService = ChatService(
            provider = LLMProviderManager.getActiveProvider(),
            hooks = pluginManager.hooks,
            tools = pluginManager.tools,
            confirmationGate = confirmationGate
        )

        // 对话框初始状态
        showOnboarding.value = !repo.onboardingCompleted
        showFreeNotice.value = !repo.freeQuotaNoticeDismissed &&
            LLMProviderManager.getActiveProvider().type == ProviderType.ZHIPU

        // 冷启动：装载人设 → 装载插件 → 恢复聊天历史
        viewModelScope.launch {
            PersonaConfig.load(app)
            loadPlugins()
            restoreChatHistory()
        }
    }

    // ── 插件装载 ──

    /**
     * 装载所有插件（suspend：等待 PersonaConfig.load 完成后再读 getVisionModel）。
     *
     * - VisionPlugin：智谱 GLM 多模态视觉（截图分析）
     * - MemoryPlugin：长期记忆（Room 数据库 + 摘要 LLM）
     * - ProactivePlugin：主动陪伴（WorkManager 周期检查，总开关 repo.proactiveEnabled）
     */
    private suspend fun loadPlugins() {
        // VisionPlugin + MemoryPlugin（无条件装载）
        pluginManager.loadPlugins(
            listOf(
                VisionPlugin(
                    VisionPluginConfig(
                        apiKey = repo.zhipuApiKey,
                        visionModel = PersonaConfig.getVisionModel(),
                        screenshotProvider = AndroidScreenshotProvider(app),
                    )
                ),
                MemoryPlugin(
                    MemoryPluginConfig(
                        context = app,
                        summaryLlmClient = LlmSummaryClient(LLMProviderManager.getActiveProvider()),
                    )
                )
            )
        )

        // ProactivePlugin（总开关；关闭时不装载、不排后台任务）
        if (repo.proactiveEnabled) {
            val timing = ProactiveTuning.forDebug(BuildConfig.DEBUG)
            pluginManager.loadPlugins(
                listOf(
                    ProactivePlugin(
                        ProactivePluginConfig(
                            context = app,
                            greetingLlmClient = ProactiveGreetingClient(
                                LLMProviderManager.getActiveProvider()
                            ),
                            memoriesProvider = { ProactiveMemorySource.recentFacts(app) },
                            idleThresholdMinutes = timing.idleThresholdMinutes,
                            cooldownMinutes = timing.cooldownMinutes,
                            workManagerIntervalMinutes = timing.workManagerIntervalMinutes,
                            debugMode = BuildConfig.DEBUG,
                        )
                    )
                )
            )
        }
    }

    // ── 聊天历史恢复 ──

    /**
     * 从磁盘恢复显示消息（仅 UI 层，不恢复 ChatService.messages）。
     *
     * 架构约束 §8.2-4：重启后 ChatService 从头开始，system prompt 重新注入。
     * 若无历史文件 / 文件损坏 → 静默返回空列表，不崩溃。
     */
    private suspend fun restoreChatHistory() {
        val restored = withContext(Dispatchers.IO) {
            ChatHistoryStore.load(app)
        }
        if (displayMessages.value.isEmpty() && restored.isNotEmpty()) {
            displayMessages.value = restored
        }
    }

    // ── 发送消息 ──

    /**
     * 发送用户消息到 LLM，流式接收 delta 并更新 displayMessages。
     *
     * 流程：
     * 1. 打断当前 TTS 播报（避免新旧播报重叠）
     * 2. 添加用户消息到 displayMessages
     * 3. 持久化聊天历史（用户消息完整时保存，避免流式 delta 高频写盘）
     * 4. 发起 LLM 流式请求 → 收集 delta 追加/新建助手消息
     * 5. 错误处理：网络失败中文友好提示 / 其他异常展示原始信息
     * 6. finally：isLoading = false + 持久化完整回复
     *
     * @param text 用户输入的文本（已 trim）
     */
    fun sendMessage(text: String) {
        // 发送新消息前打断当前 TTS 播报
        voiceIo.stopSpeaking()

        // 显示用户消息
        displayMessages.value =
            displayMessages.value + DisplayMessage(content = text, isUser = true)
        inputText.value = ""
        isLoading.value = true

        // 用户消息加入即持久化（完整消息时保存，避免流式 delta 高频写盘）
        viewModelScope.launch {
            withContext(Dispatchers.IO) {
                ChatHistoryStore.save(app, displayMessages.value)
            }
        }

        // 发起 LLM 流式请求
        viewModelScope.launch {
            try {
                chatService.sendMessage(text).collect { delta ->
                    // 共享免费 Key 额度耗尽：剥离标记、标记为错误并提供设置入口
                    val isQuotaExhausted =
                        delta.startsWith(ChatService.QUOTA_EXHAUSTED_MARKER)
                    val displayDelta = if (isQuotaExhausted) {
                        delta.removePrefix(ChatService.QUOTA_EXHAUSTED_MARKER)
                    } else {
                        delta
                    }
                    val current = displayMessages.value
                    val lastIdx = current.lastIndex
                    if (lastIdx >= 0 &&
                        !current[lastIdx].isUser &&
                        !current[lastIdx].isError
                    ) {
                        // 追加到已有的助手消息（流式 delta）：整体替换列表（StateFlow 不可变快照）
                        displayMessages.value = current.toMutableList().also { list ->
                            list[lastIdx] = list[lastIdx].copy(
                                content = list[lastIdx].content + displayDelta
                            )
                        }
                    } else {
                        // 新助手消息
                        displayMessages.value = current + DisplayMessage(
                            content = displayDelta,
                            isUser = false,
                            isError = isQuotaExhausted,
                            showSettingsButton = isQuotaExhausted
                        )
                    }
                }
            } catch (e: Exception) {
                // 离线/网络失败 → 中文友好提示；其他异常 → 展示原始信息
                val errorMessage = when (e) {
                    is UnknownHostException ->
                        "网络连接失败，请检查网络后重试"
                    is IOException ->
                        "网络连接失败，请检查网络后重试"
                    else ->
                        e.message?.let { "发送失败: $it" }
                            ?: "发生未知错误，请重试"
                }
                displayMessages.value = displayMessages.value + DisplayMessage(
                    content = errorMessage,
                    isUser = false,
                    isError = true
                )
            } finally {
                isLoading.value = false
                // M9 遗留修复：流式 delta 不经 afterReceive hook，完整回复结束时统一剥离
                // 结构化 JSON（TTS 已不朗读，此处保证 UI 字幕干净，不显示 JSON 块原文）
                val controller = emotionController
                if (controller != null) {
                    val msgs = displayMessages.value
                    val lastIdx = msgs.lastIndex
                    if (lastIdx >= 0 && !msgs[lastIdx].isUser && !msgs[lastIdx].isError) {
                        val stripped = controller.stripStructuredJson(msgs[lastIdx].content)
                        if (stripped != msgs[lastIdx].content) {
                            displayMessages.value = msgs.toMutableList().also { list ->
                                list[lastIdx] = list[lastIdx].copy(content = stripped)
                            }
                        }
                    }
                }
                // 助手回复完整/失败后持久化
                viewModelScope.launch {
                    withContext(Dispatchers.IO) {
                        ChatHistoryStore.save(app, displayMessages.value)
                    }
                }
            }
        }
    }

    // ── 语音输入（ASR） ──

    /**
     * 启动语音识别（ASR）。
     *
     * 调用 voiceIo.listen 按降级链识别；成功 → 填入输入框（可编辑后发送）；
     * 失败/空结果 → 静默降级为文本输入（isListening 恢复 false，inputText 不变）。
     *
     * @param providerId 首选 ASR 引擎 ID（如 "sherpa_sensevoice"）；指定引擎失败后
     *                   仍按降级链自动兜底。通常来自 SettingsRepository.asrProviderId。
     */
    fun startListening(providerId: String) {
        isListening.value = true
        viewModelScope.launch {
            val recognized = voiceIo.listen(providerId)
            isListening.value = false
            if (recognized.isNotEmpty()) {
                inputText.value = recognized
            }
        }
    }

    // ── TTS 打断 ──

    /**
     * 打断当前 TTS 播报。
     *
     * 遍历降级链调用 stop()，确保切 Provider 后旧引擎资源释放。
     */
    fun stopSpeaking() {
        voiceIo.stopSpeaking()
    }

    // ── 清空对话 ──

    /**
     * 清空当前会话：重置 LLM 上下文 + 清空显示消息 + 删除本地历史文件。
     *
     * 破坏性操作（不可恢复），调用前应由 UI 层二次确认。
     */
    fun clearConversation() {
        chatService.resetConversation()
        displayMessages.value = emptyList()
        viewModelScope.launch {
            withContext(Dispatchers.IO) {
                ChatHistoryStore.clear(app)
            }
        }
        showClearConfirm.value = false
    }

    // ── 引导 / 提示关闭 ──

    /**
     * 关闭首次启动引导对话框（"以后再说"路径：仅标记 onboarding 完成，
     * 不影响免费额度 banner）。
     */
    fun dismissOnboarding() {
        repo.onboardingCompleted = true
        showOnboarding.value = false
    }

    /**
     * 关闭首次启动引导对话框（"开始聊天"路径：同时标记 onboarding 完成 +
     * 免费额度提示已读，避免紧接着再弹 banner 造成重复打扰）。
     */
    fun dismissOnboardingAndFreeNotice() {
        repo.onboardingCompleted = true
        repo.freeQuotaNoticeDismissed = true
        showOnboarding.value = false
        showFreeNotice.value = false
    }

    /**
     * 关闭免费共享额度提示 banner。
     */
    fun dismissFreeNotice() {
        repo.freeQuotaNoticeDismissed = true
        showFreeNotice.value = false
    }

    // ── Live2D 视图接线 ──

    /**
     * Live2D 视图创建后接线：EmotionController → ChatHook（afterReceive +
     * onAssistantMessage）→ LipSync。
     *
     * 应当在 AndroidView.factory 中调用，确保 Live2DView 实例已完成 initialize。
     *
     * @param view 已初始化的 Live2DView 实例
     */
    fun onLive2DViewCreated(view: Live2DView) {
        live2DView = view

        val controller = EmotionController(view)
        this.emotionController = controller

        // M7：motionLabel 触发接线（contract-M2-M3 §1.3(d) 接缝 → Live2DView.startMotionByTag，
        // 优先级 NORMAL；contract-M7 §3.1）。applyStructured 的 motionLabel 非 null 时经此触发动作。
        controller.motionTrigger = { tag ->
            view.startMotionByTag(tag, MotionPriority.NORMAL)
        }

        // 插件通道 onAssistantMessage：供主动陪伴类插件调用
        // （PluginHost.sendAssistantMessage → 表情 + TTS 播报）
        pluginManager.onAssistantMessage = { text, emotion ->
            val result = controller.parseStructured(text)
            if (result != null) {
                // 结构化 JSON 优先（M2，contract-M2-M3 §1.4）：应用混合/覆盖/动作，
                // TTS 播报剥离 JSON 后的干净文本；motionLabel 存在 → 关键词兜底整体禁用（M7 §3.2 防线①）
                controller.applyStructured(result)
                val clean = controller.stripStructuredJson(text)
                viewModelScope.launch {
                    voiceIo.speak(
                        clean,
                        repo.ttsProviderId,
                        keywordMotionEnabled = result.motionLabel == null
                    )
                }
            } else {
                // 现有路径一字不改：[keyword] 降级（stripEmotionTags + speak 内部 sanitizeForTts）
                emotion?.let { controller.setEmotion(it) }
                viewModelScope.launch {
                    voiceIo.speak(
                        EmotionController.stripEmotionTags(text),
                        repo.ttsProviderId
                    )
                }
            }
        }

        // 核心 ChatHook.afterReceive：ChatService 每条助手回复后触发
        // （表情切换 + TTS 播报剥离标签后的纯文本；ChatService 已先行 stripEmotionTags，JSON 保留）
        pluginManager.registerHook(object : ChatHook {
            override suspend fun beforeSend(
                userMessage: String,
                context: ChatContext
            ): String? = null

            override suspend fun afterReceive(
                assistantMessage: String,
                emotion: String?,
                context: ChatContext
            ) {
                val result = controller.parseStructured(assistantMessage)
                if (result != null) {
                    // 结构化 JSON 优先（M2）：应用混合/覆盖/动作，TTS 播报剥离 JSON 后的干净文本；
                    // motionLabel 存在 → 关键词兜底整体禁用（M7 §3.2 防线①）
                    controller.applyStructured(result)
                    val clean = controller.stripStructuredJson(assistantMessage)
                    voiceIo.speak(
                        clean,
                        repo.ttsProviderId,
                        keywordMotionEnabled = result.motionLabel == null
                    )
                } else {
                    // 现有路径一字不改：[keyword] 降级
                    emotion?.let { controller.setEmotion(it) }
                    voiceIo.speak(assistantMessage, repo.ttsProviderId)
                }
            }
        })

        // LipSync 接线：V1.2 起由各 TtsProvider.isSpeaking StateFlow 驱动嘴型
        voiceIo.attachLive2DView(view)
    }

    // ── 资源释放 ──

    /**
     * ViewModel 销毁时释放 voiceIo（TTS + ASR + LipSync）资源。
     */
    override fun onCleared() {
        super.onCleared()
        voiceIo.release()
    }
}
