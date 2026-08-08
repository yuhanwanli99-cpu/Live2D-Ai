package com.live2dai.android

import android.content.Context
import android.media.AudioAttributes
import android.media.MediaPlayer
import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.emptyFlow
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
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
class EdgeTtsProvider(private val context: Context) : TtsProvider {

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
     * M4：音频帧流（v1.0 空流）。
     *
     * Edge TTS 输出 MP3；MP3→PCM 客户端解码（MediaCodec）可行性/延迟按 **UNC-4**
     * 留 M4 之后评估，本版本不实现解码（contract-M4-M5 §2.2 空流策略）。
     */
    override val audioFrameFlow: Flow<AudioFrame> = emptyFlow()

    private val client = OkHttpClient.Builder()
        .connectTimeout(WS_TIMEOUT_SECONDS, TimeUnit.SECONDS)
        .readTimeout(WS_READ_TIMEOUT_SECONDS, TimeUnit.SECONDS)
        .writeTimeout(WS_TIMEOUT_SECONDS, TimeUnit.SECONDS)
        .build()

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
            val audioBytes = synthesizeEdgeTts(text)
            if (audioBytes != null) {
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
            .url(EDGE_WSS_URL + "&ConnectionId=" + UUID.randomUUID().toString().uppercase())
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
                start()
                // 真实播放开始 → isSpeaking(true)（LipSync 驱动）
                _isSpeaking.value = true
            }
            setOnCompletionListener {
                tempFile.delete()
                // 自然播放完成 → isSpeaking(false)
                _isSpeaking.value = false
            }
            setOnErrorListener { _, what, extra ->
                Log.e(TAG, "Edge TTS MediaPlayer 错误: what=$what, extra=$extra")
                tempFile.delete()
                // 播放中断 → isSpeaking(false)
                _isSpeaking.value = false
                true
            }
            prepareAsync()
        }
    }

    /**
     * 停止当前播放（打断；幂等）。
     */
    override fun stop() {
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
