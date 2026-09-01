//! 外部文本注入端点（节点 E5-T3，2026-08-30）。
//!
//! 职责：暴露 `POST /api/v1/external/chat`，把外部文本经 `supervisor.say`
//! 注入主链路（text → say → LLM 主链路）。**仅此一个外部面向公网可达的
//! 文本入口**；其它外部触发源在 E6 扩展。
//!
//! # 安全边界（P0 红线复审）
//!
//! 同 `wasm_assets` 模式一样，本 handler 是 `run_request_loop` 里
//! `wasm_assets::handle_assets` 之后、`dispatch_with_security` 之前的
//! **静态前置路由**：因此 `dispatch_with_security` 的 mutating
//! 校验（[`crate::web_api::security::check_mutating_request`]）**不会**
//! 自动套用在本端点上——本函数**自身**完成安全校验：
//!
//! - **方法**：仅 `POST`，否则 `405`。
//! - **Origin**：必须同源 loopback（`http://127.0.0.1:<port>` /
//!   `http://localhost:<port>`），校验复用
//!   [`crate::web_api::security::is_allowed_origin`]。无 Origin 时走
//!   `SecurityContext.allow_no_origin` 开关（工具客户端 / curl）。
//!   校验失败 → `403 origin_denied`。
//! - **Content-Type**：必须以 `application/json` 开头（前缀匹配，兼容
//!   `application/json; charset=utf-8`），复用
//!   [`crate::web_api::security::is_json_content_type`]；否则 `415`。
//! - **监听本身仅 loopback**（`start_server` 绑定 `127.0.0.1`），
//!   因此本端点天然防因远程访问——仅本地进程可达。
//! - **不回 `Access-Control-Allow-Origin`**（默认拒绝跨域）。
//!
//! # 限制（v1 简化）
//!
//! - **不检查 external-input Mod 是否启用**：say 通道是主链路能力，
//!   Mod 仅是外部触发源声明；v1 不增加 Mod-enable gate，E6 再补。
//! - **不添加 `mod_registry` 字段**：`ServerContext` 目前没有 mod_registry
//!   槽位，本任务不改 `ServerContext` 结构；supervisor 句柄借出失败
//!   (503) 即结束。
//! - **文本长度**：≤2000 字符，否则 `400 text_too_long`。
//! - **JSON 解析失败 / `text` 非字符串 / 空**：`400 invalid_payload`。

use std::io::Cursor;

use tiny_http::{Header, Method, Response, StatusCode};

use crate::web_api::ServerContext;

/// 路由前缀（v1 固定一个外部文本端点）。
const EXTERNAL_CHAT_PATH: &str = "/api/v1/external/chat";

/// 最大允许文本长度（字符）。
const MAX_TEXT_LEN: usize = 2000;

/// 处理 `POST /api/v1/external/chat`（外部文本 → say → 主链路）。
///
/// - 路径/方法不匹配 → `None`（交给后续 `dispatch` 继续派发）。
/// - 其它校验/执行均在本函数完成。
pub fn handle_external_chat(
    ctx: &ServerContext,
    method: &Method,
    path: &str,
    body: &str,
    origin: Option<&str>,
    content_type: Option<&str>,
) -> Option<Response<Cursor<Vec<u8>>>> {
    // 路径不匹配：交还给 dispatch。
    if path != EXTERNAL_CHAT_PATH {
        return None;
    }
    // 方法校验。
    if *method != Method::Post {
        return Some(json_error(
            StatusCode(405),
            "method_not_allowed",
            "POST /api/v1/external/chat 仅支持 POST",
        ));
    }
    // Content-Type 校验（防表单 CSRF；curl/python 走 allow_no_origin 工具模式
    // 仍需显式 application/json）。
    if !crate::web_api::security::is_json_content_type(content_type) {
        return Some(json_error(
            StatusCode(415),
            "unsupported_media_type",
            "Content-Type 必须为 application/json",
        ));
    }
    // Origin 校验：同源 loopback，复用 security 判定。
    if !crate::web_api::security::is_allowed_origin(origin, &ctx.security) {
        // 复用 mutating_check_error 的 403 体风格（origin_denied / origin_required）。
        // `from_origin` 私有：直接构造公开枚举体。
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
    // JSON 解析 + 字段校验。
    let text: String = match parse_text(body) {
        Ok(t) => t,
        Err(msg) => {
            return Some(json_error(StatusCode(400), "invalid_payload", &msg));
        }
    };
    // 文本长度。
    if text.chars().count() > MAX_TEXT_LEN {
        return Some(json_error(
            StatusCode(400),
            "text_too_long",
            &format!(
                "text 最多 {} 字符（收到 {}）",
                MAX_TEXT_LEN,
                text.chars().count()
            ),
        ));
    }
    // 执行：借出 supervisor say。
    let supervisor = match ctx.try_get_supervisor() {
        Some(s) => s,
        None => {
            return Some(json_error(
                StatusCode(503),
                "supervisor_unavailable",
                "supervisor 未就绪，无法注入文本",
            ));
        }
    };
    let ok = supervisor.say(text.clone());
    // 返回 {"ok": true/false, ...}。
    let body_json = if ok {
        serde_json::json!({"ok": true, "endpoint": "external.chat"})
    } else {
        serde_json::json!({
            "ok": false,
            "endpoint": "external.chat",
            "error": {"code": "busy", "message": "supervisor 忙碌（pending 缓冲已满）"}
        })
    };
    Some(ok_response(StatusCode(200), &body_json.to_string()))
}

/// 从 JSON body `{"text": "..."}` 抽取 text。
///
/// 要求：`text` 为非空字符串。
fn parse_text(body: &str) -> Result<String, String> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("JSON 解析失败: {e}"))?;
    let Some(text) = v.get("text") else {
        return Err("缺少 `text` 字段".to_string());
    };
    let Some(s) = text.as_str() else {
        return Err("`text` 必须为字符串".to_string());
    };
    let s = s.trim();
    if s.is_empty() {
        return Err("`text` 不得为空".to_string());
    }
    Ok(s.to_string())
}

/// 构造一个 JSON 响应（status + body；Content-Type application/json）。
fn ok_response(status: StatusCode, body: &str) -> Response<Cursor<Vec<u8>>> {
    Response::from_data(body.as_bytes().to_vec())
        .with_status_code(status)
        .with_header(
            Header::from_bytes(
                &b"Content-Type"[..],
                &b"application/json; charset=utf-8"[..],
            )
            .expect("Content-Type header"),
        )
        .with_header(
            Header::from_bytes(&b"Cache-Control"[..], &b"no-cache"[..])
                .expect("Cache-Control header"),
        )
}

/// 通用 JSON 错误响应（status + body；Content-Type application/json）。
fn json_error(status: StatusCode, code: &str, message: &str) -> Response<Cursor<Vec<u8>>> {
    let body = serde_json::json!({"error": {"code": code, "message": message}}).to_string();
    ok_response(status, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn method_post() -> Method {
        Method::Post
    }

    #[test]
    fn mismatched_path_returns_none() {
        let sec = crate::web_api::security::SecurityContext::new(18099, true);
        let ctx = dummy_ctx(sec);
        assert!(
            handle_external_chat(
                &ctx,
                &method_post(),
                "/api/v1/chat",
                r#"{"text":"hi"}"#,
                None,
                Some("application/json"),
            )
            .is_none()
        );
    }

    #[test]
    fn wrong_method_405() {
        let sec = crate::web_api::security::SecurityContext::new(18099, true);
        let ctx = dummy_ctx(sec);
        let resp = handle_external_chat(
            &ctx,
            &Method::Get,
            EXTERNAL_CHAT_PATH,
            r#"{"text":"hi"}"#,
            None,
            Some("application/json"),
        )
        .unwrap();
        assert_eq!(resp.status_code(), StatusCode(405));
    }

    #[test]
    fn non_json_ct_415() {
        let sec = crate::web_api::security::SecurityContext::new(18099, true);
        let ctx = dummy_ctx(sec);
        let resp = handle_external_chat(
            &ctx,
            &method_post(),
            EXTERNAL_CHAT_PATH,
            r#"x"#,
            None,
            Some("text/plain"),
        )
        .unwrap();
        assert_eq!(resp.status_code(), StatusCode(415));
    }

    #[test]
    fn bad_origin_403() {
        let sec = crate::web_api::security::SecurityContext::new(18099, false);
        let ctx = dummy_ctx(sec);
        let resp = handle_external_chat(
            &ctx,
            &method_post(),
            EXTERNAL_CHAT_PATH,
            r#"{"text":"hi"}"#,
            Some("http://evil.example"),
            Some("application/json"),
        )
        .unwrap();
        assert_eq!(resp.status_code(), StatusCode(403));
    }

    #[test]
    fn invalid_payload_400() {
        let sec = crate::web_api::security::SecurityContext::new(18099, true);
        let ctx = dummy_ctx(sec);
        let resp = handle_external_chat(
            &ctx,
            &method_post(),
            EXTERNAL_CHAT_PATH,
            "not json",
            None,
            Some("application/json"),
        )
        .unwrap();
        assert_eq!(resp.status_code(), StatusCode(400));
    }

    #[test]
    fn text_too_long_400() {
        let sec = crate::web_api::security::SecurityContext::new(18099, true);
        let ctx = dummy_ctx(sec);
        let long = format!("\"{}\"", "a".repeat(MAX_TEXT_LEN + 1));
        let body = serde_json::json!({"text": long}).to_string();
        let resp = handle_external_chat(
            &ctx,
            &method_post(),
            EXTERNAL_CHAT_PATH,
            &body,
            None,
            Some("application/json"),
        )
        .unwrap();
        assert_eq!(resp.status_code(), StatusCode(400));
    }

    #[test]
    fn text_empty_400() {
        let sec = crate::web_api::security::SecurityContext::new(18099, true);
        let ctx = dummy_ctx(sec);
        let resp = handle_external_chat(
            &ctx,
            &method_post(),
            EXTERNAL_CHAT_PATH,
            r#"{"text":"   "}"#,
            None,
            Some("application/json"),
        )
        .unwrap();
        assert_eq!(resp.status_code(), StatusCode(400));
    }

    // --- test helper ---

    fn dummy_ctx(sec: crate::web_api::security::SecurityContext) -> ServerContext {
        ServerContext {
            status_ctx: std::sync::Arc::new(crate::web_api::app_routes::StatusContext::new(
                live2d_ai_runtime::AppSettings::default(),
                ".scratch".to_string(),
                None,
            )),
            capabilities_ctx: std::sync::Arc::new(
                crate::web_api::app_routes::CapabilitiesContext::new(),
            ),
            supervisor_slot: crate::web_api::supervisor_slot::SupervisorSlot::new(),
            broadcaster: crate::web_api::ws::Broadcaster::new(),
            security: sec,
            models: crate::web_api::models_routes::default_store(),
            mod_registry: std::sync::Arc::new(std::sync::Mutex::new(
                crate::mod_registry::ModRegistry::new(&[], &serde_json::json!({})),
            )),
        }
    }
}
