package com.live2d.ai.android.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * SSE 流式解析纯函数 RED（契约 refactor-0814 §2.2 LLM 链路 + 架构 shell §6 SSE 现状评估，Wave 2C-2）。
 *
 * 覆盖生产文件 `core/LlmLink.kt` 内 **SseChunkDecoder**（**新增，RED 待实现**，纯 Kotlin 无网络依赖）。
 * class name 唯一：`LlmLink*Test`；test core 目录现无 `SseChunkDecoder`/`SseEvent` 符号。
 *
 * 生产 seam spec（对齐架构 §6「逐 delta 发射、[DONE] 收口、malformed 跳过、心跳/注释忽略」）：
 *   sealed interface SseEvent {
 *       data class Delta(val fragment: String) : SseEvent        // content 增量
 *       data object Done : SseEvent                              // [DONE] 终止
 *       data object Malformed : SseEvent                         // 该 chunk 无 content 增量 → 跳过
 *   }
 *   class SseChunkDecoder {
 *       fun feedLine(line: String): List<SseEvent>   // 逐行喂入；返回本行产生的事件（0..N）
 *       fun isDone(): Boolean                          // 已遇 data: [DONE]
 *       fun onSourceClose(): List<SseEvent>            // 流关闭冲刷残段（末块无 [DONE]）
 *   }
 *
 * ============================ SSE 帧语义（锁定） ============================
 * - `data: <payload>`         data 帧，多 `data:` 行跨行累积为一块；空行触发提交
 * - `data: [DONE]`            终止事件（isDone=true）
 * - `: comment` / 空行        心跳/注释/空白 → 忽略
 * - malformed（非 JSON / 无 choices.delta.content）→ 该块跳过（Malformed，不发射 Delta、不置 Done）
 * - 流关闭残段（accumulate 有块但未提交，且无 [DONE]）→ onSourceClose() 冲刷发射其 Delta
 * ==========================================================================
 */
class LlmLinkSseTest {

    private fun decoder() = SseChunkDecoder()

    /** 本地适配：逐行喂入（生产 seam 为按块 feed；行 = 完整行 + 换行符）。 */
    private fun SseChunkDecoder.feedLine(line: String): List<SseEvent> = feed(line + "\n")

    // 单个合法 delta payload（DeepSeek SSE 块）
    private fun deltaJson(content: String): String =
        "data: {\"choices\":[{\"delta\":{\"content\":\"$content\"}}]}"

    // ═══════════════════════════════════════════════════════════════
    // 一、delta 累积（逐 delta 发射；拼接 = 全量回复）
    // ═══════════════════════════════════════════════════════════════

    /** 单个合法 chunk（JSON choices[].delta.content）→ 发射 [Delta("你")]，isDone=false。 */
    @Test
    fun singleChunk_emitsDelta() {
        val d = decoder()
        val events = d.feedLine(deltaJson("你"))
        assertEquals(listOf<SseEvent>(SseEvent.Delta("你")), events)
        assertFalse("单 chunk 未遇 [DONE]，流应未完", d.isDone)
    }

    /** 连续 2 个 chunk → 各发射一个 Delta；拼接 "你"+"好" == "你好"（delta 累积成全文）。 */
    @Test
    fun successiveChunks_deltasAccumulateToFullText() {
        val d = decoder()
        val f1 = d.feedLine(deltaJson("你"))
        val f2 = d.feedLine(deltaJson("好"))
        assertEquals(listOf<SseEvent>(SseEvent.Delta("你")), f1)
        assertEquals(listOf<SseEvent>(SseEvent.Delta("好")), f2)
        val full = (f1 + f2).filterIsInstance<SseEvent.Delta>().joinToString("") { it.fragment }
        assertEquals("delta 累积应拼接成完整回复", "你好", full)
    }

    /** 网络分帧把完整 data 行拆成两段：前段无换行（半行留行缓冲），后段补全 → 重组解出 Delta("你好")。 */
    @Test
    fun splitChunk_frameBoundaryHalfLine_reassembledToDelta() {
        val d = decoder()
        // 分帧边界：完整行 `data: {...content:"你好"...}` 被拆为两个 chunk
        val first = d.feed("data: {\"choices\":[{\"delta\":{\"content\":\"你")
        assertTrue("无换行半行不产出（留行缓冲）", first.isEmpty())
        val second = d.feed("好\"}}]}\n")
        assertTrue(
            "跨 feed 分帧边界半行缓冲重组后解出完整增量",
            second.filterIsInstance<SseEvent.Delta>().joinToString("") { it.fragment } == "你好",
        )
    }

    // ═══════════════════════════════════════════════════════════════
    // 二、[DONE] 终止
    // ═══════════════════════════════════════════════════════════════

    /** `data: [DONE]` → 返回 [Done]，isDone()==true。 */
    @Test
    fun doneLine_terminatesStream() {
        val d = decoder()
        val events = d.feedLine("data: [DONE]")
        assertTrue("[DONE] 应发射 Done 事件", events.contains(SseEvent.Done))
        assertTrue("[DONE] 后流应终止（isDone）", d.isDone)
    }

    /** [DONE] 前已有合法 delta → 两者均保留（delta 先、Done 后）。 */
    @Test
    fun deltaThenDone_streamEndsAfterDelta() {
        val d = decoder()
        d.feedLine(deltaJson("完成"))
        val doneEvents = d.feedLine("data: [DONE]")
        assertTrue("delta 后接 [DONE]，Done 事件存在", doneEvents.contains(SseEvent.Done))
        assertTrue(d.isDone)
    }

    // ═══════════════════════════════════════════════════════════════
    // 三、malformed chunk 跳过
    // ═══════════════════════════════════════════════════════════════

    /** 非 JSON data → 该块跳过（Malformed），不发射 Delta、不置 Done。 */
    @Test
    fun nonJsonData_skipsAsMalformed() {
        val d = decoder()
        val events = d.feedLine("data: {not-valid-json")
        assertTrue("非 JSON 块应静默跳过（无事件产出）", events.isEmpty())
        assertFalse("malformed 不应终止流", d.isDone)
        assertTrue("malformed 不应发射 Delta", events.none { it is SseEvent.Delta })
    }

    /** JSON 合法但缺 choices.delta.content（空 content / 无 delta）→ 跳过度（无增量发射）。 */
    @Test
    fun jsonWithoutContentDelta_skips() {
        val d = decoder()
        val events = d.feedLine("data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}")
        assertTrue("无 content 增量应静默跳过（无事件产出）", events.isEmpty())
        assertTrue("无 content 不应发射 Delta", events.none { it is SseEvent.Delta })
        assertFalse("缺 content 不应误判流结束", d.isDone)
    }

    // ═══════════════════════════════════════════════════════════════
    // 四、心跳/注释/空行忽略
    // ═══════════════════════════════════════════════════════════════

    /** 心跳 `: ...` → 空（忽略，不发射、不置 Done）。 */
    @Test
    fun heartbeatLine_ignored() {
        val d = decoder()
        val events = d.feedLine(": keep-alive")
        assertEquals("心跳注释行应被忽略", emptyList<SseEvent>(), events)
        assertFalse("心跳不应终止流", d.isDone)
    }

    /** 纯空行（无 data 累积时的空行 → 空白，忽略）。 */
    @Test
    fun blankLineWithoutAccumulatedData_ignored() {
        val d = decoder()
        val events = d.feedLine("")
        assertEquals("无累积数据的空行应忽略", emptyList<SseEvent>(), events)
    }

    /** 块内 jibber（如分帧边界残留非 data 前缀、注释）→ 忽略。 */
    @Test
    fun unrelatedLine_ignored() {
        val d = decoder()
        val events = d.feedLine("event: message")
        assertEquals("非 data/ 且非注释前缀行应忽略", emptyList<SseEvent>(), events)
        assertFalse(d.isDone)
    }

    // ═══════════════════════════════════════════════════════════════
    // 五、末块无 [DONE] 的流关闭语义
    // ═══════════════════════════════════════════════════════════════

    /** 流关闭时残段为完整 data 块（从未遇 [DONE]）→ onSourceClose() 冲刷发射其 Delta。 */
    @Test
    fun onClose_flushesResidualDataBlock_withoutDone() {
        val d = decoder()
        // 流在 data 行中截断（无换行残段留行缓冲）：未提交、也未遇 [DONE]
        d.feed(deltaJson("收尾"))
        assertFalse("尚未 [DONE] 也未提交", d.isDone)
        val flushed = listOfNotNull(d.close())
        assertTrue(
            "无 [DONE] 的末块应收尾发射其内容",
            flushed.filterIsInstance<SseEvent.Delta>().joinToString("") { it.fragment } == "收尾",
        )
    }

    /** 流关闭时残段为空 / 无内容 → onSourceClose() 返回空。 */
    @Test
    fun onClose_withEmptyResidue_returnsEmpty() {
        val d = decoder()
        d.feedLine(": heartbeat")                 // 无 data 累积
        val flushed = listOfNotNull(d.close())
        assertEquals("无内容残段关闭应收尾为空", emptyList<SseEvent>(), flushed)
    }

    /** [DONE] 已发生后再 onSourceClose → 不再发射新事件（流已终止）。 */
    @Test
    fun onClose_afterDone_doesNotEmitAgain() {
        val d = decoder()
        d.feedLine("data: [DONE]")
        assertTrue(d.isDone)
        val flushed = listOfNotNull(d.close())
        assertEquals("已 [DONE] 后流关闭不应再发射", emptyList<SseEvent>(), flushed)
    }
}
