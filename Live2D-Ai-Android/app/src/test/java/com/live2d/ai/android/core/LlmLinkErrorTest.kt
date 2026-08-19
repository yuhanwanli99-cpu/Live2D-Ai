package com.live2d.ai.android.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 429/额度/网络/超时 错误分类 RED（契约 refactor-0814 §2.6 失败语义 + 架构 shell §6 错误处理，Wave 2C-2）。
 *
 * 覆盖生产文件 `core/LlmLink.kt` 内 **classifyError / classifyNetworkError**（**新增，RED 待实现**，纯 Kotlin）。
 * class name 唯一：`LlmLink*Test`；test core 目录现无 `LlmError`/`classifyError` 符号。
 *
 * 生产 seam spec（对齐契约 §2.6「429/额度关键词分类保留 + 中文文案 + quotaExhausted」）：
 *   sealed interface LlmError {
 *       val userMessage: String                       // 中文可读文案
 *       data class RateLimited(override val userMessage: String) : LlmError       // HTTP 429
 *       data class QuotaExhausted(override val userMessage: String) : LlmError    // 403/额度关键词
 *       data class Network(override val userMessage: String) : LlmError           // IO/连接
 *       data class Timeout(override val userMessage: String) : LlmError           // 超时
 *       data class Unknown(override val userMessage: String) : LlmError           // 5xx/其他
 *       fun toFailed(): LlmEvent.Failed   // 429/Network/Timeout/Unknown→quotaExhausted=false; QuotaExhausted→true
 *   }
 *   fun classifyError(httpStatus: Int, body: String): LlmError
 *   fun classifyNetworkError(t: Throwable): LlmError   // IOException→Network; TimeoutException→Timeout
 *
 * 文案锚定旧 ChatService 已真机验证且架构 §6 判「保留」的中文文案**子串**（详见 RED 报告 §6 [UNSURE-2]）：
 *   请求过于频繁 / 稍后重试 / 免费额度 / 设置 / API Key / 1 并发 / 错误
 */
class LlmLinkErrorTest {

    /** 本地适配：错误 → 失败事件（生产 LlmError 无 toFailed 扩展；Failed 事件由链路层分类后产出）。 */
    private fun LlmError.toFailed() = LlmEvent.Failed(message, quotaExhausted)

    // ═══════════════════════════════════════════════════════════════
    // 一、HTTP 状态 → 分类枚举
    // ═══════════════════════════════════════════════════════════════

    /** 429（限 1 并发）→ RateLimited；toFailed().quotaExhausted==false；文案含 请求过于频繁/稍后重试。 */
    @Test
    fun status429_classifiesAsRateLimited_userFriendly() {
        val err = classifyError(429, """{"error":{"code":"1301","message":"用户请求过于频繁"}}""")
        assertTrue("429 应为限流（文案含 请求过于频繁 + 稍后重试）", err.message.contains("请求过于频繁") && err.message.contains("稍后重试"))
        val failed = err.toFailed()
        assertEquals("429 不应标记额度耗尽", false, failed.quotaExhausted)
        assertTrue("应提示请求过于频繁", failed.message.contains("请求过于频繁"))
        assertTrue("应提示稍后重试", failed.message.contains("稍后重试"))
    }

    /** 403（账户余额不足 1002）→ QuotaExhausted；quotaExhausted=true；文案含 免费额度/设置/API Key，不含 稍后重试。 */
    @Test
    fun status403_balanceExhausted_quotaExhausted_guidesSwapKey() {
        val err = classifyError(403, """{"error":{"code":"1002","message":"账户余额不足"}}""")
        assertTrue("403 余额不足应标记额度耗尽", err.quotaExhausted)
        val failed = err.toFailed()
        assertEquals("额度耗尽须标记 quotaExhausted=true", true, failed.quotaExhausted)
        assertTrue("应说明当前使用官方免费额度", failed.message.contains("免费额度"))
        assertTrue("应引导前往设置", failed.message.contains("设置"))
        assertTrue("应引导填写自己的 API Key", failed.message.contains("API Key"))
        assertFalse("额度耗尽不应按限流重试处理", failed.message.contains("稍后重试"))
    }

    /** 400 + body 命中额度关键词（免费额度已用尽/余额不足）→ QuotaExhausted；quotaExhausted=true。 */
    @Test
    fun status400_withQuotaKeyword_classifiesQuotaExhausted() {
        val err = classifyError(400, """{"error":{"code":"2003","message":"该模型的免费额度已用尽，请充值或更换模型"}}""")
        assertTrue("400 额度关键词应识别为额度耗尽", err.quotaExhausted)
        val failed = err.toFailed()
        assertTrue("额度耗尽须标记 quotaExhausted=true", failed.quotaExhausted)
        assertTrue("应引导填写自己的 API Key", failed.message.contains("API Key"))
    }

    /** 400 无额度关键词 → 非 QuotaExhausted；quotaExhausted=false（通用分类）。 */
    @Test
    fun status400_withoutQuotaKeyword_notQuota() {
        val err = classifyError(400, """{"error":{"code":"2000","message":"参数错误"}}""")
        assertFalse("400 无额度关键词不应判额度耗尽", err.quotaExhausted)
        assertFalse("非额度耗尽 quotaExhausted=false", err.toFailed().quotaExhausted)
    }

    /** 5xx / 其他 → Unknown（通用）；quotaExhausted=false；文案含 "错误" 前缀。 */
    @Test
    fun status5xx_classifiesUnknown_genericError() {
        val err = classifyError(500, """{"error":{"code":"2000","message":"服务器内部错误"}}""")
        assertTrue("500 应输出通用错误文案（含 错误 前缀）", err.message.contains("错误"))
        val failed = err.toFailed()
        assertFalse("500 不应标记额度耗尽", failed.quotaExhausted)
        assertTrue("500 应输出通用错误（含错误前缀）", failed.message.contains("错误"))
    }

    /** 401 鉴权失败 → 通用分类（非额度、非限流）；quotaExhausted=false。 */
    @Test
    fun status401_genericError_notQuota() {
        val err = classifyError(401, """{"error":{"code":"1001","message":"unauthorized"}}""")
        assertFalse("401 不应标记额度耗尽", err.quotaExhausted)
        assertFalse("401 不应标记额度耗尽", err.toFailed().quotaExhausted)
    }

    // ═══════════════════════════════════════════════════════════════
    // 二、网络 / 超时 异常分类
    // ═══════════════════════════════════════════════════════════════

    /** IOException → Network；quotaExhausted=false；中文提示（网络语义）。 */
    @Test
    fun ioException_classifiesAsNetwork() {
        val err = classifyNetworkError(java.io.IOException("connect timeout"))
        assertTrue("网络错误应给用户可读中文（含网络/连接语义）", err.message.contains("网络") || err.message.contains("连接"))
        val failed = err.toFailed()
        assertFalse("网络错误不应标记额度耗尽", failed.quotaExhausted)
        // 中文可读性 ≠ 堆栈英文原文（异常 message 不得原文暴露为用户文案）
        assertTrue("网络错误应给用户可读中文（含网络/连接语义）", failed.message.contains("网络") || failed.message.contains("连接"))
    }

    /** TimeoutException → Timeout；quotaExhausted=false；中文提示（超时语义）。 */
    @Test
    fun timeoutException_classifiesAsTimeout() {
        val err = classifyNetworkError(java.net.SocketTimeoutException())
        assertTrue("超时应给用户可读中文（含超时语义）", err.message.contains("超时"))
        val failed = err.toFailed()
        assertFalse("超时不应标记额度耗尽", failed.quotaExhausted)
        assertTrue("超时应给用户可读中文（含超时语义）", failed.message.contains("超时"))
    }

    /** 未知异常（非网络/超时子类）→ 兜底分类，不抛异常。 */
    @Test
    fun otherException_fallsBack_doesNotThrow() {
        val err = classifyNetworkError(IllegalStateException("boom"))
        // 兜底：无论如何应返回一个可 toFailed 的 LlmError，不抛异常（I7 fail-safe 引申）
        val failed = err.toFailed()
        assertFalse("兜底错误不标记额度耗尽", failed.quotaExhausted)
        assertTrue("兜底错误仍有用户可读中文文案", failed.message.isNotBlank())
    }

    // ═══════════════════════════════════════════════════════════════
    // 三、无自动重试结构面（负向断言，防新增重试行为）
    // ═══════════════════════════════════════════════════════════════

    /** LlmEvent.Failed 仅含 message + quotaExhausted 两字段 → 语言面无法携带重试计数/预算（无自动重试结构免疫）。 */
    @Test
    fun failedEvent_carriesNoRetryState() {
        // Failed 是 data class（message, quotaExhausted）——若契约未来加重试字段则此断言提醒架构偏离
        val f1 = LlmEvent.Failed("额度耗尽", true)
        val f2 = LlmEvent.Failed("额度耗尽", true)
        assertEquals("Failed 等值（无隐藏重试状态差异）", f1, f2)
        assertEquals("Failed 用户文案字段存在", "额度耗尽", f1.message)
        assertEquals("Failed 额度标记字段存在", true, f1.quotaExhausted)
    }
}
