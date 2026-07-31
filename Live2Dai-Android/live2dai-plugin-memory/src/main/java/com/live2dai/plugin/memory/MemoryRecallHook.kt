package com.live2dai.plugin.memory

import com.live2dai.pluginsdk.ChatContext
import com.live2dai.pluginsdk.ChatHook

/**
 * 记忆钩子：提供两项能力（见 docs/plan-task-plugin-memory.md §1.2）——
 * - [beforeSend]：检索与当前用户消息相关的记忆摘要，注入为附加上下文
 * - [afterReceive]：每 N 轮触发一次摘要生成，把新事实写入 Room
 *
 * 容错约定（文档 §1.3 第 3 条）：内部任何异常（Room 读写失败、LLM 摘要超时）
 * 都被吞掉，绝不上抛——不能让记忆插件打断对话链路。
 */
class MemoryRecallHook(
    private val dao: MemoryDao,
    private val summaryLlmClient: SummaryLlmClient,
    private val triggerEveryNTurns: Int,
    /** 时钟注入点（测试用），默认取系统时间 */
    private val now: () -> Long = System::currentTimeMillis,
) : ChatHook {

    private var turnCounter = 0

    override suspend fun beforeSend(userMessage: String, context: ChatContext): String? {
        val relevant = try {
            MemoryKeywordMatcher.rankTop(dao.getAll(), userMessage)
        } catch (_: Exception) {
            // 数据库异常：本轮不注入任何记忆，对话流程不受影响
            return null
        }
        if (relevant.isEmpty()) return null

        // 命中即刷新 lastUsedAt（遗忘策略预留字段，MVP 不做主动遗忘）；刷新失败不影响注入
        val timestamp = now()
        for (fact in relevant) {
            try {
                dao.update(fact.copy(lastUsedAt = timestamp))
            } catch (_: Exception) {
                // ignore
            }
        }

        return "[系统: 相关记忆]\n" + relevant.joinToString("\n") { "- ${it.content}" }
    }

    override suspend fun afterReceive(assistantMessage: String, emotion: String?, context: ChatContext) {
        turnCounter++
        if (triggerEveryNTurns <= 0 || turnCounter % triggerEveryNTurns != 0) return

        val recentTurns = context.recentMessages
        if (recentTurns.isEmpty()) return

        // R4：摘要生成失败（LLM 超时/异常）只跳过本轮摘要，不影响下一轮触发
        val summary = try {
            summaryLlmClient.summarize(recentTurns, context.personaName)
        } catch (_: Exception) {
            return
        }
        val content = summary?.trim().orEmpty()
        if (content.isEmpty()) return

        val keywords = MemoryKeywordMatcher.extractKeywords(content).joinToString(",")
        if (keywords.isEmpty()) return

        try {
            // 去重：同一事实只存一条，避免每 N 轮重复堆积
            val exists = dao.getAll().any { it.content == content }
            if (!exists) {
                val timestamp = now()
                dao.insert(
                    MemoryFact(
                        content = content,
                        keywords = keywords,
                        createdAt = timestamp,
                        lastUsedAt = timestamp,
                    )
                )
            }
        } catch (_: Exception) {
            // 数据库异常不影响对话流程
        }
    }
}
