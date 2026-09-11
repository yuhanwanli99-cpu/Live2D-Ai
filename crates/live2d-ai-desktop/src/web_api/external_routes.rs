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
//! - **可选 token 鉴权**：当环境变量 `EXTERNAL_INPUT_TOKEN` 存在时，
//!   要求请求体 `token` 字段或 `Authorization: Bearer <token>` 匹配；
//!   否则 `401 unauthorized`。env 未设 → 不鉴权（向后兼容，参见
//!   [`check_token`] 与 [`bearer_token`]）。
//!
//! # token 来源说明
//!
//! v1 鉴权 token 来源于**环境变量** `EXTERNAL_INPUT_TOKEN`（而非 Mod config），
//! 这样 handler 保持与 `ModRegistry` 解耦——`ServerContext` 没有 mod_registry
//! 槽位，本函数仅访问 `ServerContext.security` 与 `supervisor_slot`。
//! Mod settings 中的 `token` 字段（secret）用于前端渲染；运行时 host 负责把
//! 设置值写入环境变量后再启动 server（E6 再接入 Mod config 回环）。

use std::io::Cursor;

use tiny_http::{Header, Method, Response, StatusCode};

use crate::web_api::ServerContext;

/// 鉴权 token 来源的环境变量名（v1 简化：从环境变量读，不穿 Mod config）。
const TOKEN_ENV_VAR: &str = "EXTERNAL_INPUT_TOKEN";

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
    // JSON 解析（一次性抽取 text + 可选 token）。
    let parsed = match parse_payload(body) {
        Ok(p) => p,
        Err(msg) => {
            return Some(json_error(StatusCode(400), "invalid_payload", &msg));
        }
    };
    // 可选 token 鉴权：env `EXTERNAL_INPUT_TOKEN` 设了才要求鉴权。
    let env_token = std::env::var(TOKEN_ENV_VAR).ok();
    if !check_token(parsed.token.as_deref(), None, env_token.as_deref()) {
        return Some(json_error(
            StatusCode(401),
            "unauthorized",
            "token 缺失或不匹配（env EXTERNAL_INPUT_TOKEN 已设）",
        ));
    }
    let text = parsed.text;
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

/// 从 JSON body `{"text": "...", "token": "..."}` 抽取 text 与可选 token。
///
/// 要求：`text` 为非空字符串；`token` 如存在必须为字符串（鉴权用）。
fn parse_payload(body: &str) -> Result<Payload, String> {
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
    // 可选 token 字段（string）。
    let token = match v.get("token") {
        Some(t) if t.is_string() => t.as_str().map(|s| s.to_string()),
        Some(_) => return Err("`token` 必须为字符串".to_string()),
        None => None,
    };
    Ok(Payload {
        text: s.to_string(),
        token,
    })
}

/// 解析出的请求体（text + 可选 token）。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Payload {
    text: String,
    token: Option<String>,
}

/// 可选 token 鉴权的纯函数。
///
/// - `env_token` 为 `None` 时 → 始终通过（env 未设，向后兼容）。
/// - `env_token` 为 `Some(t)` 时 → 仅当 `body_token` 或
///   `auth_header`（`Authorization: Bearer <token>`）与之相等时放行。
///   二者均缺/不匹配 → 拒绝。
///
/// 该函数不读环境，不依赖全局状态，便于 `#[cfg(test)]` 注入。
pub fn check_token(
    body_token: Option<&str>,
    auth_header: Option<&str>,
    env_token: Option<&str>,
) -> bool {
    match env_token {
        None => true,
        Some(expected) => {
            // 优先 body token；其次 Authorization: Bearer。
            let body_ok = body_token.map(|b| b == expected).unwrap_or(false);
            let bearer_ok = auth_header
                .and_then(|h| bearer_token(Some(h)))
                .map(|t| t == expected)
                .unwrap_or(false);
            body_ok || bearer_ok
        }
    }
}

/// 从 `Authorization` 头值提取 bearer token。
///
/// 仅当前缀 `Bearer `（大小写不敏感）存在时返回值；否则 `None`。
/// 例如 `"Bearer abc"` → `Some("abc")`；`"abc"` → `None`。
fn bearer_token(auth_header: Option<&str>) -> Option<&str> {
    let h = auth_header?;
    let lower = h.to_ascii_lowercase();
    if !lower.starts_with("bearer ") {
        return None;
    }
    // 跳过 `bearer ` 前缀（7 字节），原串切片（大小写已校验）。
    let token = &h[7..];
    if token.is_empty() { None } else { Some(token) }
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

    // --- 可选 token 鉴权（纯函数，无 env 副作用） ---

    #[test]
    fn check_token_no_env_always_ok() {
        // env 未设 → 无论 body/header 如何都放行。
        assert!(check_token(None, None, None));
        assert!(check_token(Some("x"), None, None));
        assert!(check_token(None, Some("Bearer x"), None));
        assert!(check_token(Some("x"), Some("Bearer y"), None));
    }

    #[test]
    fn check_token_body_match_passes() {
        assert!(check_token(Some("secret"), None, Some("secret")));
    }

    #[test]
    fn check_token_body_mismatch_fails() {
        assert!(!check_token(Some("wrong"), None, Some("secret")));
    }

    #[test]
    fn check_token_missing_fails() {
        // env 设了但 body/header 均无 token → 拒绝。
        assert!(!check_token(None, None, Some("secret")));
    }

    #[test]
    fn check_token_bearer_match_passes() {
        assert!(check_token(None, Some("Bearer secret"), Some("secret")));
    }

    #[test]
    fn check_token_bearer_mismatch_fails() {
        assert!(!check_token(None, Some("Bearer wrong"), Some("secret")));
        // 不是 Bearer 格式 → 不匹配。
        assert!(!check_token(None, Some("wrong"), Some("secret")));
    }

    #[test]
    fn bearer_token_extracts_correctly() {
        assert_eq!(bearer_token(Some("Bearer abc")), Some("abc"));
        assert_eq!(bearer_token(Some("bearer abc")), Some("abc"));
        // 缺 `Bearer ` 前缀 → None。
        assert_eq!(bearer_token(Some("abc")), None);
        // 空 token → None。
        assert_eq!(bearer_token(Some("Bearer ")), None);
        assert_eq!(bearer_token(None), None);
    }

    #[test]
    fn check_token_body_takes_priority_over_mismatched_bearer() {
        // body token 匹配即可放行。
        assert!(check_token(
            Some("secret"),
            Some("Bearer wrong"),
            Some("secret")
        ));
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
