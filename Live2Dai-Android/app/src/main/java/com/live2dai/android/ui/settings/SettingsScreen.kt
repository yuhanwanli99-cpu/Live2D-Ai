package com.live2dai.android.ui.settings

import android.os.Handler
import android.os.Looper
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
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
import com.live2dai.android.LLMProviderManager
import com.live2dai.android.ProviderType
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
) {
    val context = LocalContext.current
    val repo = repository ?: remember { SettingsRepository(context) }
    // ── State ──
    var deepseekKey by remember { mutableStateOf(repo.deepseekApiKey) }
    var zhipuKey by remember { mutableStateOf(repo.zhipuApiKey) }
    var selectedModel by remember { mutableStateOf(repo.model) }
    var temperature by remember { mutableStateOf(repo.temperature) }
    var ttsVoice by remember { mutableStateOf(repo.ttsVoice) }
    var ttsSpeed by remember { mutableStateOf(repo.ttsSpeed) }
    var selectedPersona by remember { mutableStateOf(repo.personaName) }
    var deepseekKeyVisible by remember { mutableStateOf(false) }
    var zhipuKeyVisible by remember { mutableStateOf(false) }
    var showModelMenu by remember { mutableStateOf(false) }
    var showTtsVoiceMenu by remember { mutableStateOf(false) }
    var showPersonaDialog by remember { mutableStateOf(false) }
    var showAboutDialog by remember { mutableStateOf(false) }

    // ── Live2D 形象状态 ──
    val registry = remember { ModelRegistry.load(context) }
    var selectedModelId by remember { mutableStateOf(repo.selectedModelId) }
    var switchingModelId by remember { mutableStateOf<String?>(null) }
    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
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
    val hasCustomZhipu = repo.hasCustomZhipuKey

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

            // Zhipu Key
            Spacer(Modifier.height(8.dp))
            Text(
                "智谱 (Zhipu) API Key",
                style = MaterialTheme.typography.labelLarge,
                modifier = Modifier.padding(start = 4.dp, top = 8.dp)
            )
            OutlinedTextField(
                value = zhipuKey,
                onValueChange = { zhipuKey = it },
                label = {
                    Text(
                        if (hasCustomZhipu) "自定义 Key"
                        else "留空使用编译时默认 Key"
                    )
                },
                singleLine = true,
                visualTransformation = if (zhipuKeyVisible) VisualTransformation.None
                    else PasswordVisualTransformation(),
                trailingIcon = {
                    IconButton(onClick = { zhipuKeyVisible = !zhipuKeyVisible }) {
                        Icon(
                            if (zhipuKeyVisible) Icons.Default.VisibilityOff
                            else Icons.Default.Visibility,
                            contentDescription = if (zhipuKeyVisible) "隐藏" else "显示"
                        )
                    }
                },
                keyboardOptions = KeyboardOptions(
                    keyboardType = KeyboardType.Password,
                    imeAction = ImeAction.Done
                ),
                modifier = Modifier.fillMaxWidth()
            )
            if (!hasCustomZhipu && zhipuKey.isEmpty()) {
                Text(
                    "当前使用编译时默认 Key${if (BuildConfig.ZHIPU_API_KEY.isNotEmpty()) " (已配置)" else " (未配置)"}",
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
                if (hasCustomDeepseek || hasCustomZhipu) {
                    OutlinedButton(
                        onClick = {
                            if (hasCustomDeepseek) {
                                repo.clearCustomDeepseekKey()
                                deepseekKey = ""
                            }
                            if (hasCustomZhipu) {
                                repo.clearCustomZhipuKey()
                                zhipuKey = ""
                            }
                        }
                    ) { Text("恢复 API Key 为默认") }
                }
            }

            HorizontalDivider(modifier = Modifier.padding(vertical = 12.dp))

            // ── 模型设置 ──
            SectionHeader("模型设置")

            // Provider 选择（零配置默认 = 智谱 GLM 免费兜底，见 LLMProviderManager）
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
                    ProviderType.DEEPSEEK to "DeepSeek 云端",
                    ProviderType.ZHIPU to "智谱 GLM（免费）",
                ).forEach { (type, label) ->
                    FilterChip(
                        selected = selectedProvider == type,
                        onClick = {
                            selectedProvider = type
                            LLMProviderManager.setProviderType(type)
                        },
                        label = { Text(label) }
                    )
                }
            }
            val activeProvider = LLMProviderManager.getActiveProvider()
            when {
                selectedProvider == ProviderType.ZHIPU && activeProvider.type == ProviderType.ZHIPU -> {
                    val keySource = if (hasCustomZhipu) "你的自定义智谱 API Key"
                        else "零配置免费兜底（编译期内置 Key）"
                    Text(
                        "当前正在使用免费兜底模型 GLM-4.7-Flash（$keySource）。" +
                            "个人账号限 1 并发，高并发场景建议切换到 DeepSeek 云端。",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(start = 8.dp, bottom = 4.dp)
                    )
                }
                selectedProvider == ProviderType.ZHIPU -> {
                    Text(
                        "未检测到智谱 API Key，已安全回退到 DeepSeek 云端。",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.padding(start = 8.dp, bottom = 4.dp)
                    )
                }
                else -> {
                    Text(
                        "使用 DeepSeek 云端（你自己的 API Key）。",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(start = 8.dp, bottom = 4.dp)
                    )
                }
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
                onValueChangeFinished = { repo.temperature = temperature },
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

            // ── TTS 设置 ──
            SectionHeader("TTS 设置")

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

    // ── 离开页面时自动保存所有设置 ──
    DisposableEffect(Unit) {
        onDispose {
            repo.deepseekApiKey = deepseekKey
            repo.zhipuApiKey = zhipuKey
            repo.model = selectedModel
            repo.temperature = temperature
            repo.ttsVoice = ttsVoice
            repo.ttsSpeed = ttsSpeed
            repo.personaName = selectedPersona
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
    val personas = remember {
        listOf(
            "小喵 (Xiao Miao)" to "猫娘女仆 — 可爱、活泼、忠诚",
            "绫乃 (Ayano)" to "傲娇学姐 — 表面冷漠，内心温柔",
        )
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
                                .clickable { onPersonaSelected(name) }
                                .padding(vertical = 12.dp, horizontal = 4.dp),
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            RadioButton(
                                selected = name == currentPersona,
                                onClick = { onPersonaSelected(name) }
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
                    appendLine("• Edge TTS — Apache 2.0 / MIT")
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
