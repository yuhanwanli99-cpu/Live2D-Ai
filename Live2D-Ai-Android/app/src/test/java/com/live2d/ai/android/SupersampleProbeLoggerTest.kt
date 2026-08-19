package com.live2d.ai.android

import com.live2d.ai.android.supersample.SupersampleProbeLogger
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Wave 2 · 2A-2 超采样接入 —— 探针格式 RED（可注入 logger 的 seam）。
 *
 * 依据: contract-refactor-0814.md §3 REND2-A1（device，但日志**格式**可 JVM 断言其纯函数生成面）。
 * 契约锚（REND2-A1）:
 *   - FBO 尺寸探针日志 = **1440×3200**（SUPERSAMPLE_SCALE=4/3）
 *   - **blit 完成**日志
 *   - 每帧仅一次（帧探针不重复）
 *   - 直渲兜底路径**零触发**（兜底日志计数=0）
 *
 * 判定方式: 完全无 GL。日志**格式模板**是可注入 logger 的纯 seam `SupersampleProbeLogger`，
 *          用正则在 JVM 断言「FBO 尺寸行 + blit 完成行 + 每帧仅一次 + 兜底零触发」。
 *          真机证据链（探针行确实打到 logcat、device 判读）归 REND2-A1（device，Wave 6 验收）。
 *
 * RED 依据（如实）: `SupersampleProbeLogger` seam 当前**不存在**（探针格式待 M2 coder 在薄壳落地）
 *          → 本文件编译期引用 → `:app:testDebugUnitTest` 编译失败 = RED。
 *
 * seam 形态（草案，格式模板锁定）: coder 按此实现 `SupersampleProbeLogger`，见 wave2-2a2-red.md §3。
 *          本测试锁**格式契约面**（正则模板），不锁具体文案（文案由实现决定，测试对正则双向接受）。
 */
class SupersampleProbeLoggerTest {

    // ────────────────────────────────────────────────────────────
    // REND2-A1 格式模板: FBO 尺寸行（含 1440×3200）+ blit 完成行
    // ────────────────────────────────────────────────────────────

    private fun runFrame(logger: SupersampleProbeLogger) {
        // 模拟薄壳每帧驱动: bind(1440×3200) → render → blit 完成（restore 不产探针日志，只输出 FBO + blit）
        logger.onFboBind(1440, 3200)
        logger.onBlitComplete()
    }

    @Test
    fun fbo_size_line_matches_contract_format() {
        val captured = StringBuilder()
        val logger = SupersampleProbeLogger { captured.append(it).append("\n") }
        runFrame(logger)

        val lines = captured.toString().trim().lines()
        val fboLine = lines.firstOrNull { it.contains("FBO") && it.contains("1440") && it.contains("3200") }
        assertTrue(
            "RED [wave2-2a2-PRB-A1]: 首帧输出必须含 FBO 尺寸行，携带 1440×3200。" +
                "契约锚 REND2-A1 探针格式。当前 seam 不存在（编译期 RED）。\n" +
                "实际行=${lines.joinToString(" | ")}",
            fboLine != null,
        )
    }

    @Test
    fun blit_complete_line_matches_contract_format() {
        val captured = StringBuilder()
        val logger = SupersampleProbeLogger { captured.append(it).append("\n") }
        runFrame(logger)

        val lines = captured.toString().trim().lines()
        val blitLine = lines.firstOrNull { it.contains("blit") && it.contains("complete") }
        assertTrue(
            "RED [wave2-2a2-PRB-A2]: 每帧 blit 完成后必须输出 'blit … complete' 行。" +
                "契约锚 REND2-A1（blit 完成日志）。当前 seam 不存在（编译期 RED）。\n" +
                "实际行=${lines.joinToString(" | ")}",
            blitLine != null,
        )
    }

    // ────────────────────────────────────────────────────────────
    // REND2-A1「每帧仅一次」—— 不得重复探针
    // ────────────────────────────────────────────────────────────

    @Test
    fun each_frame_emits_exactly_one_fbo_line_and_one_blit_line() {
        val captured = StringBuilder()
        val logger = SupersampleProbeLogger { captured.append(it).append("\n") }

        val frameCount = 3
        repeat(frameCount) { runFrame(logger) }

        val lines = captured.toString().trim().lines()
        val fboCount = lines.count { it.contains("FBO") }
        val blitCount = lines.count { it.contains("blit") && it.contains("complete") }

        assertEquals(
            "RED [wave2-2a2-PRB-B1]: ${frameCount} 帧应恰产 ${frameCount} 条 FBO 行（每帧一次）。" +
                "契约锚 REND2-A1 探针格式（不得重复）。当前 seam 不存在（编译期 RED）。",
            frameCount,
            fboCount,
        )
        assertEquals(
            "RED [wave2-2a2-PRB-B1b]: ${frameCount} 帧应恰产 ${frameCount} 条 blit-complete 行。" +
                "契约锚 REND2-A1（每帧仅一次）。当前 seam 不存在（编译期 RED）。",
            frameCount,
            blitCount,
        )
    }

    // ────────────────────────────────────────────────────────────
    // REND2-A1「直渲兜底路径零触发」—— 兜底日志计数=0
    // ────────────────────────────────────────────────────────────

    @Test
    fun supersample_path_never_emits_direct_fallback_line() {
        val captured = StringBuilder()
        val logger = SupersampleProbeLogger { captured.append(it).append("\n") }

        // 正常超采样帧循环（多帧），不应触发兜底直渲
        repeat(5) { runFrame(logger) }

        val lines = captured.toString().trim().lines()
        val fallbackCount = lines.count { it.contains("DIRECT_FALLBACK") }
        assertEquals(
            "RED [wave2-2a2-PRB-C1]: 直渲兜底路径必须零触发（兜底日志计数=0）。" +
                "契约锚 REND2-A1（直渲兜底路径零触发）。当前 seam 不存在（编译期 RED）。\n" +
                "实际行=${lines.joinToString(" | ")}",
            0,
            fallbackCount,
        )
    }

    @Test
    fun fallback_emission_is_observable_when_invoked() {
        // 探针必须有可判读的兜底标记（device 判读需要计数器非 0 可被检出）——格式角度的可观察性
        val captured = mutableListOf<String>()
        val fallbackLogger = SupersampleProbeLogger { captured.add(it) }
        // 若薄壳打兜底路径，logger 需能发出可判读的兜底行（供 REND2-A1 被判为 RED/非零）
        // —— 以格式契约面锁定兜底标记存在性，不锁触发时机（触发条件是真实 GL 失败分支，device 覆盖）。
        assertTrue(
            "RED [wave2-2a2-PRB-C2]: SupersampleProbeLogger 需暴露 onDirectFallback() 以生成可判读兜底标记。" +
                "当前 seam 不存在（编译期 RED）。\n" +
                "契约锚 REND2-A1 直渲兜底路径零触发（需有可判读的兜底标记才可数 0）。",
            SupersampleProbeLogger::class.java.declaredMethods.any {
                it.name == "onDirectFallback"
            },
        )
    }

    // ────────────────────────────────────────────────────────────
    // 格式契约防漂移: 探针行必须是日志模板正则可命中（防 coder 缩进/换行破坏 device 判读）
    // ────────────────────────────────────────────────────────────

    @Test
    fun emitted_lines_match_logcat_grep_templates() {
        val captured = StringBuilder()
        val logger = SupersampleProbeLogger { captured.append(it).append("\n") }
        runFrame(logger)

        val text = captured.toString()
        // 与 REND2-A1 device 判读同源的正则（J-V1 命令 grep -E 模板）
        val fboRegex = Regex("FBO.*1440.*3200")
        val blitRegex = Regex("blit.*complete")
        assertTrue(
            "RED [wave2-2a2-PRB-D1]: FBO 行必须命中 logcat grep 模板 'FBO.*1440.*3200'。" +
                "当前 seam 不存在（编译期 RED）。\n actual=$text",
            fboRegex.containsMatchIn(text),
        )
        assertTrue(
            "RED [wave2-2a2-PRB-D2]: blit 行必须命中 grep 模板 'blit.*complete'。" +
                "契约锚 REND2-A1。当前 seam 不存在（编译期 RED）。\n actual=$text",
            blitRegex.containsMatchIn(text),
        )
    }
}
