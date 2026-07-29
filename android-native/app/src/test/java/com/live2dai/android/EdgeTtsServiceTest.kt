package com.live2dai.android

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import kotlinx.coroutines.test.runTest
import org.junit.After
import org.junit.Assert.assertNotNull
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Tests for EdgeTtsService — focuses on public API lifecycle.
 *
 * The service uses WebSocket + MediaPlayer internally which are hard to mock
 * at the bytecode level in unit tests. These tests verify:
 * - Service can be constructed
 * - Lifecycle methods (stop, release) don't crash
 * - Fallback chain handles errors gracefully (no crashes)
 *
 * Full WebSocket integration tests require instrumented tests on a real device.
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class, sdk = [34])
class EdgeTtsServiceTest {

    private lateinit var context: Context
    private lateinit var ttsService: EdgeTtsService

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        ttsService = EdgeTtsService(context)
    }

    @After
    fun tearDown() {
        ttsService.release()
    }

    @Test
    fun testServiceCreation() {
        assertNotNull("EdgeTtsService should be constructable", ttsService)
    }

    @Test
    fun testStopCanBeCalledBeforeAnyPlayback() {
        // stop() should be safe to call even with no active playback
        ttsService.stop()
    }

    @Test
    fun testStopCanBeCalledMultipleTimes() {
        ttsService.stop()
        ttsService.stop()
        ttsService.stop()
    }

    @Test
    fun testReleaseDoesNotCrash() {
        ttsService.release()
    }

    @Test
    fun testReleaseCanBeCalledMultipleTimes() {
        ttsService.release()
        // Second release should not crash (null-safe cleanup)
        ttsService.release()
    }

    @Test
    fun testStopAfterReleaseDoesNotCrash() {
        ttsService.release()
        ttsService.stop()
    }

    @Test
    fun testSpeakHandlesErrorsGracefully() = runTest {
        // speak() should not throw — internal failures fall through
        // the 3-tier fallback chain and result in silent degradation
        ttsService.speak("测试文本")
    }

    @Test
    fun testSpeakEmptyText() = runTest {
        ttsService.speak("")
    }

    @Test
    fun testSpeakAfterRelease() = runTest {
        ttsService.release()
        // Should not crash even after release
        ttsService.speak("测试")
    }

    @Test
    fun testFullLifecycle() = runTest {
        ttsService.speak("你好")
        ttsService.stop()
        ttsService.speak("世界")
        ttsService.release()
    }
}
