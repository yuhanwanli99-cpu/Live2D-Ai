package com.live2dai.android

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * ChatHistoryStore 单元测试（Robolectric，使用真实 filesDir）。
 *
 * 覆盖（V1.1 模块2 契约，见 docs/plan-v11-plan.schema.json）：
 * - 保存/读取往返一致（含 id 等全部字段；id 必须持久化，LazyColumn key 稳定性依赖它）
 * - 文件不存在 → 空列表
 * - 文件损坏 → 空列表 + 删除文件（下次保存重建）
 * - 保存时 trim 到最近 maxMessages 条
 * - clear 删除文件，后续 load 返回空
 * - 旧版本数据缺 id 字段 → 加载成功并补默认 id（默认值兼容）
 * - 版本不兼容 → 空列表，不崩溃
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class, sdk = [34])
class ChatHistoryStoreTest {

    private val context: Context = ApplicationProvider.getApplicationContext()
    private val historyFile: File
        get() = File(context.filesDir, "chat_history.json")

    @Before
    fun setUp() {
        // 每个用例从干净状态开始（等价于冷启动无历史）
        historyFile.delete()
    }

    @Test
    fun saveAndLoad_roundTrip_preservesAllFields() {
        val messages = listOf(
            DisplayMessage(content = "你好", isUser = true),
            DisplayMessage(content = "喵~", isUser = false, isError = true, showSettingsButton = true)
        )

        ChatHistoryStore.save(context, messages)

        val loaded = ChatHistoryStore.load(context)
        assertEquals(messages.size, loaded.size)
        messages.zip(loaded).forEach { (expected, actual) ->
            assertEquals("id 必须持久化（LazyColumn key 依赖）", expected.id, actual.id)
            assertEquals(expected.content, actual.content)
            assertEquals(expected.isUser, actual.isUser)
            assertEquals(expected.isError, actual.isError)
            assertEquals(expected.showSettingsButton, actual.showSettingsButton)
        }
    }

    @Test
    fun load_whenFileMissing_returnsEmptyList() {
        assertTrue("前置清理", historyFile.delete() || !historyFile.exists())

        val loaded = ChatHistoryStore.load(context)

        assertEquals(emptyList<DisplayMessage>(), loaded)
    }

    @Test
    fun load_whenFileCorrupted_returnsEmptyAndDeletesFile() {
        historyFile.writeText("{ 这不是合法 JSON !!!")

        val loaded = ChatHistoryStore.load(context)

        assertEquals("损坏文件必须返回空列表而非崩溃", emptyList<DisplayMessage>(), loaded)
        assertTrue("损坏文件应删除重建", !historyFile.exists())
    }

    @Test
    fun save_trimsToMostRecentMaxMessages() {
        val messages = (1..5).map { DisplayMessage(content = "msg$it", isUser = it % 2 == 0) }

        ChatHistoryStore.save(context, messages, maxMessages = 3)

        val loaded = ChatHistoryStore.load(context)
        assertEquals(3, loaded.size)
        assertEquals("只保留最近 3 条", listOf("msg3", "msg4", "msg5"), loaded.map { it.content })
    }

    @Test
    fun clear_removesFileAndLoadReturnsEmpty() {
        ChatHistoryStore.save(context, listOf(DisplayMessage(content = "x", isUser = true)))
        assertTrue("保存后文件应存在", historyFile.exists())

        ChatHistoryStore.clear(context)

        assertTrue("clear 后文件应删除", !historyFile.exists())
        assertEquals(emptyList<DisplayMessage>(), ChatHistoryStore.load(context))
    }

    @Test
    fun load_whenIdFieldMissing_usesDefaultAndDoesNotCrash() {
        // 模拟旧版本/手工数据：无 id 字段时必须能加载，且自动补默认 id
        historyFile.writeText(
            """{"version":1,"messages":[{"content":"旧数据","isUser":true,"isError":false,"showSettingsButton":false}]}"""
        )

        val loaded = ChatHistoryStore.load(context)

        assertEquals(1, loaded.size)
        assertEquals("旧数据", loaded[0].content)
        assertTrue("缺失 id 时应补默认值", loaded[0].id.isNotBlank())
    }

    @Test
    fun load_whenVersionIncompatible_returnsEmptyWithoutCrash() {
        historyFile.writeText(
            """{"version":999,"messages":[{"id":"a","content":"未来版本数据","isUser":true,"isError":false,"showSettingsButton":false}]}"""
        )

        val loaded = ChatHistoryStore.load(context)

        assertEquals("版本不兼容必须返回空列表", emptyList<DisplayMessage>(), loaded)
    }
}
