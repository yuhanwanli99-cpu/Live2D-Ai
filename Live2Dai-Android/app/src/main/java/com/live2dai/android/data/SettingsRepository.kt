package com.live2dai.android.data

import android.content.Context
import android.content.SharedPreferences
import com.live2dai.android.BuildConfig

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
        const val KEY_PERSONA = "persona"
        const val KEY_SELECTED_MODEL_ID = "selected_model_id"
        const val KEY_FREE_QUOTA_NOTICE = "free_quota_notice_dismissed"
        const val KEY_ONBOARDING_COMPLETED = "onboarding_completed"
        const val KEY_PROACTIVE_ENABLED = "proactive_enabled"

        const val DEFAULT_MODEL = "deepseek-v4-flash"
        const val DEFAULT_TEMPERATURE = 0.8f
        const val DEFAULT_TTS_VOICE = "zh-CN-XiaoxiaoNeural"
        const val DEFAULT_TTS_SPEED = 1.0f

        /**
         * 首选 TTS 引擎默认值：K1 单引擎 sherpa_onnx_kokoro（Wave 5 清理：cosyvoice 链已删）。
         */
        const val DEFAULT_TTS_PROVIDER_ID = "sherpa_onnx_kokoro"

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
