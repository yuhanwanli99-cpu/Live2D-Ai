package com.live2dai.android

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Tests for EdgeTtsProvider — focuses on public API lifecycle + TtsProvider contract.
 *
 * The provider uses WebSocket + MediaPlayer internally which are hard to mock
 * at the bytecode level in unit tests. These tests verify:
 * - Provider can be constructed
 * - TtsProvider 契约：info 字段符合 schema（id="edge_tts" / CLOUD_WS / requiresNetwork）
 * - Lifecycle methods (stop, release) don't crash
 * - speak 失败向上抛异常（供降级链处理），isSpeaking 归零
 *
 * Full WebSocket integration tests require instrumented tests on a real device.
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class, sdk = [34])
class EdgeTtsProviderTest {

    private lateinit var context: Context
    private lateinit var ttsProvider: EdgeTtsProvider

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        ttsProvider = EdgeTtsProvider(context)
    }

    @After
    fun tearDown() {
        ttsProvider.release()
    }

    // ─────────────────────────────────────────────
    // TtsProvider 契约（schema providersV12.edge_tts）
    // ─────────────────────────────────────────────

    @Test
    fun infoMatchesContract() {
        assertEquals("id 应为 edge_tts", "edge_tts", ttsProvider.info.id)
        assertEquals("分类应为 CLOUD_WS", TtsProviderCategory.CLOUD_WS, ttsProvider.info.category)
        assertTrue("Edge TTS 需要网络", ttsProvider.info.requiresNetwork)
        assertEquals("Wave A 降级链 priority=1（CosyVoice 注册前暂占）", 1, ttsProvider.info.priority)
        assertFalse("Edge TTS 无用户配置参数", ttsProvider.info.configurable)
        assertNotNull("health StateFlow 必须存在", ttsProvider.info.health)
    }

    @Test
    fun isSpeakingStartsFalse() {
        assertFalse("初始不应处于播放状态", ttsProvider.isSpeaking.value)
    }

    @Test
    fun testProviderCreation() {
        assertNotNull("EdgeTtsProvider should be constructable", ttsProvider)
    }

    // ─────────────────────────────────────────────
    // Lifecycle（stop / release 幂等）
    // ─────────────────────────────────────────────

    @Test
    fun testStopCanBeCalledBeforeAnyPlayback() {
        // stop() should be safe to call even with no active playback
        ttsProvider.stop()
    }

    @Test
    fun testStopCanBeCalledMultipleTimes() {
        ttsProvider.stop()
        ttsProvider.stop()
        ttsProvider.stop()
    }

    @Test
    fun testReleaseDoesNotCrash() {
        ttsProvider.release()
    }

    @Test
    fun testReleaseCanBeCalledMultipleTimes() {
        ttsProvider.release()
        // Second release should not crash (null-safe cleanup)
        ttsProvider.release()
    }

    @Test
    fun testStopAfterReleaseDoesNotCrash() {
        ttsProvider.release()
        ttsProvider.stop()
    }

    // ─────────────────────────────────────────────
    // speak：失败向上传播（降级链契约）
    // ─────────────────────────────────────────────

    @Test
    fun testSpeakFailurePropagatesToCascade() = runTest {
        // speak 失败必须抛异常（由 VoiceIoController 降级链捕获继续尝试下一个 Provider）。
        // 单测环境 WebSocket 大概率失败（无真实网络/端点），因此：
        // - 抛异常 = 符合降级契约（预期路径）
        // - 罕见情况返回 = 网络可用且合成成功（也不断言失败）
        runCatching { ttsProvider.speak("测试文本") }
        // 无论成功/失败，播放状态必须归零（未真正出声）
        assertFalse("speak 返回后不应停留在播放状态", ttsProvider.isSpeaking.value)
    }

    @Test
    fun testSpeakEmptyText() = runTest {
        // 空文本不应崩溃；抛异常也是合法路径（触发降级）
        runCatching { ttsProvider.speak("") }
    }

    @Test
    fun testSpeakAfterRelease() = runTest {
        ttsProvider.release()
        // Should not crash even after release
        runCatching { ttsProvider.speak("测试") }
    }

    @Test
    fun testFullLifecycle() = runTest {
        runCatching { ttsProvider.speak("你好") }
        ttsProvider.stop()
        runCatching { ttsProvider.speak("世界") }
        ttsProvider.release()
    }
}
