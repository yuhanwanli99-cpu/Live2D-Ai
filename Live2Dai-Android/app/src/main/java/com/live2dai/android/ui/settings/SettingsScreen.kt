package com.live2dai.android.ui.settings

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Link
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import com.live2dai.android.BuildConfig
import com.live2dai.android.data.SettingsRepository

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
        }
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
