package com.live2dai.android.ui

import android.Manifest
import android.content.pm.PackageManager
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
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
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Mic
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.live2dai.android.ChatViewModel
import com.live2dai.android.ConnectionState
import com.live2dai.android.Live2DView
import com.live2dai.android.PersonaConfig
import com.live2dai.android.connectionLabel
import com.live2dai.android.data.ModelRegistry
import com.live2dai.android.data.SettingsRepository
import kotlinx.serialization.Serializable
import java.util.UUID

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
 * 主界面（R15 布局大改造：角色全屏 + 聊天浮层 + UI 简化）。
 *
 * 布局（对齐 PC 端「只有对话框 + 设置」语义）：
 * - 层0：Live2D 角色渲染区**占满全屏**（GLSurfaceView 全屏，与角色交互/渲染管线无关，仅布局）
 * - 层1：右上角浮层控制（连接状态指示 + 清空对话 + 设置入口，半透明悬浮在角色上）
 * - 层2：底部聊天浮层（半透明面板：消息气泡 + 输入框常驻底部；
 *   `imePadding` → 键盘弹出时聊天浮层整体上移，角色区不挤压——adjustNothing 已在 Manifest 锁定）
 *
 * R15 UI 简化（需求2）：只保留「对话框（聊天面板）+ 设置（设置入口）」；
 * 已移除：定时提醒入口（需求3：proactiveEnabled 默认 false，无 UI 入口）、
 * 重置模型位置按钮（杂项功能按钮）等。
 *
 * 所有业务逻辑委托给 [ChatViewModel]，Composable 层只保留：
 * - 权限启动器（麦克风）——需要 Compose 上下文
 * - AndroidView 工厂（Live2DView 创建与初始化）
 * - Box / Surface / LazyColumn / AlertDialog 等纯 UI 组件
 * - Toast 用户提示
 *
 * @param onOpenSettings 设置入口回调：通知父组件打开设置覆盖层
 * @param onLive2DViewCreated Live2DView 实例上报回调：父组件持有引用以支持生命周期管理和热切换
 */
@Composable
fun MainChatScreen(
    onOpenSettings: () -> Unit,
    onLive2DViewCreated: (Live2DView) -> Unit,
) {
    val context = LocalContext.current
    val repo = remember { SettingsRepository(context) }
    val viewModel: ChatViewModel = viewModel()
    val listState = rememberLazyListState()

    // C-M5 重置入口能力保留（Live2DView.resetUserInteraction 仍可用）；R15 UI 简化
    // 已移除主界面「重置模型位置」按钮（杂项功能按钮，只留对话框+设置）
    val live2DView = remember { mutableStateOf<Live2DView?>(null) }

    // ── 收集 ViewModel 状态（StateFlow → Compose 状态，随生命周期暂停/恢复） ──
    val displayMessages by viewModel.displayMessages.collectAsStateWithLifecycle()
    val inputText by viewModel.inputText.collectAsStateWithLifecycle()
    val isLoading by viewModel.isLoading.collectAsStateWithLifecycle()
    val isListening by viewModel.isListening.collectAsStateWithLifecycle()
    val showOnboarding by viewModel.showOnboarding.collectAsStateWithLifecycle()
    val showClearConfirm by viewModel.showClearConfirm.collectAsStateWithLifecycle()
    // R1-ui M4b：连接状态（对齐 PC chat-header）——ChatService 请求生命周期驱动
    val connectionState by viewModel.connectionState.collectAsStateWithLifecycle()

    // ── 麦克风权限按需申请（架构约束 §8.2-3）：仅点击麦克风按钮时请求，不在启动时批量申请 ──
    val requestMicPermission = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted ->
        if (granted) {
            viewModel.startListening(repo.asrProviderId)
            Toast.makeText(context, "正在聆听，请说话...", Toast.LENGTH_SHORT).show()
        } else {
            Toast.makeText(context, "需要麦克风权限才能语音输入", Toast.LENGTH_SHORT).show()
        }
    }

    val onMicClick: () -> Unit = {
        if (ContextCompat.checkSelfPermission(context, Manifest.permission.RECORD_AUDIO) ==
            PackageManager.PERMISSION_GRANTED
        ) {
            viewModel.startListening(repo.asrProviderId)
            Toast.makeText(context, "正在聆听，请说话...", Toast.LENGTH_SHORT).show()
        } else {
            requestMicPermission.launch(Manifest.permission.RECORD_AUDIO)
        }
    }

    // ── 新消息时自动滚动到底部 ──
    LaunchedEffect(displayMessages.size) {
        if (displayMessages.isNotEmpty()) {
            listState.animateScrollToItem(displayMessages.size - 1)
        }
    }

    // ── R15 主结构：角色全屏 + 聊天浮层（Box 层叠，角色区不被任何面板挤压） ──
    Box(
        modifier = Modifier.fillMaxSize()
    ) {
        // ── 层0：Live2D 角色渲染（全屏背景层） ──
        // 视图创建后上报 ViewModel（表情/LipSync 接线）和 App 级（生命周期 + 热切换）
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
                    // 业务接线（EmotionController / ChatHook / LipSync）→ ViewModel
                    viewModel.onLive2DViewCreated(view)
                    // 实例上报 → App 级（Live2DaiApp 持有，支持生命周期与热切换）
                    onLive2DViewCreated(view)
                    // 重置入口本地引用（与 App 级引用同一实例）
                    live2DView.value = view
                }
            },
            modifier = Modifier.fillMaxSize()
        )

        // ── 层1：右上角浮层控制（半透明悬浮在角色上） ──
        // R15 保留：连接状态指示（R1-ui M4b，对齐 PC chat-header）+ 清空对话（对话框辅助）
        //          + 设置入口（需求2 唯一入口）。全部半透明，不遮挡角色主体。
        // Android 15 强制 edge-to-edge：statusBarsPadding 将控制条压到状态栏之下。
        Row(
            modifier = Modifier
                .align(Alignment.TopEnd)
                .statusBarsPadding()
                .padding(12.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            // 连接状态指示（连接/断开时文案即时更新）
            Surface(
                shape = RoundedCornerShape(16.dp),
                color = MaterialTheme.colorScheme.surface.copy(alpha = 0.72f)
            ) {
                Text(
                    text = connectionLabel(connectionState),
                    style = MaterialTheme.typography.labelSmall,
                    color = when (connectionState) {
                        ConnectionState.Connected -> MaterialTheme.colorScheme.primary
                        ConnectionState.Connecting -> MaterialTheme.colorScheme.onSurfaceVariant
                        ConnectionState.Disconnected -> MaterialTheme.colorScheme.error
                    },
                    modifier = Modifier.padding(horizontal = 10.dp, vertical = 6.dp)
                )
            }

            Spacer(Modifier.width(8.dp))

            // 清空对话（对话框辅助功能）
            Surface(
                shape = RoundedCornerShape(20.dp),
                color = MaterialTheme.colorScheme.surface.copy(alpha = 0.72f)
            ) {
                IconButton(onClick = { viewModel.showClearConfirm.value = true }) {
                    Icon(
                        Icons.Default.Delete,
                        contentDescription = "清空对话",
                        modifier = Modifier.size(18.dp)
                    )
                }
            }

            Spacer(Modifier.width(8.dp))

            // 设置入口（需求2：唯一保留的功能入口之一）
            Surface(
                shape = RoundedCornerShape(20.dp),
                color = MaterialTheme.colorScheme.surface.copy(alpha = 0.72f)
            ) {
                IconButton(onClick = onOpenSettings) {
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
            if (displayMessages.isNotEmpty()) {
                LazyColumn(
                    modifier = Modifier
                        .fillMaxWidth()
                        .heightIn(max = 340.dp)
                        .padding(horizontal = 12.dp),
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
                    OutlinedTextField(
                        value = inputText,
                        onValueChange = { if (!isLoading) viewModel.inputText.value = it },
                        // M4d：placeholder 随人设联动（PersonaConfig 单一数据源驱动，勿硬编码人名）
                        placeholder = { Text("和${PersonaConfig.getPersonaName()}聊天...") },
                        modifier = Modifier.weight(1f),
                        singleLine = true,
                        enabled = !isLoading
                    )
                    Spacer(Modifier.width(8.dp))
                    // 语音输入（ASR）麦克风按钮
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
                                viewModel.sendMessage(text)
                            }
                        },
                        enabled = inputText.isNotBlank() && !isLoading
                    ) {
                        Text(if (isLoading) "..." else "发送")
                    }
                }
            }
        }
    }

    // ── 首次启动引导对话框（一次性、可跳过） ──
    // 设计要点：
    // - 默认 DeepSeek 云端：首次使用引导用户配置自己的 DeepSeek API Key
    // - 点「开始聊天」会标记引导完成（不再弹出）
    // - 「以后再说」：直接跳过，下次启动再次展示
    if (showOnboarding) {
        AlertDialog(
            onDismissRequest = { viewModel.dismissOnboarding() },
            title = { Text("欢迎来到 Live2Dai 喵~ (=^・ω・^=)") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text("在「设置 → API 设置」填写你自己的 DeepSeek API Key 即可开始聊天（密钥只保存在本地，不会上传）。")
                    Text("需要麦克风等权限时，会在真正使用时单独询问，不会一次性轰炸你。")
                }
            },
            confirmButton = {
                TextButton(onClick = { viewModel.dismissOnboardingAndFreeNotice() }) {
                    Text("开始聊天")
                }
            },
            dismissButton = {
                TextButton(onClick = { viewModel.dismissOnboarding() }) {
                    Text("以后再说")
                }
            }
        )
    }

    // ── 清空对话确认对话框（V1.1 模块2） ──
    // 破坏性操作：重置 LLM 上下文 + 清空显示消息 + 删除本地历史文件
    if (showClearConfirm) {
        AlertDialog(
            onDismissRequest = { viewModel.showClearConfirm.value = false },
            title = { Text("清空对话") },
            text = { Text("将清除当前聊天记录（本地历史与 LLM 上下文），且不可恢复。确定清空吗？") },
            confirmButton = {
                TextButton(onClick = { viewModel.clearConversation() }) {
                    Text("清空")
                }
            },
            dismissButton = {
                TextButton(onClick = { viewModel.showClearConfirm.value = false }) {
                    Text("取消")
                }
            }
        )
    }

    // ── 危险工具确认对话框 ──
    // requiresConfirmation=true 的工具执行前必须经用户确认（ConfirmationGate 硬约束）：
    // ChatService 在 IO 协程挂起等待，这里弹 Compose AlertDialog，点击「同意/拒绝」后
    // 通过 respond() 恢复工具循环。
    viewModel.confirmationGate.pending?.let { pending ->
        AlertDialog(
            onDismissRequest = { viewModel.confirmationGate.respond(false) },
            title = { Text("确认执行") },
            text = {
                Text("工具「${pending.toolName}」请求执行以下操作：\n\n${pending.toolDescription}")
            },
            confirmButton = {
                TextButton(onClick = { viewModel.confirmationGate.respond(true) }) {
                    Text("同意")
                }
            },
            dismissButton = {
                TextButton(onClick = { viewModel.confirmationGate.respond(false) }) {
                    Text("拒绝")
                }
            }
        )
    }
}

/**
 * 单条对话气泡组件。
 *
 * 用户消息 → 右侧蓝色；助手消息 → 左侧灰色（半透明）。
 * 额度耗尽提示气泡会额外展示「前往设置填写自己的 API Key」入口按钮。
 *
 * @param message 要显示的消息数据
 * @param onOpenSettings 设置页入口回调（额度耗尽时跳转）
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
