package com.live2dai.android

import android.content.Context
import android.content.SharedPreferences
import android.widget.Toast
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import java.util.concurrent.TimeUnit

/**
 * 健康检查结果
 */
sealed class HealthResult {
    /** 连接成功 */
    data object Ok : HealthResult()

    /** 连接失败 */
    data class Error(
        val message: String,
        val code: Int? = null
    ) : HealthResult()
}

/**
 * LLM Provider 运行时管理器
 *
 * 职责：
 * - Provider 切换与持久化
 * - 本地 LLM 健康检查
 * - 自动降级逻辑（仅本次请求，不修改持久化偏好）
 */
object LLMProviderManager {

    // ── SharedPreferences ──
    private const val PREFS_NAME = "llm_provider"

    // 用户自定义 API Key 所在的设置存储（与 SettingsRepository 共用同一份 prefs）
    private const val SETTINGS_PREFS_NAME = "live2dai_settings"
    private const val SETTINGS_KEY_DEEPSEEK = "deepseek_api_key"
    private const val SETTINGS_KEY_ZHIPU = "zhipu_api_key"

    private const val KEY_TYPE = "provider_type"
    private const val KEY_LOCAL_URL = "local_base_url"
    private const val KEY_LOCAL_KEY = "local_api_key"
    private const val KEY_LOCAL_MODEL = "local_model"

    private const val TYPE_DEEPSEEK = "DEEPSEEK"
    private const val TYPE_ZHIPU = "ZHIPU"
    private const val TYPE_LOCAL = "LOCAL"

    // 默认本地配置示例（空字符串表示未配置）
    private const val EMPTY = ""

    // ── 健康检查超时 ──
    private const val HEALTH_CHECK_CONNECT_TIMEOUT = 5L
    private const val HEALTH_CHECK_READ_TIMEOUT = 5L

    // ── 存储 ──
    private lateinit var prefs: SharedPreferences
    private lateinit var settingsPrefs: SharedPreferences

    // 健康检查专用 Client（短超时）
    private val healthClient = OkHttpClient.Builder()
        .connectTimeout(HEALTH_CHECK_CONNECT_TIMEOUT, TimeUnit.SECONDS)
        .readTimeout(HEALTH_CHECK_READ_TIMEOUT, TimeUnit.SECONDS)
        .build()

    // ── API ──

    /**
     * 冷启动初始化。应在 Application.onCreate 或 Activity.onCreate 中调用。
     */
    fun init(context: Context) {
        prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
        settingsPrefs = context.getSharedPreferences(SETTINGS_PREFS_NAME, Context.MODE_PRIVATE)
    }

    // ── 自定义 Key（用户在设置页填写，优先于内置/编译期 Key） ──

    /** 用户在设置页填写的自定义 Zhipu Key（未填写返回空串） */
    private fun customZhipuKey(): String =
        settingsPrefs.getString(SETTINGS_KEY_ZHIPU, EMPTY) ?: EMPTY

    /** 用户在设置页填写的自定义 DeepSeek Key（未填写返回空串） */
    private fun customDeepseekKey(): String =
        settingsPrefs.getString(SETTINGS_KEY_DEEPSEEK, EMPTY) ?: EMPTY

    /** Zhipu Key 可用性：自定义 Key 或编译期内置 Key 任一存在即视为可用 */
    private fun zhipuKeyAvailable(): Boolean =
        customZhipuKey().isNotEmpty() ||
            BuildConfig.ZHIPU_API_KEY.isNotEmpty() ||
            !(System.getenv("ZHIPU_API_KEY") ?: "").isNullOrEmpty()

    /**
     * 构造 Zhipu Provider：优先用户自定义 Key，其次编译期内置共享 Key（free_glm_defaults
     * 的 Android 对应物），再次环境变量。
     */
    private fun zhipuProvider(): LLMProvider = LLMProvider(
        type = ProviderType.ZHIPU,
        baseUrl = "https://open.bigmodel.cn/api/paas/v4",
        apiKey = customZhipuKey()
            .ifEmpty { BuildConfig.ZHIPU_API_KEY }
            .ifEmpty { System.getenv("ZHIPU_API_KEY") ?: "" },
        modelName = "glm-4.7-flash",
        displayName = "智谱 GLM（免费）"
    )

    /** 构造 DeepSeek Provider：优先用户自定义 Key，其次编译期/环境变量 Key */
    private fun deepSeekProvider(): LLMProvider = LLMProvider(
        type = ProviderType.DEEPSEEK,
        baseUrl = "https://api.deepseek.com/v1",
        apiKey = customDeepseekKey()
            .ifEmpty { BuildConfig.DEEPSEEK_API_KEY }
            .ifEmpty { System.getenv("DEEPSEEK_API_KEY") ?: "" },
        modelName = PersonaConfig.getModel(),
        displayName = "DeepSeek 云端"
    )

    /**
     * 获取当前选中的 Provider。
     *
     * 零配置（SharedPreferences 从未保存过 provider 类型）时默认返回智谱 GLM 免费
     * 兜底模型（LLMProvider.zhipu()）；若编译时未配置 ZHIPU_API_KEY（如其他开发者
     * 本地构建），安全回退到 DeepSeek，避免无 key/空 baseUrl 的坏默认值。
     * 如果类型为 LOCAL 但本地配置不完整（URL 为空），返回 DeepSeek。
     *
     * Key 解析优先级：用户在设置页填写的自定义 Key > 编译期内置 Key > 环境变量。
     * 即共享免费 Key 额度耗尽后，用户在设置页填入自己的 Key，下一次请求即可生效。
     */
    fun getActiveProvider(): LLMProvider {
        // 零配置：DeepSeek key 存在优先用（debug 内置版），否则智谱 GLM 免费兜底
        if (!prefs.contains(KEY_TYPE)) {
            return if (BuildConfig.DEEPSEEK_API_KEY.isNotEmpty()) deepSeekProvider()
                   else if (zhipuKeyAvailable()) zhipuProvider()
                   else deepSeekProvider()
        }
        val type = prefs.getString(KEY_TYPE, TYPE_DEEPSEEK) ?: TYPE_DEEPSEEK
        return when (type) {
            TYPE_ZHIPU -> {
                // 显式选了 ZHIPU 但没有 Key → 安全回退，避免空 key 请求
                if (zhipuKeyAvailable()) zhipuProvider() else deepSeekProvider()
            }
            TYPE_LOCAL -> {
                val url = prefs.getString(KEY_LOCAL_URL, EMPTY) ?: EMPTY
                if (url.isBlank()) {
                    // 配置不完整，安全回退
                    deepSeekProvider()
                } else {
                    LLMProvider(
                        type = ProviderType.LOCAL,
                        baseUrl = url.trimEnd('/'),
                        apiKey = prefs.getString(KEY_LOCAL_KEY, EMPTY) ?: EMPTY,
                        modelName = prefs.getString(KEY_LOCAL_MODEL, EMPTY) ?: EMPTY,
                        displayName = "本地 LLM"
                    )
                }
            }
            else -> deepSeekProvider()
        }
    }

    /**
     * 内置 DeepSeek Provider（用于降级；同样优先用户自定义 Key）
     */
    fun getDeepSeekProvider(): LLMProvider = deepSeekProvider()

    /**
     * 用户配置的本地 Provider，未配置时返回 null
     */
    fun getLocalProvider(): LLMProvider? {
        val url = prefs.getString(KEY_LOCAL_URL, EMPTY) ?: EMPTY
        if (url.isBlank()) return null

        return LLMProvider(
            type = ProviderType.LOCAL,
            baseUrl = url.trimEnd('/'),
            apiKey = prefs.getString(KEY_LOCAL_KEY, EMPTY) ?: EMPTY,
            modelName = prefs.getString(KEY_LOCAL_MODEL, EMPTY) ?: EMPTY,
            displayName = "本地 LLM"
        )
    }

    /**
     * 持久化本地 LLM 配置
     */
    fun saveLocalConfig(url: String, key: String, model: String) {
        prefs.edit()
            .putString(KEY_LOCAL_URL, url.trimEnd('/'))
            .putString(KEY_LOCAL_KEY, key)
            .putString(KEY_LOCAL_MODEL, model)
            .apply()
    }

    /**
     * 切换 Provider 类型（DEEPSEEK / ZHIPU / LOCAL）
     * 不修改本地连接参数。
     */
    fun setProviderType(type: ProviderType) {
        prefs.edit()
            .putString(KEY_TYPE, when (type) {
                ProviderType.DEEPSEEK -> TYPE_DEEPSEEK
                ProviderType.ZHIPU -> TYPE_ZHIPU
                ProviderType.LOCAL -> TYPE_LOCAL
            })
            .apply()
    }

    /**
     * 获取当前配置的 Provider 类型（与实际生效的 provider 一致：
     * 零配置时返回免费兜底 provider）。
     */
    fun getProviderType(): ProviderType {
        // 与 getActiveProvider() 保持一致：零配置时返回实际生效的兜底 provider
        if (!prefs.contains(KEY_TYPE)) {
            return if (zhipuKeyAvailable()) ProviderType.ZHIPU else ProviderType.DEEPSEEK
        }
        val type = prefs.getString(KEY_TYPE, TYPE_DEEPSEEK) ?: TYPE_DEEPSEEK
        return when (type) {
            TYPE_ZHIPU -> ProviderType.ZHIPU
            TYPE_LOCAL -> ProviderType.LOCAL
            else -> ProviderType.DEEPSEEK
        }
    }

    /**
     * 健康检查：尝试连接本地 LLM 的 /v1/models 端点
     *
     * OpenAI 兼容端点返回格式：
     *   GET {baseUrl}/models  → 200 OK 表示服务可用
     *   或
     *   GET {baseUrl}/v1/models → 200 OK (vLLM 默认)
     *
     * @param url 本地 LLM 的 Base URL
     * @return HealthResult.Ok 或 HealthResult.Error
     */
    suspend fun healthCheck(url: String): HealthResult = withContext(Dispatchers.IO) {
        val baseUrl = url.trimEnd('/')

        // 先尝试 /v1/models（标准 OpenAI 端点）
        val endpoints = listOf("$baseUrl/v1/models", "$baseUrl/models")
        for (endpoint in endpoints) {
            try {
                val request = Request.Builder()
                    .url(endpoint)
                    .addHeader("Accept", "application/json")
                    .build()

                val response = healthClient.newCall(request).execute()
                return@withContext if (response.isSuccessful) {
                    HealthResult.Ok
                } else {
                    // 尝试下一个端点
                    continue
                }
            } catch (e: Exception) {
                // 网络错误，尝试下一个端点
                continue
            }
        }
        // 所有端点都失败
        HealthResult.Error(
            message = "连接失败，请检查 Base URL 和网络连接",
            code = null
        )
    }

    /**
     * 获取带降级的 Provider（suspend 版本，自动健康检查）
     *
     * 当 LOCAL 不可用时，自动降级到 DeepSeek 并返回降级标记。
     *
     * @param context 用于显示 Toast
     * @return 降级后的 LLMProvider（已通过健康检查，或已回退到 DeepSeek）
     */
    suspend fun getActiveProviderWithFallback(context: Context): LLMProvider {
        val provider = getActiveProvider()
        if (provider.type != ProviderType.LOCAL) return provider

        // 本地 Provider 需要健康检查
        val health = healthCheck(provider.baseUrl)
        if (health is HealthResult.Ok) return provider

        // 健康检查失败，降级到 DeepSeek（仅本次）
        val reason = (health as HealthResult.Error).message
        withContext(Dispatchers.Main) {
            Toast.makeText(
                context,
                "本地 LLM 不可用 ($reason)，已自动切换至云端 DeepSeek",
                Toast.LENGTH_SHORT
            ).show()
        }
        return getDeepSeekProvider()
    }
}
