package com.live2d.ai.android.render

/** 动画后端选择：LEGACY（自研 AnimationSystem 管线）/ ENGINE（官方 updater + UpdateScheduler）。 */
enum class AnimationBackend { LEGACY, ENGINE }

/**
 * 动画后端选择器（seam，纯 JVM），与渲染 [BackendSelector] 同口径。
 *
 * - [backend]：当前选择（默认 [AnimationBackend.LEGACY]，可读写）；
 * - [fromFlag]：容错解析外部字符串（大小写不敏感、忽略首尾空白），
 *   未知值 / null / 空串回退 [AnimationBackend.LEGACY]。
 */
object AnimationSystemBackendSelector {

    /**
     * 默认动画后端（Wave3 主代理裁定：默认 LEGACY 保守口径）。
     * 注：Kotlin 的 const val 仅允许 primitive/String，枚举类型不可 const，
     * 故落地为 val（引用面 `AnimationSystemBackendSelector.DEFAULT` 不变）。
     */
    val DEFAULT: AnimationBackend = AnimationBackend.LEGACY

    /** 当前动画后端选择（默认 LEGACY）。 */
    var backend: AnimationBackend = AnimationBackend.LEGACY

    /** 容错解析：未知值（含 null/空串）回退 LEGACY。 */
    fun fromFlag(value: String?): AnimationBackend = when (value?.trim()?.uppercase()) {
        AnimationBackend.LEGACY.name -> AnimationBackend.LEGACY
        AnimationBackend.ENGINE.name -> AnimationBackend.ENGINE
        else -> AnimationBackend.LEGACY
    }
}
