package com.live2dai.plugin.proactive

import com.live2dai.pluginsdk.ChatContext
import com.live2dai.pluginsdk.ChatHook

/**
 * 只负责一件事：用户发起消息（[beforeSend]）与每轮真实对话结束（[afterReceive]）时刷新「最后交互时间」
 * （见 docs/plan-task-plugin-proactive.md §4.2 + V1.2 任务要求「beforeSend 记录最后交互时间」）。
 *
 * 两个时点都记录的原因：beforeSend 在 LLM 调用之前触发——即使模型调用失败（无 afterReceive），
 * 「用户刚刚来找过猫娘」这一事实也已持久化，不会被误判为长期空闲；afterReceive 则覆盖
 * 完整对话轮次结束的语义。刻意不注入任何对话内容——主动陪伴的触发判断完全基于时间。
 * 不打扰原则：只要用户最近跟猫娘说过话，[ProactiveCheckWorker] 就不会触发主动消息。
 */
class ProactiveTrackingHook(private val tracker: LastInteractionTracker) : ChatHook {

    override suspend fun beforeSend(userMessage: String, context: ChatContext): String? {
        // 用户发起消息 = 活跃中，立即刷新最后交互时间（不注入任何附加上下文）
        tracker.recordInteraction()
        return null
    }

    override suspend fun afterReceive(assistantMessage: String, emotion: String?, context: ChatContext) {
        tracker.recordInteraction()
    }
}
