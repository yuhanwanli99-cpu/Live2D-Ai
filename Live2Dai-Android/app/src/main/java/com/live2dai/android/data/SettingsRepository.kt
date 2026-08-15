package com.live2dai.android.data

import android.content.Context
import android.content.SharedPreferences
import com.live2dai.android.BuildConfig
import com.live2dai.android.core.RenderQuality
import com.live2dai.android.core.RenderQualityConfig

/**
 * 设置持久化层，使用 SharedPreferences。
 * API Key 优先使用 UI 中填入的自定义值，为空时回退到 BuildConfig 编译值。
 */
class SettingsRepository(context: Context) {

    private val prefs: SharedPreferences =
        context.getSharedPreferences(PREF_NAME, Context.MODE_PRIVATE)

    // ─── API Keys ───

    /** DeepSeek API Key (自定义覆盖) */
    var deepseekApiKey: String
        get() = prefs.getString(KEY_DEEPSEEK, null)
            ?: BuildConfig.DEEPSEEK_API_KEY
        set(value) = prefs.edit().putString(KEY_DEEPSEEK, value).apply()

    /** 用户是否手动输入过自定义 Key */
    val hasCustomDeepseekKey: Boolean
        get() = prefs.contains(KEY_DEEPSEEK) && prefs.getString(KEY_DEEPSEEK, null)!!.isNotEmpty()

    fun clearCustomDeepseekKey() {
        prefs.edit().remove(KEY_DEEPSEEK).apply()
    }

    // ─── Model ───

    var model: String
        get() = prefs.getString(KEY_MODEL, DEFAULT_MODEL) ?: DEFAULT_MODEL
        set(value) = prefs.edit().putString(KEY_MODEL, value).apply()

    var temperature: Float
        get() = prefs.getFloat(KEY_TEMPERATURE, DEFAULT_TEMPERATURE)
        set(value) = prefs.edit().putFloat(KEY_TEMPERATURE, value).apply()

    // ─── TTS ───

    var ttsVoice: String
        get() = prefs.getString(KEY_TTS_VOICE, DEFAULT_TTS_VOICE) ?: DEFAULT_TTS_VOICE
        set(value) = prefs.edit().putString(KEY_TTS_VOICE, value).apply()

    var ttsSpeed: Float
        get() = prefs.getFloat(KEY_TTS_SPEED, DEFAULT_TTS_SPEED)
        set(value) = prefs.edit().putFloat(KEY_TTS_SPEED, value).apply()

    // ─── TTS Provider（V1.2 模块1，契约 schema → Settings 新增 Keys）───

    /**
     * 首选 TTS 引擎 ID（降级链遍历时的首选，对应 TtsProviderInfo.id）。
     *
     * 默认取降级链第一位 cosyvoice_ws（schema 契约 §4.3；CosyVoice 已注册 priority=0）。
     * API Key 未配置时 CosyVoice 快速失败（health=UNAVAILABLE），降级链自动落入下一层，
     * 不阻塞播报（plan §3.2.5）。引擎选择 UI（V1.2 模块5）在设置页修改此值。
     */
    var ttsProviderId: String
        get() = prefs.getString(KEY_TTS_PROVIDER_ID, DEFAULT_TTS_PROVIDER_ID)
            ?: DEFAULT_TTS_PROVIDER_ID
        set(value) = prefs.edit().putString(KEY_TTS_PROVIDER_ID, value).apply()

    // ─── CosyVoice（V1.2 模块2：阿里云 DashScope API Key + WS 端点）───

    /**
     * 阿里云 DashScope API Key（CosyVoice 云端 TTS 用；设置页输入，默认取 BuildConfig）。
     * 为空时 CosyVoiceTtsProvider.speak 快速失败（health=UNAVAILABLE），降级链继续。
     * 设置页输入入口见 SettingsScreen（DashScope Key + WS 地址输入区）。
     */
    var aliyunApiKey: String
        get() = prefs.getString(KEY_COSYVOICE_API_KEY, DEFAULT_COSYVOICE_API_KEY)
            ?: DEFAULT_COSYVOICE_API_KEY
        set(value) = prefs.edit().putString(KEY_COSYVOICE_API_KEY, value).apply()

    /**
     * CosyVoice WebSocket 端点（可选配置；空 = 使用默认公共端点
     * `wss://dashscope.aliyuncs.com/api-ws/v1/inference`）。
     *
     * 专属服务用户的 host 是用户专属的（如 `ws-sxxx.cn-beijing.maas.aliyuncs.com`），
     * 不能硬编码进默认值；填写后 provider 以 `wss://{host}/api-ws/v1/inference` 接入。
     * 留空时回退标准公共端点（普通用户 key 适用）。默认值取 BuildConfig.COSYVOICE_WS_URL。
     */
    var cosyvoiceWsUrl: String
        get() = prefs.getString(KEY_COSYVOICE_WS_URL, DEFAULT_COSYVOICE_WS_URL)
            ?: DEFAULT_COSYVOICE_WS_URL
        set(value) = prefs.edit().putString(KEY_COSYVOICE_WS_URL, value).apply()

    // ─── CosyVoice 个性化参数（级2-C：语气 instruction + 语速 rate + 音调 pitch）───

    /**
     * CosyVoice instruction（语气/方言/情绪描述，SDK 限制 ≤100 字符；
     * run-task parameters.instruction 由 CosyVoiceTtsProvider 构造时读取）。
     * 默认值：清澈甜美少女音。
     */
    var cosyvoiceInstruction: String
        get() = prefs.getString(KEY_COSYVOICE_INSTRUCTION, DEFAULT_COSYVOICE_INSTRUCTION)
            ?: DEFAULT_COSYVOICE_INSTRUCTION
        set(value) = prefs.edit().putString(KEY_COSYVOICE_INSTRUCTION, value).apply()

    /** CosyVoice 语速倍率（run-task parameters.rate，默认 1.2 略快有活力感） */
    var cosyvoiceRate: Float
        get() = prefs.getFloat(KEY_COSYVOICE_RATE, DEFAULT_COSYVOICE_RATE)
        set(value) = prefs.edit().putFloat(KEY_COSYVOICE_RATE, value).apply()

    /** CosyVoice 音调倍率（run-task parameters.pitch，默认 1.1 升高显得更萌） */
    var cosyvoicePitch: Float
        get() = prefs.getFloat(KEY_COSYVOICE_PITCH, DEFAULT_COSYVOICE_PITCH)
        set(value) = prefs.edit().putFloat(KEY_COSYVOICE_PITCH, value).apply()

    // ─── Persona ───

    var personaName: String
        get() = prefs.getString(KEY_PERSONA, DEFAULT_PERSONA) ?: DEFAULT_PERSONA
        set(value) = prefs.edit().putString(KEY_PERSONA, value).apply()

    // ─── Live2D 形象（模型注册表） ───

    /**
     * 用户选中的 Live2D 模型 ID，与 shared/model_registry.json 的 id 字段对应。
     * 默认值 "bai" 与注册表 default 条目一致（D2：bai 唯一默认形象）。
     */
    var selectedModelId: String
        get() = prefs.getString(KEY_SELECTED_MODEL_ID, DEFAULT_MODEL_ID) ?: DEFAULT_MODEL_ID
        set(value) = prefs.edit().putString(KEY_SELECTED_MODEL_ID, value).apply()

    // ─── 免费共享额度提示（首次启动一次性提示，可关闭） ───

    var freeQuotaNoticeDismissed: Boolean
        get() = prefs.getBoolean(KEY_FREE_QUOTA_NOTICE, false)
        set(value) = prefs.edit().putBoolean(KEY_FREE_QUOTA_NOTICE, value).apply()

    // ─── 首次启动引导（onboarding wizard） ───

    /** 是否已完成首次启动引导（完成后不再自动弹出） */
    var onboardingCompleted: Boolean
        get() = prefs.getBoolean(KEY_ONBOARDING_COMPLETED, false)
        set(value) = prefs.edit().putBoolean(KEY_ONBOARDING_COMPLETED, value).apply()

    // ─── 主动陪伴开关（V1.2 模块6，契约 schema → Settings 新增 Keys）───

    /**
     * 主动陪伴插件总开关（R15 定时主动提醒移除：默认**关闭**）。
     * 关闭时 MainActivity 不装载 ProactivePlugin，不排 WorkManager 周期任务
     * （R15 需求3：定时提醒默认 false + resetAll() 恢复 false；T7 锁定签名）。
     */
    var proactiveEnabled: Boolean
        get() = prefs.getBoolean(KEY_PROACTIVE_ENABLED, false)
        set(value) = prefs.edit().putBoolean(KEY_PROACTIVE_ENABLED, value).apply()

    // ─── 渲染质量（用户需求：流畅 / 均衡 / 高清三档）───

    /**
     * 渲染质量档位（core/RenderQualityConfig 三档：SMOOTH/BALANCED/HD，默认 SMOOTH）。
     * 持久化枚举名（键 renderQuality）；缺失/未知值经 RenderQualityConfig.fromStorage 兜底 SMOOTH。
     * 设置页点击写入；Live2DRenderer 构造时一次性 restore → App 重启恢复档位。
     */
    var renderQuality: RenderQuality
        get() = RenderQualityConfig.fromStorage(prefs.getString(KEY_RENDER_QUALITY, null))
        set(value) = prefs.edit().putString(KEY_RENDER_QUALITY, value.name).apply()

    // ─── 重置所有设置 ───

    fun resetAll() {
        prefs.edit().clear().apply()
    }

    companion object {
        private const val PREF_NAME = "live2dai_settings"

        const val KEY_DEEPSEEK = "deepseek_api_key"
        const val KEY_MODEL = "model"
        const val KEY_TEMPERATURE = "temperature"
        const val KEY_TTS_VOICE = "tts_voice"
        const val KEY_TTS_SPEED = "tts_speed"
        const val KEY_TTS_PROVIDER_ID = "tts_provider_id"
        const val KEY_COSYVOICE_API_KEY = "cosyvoice_api_key"
        const val KEY_COSYVOICE_WS_URL = "cosyvoice_ws_url"
        const val KEY_COSYVOICE_INSTRUCTION = "cosyvoice_instruction"
        const val KEY_COSYVOICE_RATE = "cosyvoice_rate"
        const val KEY_COSYVOICE_PITCH = "cosyvoice_pitch"
        const val KEY_PERSONA = "persona"
        const val KEY_SELECTED_MODEL_ID = "selected_model_id"
        const val KEY_FREE_QUOTA_NOTICE = "free_quota_notice_dismissed"
        const val KEY_ONBOARDING_COMPLETED = "onboarding_completed"
        const val KEY_PROACTIVE_ENABLED = "proactive_enabled"
        const val KEY_RENDER_QUALITY = "renderQuality"

        const val DEFAULT_MODEL = "deepseek-v4-flash"
        const val DEFAULT_TEMPERATURE = 0.8f
        const val DEFAULT_TTS_VOICE = "zh-CN-XiaoxiaoNeural"
        const val DEFAULT_TTS_SPEED = 1.0f

        /**
         * 首选 TTS 引擎默认值：schema 契约 "cosyvoice_ws"（plan §4.3；N.E.K.O. 方案主引擎，
         * priority=0 降级链第一位）。API Key 未配置时快速失败自动降级到 sherpa 离线兜底。
         */
        const val DEFAULT_TTS_PROVIDER_ID = "cosyvoice_ws"

        /** CosyVoice API Key 默认值：BuildConfig 编译值（local.properties 已填新 Key），空 = 未配置 */
        val DEFAULT_COSYVOICE_API_KEY: String get() = BuildConfig.COSYVOICE_API_KEY.ifEmpty { "" }

        /** CosyVoice WS 端点默认值：BuildConfig 编译值（专属 host），空 = 使用 provider 默认公共端点 */
        val DEFAULT_COSYVOICE_WS_URL: String get() = BuildConfig.COSYVOICE_WS_URL.ifEmpty { "" }

        /** CosyVoice instruction 默认值：清澈甜美少女音（SDK 限制 ≤100 字符） */
        const val DEFAULT_COSYVOICE_INSTRUCTION = "用清澈甜美的少女音说话，声音甜美带一点傲娇"

        /** CosyVoice 语速默认值：1.2（略快有活力感） */
        const val DEFAULT_COSYVOICE_RATE = 1.2f

        /** CosyVoice 音调默认值：1.1（升高显得更萌） */
        const val DEFAULT_COSYVOICE_PITCH = 1.1f

        const val DEFAULT_PERSONA = "白 (Bai)"

        /** Live2D 默认形象 ID（与 shared/model_registry.json 的 default 条目一致，registry 默认条目 = bai） */
        const val DEFAULT_MODEL_ID = "bai"

        /** 可用模型列表（供用户在设置页手动选择聊天模型） */
        val AVAILABLE_MODELS = listOf(
            "deepseek-v4-flash",
            "deepseek-chat",
            "deepseek-reasoner",
        )

        /** 可用 TTS 语音列表 */
        val AVAILABLE_TTS_VOICES = listOf(
            "zh-CN-XiaoxiaoNeural",
            "zh-CN-XiaoyiNeural",
            "zh-CN-YunjianNeural",
            "zh-CN-YunxiaNeural",
            "zh-CN-YunyangNeural",
        )
    }
}
