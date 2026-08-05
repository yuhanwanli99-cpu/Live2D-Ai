package com.live2dai.plugin.memory

import android.content.Context
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * Room 存储层测试：内存数据库验证「写入 → 全表检索」往返、字段无损与更新
 * （docs/plan-task-plugin-memory.md §7 验收第 1 条）。
 */
@RunWith(RobolectricTestRunner::class)
class MemoryDaoTest {

    private lateinit var db: MemoryDatabase
    private lateinit var dao: MemoryDao

    @Before
    fun setUp() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        db = Room.inMemoryDatabaseBuilder(context, MemoryDatabase::class.java)
            .allowMainThreadQueries() // 单测在测试线程直接访问
            .build()
        dao = db.memoryDao()
    }

    @After
    fun tearDown() {
        db.close()
    }

    @Test
    fun insert_threeFacts_getAllReturnsAllWithFieldsIntact() = runBlocking {
        val t0 = 1_720_000_000_000L
        dao.insert(
            MemoryFact(content = "用户养了一只叫豆豆的猫", keywords = "豆豆,猫", createdAt = t0, lastUsedAt = t0)
        )
        dao.insert(
            MemoryFact(content = "用户喜欢喝美式咖啡", keywords = "咖啡,美式", createdAt = t0 + 1, lastUsedAt = t0 + 1)
        )
        dao.insert(
            MemoryFact(content = "用户周末喜欢去公园", keywords = "公园,周末", createdAt = t0 + 2, lastUsedAt = t0 + 2)
        )

        val all = dao.getAll()

        assertEquals(3, all.size)

        val cat = all.first { it.content == "用户养了一只叫豆豆的猫" }
        assertEquals("豆豆,猫", cat.keywords)
        assertEquals(t0, cat.createdAt)
        assertEquals(t0, cat.lastUsedAt)
        assertTrue("autoGenerate 应分配自增 id", cat.id > 0)

        val coffee = all.first { it.content == "用户喜欢喝美式咖啡" }
        assertEquals("咖啡,美式", coffee.keywords)
        assertEquals(t0 + 1, coffee.createdAt)
        assertEquals(t0 + 1, coffee.lastUsedAt)

        val park = all.first { it.content == "用户周末喜欢去公园" }
        assertEquals("公园,周末", park.keywords)
        assertEquals(t0 + 2, park.createdAt)
    }

    @Test
    fun update_persistsChangedFields() = runBlocking {
        val id = dao.insert(
            MemoryFact(content = "旧内容", keywords = "旧", createdAt = 1L, lastUsedAt = 1L)
        )

        dao.update(MemoryFact(id = id, content = "旧内容", keywords = "旧", createdAt = 1L, lastUsedAt = 9_999L))

        val fact = dao.getAll().single()
        assertEquals(id, fact.id)
        assertEquals(9_999L, fact.lastUsedAt)
    }

    @Test
    fun getAll_emptyDatabaseReturnsEmptyList() = runBlocking {
        assertEquals(emptyList<MemoryFact>(), dao.getAll())
    }

    @Test
    fun deleteOldest_keepsNewestNByCreatedAt() = runBlocking {
        val t0 = 1_720_000_000_000L
        repeat(5) { i ->
            dao.insert(
                MemoryFact(
                    content = "事实$i",
                    keywords = "k$i",
                    createdAt = t0 + i,
                    lastUsedAt = t0 + i,
                )
            )
        }

        dao.deleteOldest(keepCount = 3)

        val remaining = dao.getAll()
        assertEquals("应保留最近 3 条", 3, remaining.size)
        assertEquals(
            "保留的是 createdAt 最大的 3 条",
            listOf("事实2", "事实3", "事实4"),
            remaining.sortedBy { it.createdAt }.map { it.content }
        )
    }

    @Test
    fun deleteOldest_keepCountZero_removesAll() = runBlocking {
        val t0 = 1_720_000_000_000L
        repeat(3) { i ->
            dao.insert(
                MemoryFact(content = "事实$i", keywords = "k$i", createdAt = t0 + i, lastUsedAt = t0 + i)
            )
        }

        dao.deleteOldest(keepCount = 0)

        assertTrue("keepCount=0 应删除全部", dao.getAll().isEmpty())
    }
}
