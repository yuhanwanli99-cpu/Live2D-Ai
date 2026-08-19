package com.live2d.ai.android.data

import android.content.Context
import android.content.SharedPreferences

/**
 * UI 布局模式持久化（R15 新增，T7 契约 §4.2）。
 *
 * SharedPreferences：name = "live2dai_ui_mode"（MODE_PRIVATE），
 * key = [UiModeConfig.STORAGE_KEY]（"layout_mode"）。
 *
 * 语义：
 * - 无 key → [UiModeConfig.defaultMode()]（浮层）；
 * - 损坏/未知值 → 默认浮层（不崩溃）；
 * - save 后 load roundtrip 保持。
 */
class UiModeStore(context: Context) {

    private val prefs: SharedPreferences =
        context.getSharedPreferences(PREF_NAME, Context.MODE_PRIVATE)

    /** 读取持久化的布局模式；无 key / 损坏 / 未知 → 默认浮层 */
    fun load(): UiLayoutMode {
        val raw = prefs.getString(UiModeConfig.STORAGE_KEY, null) ?: return UiModeConfig.defaultMode()
        return when (raw) {
            UiLayoutMode.FULLSCREEN_CHAT_OVERLAY.name -> UiLayoutMode.FULLSCREEN_CHAT_OVERLAY
            UiLayoutMode.COLUMN_SPLIT.name -> UiLayoutMode.COLUMN_SPLIT
            else -> UiModeConfig.defaultMode()
        }
    }

    /** 持久化布局模式 */
    fun save(mode: UiLayoutMode) {
        prefs.edit().putString(UiModeConfig.STORAGE_KEY, mode.name).apply()
    }

    private companion object {
        const val PREF_NAME = "live2dai_ui_mode"
    }
}
