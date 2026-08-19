package com.live2d.ai.android.render

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Wave 3 · 3-1 AnimationSystem 双轨选择器测试（JVM，纯函数 seam `AnimationSystemBackendSelector`）。
 *
 * 依据: architecture-refactor-render-0814.md M3「AnimationSystem.kt 双轨（flag）」+ 6.1 P3「AnimationSystem.kt
 *       双轨（flag）」+ schedule-refactor-0814.md Wave3-1「AnimationSystem 双轨」+ 背景（coder 将注册官方
 *       updater 到 UpdateScheduler；AnimationSystem 双轨：LEGACY 旧动画 vs ENGINE 官方 updater 链）。
 * 契约锚: 架构 M3 / schedule Wave3-1 双轨；最近锚 ANIM1-A1（物理开关 A/B 对拍前提）与 REND1-A3（A/B 双轨对拍）。
 *
 * 生产 seam 对齐（主代理下发）：
 *   - 枚举面恰好 2 态 LEGACY/ENGINE；`fromFlag(value: String?)` 未知/空回退 LEGACY（大小写不敏感）；
 *   - `DEFAULT` 常量按主代理裁定 = LEGACY（保守口径）；`var backend = LEGACY`。
 *
 * 判定方式: 选择器纯函数 seam（与上波 Wave2-2a1 `BackendSelector` 同构）：
 *             (a) 枚举面恰好 2 态 LEGACY/ENGINE（禁第三态，防漂移）；
 *             (b) 显式 parse（engine/legacy 二态切换，大小写容忍）；
 *             (c) 未知/空 flag → 回退保守后端 LEGACY（双轨可回切兜底，防崩溃；该方向为硬不变量，与默认值无关）；
 *             (d) DEFAULT 恒等于 LEGACY（主代理裁定口径，原「待裁定 ENGINE」断言随裁定反转为 LEGACY）。
 */
class AnimationSystemBackendTest {

    @Test
    fun enum_exposesOnlyLegacyAndEngine() {
        // 枚举面冻结：恰好 LEGACY/ENGINE 两态，禁第三态
        assertEquals(
            "AnimationBackend 必须只有 LEGACY/ENGINE 两态",
            setOf("LEGACY", "ENGINE"), AnimationBackend.values().map { it.name }.toSet(),
        )
    }

    @Test
    fun default_isLegacy() {
        // 主代理裁定：DEFAULT = LEGACY（保守默认；原 RED 断言按 ENGINE 待裁定，本波随裁定反转）
        assertEquals(
            "DEFAULT 动画后端应为 LEGACY（主代理裁定保守口径）",
            AnimationBackend.LEGACY, AnimationSystemBackendSelector.DEFAULT,
        )
        assertEquals(
            "backend 变量初始值 == DEFAULT（LEGACY）",
            AnimationBackend.LEGACY, AnimationSystemBackendSelector.backend,
        )
    }

    @Test
    fun fromFlag_engineParsesToEngine() {
        assertEquals(AnimationBackend.ENGINE, AnimationSystemBackendSelector.fromFlag("engine"))
    }

    @Test
    fun fromFlag_legacyParsesToLegacy() {
        assertEquals(AnimationBackend.LEGACY, AnimationSystemBackendSelector.fromFlag("legacy"))
    }

    @Test
    fun fromFlag_caseInsensitiveTolerant() {
        assertEquals(AnimationBackend.ENGINE, AnimationSystemBackendSelector.fromFlag("ENGINE"))
        assertEquals(AnimationBackend.LEGACY, AnimationSystemBackendSelector.fromFlag("Legacy"))
    }

    @Test
    fun fromFlag_unknownOrEmpty_fallsBackToLegacy() {
        // 未知/空 flag → 保守回退 LEGACY（双轨可回切兜底，防未知 flag 崩溃；硬不变量）
        assertEquals(
            "未知 flag 应回退 LEGACY（保守兜底）",
            AnimationBackend.LEGACY, AnimationSystemBackendSelector.fromFlag("unknown_anim_xyz"),
        )
        assertEquals(
            "空 flag 应回退 LEGACY",
            AnimationBackend.LEGACY, AnimationSystemBackendSelector.fromFlag(""),
        )
    }
}
