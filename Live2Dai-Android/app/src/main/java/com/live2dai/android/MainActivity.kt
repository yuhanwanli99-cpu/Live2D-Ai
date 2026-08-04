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
 */
@Composable
fun Live2DaiApp() {
    var showSettings by remember { mutableStateOf(false) }

    if (showSettings) {
        SettingsScreen(onNavigateBack = { showSettings = false })
    } else {
        MainChatScreen(onOpenSettings = { showSettings = true })
    }
}

/**
 * 显示在气泡中的单条消息数据。
 */
data class DisplayMessage(
    val content: String,
    val isUser: Boolean,
    val isError: Boolean = false
)

/**
 * 主聊天界面。
 * 包含 Live2D 模型背景、对话气泡列表、和底部的输入栏。
 * 右上角齿轮按钮 → 设置页。
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MainChatScreen(onOpenSettings: () -> Unit) {
    val context = LocalContext.current

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

                                // 发起 DeepSeek 流式请求
                                scope.launch {
                                    try {
                                        chatService.sendMessage(text).collect { delta ->
                                            val lastIdx = displayMessages.lastIndex
                                            if (lastIdx >= 0 &&
                                                !displayMessages[lastIdx].isUser &&
                                                !displayMessages[lastIdx].isError
                                            ) {
                                                // 追加到已有的助手消息
                                                displayMessages[lastIdx] =
                                                    displayMessages[lastIdx].copy(
                                                        content = displayMessages[lastIdx].content + delta
                                                    )
                                            } else {
                                                // 新助手消息
                                                displayMessages.add(
                                                    DisplayMessage(
                                                        content = delta,
                                                        isUser = false
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
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
        ) {
            // ── 背景层：Live2D 模型渲染 ──
            val live2DView = remember { mutableStateOf<Live2DView?>(null) }
            val lifecycleOwner = LocalLifecycleOwner.current

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

            AndroidView(
                factory = { ctx ->
                    Live2DView(ctx).also { view ->
                        view.initialize("live2d/mao_pro/runtime/mao_pro.model3.json")
                        live2DView.value = view
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
                        ChatBubble(message = msg)
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
 */
@Composable
fun ChatBubble(message: DisplayMessage) {
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
        Text(
            text = message.content,
            color = textColor,
            fontSize = 15.sp,
            fontWeight = if (message.isError) FontWeight.Bold else FontWeight.Normal,
            modifier = Modifier
                .widthIn(max = 280.dp)
                .clip(RoundedCornerShape(12.dp))
                .background(bubbleColor)
                .padding(horizontal = 14.dp, vertical = 10.dp)
        )
    }
}
