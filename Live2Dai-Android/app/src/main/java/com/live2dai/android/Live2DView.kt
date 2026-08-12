package com.live2dai.android

import android.content.Context
import android.opengl.GLSurfaceView
import android.opengl.Matrix
import android.os.SystemClock
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.ViewConfiguration
import com.live2dai.android.data.GestureCommand
import com.live2dai.android.data.GestureEvent
import com.live2dai.android.data.GestureStateMachine
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

    /** 当前生效的模型 ID（用于切换失败回退时恢复对应的 adapter 缓存）。 */
    private var currentModelId: String? = null

    // ── M2.2 手势状态机（contract-R1-interaction §2.2）──
    // 设计（根因 A 修复，见 verify/gesture_diag.log）：DOWN 消费（return true）→ View 保持
    // 触摸目标资格 → MOVE/POINTER_DOWN/UP 事件链不断裂（拖拽/捏合恢复可用）。
    // 前景层防回归（M5-A9⑤⑥，Compose interop 机制保证）：
    //   · 气泡按钮/输入框/齿轮：点击在 Compose 命中链中先于本 View 消费 → 点击不受影响
    //     （唯一例外：气泡内 TextButton 的 DOWN 与 View 同命中路径——由 Tap 显式转发兜底）；
    //   · 气泡滚动：Compose scrollable 消费 MOVE 后 interop 向本 View 补发 CANCEL → 状态机复位，
    //     滚动优先于模型拖拽（与 C-M5 M5-A9⑥ 语义一致）。
    // tap 语义：状态机输出 Tap 命令 → 本 View 不 queueEvent renderer 任何状态（GS-A6），
    // 并显式转发合成 tap（contract §2.3 分支 A「显式转发」）交回 Compose 点击链处理。
    private val touchSlop: Int = ViewConfiguration.get(context).scaledTouchSlop
    private val gestureMachine: GestureStateMachine =
        GestureStateMachine(touchSlop.toFloat())

    /** 合成 tap 显式转发中：该标志期间收到的 MotionEvent 直接返回 false（不消费、不进状态机）。 */
    private var syntheticTapForwarding = false

    /** 诊断日志：本手势是否出现过捏合（UP 日志 pinchActive 字段）。 */
    private var gesturePinchSeen = false

    init {
        setEGLContextClientVersion(2)
        // 4x multisampling for anti-aliased edges
        setEGLConfigChooser(MSAAConfigChooser())

        // 不拦截触摸事件：本 View 仅作背景层，前景 Compose 元素（输入框、按钮）优先命中；
        // 触摸消费策略见 onTouchEvent（M2.2：DOWN 消费保事件链，tap 显式转发交回 Compose）。
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
     *                       例如: "live2d/bai/runtime/bai.model3.json"
     * @param modelId 模型 ID（如 "bai"），用于按模型加载适配配置（G3）
     */
    fun initialize(modelAssetPath: String, modelId: String) {
        currentAssetPath = modelAssetPath
        currentModelId = modelId
        // T1.2 + G3：惰性加载当前模型的共享适配配置（shared/model-adapter/{modelId}.adapter.json → assets）。
        // 失败 → null → MotionTags/VisemeAnalyzer/EmotionController 走硬编码常量兜底（回归门）。
        ModelAdapterConfig.load(context, modelId)
        renderer = Live2DRenderer(context, modelAssetPath)
        setRenderer(renderer)
        // 持续渲染模式，支持 120fps 平滑动画（必须在 setRenderer 之后调用）
        renderMode = RENDERMODE_CONTINUOUSLY
        // C-M5：绑定模型 ID + 应用该模型存储的用户 override（GL 线程，事件在 surface 创建后处理；
        // 无 override → C-M1 fit 默认生效；有 override → 覆盖 fit，见 contract-C-M5 §2.3 时机 1）
        queueEvent {
            renderer?.setCurrentModelId(modelId)
            renderer?.applyStoredUserOverride()
        }
    }

    /**
     * 运行时热切换模型（GL 线程 dispose + 重新加载，无需重建 View）。
     *
     * @param assetPath 新模型的完整 assets 路径，如 "live2d/bai/runtime/bai.model3.json"
     * @param modelId 新模型的 ID（如 "bai"）；切模型时同步刷新 adapter 缓存（G3）
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
        modelId: String,
        onComplete: (() -> Unit)? = null,
        onError: ((Throwable) -> Unit)? = null
    ) {
        queueEvent {
            val previousPath = currentAssetPath
            val previousModelId = currentModelId
            val current = renderer
            if (current == null) {
                // 视图尚未初始化：首次创建 renderer 并绑定（唯一一次合法 setRenderer）
                try {
                    val newRenderer = Live2DRenderer(context, assetPath)
                    renderer = newRenderer
                    currentAssetPath = assetPath
                    currentModelId = modelId
                    // G3：adapter 缓存随模型刷新
                    ModelAdapterConfig.load(context, modelId)
                    setRenderer(newRenderer)
                    renderMode = RENDERMODE_CONTINUOUSLY
                    // C-M5：绑定模型 ID + 应用该模型存储的 override（已在 GL 线程；无 → fit）
                    newRenderer.setCurrentModelId(modelId)
                    newRenderer.applyStoredUserOverride()
                    onComplete?.invoke()
                } catch (e: Throwable) {
                    android.util.Log.w("Live2DView", "切换模型失败(初始化): ${e.message}")
                    try { renderer?.dispose() } catch (_: Throwable) { }
                    renderer = null
                    currentAssetPath = previousPath
                    currentModelId = previousModelId
                    onError?.invoke(e)
                }
                return@queueEvent
            }

            // 热切换：同一 GL 线程内释放旧模型 + 加载新模型
            if (current.swapModel(assetPath)) {
                currentAssetPath = assetPath
                currentModelId = modelId
                // G3：adapter 缓存随模型切换刷新（bai → bai.adapter.json）
                ModelAdapterConfig.load(context, modelId)
                // C-M5：切模型后应用新模型的存储 override（已在 GL 线程；无 → fit）
                current.setCurrentModelId(modelId)
                current.applyStoredUserOverride()
                onComplete?.invoke()
            } else {
                // 加载失败：回退到上一个模型路径；回退也失败则保留兜底渲染
                val fallbackOk = previousPath != null && current.swapModel(previousPath)
                if (fallbackOk) {
                    currentAssetPath = previousPath
                    currentModelId = previousModelId
                    // G3：回退成功后恢复上一个模型的 adapter 缓存（与渲染中的模型保持一致）
                    if (previousModelId != null) {
                        ModelAdapterConfig.load(context, previousModelId)
                        // C-M5：回退后恢复上一个模型的 override
                        current.setCurrentModelId(previousModelId)
                        current.applyStoredUserOverride()
                    }
                } else {
                    currentAssetPath = null
                    currentModelId = null
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
     * 按语义标签（motionLabel 别名）播放 motion（M7 转发，对外公开入口）。
     * M2/关键词兜底调用此层。
     *
     * @param tag     语义别名（M1 §5 别名表；大小写不敏感）
     * @param priority 播放优先级（默认 NORMAL=2）；解析键以 "Idle_" 开头 → 强制 IDLE
     * @return true = 已开始播放；false = 未知 tag / 键未加载 / 被更高优先级阻止或同优先级不打断
     *         （不抛异常，静默降级；renderer 未接线时也返回 false）
     */
    fun startMotionByTag(tag: String, priority: Int = MotionPriority.NORMAL): Boolean {
        return renderer?.startMotionByTag(tag, priority) ?: false
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
     * 当前模型是否含可用的 Expressions（exp3）配置（A3 双路径分发判据，contract-bai-params-v0 AP-02）。
     * 转发 renderer；renderer 未接线 → false（保守）。
     */
    fun hasExpressions(): Boolean = renderer?.hasExpressions() ?: false

    /**
     * 参数驱动表情注入（A3，contract-bai-params-v0 AP-18）：以目标绝对值帧注入 ExpressionManager。
     * 同签名转发 renderer（EmotionController 只依赖 View 层，不直碰 renderer）；renderer 未接线 → 静默。
     * @param parameters 参数帧（key = 参数 Id，value = 目标绝对值；不 clamp，收敛交渲染层）
     * @param fadeMs 淡入时长（毫秒，先 coerceIn(20,1000)）
     */
    fun setParametricExpression(parameters: Map<String, Float>, fadeMs: Int) {
        renderer?.setParametricExpression(parameters, fadeMs)
    }

    /**
     * 自动眨眼开关（A3 D2-A，contract-bai-params-v0 AP-19）：
     * 参数驱动表情激活期间暂停自动眨眼，回 neutral 恢复。renderer 未接线 → 静默（null-safe）。
     */
    fun setAutoBlink(enabled: Boolean) {
        renderer?.setAutoBlink(enabled)
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
     * 按参数 ID 设置 Live2D 参数值（M5 VisemeAnalyzer 集成点）。
     * 模型无此参数 → 静默跳过；value 自动 coerceIn [0,1]。
     */
    fun setParameter(parameterId: String, value: Float) {
        renderer?.setParameter(parameterId, value)
    }

    /**
     * 半身显示：缩放 + 垂直偏移（模型单位）（contract-v3-rename §2.1，K5）。
     * 转发到 Live2DRenderer；默认值 = 全模型（现状），零行为变化。
     */
    fun setDisplayRegion(scale: Float = 1f, yOffset: Float = 0f) {
        renderer?.setDisplayRegion(scale, yOffset)
    }

    // ============================================================
    //  C-M5 用户交互（contract-C-M5 §2.3）
    // ============================================================

    /**
     * 触摸事件：事件 → GestureStateMachine → 命令流 → renderer（经 queueEvent，GL 线程）。
     *
     * - **DOWN 消费（return true）**：根因 A 修复（verify/gesture_diag.log 裁决：DOWN 不消费 →
     *   触摸目标丢失 → MOVE/POINTER_DOWN/UP 事件链断裂 → 拖拽+捏合全失效）。
     *   本 View 只收到未被前景 Compose 命中的触摸（模型区），消费不破坏前景点击；
     *   前景点击/滚动的保护由 Compose interop 机制保证（见字段区注释）。
     * - 命令流 → renderer：BeginPinch→beginUserPinch / ApplyPinch→applyUserPinch /
     *   ApplyDrag→applyUserDragPx / SaveOverride→saveUserOverride（renderer 公开接口，零改动）。
     * - Tap：不 queueEvent renderer 任何状态（GS-A6），显式转发合成 tap 交回 Compose 点击链。
     * - Reset（Cancel）：状态机内部已复位；renderer 无对应映射（resetUserOverride 属重置入口，语义不同）。
     *
     * 渲染状态全部经 queueEvent 在 GL 线程更新（避免主线程与 GL 线程数据竞争）。
     */
    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (renderer == null) return super.onTouchEvent(event)
        // 合成 tap 显式转发期间：不消费、不进状态机（防死循环/防污染状态）
        if (syntheticTapForwarding) return false

        val commands: List<GestureCommand>
        var consumed: Boolean
        var logState = "idle"
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                val id = event.getPointerId(0)
                commands = gestureMachine.onEvent(GestureEvent.Down(id, event.x, event.y))
                consumed = true // 根因 A：DOWN 消费保持事件链
                gesturePinchSeen = false
                logState = "idle"
                logGesture(action = "DOWN", id = id, count = event.pointerCount, consumed = true, state = logState)
            }
            MotionEvent.ACTION_POINTER_DOWN -> {
                val idx = event.actionIndex
                val id = event.getPointerId(idx)
                commands = gestureMachine.onEvent(
                    GestureEvent.PointerDown(id, event.getX(idx), event.getY(idx))
                )
                consumed = true
                logState = if (gestureMachine.isPinching) "pinching" else "idle"
                logGesture(action = "POINTER_DOWN", id = id, count = event.pointerCount, consumed = true, state = logState)
            }
            MotionEvent.ACTION_MOVE -> {
                // 全量活动指针快照（pointerId 锚定；T2 §6 UNSURE-3：MOVE 入参为全量快照语义）
                val pointers = (0 until event.pointerCount).associate { i ->
                    event.getPointerId(i) to (event.getX(i) to event.getY(i))
                }
                commands = gestureMachine.onEvent(GestureEvent.Move(pointers))
                consumed = true
                logState = when {
                    gestureMachine.isPinching -> "pinching"
                    gestureMachine.isDragging -> "dragging"
                    else -> "idle"
                }
                logGesture(
                    action = "MOVE",
                    count = event.pointerCount,
                    ids = pointerIds(event),
                    consumed = true,
                    state = logState
                )
            }
            MotionEvent.ACTION_POINTER_UP -> {
                val idx = event.actionIndex
                val id = event.getPointerId(idx)
                commands = gestureMachine.onEvent(
                    GestureEvent.Up(id, event.getX(idx), event.getY(idx))
                )
                consumed = true
                logState = when {
                    gestureMachine.isPinching -> "pinching"
                    gestureMachine.isDragging -> "dragging"
                    else -> "idle"
                }
                logGesture(action = "POINTER_UP", id = id, count = event.pointerCount, consumed = true, state = logState)
            }
            MotionEvent.ACTION_UP -> {
                val id = event.getPointerId(0)
                commands = gestureMachine.onEvent(GestureEvent.Up(id, event.x, event.y))
                // tap 不消费（GS-A6 / contract §2.3）；非 tap 的手势结束消费
                consumed = commands.none { it is GestureCommand.Tap }
                logState = if (commands.any { it is GestureCommand.Tap }) "tap" else "idle"
                logGesture(
                    action = "UP",
                    id = id,
                    count = event.pointerCount,
                    pinchActive = gesturePinchSeen,
                    consumed = consumed,
                    state = logState
                )
            }
            MotionEvent.ACTION_CANCEL -> {
                commands = gestureMachine.onEvent(GestureEvent.Cancel)
                consumed = true
                gesturePinchSeen = false
                logState = "idle"
                logGesture(action = "CANCEL", count = event.pointerCount, state = logState)
            }
            else -> return false
        }

        // 命令流 → renderer（GL 线程）
        for (cmd in commands) {
            when (cmd) {
                is GestureCommand.BeginPinch -> {
                    gesturePinchSeen = true
                    queueEvent { renderer?.beginUserPinch(cmd.startDistancePx) }
                }
                is GestureCommand.ApplyPinch -> {
                    gesturePinchSeen = true
                    queueEvent { renderer?.applyUserPinch(cmd.distancePx) }
                }
                is GestureCommand.ApplyDrag -> {
                    queueEvent { renderer?.applyUserDragPx(cmd.dxPx, cmd.dyPx) }
                }
                is GestureCommand.SaveOverride -> {
                    queueEvent { renderer?.saveUserOverride() }
                }
                is GestureCommand.Tap -> {
                    // 阈值内 UP、未调整：不消费、不落 renderer 状态；显式转发交回 Compose 点击链
                    // （contract §2.3 分支 A「显式转发」；防 M5-A9⑤ 气泡按钮点击回归）
                    performClick()
                    forwardTapToCompose(event)
                }
                is GestureCommand.Reset -> Unit // 状态机内部已复位；renderer 无对应映射
                is GestureCommand.Noop -> Unit
            }
        }
        return consumed
    }

    /**
     * 显式转发合成 tap（contract §2.3 分支 A「阈值内 UP 补发合成 tap 语义」）。
     *
     * 根因 A 修复后 DOWN 被消费，与 View 同命中路径的 Compose clickable（气泡内 TextButton）
     * 会因 DOWN 已被 interop 消费而跳过点击检测（detectTapAndPress 的 awaitFirstDown 要求未消费的
     * down）。这里把 Tap 作为新的 DOWN+UP 手势 post 到主线程后经根视图重新派发，使 Compose 点击链
     * 重新命中该位置元素（点击恰好一次；模型区无点击元素 → 无副作用）。
     */
    private fun forwardTapToCompose(source: MotionEvent) {
        // 捕获坐标与视图屏幕位置（MotionEvent 在派发后会被系统回收，不得在 post 后读取）
        val tapX = source.x
        val tapY = source.y
        val viewLoc = IntArray(2)
        getLocationOnScreen(viewLoc)
        post {
            val root = rootView ?: return@post
            val rootLoc = IntArray(2)
            root.getLocationOnScreen(rootLoc)
            // view 局部坐标 → 根视图局部坐标（dispatchTouchEvent 坐标系）
            val rx = tapX + (viewLoc[0] - rootLoc[0])
            val ry = tapY + (viewLoc[1] - rootLoc[1])
            syntheticTapForwarding = true
            try {
                val now = SystemClock.uptimeMillis()
                val down = MotionEvent.obtain(now, now, MotionEvent.ACTION_DOWN, rx, ry, 0)
                val up = MotionEvent.obtain(now, now + 16L, MotionEvent.ACTION_UP, rx, ry, 0)
                root.dispatchTouchEvent(down)
                root.dispatchTouchEvent(up)
                down.recycle()
                up.recycle()
            } finally {
                syntheticTapForwarding = false
            }
        }
    }

    /**
     * 诊断波插桩（contract-R1-interaction §2.1 / T2-R1-interaction-red §3）：
     * `[gesture]` 日志，tag=Live2DView。只读当前状态，零行为影响；
     * 格式与 PinchInjectionTest 机器对拍字面一致（判据①POINTER_DOWN ②MOVE count=2/ids=[ ③CANCEL）。
     */
    private fun logGesture(
        action: String,
        count: Int,
        state: String,
        id: Int? = null,
        consumed: Boolean? = null,
        pinchActive: Boolean? = null,
        ids: String? = null
    ) {
        val sb = StringBuilder("[gesture] action=$action")
        if (id != null) sb.append(" id=$id")
        sb.append(" count=$count")
        if (ids != null) sb.append(" ids=[$ids]")
        if (consumed != null) sb.append(" consumed=$consumed")
        if (pinchActive != null) sb.append(" pinchActive=$pinchActive")
        android.util.Log.i("Live2DView", sb.toString())
        android.util.Log.i("Live2DView", "[gesture] state=$state slop=$touchSlop")
    }

    /** 当前事件所有活动 pointer id 列表（判据②：MOVE count=2 ids=[0,1]）。 */
    private fun pointerIds(event: MotionEvent): String =
        (0 until event.pointerCount).joinToString(",") { event.getPointerId(it).toString() }

    /**
     * C-M5 重置入口：清除当前模型的用户 override → 回 C-M1 fit 构图（contract-C-M5 §2.3 时机 4）。
     * 主页顶部栏按钮调用（行为契约锁定，位置不锁）。
     */
    fun resetUserInteraction() {
        queueEvent { renderer?.resetUserOverride() }
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
