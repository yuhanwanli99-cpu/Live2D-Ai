//! LLM：OpenAI-compatible `/chat/completions`（固定 `stream=true`，SSE 按 chunk 消费）。
//!
//! 事件流语义：
//! - [`LlmEvent::TextDelta`]：正文文本增量；
//! - [`LlmEvent::Done`]：收到 `[DONE]`；对端未发 `[DONE]` 就关流时也兜底补发一次。
//!
//! 2026-09-11 用户裁决：**LLM 不暴露任何工具，只做对话**。请求体因此**不含**
//! `tools` 字段，本模块也不再解析任何 tool-call 流（`ToolCallDelta` 已删除）。
//! 原因（用户裁决原文「llm 不暴露任何工具只做对话」）：带工具 + 空提示词时模型
//! 有时只发工具调用、**一个字正文都不发**——turn 成功结束却说不出话。
//! 回归见 `wire_request_has_no_tools_key`。
//!
//! 解析层不假设任何网络/SSE 切割粒度：字节先进 [`SseDecoder`] 缓冲，
//! 单个 chunk 可能只是半行、一行、多行甚至半个多字节字符。
//!
//! # 行数（AGENTS.md「源码 ≤500 行」）
//!
//! 生产代码（`#[cfg(test)] mod tests` 之前）未超 500 行上限。统计时请按此
//! 口径看，不要把内联测试算进源码行数。

use std::collections::VecDeque;
use std::pin::Pin;

use futures_util::Stream;
use futures_util::stream::try_unfold;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{Error, Result};
use crate::sse::SseDecoder;

/// 消息角色（v0 只需要三种）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// 系统提示。
    System,
    /// 用户输入。
    User,
    /// 助手回复。
    Assistant,
}

impl Role {
    /// 协议字符串（wire 名）。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

/// 一条对话消息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    /// 角色。
    pub role: Role,
    /// 文本内容。
    pub content: String,
}

impl ChatMessage {
    /// 构造一条消息。
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }

    /// `system` 消息的便捷构造。
    pub fn system(content: impl Into<String>) -> Self {
        Self::new(Role::System, content)
    }

    /// `user` 消息的便捷构造。
    pub fn user(content: impl Into<String>) -> Self {
        Self::new(Role::User, content)
    }

    /// `assistant` 消息的便捷构造。
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(Role::Assistant, content)
    }
}

/// LLM 流式输出的统一增量事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmEvent {
    /// 正文文本增量片段。
    TextDelta(String),
    /// 流正常结束。
    Done,
}

// ---------------------------------------------------------------- wire 类型

#[derive(Serialize)]
struct WireMessage<'a> {
    role: &'static str,
    content: &'a str,
}

#[derive(Serialize)]
struct ChatRequestBody<'a> {
    model: &'a str,
    messages: Vec<WireMessage<'a>>,
    stream: bool,
    /// 输出 token 上限。**0 = 不限制**（字段整体省略，沿用服务端默认）。
    ///
    /// 这是「控制回复长度」的**机制**——不靠提示词求模型守规矩，
    /// 而是在协议层给出硬上限。分句由我们按 `。！？` 主动做
    /// （见 `dialogue/sentence.rs`），两者配合才是完整的长度控制。
    #[serde(skip_serializing_if = "is_zero_u32")]
    max_tokens: u32,
}

/// `skip_serializing_if` 的谓词：0 表示「不限制」，省略整个字段。
fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}

fn chat_request_body<'a>(
    model: &'a str,
    messages: &'a [ChatMessage],
    max_tokens: u32,
) -> ChatRequestBody<'a> {
    ChatRequestBody {
        model,
        messages: messages
            .iter()
            .map(|m| WireMessage {
                role: m.role.as_str(),
                content: &m.content,
            })
            .collect(),
        // 固定 stream=true：唯一路径就是 SSE 增量流。
        stream: true,
        max_tokens,
    }
}

#[derive(Deserialize, Default)]
struct ChatChunk {
    #[serde(default)]
    choices: Vec<ChatChunkChoice>,
}

#[derive(Deserialize, Default)]
struct ChatChunkChoice {
    #[serde(default)]
    delta: ChatDelta,
}

#[derive(Deserialize, Default)]
struct ChatDelta {
    #[serde(default)]
    content: Option<String>,
}

/// 把一条 SSE data 映射为零或多个 [`LlmEvent`]（或一个可观测错误）。
fn map_sse_data(data: &str, done: &mut bool, out: &mut Vec<Result<LlmEvent>>) {
    let trimmed = data.trim();
    if trimmed == "[DONE]" {
        *done = true;
        out.push(Ok(LlmEvent::Done));
        return;
    }

    let chunk: ChatChunk = match serde_json::from_str(trimmed) {
        Ok(chunk) => chunk,
        Err(e) => {
            // 带上（截断的）原始 data 片段，便于观测是哪条流坏了。
            let snippet: String = trimmed.chars().take(96).collect();
            out.push(Err(Error::Sse {
                message: format!(
                    "data 不是合法的 chat.completion.chunk JSON: {e}; data={snippet:?}"
                ),
            }));
            return;
        }
    };

    for choice in &chunk.choices {
        // 2026-09-11：只认正文。响应里若出现 `tool_calls`（例如服务端仍带旧
        // 工具定义），serde 忽略未声明字段，这里自然不产出任何事件。
        if let Some(text) = choice.delta.content.as_deref()
            && !text.is_empty()
        {
            out.push(Ok(LlmEvent::TextDelta(text.to_string())));
        }
    }
}

// ---------------------------------------------------------------- 客户端方法

/// 流式状态机（[`try_unfold`] 的状态）。
///
/// 持有原始 `Response` 并用 [`reqwest::Response::chunk`] 逐块拉取：
/// 无需给 `Bytes` 命名类型（reqwest 0.12 不再 re-export），也避免逐块拷贝。
struct ChatStreamState {
    response: reqwest::Response,
    decoder: SseDecoder,
    queue: VecDeque<Result<LlmEvent>>,
    done: bool,
}

impl ChatStreamState {
    async fn advance(mut self) -> Result<Option<(LlmEvent, Self)>> {
        loop {
            // 已解码的事件优先吐出。
            if let Some(event) = self.queue.pop_front() {
                return match event {
                    Ok(event) => Ok(Some((event, self))),
                    Err(e) => Err(e),
                };
            }

            if self.done {
                // [DONE] 之后不再消费底层连接。
                return Ok(None);
            }

            match self.response.chunk().await {
                Ok(Some(chunk)) => {
                    // 关键：任意切割的字节先进缓冲，再按「完整事件」粒度取出。
                    self.decoder.push(&chunk);
                    let mut mapped = Vec::new();
                    while let Some(event) = self.decoder.next_event() {
                        map_sse_data(&event.data, &mut self.done, &mut mapped);
                    }
                    self.queue.extend(mapped);
                }
                Ok(None) => {
                    // 对端关流：兜底 flush 解码器残余，并保证恰好补发一次 Done。
                    let mut mapped = Vec::new();
                    if let Some(event) = self.decoder.finish() {
                        map_sse_data(&event.data, &mut self.done, &mut mapped);
                    }
                    self.queue.extend(mapped);
                    if !self.done && !self.queue.is_empty() {
                        self.done = true;
                        self.queue.push_back(Ok(LlmEvent::Done));
                    } else if !self.done {
                        self.done = true;
                        return Ok(Some((LlmEvent::Done, self)));
                    }
                }
                Err(e) => return Err(Error::Http(e)),
            }
        }
    }
}

impl crate::OpenAiClient {
    /// 发起 `/chat/completions`（`stream=true`）请求并返回按 chunk 解码的事件流。
    ///
    /// 请求阶段即完成 URL 拼接与状态码检查：非 2xx 直接返回
    /// [`Error::Status`]（含截断响应体）；返回的流中每项是
    /// `Ok(LlmEvent)` / `Err(Error)`，错误会终止流。
    pub async fn chat_stream(
        &self,
        messages: &[ChatMessage],
    ) -> Result<Pin<Box<dyn Stream<Item = Result<LlmEvent>> + Send>>> {
        let url: Url = crate::join_endpoint(&self.llm.base_url, "/chat/completions")?;
        let mut request = self.http.post(url).json(&chat_request_body(
            &self.llm.model,
            messages,
            self.llm.max_tokens,
        ));
        if let Some(key) = &self.llm.api_key {
            request = request.bearer_auth(key.expose_secret());
        }

        let response = request.send().await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(Error::status(status, &body));
        }

        Ok(Box::pin(try_unfold(
            ChatStreamState {
                response,
                decoder: SseDecoder::new(),
                queue: VecDeque::new(),
                done: false,
            },
            ChatStreamState::advance,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LlmConfig;
    use crate::secret::ApiSecret;

    fn map_all(datas: &[&str]) -> (Vec<LlmEvent>, Vec<String>, bool) {
        let mut events = Vec::new();
        let mut errors = Vec::new();
        let mut done = false;
        for data in datas {
            let mut out = Vec::new();
            map_sse_data(data, &mut done, &mut out);
            for item in out {
                match item {
                    Ok(ev) => events.push(ev),
                    Err(e) => errors.push(e.to_string()),
                }
            }
        }
        (events, errors, done)
    }

    #[test]
    fn text_and_done_map_to_events() {
        let (events, errors, done) = map_all(&[
            "{\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}",
            "{\"choices\":[{\"delta\":{\"content\":\"你\"}}]}",
            "[DONE]",
        ]);
        assert!(errors.is_empty());
        assert_eq!(
            events,
            vec![LlmEvent::TextDelta("你".to_string()), LlmEvent::Done]
        );
        assert!(done);
    }

    #[test]
    fn malformed_json_yields_observable_error() {
        let (events, errors, _) = map_all(&["{not json"]);
        assert!(events.is_empty());
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("SSE"));
        assert!(!errors[0].is_empty());
    }

    #[test]
    fn empty_content_and_tool_call_only_chunks_are_skipped() {
        // 空 content、空 choices、以及**只带 tool_calls** 的 chunk 都不产生事件。
        // 最后一条即用户裁决的直接后果：LLM 不再暴露工具，tool-call 流被忽略
        // ——过去它会变成 `ToolCallDelta`，是「模型只调工具、一个字都不说」
        // 那条空转 turn 的入口。
        let (events, errors, _) = map_all(&[
            "{\"choices\":[{\"delta\":{\"content\":\"\"}}]}",
            "{\"choices\":[]}",
            "{\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"type\":\"function\"}]}}]}",
        ]);
        assert!(errors.is_empty());
        assert!(events.is_empty());
    }

    #[test]
    fn wire_request_shape_is_openai_compatible() {
        let messages = [ChatMessage::system("sys"), ChatMessage::user("hi")];
        let body = chat_request_body("qwen2.5:7b", &messages, 0);
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["model"], "qwen2.5:7b");
        assert_eq!(json["stream"], true);
        assert_eq!(json["messages"][0]["role"], "system");
        assert_eq!(json["messages"][1]["role"], "user");
    }

    /// **本任务最重要的断言（2026-09-11 用户裁决）**：请求体**不含** `tools` 键。
    ///
    /// 用户裁决原文：「llm 不暴露任何工具只做对话」。工具一旦出现，模型有时只发
    /// 工具调用、正文一个字都不发——turn 成功却什么都没说。所以这里不是断言
    /// 「tools 为空数组」，而是断言**键根本不存在**：空数组仍会在 wire 上宣告
    /// 「本模型支持工具」，语义完全不同。
    #[test]
    fn wire_request_has_no_tools_key() {
        let messages = [
            ChatMessage::system("sys"),
            ChatMessage::user("hi"),
            ChatMessage::assistant("yo"),
        ];
        // 覆盖有/无 max_tokens 两条序列化分支，tools 在两种情况下都不得出现。
        for max_tokens in [0u32, 512] {
            let json = serde_json::to_value(chat_request_body("m", &messages, max_tokens)).unwrap();
            assert!(
                json.get("tools").is_none(),
                "LLM 请求体不得含 tools 键（max_tokens={max_tokens}），got: {json}"
            );
            // 顺带锁住「没有别的工具别名」：function/tool_choice 也不该出现。
            assert!(json.get("tool_choice").is_none(), "got: {json}");
            assert!(json.get("functions").is_none(), "got: {json}");
        }
    }

    /// 输出上限的线上形态：`0`（不限制）→ 字段整体省略；非 0 → 原样出现。
    ///
    /// 「省略」很重要：部分 OpenAI 兼容服务把 `max_tokens: 0` 当作
    /// 「最多输出 0 个 token」（即拒绝生成），语义与「不限制」相反。
    #[test]
    fn wire_request_omits_max_tokens_only_when_unlimited() {
        let messages = [ChatMessage::user("hi")];

        let unlimited = chat_request_body("m", &messages, 0);
        let json = serde_json::to_value(&unlimited).unwrap();
        assert!(
            json.get("max_tokens").is_none(),
            "0 = 不限制 → 必须省略字段，got: {json}"
        );

        let capped = chat_request_body("m", &messages, 512);
        let json = serde_json::to_value(&capped).unwrap();
        assert_eq!(json["max_tokens"], 512);
    }

    // ApiSecret 在此模块的使用点：仅作为 bearer_auth 输入（见集成测试的 Authorization 断言）。
    #[test]
    fn api_key_only_flows_through_expose() {
        let key = ApiSecret::new("sk-x");
        assert_eq!(key.expose_secret(), "sk-x");
        let _ = LlmConfig::new("http://a", "m").with_api_key(key);
    }
}
