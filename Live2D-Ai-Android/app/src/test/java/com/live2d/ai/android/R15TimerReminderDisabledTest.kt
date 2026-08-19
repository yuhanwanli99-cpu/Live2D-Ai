package com.live2d.ai.android

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.live2d.ai.android.data.SettingsRepository
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * R15 布局大改造 —— 需求3「关闭定时主动提醒：定时提醒功能移除（不再主动定时推送）」—— RED 测试。
 *
 * 依据：需求真相 translation.md R15（待补：定时提醒移除）+ T7 测试契约（T7-layout-red.md）。
 *
 * 定时主动提醒 = 主动陪伴插件（ProactivePlugin / ProactiveCheckWorker，WorkManager 周期检查 →
 * 主动问候播报/通知）。其总开关现有字段：SettingsRepository.proactiveEnabled（当前默认 true）。
 * R15 要求「不再主动定时推送」→ 该开关必须**默认为关闭（false）**（机械断言：定时推送默认不启用）。
 *
 * 锁定的接口（现有）：SettingsRepository.proactiveEnabled（Boolean，可 Robolectric 持久化）。
 *
 * 请求状态（现状 → RED）：
 *   当前 SettingsRepository.proactiveEnabled **默认 true**（主动定时推送开启）。
 *   本用例断言默认 false → 现状失败（RED 成立）。
 *
 * 注意（冲突共存）：
 *   现有 `SettingsRepositoryTest.proactiveEnabled_defaultsToTrue_andPersists` 断言默认 true，
 *   与 R15 需求直接冲突。该旧用例将被 R15 需求推翻（改默认 false 后旧用例失败），
 *   属于**测试自身需同步更新**的冲突，交由主代理/ coder C15 处理（本 SOP 测试设计师不改存量测试，
 *   见 T7-layout-red.md §5 如实标注）。
 *
 * GREEN 条件（coder R15 实现）：SettingsRepository.DEFAULT 语义 —— proactiveEnabled 默认 false；
 * 显式 save(true)/save(false) 后 roundtrip 保持（持久化行为不得回归）。
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class, sdk = [34])
class R15TimerReminderDisabledTest {

    private val context: Context = ApplicationProvider.getApplicationContext()

    @Before
    fun setUp() {
        context.getSharedPreferences("live2dai_settings", Context.MODE_PRIVATE)
            .edit().clear().commit()
    }

    // ── 需求3：定时主动提醒默认关闭（不再主动定时推送）──

    @Test
    fun proactiveEnabled_defaultsToFalse() {
        val repo = SettingsRepository(context)
        assertFalse(
            "R15 需求3：定时主动提醒（ProactivePlugin 周期推送）默认必须关闭（false）。" +
                "现状默认 true（定时推送开启）→ RED。",
            repo.proactiveEnabled
        )
    }

    // ── 持久化行为回归锁：显式开关后 roundtrip 保持 ──

    @Test
    fun proactiveEnabled_persistsAfterExplicitSet() {
        val repo = SettingsRepository(context)
        // 显式开启（若旧数据曾开启，仅当用户手动改回——但 R15 默认关闭基线不受影响）
        repo.proactiveEnabled = true
        val repo2 = SettingsRepository(context)
        assertEquals("显式开启后应持久化（roundtrip）", true, repo2.proactiveEnabled)

        repo2.proactiveEnabled = false
        val repo3 = SettingsRepository(context)
        assertEquals("显式关闭后应持久化（roundtrip）", false, repo3.proactiveEnabled)
    }

    // ── resetAll 回归锁：重置后回到默认（关闭）──

    @Test
    fun proactiveEnabled_resetAll_restoresDefaultOff() {
        val repo = SettingsRepository(context)
        repo.proactiveEnabled = true
        repo.resetAll()
        val repo2 = SettingsRepository(context)
        assertFalse(
            "resetAll 后定时提醒应回到默认关闭（false）",
            repo2.proactiveEnabled
        )
    }
}
