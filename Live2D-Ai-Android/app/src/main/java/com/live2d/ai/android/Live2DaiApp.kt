package com.live2d.ai.android

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.live2d.ai.android.data.ModelRegistry
import com.live2d.ai.android.data.SettingsRepository
import com.live2d.ai.android.ui.MainChatScreen
import com.live2d.ai.android.ui.settings.SettingsScreen

/**
 * 应用根组件。
 * 管理主界面与设置界面之间的导航。
 *
 * Live2D 视图引用提升到 App 级：设置页打开时主聊天界面**保持存活**（以覆盖层方式
 * 叠在设置页下方），因此设置页切换形象时 Live2DView 实例仍然存在，可以真正热切换
 * （Live2DView.switchModel），而不是等回到聊天页才重建模型。
 */
@Composable
fun Live2DAiApp(
    /**
     * 运行时设置变更回调（MainActivity 注入）：设置保存后重建运行中 LLM 链接、
     * 刷新 TTS Provider 并更新首选引擎，无需重启即生效。
     */
    onRuntimeSettingsChanged: () -> Unit = {},
) {
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
                onRuntimeSettingsChanged = onRuntimeSettingsChanged,
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
                            modelId = entry.id,
                            onComplete = { onResult(true) },
                            onError = { onResult(false) }
                        )
                    }
                }
            )
        }
    }
}
