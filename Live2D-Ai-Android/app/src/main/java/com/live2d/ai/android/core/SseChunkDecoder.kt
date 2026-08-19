package com.live2d.ai.android.core

import com.live2d.ai.android.ChatResponse
import kotlinx.serialization.json.Json

/**
 * SSE 流解析事件。
 */
sealed interface SseEvent {
    /** 一条增量片段（`data:` JSON 中 choices[0].delta.content）。 */
    data class Delta(val fragment: String) : SseEvent

    /** 收到 `[DONE]`：流终止。 */
    data object Done : SseEvent
}

/**
 * SSE 增量解析器（状态机，用于 LLM 流式响应）。
 *
 * 行为（2C-2 锚 2）：
 * - [feed]：喂入任意切分的原始 SSE 文本（可含多行/半行），解析完整行后返回本块事件；
 * - `data:` 载荷增量累积（[accumulatedText]）；`[DONE]` 终止（[isDone]，其后 feed 不再产出）；
 * - malformed 行（非 JSON / 无 delta）→ 跳过；心跳/注释行（`:` 开头）与空行 → 忽略；
 * - [close]：流关闭冲刷——未收到 `[DONE]` 时处理残余半行，发出已累积增量。
 *
 * 用法：调用方把网络读取到的原始文本按块喂入 [feed]，把返回的 [SseEvent.Delta]
 * 逐条转发给上层；读到 [SseEvent.Done] 或底层流 EOF 后调用 [close] 冲刷。
 */
class SseChunkDecoder {

    private val lineBuffer = StringBuilder()
    private val accumulated = StringBuilder()
    private var done = false

    private val json = Json { ignoreUnknownKeys = true }

    /** 已累积的完整增量文本（全部已产出 Delta 片段的拼接）。 */
    val accumulatedText: String get() = accumulated.toString()

    /** 是否已收到 `[DONE]`（流终止）。 */
    val isDone: Boolean get() = done

    /**
     * 喂入一块原始 SSE 文本，返回本块解析出的事件（[SseEvent.Delta] / [SseEvent.Done]）。
     *
     * 内部按 `\n` 切分完整行逐行处理；行尾残缺（无换行）的文本留在行缓冲，
     * 由下一次 [feed] 补全或由 [close] 冲刷。收到 `[DONE]` 后，后续 feed 不再产出事件。
     */
    fun feed(chunk: String): List<SseEvent> {
        if (done || chunk.isEmpty()) return emptyList()
        lineBuffer.append(chunk)
        val events = mutableListOf<SseEvent>()
        while (!done) {
            val newline = lineBuffer.indexOf("\n")
            if (newline < 0) break
            val line = lineBuffer.substring(0, newline).trimEnd('\r')
            lineBuffer.delete(0, newline + 1)
            processLine(line, events)
        }
        if (done) lineBuffer.clear()
        return events
    }

    /**
     * 流关闭冲刷：未收到 `[DONE]` 且存在残余半行时，把它当作最后一行处理并返回其事件
     * （[SseEvent.Delta] 或 [SseEvent.Done]）；无残余 / 已终止 / 残余无可产出 → 返回 null。
     */
    fun close(): SseEvent? {
        if (done || lineBuffer.isEmpty()) return null
        val residual = lineBuffer.toString().trimEnd('\r')
        lineBuffer.clear()
        val events = mutableListOf<SseEvent>()
        processLine(residual, events)
        return events.firstOrNull()
    }

    private fun processLine(line: String, out: MutableList<SseEvent>) {
        if (line.isBlank()) return            // 空行（SSE 事件分隔/心跳）
        if (line.startsWith(":")) return      // 注释行（`: keep-alive` 心跳）忽略
        if (!line.startsWith("data:")) return // event:/id:/retry: 等其他字段忽略
        val payload = line.removePrefix("data:").trim()
        if (payload == "[DONE]") {
            done = true
            out.add(SseEvent.Done)
            return
        }
        val fragment = parseDelta(payload)
        if (fragment == null || fragment.isEmpty()) return // malformed / 无内容 → 跳过
        accumulated.append(fragment)
        out.add(SseEvent.Delta(fragment))
    }

    /** 解析 `data:` 载荷中的增量文本；非法 JSON / 结构不符 → null（malformed 跳过）。 */
    private fun parseDelta(payload: String): String? = try {
        json.decodeFromString<ChatResponse>(payload)
            .choices?.firstOrNull()?.delta?.content
    } catch (_: Exception) {
        null
    }
}
