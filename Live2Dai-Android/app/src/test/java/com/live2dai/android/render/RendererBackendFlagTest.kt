package com.live2dai.android.render

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Wave 2 · 2A-1 渲染替换 —— 双轨回切 build flag 测试。
 *
 * 依据: architecture-refactor-render-0814.md M2「Live2DRenderer→Live2DEngineRenderer 双轨期
 *       由 build flag 切换」+ §6.1 P2「双轨并存（flag），回归失败可回切」
 *       + schedule-refactor-0814.md Wave2A「双轨回切 build flag 测试」。
 * 契约锚: architecture M2 / schedule Wave2A「双轨回切 build flag 测试」；
 *         最近 REND 锚 REND1-A3（重构前后 A/B 帧率对拍的回切前提）。
 *
 * 判定方式: 渲染路径选择器纯函数 seam `BackendSelector`（object）：
 *             (k) 默认值稳定（主代理裁决：默认 LEGACY，双轨期保守后端，与生产
 *                 BackendSelector.backend 初始值一致）；
 *             (l) 显式 switch（engine/legacy 二态切换）；
 *             (m) 未知/空 flag → 回退保守后端（LEGACY，双轨可回切兜底，不能崩，与生产
 *                 fromValue 的 else 分支一致）。
 *           枚举 `RendererBackend`（LEGACY/ENGINE）+ `BackendSelector.fromValue/backend`。
 */
class RendererBackendFlagTest {

    @Test
    fun defaultBackend_isLegacy() {
        assertEquals(
            "默认渲染后端应为 LEGACY（主代理裁决：Wave2 双轨期默认保守后端）",
            RendererBackend.LEGACY, BackendSelector.backend,
        )
    }

    @Test
    fun fromValue_engineParsesToEngine() {
        assertEquals(RendererBackend.ENGINE, BackendSelector.fromValue("engine"))
    }

    @Test
    fun fromValue_legacyParsesToLegacy() {
        assertEquals(RendererBackend.LEGACY, BackendSelector.fromValue("legacy"))
    }

    @Test
    fun fromValue_caseInsensitiveTolerant() {
        assertEquals(RendererBackend.ENGINE, BackendSelector.fromValue("ENGINE"))
        assertEquals(RendererBackend.LEGACY, BackendSelector.fromValue("Legacy"))
    }

    @Test
    fun fromValue_unknownOrEmpty_fallsBackToLegacy() {
        // 未知值 -> 保守回退 LEGACY（双轨可回切的兜底，防未知 flag 崩溃；与生产 fromValue else 分支一致）
        assertEquals(
            "未知 flag 应回退 LEGACY（保守兜底）",
            RendererBackend.LEGACY, BackendSelector.fromValue("unknown_backend_xyz"),
        )
        assertEquals(
            "空 flag 应回退 LEGACY",
            RendererBackend.LEGACY, BackendSelector.fromValue(""),
        )
    }

    @Test
    fun enum_exposesOnlyLegacyAndEngine() {
        // 枚举面冻结：恰好 2 态，禁加第三态（防漂移）
        assertEquals(
            "RendererBackend 必须只有 LEGACY/ENGINE 两态",
            setOf("LEGACY", "ENGINE"), RendererBackend.values().map { it.name }.toSet(),
        )
    }
}
