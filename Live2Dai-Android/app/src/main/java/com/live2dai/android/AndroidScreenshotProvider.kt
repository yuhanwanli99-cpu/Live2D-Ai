package com.live2dai.android

import android.content.Context
import android.graphics.Bitmap
import com.live2dai.plugin.vision.ScreenshotProvider
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * core 侧对 [ScreenshotProvider]（插件接口）的实现，注入给 `live2dai-plugin-vision`。
 *
 * Android 截图需要 MediaProjection / AccessibilityService 等系统权限，属于宿主 UI 层
 * 的责任范围，完整实现是独立的后续任务（见 docs/plan-task-extract-vision.md §5.4）。
 * 当前为空壳：capture() 返回 null → 插件 hook 静默跳过 → 对话流程不受影响。
 */
class AndroidScreenshotProvider(
    private val context: Context,  // applicationContext
) : ScreenshotProvider {
    override suspend fun capture(): Bitmap? = withContext(Dispatchers.IO) {
        // 方案 A：通过 /system/bin/screencap（需要 root 或 shell 权限）
        // 方案 B：通过 MediaProjection API（需要前台权限弹窗）
        // 方案 C：通过 AccessibilityService.takeScreenshot()（Android 11+, 需无障碍权限）
        // 初期返回 null，后续按需实现
        null
    }
}
