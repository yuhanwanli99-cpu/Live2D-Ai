package com.live2dai.android.data

/**
 * UI 布局模式（R15 布局大改造：角色全屏 + 聊天浮层）。
 *
 * - [FULLSCREEN_CHAT_OVERLAY]：角色渲染区占满全屏，聊天面板以半透明浮层叠在角色之上
 *   （输入框常驻底部；键盘弹出时聊天浮层整体上移，角色区不挤压）。R15 默认模式。
 * - [COLUMN_SPLIT]：旧「纵向压缩」分栏语义（角色区 = 屏高 - 聊天区 - 键盘），被浮层模式取代。
 *
 * 契约：T7-layout-red.md §4.1（签名/枚举值名由 tester 锁定，禁止改名）。
 */
enum class UiLayoutMode { FULLSCREEN_CHAT_OVERLAY, COLUMN_SPLIT }

/**
 * 布局模式纯逻辑（R15 新增，T7 契约 §4.1）：
 * 默认值、判定、两态互切。无 IO / 无 Android 依赖，JVM 可单测。
 */
object UiModeConfig {
    /** UI 布局模式持久化 key（UiModeStore 落盘用；契约锁定此值） */
    const val STORAGE_KEY = "layout_mode"

    /** R15 默认布局模式 = 浮层（角色全屏 + 聊天浮层） */
    fun defaultMode(): UiLayoutMode = UiLayoutMode.FULLSCREEN_CHAT_OVERLAY

    /** 是否为浮层模式（真值表：仅 FULLSCREEN_CHAT_OVERLAY 为 true） */
    fun isOverlayMode(mode: UiLayoutMode): Boolean =
        mode == UiLayoutMode.FULLSCREEN_CHAT_OVERLAY

    /** 两态互切：浮层 ↔ 分栏 */
    fun toggleLayoutMode(current: UiLayoutMode): UiLayoutMode = when (current) {
        UiLayoutMode.FULLSCREEN_CHAT_OVERLAY -> UiLayoutMode.COLUMN_SPLIT
        UiLayoutMode.COLUMN_SPLIT -> UiLayoutMode.FULLSCREEN_CHAT_OVERLAY
    }
}
