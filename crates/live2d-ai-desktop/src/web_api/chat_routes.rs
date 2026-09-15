//! HTTP 对话入口（W2 任务，2026-08-29）。
//!
//! # 端点（D1 §1.1 + W2 任务）
//!
//! - `POST /api/v1/chat` body `{"text": "...", "session_id"?: "..."}` → 调
//!   `SupervisorHandle::say_scoped`（容量 1 通道；满则 429 busy）。
//!   `session_id` 是 **L1 会话绑定**的入口：同一句话带上会话 id 后，本轮 LLM
//!   请求的 system_prompt 取该会话的覆盖（persona / memory 按会话写），没有覆盖
//!   则回落全局 `persona.system_prompt`。id 归一化规则见
//!   `live2d_ai_mod_system::sanitize_session_id`；**非法 id 视为不带会话**
//!   （不报错——那会让前端多一条只在特定字符下才出现的失败路径）。
//! - `POST /api/v1/chat/stop` → 调 `SupervisorHandle::stop`（幂等）。
//! - `GET|POST /api/v1/chat/session` → **会话 id 宿主能力**（L1 基座）：
//!   GET 读「当前活动会话 + 已绑定的会话数/列表」，POST 设当前活动会话
//!   （`{"session_id": "..."}`；缺省/空 = 清掉）。
//!
//!   为什么切会话要**告诉服务端**：external-input（弹幕）/ voice-input（转写）
//!   这两条注入路径不带会话——它们的语义是「接在当前这段对话上」。宿主记下
//!   活动会话，注入才会落到**用户正在看的**那个会话，而不是上一次发消息的那个。
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
//!
//! # 行数豁免说明（用户硬约束 ≤500；本文件允许 ≤1000 头注豁免）
//!
//! 本文件当前约 560 行：对话入口 + stop + **L1 会话 id 路由**三个 handler，
//! 以及它们的契约测试全在同一文件。之所以不拆测试：这三个 handler 共用同一套
//! 响应构造器（`json_response` / `invalid_payload` / `busy_response` /
//! `no_supervisor`）与同一份「路由把查询串当路径」的历史教训（见 commit 历史：
//! 那次缺陷正是从 handler 与路由的边界漏出来的），拆开会让读者为了改一条响应
//! 文案来回跳两个文件。其余路由文件（`voice_routes_tests.rs` 等）把测试分出去，
//! 是因为它们的**测试**本身就比 handler 大得多。

use std::io::Cursor;

use serde::{Deserialize, Serialize};
use tiny_http::{Method, Response, StatusCode};

use live2d_ai_mod_system::SessionPromptSink as _;

use crate::supervisor::SupervisorHandle;
use crate::web_api::dto::{ErrorDetail, ErrorResponse};

/// `POST /api/v1/chat` 解析体。
#[derive(Debug, Default, Deserialize)]
pub struct ChatBody {
    /// 用户输入文本。空串视为 `invalid_payload`（空消息没意义）。
    pub text: Option<String>,
    /// 会话 id（L1 基座）。缺省 = 不带会话（全局桶）。
    ///
    /// 容忍未知额外字段（serde 默认忽略），所以旧前端 / 第三方客户端照常可用。
    #[serde(default)]
    pub session_id: Option<String>,
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
    // L1：会话 id 归一化在 `say_scoped` 内做（同一道闸，不在这里重复实现）。
    // 非法 id 的结局是「这句话按全局桶处理」，而不是 400——前端的会话 id 是
    // 本地生成的，出现非法值属于「客户端版本不匹配」，不该让用户发不出消息。
    if handle.say_scoped(text, parsed.session_id) {
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

/// 会话路由路径（L1 基座）。
pub const CHAT_SESSION_PATH: &str = "/api/v1/chat/session";

/// `GET|POST /api/v1/chat/session`（L1 会话 id 宿主能力）。
///
/// - 路径不匹配 → `None`（交还 dispatch）。
/// - GET：只读，**不校验 Origin**（与其它只读 GET 一致）。
/// - POST：mutating，校验 `application/json` + loopback Origin（与
///   external/voice 注入端点同一条安全口径）。
/// - 无 supervisor → 503 `no_supervisor`（会话表住在 supervisor 里）。
pub fn handle_chat_session(
    ctx: &crate::web_api::ServerContext,
    method: &Method,
    path: &str,
    body: &str,
    origin: Option<&str>,
    content_type: Option<&str>,
) -> Option<Response<Cursor<Vec<u8>>>> {
    if path != CHAT_SESSION_PATH {
        return None;
    }
    let Some(handle) = ctx.try_get_supervisor() else {
        return Some(no_supervisor());
    };
    let store = handle.session_scopes();
    match *method {
        Method::Get => {}
        Method::Post => {
            if !crate::web_api::security::is_json_content_type(content_type) {
                // 415（不是 400）：与 external/voice 注入端点同一条口径——
                // 「CT 不对」是协议层错误，前端据此能一眼分辨自己发错了什么。
                return Some(unsupported_media_type());
            }
            if !crate::web_api::security::is_allowed_origin(origin, &ctx.security) {
                let err = match origin {
                    Some(o) => crate::web_api::security::MutatingCheckError::BadOrigin {
                        got: Some(o.to_string()),
                    },
                    None => crate::web_api::security::MutatingCheckError::NoOrigin,
                };
                return Some(crate::web_api::security::mutating_check_error_response(
                    &err,
                ));
            }
            let parsed: SessionBody = match serde_json::from_str(body) {
                Ok(p) => p,
                Err(_) => return Some(invalid_payload("请求体不是合法 JSON")),
            };
            // 归一化后写入；非法 id 落成「清掉活动会话」而不是 400——
            // 与会话不该成为一条「只在特定字符下才失败」的路径（同 say_scoped）。
            let normalized = parsed
                .session_id
                .as_deref()
                .and_then(live2d_ai_mod_system::sanitize_session_id);
            handle.set_active_session(normalized.as_deref());
        }
        _ => {
            return Some(method_not_allowed(path, "GET/POST"));
        }
    }
    let payload = ChatSessionState {
        ok: true,
        active_session: store.active(),
        bound_sessions: store.len(),
        sessions: store.sessions(),
    };
    Some(json_response(
        StatusCode(200),
        serde_json::to_vec(&payload).unwrap_or_default(),
    ))
}

/// `POST /api/v1/chat/session` 请求体。
#[derive(Debug, Default, Deserialize)]
struct SessionBody {
    /// 目标会话 id；缺省 / 空 / 非法 = 清掉活动会话。
    #[serde(default)]
    session_id: Option<String>,
}

/// `GET|POST /api/v1/chat/session` 响应。
#[derive(Debug, Serialize)]
struct ChatSessionState {
    ok: bool,
    /// 宿主记录的当前活动会话（None = 没设过）。
    active_session: Option<String>,
    /// 已绑定人设/记忆的会话条数（会话表的长度）。
    bound_sessions: usize,
    /// 已绑定的会话 id 列表（稳定排序）。
    sessions: Vec<String>,
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

fn unsupported_media_type() -> Response<Cursor<Vec<u8>>> {
    let detail = ErrorDetail::new(
        "unsupported_media_type",
        "Content-Type 必须为 application/json",
    );
    json_response(
        StatusCode(415),
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

    // ------------------------------------------------ L1 会话 id 宿主能力

    /// 构造一个**带 supervisor** 的 ServerContext（会话表住在 supervisor 里）。
    fn ctx_with_supervisor() -> (
        crate::web_api::ServerContext,
        std::sync::Arc<SupervisorHandle>,
    ) {
        let client = live2d_ai_runtime::OpenAiClient::new(
            live2d_ai_runtime::LlmConfig::new("http://127.0.0.1:1/v1", "m"),
            live2d_ai_runtime::TtsConfig::new("http://127.0.0.1:2/v1", "alloy"),
        )
        .expect("client");
        let handle = std::sync::Arc::new(crate::supervisor::spawn_supervisor(
            crate::supervisor::SupervisorConfig {
                client,
                conversation: live2d_ai_runtime::ConversationConfig::new("人设"),
                capabilities: live2d_ai_core::ModelCapabilities::all(),
                audio: None,
                config_path: None,
                mod_events: None,
            },
            |_ev| {},
        ));
        let ctx = crate::web_api::ServerContext::new(
            live2d_ai_runtime::AppSettings::default(),
            "/tmp/l1-session-route.toml".into(),
            None,
        )
        // 真实端口白名单：`ServerContext::new` 出厂 port=0（严格模式），
        // 不显式给端口的话任何 Origin 都会被判 403——那是**测试夹具**的问题，
        // 不是产品行为（生产由 cli_entry 注入真实端口）。
        .with_security(crate::web_api::security::SecurityContext::new(18080, false))
        .with_supervisor(std::sync::Arc::clone(&handle));
        (ctx, handle)
    }

    /// 路径不匹配 → None（交还 dispatch）；无 supervisor → 503。
    #[test]
    fn chat_session_route_is_opt_in_and_needs_supervisor() {
        let ctx = crate::web_api::ServerContext::new(
            live2d_ai_runtime::AppSettings::default(),
            "/tmp/l1-session-route-none.toml".into(),
            None,
        );
        assert!(
            handle_chat_session(&ctx, &Method::Get, "/api/v1/chat", "", None, None).is_none(),
            "只认 /api/v1/chat/session"
        );
        let resp = handle_chat_session(&ctx, &Method::Get, CHAT_SESSION_PATH, "", None, None)
            .expect("path matches");
        assert_eq!(
            resp.status_code().0,
            503,
            "无 supervisor → 503 no_supervisor"
        );
        assert!(body(resp).contains("no_supervisor"));
    }

    /// mutating 安全闸：CT 非 JSON → 415；无 Origin → 403；错 method → 405。
    #[test]
    fn chat_session_post_enforces_mutating_contract() {
        let (ctx, handle) = ctx_with_supervisor();
        let ct_bad = handle_chat_session(
            &ctx,
            &Method::Post,
            CHAT_SESSION_PATH,
            "{}",
            Some("http://127.0.0.1:18080"),
            Some("text/plain"),
        )
        .expect("path matches");
        assert_eq!(ct_bad.status_code().0, 415);
        let no_origin = handle_chat_session(
            &ctx,
            &Method::Post,
            CHAT_SESSION_PATH,
            "{}",
            None,
            Some("application/json"),
        )
        .expect("path matches");
        assert_eq!(no_origin.status_code().0, 403);
        let bad_method = handle_chat_session(&ctx, &Method::Put, CHAT_SESSION_PATH, "", None, None)
            .expect("path matches");
        assert_eq!(bad_method.status_code().0, 405);
        handle.quit();
    }

    /// 写活动会话 + 读回：会话表条数与列表如实反映；非法 id 清掉活动会话（不是 400）。
    #[test]
    fn chat_session_round_trip_reports_active_and_bound_sessions() {
        use live2d_ai_mod_system::SessionPromptSink as _;
        let (ctx, handle) = ctx_with_supervisor();
        handle.session_scopes().set("s1", "会话一人设");
        handle.session_scopes().set("s2", "会话二人设");

        let resp = handle_chat_session(
            &ctx,
            &Method::Post,
            CHAT_SESSION_PATH,
            r#"{"session_id":"s1"}"#,
            Some("http://127.0.0.1:18080"),
            Some("application/json; charset=utf-8"),
        )
        .expect("path matches");
        assert_eq!(resp.status_code().0, 200);
        let b = body(resp);
        assert!(b.contains("\"active_session\":\"s1\""), "got: {b}");
        assert!(b.contains("\"bound_sessions\":2"), "got: {b}");
        assert!(b.contains("\"s2\""), "got: {b}");

        // GET 读回（只读端点不校验 Origin）。
        let read = handle_chat_session(&ctx, &Method::Get, CHAT_SESSION_PATH, "", None, None)
            .expect("path matches");
        assert_eq!(read.status_code().0, 200);
        assert!(body(read).contains("\"active_session\":\"s1\""));

        // 非法 id：按「不带会话」处理（清掉游标），**不是** 400。
        let bad = handle_chat_session(
            &ctx,
            &Method::Post,
            CHAT_SESSION_PATH,
            r#"{"session_id":"../etc/passwd"}"#,
            Some("http://127.0.0.1:18080"),
            Some("application/json"),
        )
        .expect("path matches");
        assert_eq!(bad.status_code().0, 200);
        assert!(body(bad).contains("\"active_session\":null"));

        handle.quit();
    }
}
