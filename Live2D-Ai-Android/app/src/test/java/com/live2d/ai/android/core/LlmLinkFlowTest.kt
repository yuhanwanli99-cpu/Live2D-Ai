package com.live2d.ai.android.core

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.runBlocking
import okhttp3.OkHttpClient
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import com.live2d.ai.android.PersonaConfig
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import java.util.concurrent.TimeUnit

/**
 * Flow 冷流单消费者 + reset 语义 RED（契约 refactor-0814 §2.2 `LlmLink.send(text): Flow<LlmEvent>`，
 * `reset()` 清上下文；架构 shell §6 SSE/错误处理，Wave 2C-2）。
 *
 * 覆盖生产文件 `core/LlmLink.kt` 的 **LlmLink** 类（**新增，RED 待实现**）。
 * class name 唯一：`LlmLink*Test`；test core 目录现无 `LlmLink`/`LlmEvent` 符号。
 *
 * 生产 seam spec（对齐契约 §2.2）：
 *   class LlmLink(
 *       private val baseUrl: String,
 *       private val apiKey: String,
 *       private val client: OkHttpClient,
 *       private val persona: PersonaConfig,
 *       private val bridge: EmotionClassifierBridge = EmotionClassifierBridge(HttpEmotionClassifier(...)),
 *   ) {
 *       fun send(text: String): Flow<LlmEvent>   // 冷流、单消费者
 *       fun reset()                               // 清上下文
 *   }
 *
 * 判读面（见 RED 报告 §6 [UNSURE-4]）：本测试用 MockWebServer 驱动真实 HTTP 冷流，
 * 锚定「冷流、零重放（非 SharedFlow）」「reset 清上下文（后续请求不带旧轮）」三个**可观测断言**；
 * 真协程并发单消费者/在途流取消交 Wave 4。
 */
class LlmLinkFlowTest {

    private lateinit var mockWebServer: MockWebServer

    @Before
    fun setUp() {
        mockWebServer = MockWebServer()
        mockWebServer.start()
    }

    @After
    fun tearDown() {
        mockWebServer.shutdown()
    }

    private fun link(): LlmLink {
        val baseUrl = mockWebServer.url("").toString().trimEnd('/')
        return HttpLlmLink(
            baseUrl = baseUrl,
            apiKey = "test-key",
            client = OkHttpClient.Builder().callTimeout(5, TimeUnit.SECONDS).build(),
            persona = PersonaConfig,
            // 第 5 参注入恒 null 分类器 fake（生产 seam）：默认桥（HttpEmotionClassifier）会对
            // MockWebServer 发真实分类 HTTP（单次超时 3s），吞掉队列中的 SSE 响应并污染
            // requestCount/响应时序 → 冷流与 reset 断言失真。fake 恒 null → 情绪回退
            // EmotionText.parse 内嵌标签（本测试 SSE 无 [tag] → neutral），时序确定性恢复。
            bridge = EmotionClassifierBridge(
                object : EmotionClassifier {
                    override suspend fun classify(text: String): EmotionClassifierResult? = null
                },
            ),
        )
    }

    private fun sse(deltas: List<String>): String =
        deltas.joinToString("") { """data: {"choices":[{"delta":{"role":"assistant","content":"$it"}}]}""" + "\n" } +
            "data: [DONE]\n"

    private fun enqueue(body: String) {
        mockWebServer.enqueue(
            MockResponse()
                .setResponseCode(200)
                .setHeader("Content-Type", "text/event-stream")
                .setBody(body),
        )
    }

    // ═══════════════════════════════════════════════════════════════
    // 一、冷流语义：每 collect 独立
    // ═══════════════════════════════════════════════════════════════

    /** 同一 LlmLink.send 返回的流 collect 两次 → 两次各自独立产生发射（两次 HTTP），产物均为各自 delta。 */
    @Test
    fun coldFlow_eachCollectIsIndependent() = runBlocking(Dispatchers.IO) {
        enqueue(sse(listOf("第一", "句")))
        enqueue(sse(listOf("第二", "句")))

        val llm = link()
        val flow: Flow<LlmEvent> = llm.send("你好")

        val first = flow.toList()
        val second = flow.toList()

        // 冷流：同一 Flow 对象可被 collect 两次，两次都重新跑生产者
        assertEquals(
            "第一次 collect 产物（Delta 面；流尾另有 Completed）",
            listOf(LlmEvent.Delta("第一"), LlmEvent.Delta("句")),
            first.filterIsInstance<LlmEvent.Delta>(),
        )
        assertEquals(
            "第二次 collect 产物（独立重跑，非复用第一次结果）",
            listOf(LlmEvent.Delta("第二"), LlmEvent.Delta("句")),
            second.filterIsInstance<LlmEvent.Delta>(),
        )
        assertTrue("第一次 collect 流尾发射 Completed（收口）", first.any { it is LlmEvent.Completed })
        assertTrue("第二次 collect 流尾发射 Completed（收口）", second.any { it is LlmEvent.Completed })
        // 两次 collect → 两次独立 HTTP（MockWebServer 若为 SharedFlow 重放则不会二次请求）
        assertEquals("冷流每 collect 独立 → 两次 HTTP 请求", 2, mockWebServer.requestCount)
    }

    // ═══════════════════════════════════════════════════════════════
    // 二、零重放（非 SharedFlow）
    // ═══════════════════════════════════════════════════════════════

    /** 首个 collect 完成后二次 collect → 二次 collect 触发**自己**的 HTTP，而非重放首次已发射事件（零重放）。 */
    @Test
    fun zeroReplay_secondCollect_doesNotReplayFirst() = runBlocking(Dispatchers.IO) {
        enqueue(sse(listOf("甲", "乙")))
        enqueue(sse(listOf("丙", "丁")))

        val llm = link()
        val flow = llm.send("测试")

        val first = flow.toList()
        // 首 collect 完成（[DONE] 已读）
        assertEquals(listOf(LlmEvent.Delta("甲"), LlmEvent.Delta("乙")), first.filterIsInstance<LlmEvent.Delta>())
        assertTrue("首 collect 流尾发射 Completed", first.any { it is LlmEvent.Completed })

        // 二次 collect 前绝不因 SharedFlow 缓存「免费」获得首次结果：先断言请求未发生
        assertEquals("二次 collect 前不应有额外缓冲发射（零重放/非 hot）", 1, mockWebServer.requestCount)

        val second = flow.toList()
        assertEquals("二次 collect 触发自己 HTTP 拿新数据", listOf(LlmEvent.Delta("丙"), LlmEvent.Delta("丁")), second.filterIsInstance<LlmEvent.Delta>())
        assertTrue("二次 collect 流尾发射 Completed", second.any { it is LlmEvent.Completed })
        assertEquals("二次 collect 产生第二个 HTTP 请求", 2, mockWebServer.requestCount)
    }

    // ═══════════════════════════════════════════════════════════════
    // 三、reset 清上下文（后续请求不带旧轮历史）
    // ═══════════════════════════════════════════════════════════════

    /** send(旧话) → reset() → send(新话)：reset 清上下文 → 第二个请求体不含旧话历史。 */
    @Test
    fun reset_clearsHistory_subsequentRequestOmitsPriorTurn() = runBlocking(Dispatchers.IO) {
        enqueue(sse(listOf("旧回复")))
        enqueue(sse(listOf("新回复")))

        val llm = link()

        // 第一轮
        llm.send("旧话").toList()
        val req1 = mockWebServer.takeRequest().body.readUtf8()
        // 第一轮请求应含 persona system 注入
        assertTrue("首 token 请求应含 persona 注入的 system 提示", req1.contains("你是白"))

        // reset 清上下文
        llm.reset()

        // 第二轮（同 Link，未另建会话）
        llm.send("新话").toList()
        val req2 = mockWebServer.takeRequest().body.readUtf8()

        // reset 语义：第二轮请求不含第一轮 "旧话" 历史（若未 reset 则 50 对内会保留该轮 pair）
        assertTrue("第二轮请求应含当前消息", req2.contains("新话"))
        assertTrue("第二轮请求应再次注入 persona system", req2.contains("你是白"))
        assertFalse("reset 清上下文：第二轮请求不得携带第一轮 user 消息", req2.contains("旧话"))
    }

    /** reset() 幂等且不抛异常（重复调用安全）。 */
    @Test
    fun reset_isIdempotent_andDoesNotThrow() = runBlocking(Dispatchers.IO) {
        enqueue(sse(listOf("ok")))

        val llm = link()
        llm.reset()
        llm.reset()
        llm.reset()

        // reset 后仍能正常 send
        val result = llm.send("消息").toList()
        assertEquals(listOf(LlmEvent.Delta("ok")), result.filterIsInstance<LlmEvent.Delta>())
        assertTrue("流尾发射 Completed", result.any { it is LlmEvent.Completed })
    }

    // ═══════════════════════════════════════════════════════════════
    // 四、签名合规：send 返回 [Flow/backing of] LlmEvent（编译期契约断言）
    // ═══════════════════════════════════════════════════════════════

    /** 反射/编译面：LlmLink 暴露 send(text): Flow<LlmEvent> 与 reset():Unit（对齐契约§2.2）。 */
    @Test
    fun link_exposesContractSignatures_viaReflection() {
        val sendMethod = try {
            LlmLink::class.java.getMethod("send", String::class.java)
        } catch (e: NoSuchMethodException) {
            throw AssertionError("LlmLink 缺 send(String) （契约 §2.2）", e)
        }
        assertTrue(
            "send 返回类型应为 Flow（契约 §2.2 冷流）",
            Flow::class.java.isAssignableFrom(sendMethod.returnType),
        )
        val resetMethod = try {
            LlmLink::class.java.getMethod("reset")
        } catch (e: NoSuchMethodException) {
            throw AssertionError("LlmLink 缺 reset() （契约 §2.2）", e)
        }
        assertEquals("reset 返回 Unit（清上下文，无返回值语义）", Void.TYPE, resetMethod.returnType)
    }
}
