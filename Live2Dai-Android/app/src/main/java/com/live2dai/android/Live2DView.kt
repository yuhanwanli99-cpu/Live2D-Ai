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

    /** 当前生效的模型 assets 路径（用于切换失败时回退到上一个模型）。 */
    private var currentAssetPath: String? = null

    init {
        setEGLContextClientVersion(2)
        // 4x multisampling for anti-aliased edges
        setEGLConfigChooser(MSAAConfigChooser())

        // 不拦截触摸事件，让 Compose 底层 UI（输入框、按钮）可点击
        isClickable = false
        isFocusable = false
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
        currentAssetPath = modelAssetPath
        renderer = Live2DRenderer(context, modelAssetPath)
        setRenderer(renderer)
        // 持续渲染模式，支持 120fps 平滑动画（必须在 setRenderer 之后调用）
        renderMode = RENDERMODE_CONTINUOUSLY
    }

    /**
     * 运行时热切换模型（GL 线程 dispose + 重新加载，无需重建 View）。
     *
     * @param assetPath 新模型的完整 assets 路径，如 "live2d/shizuku/runtime/shizuku.model3.json"
     * @param onComplete 切换成功回调（在 GL 线程执行，更新 UI 需 post 到主线程）
     * @param onError 切换失败回调。失败时已自动尝试回退到上一个模型；
     *                回退也失败则渲染兜底渐变着色器。同样在 GL 线程执行。
     *
     * ## 实现说明（相对设计文档的修正）
     * 设计文档建议“dispose 旧 renderer → new Live2DRenderer → setRenderer()”。
     * 但 AOSP GLSurfaceView.setRenderer() 只能调用一次（第二次抛
     * IllegalStateException: "setRenderer has already been called"，见
     * frameworks/base/opengl/java/android/opengl/GLSurfaceView.java）。
     * 因此改为：复用同一个 GLSurfaceView.Renderer 实例，在 GL 线程内
     * dispose 旧模型资源 + 加载新模型（Live2DRenderer.swapModel），
     * 不触发 EGL 表面重建，也不会中断 RENDERMODE_CONTINUOUSLY。
     */
    fun switchModel(
        assetPath: String,
        onComplete: (() -> Unit)? = null,
        onError: ((Throwable) -> Unit)? = null
    ) {
        queueEvent {
            val previousPath = currentAssetPath
            val current = renderer
            if (current == null) {
                // 视图尚未初始化：首次创建 renderer 并绑定（唯一一次合法 setRenderer）
                try {
                    val newRenderer = Live2DRenderer(context, assetPath)
                    renderer = newRenderer
                    currentAssetPath = assetPath
                    setRenderer(newRenderer)
                    renderMode = RENDERMODE_CONTINUOUSLY
                    onComplete?.invoke()
                } catch (e: Throwable) {
                    android.util.Log.w("Live2DView", "切换模型失败(初始化): ${e.message}")
                    try { renderer?.dispose() } catch (_: Throwable) { }
                    renderer = null
                    currentAssetPath = previousPath
                    onError?.invoke(e)
                }
                return@queueEvent
            }

            // 热切换：同一 GL 线程内释放旧模型 + 加载新模型
            if (current.swapModel(assetPath)) {
                currentAssetPath = assetPath
                onComplete?.invoke()
            } else {
                // 加载失败：回退到上一个模型路径；回退也失败则保留兜底渲染
                val fallbackOk = previousPath != null && current.swapModel(previousPath)
                if (fallbackOk) {
                    currentAssetPath = previousPath
                } else {
                    currentAssetPath = null
                }
                onError?.invoke(
                    RuntimeException(
                        if (fallbackOk) "模型加载失败，已回退到上一个模型: $assetPath"
                        else "模型加载失败且回退失败: $assetPath"
                    )
                )
            }
        }
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
    fun setExpression(expressionIndex: Int = 0, intensity: Float = 1.0f) {
        renderer?.setExpression(expressionIndex, intensity)
    }

    /**
     * 结构化表情混合（M2）：多表情加权混合 + 参数覆盖。
     * @param mix (EMOTION_MAP 索引, weight) 列表，按数组顺序应用；索引查不到的表情跳过（部分生效）。
     * @param overrides 参数覆盖乘数（key = 参数 ID）
     */
    fun setExpressionMix(mix: List<Pair<Int, Float>>, overrides: Map<String, Float>) {
        renderer?.setExpressionMix(mix, overrides)
    }

    /**
     * 当前模型 cdi3.json 参数 ID 白名单（M2 parameterOverrides key 校验源）。
     */
    fun getParameterWhitelist(): Set<String> = renderer?.getParameterWhitelist() ?: emptySet()

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
