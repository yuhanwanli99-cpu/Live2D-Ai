//! 外部文本注入端点（节点 E5-T3，2026-08-30；0.2.0-rc.1 加强）。
//!
//! 职责：暴露 `POST /api/v1/external/chat`，把外部事件（本地程序 / 直播
//! 弹幕 sidecar / 消息回调）经 `supervisor.say` 注入主链路
//! （text → say → LLM 主链路）。**仅此一个外部面向公网的文本入口**。
//!
//! 契约全文（含 curl / sidecar 说明）：`docs/external-input.md`。
//! **B 站协议抓取不在主仓**——它在 Windows sidecar（`docs/examples/bilibili-sidecar/`），
//! 清洗成纯文本后打本端点；主仓/Rust 侧不实现 blivedm / WSS / 开放平台 SDK。
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
//!   因此本端点天然防远程访问——仅本地进程可达。
//! - **不回 `Access-Control-Allow-Origin`**（默认拒绝跨域）。
//!
//! # 启停门禁（0.2.0-rc.1）
//!
//! `external-input` Mod 是**唯一真源**的开关：
//!
//! - Mod 在注册表里且 `enabled=false` → `403 mod_disabled`
//!   （**不静默吞掉**外部文本，发送方必须知道）。
//! - Mod 在注册表里且 `enabled=true` → 放行。
//! - 注册表里**没有**该 Mod（极简测试上下文 / 自定义装配）→ 不设门禁（放行）：
//!   本端点属于 web_api 的核心 say 能力，不应因一个可选 Mod 缺失而失效。
//!   生产装配始终经 `crate::AVAILABLE_MOD_FACTORIES`，故一定有它。
//!
//! # 文本模板 / 前缀（0.2.0-rc.1）
//!
//! 注入前用 Mod config 的 `prefix` + `text_template`（`{text}` 为占位符）
//! 渲染，纯逻辑在同名 Mod crate（`render_from_config`），handler 不重写一份。
//! 长度上限对**渲染后**文本生效（模板膨胀同样会 400 `text_too_long`）。
//!
//! # token 来源（env 优先，0.2.0-rc.1）
//!
//! - env `EXTERNAL_INPUT_TOKEN` 非空 → 以它为准（部署期密钥，不落配置文件）；
//! - env 未设/为空 → 回落到 Mod config 的 `token`（前端可填，secret 存储）；
//! - 两者都空 → 不鉴权（仅 loopback，向后兼容）。
//!
//! 请求侧可用 body `token` 或 `Authorization: Bearer <token>`（二选一）。
//!
//! # 限制
//!
//! - **文本长度**：渲染后 ≤2000 字符，否则 `400 text_too_long`。
//! - **JSON 解析失败 / `text` 非字符串 / 空**：`400 invalid_payload`。
//! - **supervisor 未就绪**：`503 supervisor_unavailable`。
//! - **supervisor 忙碌（pending 缓冲已满）**：HTTP `200` + `{"ok":false,
//!   "error":{"code":"busy"}}` —— 文本已被丢弃，发送方应自行退避重试。

use std::io::Cursor;

use tiny_http::{Header, Method, Response, StatusCode};

use crate::web_api::ServerContext;

/// 鉴权 token 环境变量名（**优先于** Mod config 的 `token`）。
const TOKEN_ENV_VAR: &str = "EXTERNAL_INPUT_TOKEN";

/// 路由前缀（v1 固定一个外部文本端点）。
const EXTERNAL_CHAT_PATH: &str = "/api/v1/external/chat";

/// 提供启停门禁 / 模板 / token 的 Mod id。
const MOD_ID: &str = "external-input";

/// 最大允许文本长度（字符，作用于**渲染后**文本）。
const MAX_TEXT_LEN: usize = 2000;

/// 启停门禁 + Mod 配置快照。
struct ModGate {
    /// `true` = 放行（已启用，或注册表里没有该 Mod）。
    enabled: bool,
    /// Mod config（模板 / 前缀 / token）；
    config: serde_json::Value,
}

/// 读 `external-input` 的启停状态与配置（锁粒度：一次 clone）。
fn external_input_gate(ctx: &ServerContext) -> ModGate {
    let reg = match ctx.mod_registry.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(), // 锁中毒不 panic：门禁读快照即可。
    };
    match reg.config(MOD_ID) {
        Some(config) => ModGate {
            enabled: reg.is_enabled(MOD_ID),
            config: config.clone(),
        },
        // 未注册 = 无门禁（见头注「启停门禁」第三点）。
        None => ModGate {
            enabled: true,
            config: serde_json::json!({}),
        },
    }
}

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
    auth_header: Option<&str>,
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
    // 启停门禁：Mod 已注册但停用 → 不注入、明确告知发送方。
    let gate = external_input_gate(ctx);
    if !gate.enabled {
        return Some(json_error(
            StatusCode(403),
            "mod_disabled",
            "external-input Mod 未启用：请在前端「Mod 管理」启用，或 POST /api/v1/mods/external-input/enable",
        ));
    }
    // JSON 解析（一次性抽取 text + 可选 token）。
    let parsed = match parse_payload(body) {
        Ok(p) => p,
        Err(msg) => {
            return Some(json_error(StatusCode(400), "invalid_payload", &msg));
        }
    };
    // token：env 优先，其次 Mod config（secret）；都空 = 不鉴权。
    let env_token = std::env::var(TOKEN_ENV_VAR)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let config_token = live2d_ai_mod_external_input::token_from_config(&gate.config);
    let effective = env_token.as_deref().or(config_token.as_deref());
    if !check_token(parsed.token.as_deref(), auth_header, effective) {
        return Some(json_error(
            StatusCode(401),
            "unauthorized",
            "token 缺失或不匹配（env EXTERNAL_INPUT_TOKEN 或 Mod config token 已设）",
        ));
    }
    // 模板 / 前缀渲染（纯逻辑在 Mod crate；handler 不重写一份）。
    let text = live2d_ai_mod_external_input::render_from_config(&gate.config, &parsed.text);
    // 文本长度（对**渲染后**文本判定：模板膨胀也要拦）。
    if text.chars().count() > MAX_TEXT_LEN {
        return Some(json_error(
            StatusCode(400),
            "text_too_long",
            &format!(
                "text 最多 {} 字符（渲染后收到 {}）",
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
            "error": {"code": "busy", "message": "supervisor 忙碌（pending 缓冲已满），本条文本已被丢弃"}
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
/// - `env_token` 为 `None` 时 → 始终通过（未配置 token，向后兼容）。
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

    /// 一次调用（省略 auth 头 = None）。
    fn call(
        ctx: &ServerContext,
        method: &Method,
        path: &str,
        body: &str,
        origin: Option<&str>,
        ct: Option<&str>,
    ) -> Option<Response<Cursor<Vec<u8>>>> {
        handle_external_chat(ctx, method, path, body, origin, ct, None)
    }

    #[test]
    fn mismatched_path_returns_none() {
        let sec = crate::web_api::security::SecurityContext::new(18099, true);
        let ctx = dummy_ctx(sec);
        assert!(
            call(
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
        let resp = call(
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
        let resp = call(
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
        let resp = call(
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
        let resp = call(
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
        let resp = call(
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
        let resp = call(
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

    // --- 启停门禁（0.2.0-rc.1） ---

    /// 注册表中 external-input 停用 → 403 `mod_disabled`（不静默吞文本）。
    #[test]
    fn mod_disabled_returns_403() {
        let sec = crate::web_api::security::SecurityContext::new(18099, true);
        let ctx = ctx_with_manifest(sec, &serde_json::json!({}));
        let resp = call(
            &ctx,
            &method_post(),
            EXTERNAL_CHAT_PATH,
            r#"{"text":"hi"}"#,
            None,
            Some("application/json"),
        )
        .unwrap();
        assert_eq!(resp.status_code(), StatusCode(403));
    }

    /// 注册表中 external-input 启用 → 通过门禁（无 supervisor 时止于 503）。
    #[test]
    fn mod_enabled_passes_gate_to_supervisor() {
        let sec = crate::web_api::security::SecurityContext::new(18099, true);
        let ctx = ctx_with_manifest(
            sec,
            &serde_json::json!({"mods":{"external-input":{"enabled":true}}}),
        );
        let resp = call(
            &ctx,
            &method_post(),
            EXTERNAL_CHAT_PATH,
            r#"{"text":"hi"}"#,
            None,
            Some("application/json"),
        )
        .unwrap();
        assert_eq!(
            resp.status_code(),
            StatusCode(503),
            "启用后应过门禁，止于 supervisor 未就绪（而非 403）"
        );
    }

    /// 注册表里没有该 Mod（极简上下文）→ 不设门禁（止于 503，不是 403）。
    #[test]
    fn absent_mod_is_not_gated() {
        let sec = crate::web_api::security::SecurityContext::new(18099, true);
        let ctx = dummy_ctx(sec); // 空 factories
        let resp = call(
            &ctx,
            &method_post(),
            EXTERNAL_CHAT_PATH,
            r#"{"text":"hi"}"#,
            None,
            Some("application/json"),
        )
        .unwrap();
        assert_eq!(resp.status_code(), StatusCode(503));
    }

    // --- token：env 优先 / config 回落 / Bearer 头 ---

    /// `Authorization: Bearer` 必须真的被 handler 读取（旧版传 None，是缺陷）。
    #[test]
    fn config_token_accepts_bearer_header() {
        if std::env::var(TOKEN_ENV_VAR).is_ok() {
            return; // 环境已设 env token 时该路径不适用。
        }
        let sec = crate::web_api::security::SecurityContext::new(18099, true);
        let ctx = ctx_with_manifest(
            sec,
            &serde_json::json!({"mods":{"external-input":{
                "enabled": true,
                "config": {"token": "cfg-secret"}
            }}}),
        );
        let resp = handle_external_chat(
            &ctx,
            &method_post(),
            EXTERNAL_CHAT_PATH,
            r#"{"text":"hi"}"#,
            None,
            Some("application/json"),
            Some("Bearer cfg-secret"),
        )
        .unwrap();
        assert_ne!(
            resp.status_code(),
            StatusCode(401),
            "Bearer 头匹配 config token 时应放行（止于 503）"
        );
    }

    /// config token 已设但请求不带 token → 401。
    #[test]
    fn config_token_missing_401() {
        if std::env::var(TOKEN_ENV_VAR).is_ok() {
            return;
        }
        let sec = crate::web_api::security::SecurityContext::new(18099, true);
        let ctx = ctx_with_manifest(
            sec,
            &serde_json::json!({"mods":{"external-input":{
                "enabled": true,
                "config": {"token": "cfg-secret"}
            }}}),
        );
        let resp = call(
            &ctx,
            &method_post(),
            EXTERNAL_CHAT_PATH,
            r#"{"text":"hi"}"#,
            None,
            Some("application/json"),
        )
        .unwrap();
        assert_eq!(resp.status_code(), StatusCode(401));
    }

    // --- 模板膨胀后的长度门禁 ---

    #[test]
    fn template_inflated_text_too_long_400() {
        if std::env::var(TOKEN_ENV_VAR).is_ok() {
            return;
        }
        let sec = crate::web_api::security::SecurityContext::new(18099, true);
        let ctx = ctx_with_manifest(
            sec,
            &serde_json::json!({"mods":{"external-input":{
                "enabled": true,
                "config": {"prefix": "x".repeat(MAX_TEXT_LEN + 5)}
            }}}),
        );
        let resp = call(
            &ctx,
            &method_post(),
            EXTERNAL_CHAT_PATH,
            r#"{"text":"hi"}"#,
            None,
            Some("application/json"),
        )
        .unwrap();
        assert_eq!(
            resp.status_code(),
            StatusCode(400),
            "渲染后超长同样应 400 text_too_long"
        );
    }

    // --- 可选 token 鉴权（纯函数，无 env 副作用） ---

    #[test]
    fn check_token_no_env_always_ok() {
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
        assert!(!check_token(None, None, Some("secret")));
    }

    #[test]
    fn check_token_bearer_match_passes() {
        assert!(check_token(None, Some("Bearer secret"), Some("secret")));
    }

    #[test]
    fn check_token_bearer_mismatch_fails() {
        assert!(!check_token(None, Some("Bearer wrong"), Some("secret")));
        assert!(!check_token(None, Some("wrong"), Some("secret")));
    }

    #[test]
    fn bearer_token_extracts_correctly() {
        assert_eq!(bearer_token(Some("Bearer abc")), Some("abc"));
        assert_eq!(bearer_token(Some("bearer abc")), Some("abc"));
        assert_eq!(bearer_token(Some("abc")), None);
        assert_eq!(bearer_token(Some("Bearer ")), None);
        assert_eq!(bearer_token(None), None);
    }

    #[test]
    fn check_token_body_takes_priority_over_mismatched_bearer() {
        assert!(check_token(
            Some("secret"),
            Some("Bearer wrong"),
            Some("secret")
        ));
    }

    // --- test helpers ---

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

    /// 用真实工厂表 + 指定 manifest 装配注册表（启停门禁测试用）。
    fn ctx_with_manifest(
        sec: crate::web_api::security::SecurityContext,
        manifest: &serde_json::Value,
    ) -> ServerContext {
        let mut ctx = dummy_ctx(sec);
        ctx.mod_registry = std::sync::Arc::new(std::sync::Mutex::new(
            crate::mod_registry::ModRegistry::new(crate::AVAILABLE_MOD_FACTORIES, manifest),
        ));
        ctx
    }
}
