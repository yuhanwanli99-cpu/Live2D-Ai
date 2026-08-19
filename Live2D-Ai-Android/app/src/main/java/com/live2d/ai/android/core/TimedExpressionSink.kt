package com.live2d.ai.android.core

/**
 * 可注入时钟（契约 refactor-0814 2C-3 §2；测试注入 fake 实现确定性驱动收敛判定）。
 */
interface Clock {
    /** 当前时间（毫秒，单调或墙钟均可——同源比较，不跨源混用）。 */
    fun nowMillis(): Long
}

/** 默认系统时钟（毫秒墙钟）。 */
object SystemClock : Clock {
    override fun nowMillis(): Long = System.currentTimeMillis()
}

/**
 * 带 5 秒自动收敛的 [ExpressionSink] 实现（契约 refactor-0814 2C-3 §2）。
 *
 * 语义：
 *  - [apply]：记录当前表情 + 时间戳（[Clock.nowMillis]），激活收敛定时；
 *    `null` = neutral（立即视为已释放，同 [ExpressionSink] 接口文档）；
 *  - 收敛判定：**查询时判定超时**（确定性优先，无后台调度线程）——
 *    外部驱动（帧循环/协程）周期调用 [tick]，距 [apply] 达 [CONVERGE_DELAY_MS] 即触发 release 语义；
 *  - [release]：幂等——重复调用行为一次、不抛异常，收敛到 neutral（activeEmotion=null）；
 *    未 [apply] 时调用无副作用；
 *  - [release] 后可再次 [apply] 重新激活（时间戳重置，收敛窗口重新起算）。
 */
class TimedExpressionSink(private val clock: Clock = SystemClock) : ExpressionSink {

    companion object {
        /** 收敛延迟：apply 后 5 秒自动触发 release 语义（C-2 裁定值 = N = 5s，毫秒）。 */
        const val CONVERGE_DELAY_MS: Long = 5_000L
    }

    /** 当前激活表情（null = neutral/已释放）。 */
    private var activeEmotion: String? = null

    /** 最近一次 [apply] 的时间戳（毫秒，来自注入的 [Clock]）。 */
    private var lastApplyMs: Long = 0L

    /** 是否处于已释放（neutral）状态。初始 true（未 apply = 已释放，release 无副作用）。 */
    private var released: Boolean = true

    /** 状态查询：当前是否已收敛到 neutral。**查询时判定**——距上次 apply 达 [CONVERGE_DELAY_MS] 即视为已收敛
     *（测试注入 fake clock 确定性驱动；显式 [release] 同样收敛）。 */
    val isReleased: Boolean
        get() = if (activeEmotion == null) true
                else clock.nowMillis() - lastApplyMs >= CONVERGE_DELAY_MS

    /** 状态查询：当前激活表情（null = neutral/已释放）。 */
    fun currentEmotion(): String? = activeEmotion

    override fun apply(emotion: String?) {
        activeEmotion = emotion
        lastApplyMs = clock.nowMillis()
        // null = neutral（接口文档语义）→ 立即视为已释放；非 null → 激活并起算收敛窗口。
        released = emotion == null
    }

    override fun release() {
        // 幂等：已释放（含从未 apply）→ 直接返回，无副作用、不抛异常。
        if (released) return
        released = true
        activeEmotion = null
    }

    /**
     * 收敛驱动入口：外部按时钟周期调用（nowMs 须与 [Clock] 同一时间基）。
     *
     * @param nowMs 当前时间（毫秒）。
     * @return 本次调用是否恰好触发了收敛（release 语义生效）；未激活/未到期返回 false。
     */
    fun tick(nowMs: Long): Boolean {
        if (released) return false
        if (nowMs - lastApplyMs >= CONVERGE_DELAY_MS) {
            release()
            return true
        }
        return false
    }
}
