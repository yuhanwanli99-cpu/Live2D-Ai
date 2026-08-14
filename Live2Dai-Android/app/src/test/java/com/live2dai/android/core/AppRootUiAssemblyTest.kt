package com.live2dai.android

import com.live2dai.android.ui.MainScreenUiState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * AppRoot 单屏组装 RED 测试（契约 refactor-0814 §2.2/§3 · 架构 shell §3.1，Wave 2C-4）。
 *
 * 依据（唯一真相源 = 契约 + 裁定，**禁读 src/ 实现，禁对答案出题**）：
 *   - C-11=1A（Compose+AndroidView）→ AppRoot 单屏按状态装配 Live2D 宿主 + 聊天浮层
 *   - C-1 保留极简设置页 → 设置面板随 `settingsVisible` 打开
 *   - C-5 ASR 删 → 装配不含 ASR/引导/确认门层
 *
 * **RED 机制**：目标符号 `AppRoot` / `AppLayer` / `AppRootAssembly` 在 src/ 尚不存在（AppRoot.kt 是 C 片
 * 待建）→ 本文件为**编译期 RED**（Unresolved reference）。且依赖 `MainScreenUiState`（同波待建）。
 * coder 实现 seam 后编译全绿 ≡ GREEN（seam spec 见波报告 §7 / 测试用例.md §E）。
 *
 * 范围界定：**纯态装配模型断言**（宿主/浮层/设置三层可见性）；Compose 实际渲染树归 Wave 6 真机 E2E
 * （见波报告 [UNSURE-2]）。`ConnectionState` 为既有符号。
 */
class AppRootUiAssemblyTest {

    // ═══════════════════════════════════════════════════════════════
    // E. AppRoot 组装（纯态装配模型）
    // ═══════════════════════════════════════════════════════════════

    @Test
    fun settingsHidden_assemblesHostAndChatOverlay_only() {
        // E/TC20：设置收起 → 宿主 + 聊天浮层可见，无设置面板
        val state = MainScreenUiState(settingsVisible = false)
        val asm = AppRoot.assemble(state)
        assertTrue("宿主常驻可见（C-11=1A）", asm.layers.contains(AppLayer.LIVE2D_HOST))
        assertTrue("聊天浮层常驻可见（S2 输入/气泡载体）", asm.layers.contains(AppLayer.CHAT_OVERLAY))
        assertTrue("设置收起时无设置面板", !asm.layers.contains(AppLayer.SETTINGS_PANEL))
    }

    @Test
    fun settingsShown_assemblesSettingsPanelOnTop() {
        // E/TC21：设置打开 → 宿主 + 聊天浮层仍常驻，设置面板叠加
        val state = MainScreenUiState(settingsVisible = true)
        val asm = AppRoot.assemble(state)
        assertTrue("设置面板随 settingsVisible 打开（C-1）", asm.layers.contains(AppLayer.SETTINGS_PANEL))
        assertTrue("打开设置时宿主仍可见（浮层层叠）", asm.layers.contains(AppLayer.LIVE2D_HOST))
        assertTrue("打开设置时聊天浮层仍可见", asm.layers.contains(AppLayer.CHAT_OVERLAY))
    }

    @Test
    fun assemble_neverContainsAsrOnboardingConfirmationLayers() {
        // E/TC22 / C-5 / DEL1-A1：装配层枚举不含 ASR/引导/确认门层
        for (settingsVis in listOf(false, true)) {
            val state = MainScreenUiState(settingsVisible = settingsVis)
            val names = AppRoot.assemble(state).layers.map { it.name.lowercase() }
            assertTrue(
                "装配不得含 asr/onboarding/confirmation 层，实得: $names",
                names.none { n -> n.contains("asr") || n.contains("onboard") || n.contains("confirm") },
            )
        }
    }

    @Test
    fun settingsToggle_flipsOnlySettingsPanelLayer() {
        // E/TC23：开→收 仅翻转 SETTINGS_PANEL，宿主/浮层不变
        val closed = AppRoot.assemble(MainScreenUiState(settingsVisible = false))
        val opened = AppRoot.assemble(MainScreenUiState(settingsVisible = true))
        assertTrue("宿主在开/收两态均可见", closed.layers.contains(AppLayer.LIVE2D_HOST))
        assertTrue("聊天浮层在开/收两态均可见", closed.layers.contains(AppLayer.CHAT_OVERLAY))
        assertTrue("仅打开状态含设置面板", opened.layers.contains(AppLayer.SETTINGS_PANEL))
    }

    @Test
    fun assembly_isPureFunctionOfState() {
        // 装配是状态的纯函数：同一状态 → 同一装配；不同状态 → 装配按 settingsVisible 区分
        val s1 = MainScreenUiState(settingsVisible = true)
        val s2 = MainScreenUiState(settingsVisible = true)
        val s3 = MainScreenUiState(settingsVisible = false)
        assertEquals("同状态装配确定性一致", AppRoot.assemble(s1), AppRoot.assemble(s2))
        assertTrue(
            "settingsVisible 区分装配",
            AppRoot.assemble(s1).layers != AppRoot.assemble(s3).layers,
        )
    }
}
