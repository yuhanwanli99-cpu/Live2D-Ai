package com.live2d.ai.android

import com.live2d.ai.android.supersample.FboPhase
import com.live2d.ai.android.supersample.SupersampleFboStateMachine
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Wave 2 · 2A-2 超采样接入（FBO 包裹 + blit 交接）—— FBO 状态机测试（JVM seam，无 GL）。
 *
 * 依据: architecture-refactor-render-0814.md U5 设计注记 + §4.2 每帧路径 + §6.2/6.1 P2 超采样段。
 * 契约锚:
 *   - U5 注记（锁定）: "外层 FBO 交接由薄壳自己保存/恢复（与现 C8h 代码同责）"——
 *     FRAMEBUFFER_BINDING 与 VIEWPORT 不在官方 RendererProfile Save/Restore 里，
 *     超采样外层 FBO 的「进入前保存、退出后还原」对称性由薄壳负责。
 *   - §4.2 每帧路径: 超采样分支（B2）"bind 2K FBO → engineDraw() → mipmap → blit"。
 *   - §4.2/pipeline 图: 每帧帧序 = lipSync drain → update(dt) → draw；超采样 FBO 包 draw。
 *   - 架构 §1.C/§4.2 (B2): bind 2K FBO → 官方 DrawModel → blit 回 1080p（清晰度硬指标载体）；
 *     状态序 bind→render→blit→restore，FRAMEBUFFER_BINDING/VIEWPORT 必须对称恢复。
 *
 * 判定方式: 纯 JVM 无 GL 状态机断言（[UNSURE-A] 的机器可断言面）。状态转换是纯函数，
 *          不触 GL 上下文（GL glGet/glBind 序列的真机证据归 REND2-A1/A2，device，Wave 6）。
 *
 * RED 依据（如实）: `SupersampleFboStateMachine` / `FboPhase` seam 类当前**不存在**
 *          （超采样段尚待 M2 coder 自 C8h 迁入薄壳并抽纯函数状态机）→ 本文件编译期引用它们
 *          → `:app:testDebugUnitTest` 整包编译失败 = RED（编译期 RED，同 M1 JNI / T7 layout seam 先例）。
 *
 * seam 形态（草案，供 coder）: 见 wave2-2a2-red.md §3 签名映射表。状态机只建模
 *          「帧内 FboPhase 序列合法性」+「bind/restore 对称不变量」，是可在 JVM 验证的纯函数；
 *          真 GL 交接（glBindFramebuffer/glGetIntegerv FRAMEBUFFER_BINDING/glViewport）由薄壳驱动
 *          该状态机并在进入/退出时调用同一守卫（保存前状态 → 退出后还原），非本测试范围。
 */
class SupersampleFboStateMachineTest {

    /** 契约锁定帧序: IDLE → BIND → RENDER → BLIT → RESTORE → 回 BIND(下一帧) 的规整序列。 */
    private val canonicalFrame = listOf(
        FboPhase.BIND, FboPhase.RENDER, FboPhase.BLIT, FboPhase.RESTORE,
    )

    // ────────────────────────────────────────────────────────────
    // REND2-A1 超采样路径 = bind 2K FBO → render → blit → restore，每帧必经
    // ────────────────────────────────────────────────────────────

    @Test
    fun bind_render_blit_restore_is_valid_frame_sequence() {
        val phases = listOf(
            FboPhase.IDLE,
            *canonicalFrame.toTypedArray(),
            FboPhase.IDLE,
        )
        assertTrue(
            "RED [wave2-2a2-FSM-A1]: 契约每帧路径（bind 2K FBO → render → blit）应是合法序列。" +
                "SupersampleFboStateMachine 当前不存在（编译期 RED）。\n" +
                "契约锚: 架构 §4.2 超采样分支 'bind 2K FBO → engineDraw() → mipmap → blit' + " +
                "U5 注记（FRAMEBUFFER_BINDING/VIEWPORT 由薄壳保存恢复）。",
            SupersampleFboStateMachine.isValidFrameSequence(phases),
        )
    }

    @Test
    fun transition_sequence_matches_contract_order() {
        // 逐边推进: IDLE→BIND→RENDER→BLIT→RESTORE，每边都应被接受（合法的帧前进序）
        var current = FboPhase.IDLE
        for (expected in canonicalFrame) {
            val next = SupersampleFboStateMachine.transition(current, expected)
            assertEquals(
                "RED [wave2-2a2-FSM-A2]: 状态机必须接受契约规定的帧内转换 " +
                    "$current → $expected。当前 seam 不存在（编译期 RED）。\n" +
                    "契约锚: 架构 §4.2 bind→engineDraw→blit→restore 帧序。",
                expected,
                next,
            )
            current = next
        }
        // 一帧结束后回 IDLE（下一帧从 BIND 重来）——帧间对称
        assertEquals(
            "RED [wave2-2a2-FSM-A2b]: RESTORE 后应可由薄壳驱动回 IDLE（帧间不残留 bound 态）。" +
                "当前 seam 不存在（编译期 RED）。",
            FboPhase.IDLE,
            SupersampleFboStateMachine.transition(current, FboPhase.IDLE),
        )
    }

    // ────────────────────────────────────────────────────────────
    // U5 不变量: bind/restore 对称性（进入前保存、退出后还原 FRAMEBUFFER_BINDING/VIEWPORT）
    // ────────────────────────────────────────────────────────────

    @Test
    fun bind_and_restore_are_symmetric_pair() {
        // RESTORE 只允许紧跟 BLIT（先渲染并 blit 完，才还原外层 FBO/视口）。
        // 对称不变量: 每个 BIND 必须有且仅有一个对应 RESTORE，且其间不出现第二个 BIND/RESTORE 错位。
        assertTrue(
            "RED [wave2-2a2-FSM-B1]: 状态机必须保证 bind/restore 对称（每 BIND 恰一 RESTORE，序=BIND…RESTORE）。" +
                "当前 seam 不存在（编译期 RED）。\n" +
                "契约锚: U5 注记 '外层 FBO 交接由薄壳自己保存/恢复'（对称不变量）。",
            SupersampleFboStateMachine.hasSymmetricBindRestore(canonicalFrame),
        )
    }

    @Test
    fun bind_without_matching_restore_is_invalid() {
        // 缺 restore（渲染完不还原 FRAMEBUFFER_BINDING/VIEWPORT）→ 状态机必须判非对称（fail-safe）。
        val unbalanced = listOf(
            FboPhase.BIND, FboPhase.RENDER, FboPhase.BLIT, // 无 RESTORE
        )
        assertTrue(
            "RED [wave2-2a2-FSM-B2]: 状态机必须识别 bind 无配对 restore 为非对称（禁止泄漏外层 GL 状态）。" +
                "当前 seam 不存在（编译期 RED）。\n" +
                "契约锚: U5 注记（保存/恢复对称性）——缺失还原 = FRAMEBUFFER_BINDING/VIEWPORT 泄漏到主表面。",
            SupersampleFboStateMachine.isImbalancedBind(unbalanced),
        )
    }

    @Test
    fun unsampled_restore_without_bind_is_invalid() {
        // 凭空 restore（无对应 bind）→ 非法（fail-safe，防薄壳错序）
        val strayRestore = listOf(
            FboPhase.IDLE, FboPhase.RESTORE,
        )
        assertTrue(
            "RED [wave2-2a2-FSM-B3]: 状态机必须识别无 bind 的游散 restore 为非法。" +
                "当前 seam 不存在（编译期 RED）。\n" +
                "契约锚: U5 对称性 + 泛化 fail-safe（异序不得把 GL 状态改坏）。",
            SupersampleFboStateMachine.rejectsStrayRestore(strayRestore),
        )
    }

    // ────────────────────────────────────────────────────────────
    // 非法转移 fail-safe（泛化自契约 §2.1 I7 语义：异常转移收敛回 IDLE）——超采样帧序的局部版本
    // ────────────────────────────────────────────────────────────

    @Test
    fun invalid_edge_never_enters_gl_dirty_state() {
        // 尝试从 RESTORE 直接 RENDER（跳步）应被拒绝或回落 IDLE，不得停留在"绑定/视口未还原"区间。
        val next = try {
            SupersampleFboStateMachine.transition(FboPhase.RESTORE, FboPhase.RENDER)
        } catch (expected: IllegalArgumentException) {
            // 允许抛异常表达非法边（fail-safe），等同拒绝
            FboPhase.IDLE
        }
        assertTrue(
            "RED [wave2-2a2-FSM-F1]: 非法边（RESTORE→RENDER 跳步）必须被拒绝或回落 IDLE，" +
                "不得进入未还原的可绘制态。当前 seam 不存在（编译期 RED）。\n" +
                "契约锚: §2.1 I7 fail-safe 语义在超采样帧序的局部化 + U5 GL 状态不得泄漏。",
            next == FboPhase.IDLE || next != FboPhase.RENDER,
        )
    }
}
