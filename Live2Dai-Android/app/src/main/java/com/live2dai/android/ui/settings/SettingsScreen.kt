package com.live2dai.android.ui.settings

import android.os.Handler
import android.os.Looper
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Link
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.live2dai.android.BuildConfig
import com.live2dai.android.DownloadState
import com.live2dai.android.LLMProviderManager
import com.live2dai.android.PersonaConfig
import com.live2dai.android.ModelDownloadManager
import com.live2dai.android.ModelSpec
import com.live2dai.android.ProviderType
import com.live2dai.android.refreshCosyVoiceProvider
import com.live2dai.android.TtsHealth
import com.live2dai.android.TtsProviderRegistry
import com.live2dai.android.core.RenderQuality
import com.live2dai.android.core.RenderQualityConfig
import com.live2dai.android.data.ModelEntry
import com.live2dai.android.data.ModelRegistry
import com.live2dai.android.data.SettingsRepository
import kotlinx.coroutines.launch

/**
 * 设置主页面。
 * 包含 API Key、模型、TTS、人设和关于五个板块。
 * 离开页面时自动保存所有设置到 SharedPreferences。
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    onNavigateBack: () -> Unit,
    repository: SettingsRepository? = null,
    /**
     * 形象切换回调（由 MainActivity 注入）：
     * @param modelId 选中的模型 ID
     * @param onResult 切换结果回调（success=true 表示新模型已生效）。
     *                 回调可能在 GL 线程触发，SettingsScreen 内部负责 post 到主线程。
     *                 null 表示未接入（预览/测试场景），此时仅持久化选择。
     *
     * 说明：schema 中 onSwitchModel 签名为 (String) -> Unit，但 UI 需要在切换
     * 失败时回滚选中态并弹 SnackBar，必须拿到异步结果，故增加 onResult 通道。
     */
    onSwitchModel: ((modelId: String, onResult: (Boolean) -> Unit) -> Unit)? = null,
    /**
     * 运行时设置变更回调（由应用层注入）：设置页保存 Provider/API Key/模型/温度/TTS 引擎后，
     * 通知 MainActivity 重建运行中 LLM 链接、刷新 TTS Provider 列表并更新首选引擎，
     * 使「设置写了就能立即用」而不必重启（修复 F2.1/F2.3 调用链断链）。
     */
    onRuntimeSettingsChanged: (() -> Unit)? = null,
) {
    val context = LocalContext.current
    val repo = repository ?: remember { SettingsRepository(context) }
    // ── State ──
    var deepseekKey by remember { mutableStateOf(repo.deepseekApiKey) }
    var selectedModel by remember { mutableStateOf(repo.model) }
    var temperature by remember { mutableStateOf(repo.temperature) }
    var ttsVoice by remember { mutableStateOf(repo.ttsVoice) }
    var ttsSpeed by remember { mutableStateOf(repo.ttsSpeed) }
    var aliyunKey by remember { mutableStateOf(repo.aliyunApiKey) }
    var aliyunKeyVisible by remember { mutableStateOf(false) }
    var cosyvoiceWsUrl by remember { mutableStateOf(repo.cosyvoiceWsUrl) }
    var selectedPersona by remember { mutableStateOf(repo.personaName) }
    var deepseekKeyVisible by remember { mutableStateOf(false) }
    var showModelMenu by remember { mutableStateOf(false) }
    var showTtsVoiceMenu by remember { mutableStateOf(false) }
    var asrEnabled by remember { mutableStateOf(repo.asrEnabled) }
    var asrEngine by remember { mutableStateOf(repo.asrEngine) }
    var showAsrEngineMenu by remember { mutableStateOf(false) }
    var qwenApiKey by remember { mutableStateOf(repo.qwenApiKey) }
    var qwenModel by remember { mutableStateOf(repo.qwenModel) }
    var zhipuApiKey by remember { mutableStateOf(repo.zhipuApiKey) }
    var zhipuModel by remember { mutableStateOf(repo.zhipuModel) }
    var openAiBaseUrl by remember { mutableStateOf(repo.openAiBaseUrl) }
    var openAiApiKey by remember { mutableStateOf(repo.openAiApiKey) }
    var openAiModel by remember { mutableStateOf(repo.openAiModel) }
    var showPersonaDialog by remember { mutableStateOf(false) }
    var showAboutDialog by remember { mutableStateOf(false) }
    var showExitDialog by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()

    // ── 返回拦截：滑屏/系统返回键弹出确认（避免误触退出）──
    BackHandler { showExitDialog = true }

    // ── V1.2 模块5：TTS 引擎选择 + 离线模型下载状态 ──
    // 引擎选择立即持久化（speak 时 VoiceIoController 读取），不随 onDispose 延迟。
    val ttsProviders = remember { TtsProviderRegistry.all() }
    var selectedTtsProviderId by remember { mutableStateOf(repo.ttsProviderId) }
    // 模型下载状态（modelId → 状态）：初始即查询已下载模型（幂等重入，避免闪烁"未下载"）
    val modelDownloadManager = remember { ModelDownloadManager(context) }
    val modelDownloadStates = remember {
        mutableStateMapOf<String, DownloadState>().apply {
            ModelDownloadManager.CATALOG.forEach { spec ->
                if (modelDownloadManager.isDownloaded(spec)) {
                    put(spec.id, DownloadState.Completed(modelDownloadManager.modelDir(spec).absolutePath))
                }
            }
        }
    }

    // 触发模型下载：状态流收集更新 UI（下载中/进度/完成/失败）；同一模型去重由 canStartDownload 保证
    val startModelDownload: (ModelSpec) -> Unit = { spec ->
        if (canStartDownload(modelDownloadStates[spec.id])) {
            scope.launch {
                modelDownloadManager.download(spec).collect { state ->
                    modelDownloadStates[spec.id] = state
                }
            }
        }
    }

    // 保存阿里云 API Key / WS 地址：持久化 + 重建 CosyVoice Provider（register 同名覆盖，新配置立即生效）
    val saveAliyunKey: () -> Unit = {
        if (aliyunKey != repo.aliyunApiKey || cosyvoiceWsUrl != repo.cosyvoiceWsUrl) {
            repo.aliyunApiKey = aliyunKey
            repo.cosyvoiceWsUrl = cosyvoiceWsUrl
            refreshCosyVoiceProvider(context)
            // CosyVoice Key/WS 更新 → 刷新运行中 Provider 列表（新配置立即生效）
            onRuntimeSettingsChanged?.invoke()
        }
    }

    // ── Live2D 形象状态 ──
    val registry = remember { ModelRegistry.load(context) }
    var selectedModelId by remember { mutableStateOf(repo.selectedModelId) }
    var switchingModelId by remember { mutableStateOf<String?>(null) }
    val snackbarHostState = remember { SnackbarHostState() }
    val mainHandler = remember { Handler(Looper.getMainLooper()) }

    // 点击模型卡片：持久化 + 回调切换（成功才更新选中态与持久化值）
    val handleSwitch: (String) -> Unit = { modelId ->
        val entry = registry.getById(modelId)
        if (entry != null && switchingModelId == null && modelId != selectedModelId) {
            switchingModelId = modelId
            if (onSwitchModel == null) {
                // 未接入切换回调（预览/测试）：仅持久化选择
                selectedModelId = modelId
                repo.selectedModelId = modelId
                switchingModelId = null
            } else {
                onSwitchModel(modelId) { success ->
                    mainHandler.post {
                        switchingModelId = null
                        if (success) {
                            selectedModelId = modelId
                            repo.selectedModelId = modelId
                            scope.launch {
                                snackbarHostState.showSnackbar("已切换为 ${entry.name}")
                            }
                        } else {
                            scope.launch {
                                snackbarHostState.showSnackbar("形象切换失败，已回退到之前的形象")
                            }
                        }
                    }
                }
            }
        }
    }

    val hasCustomDeepseek = repo.hasCustomDeepseekKey

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("设置") },
                navigationIcon = {
                    IconButton(onClick = onNavigateBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "返回")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.primaryContainer,
                    titleContentColor = MaterialTheme.colorScheme.onPrimaryContainer,
                )
            )
        },
        snackbarHost = { SnackbarHost(snackbarHostState) }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .verticalScroll(rememberScrollState())
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp)
        ) {
            // ── API 设置 ──
            SectionHeader("API 设置")

            // DeepSeek Key
            Text(
                "DeepSeek API Key",
                style = MaterialTheme.typography.labelLarge,
                modifier = Modifier.padding(start = 4.dp, top = 8.dp)
            )
            OutlinedTextField(
                value = deepseekKey,
                onValueChange = { deepseekKey = it },
                label = {
                    Text(
                        if (hasCustomDeepseek) "自定义 Key"
                        else "留空使用编译时默认 Key"
                    )
                },
                singleLine = true,
                visualTransformation = if (deepseekKeyVisible) VisualTransformation.None
                    else PasswordVisualTransformation(),
                trailingIcon = {
                    IconButton(onClick = { deepseekKeyVisible = !deepseekKeyVisible }) {
                        Icon(
                            if (deepseekKeyVisible) Icons.Default.VisibilityOff
                            else Icons.Default.Visibility,
                            contentDescription = if (deepseekKeyVisible) "隐藏" else "显示"
                        )
                    }
                },
                keyboardOptions = KeyboardOptions(
                    keyboardType = KeyboardType.Password,
                    imeAction = ImeAction.Done
                ),
                modifier = Modifier.fillMaxWidth()
            )
            if (!hasCustomDeepseek && deepseekKey.isEmpty()) {
                Text(
                    "当前使用编译时默认 Key${if (BuildConfig.DEEPSEEK_API_KEY.isNotEmpty()) " (已配置)" else " (未配置)"}",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(start = 8.dp, bottom = 4.dp)
                )
            }

            // 恢复默认按钮
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                if (hasCustomDeepseek) {
                    OutlinedButton(
                        onClick = {
                            if (hasCustomDeepseek) {
                                repo.clearCustomDeepseekKey()
                                deepseekKey = ""
                            }
                        }
                    ) { Text("恢复 API Key 为默认") }
                }
            }

            HorizontalDivider(modifier = Modifier.padding(vertical = 12.dp))

            // ── 模型设置 ──
            SectionHeader("模型设置")

            // Provider 选择（零配置默认 = DeepSeek 云端，见 LLMProviderManager）
            LLMProviderManager.init(context)
            var selectedProvider by remember { mutableStateOf(LLMProviderManager.getProviderType()) }
            Text(
                "服务商 (Provider)",
                style = MaterialTheme.typography.labelLarge,
                modifier = Modifier.padding(start = 4.dp, top = 8.dp)
            )
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                listOf(
                    ProviderType.DEEPSEEK to "DeepSeek",
                    ProviderType.QWEN to "通义千问",
                    ProviderType.ZHIPU to "智谱 GLM",
                    ProviderType.OPENAI_COMPATIBLE to "OpenAI 兼容",
                ).forEach { (type, label) ->
                    FilterChip(
                        selected = selectedProvider == type,
                        onClick = {
                            selectedProvider = type
                            LLMProviderManager.setProviderType(type)
                            // Provider 切换 → 下一次请求即用新端点/Key/模型（无需重启）
                            onRuntimeSettingsChanged?.invoke()
                        },
                        label = { Text(label) }
                    )
                }
            }

            when (selectedProvider) {
                ProviderType.DEEPSEEK -> {
                    Text(
                        "使用 DeepSeek 云端（你自己的 API Key）。",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(start = 8.dp, bottom = 4.dp)
                    )
                }
                ProviderType.QWEN -> {
                    OutlinedTextField(
                        value = qwenApiKey,
                        onValueChange = { qwenApiKey = it; repo.qwenApiKey = it },
                        label = { Text("通义千问 API Key") },
                        singleLine = true,
                        visualTransformation = PasswordVisualTransformation(),
                        modifier = Modifier.fillMaxWidth()
                    )
                    OutlinedTextField(
                        value = qwenModel,
                        onValueChange = { qwenModel = it; repo.qwenModel = it },
                        label = { Text("模型（如 qwen3.7-flash）") },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth()
                    )
                }
                ProviderType.ZHIPU -> {
                    OutlinedTextField(
                        value = zhipuApiKey,
                        onValueChange = { zhipuApiKey = it; repo.zhipuApiKey = it },
                        label = { Text("智谱 API Key") },
                        singleLine = true,
                        visualTransformation = PasswordVisualTransformation(),
                        modifier = Modifier.fillMaxWidth()
                    )
                    OutlinedTextField(
                        value = zhipuModel,
                        onValueChange = { zhipuModel = it; repo.zhipuModel = it },
                        label = { Text("模型（如 glm-4.7-flash）") },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth()
                    )
                }
                ProviderType.OPENAI_COMPATIBLE -> {
                    OutlinedTextField(
                        value = openAiBaseUrl,
                        onValueChange = { openAiBaseUrl = it; repo.openAiBaseUrl = it },
                        label = { Text("Base URL（如 https://api.openai.com/v1）") },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth()
                    )
                    OutlinedTextField(
                        value = openAiApiKey,
                        onValueChange = { openAiApiKey = it; repo.openAiApiKey = it },
                        label = { Text("API Key") },
                        singleLine = true,
                        visualTransformation = PasswordVisualTransformation(),
                        modifier = Modifier.fillMaxWidth()
                    )
                    OutlinedTextField(
                        value = openAiModel,
                        onValueChange = { openAiModel = it; repo.openAiModel = it },
                        label = { Text("模型") },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth()
                    )
                }
                else -> Unit
            }

            // 模型选择
            Text(
                "聊天模型",
                style = MaterialTheme.typography.labelLarge,
                modifier = Modifier.padding(start = 4.dp, top = 8.dp)
            )
            ExposedDropdownMenuBox(
                expanded = showModelMenu,
                onExpandedChange = { showModelMenu = it }
            ) {
                OutlinedTextField(
                    value = selectedModel,
                    onValueChange = {},
                    readOnly = true,
                    trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = showModelMenu) },
                    modifier = Modifier
                        .fillMaxWidth()
                        .menuAnchor()
                )
                ExposedDropdownMenu(
                    expanded = showModelMenu,
                    onDismissRequest = { showModelMenu = false }
                ) {
                    SettingsRepository.AVAILABLE_MODELS.forEach { model ->
                        DropdownMenuItem(
                            text = { Text(model) },
                            onClick = {
                                selectedModel = model
                                repo.model = model
                                showModelMenu = false
                                // 聊天模型变更 → 下一次 LLM 请求即用新模型（F2.2/F3.2）
                                onRuntimeSettingsChanged?.invoke()
                            }
                        )
                    }
                }
            }

            // Temperature 滑块
            Text(
                "Temperature: %.2f".format(temperature),
                style = MaterialTheme.typography.labelLarge,
                modifier = Modifier.padding(start = 4.dp, top = 12.dp)
            )
            Slider(
                value = temperature,
                onValueChange = { temperature = it },
                onValueChangeFinished = {
                    repo.temperature = temperature
                    // 温度变更 → 下一次 LLM 请求即用新值（F3.2）
                    onRuntimeSettingsChanged?.invoke()
                },
                valueRange = 0.0f..1.0f,
                steps = 19,
                modifier = Modifier.fillMaxWidth()
            )

            HorizontalDivider(modifier = Modifier.padding(vertical = 12.dp))

            // ── 形象设置（Live2D 模型选择） ──
            SectionHeader("形象设置")

            Text(
                "选择数字人形象，切换即时生效",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(start = 4.dp, bottom = 4.dp)
            )
            ModelSelector(
                registry = registry,
                selectedModelId = selectedModelId,
                switchingModelId = switchingModelId,
                onSwitchModel = handleSwitch
            )

            HorizontalDivider(modifier = Modifier.padding(vertical = 12.dp))

            // ── 渲染质量（用户需求：流畅 / 均衡 / 高清三档）──
            SectionHeader("渲染质量")

            var renderQuality by remember { mutableStateOf(repo.renderQuality) }
            Text(
                "选择画面质量档位：超采样切换即时生效；均衡档 MSAA 4x 需重启生效",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(start = 4.dp, bottom = 8.dp)
            )
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                RenderQuality.entries.forEach { q ->
                    FilterChip(
                        selected = renderQuality == q,
                        onClick = {
                            renderQuality = q
                            // 即时生效：超采样（GL 线程下一帧读 current 重建 FBO）；
                            // MSAA（BALANCED）需 EGL 上下文重建 → 重启生效（UI 已注明）。
                            RenderQualityConfig.current = q
                            repo.renderQuality = q
                        },
                        label = { Text(q.label) }
                    )
                }
            }
            Text(
                renderQuality.hint,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(start = 8.dp, bottom = 4.dp)
            )

            HorizontalDivider(modifier = Modifier.padding(vertical = 12.dp))

            // ── 语音输入设置（P0）──
            SectionHeader("语音输入")
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text("启用语音输入", style = MaterialTheme.typography.bodyLarge)
                    Text(
                        "使用系统语音识别，识别结果直接进入对话。",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
                Switch(
                    checked = asrEnabled,
                    onCheckedChange = {
                        asrEnabled = it
                        repo.asrEnabled = it
                    }
                )
            }
            Text(
                "识别引擎",
                style = MaterialTheme.typography.bodyLarge,
                modifier = Modifier.padding(top = 8.dp)
            )
            ExposedDropdownMenuBox(
                expanded = showAsrEngineMenu,
                onExpandedChange = { showAsrEngineMenu = it }
            ) {
                OutlinedTextField(
                    value = if (asrEngine == "sherpa") "离线 SenseVoice（需下载模型）" else "系统语音识别",
                    onValueChange = {},
                    readOnly = true,
                    trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = showAsrEngineMenu) },
                    modifier = Modifier
                        .fillMaxWidth()
                        .menuAnchor()
                )
                ExposedDropdownMenu(
                    expanded = showAsrEngineMenu,
                    onDismissRequest = { showAsrEngineMenu = false }
                ) {
                    DropdownMenuItem(
                        text = { Text("系统语音识别") },
                        onClick = {
                            asrEngine = "system"
                            repo.asrEngine = "system"
                            showAsrEngineMenu = false
                        }
                    )
                    DropdownMenuItem(
                        text = { Text("离线 SenseVoice（需下载模型）") },
                        onClick = {
                            asrEngine = "sherpa"
                            repo.asrEngine = "sherpa"
                            showAsrEngineMenu = false
                        }
                    )
                }
            }
            HorizontalDivider(modifier = Modifier.padding(vertical = 12.dp))

            // ── TTS 设置 ──
            SectionHeader("TTS 设置")

            // ── V1.2 模块5：TTS 引擎选择器（首选引擎，降级链系统内置自动兜底） ──
            val ttsOptions = ttsProvidersToUi(ttsProviders, selectedTtsProviderId)
            Text(
                "首选引擎（未选中的引擎自动作为降级兜底）",
                style = MaterialTheme.typography.labelLarge,
                modifier = Modifier.padding(start = 4.dp, top = 8.dp)
            )
            EngineSelectorSection(
                options = ttsOptions,
                onSelect = { id ->
                    selectedTtsProviderId = id
                    repo.ttsProviderId = id
                    // 首选引擎变更 → 运行时立即生效（F2.3：不再纯装饰）
                    onRuntimeSettingsChanged?.invoke()
                }
            )

            // 选中引擎需配置（CosyVoice 云端）→ 展开阿里云 API Key + WS 地址输入区
            if (ttsOptions.any { it.isSelected && it.configurable }) {
                Text(
                    "阿里云 API Key（CosyVoice 云端，DashScope）",
                    style = MaterialTheme.typography.labelLarge,
                    modifier = Modifier.padding(start = 4.dp, top = 8.dp)
                )
                OutlinedTextField(
                    value = aliyunKey,
                    onValueChange = { aliyunKey = it },
                    label = { Text(if (aliyunKey.isEmpty()) "留空 = 未配置（自动降级）" else "已配置") },
                    singleLine = true,
                    visualTransformation = if (aliyunKeyVisible) VisualTransformation.None
                        else PasswordVisualTransformation(),
                    trailingIcon = {
                        IconButton(onClick = { aliyunKeyVisible = !aliyunKeyVisible }) {
                            Icon(
                                if (aliyunKeyVisible) Icons.Default.VisibilityOff
                                else Icons.Default.Visibility,
                                contentDescription = if (aliyunKeyVisible) "隐藏" else "显示"
                            )
                        }
                    },
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Password,
                        imeAction = ImeAction.Done
                    ),
                    keyboardActions = KeyboardActions(onDone = { saveAliyunKey() }),
                    modifier = Modifier.fillMaxWidth()
                )
                if (aliyunKey.isEmpty()) {
                    Text(
                        "未配置时 CosyVoice 快速失败并自动降级到下一个引擎；阿里云每月有免费额度。",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(start = 8.dp, bottom = 4.dp)
                    )
                }

                Text(
                    "WS 地址（可选；留空 = 默认公共端点）",
                    style = MaterialTheme.typography.labelLarge,
                    modifier = Modifier.padding(start = 4.dp, top = 8.dp)
                )
                OutlinedTextField(
                    value = cosyvoiceWsUrl,
                    onValueChange = { cosyvoiceWsUrl = it },
                    label = { Text(if (cosyvoiceWsUrl.isEmpty()) "wss://dashscope.aliyuncs.com/api-ws/v1/inference" else "已填专属端点") },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Uri,
                        imeAction = ImeAction.Done
                    ),
                    keyboardActions = KeyboardActions(onDone = { saveAliyunKey() }),
                    modifier = Modifier.fillMaxWidth()
                )
                if (cosyvoiceWsUrl.isEmpty()) {
                    Text(
                        "留空使用默认公共端点；专属服务用户填 wss://{专属host}/api-ws/v1/inference。",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(start = 8.dp, bottom = 4.dp)
                    )
                }
            }

            // 降级链说明（CosyVoice 云端 → Kokoro 离线）
            Text(
                "ℹ️ 降级链: ${cascadeSummary(ttsProviders.map { it.info.name })}",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(start = 4.dp, bottom = 4.dp)
            )

            Text(
                "语音",
                style = MaterialTheme.typography.labelLarge,
                modifier = Modifier.padding(start = 4.dp, top = 8.dp)
            )
            ExposedDropdownMenuBox(
                expanded = showTtsVoiceMenu,
                onExpandedChange = { showTtsVoiceMenu = it }
            ) {
                OutlinedTextField(
                    value = ttsVoice,
                    onValueChange = {},
                    readOnly = true,
                    trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = showTtsVoiceMenu) },
                    modifier = Modifier
                        .fillMaxWidth()
                        .menuAnchor()
                )
                ExposedDropdownMenu(
                    expanded = showTtsVoiceMenu,
                    onDismissRequest = { showTtsVoiceMenu = false }
                ) {
                    SettingsRepository.AVAILABLE_TTS_VOICES.forEach { voice ->
                        DropdownMenuItem(
                            text = { Text(voice) },
                            onClick = {
                                ttsVoice = voice
                                repo.ttsVoice = voice
                                showTtsVoiceMenu = false
                            }
                        )
                    }
                }
            }

            // 语速
            Text(
                "语速: %.1fx".format(ttsSpeed),
                style = MaterialTheme.typography.labelLarge,
                modifier = Modifier.padding(start = 4.dp, top = 12.dp)
            )
            Slider(
                value = ttsSpeed,
                onValueChange = { ttsSpeed = it },
                onValueChangeFinished = { repo.ttsSpeed = ttsSpeed },
                valueRange = 0.5f..2.0f,
                steps = 14,
                modifier = Modifier.fillMaxWidth()
            )

            HorizontalDivider(modifier = Modifier.padding(vertical = 12.dp))

            // ── V1.2 模块5：离线模型下载（Kokoro TTS，按需下载不打包进 APK） ──
            SectionHeader("离线模型")

            Text(
                "模型按需下载到应用外部存储（不打包进 APK），下载完成后离线引擎自动可用。",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(start = 4.dp, bottom = 8.dp)
            )
            ModelDownloadManager.CATALOG.forEach { spec ->
                ModelDownloadCard(
                    spec = spec,
                    state = modelDownloadStates[spec.id],
                    onDownload = { startModelDownload(spec) }
                )
                Spacer(Modifier.height(8.dp))
            }

            HorizontalDivider(modifier = Modifier.padding(vertical = 12.dp))

            // ── 人设设置 ──
            SectionHeader("人设设置")

            PersonaSelector(
                currentPersona = selectedPersona,
                onPersonaSelected = { name ->
                    selectedPersona = name
                    repo.personaName = name
                    showPersonaDialog = false
                },
                showDialog = showPersonaDialog,
                onShowDialogChange = { showPersonaDialog = it }
            )

            HorizontalDivider(modifier = Modifier.padding(vertical = 12.dp))

            // ── 关于 ──
            SectionHeader("关于")

            AboutSection(onShowLicenses = { showAboutDialog = true })

            Spacer(Modifier.height(32.dp))
        }
    }

    // ── 关于弹窗 ──
    if (showAboutDialog) {
        AboutDialog(onDismiss = { showAboutDialog = false })
    }

    // ── 退出确认弹窗 ──
    if (showExitDialog) {
        AlertDialog(
            onDismissRequest = { showExitDialog = false },
            title = { Text("退出设置") },
            text = { Text("所有设置会自动保存，确定退出吗？") },
            confirmButton = {
                TextButton(onClick = { showExitDialog = false; onNavigateBack() }) {
                    Text("确定退出")
                }
            },
            dismissButton = {
                TextButton(onClick = { showExitDialog = false }) { Text("继续设置") }
            }
        )
    }

    // ── 离开页面时自动保存所有设置 ──
    DisposableEffect(Unit) {
        onDispose {
            val cosyChanged = aliyunKey != repo.aliyunApiKey || cosyvoiceWsUrl != repo.cosyvoiceWsUrl
            repo.deepseekApiKey = deepseekKey
            repo.model = selectedModel
            repo.temperature = temperature
            repo.ttsVoice = ttsVoice
            repo.ttsSpeed = ttsSpeed
            repo.personaName = selectedPersona
            repo.aliyunApiKey = aliyunKey
            repo.cosyvoiceWsUrl = cosyvoiceWsUrl
            // Key/WS 地址变更 → 重建 CosyVoice Provider（新配置立即生效，无需重启）
            if (cosyChanged) refreshCosyVoiceProvider(context)
            // 离开页面 → 应用全部运行时设置（LLM 链接 + TTS Provider 列表 + 首选引擎）
            onRuntimeSettingsChanged?.invoke()
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  辅助组件
// ═══════════════════════════════════════════════════════════════

@Composable
private fun SectionHeader(title: String) {
    Text(
        text = title,
        style = MaterialTheme.typography.titleMedium,
        fontWeight = FontWeight.Bold,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(vertical = 4.dp)
    )
}

@Composable
private fun PersonaSelector(
    currentPersona: String,
    onPersonaSelected: (String) -> Unit,
    showDialog: Boolean,
    onShowDialogChange: (Boolean) -> Unit,
) {
    // M4c 单一数据源：人设列表由 PersonaConfig.getAvailablePersonas() 驱动
    // （assets/persona.yaml + assets/personas/*.yaml 枚举，禁硬编码条目）
    val context = LocalContext.current
    val personas = remember { PersonaConfig.getAvailablePersonas(context) }

    // M4c：选择即加载对应人设资产（路径来自 getAvailablePersonas 返回值），随后回调上层持久化
    val selectPersona: (String) -> Unit = { name ->
        personas.find { it.first == name }?.second?.let { PersonaConfig.loadFile(context, it) }
        onPersonaSelected(name)
    }

    OutlinedCard(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onShowDialogChange(true) }
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(currentPersona, style = MaterialTheme.typography.bodyLarge)
                val desc = personas.find { it.first == currentPersona }?.second ?: ""
                if (desc.isNotEmpty()) {
                    Text(
                        desc,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }
            Text(
                "切换 >",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.primary
            )
        }
    }

    if (showDialog) {
        AlertDialog(
            onDismissRequest = { onShowDialogChange(false) },
            title = { Text("选择人设") },
            text = {
                Column {
                    personas.forEach { (name, desc) ->
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable { selectPersona(name) }
                                .padding(vertical = 12.dp, horizontal = 4.dp),
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            RadioButton(
                                selected = name == currentPersona,
                                onClick = { selectPersona(name) }
                            )
                            Spacer(Modifier.width(12.dp))
                            Column {
                                Text(name, style = MaterialTheme.typography.bodyLarge)
                                Text(
                                    desc,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                            }
                        }
                    }
                }
            },
            confirmButton = {
                TextButton(onClick = { onShowDialogChange(false) }) {
                    Text("确定")
                }
            }
        )
    }
}

@Composable
private fun AboutSection(onShowLicenses: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 8.dp, horizontal = 4.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text("版本号", style = MaterialTheme.typography.bodyLarge)
        Text(
            "${BuildConfig.VERSION_NAME} (${BuildConfig.VERSION_CODE})",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
    }

    TextButton(onClick = onShowLicenses) {
        Icon(
            Icons.Default.Link,
            contentDescription = null,
            modifier = Modifier.size(16.dp)
        )
        Spacer(Modifier.width(4.dp))
        Text("查看开源许可")
    }
}

@Composable
private fun AboutDialog(onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("开源许可") },
        text = {
            Text(
                buildString {
                    appendLine("Live2Dai — Open Source Licenses\n")
                    appendLine("This application uses the following open-source components:\n")
                    appendLine("• Jetpack Compose — Apache 2.0")
                    appendLine("• Kotlinx Serialization — Apache 2.0")
                    appendLine("• OkHttp — Apache 2.0")
                    appendLine("• SnakeYAML — Apache 2.0")
                    appendLine("• Purism Core (Live2D) — MIT")
                    appendLine("• Cubism Java Framework — MIT")
                    appendLine("\nFull license texts are available in the GitHub repo.")
                },
                style = MaterialTheme.typography.bodySmall
            )
        },
        confirmButton = {
            TextButton(onClick = onDismiss) {
                Text("关闭")
            }
        }
    )
}

// ═══════════════════════════════════════════════════════════════
//  Live2D 形象选择器
// ═══════════════════════════════════════════════════════════════

/**
 * Live2D 模型选择列表：可用条目在前（可点击切换），不可用条目在后（灰显"即将推出"）。
 */
@Composable
fun ModelSelector(
    registry: ModelRegistry,
    selectedModelId: String,
    switchingModelId: String?,
    onSwitchModel: (String) -> Unit,
) {
    val available = remember(registry) { registry.getAvailable() }
    val unavailable = remember(registry) { registry.getAll().filter { !it.available } }
    val models = remember(registry, available, unavailable) { available + unavailable }

    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        models.forEach { model ->
            ModelCard(
                model = model,
                isSelected = model.id == selectedModelId,
                isSwitchingThis = model.id == switchingModelId,
                listDisabled = switchingModelId != null,
                onSwitchModel = onSwitchModel
            )
        }
    }
}

@Composable
private fun ModelCard(
    model: ModelEntry,
    isSelected: Boolean,
    isSwitchingThis: Boolean,
    listDisabled: Boolean,
    onSwitchModel: (String) -> Unit,
) {
    // 可用且未在切换中且非当前选中 → 可点击；其余（不可用/切换中/已选中）不可点
    val clickable = model.available && !listDisabled && !isSelected
    val containerColor = when {
        isSelected -> MaterialTheme.colorScheme.primaryContainer
        model.available -> MaterialTheme.colorScheme.surface
        else -> MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)
    }

    OutlinedCard(
        modifier = Modifier
            .fillMaxWidth()
            .then(if (clickable) Modifier.clickable { onSwitchModel(model.id) } else Modifier),
        colors = CardDefaults.outlinedCardColors(containerColor = containerColor)
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(12.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            // 缩略图占位（registry 中 thumbnailPath 当前均为 null，按约定后续放
            // assets/live2d/<id>/thumbnail.png 后可扩展为 AsyncImage 加载）
            Box(
                modifier = Modifier
                    .size(44.dp)
                    .clip(CircleShape)
                    .background(
                        if (isSelected) MaterialTheme.colorScheme.primary.copy(alpha = 0.15f)
                        else MaterialTheme.colorScheme.surfaceVariant
                    ),
                contentAlignment = Alignment.Center
            ) {
                Icon(
                    Icons.Default.Person,
                    contentDescription = null,
                    tint = if (isSelected) MaterialTheme.colorScheme.primary
                        else MaterialTheme.colorScheme.onSurfaceVariant
                )
            }

            Spacer(Modifier.width(12.dp))

            Column(modifier = Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        model.name,
                        style = MaterialTheme.typography.titleSmall,
                        fontWeight = FontWeight.Bold,
                        color = if (model.available) MaterialTheme.colorScheme.onSurface
                            else MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    if (isSelected) {
                        Spacer(Modifier.width(6.dp))
                        Surface(
                            shape = RoundedCornerShape(4.dp),
                            color = MaterialTheme.colorScheme.primary
                        ) {
                            Text(
                                "✓ 使用中",
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onPrimary,
                                modifier = Modifier.padding(horizontal = 4.dp, vertical = 1.dp)
                            )
                        }
                    } else if (!model.available) {
                        Spacer(Modifier.width(6.dp))
                        Surface(
                            shape = RoundedCornerShape(4.dp),
                            color = MaterialTheme.colorScheme.surfaceVariant
                        ) {
                            Text(
                                "即将推出",
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.padding(horizontal = 4.dp, vertical = 1.dp)
                            )
                        }
                    }
                }
                if (!model.description.isNullOrEmpty()) {
                    Spacer(Modifier.height(2.dp))
                    Text(
                        model.description,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis
                    )
                }
            }

            if (isSwitchingThis) {
                Spacer(Modifier.width(8.dp))
                CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp)
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  V1.2 模块5：TTS/ASR 引擎选择器 + 离线模型下载卡片
// ═══════════════════════════════════════════════════════════════

/**
 * 引擎单选列表（plan §5.5.2 方案A RadioGroup）：每行显示引擎名 + 健康徽标
 * （正常/降级/不可用）+ 网络/离线标识；点击行或 RadioButton 触发 onSelect(id)。
 *
 * options 由 [ttsProvidersToUi] 纯函数产出（可单测），
 * 本组件只负责渲染与交互。
 */
@Composable
fun EngineSelectorSection(
    options: List<EngineOptionUi>,
    onSelect: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(modifier = modifier.fillMaxWidth()) {
        options.forEach { option ->
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { onSelect(option.id) }
                    .padding(vertical = 4.dp, horizontal = 4.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                RadioButton(
                    selected = option.isSelected,
                    onClick = { onSelect(option.id) }
                )
                Spacer(Modifier.width(8.dp))
                Column(modifier = Modifier.weight(1f)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            option.name,
                            style = MaterialTheme.typography.bodyLarge,
                            modifier = Modifier.padding(end = 8.dp)
                        )
                        HealthBadge(health = option.health)
                    }
                    Spacer(Modifier.height(2.dp))
                    Text(
                        if (option.requiresNetwork) "需联网 · 云端引擎" else "离线可用 · 本地引擎",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }
        }
    }
}

/** 健康状态徽标（正常=primary / 降级=tertiary / 不可用=error）。 */
@Composable
private fun HealthBadge(health: TtsHealth) {
    val color = when (health) {
        TtsHealth.OK -> MaterialTheme.colorScheme.primary
        TtsHealth.DEGRADED -> MaterialTheme.colorScheme.tertiary
        TtsHealth.UNAVAILABLE -> MaterialTheme.colorScheme.error
    }
    Surface(
        shape = RoundedCornerShape(4.dp),
        color = color.copy(alpha = 0.12f)
    ) {
        Text(
            healthLabel(health),
            style = MaterialTheme.typography.labelSmall,
            color = color,
            modifier = Modifier.padding(horizontal = 6.dp, vertical = 2.dp)
        )
    }
}

/**
 * 离线模型下载卡片：模型名 + 体积 + 状态（未下载/下载中 x%/校验中/已下载/下载失败）
 * + 进度条 + "下载"按钮（失败态变为"重试下载"）。
 *
 * state 来自 ModelDownloadManager.download(spec) 的状态流收集；
 * 按钮可用性由纯函数 [canStartDownload] 判定（去重：下载中/已完成禁用）。
 */
@Composable
fun ModelDownloadCard(
    spec: ModelSpec,
    state: DownloadState?,
    onDownload: () -> Unit,
    modifier: Modifier = Modifier,
) {
    OutlinedCard(modifier = modifier.fillMaxWidth()) {
        Column(Modifier.padding(12.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    spec.name,
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.weight(1f)
                )
                Text(
                    formatModelSize(spec.sizeBytes),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }

            Spacer(Modifier.height(6.dp))

            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    downloadStatusLabel(state),
                    style = MaterialTheme.typography.bodyMedium,
                    color = when (state) {
                        is DownloadState.Failed -> MaterialTheme.colorScheme.error
                        is DownloadState.Completed -> MaterialTheme.colorScheme.primary
                        else -> MaterialTheme.colorScheme.onSurfaceVariant
                    },
                    modifier = Modifier.padding(end = 8.dp)
                )
                if (state is DownloadState.Downloading) {
                    LinearProgressIndicator(
                        progress = { state.progress },
                        modifier = Modifier
                            .weight(1f)
                            .height(6.dp)
                            .clip(RoundedCornerShape(3.dp))
                    )
                }
            }

            if (state is DownloadState.Failed) {
                Spacer(Modifier.height(4.dp))
                Text(
                    state.error,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis
                )
            }

            Spacer(Modifier.height(8.dp))
            OutlinedButton(
                onClick = onDownload,
                enabled = canStartDownload(state)
            ) {
                Text(if (state is DownloadState.Failed) "重试下载" else "下载")
            }
        }
    }
}
