package com.live2dai.android.core

import com.live2dai.android.ChatMessage
import com.live2dai.android.PersonaConfig
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 历史修剪 + persona 注入 RED（契约 refactor-0814 §2.2「历史修剪 50 对；persona 注入」+ 架构 shell §6 persona，
 * Wave 2C-2）。
 *
 * 覆盖生产文件 `core/LlmLink.kt`/`core/LlmModels.kt` 的 **ChatHistoryBuilder**、**PersonaConfig**、
 * `HISTORY_MAX_PAIRS`（**新增，RED 待实现**，纯 Kotlin）。
 * class name 唯一：`LlmLink*Test`；test core 目录现无 `ChatHistoryBuilder`/`PersonaConfig` 符号。
 *
 * 生产 seam spec：
 *   data class PersonaConfig(
 *       val name: String,
 *       val systemPrompt: String,       // 含 emotionInstruction 拼接（架构 §6 persona 注入）
 *       val model: String,
 *       val temperature: Double,
 *       val maxTokens: Int,
 *   )
 *   const val HISTORY_MAX_PAIRS = 50    // 契约：历史修剪 50 对
 *   class ChatHistoryBuilder(maxPairs: Int = HISTORY_MAX_PAIRS) {
 *       // 返回：[system, (u0,a0)...(uK-1,aK-1), userText]
 *       //  - system 恒为 index0（persona 注入位置，system 首位）
 *       //  - historyPairs 超长 → 仅保留最近 maxPairs 对（保序，先旧后新）
 *       //  - userText 追加末位（刚发消息）
 *       fun build(persona: PersonaConfig, historyPairs: List<Pair<String, String>>, userText: String): List<LlmMessage>
 *   }
 */
class LlmLinkHistoryTest {

    /** PersonaConfig 为 object（无构造参数）；extra 参数仅为兼容旧调用点，提示读生产 getSystemPrompt()。 */
    private fun persona(extra: String = "") = PersonaConfig

    private fun pairs(n: Int): List<Pair<String, String>> =
        (1..n).map { "用户问题$it" to "助手回答$it" }

    /** 构建请求消息：setSystem(persona 提示) + 逐对 addTurn + withUser(userText)。 */
    private fun build(
        persona: PersonaConfig,
        historyPairs: List<Pair<String, String>>,
        userText: String,
        maxPairs: Int = HISTORY_MAX_PAIRS,
    ): List<ChatMessage> {
        val builder = ChatHistoryBuilder(maxPairs = maxPairs)
        builder.setSystem(persona.getSystemPrompt())
        historyPairs.forEach { (user, assistant) -> builder.addTurn(user, assistant) }
        return builder.withUser(userText)
    }

    // ═══════════════════════════════════════════════════════════════
    // 一、超长历史截断（保留最近 50 对）
    // ═══════════════════════════════════════════════════════════════

    /** 60 对超长历史 → 仅保留最近 50 对；HISTORY_MAX_PAIRS==50。 */
    @Test
    fun overlongHistory_trimsToRecent50() {
        assertEquals("契约锁定 HISTORY_MAX_PAIRS==50", 50, HISTORY_MAX_PAIRS)
        val result = build(persona(), pairs(60), "当前问题")
        // 结构 = [system] + 50 对 + [userText]
        assertEquals("输出条数 = 1(system) + 2×50 条对话 + 1(user)", 1 + 2 * 50 + 1, result.size)
        assertEquals("首元素 persona/system", "system", result.first().role)
        assertEquals("末元素当前 user 消息", "user", result.last().role)
        assertTrue("末元素内容是当前问题", result.last().content == "当前问题")
        // 修剪起点：第 11 对（问题11）为保留区首条（丢掉了 1..10 对）
        assertEquals("保留区首条 = 问题11", "用户问题11", result[1].content)
        assertTrue("保留区内不含第 10 对（被截断）", result.none { it.content == "用户问题10" })
    }

    /** 恰好 50 对 → 全部保留（不误剪、不抛）。 */
    @Test
    fun exactly50Pairs_keptAll() {
        val result = build(persona(), pairs(50), "问题")
        assertEquals("恰好 50 对时不截断（1+2×50+1）", 1 + 2 * 50 + 1, result.size)
        assertEquals("保留区首条 = 问题1（50 对全留）", "用户问题1", result[1].content)
        assertEquals("保留区末条 = 问题50", "用户问题50", result[result.size - 3].content)
    }

    /** 截断保序：保留的 50 对相对顺序 先旧后新。 */
    @Test
    fun trimmedHistory_preservesRelativeOrder() {
        val result = build(persona(), pairs(60), "问题")
        // 从第11对起：`用户问题11..问题60` 依序、且助手回答跟在各自用户问题后（u,a 交替）
        val turnContents = result.drop(1).dropLast(1).map { it.content }
        val userIdx = turnContents.indices.filter { it % 2 == 0 }
        val expectedUsers = (11..60).map { "用户问题$it" }
        assertEquals("保留的 user 轮依序 = 问题11..问题60（保序不翻转）", expectedUsers, userIdx.map { turnContents[it] })
    }

    // ═══════════════════════════════════════════════════════════════
    // 二、persona 注入位置（system 首位）
    // ═══════════════════════════════════════════════════════════════

    /** persona 注入：system 恒为 index0，content 含 persona.systemPrompt。 */
    @Test
    fun personaInjected_atSystemFirstPosition() {
        val persona = persona("·请用[快乐]等标签表达情绪。")
        val result = build(persona, pairs(3), "你好")
        assertEquals("index0 恒为 system（persona 注入首位）", "system", result.first().role)
        assertTrue("index0 content 含 persona.systemPrompt", result.first().content == PersonaConfig.getSystemPrompt())
        assertTrue("persona system 含 emotionInstruction 拼接（架构§6）", result.first().content?.contains("标签") == true)
    }

    /** 短历史（3 对）→ 不修剪、全保留仍 system 首位。 */
    @Test
    fun shortHistory_notTrimmed_systemFirst() {
        val result = build(persona(), pairs(3), "话")
        assertEquals("短历史全保留（1+2×3+1）", 1 + 2 * 3 + 1, result.size)
        assertEquals("system 首位", "system", result.first().role)
        assertEquals("user 当前消息末位", "话", result.last().content)
    }

    /** 零历史 → [system] + [userText]（两元结构）。 */
    @Test
    fun emptyHistory_buildsSystemPlusUserOnly() {
        val result = build(persona(), emptyList(), "单独问")
        assertEquals(2, result.size)
        assertEquals(listOf("system", "user"), result.map { it.role })
        assertTrue(result.last().content == "单独问")
    }

    /** ChatHistoryBuilder 可自定义 maxPairs（非硬编码 50 时逻辑仍成立）。 */
    @Test
    fun customMaxPairs_respected() {
        val result = build(persona(), pairs(5), "问", maxPairs = 2)
        assertEquals("自定义 maxPairs=2 → 1(system)+2×2+1", 1 + 2 * 2 + 1, result.size)
        assertEquals("保留区首条=问题4（最近2对：4,5）", "用户问题4", result[1].content)
    }

}
