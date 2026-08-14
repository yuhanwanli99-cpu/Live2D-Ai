package com.live2dai.android

import android.content.Context
import android.media.AudioAttributes
import android.media.MediaCodec
import android.media.MediaExtractor
import android.media.MediaFormat
import android.media.MediaPlayer
import android.util.Log
import com.live2dai.android.data.SettingsRepository
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
 *   task-started → result-generated（JSON 字符串消息，音频块在
 *   payload.output.audio，base64 编码 MP3）* → task-finished | task-failed
 *   （真机协议实证：音频走 JSON 字符串消息而非二进制；二进制消息分支保留为
 *   协议变体兜底——若未来协议变化仍可用）
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
     * 语气 instruction（语气/方言/情绪描述，SDK 限制 ≤100 字符；
     * run-task parameters.instruction）。null = 构造时从 SettingsRepository 读取用户配置
     * （默认「用清澈甜美的少女音说话，声音甜美带一点傲娇」）；测试可显式注入断言 JSON。
     */
    private val instruction: String? = null,
    /** 语速倍率（run-task parameters.rate，默认 1.2 略快有活力感）；null = 从 SettingsRepository 读取 */
    private val rate: Float? = null,
    /** 音调倍率（run-task parameters.pitch，默认 1.1 升高显得更萌）；null = 从 SettingsRepository 读取 */
    private val pitch: Float? = null,
    /** M-AND-1 测试注入：MP3→PCM 解码器（默认 [decodeMp3ToPcm] MediaExtractor+MediaCodec；JVM 测试注入 fake）。 */
    private val pcmDecoder: (ByteArray) -> DecodedAudio? = ::decodeMp3ToPcm,
    /** M-AND-1 测试注入：播放位置轮询作用域（默认 Main；测试注入 TestDispatcher 驱动虚拟时间）。 */
    private val positionPollScope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate),
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
    // 否则构造时从 SettingsRepository 读取（默认 instruction=清澈甜美少女音、rate=1.2、pitch=1.1）。
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

    /**
     * M-AND-1（2a-①）：MP3 整段预解码 → PCM 帧流（150ms 固定分块，播放前全量 emit）。
     * 时序契约（AND1-A7）：消费者在 speak() 前启动 collect；speak 后 1s 内首帧。
     * tryEmit 非阻塞：无订阅者/缓冲满 → 丢块 + Log.w 一次，不影响 speak 主流程。
     */
    private val _audioFrameFlow = MutableSharedFlow<AudioFrame>(
        replay = 0,
        extraBufferCapacity = 64,
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

    /**
     * M-AND-1 C1 修复（cosyvoice_ws WS 流式路径）：本次 speak 会话内 WS 流式路径
     * 已 emit 的帧数（音频块到达时增量 emit；>0 时 speak() 跳过整段回退 emit 防重复）。
     * 下次 speak 重置。OkHttp 回调线程写、speak IO 线程读 → @Volatile。
     */
    @Volatile
    private var wsStreamEmittedFrames = 0L

    private var mediaPlayer: MediaPlayer? = null

    /**
     * 语音合成 + 播放（TtsProvider 契约）。
     *
     * 1. API Key 未配置 → health=UNAVAILABLE 快速失败（不发网络，不阻塞降级链）
     * 2. DashScope WebSocket 流式合成（run-task → task-started → continue-task →
     *    result-generated 音频块（JSON output.audio base64 MP3）→ finish-task →
     *    task-finished）
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
            // M-AND-1：新 speak 会话重置 emit 守卫
            stopped = false
            dropWarned = false
            wsStreamEmittedFrames = 0L
            val audioBytes = synthesize(text)
            if (audioBytes.isNotEmpty()) {
                // M-AND-1 C1 修复：WS 流式路径已按到达块实时 emit 帧（真机路径）→
                // 不重复整段 emit；流式路径零帧（JVM 测试/解码前缀未完整）→ 回退
                // 整段预解码 emit（既有 2a-① 行为，AND1-A1/A2/A7 语义不变）
                if (wsStreamEmittedFrames == 0L) {
                    // [C1-r3 诊断] 流式路径零帧时的回退决策打点（真机验证流式 emit 是否产出）
                    Log.w(TAG, "CosyVoice WS 流式路径零帧（wsStreamEmittedFrames=0），回退整段预解码 emit")
                    emitAudioFrames(audioBytes)
                } else {
                    Log.i(TAG, "CosyVoice WS 流式路径已 emit $wsStreamEmittedFrames 帧，跳过整段回退（防重复）")
                }
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
     * 4. 接收音频块（真机实证：result-generated JSON 字符串消息 payload.output.audio
     *    的 base64 MP3 分块；二进制消息为协议变体兜底）合并
     * 5. task-finished → 完成返回音频字节；task-failed → 抛异常；超时 → 抛异常
     *
     * @throws Exception 连接失败 / task-failed / 超时 / 未返回音频（由降级链处理）
     */
    private fun synthesize(text: String): ByteArray {
        val audioParts = ConcurrentLinkedQueue<ByteArray>()
        var done = false
        var error: Exception? = null
        val taskId = UUID.randomUUID().toString().replace("-", "")
        // M-AND-1 C1 修复（cosyvoice_ws WS 流式路径）：累积已到达的 MP3 字节前缀，
        // 每个音频块到达时增量解码 → 150ms 帧实时 emit（不等待整段合成结束）。
        // 仅 OkHttp 回调线程写（WebSocketListener 串行回调），无需加锁。
        val mp3Accumulator = java.io.ByteArrayOutputStream()

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
                    "result-generated" -> {
                        // [WSDBG] 临时诊断日志（C1-r3，真机验证后由主代理安排降噪）：
                        // 打印消息体前 200 字符，供真机抓取真实 JSON 结构、验证
                        // payload.output.audio 字段名假设（r2 该假设无原始 JSON 证据）。
                        Log.w(TAG, "[WSDBG] result-generated 消息体前200字符: ${message.take(200)}")
                        // M-AND-1 C1 修复轮2（真机协议实证）：DashScope 的音频块不在二进制
                        // 消息，而在 result-generated JSON 字符串消息的 payload.output.audio
                        // 字段（base64 编码 MP3 分块）——修复轮1 挂在 onMessage(bytes) 二进制
                        // 分支导致真机零帧。此处解析 → base64 解码 → 复用流式增量 emit
                        // （emitStreamingFrames：mp3Accumulator + pcmDecoder + chunk 150ms）。
                        val audioB64 = try {
                            JSONObject(message)
                                .optJSONObject("payload")
                                ?.optJSONObject("output")
                                ?.optString("audio", "")
                                ?.takeIf { it.isNotEmpty() }
                                // 协议变体防御：部分服务端把 output 直接放顶层
                                ?: JSONObject(message)
                                    .optJSONObject("output")
                                    ?.optString("audio", "")
                        } catch (e: Exception) {
                            null
                        }
                        if (audioB64.isNullOrEmpty()) {
                            // [WSDBG] 字段名验证失败分支：打印实际 payload/output 键集合，
                            // 直接回答「真实字段名是什么」（r2 假设 payload.output.audio）。
                            val payloadKeys = try {
                                JSONObject(message).optJSONObject("payload")?.keys()?.asSequence()?.joinToString(",") ?: "null"
                            } catch (e: Exception) {
                                "parse-error"
                            }
                            val outputObj = try {
                                JSONObject(message).optJSONObject("payload")?.optJSONObject("output")
                                    ?: JSONObject(message).optJSONObject("output")
                            } catch (e: Exception) {
                                null
                            }
                            val outputKeys = if (outputObj == null) "null" else outputObj.keys().asSequence().joinToString(",")
                            Log.w(TAG, "[WSDBG] result-generated 无 payload.output.audio 字段（payloadKeys=[$payloadKeys], outputKeys=[$outputKeys]）")
                            Log.i(TAG, "DashScope result-generated（无音频块，跳过）")
                        } else {
                            val part = try {
                                java.util.Base64.getDecoder().decode(audioB64)
                            } catch (e: Exception) {
                                null
                            }
                            if (part == null || part.isEmpty()) {
                                Log.w(TAG, "DashScope result-generated audio base64 解码失败，跳过该块")
                            } else {
                                Log.i(TAG, "DashScope result-generated 音频块: ${part.size} bytes")
                                audioParts.add(part)
                                emitStreamingFrames(part, mp3Accumulator)
                            }
                        }
                    }
                    else -> Log.i(TAG, "DashScope 忽略未知事件: $eventName")
                }
            }

            override fun onMessage(ws: WebSocket, bytes: ByteString) {
                val part = bytes.toByteArray()
                audioParts.add(part)
                // M-AND-1 C1 修复：WS 流式路径按到达块 emit 帧（增量解码已累积前缀）
                emitStreamingFrames(part, mp3Accumulator)
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
     * 级2-C：parameters 增加 instruction（语气）/rate（语速）/pitch（音调）个性化参数，
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
                // M-AND-1（3-B）：prepared→start 间恒 0；start 后轮询非递减
                playbackClock.onPrepared()
                start()
                // 真实播放开始 → isSpeaking(true)（LipSync 驱动）
                _isSpeaking.value = true
                playbackClock.onStart()
            }
            setOnCompletionListener {
                Log.i(TAG, "播放完成")
                playbackClock.onStop()
                tempFile.delete()
                // 自然播放完成 → isSpeaking(false)
                _isSpeaking.value = false
            }
            setOnErrorListener { _, what, extra ->
                Log.e(TAG, "MediaPlayer 播放错误: what=$what, extra=$extra")
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
     * M-AND-1 C1 修复（cosyvoice_ws WS 流式路径）：音频块实时到达 → 把已累积的 MP3
     * 前缀增量解码 → [LipSyncFrameChunker] 150ms 分块 → 只 emit 尚未发过的帧
     * （timestampMs 相对 speak 起点、首帧 0、严格非递减——AND1-A2/A7 语义）。
     *
     * - 前缀未完整（解码 null/空）→ 跳过，等下一块到达再试；最终由 speak() 回退
     *   [emitAudioFrames] 整段预解码兜底（wsStreamEmittedFrames==0 时）。
     * - stop() 后（stopped=true）不再 emit（守卫在方法首行）。
     * - tryEmit 非阻塞：无订阅者/缓冲满 → 丢块 + Log.w 一次，不影响 WS 主流程。
     * - 仅 OkHttp 回调线程调用（WebSocketListener 串行），mp3Accumulator 无并发写。
     */
    private fun emitStreamingFrames(
        part: ByteArray,
        mp3Accumulator: java.io.ByteArrayOutputStream,
    ) {
        if (stopped) return
        mp3Accumulator.write(part)
        val accumulatedSize = mp3Accumulator.size()
        val decoded = try {
            pcmDecoder(mp3Accumulator.toByteArray())
        } catch (e: Exception) {
            Log.w(TAG, "CosyVoice WS 流式解码异常（累积 ${accumulatedSize}B，块 ${part.size}B）: ${e.message}")
            null
        }
        if (decoded == null) {
            // [C1-r3 诊断] 原静默路径：decodeMp3ToPcm 内部吞异常返回 null（MediaExtractor 未就绪 /
            // MediaCodec 解码失败 / 前缀未完整）——区分不了真机失败原因，现逐块打点含字节数。
            Log.w(TAG, "CosyVoice WS 流式解码返回 null（累积 ${accumulatedSize}B，块 ${part.size}B）——前缀未完整或 MediaCodec 失败，等下一块重试")
            return
        }
        if (decoded.samples.isEmpty()) {
            Log.w(TAG, "CosyVoice WS 流式解码输出空 PCM（累积 ${accumulatedSize}B，块 ${part.size}B）")
            return
        }
        if (decoded.sampleRate <= 0) {
            Log.w(TAG, "CosyVoice WS 流式解码输出非法采样率 ${decoded.sampleRate}（累积 ${accumulatedSize}B）")
            return
        }
        val frames = LipSyncFrameChunker.chunk(decoded.samples, decoded.sampleRate)
        var dropped = false
        while (wsStreamEmittedFrames < frames.size) {
            val frame = frames[wsStreamEmittedFrames.toInt()]
            if (!_audioFrameFlow.tryEmit(frame)) dropped = true
            wsStreamEmittedFrames++
        }
        // [C1-r3 诊断] 成功路径打点：确认流式增量解码/发帧链路在真机确实工作（含累积字节数与帧数）
        Log.i(TAG, "CosyVoice WS 流式解码成功: ${decoded.samples.size} samples @${decoded.sampleRate}Hz（累积 ${accumulatedSize}B），本次共 ${frames.size} 帧，已 emit ${wsStreamEmittedFrames} 帧")
        if (dropped && !dropWarned) {
            Log.w(TAG, "CosyVoice WS 流式 audioFrameFlow 无订阅者或缓冲满，丢弃部分音频块（不影响播放）")
            dropWarned = true
        }
    }

    /**
     * M-AND-1（2a-①，C1 后为 WS 流式路径零帧时的回退）：MP3 → PCM 整段预解码 →
     * 150ms 分块 emit（播放前全量；非阻塞 tryEmit）。解码失败（真机异常 / JVM 无
     * MediaCodec）→ 无帧流 + Log.w，不影响播放路径。
     */
    private fun emitAudioFrames(mp3Bytes: ByteArray) {
        if (stopped) return
        val decoded = try {
            pcmDecoder(mp3Bytes)
        } catch (e: Exception) {
            Log.w(TAG, "CosyVoice MP3 解码异常，无帧流（不影响播放）: ${e.message}")
            null
        }
        if (decoded == null || decoded.samples.isEmpty() || decoded.sampleRate <= 0) {
            Log.w(TAG, "CosyVoice MP3 解码失败/空 PCM（${mp3Bytes.size}B），无帧流（不影响播放）")
            return
        }
        val frames = LipSyncFrameChunker.chunk(decoded.samples, decoded.sampleRate)
        var dropped = false
        for (frame in frames) {
            if (!_audioFrameFlow.tryEmit(frame)) dropped = true
        }
        // [C1-r3 诊断] 回退整段预解码成功打点（区分「流式失败→回退成功」与「回退也失败」）
        Log.i(TAG, "CosyVoice 整段预解码回退成功: ${decoded.samples.size} samples @${decoded.sampleRate}Hz，emit ${frames.size} 帧")
        if (dropped && !dropWarned) {
            Log.w(TAG, "CosyVoice audioFrameFlow 无订阅者或缓冲满，丢弃部分音频块（不影响播放）")
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
     * 释放所有资源（幂等）。WebSocket 每次合成即建即断，OkHttpClient 随 Provider
     * 生命周期；此处仅停止播放（与 EdgeTtsProvider.release 一致）。
     */
    override fun release() {
        stop()
    }
}
