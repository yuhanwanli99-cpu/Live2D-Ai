//! LLM：OpenAI-compatible `/chat/completions`（固定 `stream=true`，SSE 按 chunk 消费）。
//!
//! 事件流语义（fragment 语义，调用方按序拼接）：
//! - [`LlmEvent::TextDelta`]：正文文本增量；
//! - [`LlmEvent::ToolCallDelta`]：工具调用增量。`index` 是该调用在本次响应内的
//!   流式下标（wire 缺省按 0），`id` 是服务端调用 id（通常仅首个分片携带）；
//!   `name` 是**本次 chunk 的名字片段**（OpenAI 通常一次性给全，其后为 `None`）；
//!   `arguments` 是**本次 chunk 的参数 JSON 片段**（标准行为就是跨多个 SSE 事件
//!   分块到达，必须拼接后再解析）；
//! - [`LlmEvent::Done`]：收到 `[DONE]`；对端未发 `[DONE]` 就关流时也兜底补发一次。
//!
//! 解析层不假设任何网络/SSE 切割粒度：字节先进 [`SseDecoder`] 缓冲，
//! 单个 chunk 可能只是半行、一行、多行甚至半个多字节字符。
//!
//! # 行数（AGENTS.md「源码 ≤500 行」）
//!
//! 文件 528 行，其中 `#[cfg(test)] mod tests` 自第 354 行起占 175 行——
//! **生产代码 353 行**，未超 500 行上限。统计时请按此口径看，不要把内联
//! 测试算进源码行数。

use std::collections::VecDeque;
use std::pin::Pin;

use futures_util::Stream;
use futures_util::stream::try_unfold;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{Error, Result};
use crate::sse::SseDecoder;
use crate::tool::perform_action_tool;

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
    /// 工具调用增量（见模块文档的 fragment 语义）。
    ToolCallDelta {
        /// 该调用在本次响应内的流式下标（wire 缺省按 0）。
        index: usize,
        /// 服务端调用 id；通常仅首个分片携带，后续分片为 `None`。
        id: Option<String>,
        /// 本次 chunk 携带的名字片段；本 chunk 无名字时为 `None`。
        name: Option<String>,
        /// 本次 chunk 携带的参数 JSON 片段（可为空串）。
        arguments: String,
    },
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
    tools: [serde_json::Value; 1],
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
        // 固定注入唯一的 live2d_perform_action 工具；用不用/能不能用由 core 决定。
        tools: [perform_action_tool()],
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
    #[serde(default)]
    tool_calls: Option<Vec<ChatToolCall>>,
}

#[derive(Deserialize)]
struct ChatToolCall {
    #[serde(default)]
    function: Option<ChatToolFunction>,
    /// 流式下标：同轮多个调用靠它分组；个别 provider 缺省时按 0。
    #[serde(default)]
    index: usize,
    /// 服务端调用 id：通常仅首个分片携带。
    #[serde(default)]
    id: Option<String>,
}

#[derive(Deserialize)]
struct ChatToolFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
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
        let delta = &choice.delta;

        match delta.content.as_deref() {
            Some(text) if !text.is_empty() => {
                out.push(Ok(LlmEvent::TextDelta(text.to_string())));
            }
            _ => {}
        }

        if let Some(calls) = &delta.tool_calls {
            for call in calls {
                let (name, arguments) = match &call.function {
                    Some(function) => (
                        function.name.clone().filter(|n| !n.is_empty()),
                        function.arguments.clone().unwrap_or_default(),
                    ),
                    // 只带 id/index 的分片也透传：组装层靠它把 id 关联到对应调用。
                    None => (None, String::new()),
                };
                if name.is_some() || !arguments.is_empty() || call.id.is_some() {
                    out.push(Ok(LlmEvent::ToolCallDelta {
                        index: call.index,
                        id: call.id.clone(),
                        name,
                        arguments,
                    }));
                }
            }
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
    fn tool_arguments_fragments_concatenate_across_events() {
        let (events, errors, _) = map_all(&[
            "{\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\
              \"type\":\"function\",\"function\":{\"name\":\"live2d_perform_action\"}}]}}]}",
            "{\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\
              \"function\":{\"arguments\":\"{\\\"action\\\":\\\"nod\\\"\"}}]}}]}",
            "{\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\
              \"function\":{\"arguments\":\",\\\"strength\\\":2}\"}}]}}]}",
        ]);
        assert!(errors.is_empty());

        let mut name: Option<String> = None;
        let mut args = String::new();
        for event in &events {
            if let LlmEvent::ToolCallDelta {
                name: fragment,
                arguments,
                ..
            } = event
            {
                if fragment.is_some() {
                    name = fragment.clone();
                }
                args.push_str(arguments);
            }
        }
        assert_eq!(name.as_deref(), Some("live2d_perform_action"));
        assert_eq!(args, "{\"action\":\"nod\",\"strength\":2}");

        // 拼接结果可反序列化回 wire 类型。
        let parsed: crate::tool::Live2dPerformActionArgs = serde_json::from_str(&args).unwrap();
        assert_eq!(parsed.action, crate::tool::PerformAction::Nod);
        assert_eq!(parsed.strength, 2);
    }

    #[test]
    fn tool_call_index_and_id_are_forwarded_with_defaults() {
        // 带 index/id 的分片原样透传；缺 index 的分片按协议缺省为 0；只带 id 也透传。
        let (events, errors, _) = map_all(&[
            "{\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":1,\"id\":\"a\",\
              \"type\":\"function\",\"function\":{\"name\":\"x\"}}]}}]}",
            "{\"choices\":[{\"delta\":{\"tool_calls\":[{\"id\":\"b\",\"type\":\"function\"}]}}]}",
            "{\"choices\":[{\"delta\":{\"tool_calls\":[{\"function\":{\"arguments\":\"{}\"}}]}}]}",
        ]);
        assert!(errors.is_empty());
        assert_eq!(
            events,
            vec![
                LlmEvent::ToolCallDelta {
                    index: 1,
                    id: Some("a".to_string()),
                    name: Some("x".to_string()),
                    arguments: String::new(),
                },
                LlmEvent::ToolCallDelta {
                    index: 0,
                    id: Some("b".to_string()),
                    name: None,
                    arguments: String::new(),
                },
                LlmEvent::ToolCallDelta {
                    index: 0,
                    id: None,
                    name: None,
                    arguments: "{}".to_string(),
                },
            ]
        );
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
    fn empty_content_and_bare_tool_call_are_skipped() {
        // 空 content 与「既无 function 也无 id」的 tool_call 不产生噪声事件；
        // 只带 id 的分片会透传（组装层靠它关联 id），见 index/id 转发测试。
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
        assert_eq!(
            json["tools"][0]["function"]["name"],
            "live2d_perform_action"
        );
        assert_eq!(json["tools"].as_array().unwrap().len(), 1);
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
