package com.live2dai.plugin.proactive

/**
 * 主动问候文案生成接缝（沿用 memory 插件的注入模式，见 docs/plan-task-plugin-proactive.md §3.2）。
 *
 * 由 core 在构造 [ProactivePluginConfig] 时注入真实 LLM 实现，插件不关心具体厂商。
 * 返回 `null` 或抛异常时，[ProactiveCheckWorker] 回退到 [ProactivePluginConfig.fallbackGreetings]，
 * 保证"主动陪伴"不会因为网络问题而完全失效。
 */
fun interface GreetingLlmClient {

    /**
     * @param personaName 人设名称（如"小喵"），供 LLM 称呼用户
     * @param systemPrompt 人设 system prompt，让文案贴合语气
     * @param idleMinutes 距上次对话的分钟数（让文案带上时间感，如"好久没理我了"）
     * @param memories 可选的最近长期记忆（来自记忆插件），用于生成个性化话题（V1.2 记忆驱动话题，
     *                 docs/plan-v12-plan.md §5.6.3）；为 null / 空列表时仅按人设+时间生成
     * @return 问候文案；生成失败时返回 null（由调用方回退静态文案池）
     */
    suspend fun generateGreeting(
        personaName: String,
        systemPrompt: String,
        idleMinutes: Long,
        memories: List<String>?,
    ): String?
}
