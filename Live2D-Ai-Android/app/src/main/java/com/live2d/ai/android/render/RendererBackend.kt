package com.live2d.ai.android.render

/** 渲染后端选择：LEGACY（既有渲染管线）/ ENGINE（新引擎渲染管线）。 */
enum class RendererBackend { LEGACY, ENGINE }

/**
 * 后端选择器（seam，纯 JVM）：默认 [RendererBackend.ENGINE]。
 * - [backend]：当前选择（可读写）；
 * - [select]：返回当前选择；
 * - [fromValue]：容错解析外部字符串（大小写不敏感、忽略首尾空白），
 *   未知值 / null / 空串回退 [RendererBackend.ENGINE]。
 */
object BackendSelector {

    /** 当前后端选择（默认 ENGINE）。 */
    var backend: RendererBackend = RendererBackend.LEGACY

    /** 返回当前选择。 */
    fun select(): RendererBackend = backend

    /** 容错解析：未知值（含 null/空串）回退 ENGINE。 */
    fun fromValue(value: String?): RendererBackend = when (value?.trim()?.uppercase()) {
        RendererBackend.LEGACY.name -> RendererBackend.LEGACY
        RendererBackend.ENGINE.name -> RendererBackend.ENGINE
        else -> RendererBackend.LEGACY
    }
}
