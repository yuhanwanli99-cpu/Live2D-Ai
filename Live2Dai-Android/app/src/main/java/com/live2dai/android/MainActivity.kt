package com.live2dai.android

import android.Manifest
import android.content.pm.PackageManager
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Mic
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.live2dai.android.ui.settings.SettingsScreen
import com.live2dai.android.ui.theme.Live2DaiTheme
import com.live2dai.android.data.ModelRegistry
import com.live2dai.android.data.SettingsRepository
import com.live2dai.plugin.memory.MemoryPlugin
import com.live2dai.plugin.memory.MemoryPluginConfig
import com.live2dai.plugin.vision.VisionPlugin
import com.live2dai.plugin.vision.VisionPluginConfig
import com.live2dai.pluginsdk.ChatContext
import com.live2dai.pluginsdk.ChatHook
import java.util.UUID
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.Serializable

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            Live2DaiTheme {
                Live2DaiApp()
            }
        }
    }
}

/**
 * 应用根组件。
 * 管理主界面与设置界面之间的导航。
 *
 * Live2D 视图引用提升到 App 级：设置页打开时主聊天界面**保持存活**（以覆盖层方式
 * 叠在设置页下方），因此设置页切换形象时 Live2DView 实例仍然存在，可以真正热切换
 * （Live2DView.switchModel），而不是等回到聊天页才重建模型。
 */
@Composable
fun Live2DaiApp() {
    var showSettings by remember { mutableStateOf(false) }
    val context = LocalContext.current
    val repo = remember { SettingsRepository(context) }
    val registry = remember { ModelRegistry.load(context) }
    // Live2D 视图引用（由 MainChatScreen 创建后上报）
    val live2DView = remember { mutableStateOf<Live2DView?>(null) }
    val lifecycleOwner = LocalLifecycleOwner.current

    // Live2D 视图生命周期：ON_RESUME / ON_PAUSE / ON_DESTROY 转发
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            when (event) {
                Lifecycle.Event.ON_RESUME -> live2DView.value?.onResume()
                Lifecycle.Event.ON_PAUSE -> live2DView.value?.onPause()
                Lifecycle.Event.ON_DESTROY -> live2DView.value?.dispose()
                else -> {}
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose {
            lifecycleOwner.lifecycle.removeObserver(observer)
            live2DView.value?.dispose()
        }
    }

    Box(modifier = Modifier.fillMaxSize()) {
        // 主聊天界面始终存活：设置页以覆盖层打开，Live2D 背景不销毁，形象可热切换
        MainChatScreen(
            onOpenSettings = { showSettings = true },
            onLive2DViewCreated = { live2DView.value = it }
        )

        if (showSettings) {
            SettingsScreen(
                onNavigateBack = { showSettings = false },
                onSwitchModel = { modelId, onResult ->
                    val entry = registry.getById(modelId)
                    val view = live2DView.value
                    if (entry == null || view == null) {
                        onResult(false)
                    } else {
                        val path = "live2d/${entry.model3JsonPath}"
                        // 回调在 GL 线程执行；SettingsScreen 内部负责 post 回主线程
                        view.switchModel(
                            assetPath = path,
                            onComplete = { onResult(true) },
                            onError = { onResult(false) }
                        )
                    }
                }
            )
        }
    }
}

/**
 * 显示在气泡中的单条消息数据。
 *
 * [Serializable]：V1.1 模块2 聊天历史持久化（ChatHistoryStore 序列化到 JSON）。
 * `id` 带默认值（UUID），反序列化时缺失 id 的旧数据会自动补默认值，不破坏兼容性。
 */
@Serializable
data class DisplayMessage(
    /** 稳定唯一标识：作为 LazyColumn key，保证流式追加（copy 保留 id）时 key 不变且互不冲突 */
    val id: String = UUID.randomUUID().toString(),
    val content: String,
    val isUser: Boolean,
    val isError: Boolean = false,
    /** 共享免费 Key 额度耗尽提示：气泡下方展示「前往设置填写自己的 API Key」入口 */
    val showSettingsButton: Boolean = false
)

/**
 * 主聊天界面。
 * 包含 Live2D 模型背景、对话气泡列表、和底部的输入栏。
 * 右上角齿轮按钮 → 设置页。
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MainChatScreen(
    onOpenSettings: () -> Unit,
    onLive2DViewCreated: (Live2DView) -> Unit,
) {
    val context = LocalContext.current
    val repo = remember { SettingsRepository(context) }

    // ChatService 实例（整个 Screen 生命周期内保持）
    // 插件在冷启动 LaunchedEffect 中装载（依赖 PersonaConfig/SettingsRepository 配置源），
    // ChatService 持有 pluginManager.hooks 的活引用，之后注册的 hook 同样可见。
    val pluginManager = remember { PluginManager() }
    val confirmationGate = remember { ToolConfirmationGate() }
    val chatService = remember {
        // 先初始化 provider 管理器再取活动 provider：零配置（未存过 provider 类型）时
        // 走智谱 GLM 免费兜底（无 ZHIPU key 则回退 DeepSeek），见 LLMProviderManager。
        LLMProviderManager.init(context)
        ChatService(
            provider = LLMProviderManager.getActiveProvider(),
            hooks = pluginManager.hooks,
            tools = pluginManager.tools,
            confirmationGate = confirmationGate
        )
    }

    // V1.1 模块1：语音闭环协调器（TTS 说 + ASR 听）。
    // 用 applicationContext 持有（EdgeTtsService 内部持有 TextToSpeech/MediaPlayer），
    // 避免泄漏 Activity 引用。
    val voiceIo = remember { VoiceIoController(context.applicationContext) }

    // V1.1 模块1：Activity 销毁时释放 TTS + ASR 资源
    DisposableEffect(Unit) {
        onDispose {
            voiceIo.release()
        }
    }

    // 冷启动初始化
    LaunchedEffect(Unit) {
        PersonaConfig.load(context)
        // LLMProviderManager.init 已在 ChatService 创建前调用（幂等，此处不再重复）
        // 装载视觉插件（依赖上面的配置源：SettingsRepository.zhipuApiKey / PersonaConfig.getVisionModel()）
        // 截图能力由 core 注入（AndroidScreenshotProvider），当前为空壳（capture()=null），
        // hook 会静默跳过，对话流程不受影响。参见 docs/plan-task-extract-vision.md
        pluginManager.loadPlugins(
            listOf(
                VisionPlugin(
                    VisionPluginConfig(
                        apiKey = SettingsRepository(context.applicationContext).zhipuApiKey,
                        visionModel = PersonaConfig.getVisionModel(),
                        screenshotProvider = AndroidScreenshotProvider(context.applicationContext),
                    )
                ),
                // V1.1 模块3：长期记忆集成（docs/plan-v11-plan.md §3.3）
                // - 摘要 LLM 由 core 注入（LlmSummaryClient 复用当前活动 Provider，见 §5.3），
                //   插件自身不接 LLM 客户端；超时/失败静默，不影响对话主链路（架构约束 §8.2-2）
                // - Room 数据库懒打开：onLoad 的 MemoryDatabase.build 不触达 SQLite，不阻塞启动
                // - beforeSend 注入相关记忆 → afterReceive 每 10 轮摘要写入（默认）→ 写入后 trim 到 500 条
                MemoryPlugin(
                    MemoryPluginConfig(
                        context = context.applicationContext,
                        summaryLlmClient = LlmSummaryClient(LLMProviderManager.getActiveProvider()),
                    )
                )
            )
        )
    }

    // ── 聊天状态 ──
    var inputText by remember { mutableStateOf("") }
    val displayMessages = remember { mutableStateListOf<DisplayMessage>() }
    var isLoading by remember { mutableStateOf(false) }
    // 清空对话确认对话框开关
    var showClearConfirm by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val listState = rememberLazyListState()

    // ── V1.1 模块1：语音输入（ASR）──
    // 识别中状态：麦克风按钮切换为 loading 图标，防止重复触发
    var isListening by remember { mutableStateOf(false) }

    // 启动识别（权限已就绪时调用）：识别成功填入输入框（可编辑后发送），
    // 失败/空结果静默降级为文本输入（plan §3.1.3 ASR 路径）
    val startListening: () -> Unit = {
        isListening = true
        Toast.makeText(context, "正在聆听，请说话...", Toast.LENGTH_SHORT).show()
        scope.launch {
            val recognized = voiceIo.listen()
            isListening = false
            if (recognized.isNotEmpty()) {
                inputText = recognized
            }
        }
    }

    // 麦克风权限按需申请（plan §8.2-3）：仅点击麦克风按钮时请求，不在启动时批量申请
    val requestMicPermission = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted ->
        if (granted) {
            startListening()
        } else {
            Toast.makeText(context, "需要麦克风权限才能语音输入", Toast.LENGTH_SHORT).show()
        }
    }

    val onMicClick: () -> Unit = {
        if (ContextCompat.checkSelfPermission(context, Manifest.permission.RECORD_AUDIO) ==
            PackageManager.PERMISSION_GRANTED
        ) {
            startListening()
        } else {
            requestMicPermission.launch(Manifest.permission.RECORD_AUDIO)
        }
    }

    // ── 聊天历史恢复（V1.1 模块2）：冷启动从磁盘恢复显示消息 ──
    // 只恢复 UI 显示消息，不恢复 ChatService.messages（重启后 system prompt 重新注入，
    // 架构约束 §8.2-4）。IO 线程读取避免卡主线程；若已有消息（如恢复前已发生交互）则不覆盖。
    // 恢复完成后 displayMessages.size 变化会触发下方自动滚动到底部。
    LaunchedEffect(Unit) {
        val restored = withContext(Dispatchers.IO) {
            ChatHistoryStore.load(context.applicationContext)
        }
        if (displayMessages.isEmpty() && restored.isNotEmpty()) {
            displayMessages.addAll(restored)
        }
    }

    // ── 首次启动引导（一次性、可跳过、不阻塞聊天） ──
    var showOnboarding by remember { mutableStateOf(!repo.onboardingCompleted) }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Live2Dai") },
                actions = {
                    IconButton(onClick = { showClearConfirm = true }) {
                        Icon(Icons.Default.Delete, contentDescription = "清空对话")
                    }
                    IconButton(onClick = onOpenSettings) {
                        Icon(Icons.Default.Settings, contentDescription = "设置")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.primaryContainer,
                    titleContentColor = MaterialTheme.colorScheme.onPrimaryContainer,
                )
            )
        },
        bottomBar = {
            Surface(
                modifier = Modifier.fillMaxWidth(),
                tonalElevation = 3.dp
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    OutlinedTextField(
                        value = inputText,
                        onValueChange = { if (!isLoading) inputText = it },
                        placeholder = { Text("和小喵聊天...") },
                        modifier = Modifier.weight(1f),
                        singleLine = true,
                        enabled = !isLoading
                    )
                    Spacer(Modifier.width(8.dp))
                    // ── V1.1 模块1：语音输入（ASR）麦克风按钮 ──
                    // 识别中：loading 圆环反馈；失败/空结果静默（UI 无变化，降级文本输入）
                    IconButton(
                        onClick = onMicClick,
                        enabled = !isLoading
                    ) {
                        if (isListening) {
                            CircularProgressIndicator(
                                modifier = Modifier.size(20.dp),
                                strokeWidth = 2.dp
                            )
                        } else {
                            Icon(Icons.Default.Mic, contentDescription = "语音输入")
                        }
                    }
                    Spacer(Modifier.width(8.dp))
                    FilledTonalButton(
                        onClick = {
                            val text = inputText.trim()
                            if (text.isNotEmpty() && !isLoading) {
                                // V1.1 模块1：发送新消息前打断当前 TTS 播报（避免新旧播报重叠）
                                voiceIo.stopSpeaking()
                                // 显示用户消息
                                displayMessages.add(
                                    DisplayMessage(content = text, isUser = true)
                                )
                                inputText = ""
                                isLoading = true

                                // V1.1 模块2：用户消息加入即持久化（消息完整时保存，
                                // 避免流式 delta 高频写盘）
                                scope.launch {
                                    withContext(Dispatchers.IO) {
                                        ChatHistoryStore.save(context.applicationContext, displayMessages)
                                    }
                                }

                                // 发起 LLM 流式请求（零配置默认走 GLM 免费兜底，见 LLMProviderManager）
                                scope.launch {
                                    try {
                                        chatService.sendMessage(text).collect { delta ->
                                            // 共享免费 Key 额度耗尽提示：剥离标记、标记为错误并提供设置入口
                                            val isQuotaExhausted = delta.startsWith(ChatService.QUOTA_EXHAUSTED_MARKER)
                                            val displayDelta = if (isQuotaExhausted) {
                                                delta.removePrefix(ChatService.QUOTA_EXHAUSTED_MARKER)
                                            } else {
                                                delta
                                            }
                                            val lastIdx = displayMessages.lastIndex
                                            if (lastIdx >= 0 &&
                                                !displayMessages[lastIdx].isUser &&
                                                !displayMessages[lastIdx].isError
                                            ) {
                                                // 追加到已有的助手消息
                                                displayMessages[lastIdx] =
                                                    displayMessages[lastIdx].copy(
                                                        content = displayMessages[lastIdx].content + displayDelta
                                                    )
                                            } else {
                                                // 新助手消息
                                                displayMessages.add(
                                                    DisplayMessage(
                                                        content = displayDelta,
                                                        isUser = false,
                                                        isError = isQuotaExhausted,
                                                        showSettingsButton = isQuotaExhausted
                                                    )
                                                )
                                            }
                                        }
                                    } catch (e: Exception) {
                                        // 离线/网络失败（UnknownHostException / IOException 及其子类）展示中文
                                        // 友好提示；其他异常保持展示原始信息（null 时给出兜底文案）
                                        val errorMessage = when (e) {
                                            is java.net.UnknownHostException ->
                                                "网络连接失败，请检查网络后重试"
                                            is java.io.IOException ->
                                                "网络连接失败，请检查网络后重试"
                                            else ->
                                                e.message?.let { "发送失败: $it" }
                                                    ?: "发生未知错误，请重试"
                                        }
                                        displayMessages.add(
                                            DisplayMessage(
                                                content = errorMessage,
                                                isUser = false,
                                                isError = true
                                            )
                                        )
                                    } finally {
                                        isLoading = false
                                        // V1.1 模块2：助手回复完整/失败后持久化
                                        // （流式过程中不写盘，避免每个 delta 都触发磁盘 IO）
                                        scope.launch {
                                            withContext(Dispatchers.IO) {
                                                ChatHistoryStore.save(context.applicationContext, displayMessages)
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        enabled = inputText.isNotBlank() && !isLoading
                    ) {
                        Text(if (isLoading) "..." else "发送")
                    }
                }
            }
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
        ) {
            // ── 免费共享额度提示（仅零配置默认 Provider 时、且用户未手动关闭时展示） ──
            val showFreeNotice = remember {
                mutableStateOf(
                    !repo.freeQuotaNoticeDismissed &&
                        LLMProviderManager.getActiveProvider().type == ProviderType.ZHIPU
                )
            }
            if (showFreeNotice.value) {
                Surface(
                    modifier = Modifier.fillMaxWidth(),
                    color = MaterialTheme.colorScheme.secondaryContainer.copy(alpha = 0.9f)
                ) {
                    Row(
                        modifier = Modifier.padding(start = 12.dp, top = 4.dp, bottom = 4.dp, end = 4.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            "当前使用免费共享额度（GLM-4.7-Flash）：多人共用、限 1 并发。" +
                                "可在「设置 → 服务商」换用自己的 API Key。",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSecondaryContainer,
                            modifier = Modifier.weight(1f)
                        )
                        IconButton(onClick = {
                            repo.freeQuotaNoticeDismissed = true
                            showFreeNotice.value = false
                        }) {
                            Icon(Icons.Default.Close, contentDescription = "关闭提示")
                        }
                    }
                }
            }

            // ── 聊天主区域 ──
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f)
            ) {
            // ── 背景层：Live2D 模型渲染 ──
            // 视图创建后上报给 App 级（Live2DaiApp）持有；生命周期由 App 级统一管理。
            // 设置页打开时本界面仍存活，因此 Live2DView 可在设置页内热切换形象。
            AndroidView(
                factory = { ctx ->
                    Live2DView(ctx).also { view ->
                        // 启动时按持久化的 selectedModelId 加载模型；
                        // registry 解析失败 / 模型不存在时回退 getDefault()（硬编码 mao_pro）
                        val repo = SettingsRepository(ctx)
                        val registry = ModelRegistry.load(ctx)
                        val modelId = repo.selectedModelId
                        val entry = registry.getById(modelId) ?: registry.getDefault()
                        view.initialize("live2d/${entry.model3JsonPath}")
                        // ── 表情链路接线（修复 T11：LLM [emotion] 标签 → Live2D 表情切换）──
                        // ab59b45 插件化重构后 EmotionController 从未被实例化、onAssistantMessage 从未赋值，
                        // 导致 ChatService.runAfterReceive 解析出的 emotion 无法到达 Live2DView.setExpression。
                        // 在 Live2DView 创建后（onLive2DViewCreated 触发处）立即接线，视图生命周期内保持：
                        //   1) 核心 ChatHook：走 ChatService afterReceive 主链路（不依赖插件通道）
                        //   2) 插件通道 onAssistantMessage：供主动陪伴类插件（PluginHost.sendAssistantMessage）调用
                        // 注意：ChatService 持有 pluginManager.hooks 活引用，后注册的 hook 立即可见，无需重建 ChatService。
                        val emotionController = EmotionController(view)
                        pluginManager.onAssistantMessage = { text, emotion ->
                            emotion?.let { emotionController.setEmotion(it) }
                            // V1.1 模块1：主动陪伴类消息（PluginHost.sendAssistantMessage）
                            // 同样接入 TTS 播报；插件文本可能含 [emotion] 标签，播报前剥离
                            scope.launch {
                                voiceIo.speak(EmotionController.stripEmotionTags(text))
                            }
                        }
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
                                // assistantMessage 已剥离 [emotion] 标签，直接用解析出的关键词设置表情；
                                // emotion 为 null 时跳过，保持当前表情。
                                // V1.1 模块1 TTS 接线（plan §3.1.3 TTS 路径）：与表情切换同一 afterReceive 时机——
                                // 表情先切换（视觉反馈立即），TTS 随后播报剥离标签后的纯文本（音频延迟 1-3s）。
                                emotion?.let { emotionController.setEmotion(it) }
                                voiceIo.speak(assistantMessage)
                            }
                        })
                        // V1.1 模块4：LipSync 接线——TTS 播放状态回调（EdgeTtsService → VoiceIoController）
                        // 驱动嘴型开合；视图创建后注入，播放时协程动画、停止时归零。
                        voiceIo.attachLive2DView(view)
                        onLive2DViewCreated(view)
                    }
                },
                modifier = Modifier.fillMaxSize()
            )

            // ── 前景层：对话气泡列表 ──
            if (displayMessages.isNotEmpty()) {
                LazyColumn(
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(horizontal = 12.dp, vertical = 8.dp),
                    state = listState,
                    verticalArrangement = Arrangement.Bottom
                ) {
                    items(
                        items = displayMessages,
                        key = { "msg_${it.id}" }
                    ) { msg ->
                        ChatBubble(message = msg, onOpenSettings = onOpenSettings)
                        Spacer(Modifier.height(8.dp))
                    }
                }

                // 新消息时自动滚动到底部
                LaunchedEffect(displayMessages.size) {
                    if (displayMessages.isNotEmpty()) {
                        listState.animateScrollToItem(displayMessages.size - 1)
                    }
                }
            }
            }
        }
    }

    // ── 首次启动引导对话框（一次性、可跳过） ──
    // 设计要点：
    // - 零配置即聊：默认使用免费 GLM-4.7-Flash 共享额度（无需填 Key），与既有 banner 心智一致
    // - 免费共享额度一次讲清，不重复弹 banner：点「开始聊天」会同时标记免费额度提示已读
    // - 不申请任何权限：麦克风/悬浮窗功能尚未接线，等真正使用对应功能时再按需申请
    // - 可选「以后再说」：直接跳过，保留免费额度 banner 作为后续提醒；聊天全程可用
    if (showOnboarding) {
        AlertDialog(
            onDismissRequest = { showOnboarding = false },
            title = { Text("欢迎来到 Live2Dai 喵~ (=^・ω・^=)") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text("零配置开聊：不用填任何 API Key，默认使用免费的 GLM-4.7-Flash 共享额度（多人共用、限 1 并发）。")
                    Text("想更稳定/更高并发：在「设置 → 服务商」填你自己的 API Key 即可升级。")
                    Text("需要麦克风/悬浮窗等权限时，会在真正使用时单独询问，不会一次性轰炸你。")
                }
            },
            confirmButton = {
                TextButton(onClick = {
                    repo.onboardingCompleted = true
                    // 引导里已讲清免费共享额度，避免紧接着再弹 banner 造成重复打扰
                    repo.freeQuotaNoticeDismissed = true
                    showOnboarding = false
                }) {
                    Text("开始聊天")
                }
            },
            dismissButton = {
                TextButton(onClick = {
                    // 「以后再说」= 永久跳过引导弹窗（不再重复打扰），但不代替免费额度 banner 的提醒，
                    // 因此不设置 freeQuotaNoticeDismissed。
                    repo.onboardingCompleted = true
                    showOnboarding = false
                }) {
                    Text("以后再说")
                }
            }
        )
    }

    // ── 清空对话确认对话框（V1.1 模块2） ──
    // 破坏性操作：重置 LLM 上下文 + 清空显示消息 + 删除本地历史文件
    if (showClearConfirm) {
        AlertDialog(
            onDismissRequest = { showClearConfirm = false },
            title = { Text("清空对话") },
            text = { Text("将清除当前聊天记录（本地历史与 LLM 上下文），且不可恢复。确定清空吗？") },
            confirmButton = {
                TextButton(onClick = {
                    chatService.resetConversation()
                    displayMessages.clear()
                    scope.launch {
                        withContext(Dispatchers.IO) {
                            ChatHistoryStore.clear(context.applicationContext)
                        }
                    }
                    showClearConfirm = false
                }) {
                    Text("清空")
                }
            },
            dismissButton = {
                TextButton(onClick = { showClearConfirm = false }) {
                    Text("取消")
                }
            }
        )
    }

    // ── 危险工具确认对话框 ──
    // requiresConfirmation=true 的工具执行前必须经用户确认（ConfirmationGate 硬约束）：
    // ChatService 在 IO 协程挂起等待，这里弹 Compose AlertDialog，点击「同意/拒绝」后
    // 通过 respond() 恢复工具循环。
    confirmationGate.pending?.let { pending ->
        AlertDialog(
            onDismissRequest = { confirmationGate.respond(false) },
            title = { Text("确认执行") },
            text = {
                Text("工具「${pending.toolName}」请求执行以下操作：\n\n${pending.toolDescription}")
            },
            confirmButton = {
                TextButton(onClick = { confirmationGate.respond(true) }) {
                    Text("同意")
                }
            },
            dismissButton = {
                TextButton(onClick = { confirmationGate.respond(false) }) {
                    Text("拒绝")
                }
            }
        )
    }
}

/**
 * 单条对话气泡组件。
 * 用户消息 → 右侧蓝色；助手消息 → 左侧灰色（半透明）。
 * 额度耗尽提示气泡会额外展示「前往设置填写自己的 API Key」入口按钮。
 */
@Composable
fun ChatBubble(
    message: DisplayMessage,
    onOpenSettings: () -> Unit = {}
) {
    val alignment = if (message.isUser) Arrangement.End else Arrangement.Start
    val bubbleColor = when {
        message.isError -> MaterialTheme.colorScheme.errorContainer.copy(alpha = 0.85f)
        message.isUser -> MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.85f)
        else -> MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.80f)
    }
    val textColor = when {
        message.isError -> MaterialTheme.colorScheme.onErrorContainer
        message.isUser -> MaterialTheme.colorScheme.onPrimaryContainer
        else -> MaterialTheme.colorScheme.onSurfaceVariant
    }

    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = alignment
    ) {
        Column(
            modifier = Modifier.widthIn(max = 280.dp)
        ) {
            Text(
                text = message.content,
                color = textColor,
                fontSize = 15.sp,
                fontWeight = if (message.isError) FontWeight.Bold else FontWeight.Normal,
                modifier = Modifier
                    .clip(RoundedCornerShape(12.dp))
                    .background(bubbleColor)
                    .padding(horizontal = 14.dp, vertical = 10.dp)
            )
            if (message.showSettingsButton) {
                // 共享免费 Key 额度耗尽：直接跳转设置页（API Key 输入项位于设置页顶部）
                TextButton(
                    onClick = onOpenSettings,
                    modifier = Modifier.padding(top = 4.dp)
                ) {
                    Text("前往设置填写我的 API Key →", color = MaterialTheme.colorScheme.primary)
                }
            }
        }
    }
}
