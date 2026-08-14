package com.live2dai.android.ui

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
    var inputText: String = "",
    var isLoading: Boolean = false,
    var settingsVisible: Boolean = false,
    var connectionState: ConnectionState = ConnectionState.Disconnected,
) {
    private val _bubbles = mutableListOf<UiBubble>()

    /** 气泡列表（追加序 = 插入序，尾插）。 */
    val bubbles: List<UiBubble> get() = _bubbles.toList()

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
