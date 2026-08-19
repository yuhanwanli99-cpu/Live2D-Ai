package com.live2d.ai.android.core

import java.io.IOException
import java.io.InterruptedIOException
import java.net.SocketTimeoutException

/**
 * LLM 错误分类类别。
 *
 * 契约 §2.2（refactor-0814）：无自动重试（结构上不携带重试状态），
 * 429/额度错误分类 + 中文文案。
 */
enum class LlmErrorKind {
    /** HTTP 429：限流，可稍后重试。 */
    RateLimited,

    /** HTTP 403 / 400（额度关键词命中）：共享 Key 额度耗尽，引导用户换自己的 Key。 */
    QuotaExhausted,

    /** 其他 HTTP 错误 / 未识别异常。 */
    Unknown,

    /** 网络连接失败（IOException）。 */
    Network,

    /** 请求超时。 */
    Timeout,
}

/**
 * LLM 错误（仅 [message] + [quotaExhausted] 两个字段）。
 *
 * 结构约束：**无重试状态字段**——无自动重试是结构要求，重试语义归上层
 * （LoopCoordinator / UI）决策，本类型不承载重试计数/状态。
 *
 * 注：事件类型 [LlmEvent] 已由 2C-1 在 core/LlmLink.kt 锁定（sealed：
 * Delta / Completed / Failed），本文件只承载错误分类，不重复声明 LlmEvent。
 */
data class LlmError(
    val message: String,
    val quotaExhausted: Boolean,
)

/** 限流文案（必含子串：请求过于频繁 / 稍后重试）。 */
private const val MESSAGE_RATE_LIMITED = "[提示] 请求过于频繁，请稍后重试（免费模型限 1 并发）"

/** 额度耗尽文案（必含子串：免费额度 / 设置 / API Key）。 */
private const val MESSAGE_QUOTA_EXHAUSTED =
    "当前 API 免费额度已用尽或服务繁忙。" +
        "可前往「设置 → API 设置」填写你自己的 API Key 继续使用。"

/** 网络失败文案（必含子串：网络 / 错误 / 稍后重试）。 */
private const val MESSAGE_NETWORK = "[错误] 网络连接失败，请检查网络后重试"

/** 超时文案（必含子串：超时 / 网络 / 错误 / 稍后重试）。 */
private const val MESSAGE_TIMEOUT = "[错误] 请求超时，请检查网络后重试"

/**
 * 额度耗尽/欠费类错误的识别关键词（服务端错误响应 body 中可能出现的标记）。
 * 与 ChatService 的 QUOTA_KEYWORDS 保持同一词汇表。
 */
private val QUOTA_KEYWORDS = listOf(
    "quota", "balance", "insufficient", "exhausted", "overdue", "frozen",
    "余额", "额度", "欠费", "冻结", "充值", "已用尽",
)

/**
 * 按 HTTP 状态码 + 响应体分类错误。
 *
 * | 输入 | 分类 | quotaExhausted |
 * |---|---|---|
 * | 429 | [LlmErrorKind.RateLimited] | false |
 * | 403，或 400 且 body 命中额度关键词 | [LlmErrorKind.QuotaExhausted] | true |
 * | 其余（含 500） | [LlmErrorKind.Unknown] | false |
 */
fun classifyError(code: Int, body: String): LlmError {
    val kind = when {
        code == 429 -> LlmErrorKind.RateLimited
        code == 403 || (code == 400 && QUOTA_KEYWORDS.any { body.contains(it, ignoreCase = true) }) ->
            LlmErrorKind.QuotaExhausted
        else -> LlmErrorKind.Unknown
    }
    return errorFor(kind, code, body)
}

/**
 * 按网络异常分类错误。
 *
 * | 输入 | 分类 | quotaExhausted |
 * |---|---|---|
 * | [SocketTimeoutException] / [InterruptedIOException]（超时） | [LlmErrorKind.Timeout] | false |
 * | 其他 [IOException] | [LlmErrorKind.Network] | false |
 * | 其余异常 | [LlmErrorKind.Unknown] | false |
 */
fun classifyNetworkError(cause: Exception): LlmError = when (cause) {
    is SocketTimeoutException, is InterruptedIOException -> errorFor(LlmErrorKind.Timeout)
    is IOException -> errorFor(LlmErrorKind.Network)
    else -> LlmError("[错误] 请求失败：${cause.message ?: "未知错误"}", quotaExhausted = false)
}

private fun errorFor(kind: LlmErrorKind, code: Int = 0, body: String = ""): LlmError = when (kind) {
    LlmErrorKind.RateLimited -> LlmError(MESSAGE_RATE_LIMITED, quotaExhausted = false)
    LlmErrorKind.QuotaExhausted -> LlmError(MESSAGE_QUOTA_EXHAUSTED, quotaExhausted = true)
    LlmErrorKind.Unknown ->
        LlmError("[错误] API 请求失败：$code - ${body.ifBlank { "未知错误" }}", quotaExhausted = false)
    LlmErrorKind.Network -> LlmError(MESSAGE_NETWORK, quotaExhausted = false)
    LlmErrorKind.Timeout -> LlmError(MESSAGE_TIMEOUT, quotaExhausted = false)
}
