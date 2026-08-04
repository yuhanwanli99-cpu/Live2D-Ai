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
 * 覆盖「智谱 GLM-4.7-Flash 免费兜底」的加载门逻辑：
 * - 零配置（SharedPreferences 从未存过 provider 类型）+ 编译期有 ZHIPU_API_KEY
 *   → 默认走 zhipu()（glm-4.7-flash / open.bigmodel.cn）
 * - 零配置 + 无 ZHIPU_API_KEY（其他开发者/CI 构建）→ 安全回退 deepSeek()，不崩溃、不走空 baseUrl
 * - 显式选择 DEEPSEEK / ZHIPU 时按配置返回（无 key 的 ZHIPU 选择同样安全回退）
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
    // 自定义 Key 优先（共享免费 Key 额度耗尽后，用户在设置页填自己的 Key）
    // ─────────────────────────────────────────────────────────────

    private fun setCustomZhipuKey(key: String) {
        context.getSharedPreferences("live2dai_settings", Context.MODE_PRIVATE)
            .edit().putString("zhipu_api_key", key).commit()
    }

    private fun setCustomDeepseekKey(key: String) {
        context.getSharedPreferences("live2dai_settings", Context.MODE_PRIVATE)
            .edit().putString("deepseek_api_key", key).commit()
    }

    @Test
    fun customZhipuKey_takesPrecedenceOverBuiltin_forZeroConfigDefault() {
        // 用户填了自己的 Key（即使编译期内置 Key 也存在，也应优先用自定义 Key）
        setCustomZhipuKey("sk-user-custom-zhipu-12345")

        val provider = LLMProviderManager.getActiveProvider()
        assertEquals(ProviderType.ZHIPU, provider.type)
        assertEquals("sk-user-custom-zhipu-12345", provider.apiKey)
        assertEquals("glm-4.7-flash", provider.modelName)
    }

    @Test
    fun customZhipuKey_usedForExplicitZhipuSelection() {
        setCustomZhipuKey("sk-user-custom-zhipu-12345")
        LLMProviderManager.setProviderType(ProviderType.ZHIPU)

        val provider = LLMProviderManager.getActiveProvider()
        assertEquals(ProviderType.ZHIPU, provider.type)
        assertEquals("sk-user-custom-zhipu-12345", provider.apiKey)
    }

    @Test
    fun customZhipuKey_letsZeroConfigUseZhipu_evenWithoutBuiltinKey() {
        // 编译期没有内置 Key 时，只要用户填了自定义 Zhipu Key，零配置默认仍走 Zhipu
        setCustomZhipuKey("sk-user-custom-zhipu-12345")
        // 不依赖 BuildConfig：直接断言返回的是 Zhipu 且用自定义 Key
        val provider = LLMProviderManager.getActiveProvider()
        assertEquals(ProviderType.ZHIPU, provider.type)
        assertEquals("sk-user-custom-zhipu-12345", provider.apiKey)
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
    fun zeroConfig_defaultsToZhipuWhenKeyPresent() {
        val provider = LLMProviderManager.getActiveProvider()
        if (BuildConfig.ZHIPU_API_KEY.isNotEmpty()) {
            // 本机已配置 ZHIPU_API_KEY：零配置默认应为智谱 GLM 免费兜底
            assertEquals(ProviderType.ZHIPU, provider.type)
            assertEquals("glm-4.7-flash", provider.modelName)
            assertEquals("https://open.bigmodel.cn/api/paas/v4", provider.baseUrl)
            assertEquals(ProviderType.ZHIPU, LLMProviderManager.getProviderType())
        } else {
            // 未配置 key 的构建：安全回退 DeepSeek，且 baseUrl 非空（不崩溃）
            assertEquals(ProviderType.DEEPSEEK, provider.type)
            assertEquals("https://api.deepseek.com/v1", provider.baseUrl)
        }
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
    fun explicitZhipu_returnsZhipuWhenKeyPresent_elseFallsBackSafely() {
        LLMProviderManager.setProviderType(ProviderType.ZHIPU)
        assertEquals(ProviderType.ZHIPU, LLMProviderManager.getProviderType())
        val provider = LLMProviderManager.getActiveProvider()
        if (BuildConfig.ZHIPU_API_KEY.isNotEmpty()) {
            assertEquals(ProviderType.ZHIPU, provider.type)
            assertEquals("glm-4.7-flash", provider.modelName)
        } else {
            // 无 key 时配置的 ZHIPU 选择同样安全回退，避免空 key 请求
            assertEquals(ProviderType.DEEPSEEK, provider.type)
            assertEquals("https://api.deepseek.com/v1", provider.baseUrl)
        }
    }
}
