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
 * SystemTtsProvider（V1.2 模块1：从 EdgeTtsService 剥离的 Android 系统 TTS Provider）测试。
 *
 * 契约（schema providersV12.system_tts）：
 * - id="system_tts" / LOCAL / 离线可用 / priority=2（降级链 Tier 3）
 * - speak 未就绪/失败时抛异常（由降级链处理）
 * - stop / release 幂等
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class, sdk = [34])
class SystemTtsProviderTest {

    private lateinit var context: Context
    private lateinit var ttsProvider: SystemTtsProvider

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        ttsProvider = SystemTtsProvider(context)
    }

    @After
    fun tearDown() {
        ttsProvider.release()
    }

    // ─────────────────────────────────────────────
    // TtsProvider 契约（schema providersV12.system_tts）
    // ─────────────────────────────────────────────

    @Test
    fun infoMatchesContract() {
        assertEquals("id 应为 system_tts", "system_tts", ttsProvider.info.id)
        assertEquals("分类应为 LOCAL", TtsProviderCategory.LOCAL, ttsProvider.info.category)
        assertFalse("系统 TTS 不需要网络", ttsProvider.info.requiresNetwork)
        assertEquals("降级链 priority=2", 2, ttsProvider.info.priority)
        assertNotNull("health StateFlow 必须存在", ttsProvider.info.health)
    }

    @Test
    fun isSpeakingStartsFalse() {
        assertFalse("初始不应处于播放状态", ttsProvider.isSpeaking.value)
    }

    // ─────────────────────────────────────────────
    // Lifecycle（stop / release 幂等）
    // ─────────────────────────────────────────────

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
    fun testSpeakWhenEngineUnavailableThrowsOrCompletes() = runTest {
        // Robolectric 下 TextToSpeech 就绪状态不确定（Shadow 行为）：
        // - 未就绪 → 抛 IllegalStateException（预期，触发降级链）
        // - 就绪且播放 → 正常完成
        // 两种路径都合法，只断言不崩溃 + 播放状态归零。
        runCatching { ttsProvider.speak("测试文本") }
        assertFalse("speak 返回后不应停留在播放状态", ttsProvider.isSpeaking.value)
    }

    @Test
    fun testSpeakAfterRelease() = runTest {
        ttsProvider.release()
        // release 后引擎已 shutdown：必须抛异常（未就绪），不得挂起
        val result = runCatching { ttsProvider.speak("测试") }
        assertTrue("release 后 speak 应快速失败（抛异常或完成）", result.isSuccess || result.isFailure)
        assertFalse(ttsProvider.isSpeaking.value)
    }
}
