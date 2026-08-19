package com.live2d.ai.android.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Mic
import androidx.compose.material.icons.filled.MicOff
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import com.live2d.ai.android.ConnectionState
import com.live2d.ai.android.Live2DView
import com.live2d.ai.android.PersonaConfig
import com.live2d.ai.android.VoiceInputState
import com.live2d.ai.android.connectionLabel
import com.live2d.ai.android.core.FpsOverlayState
import com.live2d.ai.android.core.LoopState
import com.live2d.ai.android.data.ModelRegistry
import com.live2d.ai.android.data.SettingsRepository
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.serialization.Serializable
import java.util.UUID

/**
 * 显示在气泡中的单条消息数据（保留，2C-4）。
 *
 * 2C-4 起 UI 气泡改由 [UiBubble] 承担；[DisplayMessage] 为旧 JSON 持久化模型
 * （Wave 5 随旧 Chat 链删除面移除——本类保留待后续清理）。
 *
 * [Serializable]：兼容旧序列化数据。
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

// ── Wave 4 · 4-1 核心环路消费面绑定（MainActivity → MainScreen，经 CompositionLocal 提供）──

/**
 * 核心环路消费面绑定（由 MainActivity 提供）：
 * - [onSubmit]：发送按钮 → 状态机 SubmitText（MainActivity 暴露的回调，经 LoopCoordinator.sendMessage）；
 * - [bubbles]：气泡列表 StateFlow（追加序 = 状态机 UI 命令尾插序）；
 * - [loopState]：状态机状态（SENDING/SPEAKING → isLoading → 禁输入）；
 * - [onLive2DViewWired]：Live2DView 实例上报（host.zeroMouth 目标 + VoiceIo LipSync 绑定）。
 */
class LoopWireBindings(
    val onSubmit: (String) -> Unit = {},
    val bubbles: StateFlow<List<UiBubble>>,
    val loopState: StateFlow<LoopState>,
    val onLive2DViewWired: (Live2DView) -> Unit = {},
    val voiceState: StateFlow<VoiceInputState> = MutableStateFlow(VoiceInputState.Idle),
    val onMicClick: () -> Unit = {},
    val isSpeaking: StateFlow<Boolean> = MutableStateFlow(false),
    val activeTtsProvider: StateFlow<String?> = MutableStateFlow(null),
)

/** 默认空绑定（未提供 LocalLoopWire 时：零行为，UI 保持 seam 静态）。 */
private val DefaultLoopWireBindings = LoopWireBindings(
    bubbles = MutableStateFlow(emptyList()),
    loopState = MutableStateFlow(LoopState.IDLE),
)

/** 核心环路消费面 CompositionLocal（MainActivity 在 setContent 内 provide）。 */
val LocalLoopWire = staticCompositionLocalOf { DefaultLoopWireBindings }

/**
 * 主界面（R15 布局 + 2C-4 UI 精简 + Wave 4 核心环路接线）。
 *
 * 布局（对齐 PC 端「只有对话框 + 设置」语义）：
 * - 层0：Live2D 角色渲染区**占满全屏**（GLSurfaceView 全屏，与角色交互/渲染管线无关，仅布局）
 * - 层1：右上角浮层控制（连接状态指示 + 设置入口，半透明悬浮在角色上）
 * - 层2：底部聊天浮层（半透明面板：消息气泡 + 输入框常驻底部；
 *   `imePadding` → 键盘弹出时聊天浮层整体上移，角色区不挤压——adjustNothing 已在 Manifest 锁定）
 *
 * 2C-4 UI 精简（删除面）：
 * - 已删除：ASR 麦克风按钮（isListening/CircularProgressIndicator 相关）、
 *   首次启动引导弹窗（showOnboarding/AlertDialog 块）、
 *   清空对话确认 + 危险工具确认门（showClearConfirm / confirmationGate 相关）、
 *   手势相关（重置交互入口本地引用等）。
 * - 保留：OutlinedTextField 输入框、发送按钮（uiCanSend 语义）、
 *   气泡列表（[MainScreenUiState.bubbles]）、设置入口（settingsVisible 开关）、连接状态显示。
 *
 * 状态来源：2C-4 seam [MainScreenUiState]（纯 JVM 可测，不含 isListening/onboarding/confirmation）。
 * Wave 4 起生产链路经 [LocalLoopWire] 接入：发送 → 状态机 SubmitText（MainActivity 回调）；
 * 气泡列表/ isLoading 由状态机驱动（SENDING/SPEAKING → 禁输入）。
 *
 * @param onOpenSettings 设置入口回调：通知父组件打开设置覆盖层
 * @param onLive2DViewCreated Live2DView 实例上报回调：父组件持有引用以支持生命周期管理和热切换
 */
@Composable
fun MainChatScreen(
    onOpenSettings: () -> Unit,
    onLive2DViewCreated: (Live2DView) -> Unit,
) {
    val uiState = remember { MainScreenUiState() }
    val listState = rememberLazyListState()

    // ── Wave 4：核心环路消费面（MainActivity 经 LocalLoopWire 提供）──
    // 气泡列表（追加序）与 isLoading（SENDING/SPEAKING → 禁输入）由状态机驱动。
    val loopWire = LocalLoopWire.current
    val bubbles by loopWire.bubbles.collectAsState(initial = emptyList())
    val loopState by loopWire.loopState.collectAsState(initial = LoopState.IDLE)
    val voiceState by loopWire.voiceState.collectAsState(initial = VoiceInputState.Idle)
    val isSpeaking by loopWire.isSpeaking.collectAsState(initial = false)
    val activeTtsProvider by loopWire.activeTtsProvider.collectAsState(initial = null)
    val isLoading = loopState != LoopState.IDLE

    // 同步到 seam uiState（消费面）：气泡追加序（uiSetBubbles 保序尾插）+ isLoading
    LaunchedEffect(bubbles) { uiState.uiSetBubbles(bubbles) }
    LaunchedEffect(isLoading) { uiState.isLoading = isLoading }

    // ── FPS 角标（用户需求①）：每 1s 读一次 FpsOverlayState.currentFps 触发重组 ──
    // GL 线程（Live2DRenderer.onDrawFrame）每秒更新 currentFps；此处循环 delay(1000)
    // 刷新本地 state → 角标文本每秒更新一次（首秒 currentFps=0 → 显示 "FPS: 0"）。
    var fps by remember { mutableStateOf(FpsOverlayState.currentFps) }
    LaunchedEffect(Unit) {
        while (true) {
            fps = FpsOverlayState.currentFps
            delay(1000)
        }
    }

    // ── 新消息时自动滚动到底部 ──
    LaunchedEffect(bubbles.size) {
        if (bubbles.isNotEmpty()) {
            listState.animateScrollToItem(bubbles.size - 1)
        }
    }

    // ── R15 主结构：角色全屏 + 聊天浮层（Box 层叠，角色区不被任何面板挤压） ──
    Box(
        modifier = Modifier.fillMaxSize()
    ) {
        // ── 层0：Live2D 角色渲染（全屏背景层） ──
        // 视图创建后上报 App 级（Live2DAiApp 持有，支持生命周期 + 热切换）
        AndroidView(
            factory = { ctx ->
                Live2DView(ctx).also { view ->
                    // 启动时按持久化的 selectedModelId 加载模型；
                    // registry 解析失败 / 模型不存在时回退 getDefault()（硬编码 bai）
                    val ctxRepo = SettingsRepository(ctx)
                    val registry = ModelRegistry.load(ctx)
                    val modelId = ctxRepo.selectedModelId
                    val entry = registry.getById(modelId) ?: registry.getDefault()
                    view.initialize("live2d/${entry.model3JsonPath}", entry.id)
                    // 实例上报 → App 级（Live2DAiApp 持有，支持生命周期与热切换）
                    onLive2DViewCreated(view)
                    // Wave 4：核心环路视图绑定（host.zeroMouth 目标 + VoiceIo LipSync 绑定；
                    // 旧业务接线随旧 Chat 链删除面已移除）
                    loopWire.onLive2DViewWired(view)
                }
            },
            modifier = Modifier.fillMaxSize()
        )

        // ── 层0.5：左上角 FPS 角标（用户需求①） ──
        // 显示 "FPS: n"（每秒刷新，数据源 = FpsOverlayState，GL 线程写）。
        // 开关落点说明（任务③ fallback 分支）：SettingsScreen 不在本任务文件锁内
        // （只允许动 Live2DRenderer.kt / core/FpsOverlayState.kt / MainScreen.kt /
        // MainScreenUiState.kt），设置面板无法落 Switch → 按任务预案把开关做成徽章旁的
        // 小图标（Visibility/VisibilityOff）切换 fpsOverlayVisible，零改动设置页。
        Row(
            modifier = Modifier
                .align(Alignment.TopStart)
                .statusBarsPadding()
                .padding(12.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            if (uiState.fpsOverlayVisible) {
                Surface(
                    shape = RoundedCornerShape(16.dp),
                    color = MaterialTheme.colorScheme.surface.copy(alpha = 0.72f)
                ) {
                    Text(
                        text = "FPS: $fps",
                        style = MaterialTheme.typography.labelSmall,
                        modifier = Modifier.padding(horizontal = 10.dp, vertical = 6.dp)
                    )
                }
                Spacer(Modifier.width(4.dp))
            }
            Surface(
                shape = RoundedCornerShape(20.dp),
                color = MaterialTheme.colorScheme.surface.copy(alpha = 0.72f)
            ) {
                IconButton(onClick = { uiState.fpsOverlayVisible = !uiState.fpsOverlayVisible }) {
                    Icon(
                        imageVector = if (uiState.fpsOverlayVisible) Icons.Default.VisibilityOff else Icons.Default.Visibility,
                        contentDescription = if (uiState.fpsOverlayVisible) "隐藏 FPS" else "显示 FPS",
                        modifier = Modifier.size(16.dp)
                    )
                }
            }
        }

        // ── 层1：右上角浮层控制（半透明悬浮在角色上） ──
        // 2C-4 精简后仅保留：连接状态指示 + 设置入口（需求2 唯一入口）。
        // 已删除：清空对话按钮（其确认流程依赖旧 Chat 链，Wave 4 迁移）。
        // Android 15 强制 edge-to-edge：statusBarsPadding 将控制条压到状态栏之下。
        Row(
            modifier = Modifier
                .align(Alignment.TopEnd)
                .statusBarsPadding()
                .padding(12.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            // P1: TTS 播放状态（说话中/引擎）
            Surface(
                shape = RoundedCornerShape(16.dp),
                color = MaterialTheme.colorScheme.surface.copy(alpha = 0.72f)
            ) {
                Text(
                    text = when {
                        isSpeaking -> "🔊 ${activeTtsProvider ?: "TTS"}"
                        activeTtsProvider != null -> "TTS: $activeTtsProvider"
                        else -> "TTS: -"
                    },
                    style = MaterialTheme.typography.labelSmall,
                    modifier = Modifier.padding(horizontal = 10.dp, vertical = 6.dp)
                )
            }
            Spacer(Modifier.width(8.dp))

            // 连接状态指示（连接/断开时文案即时更新；2C-4 状态源 = MainScreenUiState.connectionState）
            Surface(
                shape = RoundedCornerShape(16.dp),
                color = MaterialTheme.colorScheme.surface.copy(alpha = 0.72f)
            ) {
                Text(
                    text = connectionLabel(uiState.connectionState),
                    style = MaterialTheme.typography.labelSmall,
                    color = when (uiState.connectionState) {
                        ConnectionState.Connected -> MaterialTheme.colorScheme.primary
                        ConnectionState.Connecting -> MaterialTheme.colorScheme.onSurfaceVariant
                        ConnectionState.Disconnected -> MaterialTheme.colorScheme.error
                    },
                    modifier = Modifier.padding(horizontal = 10.dp, vertical = 6.dp)
                )
            }

            Spacer(Modifier.width(8.dp))

            // 设置入口（需求2：唯一保留的功能入口之一；2C-4 同时翻转 settingsVisible → AppRoot 装配）
            Surface(
                shape = RoundedCornerShape(20.dp),
                color = MaterialTheme.colorScheme.surface.copy(alpha = 0.72f)
            ) {
                IconButton(onClick = {
                    uiState.uiSetSettingsVisible(true)
                    onOpenSettings()
                }) {
                    Icon(
                        Icons.Default.Settings,
                        contentDescription = "设置",
                        modifier = Modifier.size(18.dp)
                    )
                }
            }
        }

        // ── 层2：底部聊天浮层（半透明面板浮在角色上；键盘弹出整体上移，角色区不挤压） ──
        // navigationBarsPadding：edge-to-edge 下避开系统手势条；
        // imePadding：键盘弹出时浮层整体上移（adjustNothing 角色区不动）。
        Column(
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .fillMaxWidth()
                .navigationBarsPadding()
                .imePadding()
        ) {
            // ── 对话气泡列表（半透明浮层，上限高度避免盖住角色主体） ──
            // Wave 4：数据源 = 状态机 UI 命令追加的气泡流（追加序；同步到 uiState.bubbles）
            if (bubbles.isNotEmpty()) {
                LazyColumn(
                    modifier = Modifier
                        .fillMaxWidth()
                        .heightIn(max = 340.dp)
                        .padding(horizontal = 12.dp),
                    state = listState,
                    verticalArrangement = Arrangement.Bottom
                ) {
                    items(bubbles) { bubble ->
                        ChatBubble(bubble = bubble, onOpenSettings = onOpenSettings)
                        Spacer(Modifier.height(8.dp))
                    }
                }
            }

            // ── 输入栏（常驻底部，半透明浮层） ──
            Surface(
                modifier = Modifier.fillMaxWidth(),
                color = MaterialTheme.colorScheme.surface.copy(alpha = 0.88f),
                tonalElevation = 3.dp
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    IconButton(
                        onClick = { loopWire.onMicClick() },
                        enabled = !isLoading
                    ) {
                        Icon(
                            imageVector = if (voiceState == VoiceInputState.Listening || voiceState == VoiceInputState.Recognizing) Icons.Default.MicOff else Icons.Default.Mic,
                            contentDescription = if (voiceState == VoiceInputState.Listening) "停止录音" else "语音输入",
                            tint = if (voiceState == VoiceInputState.Listening) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.size(24.dp)
                        )
                    }
                    Spacer(Modifier.width(4.dp))
                    OutlinedTextField(
                        value = uiState.inputText,
                        onValueChange = { if (!isLoading) uiState.inputText = it },
                        // M4d：placeholder 随人设联动（PersonaConfig 单一数据源驱动，勿硬编码人名）
                        placeholder = { Text("和${PersonaConfig.getPersonaName()}聊天...") },
                        modifier = Modifier.weight(1f),
                        singleLine = true,
                        enabled = !isLoading
                    )
                    Spacer(Modifier.width(8.dp))
                    FilledTonalButton(
                        onClick = {
                            val text = uiState.inputText.trim()
                            if (text.isNotEmpty() && !isLoading) {
                                // Wave 4：状态机 SubmitText（MainActivity 暴露的回调 →
                                // coordinator.sendMessage；用户气泡由状态机 UiAddUserBubble 命令追加）
                                loopWire.onSubmit(text)
                                uiState.inputText = ""
                            }
                        },
                        enabled = uiState.inputText.isNotBlank() && !isLoading
                    ) {
                        Text(if (isLoading) "..." else "发送")
                    }
                }
            }
        }
    }
}

/**
 * 单条对话气泡组件（2C-4 起渲染 [UiBubble]）。
 *
 * 用户消息 → 右侧蓝色；助手消息 → 左侧灰色（半透明）；错误 → 左侧错误色（加粗）。
 *
 * @param bubble 要显示的气泡（User / Assistant / Error）
 * @param onOpenSettings 设置页入口回调（签名保留，Wave 4 复用）
 */
@Composable
fun ChatBubble(
    bubble: UiBubble,
    onOpenSettings: () -> Unit = {},
) {
    val alignment = if (bubble is UiBubble.User) Arrangement.End else Arrangement.Start
    val bubbleColor = when (bubble) {
        is UiBubble.Error -> MaterialTheme.colorScheme.errorContainer.copy(alpha = 0.85f)
        is UiBubble.User -> MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.85f)
        is UiBubble.Assistant -> MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.80f)
    }
    val textColor = when (bubble) {
        is UiBubble.Error -> MaterialTheme.colorScheme.onErrorContainer
        is UiBubble.User -> MaterialTheme.colorScheme.onPrimaryContainer
        is UiBubble.Assistant -> MaterialTheme.colorScheme.onSurfaceVariant
    }
    val text = when (bubble) {
        is UiBubble.User -> bubble.text
        is UiBubble.Assistant -> bubble.text
        is UiBubble.Error -> bubble.text
    }

    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = alignment
    ) {
        Column(
            modifier = Modifier.widthIn(max = 280.dp)
        ) {
            Text(
                text = text,
                color = textColor,
                fontSize = 15.sp,
                fontWeight = if (bubble is UiBubble.Error) FontWeight.Bold else FontWeight.Normal,
                modifier = Modifier
                    .clip(RoundedCornerShape(12.dp))
                    .background(bubbleColor)
                    .padding(horizontal = 14.dp, vertical = 10.dp)
            )
        }
    }
}
