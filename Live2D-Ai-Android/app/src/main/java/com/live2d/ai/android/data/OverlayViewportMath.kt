package com.live2d.ai.android.data

/**
 * 角色渲染区高度计算（R15 新增，T7 契约 §4.3，纯函数）。
 *
 * 语义：
 * - [UiLayoutMode.FULLSCREEN_CHAT_OVERLAY]（R15 默认）：角色区**恒等于屏高**，
 *   对 imeHeight / chatHeight 不敏感——键盘弹出时聊天浮层整体上移，角色区不挤压。
 * - [UiLayoutMode.COLUMN_SPLIT]（旧分栏）：角色区 = max(screenHeight - chatHeight - imeHeight, 0f)，
 *   键盘弹出时角色区缩小（旧纵向压缩语义，被浮层替换）。
 *
 * @param screenHeight 屏幕（窗口）高度 px
 * @param imeHeight 输入法键盘高度 px（未弹出 = 0f）
 * @param chatHeight 聊天面板高度 px
 * @return 角色渲染区高度 px（恒 ≥ 0f）
 */
fun characterRegionHeight(
    mode: UiLayoutMode,
    screenHeight: Float,
    imeHeight: Float,
    chatHeight: Float,
): Float = when (mode) {
    UiLayoutMode.FULLSCREEN_CHAT_OVERLAY -> screenHeight
    UiLayoutMode.COLUMN_SPLIT -> maxOf(screenHeight - chatHeight - imeHeight, 0f)
}
