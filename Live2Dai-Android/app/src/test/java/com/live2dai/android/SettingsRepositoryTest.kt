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
    fun proactiveEnabled_defaultsToTrue_andPersists() {
        val repo = SettingsRepository(context)
        assertTrue("主动陪伴默认开启（schema KEY_PROACTIVE_ENABLED default: true）", repo.proactiveEnabled)

        repo.proactiveEnabled = false
        val repo2 = SettingsRepository(context)
        assertFalse("关闭后应持久化（重启后不装载主动陪伴插件）", repo2.proactiveEnabled)

        repo2.proactiveEnabled = true
        val repo3 = SettingsRepository(context)
        assertTrue("重新开启应持久化", repo3.proactiveEnabled)
    }

    // ── TTS Provider（V1.2 模块1）──

    @Test
    fun ttsProviderId_defaultsToCosyVoice() {
        val repo = SettingsRepository(context)
        assertEquals(
            "默认首选应为降级链第一位 cosyvoice_ws（schema 契约 §4.3）",
            SettingsRepository.DEFAULT_TTS_PROVIDER_ID,
            repo.ttsProviderId
        )
        assertEquals("cosyvoice_ws", repo.ttsProviderId)
    }

    @Test
    fun ttsProviderId_persistsAcrossRestart() {
        val repo = SettingsRepository(context)
        repo.ttsProviderId = "system_tts"
        // 重新创建实例（模拟重启），值应仍为 system_tts
        val repo2 = SettingsRepository(context)
        assertEquals("切换后应持久化，重启仍为 system_tts", "system_tts", repo2.ttsProviderId)
    }

    @Test
    fun ttsProviderId_resetAll_restoresDefault() {
        val repo = SettingsRepository(context)
        repo.ttsProviderId = "cosyvoice_ws"
        repo.resetAll()
        val repo2 = SettingsRepository(context)
        assertEquals(
            "resetAll 后应回退默认首选引擎",
            SettingsRepository.DEFAULT_TTS_PROVIDER_ID,
            repo2.ttsProviderId
        )
    }

    // ── ASR Provider（V1.2 模块4）──

    @Test
    fun asrProviderId_defaultsToSherpaSensevoice() {
        val repo = SettingsRepository(context)
        assertEquals(
            "默认首选 ASR 应为离线 sherpa_sensevoice（plan §4.3）",
            SettingsRepository.DEFAULT_ASR_PROVIDER_ID,
            repo.asrProviderId
        )
        assertEquals("sherpa_sensevoice", repo.asrProviderId)
    }

    @Test
    fun asrProviderId_persistsAcrossRestart() {
        val repo = SettingsRepository(context)
        repo.asrProviderId = "system_speech_recognizer"
        val repo2 = SettingsRepository(context)
        assertEquals("切换后应持久化，重启仍为 system_speech_recognizer", "system_speech_recognizer", repo2.asrProviderId)
    }

    @Test
    fun asrProviderId_resetAll_restoresDefault() {
        val repo = SettingsRepository(context)
        repo.asrProviderId = "system_speech_recognizer"
        repo.resetAll()
        val repo2 = SettingsRepository(context)
        assertEquals(
            "resetAll 后应回退默认离线 ASR",
            SettingsRepository.DEFAULT_ASR_PROVIDER_ID,
            repo2.asrProviderId
        )
    }

    // ── CosyVoice API Key（V1.2 模块2）──

    @Test
    fun aliyunApiKey_defaultsToEmpty() {
        val repo = SettingsRepository(context)
        assertEquals(
            "首次启动 aliyunApiKey 应为空（未配置）",
            SettingsRepository.DEFAULT_COSYVOICE_API_KEY,
            repo.aliyunApiKey
        )
    }

    @Test
    fun aliyunApiKey_persistsAcrossRestart() {
        val repo = SettingsRepository(context)
        repo.aliyunApiKey = "sk-test-123"
        // 重新创建实例（模拟重启），值应仍为 sk-test-123
        val repo2 = SettingsRepository(context)
        assertEquals("设置后应持久化，重启仍为 sk-test-123", "sk-test-123", repo2.aliyunApiKey)
    }

    @Test
    fun aliyunApiKey_resetAll_restoresDefault() {
        val repo = SettingsRepository(context)
        repo.aliyunApiKey = "sk-test"
        repo.resetAll()
        val repo2 = SettingsRepository(context)
        assertEquals("resetAll 后应回退默认值", SettingsRepository.DEFAULT_COSYVOICE_API_KEY, repo2.aliyunApiKey)
    }

    // ── CosyVoice WS 端点（V1.2 模块2 专属服务可选配置）──

    @Test
    fun cosyvoiceWsUrl_defaultsToEmpty() {
        val repo = SettingsRepository(context)
        assertEquals(
            "首次启动 cosyvoiceWsUrl 应为空（= 使用默认公共 inference 端点）",
            SettingsRepository.DEFAULT_COSYVOICE_WS_URL,
            repo.cosyvoiceWsUrl
        )
    }

    @Test
    fun cosyvoiceWsUrl_persistsAcrossRestart() {
        val repo = SettingsRepository(context)
        repo.cosyvoiceWsUrl = "wss://ws-s8xxx.cn-beijing.maas.aliyuncs.com/api-ws/v1/inference"
        val repo2 = SettingsRepository(context)
        assertEquals(
            "设置后应持久化，重启仍为专属端点",
            "wss://ws-s8xxx.cn-beijing.maas.aliyuncs.com/api-ws/v1/inference",
            repo2.cosyvoiceWsUrl
        )
    }

    @Test
    fun cosyvoiceWsUrl_resetAll_restoresDefault() {
        val repo = SettingsRepository(context)
        repo.cosyvoiceWsUrl = "wss://custom.example.com/api-ws/v1/inference"
        repo.resetAll()
        val repo2 = SettingsRepository(context)
        assertEquals("resetAll 后应回退默认值", SettingsRepository.DEFAULT_COSYVOICE_WS_URL, repo2.cosyvoiceWsUrl)
    }

    // ── CosyVoice 个性化参数（级2-C：instruction / rate / pitch）──

    @Test
    fun cosyvoiceInstruction_defaultsToCatgirlTone() {
        val repo = SettingsRepository(context)
        assertEquals(
            "默认 instruction 应为少女音语气（级2-C）",
            SettingsRepository.DEFAULT_COSYVOICE_INSTRUCTION,
            repo.cosyvoiceInstruction
        )
        assertEquals("用清澈甜美的少女音说话，声音甜美带一点傲娇", repo.cosyvoiceInstruction)
    }

    @Test
    fun cosyvoiceInstruction_persistsAcrossRestart() {
        val repo = SettingsRepository(context)
        repo.cosyvoiceInstruction = "用御姐音说话"
        // 重新创建实例（模拟重启），值应仍为自定义语气
        val repo2 = SettingsRepository(context)
        assertEquals("设置后应持久化，重启仍为自定义语气", "用御姐音说话", repo2.cosyvoiceInstruction)
    }

    @Test
    fun cosyvoiceInstruction_resetAll_restoresDefault() {
        val repo = SettingsRepository(context)
        repo.cosyvoiceInstruction = "自定义语气"
        repo.resetAll()
        val repo2 = SettingsRepository(context)
        assertEquals(
            "resetAll 后应回退默认语气",
            SettingsRepository.DEFAULT_COSYVOICE_INSTRUCTION,
            repo2.cosyvoiceInstruction
        )
    }

    @Test
    fun cosyvoiceRate_defaultsTo1_2() {
        val repo = SettingsRepository(context)
        assertEquals(
            "默认语速应为 1.2（略快有活力感）",
            SettingsRepository.DEFAULT_COSYVOICE_RATE,
            repo.cosyvoiceRate,
            0.0f
        )
        assertEquals(1.2f, repo.cosyvoiceRate, 0.0f)
    }

    @Test
    fun cosyvoiceRate_persistsAcrossRestart() {
        val repo = SettingsRepository(context)
        repo.cosyvoiceRate = 0.8f
        // 重新创建实例（模拟重启），值应仍为 0.8
        val repo2 = SettingsRepository(context)
        assertEquals("设置后应持久化，重启仍为 0.8", 0.8f, repo2.cosyvoiceRate, 0.0f)
    }

    @Test
    fun cosyvoiceRate_resetAll_restoresDefault() {
        val repo = SettingsRepository(context)
        repo.cosyvoiceRate = 0.5f
        repo.resetAll()
        val repo2 = SettingsRepository(context)
        assertEquals(
            "resetAll 后应回退默认语速",
            SettingsRepository.DEFAULT_COSYVOICE_RATE,
            repo2.cosyvoiceRate,
            0.0f
        )
    }

    @Test
    fun cosyvoicePitch_defaultsTo1_1() {
        val repo = SettingsRepository(context)
        assertEquals(
            "默认音调应为 1.1（升高显得更萌）",
            SettingsRepository.DEFAULT_COSYVOICE_PITCH,
            repo.cosyvoicePitch,
            0.0f
        )
        assertEquals(1.1f, repo.cosyvoicePitch, 0.0f)
    }

    @Test
    fun cosyvoicePitch_persistsAcrossRestart() {
        val repo = SettingsRepository(context)
        repo.cosyvoicePitch = 1.5f
        // 重新创建实例（模拟重启），值应仍为 1.5
        val repo2 = SettingsRepository(context)
        assertEquals("设置后应持久化，重启仍为 1.5", 1.5f, repo2.cosyvoicePitch, 0.0f)
    }

    @Test
    fun cosyvoicePitch_resetAll_restoresDefault() {
        val repo = SettingsRepository(context)
        repo.cosyvoicePitch = 2.0f
        repo.resetAll()
        val repo2 = SettingsRepository(context)
        assertEquals(
            "resetAll 后应回退默认音调",
            SettingsRepository.DEFAULT_COSYVOICE_PITCH,
            repo2.cosyvoicePitch,
            0.0f
        )
    }

    // ── Live2D 形象（selectedModelId）──

    @Test
    fun selectedModelId_defaultsToBai() {
        val repo = SettingsRepository(context)
        // 契约 v0 AD-02：DEFAULT_MODEL_ID 同步为 bai（默认形象 = bai 白-免费版）
        assertEquals("首次启动默认形象应为 bai", "bai", SettingsRepository.DEFAULT_MODEL_ID)
        assertEquals("首次启动默认形象应为 niziiro_mao", SettingsRepository.DEFAULT_MODEL_ID, repo.selectedModelId)
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
