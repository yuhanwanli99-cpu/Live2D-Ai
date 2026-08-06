package com.live2dai.android

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.live2dai.android.data.SettingsRepository
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestCoroutineScheduler
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okio.ByteString.Companion.toByteString
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.mockito.kotlin.any
import org.mockito.kotlin.mock
import org.mockito.kotlin.never
import org.mockito.kotlin.verify
import org.mockito.kotlin.whenever
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowMediaPlayer
import org.robolectric.shadows.util.DataSource
import java.io.File
import java.util.concurrent.atomic.AtomicReference

/**
 * CosyVoiceTtsProvider（V1.2 模块2：阿里云 DashScope CosyVoice WebSocket 流式 TTS）测试。
 *
 * 契约见 docs/plan-v12-plan.schema.json → providersV12.cosyvoice_ws / apiEndpoints：
 * - id="cosyvoice_ws" / CLOUD_WS / requiresNetwork / priority=0（降级链第一位）/ configurable
 * - API Key 缺失 → 快速失败（不发网络，health=UNAVAILABLE，降级链继续）
 * - WebSocket 协议：run-task(parameters) → task-started → continue-task(text) →
 *   二进制音频帧(MP3) → finish-task → task-finished（官方 SDK 三事件协议，
 *   与 PC 端同协议；用 MockWebServer WebSocket 升级模拟 DashScope 服务端）
 * - 播放状态：isSpeaking 随 MediaPlayer prepared/completion 切换
 * - 注册后 cascade() 第一位是 cosyvoice_ws（降级链集成）
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class, sdk = [34])
class CosyVoiceTtsProviderTest {

    private lateinit var context: Context

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        TtsProviderRegistry.clear()
        ShadowMediaPlayer.setCreateListener(null)
    }

    @After
    fun tearDown() {
        TtsProviderRegistry.clear()
        ShadowMediaPlayer.setCreateListener(null)
    }

    // ─────────────────────────────────────────────
    // TtsProvider 契约（schema providersV12.cosyvoice_ws）
    // ─────────────────────────────────────────────

    @Test
    fun infoMatchesContract() {
        val provider = CosyVoiceTtsProvider(context, apiKey = "sk-test")
        assertEquals("id 应为 cosyvoice_ws", "cosyvoice_ws", provider.info.id)
        assertEquals("name 应为 CosyVoice 云端", "CosyVoice 云端", provider.info.name)
        assertEquals(
            "分类应为 CLOUD_WS（DashScope CosyVoice WebSocket）",
            TtsProviderCategory.CLOUD_WS,
            provider.info.category
        )
        assertTrue("CosyVoice 云端需要网络", provider.info.requiresNetwork)
        assertEquals("降级链 priority=0（最高，排最前）", 0, provider.info.priority)
        assertTrue("CosyVoice 有用户可配置参数（API Key）", provider.info.configurable)
        assertNotNull("health StateFlow 必须存在", provider.info.health)
        provider.release()
    }

    @Test
    fun defaultEndpoint_isInferenceStandardEndpoint() {
        assertEquals(
            "默认端点应为 DashScope 标准 inference 端点（非 realtime）",
            "wss://dashscope.aliyuncs.com/api-ws/v1/inference",
            CosyVoiceTtsProvider.DASHSCOPE_WS_URL
        )
    }

    @Test
    fun health_initialDegradedWhenNoApiKey() {
        val provider = CosyVoiceTtsProvider(context, apiKey = "")
        assertEquals(
            "无 API Key 时初始 health 应为 DEGRADED（未配置但引擎可用性未知）",
            TtsHealth.DEGRADED,
            provider.info.health.value
        )
        provider.release()
    }

    @Test
    fun health_initialOkWhenApiKeyConfigured() {
        val provider = CosyVoiceTtsProvider(context, apiKey = "sk-test")
        assertEquals(
            "配置 API Key 时初始 health 应为 OK",
            TtsHealth.OK,
            provider.info.health.value
        )
        provider.release()
    }

    @Test
    fun isSpeakingStartsFalse() {
        val provider = CosyVoiceTtsProvider(context, apiKey = "sk-test")
        assertFalse("初始不应处于播放状态", provider.isSpeaking.value)
        provider.release()
    }

    // ─────────────────────────────────────────────
    // API Key 缺失 → 快速失败（不发网络，不阻塞降级链）
    // ─────────────────────────────────────────────

    @Test
    fun speakWithoutApiKey_fastFails_neverTouchesNetwork() = runTest {
        val client = mock<okhttp3.OkHttpClient>()
        val provider = CosyVoiceTtsProvider(context, apiKey = "", client = client)

        var failure: Exception? = null
        try {
            provider.speak("你好喵")
        } catch (e: Exception) {
            failure = e
        }

        assertNotNull("无 Key 时 speak 必须快速失败（抛异常供降级链捕获）", failure)
        assertTrue(
            "失败类型应为 IllegalStateException（配置缺失）",
            failure is IllegalStateException
        )
        assertEquals(
            "快速失败后 health 应为 UNAVAILABLE（降级链自动跳过）",
            TtsHealth.UNAVAILABLE,
            provider.info.health.value
        )
        assertFalse("快速失败不应进入播放状态", provider.isSpeaking.value)
        verify(client, never()).newWebSocket(any(), any())
        provider.release()
    }

    // ─────────────────────────────────────────────
    // 注册：priority=0 排降级链第一位
    // ─────────────────────────────────────────────

    @Test
    fun registerDefault_cosyvoiceFirstInCascade() {
        registerDefaultTtsProviders(context)
        val chain = TtsProviderRegistry.cascade()
        assertEquals(
            "注册后 cascade() 第一位应为 cosyvoice_ws（priority=0 自动前移）",
            "cosyvoice_ws",
            chain.first().info.id
        )
        assertEquals(
            "降级链顺序应为 cosyvoice → sherpa → edge → system（V1.2 模块3 sherpa 插入 Tier 2）",
            listOf("cosyvoice_ws", "sherpa_onnx_kokoro", "edge_tts", "system_tts"),
            chain.map { it.info.id }
        )
        // 释放 provider 实例（EdgeTtsProvider 的 OkHttpClient 等资源）
        chain.forEach { it.release() }
    }

    // ─────────────────────────────────────────────
    // DashScope WebSocket 合成事件流（run-task 三事件，MockWebServer 模拟服务端）
    // ─────────────────────────────────────────────

    @Test
    fun speak_successFlow_runTaskThreeEventsAndPlayback() = runTest {
        val server = MockWebServer()
        server.start()
        // 三事件：服务端依次收到 run-task / continue-task / finish-task
        val receivedRunTask = AtomicReference<String?>(null)
        val receivedContinueTask = AtomicReference<String?>(null)
        val receivedFinishTask = AtomicReference<String?>(null)
        // 假 MP3 音频帧（ID3 头）——服务端收到 continue-task 后推音频，收到 finish-task 后收尾
        val fakeAudio = byteArrayOf(
            0x49, 0x44, 0x33, 0x03.toByte(), 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
        )
        server.enqueue(MockResponse().withWebSocketUpgrade(object : WebSocketListener() {
            override fun onMessage(webSocket: WebSocket, text: String) {
                val message = JSONObject(text)
                when (message.getJSONObject("header").getString("action")) {
                    "run-task" -> {
                        receivedRunTask.set(text)
                        // 服务端确认会话建立（task-started）
                        webSocket.send("""{"header":{"task_id":"t-1","event":"task-started"}}""")
                    }
                    "continue-task" -> {
                        receivedContinueTask.set(text)
                        // 开始流式返回 MP3 音频帧
                        webSocket.send(fakeAudio.toByteString())
                    }
                    "finish-task" -> {
                        receivedFinishTask.set(text)
                        // 会话结束
                        webSocket.send("""{"header":{"task_id":"t-1","event":"task-finished"}}""")
                    }
                }
            }
        }))

        val provider = CosyVoiceTtsProvider(
            context,
            apiKey = "sk-test",
            wsUrl = wsUrlOf(server),
            synthesisTimeoutMs = 5_000,
        )

        // Robolectric ShadowMediaPlayer：临时文件需注册 MediaInfo，否则 setDataSource 抛异常
        ShadowMediaPlayer.addMediaInfo(
            DataSource.toDataSource(File(context.cacheDir, "cosyvoice_tts_temp.mp3").absolutePath),
            ShadowMediaPlayer.MediaInfo(1000, 0) // duration=1000ms, audioSessionId=0
        )

        val capturedShadow = AtomicReference<ShadowMediaPlayer?>(null)
        ShadowMediaPlayer.setCreateListener { _, shadow -> capturedShadow.set(shadow) }

        try {
            provider.speak("你好，我是猫娘~")

            // 1) 服务端收到的 run-task 事件内容（协议契约：action/streaming/model/parameters）
            val runTaskEvent = receivedRunTask.get()
            assertNotNull("服务端应收到 run-task 事件", runTaskEvent)
            val runHeader = JSONObject(runTaskEvent!!).getJSONObject("header")
            assertEquals("header.action 应为 run-task", "run-task", runHeader.getString("action"))
            assertTrue("header.task_id 必须非空（会话唯一 ID）", runHeader.getString("task_id").isNotBlank())
            assertEquals("header.streaming 应为 duplex", "duplex", runHeader.getString("streaming"))
            val runPayload = JSONObject(runTaskEvent).getJSONObject("payload")
            assertEquals("payload.model 应为 cosyvoice-v1", "cosyvoice-v1", runPayload.getString("model"))
            assertEquals("payload.task_group 应为 audio", "audio", runPayload.getString("task_group"))
            assertEquals("payload.task 应为 tts", "tts", runPayload.getString("task"))
            assertEquals(
                "payload.function 应为 SpeechSynthesizer",
                "SpeechSynthesizer",
                runPayload.getString("function")
            )
            assertEquals("run-task 的 input 应为空对象（文本走 continue-task）", 0, runPayload.getJSONObject("input").length())
            val params = runPayload.getJSONObject("parameters")
            assertEquals("parameters.voice 应为 longxiaochun", "longxiaochun", params.getString("voice"))
            assertEquals("parameters.volume 应为 50", 50, params.getInt("volume"))
            assertEquals("parameters.text_type 应为 PlainText", "PlainText", params.getString("text_type"))
            assertEquals("parameters.sample_rate 应为 24000", 24000, params.getInt("sample_rate"))
            assertEquals("parameters.rate 应为 1.2（级2-C 默认语速）", 1.2, params.getDouble("rate"), 0.0)
            assertEquals("parameters.format 应为 mp3", "mp3", params.getString("format"))
            assertEquals("parameters.pitch 应为 1.1（级2-C 默认音调）", 1.1, params.getDouble("pitch"), 0.0)
            assertEquals("parameters.seed 应为 0", 0, params.getInt("seed"))
            assertEquals("parameters.type 应为 0", 0, params.getInt("type"))
            assertEquals(
                "parameters.instruction 应为默认猫娘萝莉语气（级2-C）",
                "用可爱的猫娘萝莉语气说话，声音甜美带一点傲娇",
                params.getString("instruction")
            )

            // 2) 服务端收到的 continue-task（携带待合成文本）
            val continueEvent = receivedContinueTask.get()
            assertNotNull("服务端应收到 continue-task 事件", continueEvent)
            val continueHeader = JSONObject(continueEvent!!).getJSONObject("header")
            assertEquals("continue-task 的 action 应为 continue-task", "continue-task", continueHeader.getString("action"))
            assertEquals("continue-task 的 streaming 应为 duplex", "duplex", continueHeader.getString("streaming"))
            assertEquals(
                "continue-task 应复用 run-task 的 task_id",
                runHeader.getString("task_id"),
                continueHeader.getString("task_id")
            )
            val continueInput = JSONObject(continueEvent).getJSONObject("payload").getJSONObject("input")
            assertEquals("continue-task 的 input.text 应为原文", "你好，我是猫娘~", continueInput.getString("text"))

            // 3) 服务端收到的 finish-task（结束会话）
            val finishEvent = receivedFinishTask.get()
            assertNotNull("服务端应收到 finish-task 事件", finishEvent)
            val finishHeader = JSONObject(finishEvent!!).getJSONObject("header")
            assertEquals("finish-task 的 action 应为 finish-task", "finish-task", finishHeader.getString("action"))
            assertEquals("finish-task 的 streaming 应为 duplex", "duplex", finishHeader.getString("streaming"))
            assertEquals(
                "finish-task 应复用 run-task 的 task_id",
                runHeader.getString("task_id"),
                finishHeader.getString("task_id")
            )
            assertEquals("finish-task 的 input 应为空对象", 0, JSONObject(finishEvent).getJSONObject("payload").getJSONObject("input").length())

            // 4) 请求头携带 Authorization: Bearer {apiKey}
            val request = server.takeRequest()
            assertEquals("Authorization 头应为 Bearer sk-test", "Bearer sk-test", request.getHeader("Authorization"))

            // 5) 合成成功 → health=OK
            assertEquals("合成成功 health 应为 OK", TtsHealth.OK, provider.info.health.value)

            // 6) 播放状态：MediaPlayer prepared → isSpeaking(true)；completion → false
            val shadow = capturedShadow.get()
            assertNotNull("speak 后应创建 MediaPlayer 播放音频", shadow)
            shadow!!.invokePreparedListener()
            assertTrue("音频就绪开始播放后 isSpeaking 应为 true", provider.isSpeaking.value)
            shadow.invokeCompletionListener()
            assertFalse("播放完成后 isSpeaking 应为 false", provider.isSpeaking.value)
        } finally {
            provider.release()
            server.shutdown()
        }
    }

    // ─────────────────────────────────────────────
    // 级2-C：个性化参数 instruction / rate / pitch
    // ─────────────────────────────────────────────

    @Test
    fun speak_customPersonalizationParams_landInRunTaskParameters() = runTest {
        val server = MockWebServer()
        server.start()
        val receivedRunTask = AtomicReference<String?>(null)
        val fakeAudio = byteArrayOf(
            0x49, 0x44, 0x33, 0x03.toByte(), 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
        )
        server.enqueue(MockResponse().withWebSocketUpgrade(object : WebSocketListener() {
            override fun onMessage(webSocket: WebSocket, text: String) {
                val message = JSONObject(text)
                when (message.getJSONObject("header").getString("action")) {
                    "run-task" -> {
                        receivedRunTask.set(text)
                        webSocket.send("""{"header":{"task_id":"t-1","event":"task-started"}}""")
                    }
                    "continue-task" -> {
                        webSocket.send(fakeAudio.toByteString())
                    }
                    "finish-task" -> {
                        webSocket.send("""{"header":{"task_id":"t-1","event":"task-finished"}}""")
                    }
                }
            }
        }))

        // 显式构造参数应原样进入 run-task parameters（覆盖 SettingsRepository 默认值）
        val provider = CosyVoiceTtsProvider(
            context,
            apiKey = "sk-test",
            wsUrl = wsUrlOf(server),
            synthesisTimeoutMs = 5_000,
            instruction = "用沉稳御姐音说话",
            rate = 0.8f,
            pitch = 1.5f,
        )
        ShadowMediaPlayer.addMediaInfo(
            DataSource.toDataSource(File(context.cacheDir, "cosyvoice_tts_temp.mp3").absolutePath),
            ShadowMediaPlayer.MediaInfo(1000, 0)
        )

        try {
            provider.speak("你好喵")

            val params = JSONObject(receivedRunTask.get()!!).getJSONObject("payload").getJSONObject("parameters")
            assertEquals(
                "instruction 应为构造参数（覆盖默认猫娘语气）",
                "用沉稳御姐音说话",
                params.getString("instruction")
            )
            assertEquals("rate 应为构造参数 0.8", 0.8, params.getDouble("rate"), 0.0)
            assertEquals("pitch 应为构造参数 1.5", 1.5, params.getDouble("pitch"), 0.0)
        } finally {
            provider.release()
            server.shutdown()
        }
    }

    @Test
    fun speak_readsPersonalizationFromSettingsRepository() = runTest {
        // 用户设置 → provider 构造时读取（未显式传参时）→ run-task parameters
        val settings = SettingsRepository(context)
        settings.cosyvoiceInstruction = "从设置读取的温柔语气"
        settings.cosyvoiceRate = 2.0f
        settings.cosyvoicePitch = 0.9f

        val server = MockWebServer()
        server.start()
        val receivedRunTask = AtomicReference<String?>(null)
        val fakeAudio = byteArrayOf(
            0x49, 0x44, 0x33, 0x03.toByte(), 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
        )
        server.enqueue(MockResponse().withWebSocketUpgrade(object : WebSocketListener() {
            override fun onMessage(webSocket: WebSocket, text: String) {
                val message = JSONObject(text)
                when (message.getJSONObject("header").getString("action")) {
                    "run-task" -> {
                        receivedRunTask.set(text)
                        webSocket.send("""{"header":{"task_id":"t-1","event":"task-started"}}""")
                    }
                    "continue-task" -> webSocket.send(fakeAudio.toByteString())
                    "finish-task" -> {
                        webSocket.send("""{"header":{"task_id":"t-1","event":"task-finished"}}""")
                    }
                }
            }
        }))

        val provider = CosyVoiceTtsProvider(
            context,
            apiKey = "sk-test",
            wsUrl = wsUrlOf(server),
            synthesisTimeoutMs = 5_000,
        )
        ShadowMediaPlayer.addMediaInfo(
            DataSource.toDataSource(File(context.cacheDir, "cosyvoice_tts_temp.mp3").absolutePath),
            ShadowMediaPlayer.MediaInfo(1000, 0)
        )

        try {
            provider.speak("你好喵")

            val params = JSONObject(receivedRunTask.get()!!).getJSONObject("payload").getJSONObject("parameters")
            assertEquals(
                "instruction 应来自 SettingsRepository 用户设置",
                "从设置读取的温柔语气",
                params.getString("instruction")
            )
            assertEquals("rate 应来自 SettingsRepository 用户设置", 2.0, params.getDouble("rate"), 0.0)
            assertEquals("pitch 应来自 SettingsRepository 用户设置", 0.9, params.getDouble("pitch"), 0.0)
        } finally {
            provider.release()
            server.shutdown()
        }
    }

    @Test
    fun speak_taskFailed_eventRaisesAndSetsDegraded() = runTest {
        val server = MockWebServer()
        server.start()
        server.enqueue(MockResponse().withWebSocketUpgrade(object : WebSocketListener() {
            override fun onMessage(webSocket: WebSocket, text: String) {
                // 服务端在收到 run-task 后直接拒绝（task-failed）
                val message = JSONObject(text)
                if (message.getJSONObject("header").getString("action") == "run-task") {
                    webSocket.send(
                        """{"header":{"task_id":"t-2","event":"task-failed"},"payload":{"message":"invalid voice"}}"""
                    )
                }
            }
        }))

        val provider = CosyVoiceTtsProvider(
            context,
            apiKey = "sk-test",
            wsUrl = wsUrlOf(server),
            synthesisTimeoutMs = 5_000,
        )
        try {
            var failure: Exception? = null
            try {
                provider.speak("你好")
            } catch (e: Exception) {
                failure = e
            }
            assertNotNull("task-failed 应抛异常触发降级", failure)
            assertTrue(
                "失败信息应包含服务端错误（invalid voice）",
                failure!!.message!!.contains("invalid voice")
            )
            assertEquals("失败后 health 应为 DEGRADED", TtsHealth.DEGRADED, provider.info.health.value)
            assertFalse("失败后不应处于播放状态", provider.isSpeaking.value)
        } finally {
            provider.release()
            server.shutdown()
        }
    }

    // ─────────────────────────────────────────────
    // WS 失败：异常向上传播（降级链契约）
    // ─────────────────────────────────────────────

    @Test
    fun speak_wsFailure_propagatesAndSetsDegraded() = runTest {
        // 不可达端口（127.0.0.1:1 未监听）→ 连接拒绝，快速失败
        val provider = CosyVoiceTtsProvider(
            context,
            apiKey = "sk-test",
            wsUrl = "ws://127.0.0.1:1/",
            synthesisTimeoutMs = 3_000,
        )
        try {
            var failure: Exception? = null
            try {
                provider.speak("你好")
            } catch (e: Exception) {
                failure = e
            }
            assertNotNull("WS 连接失败必须抛异常（由降级链处理）", failure)
            assertEquals("WS 失败后 health 应为 DEGRADED", TtsHealth.DEGRADED, provider.info.health.value)
            assertFalse("失败后不应停留在播放状态", provider.isSpeaking.value)
        } finally {
            provider.release()
        }
    }

    // ─────────────────────────────────────────────
    // Lifecycle（stop / release 幂等）
    // ─────────────────────────────────────────────

    @Test
    fun stopAndReleaseAreIdempotent() {
        val provider = CosyVoiceTtsProvider(context, apiKey = "sk-test")
        provider.stop()
        provider.stop()
        provider.release()
        provider.release()
        provider.stop()
        assertFalse("release 后播放状态应归零", provider.isSpeaking.value)
    }

    @Test
    fun releaseStopsActivePlayback() = runTest {
        val provider = CosyVoiceTtsProvider(context, apiKey = "sk-test")
        // 无真实播放时 release 也不崩溃且状态归零
        provider.release()
        assertFalse(provider.isSpeaking.value)
    }

    // ─────────────────────────────────────────────
    // 降级链集成：CosyVoice 无 Key 快速失败 → 下一个 Provider
    // ─────────────────────────────────────────────

    @OptIn(ExperimentalCoroutinesApi::class)
    @Test
    fun cascadeIntegration_cosyvoiceNoKey_fallsBackToEdgeProvider() = runTest {
        val scheduler = TestCoroutineScheduler()
        Dispatchers.setMain(StandardTestDispatcher(scheduler))
        try {
            TtsProviderRegistry.clear()
            val cosy = CosyVoiceTtsProvider(context, apiKey = "")
            val edge = mock<TtsProvider>()
            whenever(edge.info).thenReturn(
                TtsProviderInfo(
                    id = "edge_tts",
                    name = "Edge TTS",
                    category = TtsProviderCategory.CLOUD_WS,
                    requiresNetwork = true,
                    health = MutableStateFlow(TtsHealth.OK),
                    priority = 1,
                    configurable = false,
                )
            )
            whenever(edge.isSpeaking).thenReturn(MutableStateFlow(false))
            TtsProviderRegistry.register(cosy)
            TtsProviderRegistry.register(edge)

            val chain = TtsProviderRegistry.cascade()
            assertEquals(
                "降级链第一位应为 cosyvoice_ws",
                "cosyvoice_ws",
                chain.first().info.id
            )

            val controller = VoiceIoController(context, chain, mock())
            controller.speak("你好喵")
            // CosyVoice（无 Key）快速失败 → 降级链继续 → Edge 收到文本
            verify(edge).speak("你好喵")
            controller.release()
            cosy.release()
        } finally {
            Dispatchers.resetMain()
            TtsProviderRegistry.clear()
        }
    }

    /** 将 MockWebServer 地址转为 ws:// 协议（OkHttp WebSocket 要求 ws/wss scheme）。 */
    private fun wsUrlOf(server: MockWebServer): String =
        "ws://${server.hostName}:${server.port}/"
}
