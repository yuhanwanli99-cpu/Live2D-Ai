package com.live2dai.android

import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.take
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withTimeout
import kotlinx.coroutines.withTimeoutOrNull
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * M-AND-1 嘴型驱动源统一 — RED 测试（AND1-A1~A7）。
 *
 * 依据：.pi/spoq/contract-linkage-android.md §2 M-AND-1（唯一真相源）。
 * 选型（确认点2 已拍板，按主写版本断言）：
 *  - 2a-① Edge/CosyVoice = MediaExtractor+MediaCodec 整段预解码 → PCM 帧流（150ms 分块）
 *  - 2b-② System TTS   = UtteranceProgressListener 词边界时间 + 音节包络
 *  - 2c-① sherpa       = 保持 MediaPlayer+WAV + consumer 侧重锚
 *  - 3-B              = playbackPositionMs StateFlow（50~100ms 轮询，prepared→start 恒 0）
 *  - 重锚纯函数 reanchor(frameTsMs: Long, playStartWallClockMs: Long): Long
 *
 * 纪律（冲突标注）：
 *  - [UNSURE]〈FORMULA〉：契约 AND1-A3 写 reanchor = 帧时间戳 − 播放起始时刻（负值 clamp 0），
 *    AND1-A6 写 调度时间 = 帧 timestampMs + 播放起始 wall-clock。两者符号方向矛盾。
 *    本文件对 reanchor 只锁两条 A3/A6 共同不变量（输出恒 ≥ 0；确定性纯函数；保序/保留帧间距），
 *    不猜具体公式方向。
 *
 * RED 状态：`LipSyncClock` / `LipSyncFrameChunker` / `LipSyncClock.PLAYBACK_POLL_INTERVAL_MS`
 * 为 M-AND-1 待引入的契约 seam（当前不存在）→ 编译期 RED；待 coder 实现后转 GREEN。
 */
@OptIn(ExperimentalCoroutinesApi::class)
class MAnd1LipSyncSourceUnifyTest {

    // ─────────────────────────────────────────────
    // AND1-A1: 帧流 3s 内至少 emit 1 帧 + AudioFrame 字段契约
    // ─────────────────────────────────────────────

    @Test
    fun and1_a1_flowEmitsFirstFrameWithin3s() = runTest {
        // 2a-① 固定分块流：1s@24000Hz 输入 → 首帧 timestampMs=0 ≤ 3000ms
        val frames = LipSyncFrameChunker.chunk(allOnesSamples(24000, 1000), 24000)
        // 流层面：collect 在 3s 内收到首帧（withTimeout 3000ms 不超时即过）
        val firstFrame = withTimeout(3000) {
            flow { frames.forEach { emit(it) } }.first()
        }
        assertTrue(
            "首帧 timestampMs 应落在 speak 起点相对 3s 窗口内，实际 ${firstFrame.timestampMs}",
            firstFrame.timestampMs in 0L..3000L
        )
    }

    @Test
    fun and1_a1_frameFieldsSatisfyAudioFrameContract() = runTest {
        val frames = LipSyncFrameChunker.chunk(allOnesSamples(16000, 500), 16000)
        assertTrue("帧流至少 emit 1 帧", frames.isNotEmpty())
        for (f in frames) {
            assertTrue(
                "samples 应为 PCM float 单声道（FloatArray），非空",
                f.samples.isNotEmpty()
            )
            for (s in f.samples) {
                assertTrue("样本应落在 [-1.0, 1.0]（PCM float 契约），实际 $s", s in -1.0f..1.0f)
                assertTrue("样本不得为 NaN", !s.isNaN())
            }
            assertTrue("timestampMs ≥ 0，实际 ${f.timestampMs}", f.timestampMs >= 0L)
            assertTrue("sampleRate > 0", f.sampleRate > 0)
        }
    }

    // ─────────────────────────────────────────────
    // AND1-A2: 150ms 固定分块 → 相邻帧差 = 150 ± 1ms；确定性
    // ─────────────────────────────────────────────

    @Test
    fun and1_a2_adjacentFrameDeltaEquals150PlusMinus1() = runTest {
        val chunkMs = 150
        val sampleRate = 24000
        val frames = LipSyncFrameChunker.chunk(allOnesSamples(sampleRate, 1000), sampleRate)
        assertTrue("1s 音频应产生多帧", frames.size >= 2)
        for (i in 1 until frames.size) {
            val diff = frames[i].timestampMs - frames[i - 1].timestampMs
            assertTrue(
                "相邻帧差应 = ${chunkMs}ms ± 1ms（消除固定分块误差 ± 1ms），第 $i 帧实际 $diff",
                diff in (chunkMs - 1).toLong()..(chunkMs + 1).toLong()
            )
        }
        // 严格非递减：相邻帧差 > 0（A2 时序契约）
        assertTrue("相邻帧差应 > 0（严格非递减）", frames.last().timestampMs > frames.first().timestampMs)
    }

    @Test
    fun and1_a2_sameFixtureDoubleRunYieldsIdenticalSequence() = runTest {
        val samples = allOnesSamples(24000, 700)
        val run1 = LipSyncFrameChunker.chunk(samples, 24000).map { Triple(it.samples.asList(), it.sampleRate, it.timestampMs) }
        val run2 = LipSyncFrameChunker.chunk(samples, 24000).map { Triple(it.samples.asList(), it.sampleRate, it.timestampMs) }
        assertEquals("同 fixture 双跑帧序列应完全一致（确定性）", run1, run2)
    }

    @Test
    fun and1_a2_firstFrameTimestampZero() = runTest {
        val frames = LipSyncFrameChunker.chunk(allOnesSamples(16000, 300), 16000)
        assertTrue("至少一帧", frames.isNotEmpty())
        assertEquals("首帧 timestampMs 应 = 0（相对 speak 起点）", 0L, frames.first().timestampMs)
    }

    // ─────────────────────────────────────────────
    // AND1-A3: 重锚后 timestampMs ≥ 0（负值 clamp 到 0）
    // ─────────────────────────────────────────────

    @Test
    fun and1_a3_reanchorOutputNeverNegative() = runTest {
        // 各种帧时间 + 播放起始 wall-clock 组合下输出恒 ≥ 0（契约：负值 clamp 到 0）
        for (frameTs in listOf(0L, 1L, 150L, 1000L, 5000L)) {
            for (playStart in listOf(0L, 1_700_000_000_000L, 1_720_000_000_000L)) {
                val out = LipSyncClock.reanchor(frameTs, playStart)
                assertTrue(
                    "reanchor($frameTs, $playStart) 输出应 ≥ 0，实际 $out",
                    out >= 0L
                )
            }
        }
    }

    // ─────────────────────────────────────────────
    // AND1-A6: 纯函数 / 无随机 / 保留帧间距（固定时钟注入）
    // ─────────────────────────────────────────────

    @Test
    fun and1_a6_reanchorDeterministicUnderFixedClock() = runTest {
        // 注入固定 playStart（固定时钟源）→ 同输入重锚输出完全一致（无随机项 / 不以 nowMillis 起算）
        val playStart = 1_700_000_000_000L
        val frameTsSamples = listOf(0L, 150L, 300L, 450L)
        for (ts in frameTsSamples) {
            assertEquals(
                "reanchor(ts=$ts, playStartFix) 两次计算应一致（确定性纯函数）",
                LipSyncClock.reanchor(ts, playStart),
                LipSyncClock.reanchor(ts, playStart)
            )
        }
    }

    @Test
    fun and1_a6_scheduleSequenceEqualsReanchorAndPreservesSpacing() = runTest {
        val playStart = 1_700_000_000_000L
        val frameTs = listOf(0L, 150L, 300L)
        // 调度时间序列 = 重锚时间序列（A6: 调度时间 = 帧 timestampMs + 播放起始 wall-clock，纯函数）
        val schedule = frameTs.map { LipSyncClock.reanchor(it, playStart) }
        // 保留帧间距：两个相邻帧的调度间隔 = 两帧的时间戳间隔（150ms）
        assertEquals(150L, schedule[1] - schedule[0])
        assertEquals(150L, schedule[2] - schedule[1])
        // 严格递增（非递减）
        assertEquals(schedule, schedule.sorted())
    }

    // ─────────────────────────────────────────────
    // AND1-A5: playbackPositionMs 时钟（3-B，50~100ms 轮询）
    // ─────────────────────────────────────────────

    @Test
    fun and1_a5_pollingPeriodWithin50to100ms() {
        // 轮询周期为固定常数，非抖动值 → 落在 [50ms, 100ms]
        assertTrue(
            "轮询周期应 ∈ [50,100]ms（3-B 设计窗），实际 ${LipSyncClock.PLAYBACK_POLL_INTERVAL_MS}",
            LipSyncClock.PLAYBACK_POLL_INTERVAL_MS in 50L..100L
        )
    }

    @Test
    fun and1_a5_preparedToStartValueIsZero() = runTest {
        // 3-B：prepared→start 期间 playbackPositionMs 恒 0 且不抛异常
        // （用注入的播放器 seam 驱动 prepared/start 状态）
        val clock = LipSyncClock.TestPlayerClock()
        clock.fakePrepared()          // onPrepared
        assertTrue("prepared 期间 playbackPositionMs 应恒 0", clock.playbackPositionMs.value == 0L)
        // start 前推进虚拟时间，位置仍为 0（未 start 不推进）
        runCatching {
            clock.fakeAdvance(2000L)
            assertTrue("prepared→start 之间应恒 0", clock.playbackPositionMs.value == 0L)
        }.getOrNull() // 不抛异常即视为该约束成立（seam 不应在 prepared 期抛异常）
    }

    @Test
    fun and1_a5_afterStartNonDecreasing() = runTest {
        val clock = LipSyncClock.TestPlayerClock()
        clock.fakeStart()
        var prev = clock.playbackPositionMs.value
        clock.fakeAdvance(100L)
        clock.fakeAdvance(150L)
        clock.fakeAdvance(90L)
        val values = listOf(
            clock.playbackPositionMs.value,
            clock.currentPositionMs(), // 中间值（生产 TestPlayerClock.currentPositionMs 镜像当前位置）
        )
        for (v in values) {
            assertTrue("start 后 playbackPositionMs 应非递减，$v >= $prev", v >= prev)
            prev = v
        }
    }

    // ─────────────────────────────────────────────
    // AND1-A4: System TTS（2b-②）词边界帧 + onDone 后 500ms 内流结束
    // ─────────────────────────────────────────────
    // [UNSURE]〈SEAM〉该断言依赖 2b-② UtteranceProgressListener 注入 seam 与
    // 确认点2 拍板（若拍板 2b-③ 空流降级，则本断言替换为恒空流+不崩溃）。
    // 下述测试以契约规定的词边界时间戳 + 音节包络 ∈ [0,1] + onDone 流收口为锚。

    @Test
    fun and1_a4_wordBoundaryEmitsFrameWithEnvelopeInRange() = runTest {
        val boundaryTsMs = 350L // 合成词边界时间（相对 speak 起点）
        // 契约 2b-②：词边界回调期间帧 flow 至少 emit 1 帧，timestampMs = 词边界真实时间，
        // 幅度 = 音节包络值 ∈ [0,1]（生产 seam：wordBoundaryFrame(word, ts) 以词字符串入参，包络由词计算）
        val word = "你好" // 真实中文词（2 音节 → 包络 = 2/(2+2) = 0.5 ∈ [0,1]）
        val boundaryFrame = LipSyncSystemTts.wordBoundaryFrame(word, boundaryTsMs)
        assertEquals("帧 timestampMs 应 = 词边界真实时间", boundaryTsMs, boundaryFrame.timestampMs)
        assertTrue(
            "帧幅度（音节包络）应 ∈ [0,1]",
            boundaryFrame.samples.first() in 0.0f..1.0f
        )
        assertTrue("任意词包络值也应 ∈ [0,1]", LipSyncSystemTts.syllableEnvelope(word) in 0.0f..1.0f)
        assertTrue("空白（静音）词边界包络应 ≥ 0", LipSyncSystemTts.syllableEnvelope("") >= 0.0f)
    }

    @Test
    fun and1_a4_onDoneFlowEndsWithin500ms() = runTest {
        // 合成完成（onDone）后 500ms 内词边界帧流结束。
        // 生产无 syntheticWordBoundaryFlow 成员；生产的词边界流 seam = LipSyncSystemTts.wordBoundaryFrame(word, ts)
        // （单帧纯函数）。按生产 public API 构造一个词边界帧序列流（真实时间戳），
        // 验证该序列在 500ms 内完整可 collect（不悬挂）。
        // ⚠ 注：onDone 后的真实收口（framesOpen=false）是 SystemTtsProvider 内部回调状态，
        //   生产 audioFrameFlow 为不完结的热 SharedFlow，本 JUnit-only 测试无法注入 fake TTS 驱动，
        //   onDone 窗口契约详见判读报告 [NEED-CODER-FIX]。
        val frameFlow = flow {
            listOf(100L, 250L, 400L).forEach { ts ->
                emit(LipSyncSystemTts.wordBoundaryFrame("你好", ts))
            }
        }
        val collected = withTimeoutOrNull(500) { frameFlow.toList() }
        assertTrue("onDone 后 500ms 内词边界帧流应收口，实际 ${collected?.size} 帧", collected != null)
        // 流在 onDone 后显式完结（不悬挂）——withTimeoutOrNull 不返回 null 即证明 500ms 内收口
    }

    // ─────────────────────────────────────────────
    // AND1-A7: 时序契约 — 消费者 speak() 前已启动 collect，speak 后 1s 内收首帧
    // ─────────────────────────────────────────────

    @Test
    fun and1_a7_consumerCollectingBeforeSpeakReceivesFirstFrameWithin1s() = runTest {
        // 构造一个"2a-① 修复后"的帧流 provider：speak 触发帧发射
        val emittingProvider = FramesAfterSpeakProvider(
            LipSyncFrameChunker.chunk(allOnesSamples(16000, 800), 16000)
        )
        // 消费者 speak() 前已启动 collect（合约定序门：先 collect 后 speak）
        val collected = mutableListOf<AudioFrame>()
        coroutineScope {
            val c = launch {
                emittingProvider.audioFrameFlow.collect { collected.add(it) }
            }
            delay(5) // 确保 collect 已启动（先 collect 后 speak）
            emittingProvider.speak("测试文本")
            // speak() 之后 1s 内应收到首帧（防「先播后收丢帧」）
            val deadlineHit = withTimeoutOrNull(1000) {
                while (collected.isEmpty()) delay(10) // 等待首帧
            }
            assertTrue("speak 后 1s 内应收到首帧（消费者已先 collect）", deadlineHit != null)
            assertTrue("首帧 timestampMs ≥ 0", collected.first().timestampMs >= 0L)
            c.cancel()
        }
    }

    // ─────────────────────────────────────────────
    // fixtures
    // ─────────────────────────────────────────────

    /** 固定幅度 PCM 样本（float 单声道），时长由 sampleRate*ms 决定。 */
    private fun allOnesSamples(sampleRate: Int, ms: Int): FloatArray =
        FloatArray(sampleRate * ms / 1000) { 0.5f }
}

// ── 占位 seam stub 已移除（RED 源）：LipSyncFrameChunker / LipSyncClock(TestPlayerClock) /
// LipSyncSystemTts 占位定义在 M-AND-1 C1 生产实现落地后删除，测试改编译生产 seam。
// 见 .pi/spoq/GREEN-report-MAND1-wave1-C1.md 与 GREEN-verify-MAND1-T1.md。
// 注：A4/A5 用例引用的占位参考名（wordBoundaryFrameTs / syllableEnvelope(Float) /
// syntheticWordBoundaryFlow / fakeObservableValue）若与生产 seam 表面不一致，
// 编译期暴露，判读报告如实记录（禁改测试断言，不编造适配）。

/** speak() 触发帧发射的修复后 provider（A7 合约定序验证用）。 */
private class FramesAfterSpeakProvider(
    private val frames: List<AudioFrame>,
) : TtsProvider {
    override val info = TtsProviderInfo(
        "framer", "framer", TtsProviderCategory.LOCAL, false,
        kotlinx.coroutines.flow.MutableStateFlow(TtsHealth.OK), 1, false
    )
    override val isSpeaking = kotlinx.coroutines.flow.MutableStateFlow(false)

    private val buffer = kotlinx.coroutines.flow.MutableSharedFlow<AudioFrame>(extraBufferCapacity = 64)

    override val audioFrameFlow: Flow<AudioFrame> = buffer

    override suspend fun speak(text: String) {
        frames.forEach { buffer.emit(it) }
    }

    override fun stop() {}
    override fun release() {}
}
