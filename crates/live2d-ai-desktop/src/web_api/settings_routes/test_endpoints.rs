//! `POST /api/v1/settings/test/{llm,tts}` 处理器（设置测试端点，W6-D）。
//!
//! 不写盘：用「暂存配置」发一次最小请求，失败按 D1 错误码分类。
//!
//! 从 [`super`] 拆出以控制 `mod.rs` 文件行数（500 上限）—— handler
//! `handle_test_llm` / `handle_test_tts` 在 `super` 重导出。

use std::time::Duration;

use serde::Deserialize;
use tiny_http::{Response, StatusCode};

use live2d_ai_runtime::AppSettings;

use crate::web_api::app_routes::json_response;
use crate::web_api::dto::ErrorDetail;

/// `/api/v1/settings/test/{llm,tts}` body（可选超时）。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TestBody {
    /// 测试请求超时毫秒数（默认 3000，上限 8000）。
    #[serde(default)]
    pub timeout_ms: Option<u32>,
}

/// `POST /api/v1/settings/test/llm` 处理器：用 `current` 配置发最小 chat。
///
/// **不**写盘；返回测试结果（HTTP 200，无论 ok/fail——D1 §1.2 语义）。
pub fn handle_test_llm(
    current: &AppSettings,
    body: &TestBody,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let resp = run_llm_test(current, body.timeout_ms);
    json_response(StatusCode(200), &resp)
}

/// `POST /api/v1/settings/test/tts` 处理器。
pub fn handle_test_tts(
    current: &AppSettings,
    body: &TestBody,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let resp = run_tts_test(current, body.timeout_ms);
    json_response(StatusCode(200), &resp)
}

/// LLM 连通性测试结果（统一响应体：HTTP 200 + 语义字段）。
///
/// 序列化形态：`{"ok": true|false, "latency_ms"?: N, "model_echo"?: "...", "error"?: {...}}`。
/// `ok = false` 时 `error` 必填；`ok = true` 时 `latency_ms` + `model_echo` 必填。
#[derive(Debug, Clone, serde::Serialize)]
pub struct TestOutcome {
    /// `true` = 成功；`false` = 失败（带 `error` 字段）。
    pub ok: bool,
    /// 成功时：往返延迟（毫秒）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u128>,
    /// 成功时：回显字段（LLM = model 名，TTS = voice）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_echo: Option<String>,
    /// 失败时：错误详情。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorDetail>,
}

impl TestOutcome {
    /// 构造成功结果。
    pub fn success(latency_ms: u128, model_echo: impl Into<String>) -> Self {
        Self {
            ok: true,
            latency_ms: Some(latency_ms),
            model_echo: Some(model_echo.into()),
            error: None,
        }
    }
    /// 构造失败结果（按 D1 错误码）。
    pub fn failure(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            latency_ms: None,
            model_echo: None,
            error: Some(ErrorDetail::new(code, message)),
        }
    }
}

/// 把请求级 timeout_ms 转 tokio 兼容秒数（精度损失可接受）。
pub fn timeout_to_duration(ms: Option<u32>) -> Duration {
    const DEFAULT_MS: u32 = 3000;
    const MAX_MS: u32 = 8000;
    let ms = ms.unwrap_or(DEFAULT_MS).clamp(1, MAX_MS);
    Duration::from_millis(u64::from(ms))
}

/// 显式拼接 `base_url + path`（与 `live2d_ai_runtime` 私有实现同语义；
/// 仅用于 test 端点直发 reqwest——[`OpenAiClient::new`] 默认无 timeout，
/// 故绕开）。
fn join_endpoint(base: &str, path: &str) -> Result<url::Url, ()> {
    let trimmed = base.trim_end_matches('/');
    let parsed = url::Url::parse(&format!("{trimmed}{path}")).map_err(|_| ())?;
    match parsed.scheme() {
        "http" | "https" => Ok(parsed),
        _ => Err(()),
    }
}

/// 构造一个带 timeout 的 reqwest::Client。
#[allow(clippy::result_large_err)] // TestOutcome 是端点统一响应体，不可 Box。
fn build_http(timeout: Duration) -> Result<reqwest::Client, TestOutcome> {
    reqwest::Client::builder()
        .timeout(timeout)
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .map_err(|e| TestOutcome::failure("network_failed", format!("构造 HTTP 客户端: {e}")))
}

/// 把 `api_key_env` 解析为可写入 Authorization 头的 Option<String>（**不**进错误日志）。
fn resolve_api_key(env_name: Option<&str>) -> Option<String> {
    env_name
        .and_then(|n| std::env::var(n).ok())
        .filter(|v| !v.is_empty())
}

/// 同步 LLM 测试：用 `current.llm` 直接打 `/chat/completions`。
///
/// 不复用 [`OpenAiClient`]（`from_parts` 不可见；新构造默认无 timeout）。
/// 失败分类按 D1 错误码（§1.2）。
pub fn run_llm_test(current: &AppSettings, timeout_ms: Option<u32>) -> TestOutcome {
    let timeout = timeout_to_duration(timeout_ms);
    if current.llm.base_url.is_empty() {
        return TestOutcome::failure("llm_unreachable", "llm.base_url 未配置");
    }
    let http = match build_http(timeout) {
        Ok(c) => c,
        Err(e) => return e,
    };
    let started = std::time::Instant::now();
    let result: Result<(), HttpTestError> = (|| -> Result<(), HttpTestError> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(HttpTestError::Runtime)?;
        let url = match join_endpoint(&current.llm.base_url, "/chat/completions") {
            Ok(u) => u,
            Err(_) => return Err(HttpTestError::BadUrl),
        };
        let body = serde_json::json!({
            "model": current.llm.model,
            "stream": true,
            "messages": [{"role":"user","content":"hi"}],
        });
        let mut req = http.post(url).json(&body);
        if let Some(k) = resolve_api_key(current.llm.api_key_env.as_deref()) {
            req = req.bearer_auth(&k);
        }
        let fut = async move {
            let resp = req.send().await.map_err(HttpTestError::Http)?;
            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(HttpTestError::Upstream { status, body });
            }
            use futures_util::StreamExt;
            let mut stream = resp.bytes_stream();
            let _ = stream.next().await;
            Ok(())
        };
        rt.block_on(fut)
    })();
    let latency_ms = started.elapsed().as_millis();
    match result {
        Ok(()) => TestOutcome::success(latency_ms, current.llm.model.clone()),
        Err(e) => classify_http_error(e),
    }
}

/// 同步 TTS 测试：发最小 synth 请求。
fn run_tts_test(current: &AppSettings, timeout_ms: Option<u32>) -> TestOutcome {
    let timeout = timeout_to_duration(timeout_ms);
    if current.tts.base_url.is_empty() {
        return TestOutcome::failure("tts_unreachable", "tts.base_url 未配置");
    }
    let http = match build_http(timeout) {
        Ok(c) => c,
        Err(e) => return e,
    };
    let started = std::time::Instant::now();
    let result: Result<(), HttpTestError> = (|| -> Result<(), HttpTestError> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(HttpTestError::Runtime)?;
        let url = match join_endpoint(&current.tts.base_url, "/audio/speech") {
            Ok(u) => u,
            Err(_) => return Err(HttpTestError::BadUrl),
        };
        let body = serde_json::json!({
            "input": "test",
            "voice": current.tts.voice,
            "response_format": "pcm",
        });
        let mut req = http.post(url).json(&body);
        if let Some(k) = resolve_api_key(current.tts.api_key_env.as_deref()) {
            req = req.bearer_auth(&k);
        }
        let fut = async move {
            let resp = req.send().await.map_err(HttpTestError::Http)?;
            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(HttpTestError::Upstream { status, body });
            }
            use futures_util::StreamExt;
            let mut stream = resp.bytes_stream();
            if let Some(chunk) = stream.next().await {
                chunk.map_err(HttpTestError::Http)?;
            }
            Ok(())
        };
        rt.block_on(fut)
    })();
    let latency_ms = started.elapsed().as_millis();
    match result {
        Ok(()) => TestOutcome::success(latency_ms, current.tts.voice.clone()),
        Err(e) => classify_http_error(e),
    }
}

/// 测试端点专用错误：reqwest + tokio runtime + 上游状态。
#[derive(Debug)]
enum HttpTestError {
    /// tokio runtime 启动失败。
    Runtime(std::io::Error),
    /// reqwest 传输层错误。
    Http(reqwest::Error),
    /// 上游返回非 2xx 状态码。
    Upstream {
        status: reqwest::StatusCode,
        body: String,
    },
    /// URL 解析失败。
    BadUrl,
}

/// 把 [`HttpTestError`] 按 D1 错误码分类。
///
/// 分类口径：
/// - 401/403 → `auth_failed`；
/// - 404 → `model_not_found`；
/// - reqwest `is_timeout` → `timeout`；
/// - reqwest `is_connect` → `network_failed`；
/// - 其余 HTTP → `network_failed`；
/// - URL 解析失败 → `url_invalid`；
/// - 上游 5xx → `protocol_error`。
fn classify_http_error(e: HttpTestError) -> TestOutcome {
    match e {
        HttpTestError::Http(inner) => {
            if inner.is_timeout() {
                TestOutcome::failure("timeout", "请求超时")
            } else if inner.is_connect() {
                TestOutcome::failure("network_failed", "无法连接到上游")
            } else {
                TestOutcome::failure("network_failed", format!("HTTP 错误: {inner}"))
            }
        }
        HttpTestError::Upstream { status, body } => {
            let s = status.as_u16();
            // 响应体截断（≤512 字符）防日志/错误体爆体积。
            let snippet: String = body.chars().take(512).collect();
            if s == 401 || s == 403 {
                TestOutcome::failure("auth_failed", format!("鉴权失败 ({s})"))
            } else if s == 404 {
                TestOutcome::failure("model_not_found", "模型/资源未找到")
            } else {
                TestOutcome::failure("protocol_error", format!("上游 {s}: {snippet}"))
            }
        }
        HttpTestError::Runtime(e) => {
            TestOutcome::failure("network_failed", format!("runtime: {e}"))
        }
        HttpTestError::BadUrl => TestOutcome::failure("url_invalid", "base_url 非法"),
    }
}
