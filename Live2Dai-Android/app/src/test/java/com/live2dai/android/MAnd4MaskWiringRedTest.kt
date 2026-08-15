package com.live2dai.android

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.io.File

/**
 * M-AND-4 VIS 碎片修复（mask 接线）— RED 测试（AND4-P1）。
 *
 * 依据：.pi/spoq/contract-linkage-android.md §2 M-AND-4 AND4-P1（唯一真相源；拍板记录 §3 确认点2：
 * H1 成立 → P1 启用；P2/P3 禁用）。
 *
 * AND4-P1（启用）:  drawObjectLoop 对 `maskCount > 0` 的 drawable 分派 MASKED / MASKED_INVERTED
 * （ShaderManager.get 分派已备，77-78 回归锁定）；`PurismClippingManager.prepareFrame/renderMaskBuffers`
 * 存在调用点（结构断言 + 真机日志「clipping prepared」出现）。
 *
 * 本文件的两类断言（机器可判，strictly 来自契约字面语义）：
 *  (a) shader 选择纯函数 seam：`ShaderManager.selectShaderType(isMasked, isInvertedMask)` 是契约允许的
 *      可注入 seam，对「maskCount>0 → MASKED/MASKED_INVERTED、maskCount=0 → NORMAL」选择逻辑直接单测。
 *      契约注「已备，77-78 回归锁定」→ 本组为 GREEN 基线回归锁（RED 主战场在下文 (b)）。
 *  (b) 渲染器 mask 接线 + PurismClippingManager 调用点：静态源码扫描测试（运行时读 Live2DRenderer.kt
 *      源文本，断言契约强制标记，机械可判）。证据（probe0-report）：渲染器当前恒 NORMAL 不裁剪，
 *      84 个带 mask 的 drawable 全裸绘 → 本组当前 RED（H1 修复前不落地）。
 *
 * 真机日志断言（logcat「clipping prepared」出现）属 AND4-P1 的真机面，归波次4 真机验收，
 * 本文件不写 JVM 测试（详见 RED 报告标注）。
 *
 * 纪律：禁读 src/main 实现代码「对答案出题」；本类的源码扫描锚点全部取自契约 AND4-P1 字面
 * （PurismClippingManager / prepareFrame / renderMaskBuffers / MASKED / MASKED_INVERTED），非实现细节。
 * [UNSURE] 见 RED 报告：drawObjectLoop 逐分支级别的机械锚定方式无法在不读实现的前提下唯一确定，
 *   故 (b) 用「源码包含强制标记」的契约直读形式，不做逐行等价。
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class, sdk = [34])
class MAnd4MaskWiringRedTest {

    private val context: Context = ApplicationProvider.getApplicationContext()

    // ─────────────────────────────────────────────
    // (a) shader 选择纯函数 seam（AND4-P1：maskCount>0 → MASKED / MASKED_INVERTED）
    //     契约注「ShaderManager.get 分派已备，77-78 回归锁定」→ GREEN 基线回归锁
    // ─────────────────────────────────────────────

    @Test
    fun and4_p1_selectShaderType_masked_when_isMasked_only() {
        assertEquals(
            "maskCount>0 且非反向 → 分派 MASKED（AND4-P1）",
            ShaderManager.ShaderType.MASKED,
            ShaderManager(context).selectShaderType(isMasked = true, isInvertedMask = false)
        )
    }

    @Test
    fun and4_p1_selectShaderType_maskedInverted_when_isMasked_and_inverted() {
        assertEquals(
            "maskCount>0 且反向 → 分派 MASKED_INVERTED（AND4-P1）",
            ShaderManager.ShaderType.MASKED_INVERTED,
            ShaderManager(context).selectShaderType(isMasked = true, isInvertedMask = true)
        )
    }

    @Test
    fun and4_p1_selectShaderType_normal_when_notMasked() {
        assertEquals(
            "maskCount=0 → 分派 NORMAL（AND4-P1：仅 maskCount>0 才 MASKED）",
            ShaderManager.ShaderType.NORMAL,
            ShaderManager(context).selectShaderType(isMasked = false, isInvertedMask = false)
        )
    }

    // ─────────────────────────────────────────────
    // (b) 渲染器 mask 接线 + PurismClippingManager 调用点（静态源码扫描，AND4-P1 结构断言）
    //     当前（probe0）渲染器恒 NORMAL 不裁剪 → 本组 RED
    // ─────────────────────────────────────────────

    /** 从任一 CWD（app 模块目录 / 仓库根）上溯定位 Live2DRenderer.kt 源文件。 */
    private fun locateLive2DRendererSource(): File {
        var dir: File? = File(requireNotNull(System.getProperty("user.dir"))).absoluteFile
        val candidates = listOf(
            "app/src/main/java/com/live2dai/android/Live2DRenderer.kt",
            "Live2Dai-Android/app/src/main/java/com/live2dai/android/Live2DRenderer.kt",
        )
        while (dir != null) {
            for (rel in candidates) {
                val c = File(dir, rel)
                if (c.isFile && c.name == "Live2DRenderer.kt") return c
            }
            dir = dir.parentFile
        }
        throw AssertionError(
            "未定位到 Live2DRenderer.kt（user.dir=" + System.getProperty("user.dir") + "）"
        )
    }

    private val rendererSource: String by lazy { locateLive2DRendererSource().readText() }

    @Test
    fun and4_p1_source_binds_purismClippingManager() {
        assertTrue(
            "AND4-P1 要求渲染器接线 PurismClippingManager（H1 mask 修复引入；当前 RED：未绑定）。\n" +
                "源文件=" + locateLive2DRendererSource().absolutePath,
            rendererSource.contains("PurismClippingManager")
        )
    }

    @Test
    fun and4_p1_source_calls_prepareFrame() {
        assertTrue(
            "AND4-P1 要求 PurismClippingManager.prepareFrame 存在调用点（结构断言）；当前 RED。\n" +
                "源文件=" + locateLive2DRendererSource().absolutePath,
            rendererSource.contains("prepareFrame")
        )
    }

    @Test
    fun and4_p1_source_calls_renderMaskBuffers() {
        assertTrue(
            "AND4-P1 要求 PurismClippingManager.renderMaskBuffers 存在调用点（结构断言）；当前 RED。\n" +
                "源文件=" + locateLive2DRendererSource().absolutePath,
            rendererSource.contains("renderMaskBuffers")
        )
    }

    @Test
    fun and4_p1_source_dispatches_masked_shaders() {
        // AND4-P1: drawObjectLoop 对 maskCount>0 分派 MASKED shader。
        // 锚点用枚举字面 token `ShaderType.MASKED`（分派目标）而非裸 `MASKED`（避免命中注释/字符串噪音）。
        assertTrue(
            "AND4-P1 要求渲染器分派 ShaderType.MASKED shader（maskCount>0）；当前 RED（probe0：恒 NORMAL）。\n" +
                "源文件=" + locateLive2DRendererSource().absolutePath,
            rendererSource.contains("ShaderType.MASKED")
        )
    }

    @Test
    fun and4_p1_source_dispatches_masked_inverted_shaders() {
        assertTrue(
            "AND4-P1 要求渲染器分派 ShaderType.MASKED_INVERTED shader（maskCount>0）；当前 RED。\n" +
                "源文件=" + locateLive2DRendererSource().absolutePath,
            rendererSource.contains("ShaderType.MASKED_INVERTED")
        )
    }
}
