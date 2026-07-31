package com.live2dai.plugin.proactive

import com.live2dai.pluginsdk.ChatContext
import com.live2dai.pluginsdk.ChatHook

/**
 * 只负责一件事：每轮真实对话结束后刷新「最后交互时间」（见 docs/plan-task-plugin-proactive.md §4.2）。
 *
 * 刻意不注入任何对话上下文——主动陪伴的触发判断完全基于时间，
 * 不需要读取/改写对话内容。不打扰原则：只要用户最近跟猫娘说过话，
 * [ProactiveCheckWorker] 就不会触发主动消息。
 */
class ProactiveTrackingHook(private val tracker: LastInteractionTracker) : ChatHook {

    override suspend fun beforeSend(userMessage: String, context: ChatContext): String? = null

    override suspend fun afterReceive(assistantMessage: String, emotion: String?, context: ChatContext) {
        tracker.recordInteraction()
    }
}
