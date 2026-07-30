package com.live2dai.android

import android.app.Service
import android.content.Context
import android.content.Intent
import android.graphics.PixelFormat
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.provider.Settings
import android.util.DisplayMetrics
import android.view.GestureDetector
import android.view.Gravity
import android.view.MotionEvent
import android.view.ViewConfiguration
import android.view.WindowManager
import android.widget.FrameLayout

/**
 * 悬浮窗 Service
 *
 * 在 [WindowManager] 上创建一个覆盖层，显示 Live2D 猫娘。
 * 支持拖拽移动和点击打开主 App。
 *
 * ## 用法
 * ```kotlin
 * // 启动
 * FloatingWindowService.start(context)
 * // 停止
 * FloatingWindowService.stop(context)
 * ```
 *
 * ## 权限
 * 需要 [Manifest.permission.SYSTEM_ALERT_WINDOW] 权限。
 * Android 6+ 需用户在设置中手动授权。
 * 使用 [FloatingWindowService.hasOverlayPermission] 检查。
 * 使用 [FloatingWindowService.openOverlaySettings] 打开权限设置页。
 */
class FloatingWindowService : Service() {

    companion object {
        private const val TAG = "FloatingWindow"
        private const val MODEL_PATH = "live2d/mao_pro/runtime/mao_pro.model3.json"

        // 悬浮窗尺寸 (dp)
        private const val WINDOW_WIDTH_DP = 300
        private const val WINDOW_HEIGHT_DP = 400

        // 30fps 帧间隔 (ms)
        private const val FRAME_INTERVAL_MS = 33L

        // Intent actions
        private const val ACTION_START = "com.live2dai.android.action.START_FLOATING"
        private const val ACTION_STOP = "com.live2dai.android.action.STOP_FLOATING"

        // Preference key
        private const val PREF_FLOATING_ENABLED = "floating_window_enabled"

        /**
         * 检查悬浮窗权限是否已授予
         */
        fun hasOverlayPermission(context: Context): Boolean {
            return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
                Settings.canDrawOverlays(context)
            } else {
                true // Android 6 以下不需要运行时悬浮窗权限
            }
        }

        /**
         * 打开悬浮窗权限设置页
         */
        fun openOverlaySettings(context: Context) {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
                val intent = Intent(
                    Settings.ACTION_MANAGE_OVERLAY_PERMISSION,
                    android.net.Uri.parse("package:${context.packageName}")
                )
                intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                context.startActivity(intent)
            }
        }

        /**
         * 保存悬浮窗开关状态到 SharedPreferences
         */
        fun setEnabled(context: Context, enabled: Boolean) {
            context.getSharedPreferences(TAG, Context.MODE_PRIVATE)
                .edit()
                .putBoolean(PREF_FLOATING_ENABLED, enabled)
                .apply()
        }

        /**
         * 读取悬浮窗开关状态
         */
        fun isEnabled(context: Context): Boolean {
            return context.getSharedPreferences(TAG, Context.MODE_PRIVATE)
                .getBoolean(PREF_FLOATING_ENABLED, false)
        }

        /**
         * 启动悬浮窗 Service
         */
        fun start(context: Context) {
            if (!hasOverlayPermission(context)) return
            val intent = Intent(context, FloatingWindowService::class.java).apply {
                action = ACTION_START
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }

        /**
         * 停止悬浮窗 Service
         */
        fun stop(context: Context) {
            val intent = Intent(context, FloatingWindowService::class.java).apply {
                action = ACTION_STOP
            }
            context.startService(intent)
        }
    }

    // ── 窗口管理 ──
    private lateinit var windowManager: WindowManager
    private var floatContainer: DragContainer? = null
    private lateinit var live2DView: Live2DView
    private var windowParams: WindowManager.LayoutParams? = null

    // ── 30fps 帧率控制 ──
    private val frameHandler = Handler(Looper.getMainLooper())
    private val frameRunnable = object : Runnable {
        override fun run() {
            if (::live2DView.isInitialized) {
                live2DView.requestRender()
            }
            frameHandler.postDelayed(this, FRAME_INTERVAL_MS)
        }
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        android.util.Log.i(TAG, "FloatingWindowService created")
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> {
                if (floatContainer == null) {
                    showFloatingWindow()
                }
            }
            ACTION_STOP -> {
                removeFloatingWindow()
                stopSelf()
            }
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        removeFloatingWindow()
        super.onDestroy()
    }

    /**
     * 创建悬浮窗 View 并添加到 WindowManager
     */
    private fun showFloatingWindow() {
        if (!hasOverlayPermission(this)) {
            android.util.Log.w(TAG, "No overlay permission, cannot show floating window")
            stopSelf()
            return
        }

        windowManager = getSystemService(WINDOW_SERVICE) as WindowManager

        // 计算像素尺寸 (300x400 dp)
        val metrics = DisplayMetrics().also { windowManager.defaultDisplay.getMetrics(it) }
        val density = metrics.density
        val widthPx = (WINDOW_WIDTH_DP * density).toInt()
        val heightPx = (WINDOW_HEIGHT_DP * density).toInt()

        // 创建 Live2D 渲染 View
        live2DView = Live2DView(this)
        live2DView.initialize(MODEL_PATH)

        // 设置渲染模式为 WHEN_DIRTY + Handler 定时请求渲染（实现 30fps 节能）
        live2DView.renderMode = GLSurfaceView.RENDERMODE_WHEN_DIRTY

        // 创建拖拽容器
        floatContainer = DragContainer(this, windowManager) {
            // 点击事件：打开主 App
            onFloatingWindowClick()
        }

        // 添加 Live2DView 到容器
        floatContainer!!.addView(
            live2DView,
            FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            )
        )

        // 初始化默认位置（屏幕右上角）
        val defaultX = metrics.widthPixels - widthPx - (16 * density).toInt()
        val defaultY = (64 * density).toInt()

        windowParams = WindowManager.LayoutParams(
            widthPx,
            heightPx,
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE
                    or WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS
                    or WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL,
            PixelFormat.TRANSLUCENT
        ).apply {
            gravity = Gravity.TOP or Gravity.START
            x = defaultX
            y = defaultY
        }

        windowManager.addView(floatContainer, windowParams)

        // 启动 30fps 渲染循环
        frameHandler.post(frameRunnable)

        android.util.Log.i(TAG, "Floating window shown (${widthPx}x${heightPx}px)")
    }

    /**
     * 移除悬浮窗并释放资源
     */
    private fun removeFloatingWindow() {
        // 停止帧循环
        frameHandler.removeCallbacks(frameRunnable)

        // 释放 Live2D 资源
        if (::live2DView.isInitialized) {
            live2DView.dispose()
        }

        // 从 WindowManager 移除
        val container = floatContainer
        if (container != null && container.isAttachedToWindow) {
            try {
                windowManager.removeView(container)
            } catch (e: Exception) {
                android.util.Log.w(TAG, "removeView failed: ${e.message}")
            }
        }
        floatContainer = null
        windowParams = null

        android.util.Log.i(TAG, "Floating window removed")
    }

    /**
     * 悬浮窗点击事件：将主 Activity 带到前台
     */
    private fun onFloatingWindowClick() {
        try {
            val launchIntent = packageManager.getLaunchIntentForPackage(packageName)
            if (launchIntent != null) {
                launchIntent.addFlags(
                    Intent.FLAG_ACTIVITY_NEW_TASK
                            or Intent.FLAG_ACTIVITY_REORDER_TO_FRONT
                )
                startActivity(launchIntent)
            }
        } catch (e: Exception) {
            android.util.Log.w(TAG, "Failed to open main activity: ${e.message}")
        }
    }
}

// 需要使用 GLSurfaceView
import android.opengl.GLSurfaceView

/**
 * 可拖拽的 FrameLayout，支持手势检测
 *
 * 使用 [GestureDetector] 区分点击和拖拽：
 * - 短触（无拖拽）→ 触发点击回调
 * - 拖拽 → 更新 [WindowManager.LayoutParams.x/y]
 */
private class DragContainer(
    context: Context,
    private val windowManager: WindowManager,
    private val onClick: () -> Unit
) : FrameLayout(context) {

    private val gestureDetector: GestureDetector
    private val touchSlop = ViewConfiguration.get(context).scaledTouchSlop

    private var initialX = 0
    private var initialY = 0
    private var initialTouchX = 0f
    private var initialTouchY = 0f
    private var isDragging = false

    init {
        gestureDetector = GestureDetector(context, object : GestureDetector.SimpleOnGestureListener() {
            override fun onSingleTapConfirmed(e: MotionEvent): Boolean {
                // 确认单击（非拖拽）
                onClick()
                return true
            }

            override fun onDoubleTap(e: MotionEvent): Boolean {
                // 双击也可以触发点击
                onClick()
                return true
            }
        })
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        gestureDetector.onTouchEvent(event)

        val lp = layoutParams as? WindowManager.LayoutParams ?: return super.onTouchEvent(event)

        when (event.action) {
            MotionEvent.ACTION_DOWN -> {
                initialX = lp.x
                initialY = lp.y
                initialTouchX = event.rawX
                initialTouchY = event.rawY
                isDragging = false
                // 触摸时给一点反馈：提升窗口层级（可选）
                return true
            }

            MotionEvent.ACTION_MOVE -> {
                val dx = event.rawX - initialTouchX
                val dy = event.rawY - initialTouchY

                if (!isDragging) {
                    // 超过触摸阈值时才判定为拖拽
                    if (kotlin.math.abs(dx) > touchSlop || kotlin.math.abs(dy) > touchSlop) {
                        isDragging = true
                    }
                }

                if (isDragging) {
                    lp.x = (initialX + dx).toInt()
                    lp.y = (initialY + dy).toInt()
                    try {
                        windowManager.updateViewLayout(this, lp)
                    } catch (_: Exception) { }
                }
                return true
            }

            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                isDragging = false
                return true
            }
        }

        return super.onTouchEvent(event)
    }

    override fun onInterceptTouchEvent(ev: MotionEvent): Boolean {
        // 不拦截子 View 事件（让 GLSurfaceView 也能接收触摸，虽然我们不需要）
        return false
    }
}
