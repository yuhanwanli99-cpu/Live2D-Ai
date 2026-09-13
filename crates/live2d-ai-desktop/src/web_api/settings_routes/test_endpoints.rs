//! `POST /api/v1/settings/test/{llm,tts}` 处理器（设置测试端点，W6-D）。
//!
//! 不写盘：用「暂存配置」发一次最小请求，失败按 D1 错误码分类。
//!
//! # TTS 自检为什么改成「轻量探针」而不是「真合成一次」（2026-09-13）
//!
//! 现场：用户报「**自检没通，但实际能正常出声**」。查证三条：
//! 1. 本机 CosyVoice 合成一次要 **2.4s**（冷启动更长），而自检默认只等 **3s**；
//! 2. 链路正在合成时再点自检 → 实测稳定复现 `{"ok":false,"code":"timeout"}`，
//!    **而同一时刻的语音链条完全正常**；
//! 3. HTTP 循环是**单线程**的（`run_request_loop` 一次只处理一个请求），
//!    所以这个同步探针还会把整个 API 占住它等待的全长——把它调到 15s 只会
//!    把「自检说谎」换成「点一下自检，界面卡 15 秒」。
//!
//! 结论：**「连通性自检」应当测连通性，而不是测合成本身**。现在打
//! `GET {base}/models`：毫秒级、不加载模型、不与真实链路抢资源，
//! 并且足以判定「端点可达 / 鉴权对不对」。合成快慢与是否留白**不在这里测**
//! ——那件事的证据是「发一条消息，听得见」。
//!
//! 反面教材就是本文件原来的实现：一个**会与产品链路抢资源、还会在链路繁忙时
//! 报假失败**的自检，比没有自检更坏——用户会据此去修一个没坏的东西。
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
    /// 成功但有话要说（例如「上游可达但没有 /models 端点」）。
    ///
    /// **只在真有解释价值时出现**——成功路径默认不带 note，否则每次自检都多一句
    /// 噪音，用户很快就不看它了。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl TestOutcome {
    /// 构造成功结果。
    pub fn success(latency_ms: u128, model_echo: impl Into<String>) -> Self {
        Self {
            ok: true,
            latency_ms: Some(latency_ms),
            model_echo: Some(model_echo.into()),
            error: None,
            note: None,
        }
    }
    /// 成功 + 一句解释（见 [`TestOutcome::note`]）。
    pub fn success_with_note(
        latency_ms: u128,
        model_echo: impl Into<String>,
        note: impl Into<String>,
    ) -> Self {
        Self {
            note: Some(note.into()),
            ..Self::success(latency_ms, model_echo)
        }
    }
    /// 构造失败结果（按 D1 错误码）。
    pub fn failure(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            latency_ms: None,
            model_echo: None,
            error: Some(ErrorDetail::new(code, message)),
            note: None,
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
///
/// 2026-09-13：改走 [`live2d_ai_runtime::secrets::lookup`]（`.env` 快照 > 进程环境）。
/// 这里原先直接 `std::env::var`，**绕过了 `.env`**——于是「服务不是经 `ignite.sh`
/// 起的（进程环境里没有那把 key）」时，自检会报鉴权失败，而**真实链路是好的**
/// （链路读的是 `.env`）。同一类假失败，只是换了个触发条件。
fn resolve_api_key(env_name: Option<&str>) -> Option<String> {
    env_name.and_then(live2d_ai_runtime::secrets::lookup)
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

/// 同步 TTS 自检：**只探连通**（`GET {base}/models`），不合成。
///
/// 为什么不是「真合成一次」见模块头注——简单说：合成慢、会与真实链路抢资源、
/// 并在链路繁忙时报假失败，而它带来的信息（音色是否被接受）远不如
/// 「发一条消息听得见」可靠。
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
    let result: Result<(u16, String), HttpTestError> =
        (|| -> Result<(u16, String), HttpTestError> {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(HttpTestError::Runtime)?;
            let url = match join_endpoint(&current.tts.base_url, "/models") {
                Ok(u) => u,
                Err(_) => return Err(HttpTestError::BadUrl),
            };
            let mut req = http.get(url);
            if let Some(k) = resolve_api_key(current.tts.api_key_env.as_deref()) {
                req = req.bearer_auth(&k);
            }
            let fut = async move {
                let resp = req.send().await.map_err(HttpTestError::Http)?;
                let status = resp.status().as_u16();
                // 只在失败时读 body（成功时没必要，而且 /models 可能很大）。
                let body = if (200..300).contains(&status) {
                    String::new()
                } else {
                    resp.text().await.unwrap_or_default()
                };
                Ok((status, body))
            };
            rt.block_on(fut)
        })();
    let latency_ms = started.elapsed().as_millis();
    match result {
        Ok((status, body)) => probe_outcome(status, &body, latency_ms, &current.tts.voice),
        Err(HttpTestError::Http(e)) if e.is_timeout() => TestOutcome::failure(
            "timeout",
            format!(
                "自检在 {} ms 内没等到上游响应。**这不等于不能出声**——自检只探连通，\
                 而实际链路没有这个短超时；冷启动 / 上游繁忙时稍等再点一次即可",
                timeout.as_millis()
            ),
        ),
        Err(e) => classify_http_error(e),
    }
}

/// `GET {base}/models` 探针的结果分类（**纯函数**，可单测）。
///
/// 口径（每条都有理由，别随手改）：
/// - `2xx` → 通；
/// - `401`/`403` → `auth_failed`：**可达**，但 key 不对/缺失——这是自检最该抓的一类；
/// - `404` → **算通过**，附一句说明：上游回了响应就证明「可达」，而 `/models`
///   并非所有语音实现都提供（只实现 `/audio/speech` 的也合法）。这里若判失败，
///   就是又造一个「自检说没通、实际能用」的假失败——正是本次要消灭的东西；
/// - 其余 `4xx`/`5xx` → `protocol_error`（可达但坏了），带上响应片段。
fn probe_outcome(status: u16, body: &str, latency_ms: u128, echo: &str) -> TestOutcome {
    if (200..300).contains(&status) {
        return TestOutcome::success(latency_ms, echo);
    }
    let snippet: String = body.chars().take(512).collect();
    match status {
        401 | 403 => TestOutcome::failure("auth_failed", format!("鉴权失败 ({status})")),
        404 => TestOutcome::success_with_note(
            latency_ms,
            echo,
            "上游可达（未提供 /models，连通性视为通过）",
        ),
        _ => TestOutcome::failure("protocol_error", format!("上游 {status}: {snippet}")),
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

#[cfg(test)]
mod probe_tests {
    use super::*;

    /// 通：2xx。
    #[test]
    fn probe_2xx_is_success() {
        let o = probe_outcome(200, "", 12, "skystar");
        assert!(o.ok);
        assert_eq!(o.latency_ms, Some(12));
        assert_eq!(o.model_echo.as_deref(), Some("skystar"));
        assert!(o.note.is_none(), "成功路径不带 note（默认不加噪音）");
    }

    /// 鉴权失败：可达但 key 不对——自检最该抓的一类，必须是失败且码精确。
    #[test]
    fn probe_401_403_is_auth_failed() {
        for s in [401u16, 403] {
            let o = probe_outcome(s, "no key", 5, "v");
            assert!(!o.ok);
            assert_eq!(o.error.as_ref().map(|e| e.code), Some("auth_failed"));
        }
    }

    /// **404 算通过**（附说明）。
    ///
    /// 这条是本次修复的核心：上游回了响应就证明「可达」，而 `/models` 并非所有
    /// 语音实现都提供。判失败就等于又造一个「自检说没通、实际能用」的假失败。
    #[test]
    fn probe_404_is_reachable_not_a_failure() {
        let o = probe_outcome(404, "not found", 8, "v");
        assert!(o.ok, "上游回话了就是可达：{o:?}");
        assert!(
            o.note.as_deref().is_some_and(|n| n.contains("可达")),
            "必须说明为什么算通过：{o:?}"
        );
    }

    /// 其余 4xx/5xx：可达但坏了，要带上响应片段（否则用户不知道坏在哪）。
    #[test]
    fn probe_other_errors_carry_detail() {
        let o = probe_outcome(500, "boom: model crashed", 9, "v");
        assert!(!o.ok);
        assert_eq!(o.error.as_ref().map(|e| e.code), Some("protocol_error"));
        assert!(
            o.error
                .as_ref()
                .is_some_and(|e| e.message.contains("model crashed")),
            "{o:?}"
        );
    }

    /// 响应体片段截断到 512 字符（错误体不该把响应撑爆 / 灌进日志）。
    #[test]
    fn probe_body_snippet_is_truncated() {
        let o = probe_outcome(500, &"x".repeat(5000), 1, "v");
        let msg = o.error.map(|e| e.message).unwrap_or_default();
        assert!(
            msg.chars().count() < 600,
            "响应片段未截断：{} 字符",
            msg.chars().count()
        );
    }
}
