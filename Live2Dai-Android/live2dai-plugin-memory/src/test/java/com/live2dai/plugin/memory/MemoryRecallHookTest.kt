package com.live2dai.plugin.memory

import com.live2dai.pluginsdk.ChatContext
import com.live2dai.pluginsdk.ChatTurn
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 钩子行为测试：
 * - beforeSend：关键词相关性检索 → 只注入相关事实，格式符合 `[系统: 相关记忆]` 前缀（验收第 2 条）
 * - afterReceive：每 N 轮触发摘要 + 异常容错（验收第 3 条、R4）
 * - 关键词匹配工具：中英文 token 提取与打分
 */
class MemoryRecallHookTest {

    /** 内存版 [MemoryDao] 替身（记录 insert/update/deleteOldest 调用） */
    private class FakeDao(initial: List<MemoryFact> = emptyList()) : MemoryDao {
        val facts = initial.toMutableList()
        val inserted = mutableListOf<MemoryFact>()
        val updates = mutableListOf<MemoryFact>()
        val deleteOldestCalls = mutableListOf<Int>()

        override suspend fun insert(fact: MemoryFact): Long {
            val nextId = (facts.maxOfOrNull { it.id } ?: 0L) + 1
            val stored = fact.copy(id = nextId)
            facts += stored
            inserted += stored
            return nextId
        }

        override suspend fun getAll(): List<MemoryFact> = facts.toList()

        override suspend fun update(fact: MemoryFact) {
            updates += fact
            val idx = facts.indexOfFirst { it.id == fact.id }
            if (idx >= 0) facts[idx] = fact
        }

        /** 与 Room 版语义一致：按 createdAt 降序（同分取 id 大者）保留最近 [keepCount] 条 */
        override suspend fun deleteOldest(keepCount: Int) {
            deleteOldestCalls += keepCount
            if (keepCount <= 0) {
                facts.clear()
                return
            }
            val sorted = facts.sortedWith(
                compareByDescending<MemoryFact> { it.createdAt }.thenByDescending { it.id }
            )
            facts.clear()
            facts.addAll(sorted.take(keepCount))
        }
    }

    private fun chatContext(turns: List<ChatTurn> = emptyList()) =
        ChatContext(recentMessages = turns, personaName = "小喵")

    private fun fact(id: Long, content: String, keywords: String) =
        MemoryFact(id = id, content = content, keywords = keywords, createdAt = id, lastUsedAt = id)

    // ── beforeSend：关键词检索注入 ──

    @Test
    fun beforeSend_returnsOnlyRelevantFactsWithSystemPrefix() = runBlocking {
        // 验收场景（docs/plan-task-plugin-memory.md §7 第 2 条）：mock 2 条相关 + 1 条不相关 → 只注入相关的 2 条
        val dao = FakeDao(
            listOf(
                fact(1, "用户养了一只叫豆豆的猫", "豆豆,猫"),
                fact(2, "用户喜欢喝美式咖啡", "咖啡,美式"),
                fact(3, "用户周末喜欢去公园", "公园,天气"),
            )
        )
        val hook = MemoryRecallHook(dao, SummaryLlmClient { _, _ -> null }, triggerEveryNTurns = 10)

        val result = hook.beforeSend("我的猫豆豆想喝咖啡了", chatContext())

        assertTrue("应以 [系统: 相关记忆] 开头", result.orEmpty().startsWith("[系统: 相关记忆]"))
        assertTrue("应包含相关事实 1（猫/豆豆）", result.orEmpty().contains("用户养了一只叫豆豆的猫"))
        assertTrue("应包含相关事实 2（咖啡）", result.orEmpty().contains("用户喜欢喝美式咖啡"))
        assertFalse("不应包含不相关事实（公园）", result.orEmpty().contains("公园"))
        assertFalse("不应包含无关信息", result.orEmpty().contains("天气"))
    }

    @Test
    fun beforeSend_returnsNullWhenNothingRelevant() = runBlocking {
        val dao = FakeDao(
            listOf(
                fact(1, "用户喜欢喝美式咖啡", "咖啡,美式"),
                fact(2, "用户周末喜欢去公园", "公园,天气"),
            )
        )
        val hook = MemoryRecallHook(dao, SummaryLlmClient { _, _ -> null }, triggerEveryNTurns = 10)

        val result = hook.beforeSend("今天股票涨了多少", chatContext())

        assertNull("无相关记忆时不应注入", result)
    }

    @Test
    fun beforeSend_updatesLastUsedAtOnHits() = runBlocking {
        val dao = FakeDao(listOf(fact(1, "用户养了一只叫豆豆的猫", "豆豆,猫")))
        val hook = MemoryRecallHook(dao, SummaryLlmClient { _, _ -> null }, triggerEveryNTurns = 10, now = { 9_999L })

        hook.beforeSend("我的猫豆豆今天怎么样", chatContext())

        assertEquals("命中的事实应刷新 lastUsedAt", 9_999L, dao.facts.single().lastUsedAt)
    }

    @Test
    fun beforeSend_swallowsDaoExceptions() = runBlocking {
        val brokenDao = object : MemoryDao {
            override suspend fun insert(fact: MemoryFact): Long = throw IllegalStateException("db broken")
            override suspend fun getAll(): List<MemoryFact> = throw IllegalStateException("db broken")
            override suspend fun update(fact: MemoryFact) = throw IllegalStateException("db broken")
            override suspend fun deleteOldest(keepCount: Int) = throw IllegalStateException("db broken")
        }
        val hook = MemoryRecallHook(brokenDao, SummaryLlmClient { _, _ -> null }, triggerEveryNTurns = 10)

        val result = hook.beforeSend("我的猫豆豆今天怎么样", chatContext())

        assertNull("数据库异常应被吞掉，返回 null", result)
    }

    // ── afterReceive：周期性摘要写入 ──

    @Test
    fun afterReceive_triggersSummaryEveryNTurns() = runBlocking {
        val dao = FakeDao()
        var summarizeCalls = 0
        val llm = SummaryLlmClient { _, _ ->
            summarizeCalls++
            "用户喜欢喝咖啡"
        }
        val hook = MemoryRecallHook(dao, llm, triggerEveryNTurns = 10, now = { 1_000L })

        // 第 1~9 轮：不触发摘要
        repeat(9) { hook.afterReceive("回复$it", null, chatContext(listOf(ChatTurn("user", "你好$it")))) }
        assertEquals("前 9 轮不应调用摘要", 0, summarizeCalls)
        assertTrue("前 9 轮不应写入事实", dao.inserted.isEmpty())

        // 第 10 轮：触发摘要并写入
        hook.afterReceive("回复9", null, chatContext(listOf(ChatTurn("user", "你好9"))))

        assertEquals("第 10 轮应调用一次摘要", 1, summarizeCalls)
        assertEquals(1, dao.inserted.size)
        val written = dao.inserted.single()
        assertEquals("用户喜欢喝咖啡", written.content)
        assertEquals(1_000L, written.createdAt)
        assertEquals(1_000L, written.lastUsedAt)
        assertTrue("摘要应提取出关键词", written.keywords.isNotBlank())
    }

    @Test
    fun afterReceive_skipsWhenSummaryReturnsNullOrBlank() = runBlocking {
        val dao = FakeDao()
        val hook = MemoryRecallHook(dao, SummaryLlmClient { _, _ -> null }, triggerEveryNTurns = 1)

        hook.afterReceive("回复", null, chatContext(listOf(ChatTurn("user", "你好"))))
        hook.afterReceive("回复", null, chatContext(listOf(ChatTurn("user", "你好"))))

        assertTrue("摘要为 null 时不应写入", dao.inserted.isEmpty())
    }

    @Test
    fun afterReceive_skipsWhenNoRecentTurns() = runBlocking {
        val dao = FakeDao()
        val hook = MemoryRecallHook(dao, SummaryLlmClient { _, _ -> "有内容" }, triggerEveryNTurns = 1)

        hook.afterReceive("回复", null, chatContext())

        assertTrue("无近期对话时不应触发摘要", dao.inserted.isEmpty())
    }

    @Test
    fun afterReceive_swallowsSummaryExceptions() = runBlocking {
        val dao = FakeDao()
        val hook = MemoryRecallHook(
            dao,
            SummaryLlmClient { _, _ -> throw RuntimeException("llm timeout") },
            triggerEveryNTurns = 1,
        )

        hook.afterReceive("回复", null, chatContext(listOf(ChatTurn("user", "你好"))))

        assertTrue("LLM 异常应被吞掉且不写入", dao.inserted.isEmpty())
    }

    @Test
    fun afterReceive_deduplicatesSameContent() = runBlocking {
        val dao = FakeDao(listOf(fact(1, "用户喜欢喝咖啡", "咖啡")))
        val hook = MemoryRecallHook(dao, SummaryLlmClient { _, _ -> "用户喜欢喝咖啡" }, triggerEveryNTurns = 1)

        hook.afterReceive("回复", null, chatContext(listOf(ChatTurn("user", "你好"))))

        assertTrue("已存在相同内容时不应重复写入", dao.inserted.isEmpty())
    }

    @Test
    fun afterReceive_trimsToMaxFactsAfterWrite() = runBlocking {
        val dao = FakeDao()
        var t = 1_000L
        var turn = 0
        val llm = SummaryLlmClient { _, _ ->
            turn++
            "用户喜欢${listOf("咖啡", "茶", "牛奶")[turn - 1]}"
        }
        val hook = MemoryRecallHook(dao, llm, triggerEveryNTurns = 1, maxFacts = 2, now = { t++ })

        hook.afterReceive("r1", null, chatContext(listOf(ChatTurn("user", "u1"))))
        hook.afterReceive("r2", null, chatContext(listOf(ChatTurn("user", "u2"))))
        hook.afterReceive("r3", null, chatContext(listOf(ChatTurn("user", "u3"))))

        assertEquals("三次写入都发生", 3, dao.inserted.size)
        assertEquals("每次写入后都应触发 trim", listOf(2, 2, 2), dao.deleteOldestCalls)
        assertEquals("写入第 4 条前应裁剪到最近 2 条", 2, dao.facts.size)
        assertEquals(
            "保留最新的 2 条（茶、牛奶），最旧的（咖啡）被裁剪",
            listOf("用户喜欢茶", "用户喜欢牛奶"),
            dao.facts.sortedBy { it.id }.map { it.content }
        )
    }

    @Test
    fun afterReceive_doesNotTrimWhenMaxFactsNonPositive() = runBlocking {
        val dao = FakeDao()
        var turn = 0
        val hook = MemoryRecallHook(
            dao,
            SummaryLlmClient { _, _ ->
                turn++
                "用户喜欢${listOf("咖啡", "茶", "牛奶")[turn - 1]}"
            },
            triggerEveryNTurns = 1,
            maxFacts = 0, // <= 0 表示不裁剪（无限量）
        )

        repeat(3) { hook.afterReceive("r$it", null, chatContext(listOf(ChatTurn("user", "u$it")))) }

        assertTrue("maxFacts<=0 时不应调用 deleteOldest", dao.deleteOldestCalls.isEmpty())
        assertEquals("不裁剪时 3 条全部保留", 3, dao.facts.size)
    }

    // ── 关键词匹配工具 ──

    @Test
    fun extractKeywords_handlesChineseAndEnglish() {
        val tokens = MemoryKeywordMatcher.extractKeywords("用户喜欢喝美式咖啡 and coffee")
        assertTrue("英文词应小写提取", tokens.contains("coffee"))
        assertTrue("中文自然二字词应提取", tokens.contains("咖啡"))
        assertTrue("中文单字应提取", tokens.contains("咖"))
        assertFalse("停用字不应提取", tokens.contains("的"))
    }

    @Test
    fun score_isKeywordOverlapRatio() {
        val messageKeywords = MemoryKeywordMatcher.extractKeywords("我的猫豆豆今天怎么样").toSet()

        val relevant = MemoryKeywordMatcher.score(fact(1, "用户养猫", "豆豆,猫"), messageKeywords)
        assertEquals("豆豆、猫均命中 → 2/2", 1.0, relevant, 0.0001)

        val unrelated = MemoryKeywordMatcher.score(fact(2, "用户逛公园", "公园,天气"), messageKeywords)
        assertEquals("无交集 → 0", 0.0, unrelated, 0.0001)

        val emptyKeywords = MemoryKeywordMatcher.score(fact(3, "无关键词", ""), messageKeywords)
        assertEquals("事实关键词为空 → 0", 0.0, emptyKeywords, 0.0001)
    }

    @Test
    fun rankTop_returnsTopThreeRelevantInScoreOrder() {
        val facts = listOf(
            fact(1, "用户养了一只叫豆豆的猫", "豆豆,猫"),
            fact(2, "用户喜欢喝美式咖啡", "咖啡,美式"),
            fact(3, "用户周末喜欢去公园", "公园,天气"),
            fact(4, "用户咖啡因敏感只喝低因", "咖啡,低因"),
        )

        val top = MemoryKeywordMatcher.rankTop(facts, "我要一杯咖啡", limit = 3)

        assertEquals(2, top.size) // 只有两条与"咖啡"相关
        assertEquals(listOf(2L, 4L), top.map { it.id }) // score 同分 → id 升序
    }
}
