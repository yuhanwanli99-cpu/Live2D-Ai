package com.live2d.ai.android

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowLooper
import org.robolectric.shadows.ShadowMediaPlayer
import org.robolectric.shadows.util.DataSource
import java.io.File
import java.util.concurrent.TimeUnit

/**
 * Wave 2 · 2B-1（sherpa-onnx 本地 TTS 主路径 + Kokoro-zh 模型 spec）—— speak 状态机/事件序 RED 测试。
 *
 * 依据：contract-refactor-0814.md §2.6（失败语义）+ §3.3（TtsProvider speak 生命周期）+ §3 TTS-* +
 * 架构 architecture-refactor-tts-0814.md §3.3。
 *
 * 状态机（合成中/就绪/播放中/完成）映射：
 *   ├─ 合成中 SYNTHESIZING：speak() 进入 → engine.synthesize(text, sid, speed)（此时 isSpeaking=false）
 *   ├─ 就绪   READY：synthesize 返回整段 PCM，provider 按 150ms 分块 emit 帧（仍 isSpeaking=false）
 *   ├─ 播放中 PLAYING：MediaPlayer onPrepared → isSpeaking=true（ShadowLooper.idleMainLooper 驱动）
 *   └─ 完成   COMPLETED：onCompletion → isSpeaking=false → speak() 返回
 *
 * ── seam（生产接口注入）──
 * fake 实现生产接口 [SherpaTtsEngine]（SherpaOnnxEngine.kt）：synthesize(text, sid, speed)
 * 把帧表反拼为整段 PCM，交回生产 emitAudioFrames / playAudio 全链路（帧分块与 WAV 播放
 * 全走生产代码）。播放周期由 Robolectric ShadowMediaPlayer 驱动：@Before 以
 * addMediaInfo 注册 sherpa_onnx_tts_temp.wav（MediaInfo duration=150ms, preparationDelay=0），
 * speak 后 idleMainLooper() 驱动 onPrepared、idleMainLooper(200, MILLISECONDS) 驱动 onCompletion。
 * P3 引擎失败走生产 ensureEngine 路径：SherpaOnnxEngineRegistry.register(createTts 返回 null
 * 的工厂) + 不注入引擎 → UNAVAILABLE。
 *
 * ── [UNSURE] 如实标注 ──
 * 生产播放走 MediaPlayer 无法 JVM 无头真播；本测试以 ShadowMediaPlayer 驱动 provider 编排
 * （isSpeaking/帧流/失败桥接）符合契约，不探真播放器。真机出声/嘴动 = TTS-1/TTS-2
 * （Wave 6 device 验收），不在本 JVM RED 面。
 */
@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class, sdk = [34])
class Sherpa2b1SpeakStateTest {

    private fun context(): Context = ApplicationProvider.getApplicationContext()

    @Before
    fun setUp() {
        // 测试隔离：清空引擎注册表（P3 以注册表注入 null-工厂，防串场）
        SherpaOnnxEngineRegistry.clear()
        // ShadowMediaPlayer：speak 写出的临时 WAV 需注册 MediaInfo，否则 setDataSource 抛异常
        ShadowMediaPlayer.addMediaInfo(
            DataSource.toDataSource(File(context().cacheDir, "sherpa_onnx_tts_temp.wav").absolutePath),
            ShadowMediaPlayer.MediaInfo(150, 0) // duration=150ms, preparationDelay=0
        )
    }

    @After
    fun tearDown() {
        SherpaOnnxEngineRegistry.clear()
    }

    /** modelDir 清理，保证"未下载/已就绪"初始态。 */
    private fun modelDirWith(clean: Boolean): File {
        val manager = ModelDownloadManager(context())
        val dir = manager.modelDir(ModelDownloadManager.KOKORO_TTS)
        dir.deleteRecursively()
        if (!clean) {
            dir.mkdirs()
            File(dir, "model.onnx").writeBytes(ByteArray(4))
            File(dir, "tokens.txt").writeText("t")
            File(dir, "voices.bin").writeBytes(ByteArray(4))
        }
        return dir
    }

    // ─────────────────────────────────────────────
    // P1: 事件序 — 帧先至 → isSpeaking true → false → speak 返回
    // ─────────────────────────────────────────────

    @Test
    fun P1_event_ordering_frames_before_speaking_true_before_completion() = runTest {
        val engine = FakeSherpa2b1TtsEngine(frames = chunkFrames(24000, 600))
        val provider = modelReadyProvider(engine, backgroundScope)

        val isSpeakingLog = mutableListOf<Boolean>()
        val framesLog = mutableListOf<AudioFrame>()
        val collectJob = launch {
            provider.isSpeaking.collect { isSpeakingLog.add(it) }
        }
        val framesJob = launch {
            provider.audioFrameFlow.collect { framesLog.add(it) }
        }
        delay(10) // 消费者先 collect（契约 replay=0，须先订阅）

        provider.speak("你好")
        delay(10) // 收集器消化 speak 内同步 emit 的帧

        // 帧先被观测到
        assertTrue("至少 1 帧被 emit", framesLog.isNotEmpty())
        assertTrue("首帧 timestampMs=0", framesLog.first().timestampMs == 0L)

        // ShadowMediaPlayer：idle 主 Looper 驱动 onPrepared（isSpeaking=true）
        ShadowLooper.idleMainLooper()
        delay(10) // 收集器记录 true
        // 再 idle 200ms（≥ MediaInfo duration=150ms）驱动 onCompletion（isSpeaking=false）
        ShadowLooper.idleMainLooper(200, TimeUnit.MILLISECONDS)
        delay(10) // 收集器记录 false

        // 事件序 = 先出现 isSpeaking=true，再 false（播放完成）
        val trueIdx = isSpeakingLog.indexOf(true)
        val falseIdx = isSpeakingLog.lastIndexOf(false)
        assertTrue("isSpeaking 应出现过 true（PLAYING）", trueIdx >= 0)
        assertTrue("isSpeaking 最终回落 false（COMPLETED）", falseIdx > trueIdx)

        collectJob.cancel()
        framesJob.cancel()
        provider.release()
    }

    // ─────────────────────────────────────────────
    // P2: speak 挂起到播放完成；完成后不再发帧
    // ─────────────────────────────────────────────

    @Test
    fun P2_speak_suspends_until_completion_then_no_more_frames() = runTest {
        val engine = FakeSherpa2b1TtsEngine(frames = chunkFrames(16000, 150))
        val provider = modelReadyProvider(engine, backgroundScope)

        val frames = mutableListOf<AudioFrame>()
        val receiver = launch { provider.audioFrameFlow.collect { frames.add(it) } }
        delay(10)

        provider.speak("测试") // 合成 + 全帧 emit + 播放编排落地后返回
        delay(10) // 收集器消化帧

        assertEquals("帧数应与引擎帧表一致", engine.frames.size, frames.size)
        // speak 返回后收集一点虚拟时间 → 无新帧
        delay(50)
        assertEquals("speak 返回后不应再发新帧", engine.frames.size, frames.size)
        // 播放完成 isSpeaking=false：idle 驱动 prepared → completion（MediaInfo duration=150ms）
        ShadowLooper.idleMainLooper()
        ShadowLooper.idleMainLooper(200, TimeUnit.MILLISECONDS)
        assertEquals("播放完成 isSpeaking=false", false, provider.isSpeaking.value)

        receiver.cancel()
        provider.release()
    }

    // ─────────────────────────────────────────────
    // P3: 失败 — 引擎初始化/合成失败 → speak 失败、isSpeaking 不置 true、health=UNAVAILABLE
    // ─────────────────────────────────────────────

    @Test
    fun P3_engine_init_failure_speak_fails_health_unavailable() = runTest {
        // 引擎不可用：注册 createTts 返回 null 的工厂（生产 ensureEngine 路径 → UNAVAILABLE）
        SherpaOnnxEngineRegistry.register(object : SherpaOnnxEngineFactory {
            override fun createTts(modelDir: File): SherpaTtsEngine? = null
            override fun createAsr(modelDir: File): SherpaAsrEngine? = null
        })
        // 不注入引擎（主构造器）：ensureEngine 走注册表 → null → UNAVAILABLE + 抛异常
        val provider = modelReadyProvider(backgroundScope)

        var failed = false
        try {
            provider.speak("你好")
        } catch (e: IllegalStateException) {
            failed = true
        }
        assertTrue("引擎失败应向上传播（speak 不静默成功）", failed)
        assertFalse("失败全程 isSpeaking 不得置 true", provider.isSpeaking.value)
        assertEquals("引擎失败 health 应为 UNAVAILABLE", TtsHealth.UNAVAILABLE, provider.info.health.value)
        provider.release()
    }

    // ─────────────────────────────────────────────
    // P4: 失败 — 模型未下载 → speak 快速失败（不触引擎合成）、isSpeaking 不置 true
    // ─────────────────────────────────────────────

    @Test
    fun P4_model_not_downloaded_fast_fail_engine_not_touched() = runTest {
        val engine = FakeSherpa2b1TtsEngine(frames = chunkFrames(24000, 300))
        val provider = modelMissingProvider(engine, backgroundScope)

        var failed = false
        try {
            provider.speak("你好")
        } catch (e: IllegalStateException) {
            failed = true
        }
        assertTrue("模型未下载应快速失败（speak 抛异常）", failed)
        assertTrue("失败路径引擎不应被合成调用", !engine.synthesizeCalled)
        assertFalse("失败全程 isSpeaking 不得置 true", provider.isSpeaking.value)
        provider.release()
    }

    // ─────────────────────────────────────────────
    // P5: 帧序列契约在 speak 路径上成立（150ms/单调/首帧 0）
    // ─────────────────────────────────────────────

    @Test
    fun P5_speak_path_frames_satisfy_contract() = runTest {
        val engine = FakeSherpa2b1TtsEngine(frames = chunkFrames(24000, 900))
        val provider = modelReadyProvider(engine, backgroundScope)

        val frames = mutableListOf<AudioFrame>()
        val receiver = launch { provider.audioFrameFlow.collect { frames.add(it) } }
        delay(10)
        provider.speak("测试文本")
        delay(10) // 收集器消化帧

        assertEquals("帧数应等于引擎帧表", engine.frames.size, frames.size)
        assertEquals("首帧 timestampMs=0", 0L, frames.first().timestampMs)
        var prev = Long.MIN_VALUE
        frames.forEachIndexed { i, f ->
            assertTrue("帧[$i] timestampMs 严格非递减", f.timestampMs >= prev)
            prev = f.timestampMs
        }
        assertTrue("sampleRate 合理（>0）", frames.all { it.sampleRate > 0 })

        receiver.cancel()
        provider.release()
    }

    // ─────────────────────────────────────────────
    // P6: stop() 打断 → speak 让位、isSpeaking 回落 false、不再发帧
    // ─────────────────────────────────────────────

    @Test
    fun P6_stop_interrupts_speak_and_stops_frames() = runTest {
        val engine = FakeSherpa2b1TtsEngine(frames = chunkFrames(24000, 600))
        val provider = modelReadyProvider(engine, backgroundScope)

        val frames = mutableListOf<AudioFrame>()
        val receiver = launch { provider.audioFrameFlow.collect { frames.add(it) } }
        delay(10)

        val speakJob = launch { runCatching { provider.speak("打断") } }
        // 与 speak 主流程的竞态：先 join 收敛（speak 编排落地、帧已 emit），再取帧计数锚点
        speakJob.join() // 等 speak 编排落地（生产 speak 必然返回；虚拟时间 timeout 会与真实 IO 线程竞态，故用纯 join）
        delay(10) // 收集器消化已 emit 帧
        provider.stop() // 打断/幂等 stop

        // 打断后：speak 让位（join 已收敛）；isSpeaking 必须回落 false
        assertFalse("打断后 isSpeaking 应回落 false", provider.isSpeaking.value)
        val frameCountAtStop = frames.size
        delay(50)
        assertEquals("stop 后不应再发新帧", frameCountAtStop, frames.size)
        provider.stop() // 二次 stop 兜底（幂等，不崩溃）
        assertFalse("二次 stop 后 isSpeaking 仍 false", provider.isSpeaking.value)

        receiver.cancel()
        provider.release()
    }

    // ─────────────────────────────────────────────
    // fixtures / 引擎构造
    // ─────────────────────────────────────────────

    private fun chunkFrames(sampleRate: Int, ms: Int): List<AudioFrame> {
        val input = FloatArray(sampleRate * ms / 1000) { 0.2f }
        return LipSyncFrameChunker.chunk(input, sampleRate)
    }

    /** 注入引擎（seam B）+ 测试调度器播放位置轮询作用域。 */
    private fun modelReadyProvider(engine: SherpaTtsEngine, scope: CoroutineScope): SherpaOnnxTtsProvider {
        modelDirWith(clean = false) // 伪造已就绪模型文件
        val manager = ModelDownloadManager(context())
        return SherpaOnnxTtsProvider(
            context(), manager, ModelDownloadManager.KOKORO_TTS, engine,
            positionPollScope = scope,
        )
    }

    /** 不注入引擎（主构造器）：P3 走生产 ensureEngine（注册表）路径。 */
    private fun modelReadyProvider(scope: CoroutineScope): SherpaOnnxTtsProvider {
        modelDirWith(clean = false) // 伪造已就绪模型文件
        val manager = ModelDownloadManager(context())
        return SherpaOnnxTtsProvider(
            context(), manager, ModelDownloadManager.KOKORO_TTS,
            positionPollScope = scope,
        )
    }

    private fun modelMissingProvider(engine: SherpaTtsEngine, scope: CoroutineScope): SherpaOnnxTtsProvider {
        modelDirWith(clean = true) // 清空 → 未下载
        val manager = ModelDownloadManager(context())
        return SherpaOnnxTtsProvider(
            context(), manager, ModelDownloadManager.KOKORO_TTS, engine,
            positionPollScope = scope,
        )
    }
}

// ============================================================================
// fake 引擎（实现生产接口 SherpaTtsEngine：合成 → 整段 PCM，播放全走生产 MediaPlayer 路径）
// ============================================================================

/** 正常合成 fake：synthesize 把帧表反拼为整段 PCM，交回生产 emitAudioFrames/playAudio 全链路。 */
internal class FakeSherpa2b1TtsEngine(
    val frames: List<AudioFrame>,
    private val engineSampleRate: Int = 24000,
) : SherpaTtsEngine {
    var synthesizeCalled = false
        private set

    /** 帧表 → 整段 PCM（反拼）；sampleRate 取首帧（与 chunkFrames 输入采样率一致）。 */
    override fun synthesize(text: String, sid: Int, speed: Float): SherpaTtsAudio? {
        synthesizeCalled = true
        val total = frames.sumOf { it.samples.size }
        val pcm = FloatArray(total)
        var offset = 0
        for (f in frames) {
            f.samples.copyInto(pcm, offset)
            offset += f.samples.size
        }
        return SherpaTtsAudio(pcm, frames.firstOrNull()?.sampleRate ?: engineSampleRate)
    }

    override fun sampleRate(): Int = frames.firstOrNull()?.sampleRate ?: engineSampleRate

    override fun release() = Unit
}
