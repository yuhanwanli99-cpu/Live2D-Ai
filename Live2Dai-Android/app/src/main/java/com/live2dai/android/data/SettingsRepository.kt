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

    /** Zhipu / GLM API Key (自定义覆盖) */
    var zhipuApiKey: String
        get() = prefs.getString(KEY_ZHIPU, null)
            ?: BuildConfig.ZHIPU_API_KEY
        set(value) = prefs.edit().putString(KEY_ZHIPU, value).apply()

    /** 用户是否手动输入过自定义 Key */
    val hasCustomDeepseekKey: Boolean
        get() = prefs.contains(KEY_DEEPSEEK) && prefs.getString(KEY_DEEPSEEK, null)!!.isNotEmpty()

    val hasCustomZhipuKey: Boolean
        get() = prefs.contains(KEY_ZHIPU) && prefs.getString(KEY_ZHIPU, null)!!.isNotEmpty()

    fun clearCustomDeepseekKey() {
        prefs.edit().remove(KEY_DEEPSEEK).apply()
    }

    fun clearCustomZhipuKey() {
        prefs.edit().remove(KEY_ZHIPU).apply()
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

    // ─── Persona ───

    var personaName: String
        get() = prefs.getString(KEY_PERSONA, DEFAULT_PERSONA) ?: DEFAULT_PERSONA
        set(value) = prefs.edit().putString(KEY_PERSONA, value).apply()

    // ─── 免费共享额度提示（首次启动一次性提示，可关闭） ───

    var freeQuotaNoticeDismissed: Boolean
        get() = prefs.getBoolean(KEY_FREE_QUOTA_NOTICE, false)
        set(value) = prefs.edit().putBoolean(KEY_FREE_QUOTA_NOTICE, value).apply()

    // ─── 重置所有设置 ───

    fun resetAll() {
        prefs.edit().clear().apply()
    }

    companion object {
        private const val PREF_NAME = "live2dai_settings"

        const val KEY_DEEPSEEK = "deepseek_api_key"
        const val KEY_ZHIPU = "zhipu_api_key"
        const val KEY_MODEL = "model"
        const val KEY_TEMPERATURE = "temperature"
        const val KEY_TTS_VOICE = "tts_voice"
        const val KEY_TTS_SPEED = "tts_speed"
        const val KEY_PERSONA = "persona"
        const val KEY_FREE_QUOTA_NOTICE = "free_quota_notice_dismissed"

        const val DEFAULT_MODEL = "deepseek-v4-flash"
        const val DEFAULT_TEMPERATURE = 0.8f
        const val DEFAULT_TTS_VOICE = "zh-CN-XiaoxiaoNeural"
        const val DEFAULT_TTS_SPEED = 1.0f
        const val DEFAULT_PERSONA = "小喵 (Xiao Miao)"

        /** 可用模型列表（供用户在设置页手动选择聊天模型） */
        val AVAILABLE_MODELS = listOf(
            "deepseek-v4-flash",
            "deepseek-chat",
            "deepseek-reasoner",
            "glm-4.7-flash",
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
