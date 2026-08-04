package com.live2dai.android

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.live2dai.android.data.SettingsRepository
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * SettingsRepository 持久化标志测试（首次启动引导 / 免费共享额度提示）。
 *
 * 覆盖本轮 onboarding 相关的持久化行为：
 * - onboardingCompleted 默认 false（首次启动要展示引导）
 * - 设置为 true 后持久化（重启不再展示）
 * - freeQuotaNoticeDismissed 默认 false（免费共享额度 banner 首次要展示）
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class, sdk = [34])
class SettingsRepositoryTest {

    private val context: Context = ApplicationProvider.getApplicationContext()

    @Before
    fun setUp() {
        // 清空设置存储，从干净状态开始
        context.getSharedPreferences("live2dai_settings", Context.MODE_PRIVATE)
            .edit().clear().commit()
    }

    @Test
    fun onboardingCompleted_defaultsToFalse() {
        val repo = SettingsRepository(context)
        assertFalse("首次启动应展示引导", repo.onboardingCompleted)
    }

    @Test
    fun onboardingCompleted_persistsAfterSet() {
        val repo = SettingsRepository(context)
        repo.onboardingCompleted = true
        // 重新创建实例（模拟重启），值应仍为 true
        val repo2 = SettingsRepository(context)
        assertTrue("完成后应持久化，重启不再展示", repo2.onboardingCompleted)
    }

    @Test
    fun freeQuotaNoticeDismissed_defaultsToFalse_andPersists() {
        val repo = SettingsRepository(context)
        assertFalse(repo.freeQuotaNoticeDismissed)

        repo.freeQuotaNoticeDismissed = true
        val repo2 = SettingsRepository(context)
        assertTrue(repo2.freeQuotaNoticeDismissed)
    }

    // ── Live2D 形象（selectedModelId）──

    @Test
    fun selectedModelId_defaultsToMaoPro() {
        val repo = SettingsRepository(context)
        assertEquals("首次启动默认形象应为 mao_pro", SettingsRepository.DEFAULT_MODEL_ID, repo.selectedModelId)
    }

    @Test
    fun selectedModelId_persistsAcrossRestart() {
        val repo = SettingsRepository(context)
        repo.selectedModelId = "shizuku"
        // 重新创建实例（模拟重启），值应仍为 shizuku
        val repo2 = SettingsRepository(context)
        assertEquals("切换后应持久化，重启仍为 shizuku", "shizuku", repo2.selectedModelId)
    }

    @Test
    fun selectedModelId_resetAll_restoresDefault() {
        val repo = SettingsRepository(context)
        repo.selectedModelId = "shizuku"
        repo.resetAll()
        val repo2 = SettingsRepository(context)
        assertEquals("resetAll 后应回退默认形象", SettingsRepository.DEFAULT_MODEL_ID, repo2.selectedModelId)
    }
}
