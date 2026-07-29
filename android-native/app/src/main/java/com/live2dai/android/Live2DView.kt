package com.live2dai.android

import android.content.Context
import android.opengl.GLSurfaceView
import android.util.AttributeSet
import javax.microedition.khronos.egl.EGL10
import javax.microedition.khronos.egl.EGLConfig
import javax.microedition.khronos.egl.EGLDisplay

/**
 * OpenGL surface for rendering Live2D Cubism models.
 * Uses GLSurfaceView with RENDERMODE_CONTINUOUSLY for 120fps capability,
 * 4x MSAA for smooth edges.
 */
class Live2DView(
    context: Context,
    attrs: AttributeSet? = null
) : GLSurfaceView(context, attrs) {

    private var renderer: Live2DRenderer? = null

    init {
        setEGLContextClientVersion(2)
        // 4x multisampling for anti-aliased edges
        setEGLConfigChooser(MSAAConfigChooser())
    }

    /**
     * EGL config chooser with 4x MSAA support, graceful fallback.
     */
    private class MSAAConfigChooser : GLSurfaceView.EGLConfigChooser {
        override fun chooseConfig(egl: EGL10, display: EGLDisplay): EGLConfig {
            val attribs = intArrayOf(
                EGL10.EGL_RED_SIZE, 8,
                EGL10.EGL_GREEN_SIZE, 8,
                EGL10.EGL_BLUE_SIZE, 8,
                EGL10.EGL_ALPHA_SIZE, 8,
                EGL10.EGL_DEPTH_SIZE, 16,
                EGL10.EGL_SAMPLE_BUFFERS, 1,
                EGL10.EGL_SAMPLES, 4,
                EGL10.EGL_NONE
            )
            val configs = arrayOfNulls<EGLConfig>(1)
            val num = IntArray(1)
            egl.eglChooseConfig(display, attribs, configs, 1, num)
            if (num[0] > 0) return configs[0]!!
            // fallback without MSAA
            val fallback = intArrayOf(
                EGL10.EGL_RED_SIZE, 8, EGL10.EGL_GREEN_SIZE, 8,
                EGL10.EGL_BLUE_SIZE, 8, EGL10.EGL_ALPHA_SIZE, 8,
                EGL10.EGL_DEPTH_SIZE, 16, EGL10.EGL_NONE
            )
            egl.eglChooseConfig(display, fallback, configs, 1, num)
            return configs[0]!!
        }
    }

    /**
     * 初始化 Live2D 渲染器并加载模型
     * @param modelAssetPath assets 中的模型路径（不含 "assets/" 前缀）
     *                       例如: "live2d/mao_pro/runtime/mao_pro.model3.json"
     */
    fun initialize(modelAssetPath: String) {
        renderer = Live2DRenderer(context, modelAssetPath)
        setRenderer(renderer)
        // 持续渲染模式，支持 120fps 平滑动画（必须在 setRenderer 之后调用）
        renderMode = RENDERMODE_CONTINUOUSLY
    }

    /**
     * 播放指定动作组中的动作
     * @param motionGroup 动作组名，如 "Idle", "" 等
     * @param motionIndex 组内动作索引
     */
    fun startMotion(motionGroup: String, motionIndex: Int = 0) {
        renderer?.startMotion(motionGroup, motionIndex)
    }

    /**
     * 设置面部表情
     * @param expressionIndex 表情索引 (0-7, 对应 model3.json 的 Expressions 数组)
     */
    fun setExpression(expressionIndex: Int = 0) {
        renderer?.setExpression(expressionIndex)
    }

    /**
     * 设置口型同步参数
     * @param value 0.0 (闭嘴) ~ 1.0 (最大张口)
     */
    fun setLipSyncParam(value: Float) {
        renderer?.setLipSyncParam(value)
    }

    /**
     * 释放渲染器资源
     * 在 Activity onDestroy 时调用
     */
    fun dispose() {
        queueEvent {
            renderer?.dispose()
            renderer = null
        }
    }

    override fun onPause() {
        super.onPause()
        renderer?.onPause()
    }

    override fun onResume() {
        super.onResume()
        renderer?.onResume()
    }
}
