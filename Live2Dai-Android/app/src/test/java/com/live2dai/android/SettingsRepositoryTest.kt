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

    @Test
    fun proactiveEnabled_defaultsToFalse_andPersists() {
        // R15 需求3：定时主动提醒移除 → 默认 false（T7-layout-red.md §5.1/§7.4 授权同步旧期望）
        val repo = SettingsRepository(context)
        assertFalse("主动陪伴默认关闭（R15：定时提醒移除，default: false）", repo.proactiveEnabled)

        repo.proactiveEnabled = true
        val repo2 = SettingsRepository(context)
        assertTrue("开启后应持久化（重启后装载主动陪伴插件）", repo2.proactiveEnabled)

        repo2.proactiveEnabled = false
        val repo3 = SettingsRepository(context)
        assertFalse("关闭应持久化", repo3.proactiveEnabled)
    }

    // ── TTS Provider（V1.2 模块1）──

    @Test
    fun ttsProviderId_defaultsToSherpa() {
        val repo = SettingsRepository(context)
        assertEquals(
            "K1 单引擎默认 sherpa_onnx_kokoro（Wave 5 清理）",
            SettingsRepository.DEFAULT_TTS_PROVIDER_ID,
            repo.ttsProviderId
        )
        assertEquals("sherpa_onnx_kokoro", repo.ttsProviderId)
    }

    @Test
    fun ttsProviderId_persistsAcrossRestart() {
        val repo = SettingsRepository(context)
        repo.ttsProviderId = "test_engine"
        // 重新创建实例（模拟重启），值应仍为 test_engine
        val repo2 = SettingsRepository(context)
        assertEquals("切换后应持久化，重启仍为 test_engine", "test_engine", repo2.ttsProviderId)
    }

    @Test
    fun ttsProviderId_resetAll_restoresDefault() {
        val repo = SettingsRepository(context)
        repo.ttsProviderId = "sherpa_onnx_kokoro"
        repo.resetAll()
        val repo2 = SettingsRepository(context)
        assertEquals(
            "resetAll 后应回退默认首选引擎",
            SettingsRepository.DEFAULT_TTS_PROVIDER_ID,
            repo2.ttsProviderId
        )
    }

    // ── Live2D 形象（selectedModelId）──

    @Test
    fun selectedModelId_defaultsToBai() {
        val repo = SettingsRepository(context)
        // 契约 v0 AD-02：DEFAULT_MODEL_ID 同步为 bai（默认形象 = bai 白-免费版）
        assertEquals("首次启动默认形象应为 bai", "bai", SettingsRepository.DEFAULT_MODEL_ID)
        assertEquals("首次启动默认形象应为 bai（DEFAULT_MODEL_ID，D2）", SettingsRepository.DEFAULT_MODEL_ID, repo.selectedModelId)
    }

    @Test
    fun selectedModelId_persistsAcrossRestart() {
        val repo = SettingsRepository(context)
        repo.selectedModelId = "pc_only"
        // 重新创建实例（模拟重启），值应仍为 pc_only
        val repo2 = SettingsRepository(context)
        assertEquals("切换后应持久化，重启仍为 pc_only", "pc_only", repo2.selectedModelId)
    }

    @Test
    fun selectedModelId_resetAll_restoresDefault() {
        val repo = SettingsRepository(context)
        repo.selectedModelId = "pc_only"
        repo.resetAll()
        val repo2 = SettingsRepository(context)
        assertEquals("resetAll 后应回退默认形象", SettingsRepository.DEFAULT_MODEL_ID, repo2.selectedModelId)
    }
}
