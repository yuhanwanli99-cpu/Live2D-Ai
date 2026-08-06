package com.live2dai.android

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.live2dai.plugin.memory.MemoryDatabase
import com.live2dai.plugin.memory.MemoryFact
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * [ProactiveMemorySource] 单元测试：主动陪伴的话题记忆源读取记忆插件 Room 库
 * （docs/plan-v12-plan.md §5.6.3 记忆驱动话题）。
 *
 * 验证：按 createdAt 降序取最近 N 条、空库返回空、上限生效、读取失败静默降级。
 */
@RunWith(RobolectricTestRunner::class)
class ProactiveMemorySourceTest {

    private lateinit var context: Context
    private lateinit var dao: com.live2dai.plugin.memory.MemoryDao

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        ProactiveMemorySource.resetForTest()
        dao = MemoryDatabase.build(context, "live2dai_memory.db").memoryDao()
    }

    @After
    fun tearDown() {
        ProactiveMemorySource.resetForTest()
    }

    private suspend fun insert(content: String, createdAt: Long) {
        dao.insert(
            MemoryFact(
                content = content,
                keywords = "k",
                createdAt = createdAt,
                lastUsedAt = createdAt,
            )
        )
    }

    @Test
    fun recentFacts_returnsNewestFirst() = runBlocking {
        insert("旧记忆：用户喜欢喝美式咖啡", createdAt = 1_000L)
        insert("新记忆：用户养了一只叫豆豆的猫", createdAt = 3_000L)
        insert("中记忆：用户最近在准备考试", createdAt = 2_000L)

        val facts = ProactiveMemorySource.recentFacts(context)

        assertEquals(
            "应按 createdAt 降序（最新的在前）",
            listOf("新记忆：用户养了一只叫豆豆的猫", "中记忆：用户最近在准备考试", "旧记忆：用户喜欢喝美式咖啡"),
            facts,
        )
    }

    @Test
    fun recentFacts_emptyDatabase_returnsEmptyList() = runBlocking {
        val facts = ProactiveMemorySource.recentFacts(context)

        assertTrue("空记忆库应返回空列表（LLM 仅按人设+时间生成）", facts.isEmpty())
    }

    @Test
    fun recentFacts_limitsResults() = runBlocking {
        insert("记忆1", createdAt = 1_000L)
        insert("记忆2", createdAt = 2_000L)
        insert("记忆3", createdAt = 3_000L)

        val facts = ProactiveMemorySource.recentFacts(context, limit = 2)

        assertEquals(listOf("记忆3", "记忆2"), facts)
    }

    @Test
    fun recentFacts_defaultLimitIsFive() = runBlocking {
        for (i in 1L..7L) insert("记忆$i", createdAt = i)

        val facts = ProactiveMemorySource.recentFacts(context)

        assertEquals("默认最多取 5 条", 5, facts.size)
        assertEquals(listOf("记忆7", "记忆6", "记忆5", "记忆4", "记忆3"), facts)
    }
}
