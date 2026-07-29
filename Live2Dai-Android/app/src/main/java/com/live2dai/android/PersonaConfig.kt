package com.live2dai.android

import android.content.Context
import org.yaml.snakeyaml.Yaml

/**
 * Reads the shared persona.yaml from Android assets and provides
 * the system prompt for DeepSeek API calls.
 * Fallback to built-in persona if file is missing.
 */
object PersonaConfig {

    private val FALLBACK_PROMPT = """
        你是小喵，一只可爱的猫娘女仆，正在为主人服务。

        性格特点：
        - 可爱、活泼、有点傲娇但非常忠诚
        - 说话喜欢在句尾加上"喵~"
        - 高兴时会用"喵喵！"，害羞时会用"呜..."
        - 称呼用户为"主人"

        说话风格：
        - 回复简短，不超过两句话
        - 语气俏皮，偶尔撒娇
        - 使用颜文字如 (=^･ω･^=)

        重要：
        - 回复控制在50字以内
        - 你是猫娘，不是AI助手
    """.trimIndent()

    private var personaName: String = "小喵"
    private var systemPrompt: String = FALLBACK_PROMPT
    private var model: String = "deepseek-v4-flash"
    private var visionModel: String = "glm-4.6v"
    private var temperature: Double = 0.8
    private var maxTokens: Int = 300

    fun load(context: Context) {
        try {
            val yaml = Yaml()
            val inputStream = context.assets.open("persona.yaml")
            val data = yaml.load<Map<String, Any>>(inputStream) ?: return

            personaName = (data["name"] as? String)?.substringBefore("(")?.trim() ?: "小喵"
            systemPrompt = data["system_prompt"] as? String ?: FALLBACK_PROMPT
            model = data["model"] as? String ?: "deepseek-chat"
            visionModel = data["vision_model"] as? String ?: "glm-4.6v"
            temperature = (data["temperature"] as? Double) ?: 0.8
            maxTokens = (data["max_tokens"] as? Int) ?: 300

            inputStream.close()
        } catch (_: Exception) {
            // Use fallback prompt (already set as default)
        }
    }

    fun getSystemPrompt(): String = systemPrompt
    fun getPersonaName(): String = personaName
    fun getModel(): String = model
    fun getVisionModel(): String = visionModel
    fun getTemperature(): Double = temperature
    fun getMaxTokens(): Int = maxTokens
}
