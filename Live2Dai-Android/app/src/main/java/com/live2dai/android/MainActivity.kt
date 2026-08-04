package com.live2dai.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
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
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.live2dai.android.ui.settings.SettingsScreen
import com.live2dai.android.ui.theme.Live2DaiTheme
import com.live2dai.android.data.ModelRegistry
import com.live2dai.android.data.SettingsRepository
import com.live2dai.plugin.vision.VisionPlugin
import com.live2dai.plugin.vision.VisionPluginConfig
import kotlinx.coroutines.launch

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
 */
data class DisplayMessage(
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
                )
            )
        )
    }

    // ── 聊天状态 ──
    var inputText by remember { mutableStateOf("") }
    val displayMessages = remember { mutableStateListOf<DisplayMessage>() }
    var isLoading by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val listState = rememberLazyListState()

    // ── 首次启动引导（一次性、可跳过、不阻塞聊天） ──
    var showOnboarding by remember { mutableStateOf(!repo.onboardingCompleted) }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Live2Dai") },
                actions = {
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
                    FilledTonalButton(
                        onClick = {
                            val text = inputText.trim()
                            if (text.isNotEmpty() && !isLoading) {
                                // 显示用户消息
                                displayMessages.add(
                                    DisplayMessage(content = text, isUser = true)
                                )
                                inputText = ""
                                isLoading = true

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
                        key = { "msg_${it.hashCode()}_${it.content.length}" }
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
