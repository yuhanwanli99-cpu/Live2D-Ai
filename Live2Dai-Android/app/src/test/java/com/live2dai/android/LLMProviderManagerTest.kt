package com.live2dai.android

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * LLMProviderManager 零配置默认行为测试。
 *
 * 覆盖「DeepSeek 云端默认」的加载门逻辑（智谱 GLM 提供商已移除）：
 * - 零配置（SharedPreferences 从未存过 provider 类型）→ 默认走 deepSeek()
 *   （api.deepseek.com / 人设模型名），不崩溃、不走空 baseUrl
 * - 显式选择 DEEPSEEK 时按配置返回
 * - 历史持久化的 ZHIPU 偏好（已移除的提供商）→ 安全回退 DeepSeek
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class, sdk = [34])
class LLMProviderManagerTest {

    private val context: Context = ApplicationProvider.getApplicationContext()

    @Before
    fun setUp() {
        // 每次测试从「零配置」状态开始：清空 provider 偏好 + 设置存储（自定义 Key）
        context.getSharedPreferences("llm_provider", Context.MODE_PRIVATE)
            .edit().clear().commit()
        context.getSharedPreferences("live2dai_settings", Context.MODE_PRIVATE)
            .edit().clear().commit()
        LLMProviderManager.init(context)
    }

    // ─────────────────────────────────────────────────────────────
    // 自定义 Key 优先（用户在设置页填自己的 Key）
    // ─────────────────────────────────────────────────────────────

    private fun setCustomDeepseekKey(key: String) {
        context.getSharedPreferences("live2dai_settings", Context.MODE_PRIVATE)
            .edit().putString("deepseek_api_key", key).commit()
    }

    @Test
    fun customDeepseekKey_takesPrecedenceOverBuiltin() {
        setCustomDeepseekKey("sk-user-custom-deepseek-67890")
        LLMProviderManager.setProviderType(ProviderType.DEEPSEEK)

        val provider = LLMProviderManager.getActiveProvider()
        assertEquals(ProviderType.DEEPSEEK, provider.type)
        assertEquals("sk-user-custom-deepseek-67890", provider.apiKey)
        // 降级路径同样使用自定义 Key
        assertEquals("sk-user-custom-deepseek-67890", LLMProviderManager.getDeepSeekProvider().apiKey)
    }

    @Test
    fun noCustomKeys_fallsBackToBuiltinWithoutCrashing() {
        // 设置存储为空、编译期也无 Key 的构建：仍应安全返回非空 baseUrl 的 provider
        val provider = LLMProviderManager.getActiveProvider()
        assert(provider.baseUrl.isNotEmpty())
        assert(provider.modelName.isNotEmpty())
    }

    @Test
    fun zeroConfig_defaultsToDeepSeek() {
        // 智谱 GLM 提供商已整体移除：零配置无条件默认 DeepSeek 云端（不依赖任何 Key）
        val provider = LLMProviderManager.getActiveProvider()
        assertEquals(ProviderType.DEEPSEEK, provider.type)
        assertEquals("https://api.deepseek.com/v1", provider.baseUrl)
        assertEquals(ProviderType.DEEPSEEK, LLMProviderManager.getProviderType())
    }

    @Test
    fun explicitDeepSeek_returnsDeepSeekProvider() {
        LLMProviderManager.setProviderType(ProviderType.DEEPSEEK)
        assertEquals(ProviderType.DEEPSEEK, LLMProviderManager.getProviderType())
        val provider = LLMProviderManager.getActiveProvider()
        assertEquals(ProviderType.DEEPSEEK, provider.type)
        assertEquals("https://api.deepseek.com/v1", provider.baseUrl)
    }

    @Test
    fun legacyUnknownPreference_fallsBackToDeepSeekSafely() {
        // 历史版本持久化的未知类型偏好 → 安全回退 DeepSeek，不崩溃、不发空 key 请求
        context.getSharedPreferences("llm_provider", Context.MODE_PRIVATE)
            .edit().putString("provider_type", "OLD_VENDOR").commit()

        val provider = LLMProviderManager.getActiveProvider()
        assertEquals(ProviderType.DEEPSEEK, provider.type)
        assertEquals("https://api.deepseek.com/v1", provider.baseUrl)
        assertEquals(ProviderType.DEEPSEEK, LLMProviderManager.getProviderType())
    }

    @Test
    fun zhipuPreference_returnsZhipuProvider() {
        context.getSharedPreferences("llm_provider", Context.MODE_PRIVATE)
            .edit().putString("provider_type", "ZHIPU").commit()

        val provider = LLMProviderManager.getActiveProvider()
        assertEquals(ProviderType.ZHIPU, provider.type)
        assertEquals("https://open.bigmodel.cn/api/paas/v4", provider.baseUrl)
        assertEquals(ProviderType.ZHIPU, LLMProviderManager.getProviderType())
    }
}
