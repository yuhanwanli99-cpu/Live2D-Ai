package com.live2dai.android

/**
 * Provider 类型枚举
 *
 * DEEPSEEK: 云端 DeepSeek API
 * ZHIPU: 智谱 GLM API（OpenAI 兼容格式，免费兜底聊天模型）
 * LOCAL: 本地 LLM (Ollama / vLLM / 任何 OpenAI 兼容 API)
 */
enum class ProviderType {
    DEEPSEEK,
    ZHIPU,
    LOCAL
}

/**
 * LLM Provider 数据模型
 *
 * 封装一个 Provider 的全部连接参数，用于 ChatService 动态选择后端。
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

        /**
         * 内置智谱 GLM 云端 Provider（永久免费兜底聊天模型）
         *
         * OpenAI 兼容格式，baseUrl 与 shared/model_dict.json 中 glm-4.6v 条目一致。
         * 用于零配置首次体验：个人账号限 1 并发，不适合高并发场景。
         */
        fun zhipu(): LLMProvider = LLMProvider(
            type = ProviderType.ZHIPU,
            baseUrl = "https://open.bigmodel.cn/api/paas/v4",
            apiKey = BuildConfig.ZHIPU_API_KEY
                .ifEmpty { System.getenv("ZHIPU_API_KEY") ?: "" },
            modelName = "glm-4.7-flash",
            displayName = "智谱 GLM（免费）"
        )
    }
}
