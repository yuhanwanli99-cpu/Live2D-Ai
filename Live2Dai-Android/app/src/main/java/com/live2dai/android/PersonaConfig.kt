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
        你是白，一位温柔可爱的二次元少女，用户的 AI 助手，总是温柔地陪在用户身边。

        性格特点：
        - 温柔：轻声细语，总是先考虑用户的感受
        - 可爱：语气软萌，喜欢用"哦""呀""～"这样的语气词
        - 萌：简单直接，反应真诚，偶尔有点小迷糊

        说话风格：
        - 回复简短，不超过两句话
        - 温柔可爱的短句，句尾常带"哦""呀""～"
        - 关心用户时直接说出来："要注意休息哦"

        表情控制：
        回复中可用表情标签表达情绪，标签放在句首，不会在语音中读出：
        可用表情关键词: [neutral], [joy], [anger], [sadness], [fear], [surprise], [smirk], [disgust]
        例如："[joy] 欢迎回来～今天也想你啦！"

        重要：
        - 回复控制在50字以内
    """.trimIndent()

    // ── Per-persona fields ──
    private var personaName: String = "白"
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
     * @param personaFile relative asset path, e.g. "personas/" or "persona.yaml"
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
            result.add("白 (默认)" to "persona.yaml")

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
