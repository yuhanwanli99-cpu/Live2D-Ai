package com.live2dai.android

import android.content.Context
import android.media.AudioAttributes
import android.media.MediaPlayer
import android.util.Log
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.io.FileOutputStream
import java.io.RandomAccessFile
import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * sherpa-onnx + Kokoro 离线 TTS Provider（V1.2 模块3）。
 *
 * 契约见 docs/plans/plan-v12-plan.schema.json → providersV12.sherpa_onnx_kokoro：
 * - id="sherpa_onnx_kokoro" / LOCAL / requiresNetwork=false / priority=1（CosyVoice 之后，
 *   EdgeTTS 之前——同 priority 稳定排序由注册顺序保证）/ configurable=false
 * - 模型未下载 → health=UNAVAILABLE，[speak] 快速失败（不发任何网络请求）→ 降级链继续
 * - speak：sherpa-onnx Kokoro 合成 → PCM → WAV → MediaPlayer 播放（plan §3.3.4）；
 *   [isSpeaking] StateFlow 供 LipSync/UI 观察
 *
 * 引擎加载走 [SherpaOnnxEngineRegistry]（条件源码集）：AAR 未集成 / JVM 环境无法加载
 * 原生库时引擎为 null → health=UNAVAILABLE（plan §8.3 回退策略）。
 *
 * 日志规范（plan §5.7.4）：关键节点 Log.i（国产 ROM 抑制 Log.d 教训），TAG="SherpaOnnx"。
 */
class SherpaOnnxTtsProvider(
    private val context: Context,
    private val downloadManager: ModelDownloadManager = ModelDownloadManager(context),
    private val modelSpec: ModelSpec = ModelDownloadManager.KOKORO_TTS,
    /** Kokoro 音色序号（voices.bin 内，0 = 第一个音色） */
    private val sid: Int = 0,
    /** 语速（1.0 = 正常） */
    private val speed: Float = 1.0f,
    /** M-AND-1 测试注入：播放位置轮询作用域（默认 Main；测试注入 TestDispatcher 驱动虚拟时间）。 */
    private val positionPollScope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate),
) : TtsProvider {

    /**
     * 测试/注入 seam A：第 4 参直接注入引擎实例，其余参数与主构造器对齐
     * （sid 必填以区分 seam B；speed/scope 可默认）。
     *
     * 用于 JVM 测试注入假引擎验证 speak 状态机（合成中→就绪→播放中→完成事件序）与
     * 错误路径（模型未下载/初始化失败 → 明确失败语义）；模型就绪判定仍走 [downloadManager]。
     */
    constructor(
        context: Context,
        downloadManager: ModelDownloadManager,
        modelSpec: ModelSpec,
        engine: SherpaTtsEngine,
        sid: Int,
        speed: Float = 1.0f,
        positionPollScope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate),
    ) : this(context, downloadManager, modelSpec, sid = sid, speed = speed, positionPollScope = positionPollScope) {
        this.engine = engine
    }

    /**
     * 测试/注入 seam B：第 4 参直接注入引擎实例（省略 sid；默认 0）。
     * 与 seam A 区分：4 参/5 参（第 5 参为 Float）调用解析到本构造器。
     */
    constructor(
        context: Context,
        downloadManager: ModelDownloadManager,
        modelSpec: ModelSpec,
        engine: SherpaTtsEngine,
        speed: Float = 1.0f,
        positionPollScope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate),
    ) : this(context, downloadManager, modelSpec, sid = 0, speed = speed, positionPollScope = positionPollScope) {
        this.engine = engine
    }

    companion object {
        private const val TAG = "SherpaOnnx"

        /** Provider 唯一标识（契约 schema providersV12.sherpa_onnx_kokoro） */
        const val PROVIDER_ID = "sherpa_onnx_kokoro"

        /** M4：音频分块窗口（ms），与 M5 VisemeAnalyzer 默认分析窗口对齐（contract-M4-M5 §2.3.1）。 */
        private const val AUDIO_FRAME_MS = 150

        /**
         * M4：SharedFlow 缓冲容量估算（按块数级；典型语句的 150ms 块数量级）。
         * 超出部分按契约丢块 + Log.w 一次，不阻塞 speak（见 emitAudioFrames）。
         */
        private const val AUDIO_FLOW_BUFFER_CAPACITY = 64
    }

    private val _health = MutableStateFlow(
        if (downloadManager.isDownloaded(modelSpec)) TtsHealth.OK else TtsHealth.UNAVAILABLE
    )

    override val info = TtsProviderInfo(
        id = PROVIDER_ID,
        name = "Kokoro 离线",
        category = TtsProviderCategory.LOCAL,
        requiresNetwork = false, // 离线可用（plan §3.3.4）
        health = _health,
        // Android 降级链 Tier 2（plan §3.1.6）：CosyVoice(0) 之后的离线兜底；
        // 注册在 EdgeTts 之前（同 priority=1 稳定排序 → sherpa 先于 edge）
        priority = 1,
        configurable = false,
    )

    private val _isSpeaking = MutableStateFlow(false)
    override val isSpeaking: StateFlow<Boolean> = _isSpeaking.asStateFlow()

    /**
     * M4：音频帧流（可选能力）。
     *
     * synthesize 成功后按 150ms 窗口分块 emit 全部 PCM 帧（contract-M4-M5 §2.2/§2.3）：
     * - replay=0：消费者必须在 speak() 之前启动 collect，否则丢帧（时序契约 §2.3.6）
     * - tryEmit 非阻塞：无订阅者 / 缓冲满 → 丢块 + Log.w 一次，不影响 speak 主流程
     */
    private val _audioFrameFlow = MutableSharedFlow<AudioFrame>(
        replay = 0,
        extraBufferCapacity = AUDIO_FLOW_BUFFER_CAPACITY,
    )
    override val audioFrameFlow: Flow<AudioFrame> = _audioFrameFlow.asSharedFlow()

    /** M-AND-1（3-B）：播放位置时钟（MediaPlayer.getCurrentPosition 轮询重锚）。 */
    private val playbackClock = LipSyncClock.PlaybackClock(
        positionSource = LipSyncClock.PlayerPositionSource {
            try {
                mediaPlayer?.currentPosition?.toLong() ?: 0L
            } catch (_: Throwable) {
                0L
            }
        },
        scope = positionPollScope,
    )
    override val playbackPositionMs: StateFlow<Long> = playbackClock.playbackPositionMs

    /** M4：本次 speak 会话内是否已被 stop() 打断（打断后不再 emit）。下次 speak 重置。 */
    @Volatile
    private var stopped = false

    /** M4：本次 speak 会话内是否已输出过丢块警告（Log.w 一次）。 */
    @Volatile
    private var dropWarned = false

    /** 懒加载引擎（首次 speak 时创建；创建失败 → null → UNAVAILABLE） */
    private var engine: SherpaTtsEngine? = null

    private var mediaPlayer: MediaPlayer? = null

    /** 模型是否已就绪（下载完成 + 解压完成）。 */
    fun isModelReady(): Boolean = downloadManager.isDownloaded(modelSpec)

    /**
     * 合成并播放（TtsProvider 契约）。
     *
     * 1. 模型未下载 → health=UNAVAILABLE 快速失败（不发网络，降级链继续）
     * 2. 引擎不可用（AAR 未集成 / JNI 加载失败 / 初始化失败）→ health=UNAVAILABLE 抛异常
     * 3. Kokoro 合成（PCM float）→ 写 WAV 临时文件 → MediaPlayer 播放
     *
     * 任何失败向上抛（由 VoiceIoController 降级链处理），不在此内部降级（plan §3.1.3）。
     */
    override suspend fun speak(text: String) = withContext(Dispatchers.IO) {
        if (text.isBlank()) return@withContext
        // M4：新 speak 会话重置 emit 守卫（允许 emit；stop() 打断后置位跳过 emit）
        stopped = false
        dropWarned = false
        if (!isModelReady()) {
            Log.i(TAG, "模型未下载（${modelSpec.id}），快速失败，降级链继续")
            _health.value = TtsHealth.UNAVAILABLE
            throw IllegalStateException("Kokoro 离线模型未下载（${modelSpec.name}）")
        }
        val ttsEngine = ensureEngine()
            ?: run {
                _health.value = TtsHealth.UNAVAILABLE
                throw IllegalStateException("sherpa-onnx 引擎不可用（AAR 未集成或初始化失败）")
            }
        try {
            val audio = ttsEngine.synthesize(text, sid, speed)
                ?: throw IllegalStateException("Kokoro 合成失败（${modelSpec.id}）")
            // M4：合成成功后按块 emit 全部 PCM 帧（非阻塞；已被 stop() 打断则跳过）
            if (!stopped) emitAudioFrames(audio)
            playAudio(audio)
            _health.value = TtsHealth.OK
            Log.i(TAG, "Kokoro 播报完成: '${text.take(30)}'（${audio.samples.size} samples @ ${audio.sampleRate}Hz）")
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            _health.value = TtsHealth.DEGRADED
            throw e
        }
    }

    /** 首次调用时引导引擎注册并创建引擎实例（幂等）。 */
    private fun ensureEngine(): SherpaTtsEngine? {
        engine?.let { return it }
        SherpaOnnxEngineRegistry.tryBootstrap()
        val factory = SherpaOnnxEngineRegistry.factoryOrNull()
            ?: return null
        val created = factory.createTts(downloadManager.modelDir(modelSpec))
        if (created != null) {
            engine = created
            Log.i(TAG, "Kokoro 引擎初始化成功（${modelSpec.id}）")
        }
        return created
    }

    /**
     * M4：合成成功后按 150ms 窗口分块 emit PCM 帧（contract-M4-M5 §2.3 生命周期）。
     *
     * - 块长 = sampleRate * AUDIO_FRAME_MS / 1000（150ms 窗口，与 VisemeAnalyzer 对齐；末块截断）
     * - timestampMs = 块索引 × 块时长（第 0 块 = 0，同次 speak 内单调递增）
     * - tryEmit 非阻塞：无订阅者 / 缓冲满 → 丢弃该块 + Log.w 一次（不得 suspend 阻塞 speak 主流程）
     * - 本方法在 speak() 内同步执行：全部块发完后 speak 继续播放流程（预分析先于播放完成）
     */
    private fun emitAudioFrames(audio: SherpaTtsAudio) {
        val samples = audio.samples
        val sampleRate = audio.sampleRate
        if (samples.isEmpty() || sampleRate <= 0) return
        val chunkSize = sampleRate * AUDIO_FRAME_MS / 1000
        if (chunkSize <= 0) return
        var offset = 0
        var index = 0L
        var dropped = false
        while (offset < samples.size) {
            val len = minOf(chunkSize, samples.size - offset)
            val frame = AudioFrame(
                samples = samples.copyOfRange(offset, offset + len),
                sampleRate = sampleRate,
                timestampMs = index * AUDIO_FRAME_MS.toLong(),
            )
            if (!_audioFrameFlow.tryEmit(frame)) {
                dropped = true
            }
            offset += len
            index++
        }
        if (dropped && !dropWarned) {
            Log.w(TAG, "audioFrameFlow 无订阅者或缓冲满，丢弃部分音频块（不影响播放）")
            dropWarned = true
        }
    }

    /** PCM float 采样 → WAV 文件 → MediaPlayer 播放（复用 CosyVoice 播放模式）。 */
    private fun playAudio(audio: SherpaTtsAudio) {
        stop()
        val tempFile = File(context.cacheDir, "sherpa_onnx_tts_temp.wav")
        writeWav(tempFile, audio.samples, audio.sampleRate)
        mediaPlayer = MediaPlayer().apply {
            setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(AudioAttributes.USAGE_MEDIA)
                    .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                    .build()
            )
            setDataSource(tempFile.absolutePath)
            setOnPreparedListener {
                Log.i(TAG, "WAV 就绪，开始播放")
                // M-AND-1（3-B）：prepared→start 间恒 0；start 后轮询非递减
                playbackClock.onPrepared()
                start()
                _isSpeaking.value = true
                playbackClock.onStart()
            }
            setOnCompletionListener {
                Log.i(TAG, "播放完成")
                playbackClock.onStop()
                tempFile.delete()
                _isSpeaking.value = false
            }
            setOnErrorListener { _, what, extra ->
                Log.e(TAG, "MediaPlayer 播放错误: what=$what, extra=$extra")
                playbackClock.onStop()
                tempFile.delete()
                _isSpeaking.value = false
                true
            }
            prepareAsync()
        }
    }

    /**
     * 16-bit PCM 单声道 WAV 写入（44 字节标准头；sherpa-onnx samples 为 -1..1 float）。
     */
    internal fun writeWav(file: File, samples: FloatArray, sampleRate: Int) {
        val dataSize = samples.size * 2
        RandomAccessFile(file, "rw").use { raf ->
            raf.setLength(0)
            raf.writeBytes("RIFF")
            writeIntLE(raf, 36 + dataSize)
            raf.writeBytes("WAVE")
            raf.writeBytes("fmt ")
            writeIntLE(raf, 16)          // fmt chunk 长度
            writeShortLE(raf, 1)         // PCM
            writeShortLE(raf, 1)         // 单声道
            writeIntLE(raf, sampleRate)
            writeIntLE(raf, sampleRate * 2) // byte rate
            writeShortLE(raf, 2)         // block align
            writeShortLE(raf, 16)        // bits per sample
            raf.writeBytes("data")
            writeIntLE(raf, dataSize)
            val buffer = ByteBuffer.allocate(dataSize).order(ByteOrder.LITTLE_ENDIAN)
            for (sample in samples) {
                val pcm = (sample.coerceIn(-1f, 1f) * Short.MAX_VALUE).toInt().toShort()
                buffer.putShort(pcm)
            }
            raf.write(buffer.array())
        }
    }

    private fun writeIntLE(raf: RandomAccessFile, value: Int) {
        raf.write(value and 0xFF)
        raf.write((value shr 8) and 0xFF)
        raf.write((value shr 16) and 0xFF)
        raf.write((value shr 24) and 0xFF)
    }

    private fun writeShortLE(raf: RandomAccessFile, value: Int) {
        raf.write(value and 0xFF)
        raf.write((value shr 8) and 0xFF)
    }

    override fun stop() {
        // M-AND-1：stop() 后本次 speak 会话不得再 emit（emit 守卫置位；下次 speak 重置）
        stopped = true
        playbackClock.onStop()
        mediaPlayer?.apply {
            try {
                if (isPlaying) stop()
            } catch (_: Throwable) {
            }
            try {
                release()
            } catch (_: Throwable) {
            }
        }
        mediaPlayer = null
        _isSpeaking.value = false
    }

    override fun release() {
        stop()
        try {
            engine?.release()
        } catch (_: Throwable) {
        }
        engine = null
    }
}
