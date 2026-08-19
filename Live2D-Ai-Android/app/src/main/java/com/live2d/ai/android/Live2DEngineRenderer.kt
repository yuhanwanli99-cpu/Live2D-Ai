package com.live2d.ai.android

import com.live2d.ai.android.render.BackendSelector
import com.live2d.ai.android.render.FrameSinks
import com.live2d.ai.android.render.RendererFrameSequencer

/**
 * 引擎渲染薄壳（Wave2 A 片）：把 [RendererFrameSequencer] 帧序（drain → update(dt) → draw）
 * 接到 C 引擎（Live2DNative.engine* JNI）。持有 [BackendSelector]（ENGINE 路径有效开关；
 * 双轨切换点由 Live2DRenderer.onDrawFrame 开头读取 [BackendSelector.select]）。
 *
 * - [sinks]：参数帧表（外部写入 pending，帧首 drain 取走；本 shell 捕获取走值并在 update 前
 *   注入 engineSetParameters → 满足"参数写入先于更新"帧序契约）；
 * - [frame]：转发到 [RendererFrameSequencer.frame]；
 * - [setMvpMatrix]/[setLipSync]：JNI 封装（预分配缓冲，零每帧分配）；
 * - [release]：engineRelease（生命周期接线下一波）。
 *
 * drawable 顶点数据路径预分配（复用 PreallocRenderBufferProvider 语义：复用缓冲、无每帧分配）：
 * C 侧由框架 Renderer_OpenGLES2::Initialize 一次性分配 VBO / mask 渲染目标并跨帧复用
 * （engineDraw 零堆分配）；本 shell 侧预分配 MVP 缓冲 [mvpBuffer] 跨帧复用。
 */
class Live2DEngineRenderer {

    /** 后端选择器（ENGINE 路径开关；与双轨切换点同源）。 */
    val selector: BackendSelector = BackendSelector

    /** 参数帧表（drain 时捕获取走值，供 update 前注入 native）。 */
    val sinks: EngineFrameSinks = EngineFrameSinks()

    /** 引擎生命周期句柄（engineInit 返回值；0 = 初始化失败，后续 JNI 调用为 C 侧 no-op）。 */
    private var engineHandle: Long = 0L

    /** 预分配 MVP 缓冲（列主序 16 float，跨帧复用，零每帧分配）。 */
    private val mvpBuffer = FloatArray(16)

    private val sequencer = RendererFrameSequencer(
        onUpdate = { dt -> onEngineUpdate(dt) },
        onDraw = { onEngineDraw() }
    )

    init {
        engineHandle = Live2DNative.engineInit()
    }

    /** 执行一帧：drain(参数表) → update(dt) → draw()（转发到 [RendererFrameSequencer]）。 */
    fun frame(dt: Float, sinks: FrameSinks) {
        sequencer.frame(dt, sinks)
    }

    /** P2: 加载模型到官方 C++ 引擎路径（moc3/model3/cdi3 均为绝对文件路径）。 */
    fun loadModel(moc3Path: String, model3Path: String, cdi3Path: String): Long {
        engineHandle = Live2DNative.engineLoadModel(moc3Path, model3Path, cdi3Path)
        return engineHandle
    }

    /** 设置 MVP 矩阵（列主序 float[16]；拷贝进预分配缓冲后下发，跨帧复用同一缓冲）。 */
    fun setMvpMatrix(matrix: FloatArray) {
        if (matrix.size >= 16) {
            System.arraycopy(matrix, 0, mvpBuffer, 0, 16)
        }
        Live2DNative.engineSetMvpMatrix(mvpBuffer)
    }

    /** 设置嘴型同步值（C 侧 engineUpdate 时每帧替换写入 ParamMouthOpenY，F-1 修复后为替换语义）。 */
    fun setLipSync(value: Float) {
        Live2DNative.engineSetLipSync(value)
    }

    /** 释放引擎全局态（生命周期接线下一波；重复调用安全）。 */
    fun release() {
        if (engineHandle != 0L) {
            Live2DNative.engineRelease(engineHandle)
            engineHandle = 0L
        }
    }

    private fun onEngineUpdate(dt: Float) {
        // 帧首 drain 的取走值（本 shell 的 sinks）在 update 前注入 native（参数写入先于更新）
        val params = sinks.consumeDrained()
        if (params.isNotEmpty()) {
            Live2DNative.engineSetParameters(params)
        }
        Live2DNative.engineUpdate(dt)
    }

    private fun onEngineDraw() {
        Live2DNative.engineDraw()
    }

    /**
     * 参数帧表：drain 时捕获取走值（RendererFrameSequencer.frame 丢弃 drain 返回值，
     * 此处捕获以在 update 前注入 native）。外部（Live2DRenderer.ENGINE 分支）向
     * [Live2DEngineRenderer.sinks] 写入参数，每帧随 frame(dt, sinks) 传入。
     */
    class EngineFrameSinks : FrameSinks() {

        /** 最近一次 drain 取走的参数（仅 [drainedThisFrame] 为 true 时有效）。 */
        var lastDrained: Map<String, Float> = emptyMap()
            private set

        /** 本帧是否发生了本实例的 drain（onEngineUpdate 消费后复位）。 */
        var drainedThisFrame: Boolean = false
            private set

        @Synchronized
        override fun drain(): Map<String, Float> {
            val out = super.drain()
            lastDrained = out
            drainedThisFrame = true
            return out
        }

        /** 取走本帧 drain 的参数并复位标志（无本实例 drain 时返回空表）。 */
        @Synchronized
        fun consumeDrained(): Map<String, Float> {
            if (!drainedThisFrame) return emptyMap()
            drainedThisFrame = false
            return lastDrained
        }
    }
}
