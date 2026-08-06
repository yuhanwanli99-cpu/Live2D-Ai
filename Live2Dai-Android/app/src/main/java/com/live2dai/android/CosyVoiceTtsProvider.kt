package com.live2dai.android

import android.content.Context
import android.media.AudioAttributes
import android.media.MediaPlayer
import android.util.Log
import com.live2dai.android.data.SettingsRepository
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okio.ByteString
import org.json.JSONObject
import java.io.File
import java.io.FileOutputStream
import java.util.UUID
import java.util.concurrent.ConcurrentLinkedQueue
import java.util.concurrent.TimeUnit

/**
 * CosyVoice 云端 TTS Provider（V1.2 模块2：阿里云 DashScope CosyVoice WebSocket 流式）。
 *
 * 契约见 docs/plan-v12-plan.schema.json → apiEndpoints / providersV12.cosyvoice_ws：
 * - 端点 `wss://dashscope.aliyuncs.com/api-ws/v1/inference`（标准公共端点；专属服务用户可经
 *   SettingsRepository.cosyvoiceWsUrl / 构造参数 wsUrl 覆盖为 `wss://{专属host}/api-ws/v1/inference`），
 *   请求头 `Authorization: Bearer {apiKey}`
 * - 事件流（官方 SDK run-task 三事件协议，与 PC 端 cosyvoice_cloud_tts.py 同协议）：
 *   1. run-task（input 空，parameters 含 voice/volume/sample_rate/rate/format/pitch/seed/type）
 *   2. continue-task（input.text = 待合成文本）
 *   3. finish-task（input 空，结束会话）
 *   服务端响应：task-started → 二进制音频帧（MP3）* → task-finished | task-failed
 * - 无官方 Android SDK，OkHttp WebSocket 直连（协议格式对照本机安装的官方 dashscope
 *   SDK speech_synthesizer.py 的 get_start_request/get_continue_request/get_finish_request）
 *
 * 降级链位置（plan §3.1.6）：Android Tier 1 (priority=0)，失败 → edge/system。
 * API Key 未配置时 [speak] 快速失败（health=UNAVAILABLE，不发网络），由
 * VoiceIoController 降级链继续尝试下一个 Provider——不阻塞、不等待网络超时。
 *
 * 日志规范（plan §5.7.4）：连接/合成/播放/失败均为 Log.i/W/E 级别
 * （项目教训：国产 ROM 抑制 Log.d），TAG="CosyVoice"。
 */
class CosyVoiceTtsProvider(
    private val context: Context,
    /** 阿里云 DashScope API Key（来自 SettingsRepository.aliyunApiKey）；空 = 未配置 */
    private val apiKey: String,
    /** 默认音色（schema 契约 cosyvoice-v1 默认音色） */
    private val voice: String = DEFAULT_VOICE,
    /** DashScope 语音合成模型 */
    private val model: String = DEFAULT_MODEL,
    /** DashScope CosyVoice WebSocket 端点（空/空白 = 使用默认公共端点 DASHSCOPE_WS_URL；
     *  专属服务用户可传 `wss://{专属host}/api-ws/v1/inference`；测试可注入 MockWebServer 地址） */
    private val wsUrl: String = DASHSCOPE_WS_URL,
    /** OkHttpClient（测试可注入 mock 验证不发网络） */
    private val client: OkHttpClient = defaultOkHttpClient(),
    /** 单次合成总超时（连接 + 接收音频；超出抛异常触发降级） */
    private val synthesisTimeoutMs: Long = DEFAULT_SYNTHESIS_TIMEOUT_MS,
    /**
     * 猫娘语气 instruction（语气/方言/情绪描述，SDK 限制 ≤100 字符；
     * run-task parameters.instruction）。null = 构造时从 SettingsRepository 读取用户配置
     * （默认「用可爱的猫娘萝莉语气说话，声音甜美带一点傲娇」）；测试可显式注入断言 JSON。
     */
    private val instruction: String? = null,
    /** 语速倍率（run-task parameters.rate，默认 1.2 略快有活力感）；null = 从 SettingsRepository 读取 */
    private val rate: Float? = null,
    /** 音调倍率（run-task parameters.pitch，默认 1.1 升高显得更萌）；null = 从 SettingsRepository 读取 */
    private val pitch: Float? = null,
) : TtsProvider {

    companion object {
        private const val TAG = "CosyVoice"

        /** Provider 唯一标识（契约 schema providersV12.cosyvoice_ws） */
        const val PROVIDER_ID = "cosyvoice_ws"

        /** DashScope CosyVoice 标准公共 WebSocket 端点（schema 契约；中国大陆直连）。
         *  注意：不是 /api-ws/v1/realtime（那是旧版 start-synthesis 协议）；
         *  正确协议是 run-task 三事件，对应 /api-ws/v1/inference 端点。 */
        const val DASHSCOPE_WS_URL = "wss://dashscope.aliyuncs.com/api-ws/v1/inference"

        // run-task 三事件协议常量（对照官方 SDK dashscope/protocol/websocket.py）
        private const val ACTION_RUN_TASK = "run-task"
        private const val ACTION_CONTINUE_TASK = "continue-task"
        private const val ACTION_FINISH_TASK = "finish-task"
        private const val EVENT_TASK_STARTED = "task-started"
        private const val EVENT_TASK_FINISHED = "task-finished"
        private const val EVENT_TASK_FAILED = "task-failed"
        private const val STREAMING_MODE = "duplex"

        private const val DEFAULT_VOICE = "longxiaochun"
        private const val DEFAULT_MODEL = "cosyvoice-v1"
        private const val DEFAULT_SYNTHESIS_TIMEOUT_MS = 60_000L
        private const val CONNECT_TIMEOUT_SECONDS = 15L
        private const val READ_TIMEOUT_SECONDS = 60L

        // run-task parameters（对照 SDK Request 默认值与实测 143KB MP3 参数）
        private const val PARAM_VOLUME = 50
        private const val PARAM_TEXT_TYPE = "PlainText"
        private const val PARAM_SAMPLE_RATE = 24000
        private const val PARAM_FORMAT = "mp3"
        private const val PARAM_SEED = 0
        private const val PARAM_TYPE = 0

        private fun defaultOkHttpClient(): OkHttpClient = OkHttpClient.Builder()
            .connectTimeout(CONNECT_TIMEOUT_SECONDS, TimeUnit.SECONDS)
            .readTimeout(READ_TIMEOUT_SECONDS, TimeUnit.SECONDS)
            .writeTimeout(CONNECT_TIMEOUT_SECONDS, TimeUnit.SECONDS)
            .build()
    }

    /** 解析后的端点：构造参数 wsUrl 空白时回退默认公共端点（SettingsRepository 默认空串）。 */
    private val resolvedWsUrl: String = wsUrl.ifBlank { DASHSCOPE_WS_URL }

    // 个性化参数（级2-C）：显式构造参数优先（测试注入/调用方覆盖），
    // 否则构造时从 SettingsRepository 读取（默认 instruction=猫娘语气、rate=1.2、pitch=1.1）。
    // SharedPreferences 按 name 单例，重复创建 SettingsRepository 无额外开销。
    private val resolvedInstruction: String = instruction ?: SettingsRepository(context).cosyvoiceInstruction
    private val resolvedRate: Float = rate ?: SettingsRepository(context).cosyvoiceRate
    private val resolvedPitch: Float = pitch ?: SettingsRepository(context).cosyvoicePitch

    private val _health = MutableStateFlow(
        if (apiKey.isBlank()) TtsHealth.DEGRADED else TtsHealth.OK
    )

    override val info = TtsProviderInfo(
        id = PROVIDER_ID,
        name = "CosyVoice 云端",
        category = TtsProviderCategory.CLOUD_WS,
        requiresNetwork = true,
        health = _health,
        // Android 降级链 Tier 1（plan §3.1.6）：priority=0 最高，注册后 cascade 自动排最前
        priority = 0,
        configurable = true,
    )

    private val _isSpeaking = MutableStateFlow(false)
    override val isSpeaking: StateFlow<Boolean> = _isSpeaking.asStateFlow()

    private var mediaPlayer: MediaPlayer? = null

    /**
     * 语音合成 + 播放（TtsProvider 契约）。
     *
     * 1. API Key 未配置 → health=UNAVAILABLE 快速失败（不发网络，不阻塞降级链）
     * 2. DashScope WebSocket 流式合成（run-task → task-started → continue-task →
     *    二进制 MP3 帧 → finish-task → task-finished）
     * 3. 音频字节 → 临时文件 → MediaPlayer 播放（异步，prepareAsync 后返回，
     *    播放状态通过 [isSpeaking] StateFlow 暴露给 LipSync/UI）
     *
     * 任何失败向上抛异常（由 VoiceIoController 降级链捕获继续尝试下一个 Provider），
     * 不在此处内部降级（plan §3.1.3）。
     */
    override suspend fun speak(text: String) = withContext(Dispatchers.IO) {
        if (apiKey.isBlank()) {
            Log.w(TAG, "API Key 未配置，快速失败（不发网络），降级链继续")
            _health.value = TtsHealth.UNAVAILABLE
            throw IllegalStateException("CosyVoice API Key 未配置（设置页 aliyunApiKey）")
        }
        try {
            val audioBytes = synthesize(text)
            if (audioBytes.isNotEmpty()) {
                playAudioBytes(audioBytes)
                _health.value = TtsHealth.OK
            } else {
                throw IllegalStateException("CosyVoice 未返回音频数据")
            }
        } catch (e: Exception) {
            _health.value = TtsHealth.DEGRADED
            throw e
        }
    }

    /**
     * DashScope CosyVoice WebSocket 合成（官方 SDK run-task 三事件协议）：
     *
     * 1. WS 连接（Authorization: Bearer {apiKey}）
     * 2. 发送 run-task（input 空，parameters 含 voice/volume/sample_rate/rate/format/pitch/seed/type）
     * 3. 收到 task-started → 发送 continue-task（input.text=待合成文本）+ finish-task（结束会话）
     *    —— 与官方 SDK `call()` 一致：continue-task 后立即 finish-task，服务端随后
     *    流式返回全部 MP3 音频帧并以 task-finished 收尾（SDK streaming_complete 语义）
     * 4. 接收二进制音频帧（MP3）合并
     * 5. task-finished → 完成返回音频字节；task-failed → 抛异常；超时 → 抛异常
     *
     * @throws Exception 连接失败 / task-failed / 超时 / 未返回音频（由降级链处理）
     */
    private fun synthesize(text: String): ByteArray {
        val audioParts = ConcurrentLinkedQueue<ByteArray>()
        var done = false
        var error: Exception? = null
        val taskId = UUID.randomUUID().toString().replace("-", "")

        val wsRequest = Request.Builder()
            .url(resolvedWsUrl)
            .addHeader("Authorization", "Bearer $apiKey")
            .build()

        val ws = client.newWebSocket(wsRequest, object : WebSocketListener() {
            override fun onOpen(ws: WebSocket, response: Response) {
                Log.i(TAG, "DashScope WebSocket 已连接，发送 run-task（model=$model, voice=$voice）")
                sendRunTask(ws, taskId)
            }

            override fun onMessage(ws: WebSocket, message: String) {
                // JSON 控制事件：task-started → 发送文本；task-finished → 完成；
                // task-failed/error → 失败；未知事件仅记日志不中断（宽松匹配，
                // 参考 PC 端 cosyvoice_cloud_tts.py 与官方 SDK on_message）
                // 注意：参数名 message 而非 text——text 是待合成文本，不能与回调参数混淆
                val eventName = try {
                    JSONObject(message).optJSONObject("header")?.optString("event", "") ?: ""
                } catch (e: Exception) {
                    Log.w(TAG, "收到无法解析的 JSON 消息: ${message.take(120)}")
                    ""
                }
                when (eventName) {
                    EVENT_TASK_STARTED -> {
                        Log.i(TAG, "DashScope task-started，发送 continue-task（文本）+ finish-task（结束）")
                        sendContinueTask(ws, taskId, text)
                        sendFinishTask(ws, taskId)
                    }
                    EVENT_TASK_FINISHED -> {
                        Log.i(TAG, "DashScope task-finished")
                        done = true
                    }
                    EVENT_TASK_FAILED, "error" -> {
                        val payload = try {
                            JSONObject(message).optJSONObject("payload")
                        } catch (e: Exception) {
                            null
                        }
                        val errMsg = payload?.optString("message")
                            ?: payload?.optString("error")
                            ?: payload?.optString("code")
                            ?: ""
                        Log.e(TAG, "DashScope 合成失败（$eventName）: $errMsg")
                        error = RuntimeException("DashScope 合成失败（$eventName）: $errMsg")
                        done = true
                    }
                    "result-generated" -> Log.i(TAG, "DashScope result-generated")
                    else -> Log.i(TAG, "DashScope 忽略未知事件: $eventName")
                }
            }

            override fun onMessage(ws: WebSocket, bytes: ByteString) {
                audioParts.add(bytes.toByteArray())
            }

            override fun onClosing(ws: WebSocket, code: Int, reason: String) {
                Log.i(TAG, "DashScope WebSocket 关闭中: code=$code, reason=$reason")
                done = true
                ws.close(1000, null)
            }

            override fun onClosed(ws: WebSocket, code: Int, reason: String) {
                Log.i(TAG, "DashScope WebSocket 已关闭")
                done = true
            }

            override fun onFailure(ws: WebSocket, t: Throwable, response: Response?) {
                Log.e(TAG, "DashScope WebSocket 连接失败: ${t.message}")
                error = t as? Exception ?: RuntimeException(t)
                done = true
            }
        })

        // 等待完成或超时（与 EdgeTtsProvider 同步等待模式一致）
        val start = System.currentTimeMillis()
        while (!done && error == null && System.currentTimeMillis() - start < synthesisTimeoutMs) {
            Thread.sleep(50)
        }

        // 无论正常结束还是超时：立即断开连接（cancel 不做 close 握手——服务端在
        // task-finished 后本就会自行关闭，此处确保本方连接立即释放，不留半开连接）
        if (!done && error == null) {
            Log.w(TAG, "DashScope WebSocket 合成超时（${synthesisTimeoutMs}ms），强制断开")
        }
        ws.cancel()

        if (error != null) throw error!!

        if (audioParts.isEmpty()) {
            Log.w(TAG, "DashScope 连接正常关闭但未收到音频数据")
            return ByteArray(0)
        }

        // 合并所有音频分片
        val totalSize = audioParts.sumOf { it.size }
        val result = ByteArray(totalSize)
        var offset = 0
        for (part in audioParts) {
            System.arraycopy(part, 0, result, offset, part.size)
            offset += part.size
        }
        Log.i(TAG, "合成完成: ${audioParts.size} 个音频分片，共 ${result.size} bytes")
        return result
    }

    /**
     * 发送 run-task 事件（会话开始，input 空；parameters 携带合成参数）。
     * 对照官方 SDK Request.get_start_request。
     * 级2-C：parameters 增加 instruction（猫娘语气）/rate（语速）/pitch（音调）个性化参数，
     * 与官方 dashscope SDK speech_synthesizer.py（instruction/rate/pitch 均支持）对齐。
     */
    private fun sendRunTask(ws: WebSocket, taskId: String) {
        val header = JSONObject().apply {
            put("action", ACTION_RUN_TASK)
            put("task_id", taskId)
            put("streaming", STREAMING_MODE)
        }
        val parameters = JSONObject().apply {
            put("voice", voice)
            put("volume", PARAM_VOLUME)
            put("text_type", PARAM_TEXT_TYPE)
            put("sample_rate", PARAM_SAMPLE_RATE)
            put("rate", resolvedRate)
            put("format", PARAM_FORMAT)
            put("pitch", resolvedPitch)
            put("seed", PARAM_SEED)
            put("type", PARAM_TYPE)
            put("instruction", resolvedInstruction)
        }
        val payload = JSONObject().apply {
            put("model", model)
            put("task_group", "audio")
            put("task", "tts")
            put("function", "SpeechSynthesizer")
            put("input", JSONObject())
            put("parameters", parameters)
        }
        val event = JSONObject().apply {
            put("header", header)
            put("payload", payload)
        }
        ws.send(event.toString())
    }

    /**
     * 发送 continue-task 事件（携带待合成文本）。对照官方 SDK Request.get_continue_request。
     */
    private fun sendContinueTask(ws: WebSocket, taskId: String, text: String) {
        val header = JSONObject().apply {
            put("action", ACTION_CONTINUE_TASK)
            put("task_id", taskId)
            put("streaming", STREAMING_MODE)
        }
        val payload = JSONObject().apply {
            put("model", model)
            put("task_group", "audio")
            put("task", "tts")
            put("function", "SpeechSynthesizer")
            put("input", JSONObject().put("text", text))
        }
        val event = JSONObject().apply {
            put("header", header)
            put("payload", payload)
        }
        ws.send(event.toString())
    }

    /**
     * 发送 finish-task 事件（结束会话，input 空）。对照官方 SDK Request.get_finish_request。
     */
    private fun sendFinishTask(ws: WebSocket, taskId: String) {
        val header = JSONObject().apply {
            put("action", ACTION_FINISH_TASK)
            put("task_id", taskId)
            put("streaming", STREAMING_MODE)
        }
        val payload = JSONObject().apply {
            put("input", JSONObject())
        }
        val event = JSONObject().apply {
            put("header", header)
            put("payload", payload)
        }
        ws.send(event.toString())
    }

    /**
     * 播放 MP3 音频字节数据（复用 EdgeTtsProvider 的播放模式）：
     * 写缓存临时文件 → MediaPlayer.prepareAsync 异步播放；播放状态经 [isSpeaking] 暴露。
     *
     * 同步阶段（写文件 / setDataSource）失败向上抛 → speak 抛异常触发降级；
     * 异步阶段（prepareAsync 后）失败通过 onError 置 [isSpeaking]=false（此阶段
     * speak 已返回，与 EdgeTtsProvider 行为一致）。
     */
    private fun playAudioBytes(audioBytes: ByteArray) {
        stop()
        val tempFile = File(context.cacheDir, "cosyvoice_tts_temp.mp3")
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
                Log.i(TAG, "音频就绪，开始播放")
                start()
                // 真实播放开始 → isSpeaking(true)（LipSync 驱动）
                _isSpeaking.value = true
            }
            setOnCompletionListener {
                Log.i(TAG, "播放完成")
                tempFile.delete()
                // 自然播放完成 → isSpeaking(false)
                _isSpeaking.value = false
            }
            setOnErrorListener { _, what, extra ->
                Log.e(TAG, "MediaPlayer 播放错误: what=$what, extra=$extra")
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
     * 释放所有资源（幂等）。WebSocket 每次合成即建即断，OkHttpClient 随 Provider
     * 生命周期；此处仅停止播放（与 EdgeTtsProvider.release 一致）。
     */
    override fun release() {
        stop()
    }
}
