package com.live2d.ai.android

import com.live2d.ai.android.ui.MainScreenUiState

/**
 * 应用层枚举（2C-4 seam，契约 §1.3 C-11/C-15 删除面）。
 *
 * 最小壳三层：恒含 [LIVE2D_HOST]+[CHAT_OVERLAY]，[SETTINGS_PANEL] 随设置页可见开关。
 * 不含 asr / onboard / confirm 层（ASR/引导/确认门已随 2C-4 删除）。
 */
enum class AppLayer {
    /** Live2D 角色渲染宿主层（恒含）。 */
    LIVE2D_HOST,

    /** 聊天浮层（输入 + 气泡 + 连接状态，恒含）。 */
    CHAT_OVERLAY,

    /** 设置面板层（随 settingsVisible）。 */
    SETTINGS_PANEL,
}

/**
 * 应用根装配结果（2C-4 seam）：恒含 LIVE2D_HOST + CHAT_OVERLAY；
 * SETTINGS_PANEL 随 settingsVisible 加入。
 */
data class AppRootAssembly(val layers: Set<AppLayer>) {
    val hasLive2DHost: Boolean get() = AppLayer.LIVE2D_HOST in layers
    val hasChatOverlay: Boolean get() = AppLayer.CHAT_OVERLAY in layers
    val hasSettingsPanel: Boolean get() = AppLayer.SETTINGS_PANEL in layers
}

/**
 * 应用根纯态装配（2C-4 seam，纯 JVM 可测）。
 *
 * [assemble] 恒含 LIVE2D_HOST + CHAT_OVERLAY；SETTINGS_PANEL 随 settingsVisible。
 * 恒不含 asr / onboard / confirm 层（删除结构面）。
 */
object AppRoot {
    /** 纯态装配：按 [MainScreenUiState.settingsVisible] 决定 SETTINGS_PANEL 层。 */
    fun assemble(state: MainScreenUiState): AppRootAssembly = assemble(state.settingsVisible)

    /** 纯态装配：按 settingsVisible 决定 SETTINGS_PANEL 层。 */
    fun assemble(settingsVisible: Boolean): AppRootAssembly {
        val layers = mutableSetOf(AppLayer.LIVE2D_HOST, AppLayer.CHAT_OVERLAY)
        if (settingsVisible) {
            layers.add(AppLayer.SETTINGS_PANEL)
        }
        return AppRootAssembly(layers)
    }
}
