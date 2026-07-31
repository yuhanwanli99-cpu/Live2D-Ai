package com.live2dai.plugin.vision

import android.graphics.Bitmap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * [GlmVisionClient] 单元测试：用 MockWebServer 模拟 GLM-4V 响应。
 * 使用 Robolectric 提供 android.graphics.Bitmap 实现。
 */
@RunWith(RobolectricTestRunner::class)
class GlmVisionClientTest {

    private lateinit var server: MockWebServer

    @Before
    fun setUp() {
        server = MockWebServer()
        server.start()
    }

    @After
    fun tearDown() {
        server.shutdown()
    }

    private fun baseUrl(): String = server.url("/").toString().trimEnd('/')

    private fun sampleBitmap(): Bitmap = Bitmap.createBitmap(4, 4, Bitmap.Config.ARGB_8888)

    @Test
    fun describeImage_returnsContentFromMessage() = runBlocking(Dispatchers.IO) {
        server.enqueue(
            MockResponse()
                .setResponseCode(200)
                .setBody("""{"choices":[{"message":{"content":"屏幕上显示一只猫"}}]}""")
        )

        val client = GlmVisionClient(apiKey = "test-key", baseUrl = baseUrl())
        val result = client.describeImage(sampleBitmap(), "glm-4.6v", "描述")

        assertEquals("屏幕上显示一只猫", result)
    }

    @Test
    fun describeImage_handlesStreamingDeltaFormat() = runBlocking(Dispatchers.IO) {
        server.enqueue(
            MockResponse()
                .setResponseCode(200)
                .setBody("""{"choices":[{"delta":{"content":"这是一张聊天界面截图"}}]}""")
        )

        val client = GlmVisionClient(apiKey = "test-key", baseUrl = baseUrl())
        val result = client.describeImage(sampleBitmap(), "glm-4.6v")

        assertEquals("这是一张聊天界面截图", result)
    }

    @Test
    fun describeImage_returnsEmptyOnHttpError() = runBlocking(Dispatchers.IO) {
        server.enqueue(MockResponse().setResponseCode(401).setBody("""{"error":"unauthorized"}"""))

        val client = GlmVisionClient(apiKey = "test-key", baseUrl = baseUrl())
        val result = client.describeImage(sampleBitmap(), "glm-4.6v")

        assertEquals("", result)
    }

    @Test
    fun describeImage_returnsEmptyOnBlankApiKey() = runBlocking(Dispatchers.IO) {
        // apiKey 为空 → 不发请求直接返回空串
        val client = GlmVisionClient(apiKey = "", baseUrl = baseUrl())
        val result = client.describeImage(sampleBitmap(), "glm-4.6v")

        assertEquals("", result)
        assertEquals(0, server.requestCount)
    }

    @Test
    fun describeImage_sendsAuthorizationHeaderAndModel() = runBlocking(Dispatchers.IO) {
        server.enqueue(
            MockResponse()
                .setResponseCode(200)
                .setBody("""{"choices":[{"message":{"content":"ok"}}]}""")
        )

        val client = GlmVisionClient(apiKey = "secret-key", baseUrl = baseUrl())
        client.describeImage(sampleBitmap(), "glm-4v-flash")

        val request = server.takeRequest()
        assertEquals("Bearer secret-key", request.getHeader("Authorization"))
        assertTrue("Request path should be /chat/completions", request.path == "/chat/completions")
        val body = request.body.readUtf8()
        assertTrue("Body should contain model", body.contains("glm-4v-flash"))
        assertTrue("Body should contain image data url", body.contains("data:image/jpeg;base64,"))
        assertTrue("Body should not stream", body.contains("\"stream\":false"))
    }
}
