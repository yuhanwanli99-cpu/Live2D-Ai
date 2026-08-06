package com.live2dai.android

import android.content.Context
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import android.util.Log
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext
import java.util.concurrent.atomic.AtomicBoolean

/**
 * sherpa-onnx + SenseVoice 离线 ASR Provider（V1.2 模块4）。
 *
 * 契约见 docs/plan-v12-plan.schema.json → providersV12.sherpa_sensevoice：
 * - id="sherpa_sensevoice" / requiresNetwork=false / priority=0（ASR 降级链第一位）
 * - 模型未下载 → health=UNAVAILABLE，[listen] 快速返回 ""（不发网络，降级到系统 ASR）
 * - 识别：麦克风录音（16kHz/16bit 单声道）→ sherpa-onnx OfflineRecognizer 整段识别
 *   （plan §4.4.2 方案A：SenseVoice-zh，非流式整段识别）
 *
 * 引擎加载走 [SherpaOnnxEngineRegistry]（条件源码集）：AAR 未集成 / JVM 环境无法加载
 * 原生库时引擎为 null → health=UNAVAILABLE（plan §8.3 回退策略）。
 *
 * 日志规范（plan §5.7.4）：关键节点 Log.i（国产 ROM 抑制 Log.d 教训），TAG="SherpaOnnx"。
 */
class SherpaOnnxAsrProvider(
    private val context: Context,
    private val downloadManager: ModelDownloadManager = ModelDownloadManager(context),
    private val modelSpec: ModelSpec = ModelDownloadManager.SENSEVOICE_ASR,
    /** 单次监听最长时长（毫秒） */
    private val maxRecordMs: Long = DEFAULT_MAX_RECORD_MS,
    /** 静音判定阈值（RMS，0-32767 相对幅度） */
    private val silenceThreshold: Double = DEFAULT_SILENCE_THRESHOLD,
    /** 静音多久结束录音（毫秒） */
    private val silenceTimeoutMs: Long = DEFAULT_SILENCE_TIMEOUT_MS,
) : AsrProvider {

    companion object {
        private const val TAG = "SherpaOnnx"

        /** Provider 唯一标识（契约 schema providersV12.sherpa_sensevoice） */
        const val PROVIDER_ID = "sherpa_sensevoice"

        private const val SAMPLE_RATE = 16_000
        private const val DEFAULT_MAX_RECORD_MS = 10_000L
        private const val DEFAULT_SILENCE_THRESHOLD = 800.0
        private const val DEFAULT_SILENCE_TIMEOUT_MS = 1_200L
    }

    private val _health = MutableStateFlow(
        if (downloadManager.isDownloaded(modelSpec)) TtsHealth.OK else TtsHealth.UNAVAILABLE
    )

    override val info = AsrProviderInfo(
        id = PROVIDER_ID,
        name = "SenseVoice 离线 ASR",
        requiresNetwork = false, // 离线可用（plan §4.4.2）
        health = _health,
        // ASR 降级链 Tier 1（plan §4.4.4）：离线优先，模型未下载时快速降级
        priority = 0,
    )

    /** 停止监听请求（record 循环检查；幂等） */
    private val stopRequested = AtomicBoolean(false)

    /** 懒加载引擎（首次 listen 时创建；创建失败 → null → UNAVAILABLE） */
    private var engine: SherpaAsrEngine? = null

    /** 模型是否已就绪（下载完成 + 解压完成）。 */
    fun isModelReady(): Boolean = downloadManager.isDownloaded(modelSpec)

    /**
     * 开始监听并返回识别文本（AsrProvider 契约）。
     *
     * 1. 模型未下载 → health=UNAVAILABLE 快速返回 ""（降级到系统 ASR）
     * 2. 引擎不可用（AAR 未集成 / JNI 加载失败）→ health=UNAVAILABLE 返回 ""
     * 3. 录音（16kHz/16bit 单声道，静音自动结束 / 最长 10s）→ 整段识别
     *
     * 失败一律返回 ""（契约：失败/超时/无匹配返回空串），不向调用方抛异常。
     */
    override suspend fun listen(): String = withContext(Dispatchers.IO) {
        if (!isModelReady()) {
            Log.i(TAG, "模型未下载（${modelSpec.id}），快速降级到系统 ASR")
            _health.value = TtsHealth.UNAVAILABLE
            return@withContext ""
        }
        val asrEngine = ensureEngine() ?: run {
            _health.value = TtsHealth.UNAVAILABLE
            return@withContext ""
        }
        stopRequested.set(false)
        try {
            val pcm = recordPcm()
            if (pcm.isEmpty()) {
                Log.i(TAG, "未采集到有效语音")
                return@withContext ""
            }
            val text = asrEngine.recognize(pcm, SAMPLE_RATE)
            if (text.isNotEmpty()) {
                _health.value = TtsHealth.OK
                Log.i(TAG, "识别结果: '${text.take(40)}'")
            }
            text
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            Log.w(TAG, "识别异常，降级到系统 ASR: ${e.message}")
            _health.value = TtsHealth.DEGRADED
            ""
        }
    }

    /** 首次调用时引导引擎注册并创建引擎实例（幂等）。 */
    private fun ensureEngine(): SherpaAsrEngine? {
        engine?.let { return it }
        SherpaOnnxEngineRegistry.tryBootstrap()
        val factory = SherpaOnnxEngineRegistry.factoryOrNull()
            ?: return null
        val created = factory.createAsr(downloadManager.modelDir(modelSpec))
        if (created != null) {
            engine = created
            Log.i(TAG, "SenseVoice 引擎初始化成功（${modelSpec.id}）")
        }
        return created
    }

    /**
     * 麦克风录音（16kHz/16bit 单声道）：静音持续 [silenceTimeoutMs] 或达到
     * [maxRecordMs] 即结束。返回归一化 float 采样（-1..1）。
     */
    private fun recordPcm(): FloatArray {
        val minBuffer = AudioRecord.getMinBufferSize(
            SAMPLE_RATE, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT
        )
        val bufferSize = maxOf(minBuffer * 2, 8192)
        val recorder = AudioRecord(
            MediaRecorder.AudioSource.MIC, SAMPLE_RATE,
            AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT, bufferSize
        )
        if (recorder.state != AudioRecord.STATE_INITIALIZED) {
            Log.w(TAG, "AudioRecord 初始化失败（麦克风权限？）")
            return FloatArray(0)
        }
        val samples = ArrayList<Float>(SAMPLE_RATE * 5)
        val shortBuffer = ShortArray(bufferSize / 2)
        recorder.startRecording()
        try {
            var silenceMs = 0L
            val start = System.currentTimeMillis()
            while (!stopRequested.get()) {
                val read = recorder.read(shortBuffer, 0, shortBuffer.size)
                if (read <= 0) break
                var sum = 0.0
                for (i in 0 until read) {
                    val v = shortBuffer[i]
                    samples.add(v / 32768f)
                    sum += (v * v).toDouble()
                }
                val rms = Math.sqrt(sum / read)
                silenceMs = if (rms < silenceThreshold) silenceMs + (read * 1000L / SAMPLE_RATE) else 0L
                if (samples.size * 1000L / SAMPLE_RATE >= maxRecordMs) break
                if (silenceMs >= silenceTimeoutMs && samples.size * 1000L / SAMPLE_RATE > 300) break
            }
        } finally {
            try {
                recorder.stop()
            } catch (_: Throwable) {
            }
            recorder.release()
        }
        val result = FloatArray(samples.size)
        for (i in samples.indices) result[i] = samples[i]
        return result
    }

    override fun stopListening() {
        stopRequested.set(true)
    }

    override fun release() {
        stopListening()
        try {
            engine?.release()
        } catch (_: Throwable) {
        }
        engine = null
    }
}
