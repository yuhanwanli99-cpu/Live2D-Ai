package com.live2dai.plugin.memory

import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * 长期记忆事实（Room Entity），见 docs/plan-task-plugin-memory.md §3.1。
 *
 * 对应 N.E.K.O. 多层记忆中的「长期记忆（事实摘要）」层——用户的稳定信息
 * （如"用户叫小明，养了一只猫"），由对话内容周期性总结生成。
 */
@Entity(tableName = "memory_facts")
data class MemoryFact(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    /** 事实摘要文本，如"用户养了一只叫豆豆的猫" */
    val content: String,
    /** 逗号分隔的关键词，用于检索打分（见 [MemoryKeywordMatcher]） */
    val keywords: String,
    /** 创建时间（epoch millis） */
    val createdAt: Long,
    /** 最近一次被检索命中的时间，为遗忘策略预留（MVP 暂不做主动遗忘） */
    val lastUsedAt: Long,
)
