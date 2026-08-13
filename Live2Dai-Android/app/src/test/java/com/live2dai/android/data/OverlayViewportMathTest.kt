package com.live2dai.android.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * R15 布局大改造 —— 浮层模式下键盘/视口不挤压角色区（布局参数计算纯函数，RED，契约见 T7-layout-red.md）。
 *
 * 依据：需求真相 translation.md（D6 键盘视口固定：adjustNothing + 浮层，键盘浮在模型上）
 *  + R15（角色全屏 + 聊天浮层，键盘弹出聊天上移不挤压角色区）。
 *
 * 锁定的目标接口（**新增，现状不存在 → 编译期 RED 属预期**）：
 *   fun characterRegionHeight(
 *       mode: UiLayoutMode, screenHeight: Float, imeHeight: Float, chatHeight: Float
 *   ): Float
 *
 * 语义（纯函数，与具体 View / XML 解耦）：
 *   - FULLSCREEN_CHAT_OVERLAY（浮层模式）：角色渲染区高度 = screenHeight（全屏），
 *     无论 IME（imeHeight）是否弹出、聊天区（chatHeight）多高——聊天浮层叠在角色上，
 *     键盘弹出聊天上移，**不挤压角色区**（需求 1/4）。
 *   - COLUMN_SPLIT（旧分栏模式）：角色区 = screenHeight - chatHeight - imeHeight   （旧压缩语义，
 *     聊天与键盘各自占用纵向空间，最低 0）。≥ 0。
 *
 * 需求映射（机械断言）：
 *   需求4「浮层模式下 imePadding 逻辑不挤压角色区」 → 在 FULLSCREEN_CHAT_OVERLAY 下，
 *   imeHeight 变化时 characterRegionHeight 恒等于 screenHeight（对 imeHeight 不敏感）。
 *   对比 COLUMN_SPLIT 下 imeHeight 增大 → 角色区高度减小（旧行为被替换）。
 *
 * GREEN 条件（coder R15 实现）：新增 data/OverlayViewportMath.kt，实现上述纯函数。
 */
class OverlayViewportMathTest {

    private val screenH = 2400f

    // ── 需求1/4：浮层模式角色区恒占全屏高（对 IME/聊天区高度不敏感，不被挤压）──

    @Test
    fun overlay_characterRegion_alwaysFullScreenRegardlessOfIme() {
        // 无键盘：全屏
        val noIme = characterRegionHeight(
            UiLayoutMode.FULLSCREEN_CHAT_OVERLAY, screenH, 0f, 300f
        )
        // 键盘弹出（ime 加大）：仍全屏，不挤压
        val imeKb = characterRegionHeight(
            UiLayoutMode.FULLSCREEN_CHAT_OVERLAY, screenH, 500f, 300f
        )
        val imeBig = characterRegionHeight(
            UiLayoutMode.FULLSCREEN_CHAT_OVERLAY, screenH, 900f, 300f
        )
        assertEquals(screenH, noIme, 0.0f)
        assertEquals("浮层模式：键盘弹出角色区不得缩小（不挤压角色区）", screenH, imeKb, 0.0f)
        assertEquals("浮层模式：大键盘角色区仍不得缩小", screenH, imeBig, 0.0f)
    }

    @Test
    fun overlay_characterRegion_independentOfChatHeight() {
        // 聊天浮层高度变化不改变角色区高度（浮层叠在角色上）
        val smallChat = characterRegionHeight(UiLayoutMode.FULLSCREEN_CHAT_OVERLAY, screenH, 0f, 200f)
        val bigChat = characterRegionHeight(UiLayoutMode.FULLSCREEN_CHAT_OVERLAY, screenH, 0f, 800f)
        assertEquals(screenH, smallChat, 0.0f)
        assertEquals("浮层模式：聊天浮层扩大不挤压角色区", screenH, bigChat, 0.0f)
    }

    // ── 旧 COLUMN_SPLIT 语义（回归锁：键盘增大 → 角色区缩小 → 被浮层替换）──

    @Test
    fun split_characterRegion_shrinksWithIme() {
        val noIme = characterRegionHeight(
            UiLayoutMode.COLUMN_SPLIT, screenH, 0f, 400f
        )
        val withIme = characterRegionHeight(
            UiLayoutMode.COLUMN_SPLIT, screenH, 600f, 400f
        )
        assertTrue(
            "旧分栏模式：无键盘时角色区 = screen - chat",
            noIme == screenH - 400f
        )
        assertTrue(
            "旧分栏模式：键盘弹出角色区缩小（旧压缩行为，被浮层替换）",
            withIme < noIme
        )
        // 非负
        assertTrue(withIme >= 0f)
    }

    @Test
    fun split_characterRegion_neverNegative() {
        val height = characterRegionHeight(UiLayoutMode.COLUMN_SPLIT, 1000f, 900f, 900f)
        assertTrue("角色区高度不得为负", height >= 0f)
    }
}
