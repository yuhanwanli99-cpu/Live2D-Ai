package com.live2dai.android

import java.io.File
import kotlin.math.abs
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * M-AND-2 嘴型算法（RMS 幅度驱动）— RED 测试（AND2-A1~A8）。
 *
 * 依据：.pi/spoq/contract-linkage-android.md §2 M-AND-2（唯一真相源；拍板记录 §3 确认点2：选型 1-B
 * RMS 幅度驱动，替换 LipSyncMath.randomizedMouth 伪随机）。
 *
 * 契约定义的纯函数 seam（tester RED 锚，沿用 LipSyncMath 现有模式）：
 *  - rmsNormalized(samples: FloatArray): Float   —— sqrt(mean(x²))，空数组 → 0
 *  - applyEnvelope(rms: Float, prev: Float, dtMs: Long): Float —— attack/release 包络
 *  - SILENCE_THRESHOLD = 0.02f（AITuberFlow researched，来源 URL 见契约 AND2-A3）
 *  - LIP_SYNC_INTERVAL_MS（A6，主写 = 100ms）
 *
 * 数值来源（search-report-lipsync-standard.md F4，researched）：
 *  - kugutu: attackMs=50 / releaseMs=130 → 标准指数包络时间常数标记：
 *      attack 达 1 − e^(−1) ≈ 0.63×目标 于 t = attackMs（=50ms，±20% 窗 40~60ms）
 *      release 衰减至 e^(−1) ≈ 0.37×峰值 于 t = releaseMs（=130ms，±20% 窗 104~156ms）
 *  - AITuberFlow: threshold=0.02 → SILENCE_THRESHOLD
 *
 * RED 状态：
 *  - 纯函数 seam `rmsNormalized` / `applyEnvelope` / `SILENCE_THRESHOLD` / `LIP_SYNC_INTERVAL_MS`
 *    为 M-AND-2 待引入契约 seam（契约定义、当前 LipSyncMath 尚未实现）→ 编译期 RED。
 *  - A2 结构断言（生产 lip 路径不得引用 Random/randomizedMouth）：静态源码扫描（契约 AND2-A2
 *    明示「tester 静态 grep 生产调用路径」授权），当前 LipSyncMath.kt 仍以 randomizedMouth
 *    伪随机作主随机源（search-report 现状锚点）→ 运行期 RED。
 *
 * 纪律：禁读 src/main「对答案出题」。源码扫描锚点全部取自契约 AND2-A2 字面（Random / randomizedMouth /
 * 生产 lip 调用路径），非实现细节。[UNSURE] 见 RED 报告（A3 门控 seam、A6 真实时钟 API、seam 宿主名）。
 */
class MAnd2RmsLipSyncTest {

    // ─────────────────────────────────────────────
    // AND2-A1: rmsNormalized ∈[0,1]；全零→0；满幅 1.0→1.0；空数组→0；无 NaN/Inf/除零
    // ─────────────────────────────────────────────

    @Test
    fun and2_a1_rmsNormalized_zeroForAllZeros() {
        val allZero = FloatArray(4800) { 0f }
        assertEquals("全零样本 rms 应 = 0.0", 0f, LipSyncMath.rmsNormalized(allZero), 1e-6f)
    }

    @Test
    fun and2_a1_rmsNormalized_oneForFullScale() {
        val fullScale = FloatArray(4800) { 1.0f }
        assertEquals(
            "满幅 1.0 样本 rms 应 = 1.0（sqrt(mean(1)) = 1），实际 " +
                LipSyncMath.rmsNormalized(fullScale),
            1.0f, LipSyncMath.rmsNormalized(fullScale), 1e-6f
        )
    }

    @Test
    fun and2_a1_rmsNormalized_zeroForEmptyArray() {
        assertEquals("空数组 rms 应 = 0.0（契约：空数组 → 0，无除零）", 0f, LipSyncMath.rmsNormalized(FloatArray(0)), 1e-6f)
    }

    @Test
    fun and2_a1_rmsNormalized_inUnitInterval_noNaNOrInf() {
        // 含负系数混合样本（幅度驱动，x² 归一化；残差应在 [0,1]）
        val mixed = FloatArray(4000) { if (it % 2 == 0) 0.5f else -0.5f }
        val half = LipSyncMath.rmsNormalized(mixed)
        assertTrue("sqrt(mean(0.25)) = 0.5，实际 $half", abs(half - 0.5f) < 1e-3f)
        // 全样本带扫：任意有界输入下 rms ∈ [0,1] 且有限
        for (f in listOf(0f, 0.1f, 0.7f, 1.0f)) {
            val v = LipSyncMath.rmsNormalized(FloatArray(100) { f })
            assertTrue("rms 应 ∈ [0,1]，rms($f)=$v", v >= 0f && v <= 1f + 1e-6f)
            assertTrue("rms 不得为 NaN/Inf，rms=$v", v.isFinite())
        }
    }

    // ─────────────────────────────────────────────
    // AND2-A2: 确定性（同输入双跑一致）+ 生产 lip 循环不引用 Random/randomizedMouth（结构断言）
    // ─────────────────────────────────────────────

    @Test
    fun and2_a2_rmsNormalizedDeterministicSameInputDoubleRun() {
        val samples = FloatArray(8000) { (it % 7) / 7f - 0.4f }
        assertEquals(
            "同输入 rmsNormalized 双跑应完全一致（确定性）",
            LipSyncMath.rmsNormalized(samples), LipSyncMath.rmsNormalized(samples), 0f
        )
    }

    @Test
    fun and2_a2_applyEnvelopeDeterministicSameInputDoubleRun() {
        var prev = 0f
        for (i in 0 until 100) prev = LipSyncMath.applyEnvelope(0.6f, prev, 100L)
        // 同 (rms, prev, dtMs) 双跑应逐等（确定性纯函数）
        val a = LipSyncMath.applyEnvelope(0.3f, prev, 25L)
        val b = LipSyncMath.applyEnvelope(0.3f, prev, 25L)
        assertEquals("applyEnvelope 同参双跑应一致", a, b, 0f)
    }

    /** 从任一 CWD（app 模块目录 / 仓库根）上溯定位 src/main 下的源文件。 */
    private fun locateSource(fileName: String): File {
        var dir: File? = File(requireNotNull(System.getProperty("user.dir"))).absoluteFile
        val roots = listOf("app", "Live2Dai-Android")
        while (dir != null) {
            for (root in roots) {
                val cand = File(dir, "$root/src/main/java/com/live2dai/android/$fileName")
                if (cand.isFile) return cand
            }
            dir = dir.parentFile
        }
        throw AssertionError("未定位到 $fileName（user.dir=" + System.getProperty("user.dir") + "）")
    }

    private val lipSyncMathSource: String by lazy { locateSource("LipSyncMath.kt").readText() }
    private val voiceIoSource: String by lazy {
        // 生产 lip 驱动路径宿主（search-report 现状锚点：VoiceIoController.kt 持 LIP_SYNC_INTERVAL_MS /
        // 驱动嘴型循环）；契约 AND2-A2 明示「tester 静态 grep 生产调用路径」。
        locateSource("VoiceIoController.kt").readText()
    }

    @Test
    fun and2_a2_source_lipValuePath_doesNotReferenceRandom() {
        // 生产 lip 值生成不得依赖 Random（伪随机源已由 RMS 幅度替换）
        assertTrue(
            "AND2-A2：LipSyncMath.kt 嘴型值路径不得引用 kotlin.random.Random（伪随机已替换为 RMS）；当前 RED。\n" +
                "源=" + locateSource("LipSyncMath.kt").absolutePath,
            !lipSyncMathSource.contains("kotlin.random.Random")
        )
    }

    @Test
    fun and2_a2_source_lipDrivePath_doesNotCallRandomizedMouth() {
        // 生产 lip 驱动路径不得再调用 randomizedMouth（伪随机嘴型被替换）
        assertTrue(
            "AND2-A2：生产 lip 调用路径（LipSyncMath.kt / VoiceIoController.kt）不得调用 randomizedMouth；当前 RED。\n" +
                "LipSyncMath.kt=" + locateSource("LipSyncMath.kt").absolutePath + "\n" +
                "VoiceIoController.kt=" + locateSource("VoiceIoController.kt").absolutePath,
            !lipSyncMathSource.contains("randomizedMouth") && !voiceIoSource.contains("randomizedMouth")
        )
    }

    // ─────────────────────────────────────────────
    // AND2-A3: 静音阈值 0.02 两侧行为（RMS<0.02 → 口型归 0；RMS≥0.02 → 输出>0）
    // ─────────────────────────────────────────────

    @Test
    fun and2_a3_silenceThreshold_constant_is_0_02() {
        assertEquals(
            "SILENCE_THRESHOLD 应 = 0.02f（AITuberFlow researched，契约 AND2-A3）",
            0.02f, LipSyncMath.SILENCE_THRESHOLD, 0f
        )
    }

    @Test
    fun and2_a3_rmsBelowThreshold_mapsToSilence_outputZero() {
        val below = 0.015f  // < 0.02 → 静音
        val gatedTarget = if (below < LipSyncMath.SILENCE_THRESHOLD) 0f else below
        // 门控后目标 0 → 包络收敛到 0（静音不张嘴）
        var out = 0.5f  // 先打到非零，再喂静音
        out = LipSyncMath.applyEnvelope(gatedTarget, out, 100L)
        out = LipSyncMath.applyEnvelope(gatedTarget, out, 100L)
        out = LipSyncMath.applyEnvelope(gatedTarget, out, 100L)
        assertTrue("RMS<0.02 应口型归 0（静音），实际 $out", out < 0.05f)
    }

    @Test
    fun and2_a3_rmsAtOrAboveThreshold_outputPositive() {
        val above = 0.025f  // ≥ 0.02（且 >0）→ 非静音
        val gatedTarget = if (above < LipSyncMath.SILENCE_THRESHOLD) 0f else above
        assertTrue("门控断言前置：0.025 ≥ 0.02 → 目标应 > 0", gatedTarget > 0f)
        var out = 0f
        // attack 若干步使输出升至 >0
        repeat(10) { out = LipSyncMath.applyEnvelope(gatedTarget, out, 10L) }
        assertTrue("RMS≥0.02 应输出 > 0（张嘴），实际 $out", out > 0f)
        // 恰好 0.02（边界含）也应为非静音（>0）
        val boundary = LipSyncMath.SILENCE_THRESHOLD
        val gatedBoundary = if (boundary < LipSyncMath.SILENCE_THRESHOLD) 0f else boundary
        assertTrue("恰好 0.02 应为非静音（目标 > 0）", gatedBoundary > 0f)
    }

    // ─────────────────────────────────────────────
    // AND2-A4: attack/release 包络（attackMs=50→0.63×目标@50ms±20%；releaseMs=130→0.37×峰值@130ms±20%）
    // ─────────────────────────────────────────────

    @Test
    fun and2_a4_attackReaches0_63Target_within50msWindow() {
        // 阶跃输入 RMS 0→1.0（prev 从闭嘴 0 起步），小步 dt=1ms 逼近连续时间，
        // 首次输出达 0.63×目标的时刻应 ∈ [40ms, 60ms]（50ms±20%；1−e^(−1)≈0.632）。
        var prev = 0f
        var elapsed = 0L
        var reached = -1L
        for (step in 1..160) {
            prev = LipSyncMath.applyEnvelope(1.0f, prev, 1L)
            elapsed += 1
            if (prev >= 0.63f && reached < 0) reached = elapsed
        }
        assertTrue(
            "attack（RMS 0→1.0）达 0.63×目标 应在 50ms±20%（40~60ms），实际 $reached ms",
            reached in 40L..60L
        )
    }

    @Test
    fun and2_a4_releaseDecaysTo0_37Peak_within130msWindow() {
        // 峰值 1.0 后输入归 0（静音），小步 dt=1ms 逼近连续时间，
        // 首次输出衰减至 0.37×峰值 的时刻应 ∈ [104ms, 156ms]（130ms±20%；e^(−1)≈0.368）。
        var prev = 1.0f  // 峰值（发声到顶）
        var elapsed = 0L
        var reached = -1L
        for (step in 1..320) {
            prev = LipSyncMath.applyEnvelope(0.0f, prev, 1L)
            elapsed += 1
            if (prev <= 0.37f && reached < 0) reached = elapsed
        }
        assertTrue(
            "release（峰值 1.0 → 输入 0）衰减至 0.37×峰值 应在 130ms±20%（104~156ms），实际 $reached ms",
            reached in 104L..156L
        )
    }

    // ─────────────────────────────────────────────
    // AND2-A5: 写入 mouth-open 参数值 ∈ [0,1]（攻击/平滑后不越界，输出 clamp）
    // ─────────────────────────────────────────────

    @Test
    fun and2_a5_outputStaysInUnitIntervalUnderAgressiveInputs() {
        // 长时间序列 + 剧烈输入变化，输出恒 ∈ [0,1]（mouth-open 参数值域，不越界/不炸成 NaN/Inf）
        var prev = 0f
        val rmsSeq = FloatArray(500) { if (it % 3 == 0) 1.0f else if (it % 3 == 1) 0f else 0.4f }
        for (dt in listOf(1L, 8L, 100L)) {
            var out = 0f
            for (rms in rmsSeq) {
                out = LipSyncMath.applyEnvelope(rms, out, dt)
                assertTrue("mouth-open 输出应 ∈ [0,1]，实际 $out", out >= 0f && out <= 1f + 1e-6f)
                assertTrue("输出不得为 NaN/Inf，实际 $out", out.isFinite())
            }
        }
    }

    // ─────────────────────────────────────────────
    // AND2-A6: 更新周期 ∈ [8.3ms,100ms] 且由真实时钟驱动（非帧号）
    // ─────────────────────────────────────────────

    @Test
    fun and2_a6_updatePeriodWithin83to100ms() {
        // 契约 AND2-A6：更新周期 ∈ [8.3ms, 100ms]（MotionSync SampleRate 1.0~120fps 区间；主写保持 100）。
        // 宿主：LIP_SYNC_INTERVAL_MS 现状挂 VoiceIoController（search-report 现状锚点，契约 A6 主写=100）
        assertTrue(
            "LIP_SYNC_INTERVAL_MS 应 ∈ [8, 100]ms，实际 ${VoiceIoController.LIP_SYNC_INTERVAL_MS}",
            VoiceIoController.LIP_SYNC_INTERVAL_MS in 8L..100L
        )
    }

    @Test
    fun and2_a6_productionDriveUsesRealClockNotFrameIndex() {
        // A6「由真实时钟驱动（非帧号计数）」。结构断言（契约 AND2-A2 授权静态 grep）：
        // 生产 lip 驱动必须引用真实时钟（elapsedRealtimeMillis/currentTimeMillis/nanoTime/uptimeMillis），
        // 且不得以帧号作为嘴型值种子驱动的来源。
        // [UNSURE] 契约未指定生产驱动宿主文件与具体时钟 API token → 此处按 LipSyncMath.kt / VoiceIoController.kt
        //   生产调用路径扫描至少一方含真实时钟 token；若涉及宿主/API 名称差异，判读报告如实记录（禁编造适配）。
        val driveSource = lipSyncMathSource + "\n" + voiceIoSource
        val hasRealClock =
            driveSource.contains("elapsedRealtimeMillis") ||
                driveSource.contains("currentTimeMillis") ||
                driveSource.contains("nanoTime") ||
                driveSource.contains("uptimeMillis") ||
                driveSource.contains("System.currentTimeMillis")
        assertTrue(
            "AND2-A6：生产 lip 驱动应含真实时钟（非帧号）引用；当前 RED。\n" +
                "源=" + locateSource("LipSyncMath.kt").absolutePath + " / " +
                locateSource("VoiceIoController.kt").absolutePath,
            hasRealClock
        )
    }

    // ─────────────────────────────────────────────
    // AND2-A7: 相邻输出差 ≤ 0.2（防跳变；平滑回归）+ 全程无 NaN/Inf
    // ─────────────────────────────────────────────

    @Test
    fun and2_a7_adjacentOutputDeltaLeq0_2DuringSpeech() {
        // 连续语音包络（RMS 逐帧小幅缓变）→ 相邻输出差 ≤0.2（防跳变；attack/release 平滑下自然满足），
        // 且全程无 NaN/Inf。更新间隔取契约 A6 主写 100ms。
        val speechRms = FloatArray(24) { 0.2f + (it % 5) * 0.05f }  // 0.2~0.4 缓变
        var out = 0f
        var prior = 0f
        var first = true
        for (rms in speechRms) {
            out = LipSyncMath.applyEnvelope(rms, out, 100L)
            if (!first) {
                assertTrue(
                    "相邻输出差应 ≤0.2（防跳变），实际 |$out − $prior| = ${abs(out - prior)}",
                    abs(out - prior) <= 0.2f
                )
            }
            assertTrue("输出不得为 NaN/Inf，实际 $out", out.isFinite())
            assertTrue("输出应 ∈ [0,1]，实际 $out", out >= 0f && out <= 1f + 1e-6f)
            prior = out
            first = false
        }
    }

    // ─────────────────────────────────────────────
    // AND2-A8: 连续 1s 全零 PCM → 收敛 < 0.05（静音不张嘴、不残留）
    // ─────────────────────────────────────────────

    @Test
    fun and2_a8_oneSecondSilenceConvergesBelow0_05() {
        // 起始于发声峰值（先 attack 打到 ≥0.95），随后连续 1s 全零 PCM → 输出收敛 < 0.05
        var out = 0f
        // 发声到顶（attack 多步）
        repeat(200) { out = LipSyncMath.applyEnvelope(1.0f, out, 1L) }
        assertTrue("前置：先发声到高值，实际 $out", out >= 0.95f)
        // 连续 1s 全零 PCM → rms = 0
        val oneSecondZeros = FloatArray(48000) { 0f }  // 1s @ 48kHz
        val rmsZero = LipSyncMath.rmsNormalized(oneSecondZeros)
        assertEquals("全零 PCM 的 rms 应 = 0", 0f, rmsZero, 1e-6f)
        // 以更新周期（100ms）步进释放 1s
        var elapsedMs = 0L
        while (elapsedMs < 1000L) {
            out = LipSyncMath.applyEnvelope(rmsZero, out, 100L)
            elapsedMs += 100L
        }
        assertTrue(
            "连续 1s 全零 PCM 后输出应收敛 < 0.05（静音不张嘴、不残留），实际 $out",
            out < 0.05f
        )
    }
}
