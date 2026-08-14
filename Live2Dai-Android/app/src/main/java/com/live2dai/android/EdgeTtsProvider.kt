package com.live2dai.android

import android.content.Context
import android.media.AudioAttributes
import android.media.MediaCodec
import android.media.MediaExtractor
import android.media.MediaFormat
import android.media.MediaPlayer
import android.util.Log
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
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.FileOutputStream
import java.util.UUID
import java.util.concurrent.ConcurrentLinkedQueue
import java.util.concurrent.TimeUnit

/**
 * Edge TTS Provider（V1.2 模块1：EdgeTtsService → EdgeTtsProvider 重构）。
 *
 * 实现 [TtsProvider] 接口，保留原 EdgeTtsService 的 WebSocket 合成 + MediaPlayer
 * 播放逻辑；Android 系统 TTS 降级逻辑已剥离至 [SystemTtsProvider]（plan §3.1.8），
 * 原内部三层降级链（Edge → 系统 TTS → 静默）改为注册表外部降级链
 * （edge_tts → system_tts → 静默，由 VoiceIoController 遍历 [TtsProviderRegistry.cascade]）。
 *
 * 契约变化（plan §3.1.3）：speak 失败必须向上抛异常（由降级链捕获继续尝试下一个
 * Provider），不再内部吞掉后降级到系统 TTS。
 *
 * Edge TTS 使用 `wss://speech.platform.bing.com/` 公共端点，
 * 通过 WebSocket 发送 SSML 并接收 MP3 音频流。
 */
class EdgeTtsProvider(
    private val context: Context,
    /** 测试注入：OkHttpClient（默认真实客户端；MockWebServer 测试可注入）。 */
    private val client: OkHttpClient = defaultOkHttpClient(),
    /** 测试注入：Edge TTS WebSocket 端点（默认公共端点；MockWebServer 测试可覆盖）。 */
    private val wsUrl: String = EDGE_WSS_URL,
    /** M-AND-1 测试注入：MP3→PCM 解码器（默认 [decodeMp3ToPcm] MediaExtractor+MediaCodec；JVM 测试注入 fake）。 */
    private val pcmDecoder: (ByteArray) -> DecodedAudio? = ::decodeMp3ToPcm,
    /** M-AND-1 测试注入：播放位置轮询作用域（默认 Main；测试注入 TestDispatcher 驱动虚拟时间）。 */
    private val positionPollScope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate),
) : TtsProvider {

    companion object {
        private const val TAG = "TtsProvider"

        /** Provider 唯一标识（契约 schema providersV12.edge_tts） */
        const val PROVIDER_ID = "edge_tts"

        private const val EDGE_WSS_URL = "wss://speech.platform.bing.com/consumer/speech/synthesize/readaloud/edge/v1?TrustedClientToken=6A5AA1D4EAFF4E9FB37E23D68491D6F4"

        /** 默认中文语音 */
        private const val VOICE = "zh-CN-XiaoxiaoNeural"

        /** Edge TTS WebSocket 超时（连接建立 + 音频接收） */
        private const val WS_TIMEOUT_SECONDS = 30L

        /** WebSocket 读取超时 */
        private const val WS_READ_TIMEOUT_SECONDS = 60L

        /** M-AND-1：帧流缓冲容量（150ms 块数级；超出丢块 + Log.w，不阻塞 speak） */
        private const val AUDIO_FLOW_BUFFER_CAPACITY = 64

        private fun defaultOkHttpClient(): OkHttpClient = OkHttpClient.Builder()
            .connectTimeout(WS_TIMEOUT_SECONDS, TimeUnit.SECONDS)
            .readTimeout(WS_READ_TIMEOUT_SECONDS, TimeUnit.SECONDS)
            .writeTimeout(WS_TIMEOUT_SECONDS, TimeUnit.SECONDS)
            .build()
    }

    private val _health = MutableStateFlow(TtsHealth.OK)

    override val info = TtsProviderInfo(
        id = PROVIDER_ID,
        name = "Edge TTS",
        category = TtsProviderCategory.CLOUD_WS,
        requiresNetwork = true,
        health = _health,
        // Wave A：CosyVoice(0) 未注册前 edge 暂占 1；CosyVoice 注册为 0 时按数值自动前移（plan §3.1.6）
        priority = 1,
        configurable = false,
    )

    private val _isSpeaking = MutableStateFlow(false)
    override val isSpeaking: StateFlow<Boolean> = _isSpeaking.asStateFlow()

    /**
     * M-AND-1（2a-①）：MP3 整段预解码 → PCM 帧流（150ms 固定分块，播放前全量 emit）。
     * 时序契约（AND1-A7）：消费者在 speak() 前启动 collect；speak 后 1s 内首帧。
     * tryEmit 非阻塞：无订阅者/缓冲满 → 丢块 + Log.w 一次，不影响 speak 主流程。
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

    /** M-AND-1：本次 speak 会话内是否已被 stop() 打断（打断后不再 emit）。下次 speak 重置。 */
    @Volatile
    private var stopped = false

    /** M-AND-1：本次 speak 会话内是否已输出过丢块警告（Log.w 一次）。 */
    @Volatile
    private var dropWarned = false

    private var mediaPlayer: MediaPlayer? = null

    /**
     * 语音合成 + 播放（TtsProvider 契约）。
     *
     * 仅执行 Edge TTS WebSocket 合成 → MediaPlayer 播放；任何失败（WebSocket
     * 异常 / 未返回音频 / 播放器同步错误）向上抛异常，由上层降级链处理
     * （VoiceIoController → TtsProviderRegistry.cascade）。不再内部降级到系统 TTS。
     *
     * 播放为异步（MediaPlayer.prepareAsync）：本方法在音频开始缓冲后即返回，
     * 播放状态通过 [isSpeaking] StateFlow 暴露（LipSync/UI 观察）。
     */
    override suspend fun speak(text: String) = withContext(Dispatchers.IO) {
        try {
            // M-AND-1：新 speak 会话重置 emit 守卫
            stopped = false
            dropWarned = false
            val audioBytes = synthesizeEdgeTts(text)
            if (audioBytes != null) {
                // M-AND-1（2a-①）：MP3 预解码 → 150ms 帧流，先于播放全量 emit（消费侧重锚）
                emitAudioFrames(audioBytes)
                playAudioBytes(audioBytes)
                _health.value = TtsHealth.OK
            } else {
                throw IllegalStateException("Edge TTS 未返回音频数据")
            }
        } catch (e: Exception) {
            _health.value = TtsHealth.DEGRADED
            throw e
        }
    }

    /**
     * Edge TTS WebSocket 合成
     *
     * 1. 建立 WebSocket 连接到微软 TTS 端点
     * 2. 发送 speech.config 配置音频格式
     * 3. 发送 SSML 指定语音、语速和文本内容
     * 4. 从二进制消息中提取 MP3 音频分片
     * 5. 合并所有分片并返回完整 MP3 数据
     *
     * 超时控制：最多等待 WS_TIMEOUT_SECONDS 秒
     * 重试逻辑：不在此处重试，由上层降级链处理
     */
    private fun synthesizeEdgeTts(text: String): ByteArray? {
        val audioParts = ConcurrentLinkedQueue<ByteArray>()
        var done = false
        var error: Exception? = null

        val wsRequest = Request.Builder()
            .url(wsUrl + "&ConnectionId=" + UUID.randomUUID().toString().uppercase())
            .addHeader("Pragma", "no-cache")
            .addHeader("Cache-Control", "no-cache")
            .addHeader("Origin", "chrome-extension://jdiccldimpdaibmpdkjnbmckianbfold")
            .build()

        val ws = client.newWebSocket(wsRequest, object : WebSocketListener() {
            override fun onOpen(ws: WebSocket, response: Response) {
                Log.i(TAG, "Edge TTS WebSocket 已连接")
                sendConfig(ws)
                sendSsml(ws, text)
            }

            override fun onMessage(ws: WebSocket, text: String) {
                // JSON metadata messages — ignore for audio extraction
            }

            override fun onMessage(ws: WebSocket, bytes: okio.ByteString) {
                val data = parseAudioBinary(bytes.toByteArray())
                if (data != null) {
                    audioParts.add(data)
                }
            }

            override fun onClosing(ws: WebSocket, code: Int, reason: String) {
                Log.i(TAG, "Edge TTS WebSocket 关闭中: code=$code, reason=$reason")
                done = true
                ws.close(1000, null)
            }

            override fun onClosed(ws: WebSocket, code: Int, reason: String) {
                Log.i(TAG, "Edge TTS WebSocket 已关闭")
                done = true
            }

            override fun onFailure(ws: WebSocket, t: Throwable, response: Response?) {
                Log.e(TAG, "Edge TTS WebSocket 连接失败: ${t.message}")
                error = t as? Exception ?: RuntimeException(t)
                done = true
            }
        })

        // 等待完成或超时
        val start = System.currentTimeMillis()
        while (!done && error == null && System.currentTimeMillis() - start < WS_TIMEOUT_SECONDS * 1000) {
            Thread.sleep(50)
        }

        if (!done) {
            // 超时关闭
            Log.w(TAG, "Edge TTS WebSocket 超时 (${WS_TIMEOUT_SECONDS}s)，强制关闭")
            ws.close(1000, "Timeout")
            done = true
        }

        if (error != null) throw error!!

        // 没有收到音频数据不算异常（可能是空文本或 SSML 格式问题）→ 由 speak 抛异常触发降级
        if (audioParts.isEmpty()) return null

        // 合并所有音频分片
        val totalSize = audioParts.sumOf { it.size }
        val result = ByteArray(totalSize)
        var offset = 0
        for (part in audioParts) {
            System.arraycopy(part, 0, result, offset, part.size)
            offset += part.size
        }
        return result
    }

    /**
     * 发送 speech.config 消息
     * 配置：MP3 格式，24kHz 48kbit，单声道
     */
    private fun sendConfig(ws: WebSocket) {
        val configJson = """{"context":{"synthesis":{"audio":{"metadataoptions":{"sentenceBoundaryEnabled":false,"wordBoundaryEnabled":false},"outputFormat":"audio-24khz-48kbitrate-mono-mp3"}}}}}"""
        ws.send("""Path: speech.config
Content-Type: application/json; charset=utf-8

$configJson""")
    }

    /**
     * 发送 SSML 合成请求
     * 语音：zh-CN-XiaoxiaoNeural
     * 语速：0%（正常）
     * 音调：0%（正常）
     */
    private fun sendSsml(ws: WebSocket, text: String) {
        // HTML 转义文本内容
        val escaped = text
            .replace("&", "&amp;")
            .replace("<", "&lt;")
            .replace(">", "&gt;")
            .replace("\"", "&quot;")
            .replace("'", "&apos;")
        val ssml = """<speak version="1.0" xmlns="http://www.w3.org/2001/10/synthesis" xml:lang="zh-CN">
  <voice name="$VOICE">
    <prosody rate="0%" pitch="0%">$escaped</prosody>
  </voice>
</speak>"""
        ws.send("""Path: ssml
Content-Type: application/ssml+xml

$ssml""")
    }

    /**
     * 从 WebSocket 二进制消息中解析出纯 MP3 音频数据
     *
     * 微软 Edge TTS 的二进制消息格式：
     * - 消息头（包含 Path 信息，以 \r\n\r\n 结尾）
     * - 音频数据（前 2 字节为标记，后续为纯 MP3 帧）
     *
     * audio.metadata 消息不包含音频数据，跳过。
     */
    private fun parseAudioBinary(data: ByteArray): ByteArray? {
        // 检查是否为 metadata 消息（文本格式，不含音频）
        val str = data.decodeToString()
        if (str.startsWith("Path:audio.metadata")) {
            return null
        }

        // 查找 header 和 body 的分隔符
        val separator = "\r\n\r\n".toByteArray()
        val headerEnd = indexOfByteArray(data, separator)
        if (headerEnd == -1) return null

        // 提取 body（跳过 header + 分隔符）
        val body = data.copyOfRange(headerEnd + 4, data.size)

        // 跳过 body 前 2 字节标记，获取纯音频数据
        return if (body.size > 2) body.copyOfRange(2, body.size) else null
    }

    /**
     * 播放 MP3 音频字节数据
     *
     * 将字节数组写入缓存目录的临时文件，然后使用 MediaPlayer 播放。
     * 播放完成后自动删除临时文件。
     *
     * 同步阶段（写文件 / setDataSource）失败向上抛异常 → speak 抛异常触发降级；
     * 异步阶段（prepareAsync 后）失败通过 onError 置 [isSpeaking]=false 并删除临时文件
     * （此阶段 speak 已返回，无法再降级——与 V1.1 行为一致）。
     */
    private fun playAudioBytes(audioBytes: ByteArray) {
        stop()
        val tempFile = File(context.cacheDir, "tts_temp.mp3")
        FileOutputStream(tempFile).use { it.write(audioBytes) }
        mediaPlayer = MediaPlayer().apply {
            setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(AudioAttributes.USAGE_MEDIA)
                    .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                    .build()
            )
            setDataSource(tempFile.absolutePath)
            setOnPreparedListener {
                // M-AND-1（3-B）：prepared→start 间恒 0；start 后轮询非递减
                playbackClock.onPrepared()
                start()
                // 真实播放开始 → isSpeaking(true)（LipSync 驱动）
                _isSpeaking.value = true
                playbackClock.onStart()
            }
            setOnCompletionListener {
                playbackClock.onStop()
                tempFile.delete()
                // 自然播放完成 → isSpeaking(false)
                _isSpeaking.value = false
            }
            setOnErrorListener { _, what, extra ->
                Log.e(TAG, "Edge TTS MediaPlayer 错误: what=$what, extra=$extra")
                playbackClock.onStop()
                tempFile.delete()
                // 播放中断 → isSpeaking(false)
                _isSpeaking.value = false
                true
            }
            prepareAsync()
        }
    }

    /**
     * M-AND-1（2a-①）：MP3 → PCM 整段预解码 → 150ms 分块 emit（播放前全量；非阻塞 tryEmit）。
     * 解码失败（真机异常 / JVM 无 MediaCodec）→ 无帧流 + Log.w，不影响播放路径。
     */
    private fun emitAudioFrames(mp3Bytes: ByteArray) {
        if (stopped) return
        val decoded = try {
            pcmDecoder(mp3Bytes)
        } catch (e: Exception) {
            Log.w(TAG, "Edge TTS MP3 解码异常，无帧流（不影响播放）: ${e.message}")
            null
        }
        if (decoded == null || decoded.samples.isEmpty() || decoded.sampleRate <= 0) {
            Log.w(TAG, "Edge TTS MP3 解码失败/空 PCM，无帧流（不影响播放）")
            return
        }
        var dropped = false
        for (frame in LipSyncFrameChunker.chunk(decoded.samples, decoded.sampleRate)) {
            if (!_audioFrameFlow.tryEmit(frame)) dropped = true
        }
        if (dropped && !dropWarned) {
            Log.w(TAG, "Edge TTS audioFrameFlow 无订阅者或缓冲满，丢弃部分音频块（不影响播放）")
            dropWarned = true
        }
    }

    /**
     * 停止当前播放（打断；幂等）。
     */
    override fun stop() {
        // M-AND-1：stop() 后本次 speak 会话不得再 emit（emit 守卫置位；下次 speak 重置）
        stopped = true
        playbackClock.onStop()
        mediaPlayer?.apply {
            if (isPlaying) stop()
            release()
        }
        mediaPlayer = null
        // StateFlow 自动去重：未在播放时置 false 是 no-op
        _isSpeaking.value = false
    }

    /**
     * 释放所有资源（幂等）。Edge TTS 无 Android TTS/长连接需单独关闭——
     * WebSocket 每次合成即建即断，OkHttpClient 随 Provider 生命周期。
     */
    override fun release() {
        stop()
    }

    /**
     * 在 ByteArray 中查找子数组的起始位置
     * @return 子数组的起始索引，未找到返回 -1
     */
    private fun indexOfByteArray(source: ByteArray, target: ByteArray): Int {
        if (target.isEmpty()) return 0
        if (source.size < target.size) return -1
        outer@ for (i in 0..source.size - target.size) {
            for (j in target.indices) {
                if (source[i + j] != target[j]) continue@outer
            }
            return i
        }
        return -1
    }
}

/**
 * M-AND-1（2a-①）：MP3 → PCM 解码结果（float 单声道采样 + 采样率）。
 */
data class DecodedAudio(
    val samples: FloatArray,
    val sampleRate: Int,
)

/**
 * M-AND-1（2a-①）：MediaExtractor + MediaCodec 整段预解码 MP3 → float 单声道 PCM（-1..1）。
 *
 * - 多声道输出降混为第一声道（步进 2 × channelCount）
 * - 空输入 / 解码失败 → null（调用方降级为无帧流，不影响播放路径）
 * - API 21+（minSdk 26 满足）；JVM 单测无 MediaCodec（“not mocked”）→ 捕获返回 null
 * - 确定性：同 MP3 输入 → 同 PCM 输出（固定分块后帧序列完全确定，AND1-A2）
 */
internal fun decodeMp3ToPcm(mp3Bytes: ByteArray): DecodedAudio? {
    if (mp3Bytes.isEmpty()) return null
    val tempFile = File.createTempFile("tts_mp3_decode", ".mp3")
    try {
        tempFile.writeBytes(mp3Bytes)
        return decodeAudioFile(tempFile)
    } catch (_: Throwable) {
        return null
    } finally {
        tempFile.delete()
    }
}

private fun decodeAudioFile(file: File): DecodedAudio? {
    val extractor = MediaExtractor()
    var codec: MediaCodec? = null
    try {
        extractor.setDataSource(file.absolutePath)
        if (extractor.trackCount <= 0) return null
        val format = extractor.getTrackFormat(0)
        val mime = format.getString(MediaFormat.KEY_MIME)
        if (mime == null || !mime.startsWith("audio/")) return null
        extractor.selectTrack(0)
        codec = MediaCodec.createDecoderByType(mime)
        codec.configure(format, null, null, 0)
        codec.start()
        val inputData = file.readBytes()
        val bufferInfo = MediaCodec.BufferInfo()
        val pcmOut = ByteArrayOutputStream()
        var inputOffset = 0
        var inputDone = false
        var outputDone = false
        var sampleRate = if (format.containsKey(MediaFormat.KEY_SAMPLE_RATE))
            format.getInteger(MediaFormat.KEY_SAMPLE_RATE) else 24000
        var channelCount = if (format.containsKey(MediaFormat.KEY_CHANNEL_COUNT))
            format.getInteger(MediaFormat.KEY_CHANNEL_COUNT) else 1
        var idleLoops = 0
        while (!outputDone) {
            if (!inputDone) {
                val inIndex = codec.dequeueInputBuffer(10_000)
                if (inIndex >= 0) {
                    val inBuf = codec.getInputBuffer(inIndex) ?: return null
                    inBuf.clear()
                    val remaining = inputData.size - inputOffset
                    val chunk = minOf(inBuf.capacity(), remaining)
                    if (chunk > 0) inBuf.put(inputData, inputOffset, chunk)
                    inputOffset += chunk
                    val eos = inputOffset >= inputData.size
                    codec.queueInputBuffer(
                        inIndex, 0, chunk, 0L,
                        if (eos) MediaCodec.BUFFER_FLAG_END_OF_STREAM else 0,
                    )
                    if (eos) inputDone = true
                }
            }
            val outIndex = codec.dequeueOutputBuffer(bufferInfo, 10_000)
            when {
                outIndex >= 0 -> {
                    idleLoops = 0
                    if (bufferInfo.size > 0) {
                        val outBuf = codec.getOutputBuffer(outIndex) ?: return null
                        outBuf.position(bufferInfo.offset)
                        outBuf.limit(bufferInfo.offset + bufferInfo.size)
                        val chunk = ByteArray(bufferInfo.size)
                        outBuf.get(chunk)
                        pcmOut.write(chunk)
                    }
                    codec.releaseOutputBuffer(outIndex, false)
                    if (bufferInfo.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0) outputDone = true
                }
                outIndex == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED -> {
                    val outFormat = codec.outputFormat
                    if (outFormat.containsKey(MediaFormat.KEY_SAMPLE_RATE)) {
                        sampleRate = outFormat.getInteger(MediaFormat.KEY_SAMPLE_RATE)
                    }
                    if (outFormat.containsKey(MediaFormat.KEY_CHANNEL_COUNT)) {
                        channelCount = outFormat.getInteger(MediaFormat.KEY_CHANNEL_COUNT)
                    }
                }
                else -> {
                    idleLoops++
                    if (idleLoops > 500) break // 5s 无进展保护（EOS 丢失等异常态）
                }
            }
        }
        val raw = pcmOut.toByteArray()
        if (raw.size < 2 || sampleRate <= 0) return null
        // 16-bit PCM → float 单声道（-1..1）；多声道取第一声道
        val stride = 2 * channelCount.coerceAtLeast(1)
        val sampleCount = raw.size / stride
        if (sampleCount <= 0) return null
        val floats = FloatArray(sampleCount)
        var i = 0
        var s = 0
        while (s + 1 < raw.size) {
            val low = raw[s].toInt() and 0xFF
            val high = raw[s + 1].toInt() shl 8
            floats[i++] = (low or high).toShort() / 32768f
            s += stride
        }
        return DecodedAudio(floats, sampleRate)
    } catch (_: Throwable) {
        return null
    } finally {
        try {
            codec?.release()
        } catch (_: Throwable) {
        }
        try {
            extractor.release()
        } catch (_: Throwable) {
        }
    }
}
