package com.live2dai.plugin.memory

import android.content.Context
import androidx.room.Database
import androidx.room.Room
import androidx.room.RoomDatabase

/**
 * 长期记忆存储（Room / SQLite）。
 *
 * MVP 阶段不引入迁移版本管理（exportSchema = false，schema 变更直接清库重建），
 * 数据量级为个人项目（数百条以内），无需分库。
 */
@Database(entities = [MemoryFact::class], version = 1, exportSchema = false)
abstract class MemoryDatabase : RoomDatabase() {

    abstract fun memoryDao(): MemoryDao

    companion object {
        /** 构建数据库实例（懒打开：build 不触达 SQLite，首次查询才真正打开） */
        fun build(context: Context, databaseName: String): MemoryDatabase =
            Room.databaseBuilder(context, MemoryDatabase::class.java, databaseName)
                .build()
    }
}
