package com.live2dai.android.core

/**
 * 表情应用接缝（契约 refactor-0814 §2.2 锁定：`apply(emotion)` / `release()` 幂等，release 后回 neutral）。
 *
 * 实现归 2C-3/A 片（ExpressionSink 壳侧封装）；本文件仅声明接口签名。
 * 约束：表情不得无限期门控眨眼/头身 idle 通道（冻结免疫 §4.2）。
 */
interface ExpressionSink {
    /** 应用表情（null = neutral）。 */
    fun apply(emotion: String?)

    /** 表情收敛回 neutral（幂等；任意态可安全调用）。 */
    fun release()
}
