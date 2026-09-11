//! HTTP 对话入口（W2 任务，2026-08-29）。
//!
//! # 端点（D1 §1.1 + W2 任务）
//!
//! - `POST /api/v1/chat` body `{"text": "..."}` → 调 `SupervisorHandle::say`
//!   （容量 1 通道；满则 429 busy）。
//! - `POST /api/v1/chat/stop` → 调 `SupervisorHandle::stop`（幂等）。
//!
//! # 状态码语义
//!
//! - 200 + `{"accepted": true, "epoch": N}`：say 被受理（pending）。epoch 是
//!   supervisor **当前**代次（`u64`，给前端在 WS 上等待 `turn_state.epoch` 关联）。
//! - 400 `invalid_payload`：body 非 JSON 或缺 `text` 字段。
//! - 405 `method_not_allowed`：method 非 POST（保留路由层校验）。
//! - 429 `busy`：say 通道满（已有 pending）。前端应等待 WS 上
//!   `turn_state`/`runtime_status.new_epoch` 后再发。
//! - 503 `no_supervisor`：web 模式未装配 supervisor（仅纯控制平面场景）。
//!
//! # 不变量
//!
//! - handler **不**触碰 `core::apply`/`AppEvent::Render`（D1 §7.3 严格仲裁路径）。
//! - 全文**不**读取文件（除 `request_reload` 由 PATCH 路径触发）。

use std::io::Cursor;

use serde::{Deserialize, Serialize};
use tiny_http::{Method, Response, StatusCode};

use crate::supervisor::SupervisorHandle;
use crate::web_api::dto::{ErrorDetail, ErrorResponse};

/// `POST /api/v1/chat` 解析体。
#[derive(Debug, Default, Deserialize)]
pub struct ChatBody {
    /// 用户输入文本。空串视为 `invalid_payload`（空消息没意义）。
    pub text: Option<String>,
}

/// `POST /api/v1/chat` 响应。
#[derive(Debug, Serialize)]
pub struct ChatAccepted {
    /// 是否已被 supervisor 接受。
    pub accepted: bool,
    /// 受理时 supervisor 的当前代次（前端在 WS 上等待此 epoch 的 turn_state）。
    pub epoch: u64,
    /// 通道容量 1；满时 429，否则 200。
    /// 保留字段，便于前端对账（不会暴露内部状态）。
    pub pending_cleared: bool,
}

/// `POST /api/v1/chat/stop` 响应。
#[derive(Debug, Serialize)]
pub struct StopAccepted {
    /// stop 命令已发出（不阻塞；supervisor 在 turn 边界处理）。
    pub accepted: bool,
}

/// Chat handler（带 supervisor 句柄；dispatch 调用本版本）。
///
/// P1WS-1：`current_epoch` 参数**仅**在无 supervisor 句柄时作为占位（保持
/// 旧 `0` 回退）——有句柄时直接读 [`SupervisorHandle::current_epoch`]，使
/// 前端拿到的 epoch 与 WS 上 `turn_state` / `text_delta` 真正对齐。无句柄
/// 503 路径不读 epoch（不返回 body）。
pub fn handle_chat_with_supervisor(
    handle: Option<&std::sync::Arc<SupervisorHandle>>,
    method: &Method,
    path: &str,
    body: &str,
    current_epoch_unused: u64,
) -> Response<Cursor<Vec<u8>>> {
    if *method != Method::Post {
        return method_not_allowed(path, "POST");
    }
    let parsed: ChatBody = match serde_json::from_str(body) {
        Ok(p) => p,
        Err(_) => return invalid_payload("请求体不是合法 JSON 或缺字段"),
    };
    let text = match parsed.text {
        Some(t) if !t.is_empty() => t,
        _ => return invalid_payload("字段 text 必填且非空"),
    };
    let Some(handle) = handle else {
        return no_supervisor();
    };
    if handle.say(text) {
        // P1WS-1：从 supervisor 读取**真实**当前 epoch（root.epoch 镜像）。
        // `Acquire` 与 supervisor 写入的 `Release` 配对；这是无锁快速路径。
        let epoch = handle.current_epoch();
        let _ = current_epoch_unused; // 参数保留以兼容旧测试夹具签名
        let body = serde_json::to_vec(&ChatAccepted {
            accepted: true,
            epoch,
            pending_cleared: false,
        })
        .unwrap_or_default();
        json_response(StatusCode(200), body)
    } else {
        busy_response()
    }
}

/// Stop handler（`POST /api/v1/chat/stop`）。
pub fn handle_stop_with_supervisor(
    handle: Option<&std::sync::Arc<SupervisorHandle>>,
    method: &Method,
    path: &str,
) -> Response<Cursor<Vec<u8>>> {
    if *method != Method::Post {
        return method_not_allowed(path, "POST");
    }
    if let Some(handle) = handle {
        handle.stop();
    }
    // 无 supervisor 时按"幂等 stop"语义：返回 200（v1 不阻塞前端控制按钮）。
    let body = serde_json::to_vec(&StopAccepted { accepted: true }).unwrap_or_default();
    json_response(StatusCode(200), body)
}

// ---------------------------------------------------------------- 内部响应

fn json_response(status: StatusCode, body: Vec<u8>) -> Response<Cursor<Vec<u8>>> {
    Response::from_data(body)
        .with_status_code(status)
        .with_header(
            tiny_http::Header::from_bytes(
                &b"Content-Type"[..],
                &b"application/json; charset=utf-8"[..],
            )
            .expect("Content-Type header"),
        )
}

fn invalid_payload(msg: &str) -> Response<Cursor<Vec<u8>>> {
    let detail = ErrorDetail::new("invalid_payload", msg);
    json_response(
        StatusCode(400),
        serde_json::to_vec(&ErrorResponse::new(detail))
            .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}").into_bytes()),
    )
}

fn method_not_allowed(path: &str, expected: &str) -> Response<Cursor<Vec<u8>>> {
    let detail = ErrorDetail::new("method_not_allowed", format!("{path:?} 需 {expected} 方法"));
    json_response(
        StatusCode(405),
        serde_json::to_vec(&ErrorResponse::new(detail))
            .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}").into_bytes()),
    )
}

fn busy_response() -> Response<Cursor<Vec<u8>>> {
    let detail = ErrorDetail::new(
        "busy",
        "已有 pending 输入（Say 通道容量 1），请等待 turn_state 收口后再发",
    );
    json_response(
        StatusCode(429),
        serde_json::to_vec(&ErrorResponse::new(detail))
            .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}").into_bytes()),
    )
}

fn no_supervisor() -> Response<Cursor<Vec<u8>>> {
    let detail = ErrorDetail::new(
        "no_supervisor",
        "web 模式未装配 supervisor（纯控制平面）；请先在 live2d-ai.toml 配 LLM/TTS 后重启",
    );
    json_response(
        StatusCode(503),
        serde_json::to_vec(&ErrorResponse::new(detail))
            .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}").into_bytes()),
    )
}

// ---------------------------------------------------------------- 单元测试

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read as _;

    fn body(resp: Response<Cursor<Vec<u8>>>) -> String {
        let mut s = String::new();
        let _ = resp.into_reader().read_to_string(&mut s);
        s
    }

    /// 无 supervisor 时 chat 返回 503。
    #[test]
    fn chat_without_supervisor_returns_503() {
        let resp =
            handle_chat_with_supervisor(None, &Method::Post, "/api/v1/chat", r#"{"text":"hi"}"#, 0);
        assert_eq!(resp.status_code().0, 503);
        let b = body(resp);
        assert!(b.contains("no_supervisor"), "got: {b}");
    }

    /// 无 supervisor 时 stop 仍 200（幂等）。
    #[test]
    fn stop_without_supervisor_returns_200_idempotent() {
        let resp = handle_stop_with_supervisor(None, &Method::Post, "/api/v1/chat/stop");
        assert_eq!(resp.status_code().0, 200);
    }

    /// 错 method → 405。
    #[test]
    fn chat_wrong_method_returns_405() {
        let resp =
            handle_chat_with_supervisor(None, &Method::Get, "/api/v1/chat", r#"{"text":"hi"}"#, 0);
        assert_eq!(resp.status_code().0, 405);
    }

    /// body 非 JSON → 400。
    #[test]
    fn chat_invalid_body_returns_400() {
        let resp = handle_chat_with_supervisor(None, &Method::Post, "/api/v1/chat", "not json", 0);
        assert_eq!(resp.status_code().0, 400);
    }

    /// 缺 text 字段 → 400。
    #[test]
    fn chat_missing_text_returns_400() {
        let resp = handle_chat_with_supervisor(None, &Method::Post, "/api/v1/chat", r#"{}"#, 0);
        assert_eq!(resp.status_code().0, 400);
    }

    /// text 空串 → 400。
    #[test]
    fn chat_empty_text_returns_400() {
        let resp =
            handle_chat_with_supervisor(None, &Method::Post, "/api/v1/chat", r#"{"text":""}"#, 0);
        assert_eq!(resp.status_code().0, 400);
    }

    /// 有 supervisor 时 say 被接受 → 200 + accepted=true。
    /// 用真实 supervisor（指向不可达端点，只走"send 成功"路径）。
    #[test]
    fn chat_with_supervisor_returns_200() {
        // 用 thread id 拼唯一文件名（cargo test 并发安全）。
        use std::hash::{Hash, Hasher};
        let thread_id = format!("{:?}", std::thread::current().id());
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        thread_id.hash(&mut hasher);
        let tmp = std::env::temp_dir().join(format!(
            "live2d_ai_test_chat_routes_{}_{}.toml",
            std::process::id(),
            hasher.finish()
        ));
        let _ = std::fs::remove_file(&tmp);
        std::fs::write(
            &tmp,
            live2d_ai_runtime::AppSettings::default().to_toml_string(),
        )
        .expect("write tmp");
        let client = live2d_ai_runtime::OpenAiClient::new(
            live2d_ai_runtime::LlmConfig::new("http://127.0.0.1:1/v1", "m"),
            live2d_ai_runtime::TtsConfig::new("http://127.0.0.1:2/v1", "alloy"),
        )
        .expect("client");
        let supervisor = std::sync::Arc::new(crate::supervisor::spawn_supervisor(
            crate::supervisor::SupervisorConfig {
                client,
                conversation: live2d_ai_runtime::ConversationConfig::new("人设"),
                capabilities: live2d_ai_core::ModelCapabilities::all(),
                audio: None,
                config_path: Some(tmp.to_string_lossy().into_owned()),
                mod_events: None,
            },
            |_ev| {},
        ));

        let resp = handle_chat_with_supervisor(
            Some(&supervisor),
            &Method::Post,
            "/api/v1/chat",
            r#"{"text":"hi"}"#,
            // 末位参数 v1 占位；P1WS-1 后被忽略，handler 改用
            // `handle.current_epoch()`（启动瞬间 = 0）。
            42,
        );
        assert_eq!(resp.status_code().0, 200, "first say should accept");
        let b = body(resp);
        assert!(b.contains("\"accepted\":true"), "got: {b}");
        // P1WS-1：epoch 来自 supervisor.current_epoch()；启动瞬间 = 0。
        assert!(b.contains("\"epoch\":0"), "got: {b}");

        // 第二次 say → 通道满（容量 1）→ 429。
        let resp2 = handle_chat_with_supervisor(
            Some(&supervisor),
            &Method::Post,
            "/api/v1/chat",
            r#"{"text":"hi again"}"#,
            42,
        );
        assert_eq!(
            resp2.status_code().0,
            429,
            "second say while pending should be busy"
        );
        let b2 = body(resp2);
        assert!(b2.contains("busy"), "got: {b2}");

        // stop → 200。
        let resp3 =
            handle_stop_with_supervisor(Some(&supervisor), &Method::Post, "/api/v1/chat/stop");
        assert_eq!(resp3.status_code().0, 200);

        // 收尾。
        supervisor.quit();
        let _ = std::fs::remove_file(&tmp);
    }
}
