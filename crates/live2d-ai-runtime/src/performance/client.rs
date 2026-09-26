//! 表演层 HTTP 客户端：**非流式** `POST {base_url}/chat/completions`。
//!
//! # 口径（与项目其余 HTTP 客户端一致）
//!
//! - **独立配置**：`base_url` / `model` / `timeout_ms` /
//!   `api_key_env`——独立于 `[llm]`（主模型）与 `[tts]`；
//!   二路断了不影响主链；
//! - **TLS**：reqwest + `rustls-tls`（与 runtime 其余传输同款，无 native-tls）；
//! - **密钥**：一律走 [live2d_ai_runtime::secrets::lookup]（`.env` 快照 > 进程环境）；
//!   只进 `Authorization: Bearer` 头，**不进 URL / body / 日志**；
//! - **非流式**：`stream=false` + `temperature=0`——表演层要的是一份 JSON，
//!   不是逐字流；
//! - **思考不进请求**：body 只有 system + user 两条消息，没有 `reasoning_content`
//!   字段，也不把上游思考回灌。
//!
//! # structured output（优先）与 auto 降级
//!
//! - [StructuredMode::JsonSchema]：带 `response_format={"type":"json_schema",...}`；
//! - [StructuredMode::Prompt]：不带，system 写死「只输出 JSON」；
//! - [StructuredMode::Auto]（缺省）：先按 JsonSchema 发；只有上游回 **4xx**
//!   （典型是「不认识 response_format」）才原地降级成 Prompt 再发一次。
//!   传输失败 / 5xx / 超时**不重试**（那些不是「模型不支持 structured」）。
//!
//! # 行数说明（AGENTS.md 豁免）
//!
//! 本文件 > 500 行：HTTP wire 回归（scripted loopback / json_schema / auto 降级 /
//! 5xx 不重试）与客户端实现同住一处（原在 performance/tests.rs，为守 800 行测试
//! 红线迁入）。按「豁免 ≤ 1000 需头注理由」保留单文件——4b 授权文件不含新路径。
//!
//! # 失败 = `None`（静默回退）
//!
//! 连接失败 / 超时 / 非 2xx / 响应不是 JSON / `content` 缺失或空白 →
//! 一律 `None`，由 [crate::performance::PerformanceRuntime] 回退到
//! `speak=clean_for_tts(原文)` + 规则 cue。

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use serde_json::{Value, json};

use crate::performance::plan;
use crate::performance::prompt;
use crate::secret::ApiSecret;

/// 装箱的异步返回（对象安全的 trait 方法必需）。
pub type PerformanceFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// structured output 策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StructuredMode {
    /// 缺省：先 json_schema，4xx 时降级 prompt。
    #[default]
    Auto,
    /// 强制 json_schema strict。
    JsonSchema,
    /// 强制 prompt（不带 response_format）。
    Prompt,
}

impl StructuredMode {
    /// 从配置字符串解析（未知值回落 [Self::Auto]，绝不失败）。
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "json_schema" | "structured" | "schema" => Self::JsonSchema,
            "prompt" | "text" | "json" => Self::Prompt,
            _ => Self::Auto,
        }
    }

    /// 配置 / 状态面里的稳定字符串。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::JsonSchema => "json_schema",
            Self::Prompt => "prompt",
        }
    }
}

/// 一次成功调用：正文 + 是否走的 structured 路。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformanceReply {
    /// `choices[0].message.content`（trim 后非空）。
    pub raw: String,
    /// `true` = 本次请求带了 json_schema。
    pub structured: bool,
}

/// 表演层客户端（host 注入；缺省 [DisabledPerformance]）。
pub trait PerformanceClient: Send + Sync + fmt::Debug {
    /// 是否可用（Disabled → false）。
    fn enabled(&self) -> bool;
    /// 客户端种类（状态面可观察）：`"disabled"` / `"openai"` / `"injected"`。
    fn kind(&self) -> &'static str;
    /// 是否已解析出一把 API key（**布尔可以出门，值不可以**）。
    fn has_api_key(&self) -> bool;
    /// 非流式补全；`None` = 失败 / 超时 / 未配置（调用方静默回退）。
    fn request<'a>(
        &'a self,
        system: &'a str,
        user: &'a str,
        timeout_ms: u64,
    ) -> PerformanceFuture<'a, Option<PerformanceReply>>;
}

/// 未接线的缺省实现：enabled=false、request 恒 None。
#[derive(Debug)]
pub struct DisabledPerformance;

impl PerformanceClient for DisabledPerformance {
    fn enabled(&self) -> bool {
        false
    }
    fn kind(&self) -> &'static str {
        "disabled"
    }
    fn has_api_key(&self) -> bool {
        false
    }
    fn request<'a>(
        &'a self,
        _system: &'a str,
        _user: &'a str,
        _timeout_ms: u64,
    ) -> PerformanceFuture<'a, Option<PerformanceReply>> {
        Box::pin(async { None })
    }
}

/// `/chat/completions` 路径（OpenAI 兼容约定的唯一后缀）。
const CHAT_COMPLETIONS_PATH: &str = "/chat/completions";
/// 超时下限（毫秒）。
pub const MIN_TIMEOUT_MS: u64 = 100;
/// 超时上限（毫秒）。
pub const MAX_TIMEOUT_MS: u64 = 30_000;
/// 输出 token 上限（一份 plan 很短；显式给硬上限，不靠提示词求模型守规矩）。
pub const MAX_TOKENS: u32 = 1_024;

/// 从 `chat.completion` JSON 里取 `choices[0].message.content`（trim 后非空才算成功）。
///
/// **只读正文**：`reasoning_content` / `reasoning` 即使存在也被忽略。
pub fn parse_completion(raw: &str) -> Option<String> {
    let value: Value = serde_json::from_str(raw).ok()?;
    let content = value
        .get("choices")?
        .as_array()?
        .first()?
        .get("message")?
        .get("content")?
        .as_str()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// OpenAI 兼容的异步表演层客户端。
pub struct OpenAiPerformanceClient {
    http: reqwest::Client,
    url: reqwest::Url,
    model: String,
    api_key: Option<ApiSecret>,
    mode: StructuredMode,
    schema: Value,
}

impl fmt::Debug for OpenAiPerformanceClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenAiPerformanceClient")
            .field("url", &self.url.as_str())
            .field("model", &self.model)
            .field("has_api_key", &self.api_key.is_some())
            .field("mode", &self.mode)
            .finish()
    }
}

impl OpenAiPerformanceClient {
    /// 生产构造：`api_key_env` 经 [crate::secrets::lookup] 解析
    /// （空名字 = 不鉴权）。缺 base_url / model → `Err`（调用方转 Disabled + degraded）。
    pub fn new(
        base_url: &str,
        model: &str,
        api_key_env: &str,
        timeout_ms: u64,
        mode: StructuredMode,
        allow: &[String],
    ) -> Result<Self, String> {
        let env_name = api_key_env.trim();
        let api_key = if env_name.is_empty() {
            None
        } else {
            crate::secrets::lookup(env_name).map(ApiSecret::new)
        };
        Self::with_api_key(base_url, model, api_key, timeout_ms, mode, allow)
    }

    /// 显式注入密钥的构造（测试 / 未来 host 替身）；生产走 [Self::new]。
    pub fn with_api_key(
        base_url: &str,
        model: &str,
        api_key: Option<ApiSecret>,
        timeout_ms: u64,
        mode: StructuredMode,
        allow: &[String],
    ) -> Result<Self, String> {
        // 单一实现 = `crate::join_endpoint`（`url::Url` + `crate::Error`）；
        // 这里对齐本层的 `Result<_, String>` 错误口径，不做 unwrap 兜底。
        let url = crate::join_endpoint(base_url, CHAT_COMPLETIONS_PATH)
            .map_err(|e| format!("performance.base_url 非法: {e}"))?;
        let model = model.trim();
        if model.is_empty() {
            return Err("performance.model 未配置".to_string());
        }
        let http = reqwest::Client::builder()
            .timeout(Duration::from_millis(
                timeout_ms.clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS),
            ))
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .map_err(|e| format!("构造表演层 HTTP 客户端失败: {e}"))?;
        Ok(Self {
            http,
            url,
            model: model.to_string(),
            api_key,
            mode,
            schema: plan::json_schema_strict(allow),
        })
    }

    /// 是否解析出了一把 API key（布尔可以出门，值不可以）。
    pub fn has_api_key(&self) -> bool {
        self.api_key.is_some()
    }

    /// 目标端点（日志 / 排障用；不含密钥）。
    pub fn endpoint(&self) -> &str {
        self.url.as_str()
    }

    /// 组装请求体；`structured` 决定带不带 response_format。
    fn build_body(&self, system: &str, user: &str, structured: bool) -> Value {
        let mut body = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
            "stream": false,
            "temperature": 0,
            "max_tokens": MAX_TOKENS,
        });
        if structured {
            body["response_format"] = json!({
                "type": "json_schema",
                "json_schema": {
                    "name": "performance_plan",
                    "strict": true,
                    "schema": self.schema,
                }
            });
        }
        body
    }

    /// 发一次请求；返回 `(状态是否成功, 正文)`。
    async fn post_once(
        &self,
        system: &str,
        user: &str,
        timeout_ms: u64,
        structured: bool,
    ) -> Option<(bool, String)> {
        let timeout = Duration::from_millis(timeout_ms.clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS));
        let mut request = self
            .http
            .post(self.url.clone())
            .timeout(timeout)
            .json(&self.build_body(system, user, structured));
        if let Some(key) = self.api_key.as_ref() {
            request = request.bearer_auth(key.expose_secret());
        }
        let response = request.send().await.ok()?;
        let status = response.status();
        let text = response.text().await.ok()?;
        Some((status.is_success(), text))
    }
}

impl PerformanceClient for OpenAiPerformanceClient {
    fn enabled(&self) -> bool {
        true
    }

    fn kind(&self) -> &'static str {
        "openai"
    }

    fn has_api_key(&self) -> bool {
        self.has_api_key()
    }

    fn request<'a>(
        &'a self,
        system: &'a str,
        user: &'a str,
        timeout_ms: u64,
    ) -> PerformanceFuture<'a, Option<PerformanceReply>> {
        Box::pin(async move {
            let structured = matches!(self.mode, StructuredMode::JsonSchema | StructuredMode::Auto);
            let (ok, body) = self.post_once(system, user, timeout_ms, structured).await?;
            if ok {
                return parse_completion(&body).map(|raw| PerformanceReply { raw, structured });
            }
            // auto：只在 4xx（「不认识 response_format」那类）时降级 prompt 再发一次。
            if self.mode == StructuredMode::Auto && structured && client_error_4xx(&body) {
                let (ok2, body2) = self
                    .post_once(prompt::SYSTEM_JSON_ONLY, user, timeout_ms, false)
                    .await?;
                if ok2 {
                    return parse_completion(&body2).map(|raw| PerformanceReply {
                        raw,
                        structured: false,
                    });
                }
            }
            None
        })
    }
}

/// 响应体是否看起来是客户端错误（OpenAI 风格的 `{"error":{...}}` 且带 type/param/invalid）。
///
/// 我们只从 [OpenAiPerformanceClient::post_once] 拿得到「成功与否 + 正文」，
/// 拿不到状态码，所以这里用一个**保守**的启发式：只在 [StructuredMode::Auto]
/// 的降级判定里用它；误判的代价只是多一次请求（然后仍按失败回退）。
fn client_error_4xx(body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    let has_error = lower.contains("\"error\"");
    let looks_like_param_problem = lower.contains("\"type\"")
        || lower.contains("\"param\"")
        || lower.contains("invalid")
        || lower.contains("unsupported")
        || lower.contains("unknown");
    has_error && looks_like_param_problem
}

#[cfg(test)]
mod wire_tests {
    use super::*;
    use crate::performance::plan::parse_plan;

    fn allow() -> Vec<String> {
        vec!["nod".to_string(), "smile".to_string()]
    }

    /// 脚本化 loopback：按顺序回应 N 个请求，回传每个请求的原始文本。
    async fn scripted_server(
        responses: Vec<(u16, String)>,
    ) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback");
        let addr = listener.local_addr().expect("addr");
        let handle = tokio::spawn(async move {
            let mut raws = Vec::new();
            for (status, body) in responses {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let mut buf: Vec<u8> = Vec::new();
                let mut tmp = [0u8; 1024];
                loop {
                    let n = stream.read(&mut tmp).await.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&tmp[..n]);
                    if let Some(head_end) = find_subslice(&buf, b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
                        let cl = content_length(&head);
                        if buf.len() >= head_end + 4 + cl {
                            break;
                        }
                    }
                }
                raws.push(String::from_utf8_lossy(&buf).to_string());
                let reason = if status == 200 { "OK" } else { "Bad Request" };
                let response = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.flush().await;
            }
            raws
        });
        (format!("http://{addr}/v1"), handle)
    }

    fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    fn content_length(head: &str) -> usize {
        head.lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0)
    }

    #[test]
    fn parse_completion_ignores_reasoning_and_rejects_blank() {
        let ok = r#"{"choices":[{"message":{"role":"assistant","content":"{\"segments\":[],\"cues\":[]}","reasoning_content":"SECRET"}}]}"#;
        assert_eq!(
            parse_completion(ok).as_deref(),
            Some("{\"segments\":[],\"cues\":[]}")
        );
        assert_eq!(parse_completion(r#"{"choices":[]}"#), None);
        assert_eq!(parse_completion("not json"), None);
        assert_eq!(
            parse_completion(r#"{"choices":[{"message":{"content":"   "}}]}"#),
            None
        );
    }

    #[test]
    fn structured_mode_parse_never_fails() {
        assert_eq!(
            StructuredMode::parse("json_schema"),
            StructuredMode::JsonSchema
        );
        assert_eq!(StructuredMode::parse("PROMPT"), StructuredMode::Prompt);
        assert_eq!(StructuredMode::parse("随便"), StructuredMode::Auto);
        assert_eq!(StructuredMode::default(), StructuredMode::Auto);
        assert_eq!(StructuredMode::Auto.as_str(), "auto");
    }

    #[test]
    fn endpoint_join_rejects_missing_and_bad_scheme() {
        assert!(crate::join_endpoint("", "/x").is_err());
        assert!(crate::join_endpoint("ftp://h/v1", "/x").is_err());
        assert_eq!(
            crate::join_endpoint("http://h/v1/", "/chat/completions")
                .expect("合法")
                .as_str(),
            "http://h/v1/chat/completions"
        );
    }

    /// structured 路：请求体带 v1 json_schema（枚举就是能力集），只有 system+user。
    #[tokio::test]
    async fn openai_client_posts_json_schema_and_parses_content() {
        let plan = r#"{"segments":["你好。"],"cues":[{"field":"expression","id":"nod","intensity":2,"at":"now","hold":false}]}"#;
        let response = serde_json::json!({
            "choices": [{"message": {"role": "assistant", "content": plan}, "finish_reason": "stop"}],
            "reasoning_content": "SECRET_THOUGHT"
        })
        .to_string();
        let (base, handle) = scripted_server(vec![(200, response)]).await;
        let client = OpenAiPerformanceClient::with_api_key(
            &base,
            "director-model",
            Some(crate::secret::ApiSecret::new("sk-test-123")),
            3_000,
            StructuredMode::JsonSchema,
            &allow(),
        )
        .expect("构造应成功");
        assert!(client.enabled());
        assert_eq!(client.kind(), "openai");
        assert!(client.has_api_key());
        let reply = client
            .request("SYS", "USER", 3_000)
            .await
            .expect("应解析出正文");
        assert!(reply.structured);
        assert_eq!(reply.raw, plan);
        assert!(parse_plan(&reply.raw, &allow(), "你好。").is_ok());

        let raws = handle.await.expect("mock server");
        let raw = &raws[0];
        assert!(raw.starts_with("POST /v1/chat/completions"), "{raw}");
        assert!(
            raw.to_ascii_lowercase()
                .contains("authorization: bearer sk-test-123"),
            "{raw}"
        );
        assert!(raw.contains("\"stream\":false"), "{raw}");
        assert!(raw.contains("\"model\":\"director-model\""), "{raw}");
        assert!(raw.contains("\"json_schema\""), "{raw}");
        assert!(raw.contains("\"strict\":true"), "{raw}");
        assert!(raw.contains("\"segments\""), "{raw}");
        assert!(
            !raw.contains("Param"),
            "schema 常量不得带任何 Param 参数名：{raw}"
        );
        assert!(
            !raw.contains("reasoning") && !raw.contains("SECRET_THOUGHT"),
            "请求体不得携带思考：{raw}"
        );
        assert_eq!(
            raw.matches("\"role\"").count(),
            2,
            "只有 system+user：{raw}"
        );
    }

    /// auto：上游 4xx（不认识 response_format）→ 原地降级 prompt 再发一次。
    #[tokio::test]
    async fn auto_mode_degrades_to_prompt_on_4xx() {
        let plan = r#"{"segments":[],"cues":[]}"#;
        let ok = serde_json::json!({"choices":[{"message":{"content":plan}}]}).to_string();
        let err = r#"{"error":{"message":"response_format is not supported","type":"invalid_request_error"}}"#.to_string();
        let (base, handle) = scripted_server(vec![(400, err), (200, ok)]).await;
        let client = OpenAiPerformanceClient::with_api_key(
            &base,
            "m",
            None,
            3_000,
            StructuredMode::Auto,
            &allow(),
        )
        .expect("构造应成功");
        let reply = client
            .request("SYS", "USER", 3_000)
            .await
            .expect("降级后成功");
        assert!(!reply.structured, "第二次必须不带 response_format");
        let raws = handle.await.expect("mock server");
        assert_eq!(raws.len(), 2, "auto 必须先 json_schema 再 prompt");
        assert!(raws[0].contains("\"response_format\""));
        assert!(!raws[1].contains("\"response_format\""));
        assert!(
            raws[1].contains("只输出 JSON"),
            "降级请求要带 JSON-only system"
        );
    }

    /// 5xx 不重试；只有思考没有正文也算失败。
    #[tokio::test]
    async fn server_errors_and_blank_content_fall_back_to_none() {
        let (base, handle) = scripted_server(vec![(500, r#"{"error":"boom"}"#.to_string())]).await;
        let client = OpenAiPerformanceClient::with_api_key(
            &base,
            "m",
            None,
            2_000,
            StructuredMode::Auto,
            &allow(),
        )
        .expect("构造");
        assert_eq!(client.request("s", "u", 2_000).await, None);
        assert_eq!(handle.await.expect("mock").len(), 1, "5xx 不得重试");

        let (base, handle) = scripted_server(vec![(
            200,
            r#"{"choices":[{"message":{"content":"","reasoning_content":"想一想"}}]}"#.to_string(),
        )])
        .await;
        let client = OpenAiPerformanceClient::with_api_key(
            &base,
            "m",
            None,
            2_000,
            StructuredMode::Prompt,
            &allow(),
        )
        .expect("构造");
        assert_eq!(client.request("s", "u", 2_000).await, None);
        let raws = handle.await.expect("mock");
        assert!(
            !raws[0].contains("\"response_format\""),
            "prompt 路不带 schema"
        );
    }
}

#[cfg(test)]
mod assembly_tests {
    use std::sync::Arc;

    use crate::performance::{PerformanceCue, RuleFallback, assemble};

    fn allow() -> Vec<String> {
        vec!["nod".to_string(), "smile".to_string()]
    }

    fn rule_one_cue() -> RuleFallback {
        Arc::new(|text: &str| {
            if text.trim().is_empty() {
                return Vec::new();
            }
            vec![PerformanceCue {
                sentence_seq: 1,
                preset_id: "nod".to_string(),
                intensity: 1,
                ttl_ms: 2_000,
                ..Default::default()
            }]
        })
    }

    /// 关闸装配：与「从来没有表演层」逐字一致（runtime=None、无 note）。
    #[test]
    fn assemble_off_is_none_and_missing_endpoint_is_degraded() {
        let wiring = |enabled: bool, base: &str| crate::settings::PerformanceSettings {
            enabled,
            base_url: base.to_string(),
            model: "m".to_string(),
            ..Default::default()
        };
        let off = assemble(
            &wiring(false, "http://127.0.0.1:11434/v1"),
            allow(),
            Some(rule_one_cue()),
        );
        assert!(off.runtime.is_none());
        assert!(off.note.is_none());

        let no_base = assemble(&wiring(true, ""), allow(), Some(rule_one_cue()));
        assert!(no_base.note.is_some(), "缺端点必须可观察 degraded");
        assert!(!no_base.runtime.expect("仍装配").enabled());

        let configured = assemble(
            &wiring(true, "http://127.0.0.1:11434/v1"),
            allow(),
            Some(rule_one_cue()),
        );
        let rt = configured.runtime.expect("装配");
        assert!(rt.enabled());
        assert_eq!(rt.client_kind(), "openai");
        assert!(configured.note.is_none());
    }
}
