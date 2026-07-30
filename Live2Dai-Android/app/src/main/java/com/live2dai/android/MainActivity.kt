package com.live2dai.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import com.live2dai.android.ui.SettingsScreen
import kotlinx.coroutines.*

class MainActivity : ComponentActivity() {

    private lateinit var chatService: ChatService
    private lateinit var ttsService: EdgeTtsService
    private lateinit var voiceInputService: VoiceInputService
    private lateinit var live2DView: Live2DView
    private lateinit var emotionController: EmotionController

    // LipSync 协程控制
    private var lipSyncJob: Job? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // 从 assets 加载人设配置
        PersonaConfig.load(this)

        // 初始化 LLMProviderManager（必须在 ChatService 之前）
        LLMProviderManager.init(this)

        // 初始化服务（使用动态 Provider）
        chatService = ChatService(LLMProviderManager.getActiveProvider())
        ttsService = EdgeTtsService(this)
        voiceInputService = VoiceInputService(this)

        setContent {
            Live2DaiTheme {
                var showSettings by remember { mutableStateOf(false) }

                if (showSettings) {
                    SettingsScreen(
                        onBack = {
                            showSettings = false
                            // 返回主界面时重建 ChatService 以应用 Provider 切换
                            chatService = ChatService(LLMProviderManager.getActiveProvider())
                        }
                    )
                } else {
                    MainScreen(
                        chatService = chatService,
                        ttsService = ttsService,
                        voiceInputService = voiceInputService,
                        onLive2DViewCreated = { view ->
                            live2DView = view
                            emotionController = EmotionController(live2DView)
                        },
                        onOpenSettings = { showSettings = true },
                        /** 处理完整回复（Activity 层，可访问 Live2DView） */
                        onProcessResponse = { rawText ->
                            // 1. 解析表情标签，设置 Live2D 表情
                            val cleanText = emotionController.parseAndApply(rawText)
                            // 2. TTS 播放 + LipSync
                            if (cleanText.isNotEmpty()) {
                                startLipSync()
                                try {
                                    ttsService.speak(cleanText)
                                } catch (e: Exception) {
                                    android.util.Log.w("MainActivity", "TTS 播放失败: ${e.message}")
                                }
                                stopLipSync()
                                live2DView.setLipSyncParam(0f)
                            }
                            // 3. 返回纯净文本用于 UI 显示
                            cleanText
                        }
                    )
                }
            }
        }
    }

    /**
     * 启动简单的 LipSync
     *
     * 在 TTS 播放期间，周期性改变 ParamA 值来模拟说话口型。
     * 使用正弦波 + 随机抖动模拟自然的嘴部运动。
     */
    private fun startLipSync() {
        lipSyncJob?.cancel()
        lipSyncJob = CoroutineScope(Dispatchers.Main).launch {
            var phase = 0f
            while (isActive) {
                // 正弦波 (0~1) + 随机抖动
                val smooth = ((kotlin.math.sin(phase) + 1f) / 2f)
                val jitter = (Math.random().toFloat() * 0.2f)
                val value = (smooth * 0.8f + jitter * 0.2f).coerceIn(0.1f, 0.9f)
                live2DView.setLipSyncParam(value)
                phase += 0.5f
                delay(80L) // ~12.5 次/秒
            }
        }
    }

    /**
     * 停止 LipSync
     */
    private fun stopLipSync() {
        lipSyncJob?.cancel()
        lipSyncJob = null
    }

    override fun onDestroy() {
        lipSyncJob?.cancel()
        ttsService.release()
        voiceInputService.stopListening()
        if (::live2DView.isInitialized) {
            live2DView.dispose()
        }
        super.onDestroy()
    }
}

@Composable
fun Live2DaiTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = darkColorScheme(
            primary = Color(0xFF9C7CF4),
            secondary = Color(0xFFFF88AA),
            surface = Color(0xFF1A1A2E),
            background = Color(0xFF0F0F23),
            onPrimary = Color.White,
            onSurface = Color.White,
            onBackground = Color.White
        )
    ) {
        content()
    }
}

data class ChatBubble(
    val text: String,
    val isUser: Boolean
)

@Composable
fun MainScreen(
    chatService: ChatService,
    ttsService: EdgeTtsService,
    voiceInputService: VoiceInputService,
    onLive2DViewCreated: (Live2DView) -> Unit = {},
    /** 打开 LLM 设置界面 */
    onOpenSettings: () -> Unit = {},
    /** 处理完整回复: 输入带表情标签的原始文本，返回纯净文本（Activity 中实现表情+TTS） */
    onProcessResponse: suspend (String) -> String = { it }
) {
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    val messages = remember { mutableStateListOf<ChatBubble>() }
    var inputText by remember { mutableStateOf("") }
    var isListening by remember { mutableStateOf(false) }
    var isSpeaking by remember { mutableStateOf(false) }
    var streamingText by remember { mutableStateOf("") }
    val listState = rememberLazyListState()

    // ── Persona 切换状态 ──
    var showPersonaMenu by remember { mutableStateOf(false) }
    var availablePersonas by remember { mutableStateOf(listOf<Pair<String, String>>()) }

    // 首次加载时获取可用人设列表
    LaunchedEffect(Unit) {
        availablePersonas = PersonaConfig.getAvailablePersonas(context)
    }

    // Auto-scroll to bottom when new messages arrive
    LaunchedEffect(messages.size, streamingText) {
        if (messages.isNotEmpty() || streamingText.isNotEmpty()) {
            listState.animateScrollToItem(messages.size)
        }
    }

    // 发送消息
    val sendMessage: suspend (String) -> Unit = { text ->
        messages.add(ChatBubble(text = text, isUser = true))
        streamingText = ""

        try {
            // 降级检查：若当前 Provider 是 LOCAL，先做健康检查
            val provider = LLMProviderManager.getActiveProvider()
            if (provider.type == ProviderType.LOCAL) {
                val health = LLMProviderManager.healthCheck(provider.baseUrl)
                if (health !is HealthResult.Ok) {
                    val reason = (health as? HealthResult.Error)?.message ?: "连接失败"
                    android.widget.Toast.makeText(
                        context,
                        "本地 LLM 不可用 ($reason)，已自动切换至云端 DeepSeek",
                        android.widget.Toast.LENGTH_SHORT
                    ).show()
                    val fallbackProvider = LLMProviderManager.getDeepSeekProvider()
                    val tempService = ChatService(fallbackProvider)
                    val rawBuilder = StringBuilder()
                    tempService.sendMessage(text).collect { delta ->
                        rawBuilder.append(delta)
                        streamingText = EmotionController.stripEmotionTags(rawBuilder.toString())
                    }
                    val rawResponse = rawBuilder.toString().trim()
                    streamingText = ""
                    if (rawResponse.isNotEmpty()) {
                        isSpeaking = true
                        val cleanText = onProcessResponse(rawResponse)
                        if (cleanText.isNotEmpty()) {
                            messages.add(ChatBubble(text = cleanText, isUser = false))
                        }
                        isSpeaking = false
                    }
                    return@sendMessage
                }
            }

            // 收集流式响应（原始文本，可能含表情标签）
            val rawBuilder = StringBuilder()
            chatService.sendMessage(text).collect { delta ->
                rawBuilder.append(delta)
                // 在流式显示时即剥离表情标签，让用户看不到 [emotion] 标记
                streamingText = EmotionController.stripEmotionTags(rawBuilder.toString())
            }

            val rawResponse = rawBuilder.toString().trim()
            streamingText = ""

            if (rawResponse.isNotEmpty()) {
                isSpeaking = true

                // Activity 层处理: 解析表情 → 设置表情 → TTS + LipSync → 返回纯净文本
                val cleanText = onProcessResponse(rawResponse)

                // 显示纯净文本气泡
                if (cleanText.isNotEmpty()) {
                    messages.add(ChatBubble(text = cleanText, isUser = false))
                }

                isSpeaking = false
            }
        } catch (e: Exception) {
            android.util.Log.e("MainActivity", "消息处理失败", e)
            streamingText = ""
            messages.add(
                ChatBubble(
                    text = "呜...主人，小喵好像遇到问题了 (${e.message ?: "未知错误"})\n请检查网络连接或稍后再试喵~",
                    isUser = false
                )
            )
        }
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.background)
    ) {
        Column(modifier = Modifier.fillMaxSize()) {
            // Live2D View (顶部 35%)
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(0.35f)
                    .background(Color(0xFF16213E))
            ) {
                AndroidView(
                    factory = { ctx ->
                        Live2DView(ctx).apply {
                            // 修复: 使用正确路径（不含 "assets/" 前缀，使用 model3.json）
                            initialize("live2d/mao_pro/runtime/mao_pro.model3.json")
                            onLive2DViewCreated(this)
                        }
                    },
                    modifier = Modifier.fillMaxSize()
                )

                // 状态叠加层 — 包含人设名称 + 选择器 + 设置按钮
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .align(Alignment.TopStart)
                        .padding(12.dp),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.Top
                ) {
                    Column {
                        // 人设名称（可点击打开下拉选择器）
                        Text(
                            text = PersonaConfig.getPersonaName(),
                            color = Color.White,
                            fontSize = 14.sp,
                            fontWeight = FontWeight.Bold,
                            modifier = Modifier.clickable { showPersonaMenu = true }
                        )

                        // 人设切换下拉菜单
                        DropdownMenu(
                            expanded = showPersonaMenu,
                            onDismissRequest = { showPersonaMenu = false }
                        ) {
                            availablePersonas.forEach { (displayName, filePath) ->
                                val isCurrent = filePath == PersonaConfig.getCurrentPersonaFile()
                                DropdownMenuItem(
                                    text = {
                                        Text(
                                            text = displayName,
                                            fontWeight = if (isCurrent) FontWeight.Bold else FontWeight.Normal
                                        )
                                    },
                                    onClick = {
                                        showPersonaMenu = false
                                        if (filePath != PersonaConfig.getCurrentPersonaFile()) {
                                            // 切换人设 → 重新加载配置 + 重置对话
                                            PersonaConfig.loadFile(context, filePath)
                                            chatService.resetConversation()
                                            messages.clear()
                                        }
                                    }
                                )
                            }
                        }

                        if (isSpeaking) {
                            Text(
                                text = "正在说话...",
                                color = MaterialTheme.colorScheme.secondary,
                                fontSize = 12.sp
                            )
                        }
                    }

                    // 设置齿轮按钮
                    IconButton(
                        onClick = onOpenSettings,
                        modifier = Modifier.size(36.dp)
                    ) {
                        Text(
                            text = "⚙",
                            fontSize = 20.sp,
                            color = Color.White.copy(alpha = 0.7f)
                        )
                    }
                }
            }

            // 聊天气泡区域
            LazyColumn(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(0.45f)
                    .padding(horizontal = 12.dp, vertical = 8.dp),
                state = listState
            ) {
                items(messages) { bubble ->
                    ChatBubbleView(bubble)
                }

                // 显示流式响应
                if (streamingText.isNotEmpty()) {
                    item {
                        ChatBubbleView(
                            ChatBubble(text = streamingText, isUser = false)
                        )
                    }
                }
            }

            // 底部输入区域
            Surface(
                modifier = Modifier.fillMaxWidth(),
                color = MaterialTheme.colorScheme.surface,
                tonalElevation = 8.dp
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(12.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    // 语音输入按钮
                    FilledIconButton(
                        onClick = {
                            if (!isListening) {
                                isListening = true
                                scope.launch {
                                    val text = voiceInputService.listen()
                                    isListening = false
                                    if (text.isNotEmpty()) {
                                        inputText = ""
                                        sendMessage(text)
                                    }
                                }
                            } else {
                                voiceInputService.stopListening()
                                isListening = false
                            }
                        },
                        modifier = Modifier.size(48.dp),
                        shape = CircleShape,
                        colors = IconButtonDefaults.filledIconButtonColors(
                            containerColor = if (isListening)
                                MaterialTheme.colorScheme.error
                            else
                                MaterialTheme.colorScheme.primary
                        )
                    ) {
                        Text(
                            text = if (isListening) "■" else "🎤",
                            fontSize = 18.sp
                        )
                    }

                    Spacer(modifier = Modifier.width(8.dp))

                    // 文本输入框
                    OutlinedTextField(
                        value = inputText,
                        onValueChange = { inputText = it },
                        modifier = Modifier.weight(1f),
                        placeholder = { Text("给主人发消息...") },
                        colors = OutlinedTextFieldDefaults.colors(
                            focusedBorderColor = MaterialTheme.colorScheme.primary,
                            unfocusedBorderColor = Color.Gray
                        ),
                        maxLines = 3
                    )

                    Spacer(modifier = Modifier.width(8.dp))

                    // 清空对话按钮
                    FilledIconButton(
                        onClick = {
                            chatService.resetConversation()
                            messages.clear()
                        },
                        modifier = Modifier.size(48.dp),
                        shape = CircleShape,
                        colors = IconButtonDefaults.filledIconButtonColors(
                            containerColor = Color(0xFF555555)
                        )
                    ) {
                        Text("🗑", fontSize = 16.sp)
                    }

                    Spacer(modifier = Modifier.width(8.dp))

                    // 发送按钮
                    IconButton(
                        onClick = {
                            if (inputText.isNotBlank()) {
                                val text = inputText.trim()
                                inputText = ""
                                scope.launch {
                                    sendMessage(text)
                                }
                            }
                        },
                        modifier = Modifier
                            .size(48.dp)
                            .background(
                                MaterialTheme.colorScheme.primary,
                                CircleShape
                            ),
                    ) {
                        Text("→", fontSize = 18.sp, color = MaterialTheme.colorScheme.onPrimary)
                    }
                }
            }
        }
    }
}

@Composable
fun ChatBubbleView(bubble: ChatBubble) {
    val bgColor = if (bubble.isUser)
        MaterialTheme.colorScheme.primary
    else
        MaterialTheme.colorScheme.surfaceVariant

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp),
        horizontalAlignment = if (bubble.isUser) Alignment.End else Alignment.Start
    ) {
        Surface(
            shape = RoundedCornerShape(
                topStart = 16.dp,
                topEnd = 16.dp,
                bottomStart = if (bubble.isUser) 16.dp else 4.dp,
                bottomEnd = if (bubble.isUser) 4.dp else 16.dp
            ),
            color = bgColor
        ) {
            Text(
                text = bubble.text,
                modifier = Modifier.padding(12.dp),
                color = if (bubble.isUser) Color.White else MaterialTheme.colorScheme.onSurface,
                fontSize = 15.sp
            )
        }
    }
}
