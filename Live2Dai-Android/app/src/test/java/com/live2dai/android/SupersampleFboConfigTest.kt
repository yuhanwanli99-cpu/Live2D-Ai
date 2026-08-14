package com.live2dai.android

import com.live2dai.android.supersample.FboSize
import com.live2dai.android.supersample.ProbeSink
import com.live2dai.android.supersample.SupersampleFboConfig
import com.live2dai.android.supersample.SupersampleProbeLogger
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Wave 2 · 2A-2 超采样接入 —— FBO 尺寸配置测试（纯函数，无 GL）。
 *
 * 依据: contract-refactor-0814.md §3 REND2-A1/A2 + architecture-refactor-render-0814.md F-4/§4.2。
 * 契约锚:
 *   - REND2-A1: FBO 尺寸探针日志 = **1440×3200**（SUPERSAMPLE_SCALE=4/3）。
 *   - REND2-A2: 输出分辨率 = 1080×2400（设备原生 iQOO 11 Pro）。
 *   - F-4: 2K FBO 1440×3200 超采样（SUPERSAMPLE_SCALE=4/3）+ mipmap 下采样 blit 回 1080p。
 *   - 比例恒等式: 1080 × 4/3 = **1440**，2400 × 4/3 = **3200**（整数缩放，无取整损失）。
 *
 * 判定方式: 纯 JVM 无 GL 配置解析/计算函数断言。生产 seam = `SupersampleFboConfig`
 *           （fboSizeData/isAtLeast1080p/preservesSurfaceAspect）+ `SupersampleProbeLogger`
 *           （onFboBound 探针行）。GL 侧真 FBO 尺寸证据归 REND2-A1（device，Wave 6）。
 */
class SupersampleFboConfigTest {

    // 契约锁定硬值（REND2-A1 × F-4）
    private companion object {
        const val SURFACE_W = 1080
        const val SURFACE_H = 2400
        const val FBO_W = 1440
        const val FBO_H = 3200
        const val SCALE = 4f / 3f
    }

    // 测试内局部助手：4/3 整数缩放恒等式（生产无 isIntegerScaling seam，测试按契约恒等式自算：
    // 3*FBO == 4*表面、宽高各自成立 ⇔ 无取整损失；Long 乘法防溢出）。
    private fun isIntegerScaling(size: FboSize, surfaceW: Int, surfaceH: Int): Boolean =
        3L * size.width == 4L * surfaceW && 3L * size.height == 4L * surfaceH

    // ────────────────────────────────────────────────────────────
    // REND2-A1: 1080×2400 表面 × 4/3 超采样 → 1440×3200 FBO（整数比例，无取整）
    // ────────────────────────────────────────────────────────────

    @Test
    fun computeFboSize_from_1080x2400_scale_four_thirds_is_1440x3200() {
        val size: FboSize = SupersampleFboConfig.fboSizeData(SURFACE_W, SURFACE_H, SCALE)
        assertEquals(
            "RED [wave2-2a2-CFG-A1]: ${SURFACE_W}×${SURFACE_H} × 4/3 必须算得 ${FBO_W}×${FBO_H}。契约锚 REND2-A1/F-4 硬值。",
            FboSize(FBO_W, FBO_H),
            size,
        )
    }

    @Test
    fun fboWidth_is_integer_multiple_of_surface_width() {
        // 整数倍: 1440 = 1080 × (4/3)，不得产生取整损失（否则锯齿/非整像素）
        val size: FboSize = SupersampleFboConfig.fboSizeData(SURFACE_W, SURFACE_H, SCALE)
        assertTrue(
            "RED [wave2-2a2-CFG-A2]: 超采样缩放必须是整数倍（1440/1080 整除、3200/2400 整除），" +
                "避免取整损失导致非整像素。契约锚: F-4 比例约束（SUPERSAMPLE_SCALE=4/3 整数缩放）。",
            isIntegerScaling(size, SURFACE_W, SURFACE_H),
        )
    }

    // ────────────────────────────────────────────────────────────
    // REND2-A2 + S3: 输出 ≥1080p（FBO 必须覆盖 1080×2400 下界）
    // ────────────────────────────────────────────────────────────

    @Test
    fun computeFboSize_is_at_least_full_hd_lower_bound() {
        val size: FboSize = SupersampleFboConfig.fboSizeData(SURFACE_W, SURFACE_H, SCALE)
        assertTrue(
            "RED [wave2-2a2-CFG-B1]: FBO(${size.width}×${size.height}) 必须 ≥ 1080p 下界（${SURFACE_W}×${SURFACE_H}），" +
                "否则清晰度硬指标 ≥1080p 不成立。契约锚: REND2-A2 + S3 '清晰度 ≥1080p 不得回退'。",
            SupersampleFboConfig.isAtLeast1080p(size.width, size.height),
        )
    }

    @Test
    fun probeLog_width_height_match_1440x3200() {
        // REND2-A1 探针日志必须携带 1440×3200 两个尺寸值（device 探针正则的纯函数面）
        val size: FboSize = SupersampleFboConfig.fboSizeData(SURFACE_W, SURFACE_H, SCALE)
        val lines = mutableListOf<String>()
        SupersampleProbeLogger(ProbeSink { line -> lines.add(line) }).onFboBound(size.width, size.height)
        val logLine = lines.single()
        assertTrue(
            "RED [wave2-2a2-CFG-C1]: FBO 探针日志行必须含 '1440' 与 '3200'（REND2-A1 日志模板）。\nlogLine=$logLine",
            logLine.contains("1440") && logLine.contains("3200"),
        )
    }

    // ────────────────────────────────────────────────────────────
    // 比例约束: 宽高同比例放大（4/3 × 4/3），不得破坏 2K 纵横比
    // ────────────────────────────────────────────────────────────

    @Test
    fun fbo_aspect_ratio_preserved() {
        val size: FboSize = SupersampleFboConfig.fboSizeData(SURFACE_W, SURFACE_H, SCALE)
        val surfaceAspect = SURFACE_W.toDouble() / SURFACE_H
        val fboAspect = size.width.toDouble() / size.height
        assertTrue(
            "RED [wave2-2a2-CFG-D1]: FBO 纵横比必须与表面一致（1440/3200 == 1080/2400），" +
                "同比例放大，避免图像拉伸。\n" +
                "surfaceAspect=$surfaceAspect, fboAspect=$fboAspect",
            SupersampleFboConfig.preservesSurfaceAspect(
                size.width, size.height,
                SupersampleFboConfig.SURFACE_WIDTH, SupersampleFboConfig.SURFACE_HEIGHT,
            ),
        )
    }
}
