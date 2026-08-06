package com.live2dai.android

import android.content.Context
import android.util.Log
import com.live2dai.plugin.memory.MemoryDatabase
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * 主动陪伴的话题记忆源（V1.2 记忆驱动话题，docs/plan-v12-plan.md §5.6.3）。
 *
 * 读取记忆插件（live2dai-plugin-memory）的 Room 库，取最近创建的若干条事实
 * 作为 LLM 生成主动话题的上下文。与记忆插件使用同一数据库文件
 * （默认 `live2dai_memory.db`，见 MemoryPluginConfig.databaseName），
 * 数据库实例进程内缓存复用，避免重复打开连接。
 *
 * 容错（plan §8.2-2）：读取失败（数据库未初始化 / SQLite 异常）静默返回空列表——
 * 主动话题退化为「仅按人设+时间生成」，不影响主动陪伴链路。
 * 日志 TAG "Proactive"，Log.i 级别（国产 ROM 抑制 Log.d 教训）。
 */
object ProactiveMemorySource {

    private const val TAG = "Proactive"

    /** 与记忆插件默认数据库文件名保持一致（MemoryPluginConfig.databaseName 默认值） */
    private const val DATABASE_NAME = "live2dai_memory.db"

    /** 送入 LLM 的最近记忆条数上限（控制 prompt 体积） */
    private const val DEFAULT_LIMIT = 5

    @Volatile
    private var database: MemoryDatabase? = null

    /**
     * 取最近创建的 [limit] 条记忆事实文本（按 createdAt 降序）。
     *
     * @return 记忆内容列表；数据库不可用/失败时返回空列表
     */
    suspend fun recentFacts(context: Context, limit: Int = DEFAULT_LIMIT): List<String> =
        withContext(Dispatchers.IO) {
            try {
                val db = database
                    ?: MemoryDatabase.build(context.applicationContext, DATABASE_NAME).also { database = it }
                db.memoryDao().getAll()
                    .sortedByDescending { it.createdAt }
                    .take(limit)
                    .map { it.content }
            } catch (e: Exception) {
                logWarn("读取最近记忆失败（静默降级为空列表）: ${e.message}")
                emptyList()
            }
        }

    /** 测试辅助：重置进程内数据库缓存（避免单测间状态泄漏）。 */
    internal fun resetForTest() {
        database = null
    }

    private fun logWarn(message: String) {
        try {
            Log.w(TAG, message)
        } catch (_: Throwable) {
            // 单元测试环境：静默忽略
        }
    }
}
