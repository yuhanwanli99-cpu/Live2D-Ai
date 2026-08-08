package com.live2dai.android

import android.content.Context
import android.util.Log
import com.live2dai.android.ui.DisplayMessage
import java.io.File
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/**
 * 聊天历史 JSON 持久化工具（V1.1 模块2）。
 *
 * 职责（契约见 docs/plan-v11-plan.schema.json → ChatHistoryStore）：
 * - [save]：将 [DisplayMessage] 列表序列化为 JSON 写入 `filesDir/chat_history.json`，
 *   写入前自动 trim 到最近 [maxMessages] 条
 * - [load]：读取并反序列化；文件不存在 / 损坏 / 版本不兼容 → 返回空列表，绝不崩溃
 * - [clear]：删除历史文件；文件不存在时无操作
 *
 * 设计决策：
 * - 只持久化 UI 显示消息（DisplayMessage），**不持久化 ChatService.messages**
 *   （架构约束 §8.2-4：重启后 ChatService 从头开始，system prompt 重新注入）
 * - JSON 带 `version` 字段（当前 1），为未来迁移预留；损坏文件删除重建，版本不兼容保留文件仅返回空
 * - 所有读写使用 runCatching 容错 + 日志（TAG "ChatHistoryStore"），调用方（MainActivity）
 *   负责在 IO 线程调度；本 store 自身不抛异常
 * - Json 配置：`encodeDefaults = true` 确保 id 等带默认值的字段总是写入
 *   （id 是 LazyColumn key，必须持久化）；`ignoreUnknownKeys` 容忍未来新增字段
 */
object ChatHistoryStore {

    private const val TAG = "ChatHistoryStore"
    private const val FILE_NAME = "chat_history.json"
    private const val VERSION = 1
    private const val DEFAULT_MAX_MESSAGES = 200

    private val json = Json {
        encodeDefaults = true
        ignoreUnknownKeys = true
        explicitNulls = false
    }

    /** 磁盘文件结构：version 用于未来迁移，messages 为显示消息列表 */
    @Serializable
    private data class ChatHistoryFile(
        val version: Int = VERSION,
        val messages: List<DisplayMessage> = emptyList()
    )

    /**
     * 保存消息列表（自动 trim 到最近 [maxMessages] 条）。
     * 任何失败仅记录日志，不向上抛异常。
     */
    fun save(context: Context, messages: List<DisplayMessage>, maxMessages: Int = DEFAULT_MAX_MESSAGES) {
        runCatching {
            val trimmed = messages.takeLast(maxMessages)
            val payload = ChatHistoryFile(version = VERSION, messages = trimmed)
            val file = File(context.filesDir, FILE_NAME)
            file.writeText(json.encodeToString(ChatHistoryFile.serializer(), payload))
            Log.d(TAG, "Saved ${trimmed.size} messages to ${file.absolutePath}")
        }.onFailure { e ->
            Log.w(TAG, "Failed to save chat history", e)
        }
    }

    /**
     * 读取消息列表。
     * - 文件不存在 → 空列表
     * - 文件损坏（解析失败）→ 空列表 + 删除文件（下次保存重建）
     * - 版本不兼容 → 空列表（保留文件，不销毁未来可迁移的数据）
     * 绝不崩溃。
     */
    fun load(context: Context): List<DisplayMessage> {
        val file = File(context.filesDir, FILE_NAME)
        if (!file.exists()) {
            return emptyList()
        }
        return try {
            val parsed = json.decodeFromString(ChatHistoryFile.serializer(), file.readText())
            if (parsed.version != VERSION) {
                Log.w(TAG, "Unsupported version ${parsed.version}, expected $VERSION; returning empty")
                emptyList()
            } else {
                parsed.messages
            }
        } catch (e: Exception) {
            Log.w(TAG, "Failed to load chat history; deleting corrupted file", e)
            file.delete()
            emptyList()
        }
    }

    /**
     * 清空聊天历史：删除历史文件；文件不存在时无操作。
     */
    fun clear(context: Context) {
        val file = File(context.filesDir, FILE_NAME)
        if (file.exists()) {
            val deleted = file.delete()
            if (deleted) {
                Log.d(TAG, "Cleared chat history file ${file.absolutePath}")
            } else {
                Log.w(TAG, "Failed to delete chat history file ${file.absolutePath}")
            }
        }
    }
}
