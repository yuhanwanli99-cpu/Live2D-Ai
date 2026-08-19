package com.live2d.ai.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Wave 2 · 2B-2 帧链 T1 预发射+重锚（用户裁定 B-4 = 整段重锚）— RED 测试。
 *
 * 依据：`.pi/spoq/contract-refactor-0814.md` §2.2 嘴型/音频通道时序契约 +
 * `.pi/spoq/architecture-refactor-tts-0814.md` §2.3/§3.2（T1 重锚）：
 *   「播放同步 = 消费侧重锚：playStartWallClockMs + frame.timestampMs（clamp 下限 = playStartWallClockMs）」。
 * 用户裁定（Wave 2 · 2B-2 分派）：
 *   帧调度墙钟 = playStartWallClockMs + frameTsMs，clamp 下限 = playStartWallClockMs。
 *   —— 此裁定同时落幕 M-AND-1 遗留 [UNSURE]〈FORMULA〉（AND1-A3 曾写负值 clamp 0 的方向冲突），
 *      以「+ 且 clamp 下限 playStart」为最终公式。
 *
 * seam（既有，见 GREEN-verify-MAND1-T1.md：生产 `VoiceIoController.kt` 的 `LipSyncClock`）：
 *   `LipSyncClock.reanchor(frameTsMs: Long, playStartWallClockMs: Long): Long`
 *   （参数顺序 = 既有 MAnd1 契约 seam：frameTs 在前，playStart 在后——保持一致，勿改序。）
 *
 * 语义 reanchor(ts, playStart) = max(playStart, playStart + ts)：
 *   - frameTs < 0  → 钳制回 playStartWallClockMs（clamp 下限）
 *   - frameTs == 0 → 恰为 playStartWallClockMs（边界，首帧）
 *   - frameTs > 0  → playStartWallClockMs + frameTs
 *   - 严格非递减（frameTs 单调 → 重锚后墙钟单调，帧选择位置单调）
 *
 * 契约锚：TTS3-A4（严格非递减、首帧=0）、架 B §3.2（clamp 下限 playStart）。
 * RED 状态：当前 `reanchor` 是否满足上述「+ 且 clamp 下限 playStart」语义待判——若现状只做
 * AND1-A3 的「负值 clamp 0」而未钳到 playStart 语义，则运行期 RED。
 * 纪律：纯函数，不触碰 Android/GL；禁读实现代码对答案。
 */
class VoiceIoRefactorReanchorTest {

    // ─────────────────────────────────────────────
    // 边界 0：frameTs == 0 → 恰为 playStartWallClockMs
    // ─────────────────────────────────────────────

    @Test
    fun reanchor_firstFrameTsZero_mapsExactlyToPlayStart() {
        val playStart = 1_000_000L
        assertEquals(
            "首帧 frameTs=0 应恰映射到 playStartWallClockMs",
            playStart, LipSyncClock.reanchor(0L, playStart)
        )
    }

    // ─────────────────────────────────────────────
    // frameTs < 0 → 钳制（clamp 下限 = playStart）
    // ─────────────────────────────────────────────

    @Test
    fun reanchor_negativeFrameTs_clampsToPlayStart() {
        val playStart = 5_000L
        // 任意负偏移（含极端负）钳制回 playStart，不早于播放开始
        assertEquals(
            "frameTs=-150 应钳制到 playStart（clamp 下限）",
            playStart, LipSyncClock.reanchor(-150L, playStart)
        )
        assertEquals(
            "frameTs=-1 应钳制到 playStart",
            playStart, LipSyncClock.reanchor(-1L, playStart)
        )
        assertEquals(
            "frameTs=Long.MIN_VALUE 应钳制到 playStart（不溢出/不早于播放开始）",
            playStart, LipSyncClock.reanchor(Long.MIN_VALUE, playStart)
        )
    }

    @Test
    fun reanchor_negativeFrameTs_neverBeforePlayStart() {
        // 穷举若干负偏移：结果恒 ≥ playStart（墙钟不得早于播放开始）
        val playStart = 1_234_567L
        for (ts in listOf(-1L, -1_000L, -10_000L, -100_000_000L)) {
            val r = LipSyncClock.reanchor(ts, playStart)
            assertTrue(
                "frameTs=$ts 重锚后不得早于 playStart（$playStart），实际 $r",
                r >= playStart
            )
        }
    }

    // ─────────────────────────────────────────────
    // frameTs > 0 → playStart + frameTs
    // ─────────────────────────────────────────────

    @Test
    fun reanchor_positiveFrameTs_offsetsFromPlayStart() {
        val playStart = 8_000_000L
        assertEquals(
            "frameTs=150 应 = playStart + 150",
            playStart + 150L, LipSyncClock.reanchor(150L, playStart)
        )
        assertEquals(
            "frameTs=1500 应 = playStart + 1500（150ms×10 帧）",
            playStart + 1_500L, LipSyncClock.reanchor(1_500L, playStart)
        )
    }

    // ─────────────────────────────────────────────
    // 非递减：frameTs 单调 → 重锚后墙钟单调（帧选择位置单调，TTS3-A4）
    // ─────────────────────────────────────────────

    @Test
    fun reanchor_monotonicity_nonDecreasingAcrossPlay() {
        // 一次完整语音的 150ms 步进帧序列（含首帧 0 与越前的负帧）→ 重锚墙钟严格非递减
        val playStart = 42_000L
        val frameTs = (longArrayOf(-600L, -150L, 0L) + (1..10).map { it * 150L }.toLongArray())
        var prev = Long.MIN_VALUE
        for (ts in frameTs) {
            val r = LipSyncClock.reanchor(ts, playStart)
            assertTrue(
                "重锚墙钟应随 frameTs 非递减（TTS3-A4），frameTs=$ts r=$r 应 ≥ 前一 $prev",
                r >= prev
            )
            prev = r
        }
    }

    @Test
    fun reanchor_monotonicity_largePlayStart_needsPlusFormula() {
        // 大 playStart + 大正 frameTs：结果必须是 playStart + ts（不是减）——锁定「+」裁定方向。
        // 若现状沿用 AND1-A3 的「负值 clamp 0」反方向实现，此处会 FAIL（RED 根）。
        val playStart = 9_000_000_000L // ~2.5h 墙钟 9e9 ms 量级
        val r = LipSyncClock.reanchor(1_000_000L, playStart)
        assertEquals(
            "重锚 = playStart + frameTs（用户裁定 + 方向；clamp 下限 playStart）",
            playStart + 1_000_000L, r
        )
        assertTrue("重锚后墙钟应 ≥ playStart，实际 $r", r >= playStart)
    }

    // ─────────────────────────────────────────────
    // 恒定 playStart → 重锚对 ts=0 恒为 playStart（跨多轮稳定性）
    // ─────────────────────────────────────────────

    @Test
    fun reanchor_stableAcrossRepeatedCalls() {
        // 纯函数：同参双跑逐等
        val playStart = 777L
        val a = LipSyncClock.reanchor(150L, playStart)
        val b = LipSyncClock.reanchor(150L, playStart)
        assertEquals("同参双跑应一致（纯函数）", a, b)
    }
}
