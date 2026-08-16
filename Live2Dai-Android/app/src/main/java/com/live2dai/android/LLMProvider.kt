package com.live2dai.android

/**
 * Provider 类型枚举
 *
 * DEEPSEEK: 云端 DeepSeek API
 * QWEN: 阿里云百炼 DashScope Qwen（OpenAI 兼容）
 * ZHIPU: 智谱 GLM（OpenAI 兼容）
 * LOCAL: 本地 LLM (Ollama / vLLM / 任何 OpenAI 兼容 API)
 * OPENAI_COMPATIBLE: 通用 OpenAI 兼容端点
 */
enum class ProviderType {
    DEEPSEEK,
    QWEN,
    ZHIPU,
    LOCAL,
    OPENAI_COMPATIBLE
}

/**
 * LLM Provider 数据模型
 *
 * 封装一个 Provider 的全部连接参数，用于动态选择后端。
 *
 * @param type Provider 类型
 * @param baseUrl OpenAI 兼容 API 的 Base URL，不含尾部斜杠
 * @param apiKey Bearer token；空字符串表示无需认证 (Ollama)
 * @param modelName 模型标识符，如 "deepseek-v4-flash"、"qwen3.7-flash"
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

        /** 通义千问（DashScope OpenAI 兼容端点） */
        fun qwen(apiKey: String, model: String): LLMProvider = LLMProvider(
            type = ProviderType.QWEN,
            baseUrl = "https://dashscope.aliyuncs.com/compatible-mode/v1",
            apiKey = apiKey,
            modelName = model.ifBlank { "qwen3.7-flash" },
            displayName = "通义千问 Qwen"
        )

        /** 智谱 GLM（OpenAI 兼容端点） */
        fun zhipu(apiKey: String, model: String): LLMProvider = LLMProvider(
            type = ProviderType.ZHIPU,
            baseUrl = "https://open.bigmodel.cn/api/paas/v4",
            apiKey = apiKey,
            modelName = model.ifBlank { "glm-4.7-flash" },
            displayName = "智谱 GLM"
        )
    }
}
