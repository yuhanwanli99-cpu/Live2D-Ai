package com.live2dai.plugin.memory

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.Query
import androidx.room.Update

/**
 * 记忆事实的数据访问层（Room DAO），见 docs/plan-task-plugin-memory.md §3.3。
 *
 * 检索打分（关键词重叠）刻意不下沉到 SQL：MVP 数据量小（个人项目量级），
 * 在 [MemoryDao.getAll] 之上用 Kotlin 代码做全表扫描 + 内存打分足够简单可靠。
 */
@Dao
interface MemoryDao {

    @Insert
    suspend fun insert(fact: MemoryFact): Long

    @Query("SELECT * FROM memory_facts")
    suspend fun getAll(): List<MemoryFact>

    @Update
    suspend fun update(fact: MemoryFact)

    /**
     * 记忆上限裁剪：保留最近 [keepCount] 条（按 createdAt 降序，同分取 id 大者），
     * 超出部分删除；keepCount <= 0 时删除全部。
     * 见 docs/plan-v11-plan.schema.json → MemoryDao.deleteOldest。
     */
    @Query(
        "DELETE FROM memory_facts WHERE id NOT IN " +
            "(SELECT id FROM memory_facts ORDER BY createdAt DESC, id DESC LIMIT :keepCount)"
    )
    suspend fun deleteOldest(keepCount: Int)
}
