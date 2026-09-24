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
