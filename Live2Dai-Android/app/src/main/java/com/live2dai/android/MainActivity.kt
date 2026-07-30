package com.live2dai.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.live2dai.android.ui.settings.SettingsScreen
import com.live2dai.android.ui.theme.Live2DaiTheme

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
 * 主聊天界面占位。
 * 包含 Live2D 模型区域、对话气泡、和底部的输入栏。
 * 右上角齿轮按钮 → 设置页。
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MainChatScreen(onOpenSettings: () -> Unit) {
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
            // 底部输入栏占位
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
                        value = "",
                        onValueChange = {},
                        placeholder = { Text("和小喵聊天...") },
                        modifier = Modifier.weight(1f),
                        singleLine = true,
                    )
                    Spacer(Modifier.width(8.dp))
                    FilledTonalButton(onClick = { /* TODO: 发送消息 */ }) {
                        Text("发送")
                    }
                }
            }
        }
    ) { padding ->
        // Live2D 模型 + 对话区域占位
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding),
            contentAlignment = Alignment.Center
        ) {
            Text(
                "Live2D 模型将在此显示",
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}
