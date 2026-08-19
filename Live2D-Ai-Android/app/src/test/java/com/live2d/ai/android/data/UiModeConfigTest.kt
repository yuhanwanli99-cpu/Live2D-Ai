package com.live2d.ai.android.data

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * R15 布局大改造（角色全屏 + 聊天浮层）—— 布局模式状态配置测试（RED，契约见 T7-layout-red.md）。
 *
 * 依据：需求真相 translation.md（R15 待补：角色全屏 + 聊天浮层 + UI 简化 + 定时提醒移除）
 *  + T7 测试契约（本批次主代理翻译层）。
 *
 * 锁定的目标接口（**新增，现状不存在 → 编译期 RED 属预期**）：
 *   class UiModeStore(context: Context)   // data/UiModeStore.kt（Robolectric 可测，SharedPreferences 持久化）
 *       fun load(): UiLayoutMode          // key = "layout_mode"，无 key → 默认浮层模式
 *       fun save(mode: UiLayoutMode)
 *   enum UiLayoutMode { FULLSCREEN_CHAT_OVERLAY, COLUMN_SPLIT }
 *   object UiModeConfig {                  // data/UiModeConfig.kt（纯函数，可 JVM 测）
 *       fun defaultMode(): UiLayoutMode             // == FULLSCREEN_CHAT_OVERLAY
 *       fun isOverlayMode(mode: UiLayoutMode): Boolean
 *       fun toggleLayoutMode(cur: UiLayoutMode): UiLayoutMode  // 两态互切
 *       const val STORAGE_KEY = "layout_mode"
 *   }
 *
 * 需求映射：
 *   需求1「角色渲染区占满全屏 + 聊天浮层」 + 需求2「布局模式状态：FULLSCREEN_CHAT_OVERLAY vs 旧
 *   COLUMN_SPLIT、默认值＝浮层」 → 默认 layoutMode = FULLSCREEN_CHAT_OVERLAY。
 *   切换逻辑纯函数 → toggleLayoutMode / isOverlayMode。
 *   持久化 → UiModeStore load/save roundtrip；默认（无 key）＝ 浮层模式。
 *
 * GREEN 条件（coder R15 实现）：
 *   - 新增 data/UiModeStore.kt + data/UiModeConfig.kt（含 UiLayoutMode 枚举）。
 *   - STORAGE_KEY = "layout_mode"（SharedPreferences name = "live2dai_ui_mode"）。
 *   - 默认（无存储）为 FULLSCREEN_CHAT_OVERLAY。
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class, sdk = [34])
class UiModeConfigTest {

    private val context: Context = ApplicationProvider.getApplicationContext()

    /** 契约锁定的 SharedPreferences name（由实现的 UiModeStore 静态常量读取，测试不硬编码存储名）。 */
    private fun storeOf() = UiModeStore(context)

    @Before
    fun setUp() {
        // 清空 ui mode 存储，从干净状态开始
        context.getSharedPreferences("live2dai_ui_mode", Context.MODE_PRIVATE)
            .edit().clear().commit()
    }

    // ── 需求2：默认布局模式 = 浮层模式（FULLSCREEN_CHAT_OVERLAY）──

    @Test
    fun defaultMode_isFullscreenChatOverlay() {
        assertTrue(
            "需求2：布局默认模式必须是浮层模式 FULLSCREEN_CHAT_OVERLAY（角色全屏 + 聊天浮层）。" +
                "现状：UiModeConfig/UiLayoutMode 不存在 → 编译期 RED（属预期）。",
            UiModeConfig.defaultMode() == UiLayoutMode.FULLSCREEN_CHAT_OVERLAY
        )
    }

    // ── 需求2：无存储（冷启动）→ load 返回默认浮层模式 ──

    @Test
    fun store_load_withNoKey_returnsOverlay() {
        val mode = storeOf().load()
        assertEquals(
            "需求2：未做过布局模式切换时，load() 应等于默认浮层模式（角色全屏）。RED：UiModeStore 不存在。",
            UiLayoutMode.FULLSCREEN_CHAT_OVERLAY,
            mode
        )
    }

    // ── 需求2：切换逻辑纯函数（isOverlayMode / toggleLayoutMode 互切）──

    @Test
    fun isOverlayMode_returnsTrueOnlyForOverlay() {
        assertTrue(UiModeConfig.isOverlayMode(UiLayoutMode.FULLSCREEN_CHAT_OVERLAY))
        assertTrue(!UiModeConfig.isOverlayMode(UiLayoutMode.COLUMN_SPLIT))
    }

    @Test
    fun toggleLayoutMode_switchesBetweenTwoModes() {
        assertEquals(UiLayoutMode.COLUMN_SPLIT, UiModeConfig.toggleLayoutMode(UiLayoutMode.FULLSCREEN_CHAT_OVERLAY))
        assertEquals(UiLayoutMode.FULLSCREEN_CHAT_OVERLAY, UiModeConfig.toggleLayoutMode(UiLayoutMode.COLUMN_SPLIT))
    }

    // ── 需求2：布局模式持久化（save → load roundtrip / 重启保持）──

    @Test
    fun store_save_split_thenLoad_returnsSplit() {
        storeOf().save(UiLayoutMode.COLUMN_SPLIT)
        // 重新创建实例（模拟重启），值应仍为 COLUMN_SPLIT
        val reloaded = storeOf().load()
        assertEquals(UiLayoutMode.COLUMN_SPLIT, reloaded)
    }

    @Test
    fun store_save_overlay_thenLoad_returnsOverlay() {
        storeOf().save(UiLayoutMode.FULLSCREEN_CHAT_OVERLAY)
        assertEquals(UiLayoutMode.FULLSCREEN_CHAT_OVERLAY, storeOf().load())
    }
}
