package com.live2dai.android.core

/**
 * FPS 显示角标状态（用户需求①：渲染帧率实时显示）。
 *
 * 纯 JVM 无 Android 依赖：
 * - [currentFps] 由 GL 线程（Live2DRenderer.onDrawFrame）每秒更新一次；
 * - UI 线程（MainScreen）每 1s 读一次触发重组显示 "FPS: n"。
 *
 * [Volatile]：GL 线程写 / UI 线程读跨线程可见性保证（每 1s 粒度足够，无需同步）。
 */
object FpsOverlayState {
    /** 最近一秒平均帧率（GL 线程写；UI 只读）。0 = 尚无数据（首秒）。 */
    @Volatile
    var currentFps: Int = 0
}
