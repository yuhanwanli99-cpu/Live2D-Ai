package com.live2dai.android.supersample

/**
 * 超采样 FBO 帧序状态机（seam，纯 JVM，无 GL）。
 *
 * 状态枚举（契约 REND2-A1 / architecture-refactor-0814 §4.2 每帧路径
 * 「bind 2K FBO → engineDraw → mipmap → blit」+ U5 注记「FRAMEBUFFER_BINDING/VIEWPORT
 * 由薄壳保存-恢复」）：
 * ```
 * IDLE → BIND → RENDER → BLIT → RESTORE → IDLE
 * ```
 * 帧内合法序 = IDLE→BIND→RENDER→BLIT→RESTORE→IDLE（每帧恰一轮，BIND/RESTORE 对称：
 * 每 BIND 恰一 RESTORE；缺 restore / 游散 restore 判非法；非法边 fail-safe 回落 IDLE）。
 *
 * 动作层 vs 阶段层：
 * - [FboAction]（动作）只有 5 个：BIND / RENDER / BLIT / RESTORE / RESET（RESET 为显式中止/强制复位，
 *   任意阶段 → IDLE，用于 surface 销毁 / GL 上下文丢失等中止场景）；
 * - RESTORE 动作从 [FboPhase.BLIT] 一步完成"恢复默认 framebuffer 并回到 IDLE"（同步 GL 调用语义，
 *   阶段环中的 RESTORE 阶段由该动作隐式走过，动作返回即回 [FboPhase.IDLE]）；
 * - 非法转移不抛异常：回落 [FboPhase.IDLE]（本实现选"回落 IDLE"而非"保持原态"，与
 *   「非法边 fail-safe 回 IDLE」口径一致）。
 */
enum class FboPhase { IDLE, BIND, RENDER, BLIT, RESTORE }

/** 超采样 FBO 转移动作。 */
enum class FboAction { BIND, RENDER, BLIT, RESTORE, RESET }

/** [FboAction] 的别名（供按动作语义引用的调用方使用，类型相同）。 */
typealias SupersampleAction = FboAction

/**
 * 超采样 FBO 状态机实例：持有当前 [phase]，[transition] 为带状态的转移入口。
 *
 * 非法边（含缺 restore 的 re-bind、游散 restore、乱序 RENDER/BLIT 等）一律 fail-safe
 * 回落 [FboPhase.IDLE]，不抛异常。纯转移表见 [nextPhase]（companion，可无实例使用）。
 */
class SupersampleFboStateMachine(initialPhase: FboPhase = FboPhase.IDLE) {

    /** 当前阶段（仅经 [transition] 变更）。 */
    var phase: FboPhase = initialPhase
        private set

    /**
     * 执行一次转移：按纯转移表 [nextPhase] 计算下一阶段并更新 [phase]。
     * 非法转移不抛异常，回落 [FboPhase.IDLE]。返回转移后的新阶段。
     */
    fun transition(action: FboAction): FboPhase {
        phase = nextPhase(phase, action)
        return phase
    }

    companion object {

        /**
         * 纯转移表：`(current, action) → newPhase`，无副作用、可重复调用。
         *
         * 合法边：
         * - IDLE + BIND → BIND；BIND + RENDER → RENDER；RENDER + BLIT → BLIT；
         * - BLIT + RESTORE → IDLE（恢复完成，回到未绑定）；
         * - 任意阶段 + RESET → IDLE（显式中止）。
         *
         * 其余组合均为非法边 → 回落 [FboPhase.IDLE]（fail-safe，防 GL 状态泄漏），不抛异常。
         */
        fun nextPhase(current: FboPhase, action: FboAction): FboPhase = when (action) {
            FboAction.BIND -> if (current == FboPhase.IDLE) FboPhase.BIND else FboPhase.IDLE
            FboAction.RENDER -> if (current == FboPhase.BIND) FboPhase.RENDER else FboPhase.IDLE
            FboAction.BLIT -> if (current == FboPhase.RENDER) FboPhase.BLIT else FboPhase.IDLE
            FboAction.RESTORE -> FboPhase.IDLE
            FboAction.RESET -> FboPhase.IDLE
        }

        /**
         * 阶段环邻接转移：`(current, next)` 为阶段环 `IDLE→BIND→RENDER→BLIT→RESTORE→IDLE`
         * 上的合法邻接边时返回 [next]，否则回落 [FboPhase.IDLE]（非法边，不抛异常）。
         */
        fun transition(current: FboPhase, next: FboPhase): FboPhase {
            val legal = when (current) {
                FboPhase.IDLE -> next == FboPhase.BIND
                FboPhase.BIND -> next == FboPhase.RENDER
                FboPhase.RENDER -> next == FboPhase.BLIT
                FboPhase.BLIT -> next == FboPhase.RESTORE
                FboPhase.RESTORE -> next == FboPhase.IDLE
            }
            return if (legal) next else FboPhase.IDLE
        }

        /**
         * 帧序列合法性：序列为一个或多个完整帧（每帧 = `BIND→RENDER→BLIT→RESTORE`，
         * 首尾 IDLE 允许省略/补全）。空序列、缺 restore 的残缺帧、游散 restore 均判非法。
         */
        fun isValidFrameSequence(phases: List<FboPhase>): Boolean {
            if (phases.isEmpty()) return false
            val core = phases
                .dropWhile { it == FboPhase.IDLE }
                .dropLastWhile { it == FboPhase.IDLE }
            if (core.isEmpty()) return false
            val frame = listOf(FboPhase.BIND, FboPhase.RENDER, FboPhase.BLIT, FboPhase.RESTORE)
            if (core.size % frame.size != 0) return false
            return core.chunked(frame.size).all { it == frame }
        }

        /**
         * bind/restore 对称：每 BIND 恰一 RESTORE，序为 BIND…RESTORE，
         * 且 RESTORE 只能紧跟 BLIT 之后（阶段环邻接约束）。
         */
        fun hasSymmetricBindRestore(phases: List<FboPhase>): Boolean {
            var balance = 0
            for ((index, phase) in phases.withIndex()) {
                when (phase) {
                    FboPhase.BIND -> balance++
                    FboPhase.RESTORE -> {
                        if (index == 0 || phases[index - 1] != FboPhase.BLIT) return false
                        balance--
                    }
                    else -> Unit
                }
            }
            return balance == 0
        }

        /** 缺 restore（bind 无配对 restore）：BIND 数 > RESTORE 数 → true。 */
        fun isImbalancedBind(phases: List<FboPhase>): Boolean =
            phases.count { it == FboPhase.BIND } > phases.count { it == FboPhase.RESTORE }

        /** 游散 restore（无前置未配对 bind 的 restore）：任意前缀 BIND-RESTORE 余额为负 → true。 */
        fun rejectsStrayRestore(phases: List<FboPhase>): Boolean {
            var balance = 0
            var minBalance = 0
            for (phase in phases) {
                when (phase) {
                    FboPhase.BIND -> balance++
                    FboPhase.RESTORE -> balance--
                    else -> Unit
                }
                minBalance = minOf(minBalance, balance)
            }
            return minBalance < 0
        }
    }
}

/**
 * 阶段环上的纯转移扩展：[FboPhase.BLIT].transition(FboAction.RESTORE) 等。
 * 语义同 [SupersampleFboStateMachine.nextPhase]，非法边回落 [FboPhase.IDLE]，不抛异常。
 */
fun FboPhase.transition(action: FboAction): FboPhase = SupersampleFboStateMachine.nextPhase(this, action)
