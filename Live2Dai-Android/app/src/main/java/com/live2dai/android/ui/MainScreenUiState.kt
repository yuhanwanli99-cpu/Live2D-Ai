package com.live2dai.android.ui

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.live2dai.android.ConnectionState

/**
 * 聊天气泡（2C-4 seam）：密封类 3 变体 = 用户 / 助手 / 错误。
 *
 * 由 [MainScreenUiState] 持有；气泡追加序 = 插入序（尾插）。
 */
sealed interface UiBubble {
    data class User(val text: String) : UiBubble
    data class Assistant(val text: String) : UiBubble
    data class Error(val text: String) : UiBubble
}

/**
 * 主界面 UI 状态（2C-4 seam，纯 JVM 可测）。
 *
 * - 气泡追加序 = 插入序（尾插）：[uiAppendUser] / [uiAppendAssistant] / [uiAppendError]
 * - 发送使能 [uiCanSend]：输入 isNotBlank 才可发、isLoading 禁发送
 * - 设置页可见开关：[settingsVisible] / [uiSetSettingsVisible]
 * - 连接状态流入：[connectionState]
 *
 * 删除结构面（ASR / 引导 / 确认门）：本类不含 isListening / onboarding / confirmation
 * 任何字段或方法（对应 UI 组件已随 2C-4 从 MainScreen 删除）。
 */
class MainScreenUiState(
    inputText: String = "",
    isLoading: Boolean = false,
    settingsVisible: Boolean = false,
    connectionState: ConnectionState = ConnectionState.Disconnected,
    fpsOverlayVisible: Boolean = true,
) {
    // Wave 4.1 修复 1：Compose 可观察（真机输入无响应根因）——
    // 输入框 typing 不重组 / 发送按钮 enabled 恒 false / 气泡不刷新，
    // 均因本类原先为普通 var（无 Compose 状态）。改 mutableStateOf 后
    // 读取方（MainScreen）在状态变化时自动重组。
    var inputText: String by mutableStateOf(inputText)
    var isLoading: Boolean by mutableStateOf(isLoading)
    var settingsVisible: Boolean by mutableStateOf(settingsVisible)
    var connectionState: ConnectionState by mutableStateOf(connectionState)

    /** FPS 角标显示开关（用户需求①；默认 true = 显示）。 */
    var fpsOverlayVisible: Boolean by mutableStateOf(fpsOverlayVisible)

    /** 气泡列表（追加序 = 插入序，尾插；SnapshotStateList，Compose 可观察）。 */
    private val _bubbles = mutableStateListOf<UiBubble>()

    /** 气泡列表（追加序 = 插入序，尾插）。 */
    val bubbles: List<UiBubble> get() = _bubbles

    /** 发送使能（读状态字段）：isNotBlank 才可发、isLoading 禁发送。 */
    val uiCanSend: Boolean get() = inputText.isNotBlank() && !isLoading

    /** 发送使能（纯函数形态）：isNotBlank 才可发、isLoading 禁发送。 */
    fun uiCanSend(inputText: String, isLoading: Boolean): Boolean =
        inputText.isNotBlank() && !isLoading

    /** 追加用户气泡（尾插）。 */
    fun uiAppendUser(text: String) {
        _bubbles.add(UiBubble.User(text))
    }

    /** 追加助手气泡（尾插）。 */
    fun uiAppendAssistant(text: String) {
        _bubbles.add(UiBubble.Assistant(text))
    }

    /** 追加错误气泡（尾插）。 */
    fun uiAppendError(text: String) {
        _bubbles.add(UiBubble.Error(text))
    }

    /** 设置页可见开关。 */
    fun uiSetSettingsVisible(visible: Boolean) {
        settingsVisible = visible
    }

    /** 整体替换气泡列表（生产接线用；追加序 = 传入序）。 */
    fun uiSetBubbles(items: List<UiBubble>) {
        _bubbles.clear()
        _bubbles.addAll(items)
    }
}
