package com.live2dai.android

import android.content.Context
import android.media.AudioAttributes
import android.media.MediaPlayer
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.speech.tts.TextToSpeech
import android.speech.tts.UtteranceProgressListener
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import okhttp3.*
import java.io.File
import java.io.FileOutputStream
import java.util.*
import java.util.concurrent.ConcurrentLinkedQueue
import java.util.concurrent.TimeUnit
import kotlin.coroutines.resume

/**
 * Edge TTS 服务
 *
 * 三层降级策略：
 *   1. Edge TTS (WebSocket 直连微软 TTS 引擎)
 *   2. Android 系统 TTS (TextToSpeech)
 *   3. 静默降级（不播放任何音频）
 *
 * Edge TTS 使用 `wss://speech.platform.bing.com/` 公共端点，
 * 通过 WebSocket 发送 SSML 并接收 MP3 音频流。
 */
class EdgeTtsService(private val context: Context) {

    companion object {
        private const val EDGE_WSS_URL = "wss://speech.platform.bing.com/consumer/speech/synthesize/readaloud/edge/v1?TrustedClientToken=6A5AA1D4EAFF4E9FB37E23D68491D6F4"

        /** 默认中文语音 */
        private const val VOICE = "zh-CN-XiaoxiaoNeural"

        /** Edge TTS WebSocket 超时（连接建立 + 音频接收） */
        private const val WS_TIMEOUT_SECONDS = 30L

        /** WebSocket 读取超时 */
        private const val WS_READ_TIMEOUT_SECONDS = 60L
    }

    private val client = OkHttpClient.Builder()
        .connectTimeout(WS_TIMEOUT_SECONDS, TimeUnit.SECONDS)
        .readTimeout(WS_READ_TIMEOUT_SECONDS, TimeUnit.SECONDS)
        .writeTimeout(WS_TIMEOUT_SECONDS, TimeUnit.SECONDS)
        .build()

    private var mediaPlayer: MediaPlayer? = null

    private var androidTts: TextToSpeech? = null
    private var androidTtsReady = false

    /**
     * 播放状态回调（V1.1 模块4 LipSync 驱动，契约见 docs/plan-v11-plan.schema.json →
     * "EdgeTtsService (扩展) onPlaybackStateChanged"）。
     *
     * - true = 开始播放（MediaPlayer onPrepared→start / Android TTS onStart）
     * - false = 播放结束/中断（MediaPlayer onCompletion/onError / Android TTS onDone/onError / stop()）
     *
     * 契约要求：回调在 Android 主线程执行（[Handler] 主线程 post），负责启停 LipSync 动画协程；
     * 重复状态不触发（内部 [playbackState] 去重）。
     */
    var onPlaybackStateChanged: ((Boolean) -> Unit)? = null

    /** 当前播放状态（回调可能来自 MediaPlayer(IO)/Android TTS(主线程) 不同线程，@Volatile 去重） */
    @Volatile
    private var playbackState = false

    private val mainHandler = Handler(Looper.getMainLooper())

    init {
        // 初始化 Android 系统 TTS 作为降级方案
        androidTts = TextToSpeech(context) { status ->
            androidTtsReady = (status == TextToSpeech.SUCCESS)
            androidTts?.language = Locale.CHINESE
        }
    }

    /**
     * 广播播放状态变化（仅状态切换时触发；主线程回调）。
     *
     * 幂等设计：连续广播同一状态是 no-op（MediaPlayer 完成回调与 stop() 可能竞态，
     * 重复 false 无意义且会干扰 LipSync 协程启停）。
     */
    private fun notifyPlaybackState(playing: Boolean) {
        if (playing == playbackState) return
        playbackState = playing
        val callback = onPlaybackStateChanged ?: return
        mainHandler.post { callback(playing) }
    }

    /**
     * 语音合成 + 播放
     *
     * 降级链：
     *   1. Edge TTS WebSocket → 成功 → 返回
     *   2. Android 系统 TTS → 成功 → 返回
     *   3. 静默降级（什么都不做）
     */
    suspend fun speak(text: String) = withContext(Dispatchers.IO) {
        // Tier 1: Edge TTS
        try {
            val audioBytes = synthesizeEdgeTts(text)
            if (audioBytes != null) {
                playAudioBytes(audioBytes)
                return@withContext
            }
        } catch (e: Exception) {
            android.util.Log.w("EdgeTtsService", "Edge TTS 失败，降级到系统 TTS: ${e.message}")
        }

        // Tier 2: Android 系统 TTS
        try {
            speakAndroid(text)
        } catch (e: Exception) {
            android.util.Log.w("EdgeTtsService", "系统 TTS 也失败，静默降级: ${e.message}")
            // Tier 3: 静默降级 — 什么都不做
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
     * 重试逻辑：不在此处重试，由上层降级策略处理
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
                android.util.Log.d("EdgeTtsService", "WebSocket 已连接")
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
                android.util.Log.d("EdgeTtsService", "WebSocket 关闭中: code=$code, reason=$reason")
                done = true
                ws.close(1000, null)
            }

            override fun onClosed(ws: WebSocket, code: Int, reason: String) {
                android.util.Log.d("EdgeTtsService", "WebSocket 已关闭")
                done = true
            }

            override fun onFailure(ws: WebSocket, t: Throwable, response: Response?) {
                android.util.Log.e("EdgeTtsService", "WebSocket 连接失败: ${t.message}")
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
            android.util.Log.w("EdgeTtsService", "WebSocket 超时 (${WS_TIMEOUT_SECONDS}s)，强制关闭")
            ws.close(1000, "Timeout")
            done = true
        }

        if (error != null) throw error!!

        // 没有收到音频数据不算异常（可能是空文本或 SSML 格式问题）
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
     */
    private fun playAudioBytes(audioBytes: ByteArray) {
        stop()
        try {
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
                    // 模块4：真实播放开始 → onPlaybackStateChanged(true)
                    notifyPlaybackState(true)
                }
                setOnCompletionListener {
                    tempFile.delete()
                    // 模块4：自然播放完成 → onPlaybackStateChanged(false)
                    notifyPlaybackState(false)
                }
                setOnErrorListener { _, what, extra ->
                    android.util.Log.e("EdgeTtsService", "MediaPlayer 错误: what=$what, extra=$extra")
                    tempFile.delete()
                    // 模块4：播放中断 → onPlaybackStateChanged(false)
                    notifyPlaybackState(false)
                    true
                }
                prepareAsync()
            }
        } catch (e: Exception) {
            android.util.Log.e("EdgeTtsService", "播放音频失败: ${e.message}")
            // 模块4：播放未开始即失败 → 状态归零（幂等，未播放时是 no-op）
            notifyPlaybackState(false)
        }
    }

    /**
     * 使用 Android 系统 TTS 播放文本
     * 作为 Edge TTS 失败时的降级方案
     */
    private suspend fun speakAndroid(text: String) = suspendCancellableCoroutine<Unit> { cont ->
        if (!androidTtsReady) {
            android.util.Log.w("EdgeTtsService", "Android TTS 未就绪，跳过")
            cont.resume(Unit)
            return@suspendCancellableCoroutine
        }
        // 先注册监听再 speak：避免 onStart 在监听器设置前触发而漏掉播放开始状态
        androidTts?.setOnUtteranceProgressListener(object : UtteranceProgressListener() {
            override fun onStart(utteranceId: String?) {
                // 模块4：Android TTS 开始发声 → onPlaybackStateChanged(true)
                notifyPlaybackState(true)
            }
            override fun onDone(utteranceId: String?) {
                // 模块4：播报完成 → onPlaybackStateChanged(false)
                notifyPlaybackState(false)
                if (!cont.isCompleted) cont.resume(Unit)
            }
            override fun onError(utteranceId: String?) {
                android.util.Log.w("EdgeTtsService", "Android TTS 播放错误")
                // 模块4：播报中断 → onPlaybackStateChanged(false)
                notifyPlaybackState(false)
                if (!cont.isCompleted) cont.resume(Unit)
            }
        })
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
            androidTts?.speak(text, TextToSpeech.QUEUE_FLUSH, null, "utterance_id")
        } else {
            @Suppress("DEPRECATION")
            androidTts?.speak(text, TextToSpeech.QUEUE_FLUSH, null)
        }
    }

    /**
     * 停止当前播放
     */
    fun stop() {
        val wasPlaying = playbackState
        mediaPlayer?.apply {
            if (isPlaying) stop()
            release()
        }
        mediaPlayer = null
        androidTts?.stop()
        // 模块4：主动打断（stopSpeaking）→ 播放状态归零；未在播放时是 no-op
        if (wasPlaying) notifyPlaybackState(false)
    }

    /**
     * 释放所有资源
     */
    fun release() {
        stop()
        androidTts?.shutdown()
        androidTts = null
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
