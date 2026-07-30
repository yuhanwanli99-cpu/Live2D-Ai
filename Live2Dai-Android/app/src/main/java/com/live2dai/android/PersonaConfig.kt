package com.live2dai.android

import android.content.Context
import org.yaml.snakeyaml.Yaml

/**
 * Reads persona YAML files from Android assets and provides
 * the system prompt for AI API calls.
 * Supports multiple personas via the personas/ subdirectory.
 *
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

    // ── Per-persona fields ──
    private var personaName: String = "小喵"
    private var systemPrompt: String = FALLBACK_PROMPT
    private var model: String = "deepseek-v4-flash"
    private var visionModel: String = "glm-4.6v"
    private var temperature: Double = 0.8
    private var maxTokens: Int = 300

    // ── Current persona file tracking ──
    private var currentPersonaFile: String = "persona.yaml"

    // ── Public API ──

    /**
     * Load the default persona (persona.yaml).
     * Backward-compatible — same behaviour as before.
     */
    fun load(context: Context) = loadFile(context, "persona.yaml")

    /**
     * Load a specific persona from assets.
     * @param context Android context
     * @param personaFile relative asset path, e.g. "personas/catgirl.yaml" or "persona.yaml"
     */
    fun loadFile(context: Context, personaFile: String) {
        currentPersonaFile = personaFile
        try {
            val yaml = Yaml()
            val inputStream = context.assets.open(personaFile)
            val data = yaml.load<Map<String, Any>>(inputStream) ?: return

            // Extract the "short name" — the portion before the first '('
            personaName = (data["name"] as? String)
                ?.substringBefore("(")
                ?.trim()
                ?: "Unknown"

            systemPrompt = data["system_prompt"] as? String ?: FALLBACK_PROMPT
            model = data["model"] as? String ?: "deepseek-v4-flash"
            visionModel = data["vision_model"] as? String ?: "glm-4.6v"
            temperature = (data["temperature"] as? Double) ?: 0.75
            maxTokens = (data["max_tokens"] as? Int) ?: 300

            inputStream.close()
        } catch (_: Exception) {
            // Use fallback prompt (already set as default)
        }
    }

    /**
     * Enumerate available persona YAML files from assets/personas/.
     * Also includes the root-level persona.yaml for backward compat.
     *
     * Returns a list of pairs: (displayName, assetPath).
     */
    fun getAvailablePersonas(context: Context): List<Pair<String, String>> {
        val result = mutableListOf<Pair<String, String>>()
        try {
            // The default root persona
            result.add("小喵 (默认)" to "persona.yaml")

            // All .yaml files under assets/personas/
            val files = context.assets.list("personas")
                ?.filter { it.endsWith(".yaml") }
                ?.sorted()
                ?: emptyList()

            for (file in files) {
                val displayName = file
                    .removeSuffix(".yaml")
                    .replaceFirstChar { it.uppercase() }
                result.add(displayName to "personas/$file")
            }
        } catch (_: Exception) {
            // Fall back to just the default
        }
        return result
    }

    /**
     * Returns the asset path of the currently loaded persona file.
     */
    fun getCurrentPersonaFile(): String = currentPersonaFile

    // ── Getters ──

    fun getSystemPrompt(): String = systemPrompt
    fun getPersonaName(): String = personaName
    fun getModel(): String = model
    fun getVisionModel(): String = visionModel
    fun getTemperature(): Double = temperature
    fun getMaxTokens(): Int = maxTokens
}
