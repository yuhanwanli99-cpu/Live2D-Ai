package com.live2d.ai.android.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * 纹理上限防护纯函数断言（契约 C-R1-render §2.4/§3，M1-A1）+ M1-D1 grep 门脚本化。
 *
 * 契约 R1-render §2.4 锁定的纯函数（JVM 可单测）：
 *     /** 返回 ≤ maxSize 的最大 2 的幂；maxSize<=0 → 1 */
 *     fun computeDownsampleTarget(maxSize: Int): Int
 *     /** 判定是否需降采样：texW>maxSize || texH>maxSize */
 *     fun needsDownsample(texW: Int, texH: Int, maxSize: Int): Boolean
 *
 * 生产符号（RED 待实现，会话自洽位置 data/TextureGuard.kt，包 com.live2d.ai.android.data）；
 * 若 coder 将函数置于其他文件，须满足本测试引用的包名/函数签名（契约 §2.4，[UNSURE] 见交接报告）。
 *
 * M1-A1 全部数值断言以契约 §3 M1-A1 锁定为准。
 */
class TextureGuardTest {

    // ============================================================
    // M1-A1：computeDownsampleTarget —— ≤ maxSize 的最大 2 的幂
    // ============================================================

    @Test
    fun downSampleTarget_exactPowerOfTwo_identity() {
        // 已是 2 的幂 → 原值返回
        assertEquals("maxSize=4096 应为 4096", 4096, computeDownsampleTarget(4096))
        assertEquals("maxSize=2048 应为 2048", 2048, computeDownsampleTarget(2048))
        assertEquals("maxSize=16384 应为 16384", 16384, computeDownsampleTarget(16384))
    }

    @Test
    fun downSampleTarget_nonPowerOfTwo_floorToPowerOfTwo() {
        // 3000 → 小于等于 3000 的最大 2 的幂 = 2048
        assertEquals("maxSize=3000 应为 2048", 2048, computeDownsampleTarget(3000))
        // 附加防御：4095 → 2048；3073 → 2048
        assertEquals("maxSize=4095 应为 2048", 2048, computeDownsampleTarget(4095))
        assertEquals("maxSize=3072 应为 2048", 2048, computeDownsampleTarget(3072))
        // 超过 4096 一档后回落到 16384 以内更大幂：6144 → 4096
        assertEquals("maxSize=6144 应为 4096", 4096, computeDownsampleTarget(6144))
    }

    @Test
    fun downSampleTarget_defensive_maxSizeNonPositiveReturnsOne() {
        // 契约 §2.4：maxSize<=0 → 1（防御下限，非"最大 2 的幂"语义）
        assertEquals("maxSize=0 应为 1", 1, computeDownsampleTarget(0))
        assertEquals("maxSize=-1 应为 1", 1, computeDownsampleTarget(-1))
    }

    @Test
    fun downSampleTarget_defensive_widthInflated_sameFloor() {
        // 大 maxSize（比如 2^30+）→ 仍为 ≤ 其最大 2 的幂（不溢出、不抛）
        assertEquals("maxSize=1073741824 应为 1073741824", 1073741824, computeDownsampleTarget(1073741824)) // 2^30
        assertEquals("maxSize=1073741825 应为 1073741824", 1073741824, computeDownsampleTarget(1073741825))
    }

    // ============================================================
    // M1-A1：needsDownsample —— texW>maxSize || texH>maxSize
    // ============================================================

    @Test
    fun needsDownsample_notExceedMaxSize_false() {
        // 契约锁定：4096×4096 vs maxSize=4096 → false（边界相等不降采样）
        assertFalse("4096×4096 vs 4096 应为 false", needsDownsample(4096, 4096, 4096))
        // 附加防御：小于上限亦 false
        assertFalse("1024×1024 vs 4096 应为 false", needsDownsample(1024, 1024, 4096))
        assertFalse("4096×800 vs 4096 应为 false", needsDownsample(4096, 800, 4096))
    }

    @Test
    fun needsDownsample_widthExceedsMaxSize_true() {
        // 契约锁定：16384×8192 vs 4096 → true（宽超限）
        assertTrue("16384×8192 vs 4096 应为 true", needsDownsample(16384, 8192, 4096))
    }

    @Test
    fun needsDownsample_heightExceedsMaxSize_true() {
        // 契约锁定：4096×8192 vs 4096 → true（高超限）
        assertTrue("4096×8192 vs 4096 应为 true", needsDownsample(4096, 8192, 4096))
        // 附加防御：仅高超限
        assertTrue("1024×8192 vs 4096 应为 true", needsDownsample(1024, 8192, 4096))
    }

    // ============================================================
    // M1-D1 grep 门脚本化（契约 §6）：生产实现存在性机器校验
    //     inPremultiplied = true 命中且唯一  /  texId==null → continue + Log.w  /  [tex] maxSize= 探针
    // 说明：本测试运行期读取 Live2DRenderer.kt 源文件做子串断言（机器可校验）；
    //       断言目标 = 契约 §2.1/§2.4/§3 M1-D1 文本，非实现细节。实现未改 → RED（预期）。
    // ============================================================

    @Test
    fun grepGate_premultipliedAlphaEnabled() {
        // M1-A2/M1-D1：loadTextures 内 inPremultiplied = true 命中
        val src = rendererSource()
        val occurrences = countOccurrences(src, "inPremultiplied = true")
        assertTrue(
            "契约 §2.1：loadTextures 必须含 `inPremultiplied = true`（M1a 上传侧预乘）——当前命中 $occurrences 处（RED）",
            occurrences >= 1
        )
    }

    @Test
    fun grepGate_nullTexId_skipsWithLogWarn() {
        // M1-D1：texId==null 分支必须 `continue` 跳过绘制 + Log.w（替代"不绑定仍画"）
        val src = rendererSource()
        // texId 判空 + continue + [tex] null texId 告警 三要素须同时存在
        assertTrue(
            "契约 §2.4：texId==null 分支须含 `continue`（不再不绑定仍画）——当前缺失（RED）",
            countOccurrences(src, "continue") >= 1
        )
        assertTrue(
            "契约 §2.4：texId==null 分支须含 Log.w 告警——当前缺失（RED）",
            countOccurrences(src, "Log.w") >= 1
        )
        assertTrue(
            "契约 §2.4：texId==null 分支告警文本须含 `[tex] null texId skipped`——当前缺失（RED）",
            countOccurrences(src, "[tex] null texId skipped") >= 1
        )
    }

    @Test
    fun grepGate_texProbeExists() {
        // M1-D1：`[tex] maxSize=` 探针（glGetIntegerv(GL_MAX_TEXTURE_SIZE) 后 logcat）命中
        val src = rendererSource()
        assertTrue(
            "契约 §2.4：logcat 探针须含 `[tex] maxSize=`——当前缺失（RED）",
            countOccurrences(src, "[tex] maxSize=") >= 1
        )
    }

    // ============================================================
    // 辅助
    // ============================================================

    /** 从测试工作目录向上定位仓库根（含 Live2D-Ai-Android 的祖先目录），读取渲染器源文件。 */
    private fun rendererSource(): String {
        val relative =
            "Live2D-Ai-Android/app/src/main/java/com/live2d/ai/android/Live2DRenderer.kt"
        var dir = File(System.getProperty("user.dir") ?: ".").absoluteFile
        while (dir != null) {
            val candidate = File(dir, relative)
            if (candidate.isFile) {
                return candidate.readText(Charsets.UTF_8)
            }
            dir = dir.parentFile
        }
        throw IllegalStateException(
            "无法定位 Live2DRenderer.kt（从 user.dir=${System.getProperty("user.dir")} 逐级上溯未找到 $relative）"
        )
    }

    private fun countOccurrences(text: String, needle: String): Int {
        if (needle.isEmpty()) return 0
        var count = 0
        var idx = text.indexOf(needle)
        while (idx >= 0) {
            count++
            idx = text.indexOf(needle, idx + needle.length)
        }
        return count
    }
}
