package com.live2dai.plugin.memory

import android.util.Log
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
    /** 记忆上限：写入后裁剪到最近 [maxFacts] 条（plan §3.3.2 默认 500）；<= 0 表示不裁剪 */
    private val maxFacts: Int = 500,
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

        logInfo("beforeSend 注入 ${relevant.size} 条相关记忆")
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
                logInfo("afterReceive 写入新记忆：$content")
                // 记忆上限（plan §3.3.2）：写入后 trim 最旧记录；<= 0 表示不裁剪
                if (maxFacts > 0) {
                    dao.deleteOldest(maxFacts)
                    logInfo("afterReceive 裁剪记忆到最近 $maxFacts 条")
                }
            }
        } catch (_: Exception) {
            // 数据库异常不影响对话流程
        }
    }

    /**
     * 日志输出（带单元测试防护）：本地 JVM 单测（无 Android 运行时）中
     * android.util.Log 未被 mock，直接调用会抛 "Method i in android.util.Log not mocked"。
     * 真机上行为与直接调用一致（Log.i）。TAG 统一 "Memory"（取证日志，见 V1.1 教训）。
     */
    private fun logInfo(message: String) {
        try {
            Log.i("Memory", message)
        } catch (_: Throwable) {
            // 单元测试环境：静默忽略
        }
    }
}
