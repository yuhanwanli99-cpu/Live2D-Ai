package com.live2dai.plugin.proactive

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * [LastInteractionTracker] 的时间确定性测试（R4）：
 * 所有断言都用固定时间戳注入，不依赖真实系统时钟。
 *
 * 验收标准 1：模拟"3 小时前对话 + 现在" → minutesSinceLastInteraction 返回 180。
 */
@RunWith(RobolectricTestRunner::class)
class LastInteractionTrackerTest {

    private lateinit var context: Context
    private lateinit var tracker: LastInteractionTracker

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        tracker = LastInteractionTracker(context)
        clearPrefs()
    }

    @After
    fun tearDown() {
        clearPrefs()
    }

    private fun clearPrefs() {
        context.getSharedPreferences(LastInteractionTracker.PREFS_NAME, Context.MODE_PRIVATE)
            .edit().clear().commit()
    }

    // ── 验收标准 1：距上次交互时间 ──

    @Test
    fun minutesSinceLastInteraction_threeHoursAgo_returns180() {
        val now = 1_700_000_000_000L
        tracker.recordInteraction(now - 3 * 60 * 60 * 1000L)

        assertEquals(180L, tracker.minutesSinceLastInteraction(now))
    }

    @Test
    fun minutesSinceLastInteraction_neverInteracted_returnsMaxValue() {
        assertEquals(Long.MAX_VALUE, tracker.minutesSinceLastInteraction(1_700_000_000_000L))
    }

    @Test
    fun minutesSinceLastInteraction_clampsFutureTimestampToZero() {
        val now = 1_700_000_000_000L
        tracker.recordInteraction(now + 5 * 60 * 1000L) // 时钟回拨/未来时间戳

        assertEquals(0L, tracker.minutesSinceLastInteraction(now))
    }

    @Test
    fun recordInteraction_overwritesPreviousTimestamp() {
        val now = 1_700_000_000_000L
        tracker.recordInteraction(now - 60 * 60 * 1000L)
        tracker.recordInteraction(now - 10 * 60 * 1000L)

        assertEquals(10L, tracker.minutesSinceLastInteraction(now))
    }

    // ── 距上次主动消息时间 ──

    @Test
    fun minutesSinceLastProactive_returnsElapsedMinutes() {
        val now = 1_700_000_000_000L
        tracker.recordProactiveMessage(now - 360 * 60 * 1000L)

        assertEquals(360L, tracker.minutesSinceLastProactive(now))
    }

    @Test
    fun minutesSinceLastProactive_neverRecorded_returnsMaxValue() {
        assertEquals(Long.MAX_VALUE, tracker.minutesSinceLastProactive(1_700_000_000_000L))
    }
}
