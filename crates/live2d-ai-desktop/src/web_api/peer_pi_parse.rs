//! `pi --print --mode json` stdout 解析（0.4.5 rc.1）。
//!
//! **本模块是唯一知道 Pi 线格式的地方**——换 Pi 版本 / 换 stdout 形状只改这里。
//! HTTP 契约（错误码 / 状态码）在 [`crate::web_api::peer_routes`]，进程管理在
//! [`crate::web_api::peer_pi`]；解析器本身**纯函数、无 IO**，用录好的 stdout 单测。
//!
//! # 输入（真实 stdout，NDJSON）
//!
//! 一行一个 JSON 对象；形状以本机 `/home/skystar/pi` 源码为准（2026-10-23 核对
//! `packages/coding-agent/src/modes/print-mode.ts` + `modes/json-event.ts`，
//! 安装版 v0.85.1 与源码树 v1.3.0 同形）：
//!
//! 1. 可选首行 session header `{"type":"session",...}` → 忽略；
//! 2. `message_start` / `message_update`（流式增量）/ `agent_end` /
//!    `agent_settled` / `queue_update` / `entry_appended` …… → 全部忽略；
//! 3. 收尾的 `{"type":"message_end","message":{...}}` 且 `message.role == "assistant"`
//!    是**最终权威消息**（print-mode.ts 注释：message_end provides the final
//!    authoritative message）。同一次运行可能有多条 assistant `message_end`
//!    （重试 / 扩展续跑），取**最后一条**。
//!
//! `message.content` 是数组，元素形如：
//! - `{"type":"text","text":"…"}` → **只收这个**；
//! - `{"type":"thinking","thinking":"…"}` → 丢（思考不进正文）；
//! - `{"type":"toolCall",...}` / `{"type":"image",...}` → 丢。
//!
//! # 输出
//!
//! 最后一条 assistant `message_end` 的正文：各 text 块按 `\n` 连接后 `trim()`。
//! 空正文 = 失败（`EmptyText`），不是「回一个空字符串」。
//!
//! # 失败（`peer_routes` 一律映射 502 `peer_failed`）
//!
//! - [`PiParseError::NoAssistantMessage`]：没有任何 assistant `message_end`——
//!   覆盖「stdout 不是约定 JSON」/ 半行 / 只有 header；
//! - [`PiParseError::EmptyText`]：有助手消息但正文为空（只有思考 / 工具调用）；
//! - [`PiParseError::AssistantError`]：`stopReason` 为 `error` / `aborted`。
//!   **不能只看退出码**：`--mode json` 下 provider/model 报错时 `runPrintMode`
//!   仍返回 0（只有 text 模式才置 1），所以必须读 `stopReason`。
//!   `stopReason == "length"` 仍可能有正文——按「有正文就回」处理。
//!
//! 容忍策略：**不要求每一行都是合法 JSON**（崩溃前的半行 / 扩展噪音只跳过），
//! 但**必须**找到一条有正文的 assistant 消息，否则失败。这就是「stdout 不是
//! 约定的 JSON」的判据，也是坏 JSON 测试的红线。

use serde_json::Value;

/// 解析失败的三种形态（映射到固定的 `peer_failed`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiParseError {
    /// 没有找到任何 assistant `message_end`。
    NoAssistantMessage,
    /// 有 assistant 消息，但 text 块为空。
    EmptyText,
    /// `stopReason` 为 `error` / `aborted`（JSON 模式下退出码仍可能是 0）。
    AssistantError,
}

impl PiParseError {
    /// 稳定原因码（只进**日志**，不进 HTTP 响应体；响应体固定 `peer_failed`）。
    pub const fn reason(self) -> &'static str {
        match self {
            Self::NoAssistantMessage => "no_assistant_message",
            Self::EmptyText => "empty_assistant_text",
            Self::AssistantError => "assistant_error_stop",
        }
    }
}

/// 从完整 stdout 里取「Pi 的那一句」。
///
/// 纯函数：不读环境、不读文件、不打印。输入是 stdout 原文（`lossy` 之后即可），
/// 输出是只含助手正文的 `String`。
pub fn parse_pi_session_stdout(stdout: &str) -> Result<String, PiParseError> {
    // 最后一条 assistant message_end 的 message 对象（后面要按块拼正文）。
    let mut last_assistant: Option<Value> = None;
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // 噪声行（截断的半行 JSON / 非 JSON 提示）直接跳过——是否「约定 JSON」
        // 由「有没有可读正文」最终判定。
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if value.get("type").and_then(Value::as_str) != Some("message_end") {
            continue;
        }
        let Some(message) = value.get("message") else {
            continue;
        };
        if message.get("role").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        last_assistant = Some(message.clone());
    }

    let message = last_assistant.ok_or(PiParseError::NoAssistantMessage)?;

    // 错误终止必须判（JSON 模式下退出码不会变非 0）。
    if matches!(
        message.get("stopReason").and_then(Value::as_str),
        Some("error" | "aborted")
    ) {
        return Err(PiParseError::AssistantError);
    }

    let mut parts: Vec<&str> = Vec::new();
    match message.get("content") {
        Some(Value::Array(items)) => {
            for item in items {
                if item.get("type").and_then(Value::as_str) != Some("text") {
                    continue;
                }
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    parts.push(text);
                }
            }
        }
        // 兼容纯字符串 content（当前 pi 的 assistant 是数组；未来形状若变，
        // 这里是最小兜底——但 text 块规则不变）。
        Some(Value::String(text)) => parts.push(text),
        _ => {}
    }
    let text = parts.join("\n");
    let text = text.trim();
    if text.is_empty() {
        return Err(PiParseError::EmptyText);
    }
    Ok(text.to_string())
}

#[cfg(test)]
#[path = "peer_pi_parse_tests.rs"]
mod tests;
