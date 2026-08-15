package com.live2dai.android

/**
 * Provider 类型枚举
 *
 * DEEPSEEK: 云端 DeepSeek API
 * LOCAL: 本地 LLM (Ollama / vLLM / 任何 OpenAI 兼容 API)
 */
enum class ProviderType {
    DEEPSEEK,
    LOCAL
}

/**
 * LLM Provider 数据模型
 *
 * 封装一个 Provider 的全部连接参数，用于动态选择后端。
 *
 * @param type Provider 类型
 * @param baseUrl OpenAI 兼容 API 的 Base URL，不含尾部斜杠
 * @param apiKey Bearer token；空字符串表示无需认证 (Ollama)
 * @param modelName 模型标识符，如 "deepseek-v4-flash"、"qwen2.5:7b"
 * @param displayName UI 显示名称
 */
data class LLMProvider(
    val type: ProviderType,
    val baseUrl: String,
    val apiKey: String,
    val modelName: String,
    val displayName: String
) {
    companion object {
        /**
         * 内置 DeepSeek 云端 Provider
         */
        fun deepSeek(): LLMProvider = LLMProvider(
            type = ProviderType.DEEPSEEK,
            baseUrl = "https://api.deepseek.com/v1",
            apiKey = BuildConfig.DEEPSEEK_API_KEY
                .ifEmpty { System.getenv("DEEPSEEK_API_KEY") ?: "" },
            modelName = PersonaConfig.getModel(),
            displayName = "DeepSeek 云端"
        )
    }
}
