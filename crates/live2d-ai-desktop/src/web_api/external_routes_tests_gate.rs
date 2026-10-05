//! `external_routes` 门禁 / 令牌回归（路径、方法、CT、Origin、payload 长度、
//! `mod_disabled` 三态、Bearer 与 config token 优先级）。脚手架见 `tests_support`。

use super::tests_support::*;
use super::*;

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
    let _guard = counter_lock(); // 403 会计 reject → 与计数回归串行。
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
///
/// W6：改用注入的 lookup（`no_env_lookup` = 进程环境/`.env` 都没有令牌），
/// 不再直接探测进程环境——测试因此**确定性**。
#[test]
fn config_token_accepts_bearer_header() {
    let sec = crate::web_api::security::SecurityContext::new(18099, true);
    let ctx = ctx_with_manifest(
        sec,
        &serde_json::json!({"mods":{"external-input":{
            "enabled": true,
            "config": {"token": "cfg-secret"}
        }}}),
    );
    let resp = handle_external_chat_with(
        &ctx,
        ExternalRequest {
            method: &method_post(),
            path: EXTERNAL_CHAT_PATH,
            body: r#"{"text":"hi"}"#,
            origin: None,
            content_type: Some("application/json"),
            auth_header: Some("Bearer cfg-secret"),
        },
        &accept_inject,
        &no_env_lookup,
    )
    .unwrap();
    assert_ne!(
        resp.status_code(),
        StatusCode(401),
        "Bearer 头匹配 config token 时应放行（止于 503）"
    );
}

/// config token 已设但请求不带 token → 401（注入 lookup = 无 env 令牌）。
#[test]
fn config_token_missing_401() {
    let _guard = counter_lock(); // 401 会计 reject → 与计数回归串行。
    let sec = crate::web_api::security::SecurityContext::new(18099, true);
    let ctx = ctx_with_manifest(
        sec,
        &serde_json::json!({"mods":{"external-input":{
            "enabled": true,
            "config": {"token": "cfg-secret"}
        }}}),
    );
    let resp = handle_external_chat_with(
        &ctx,
        req(&method_post(), r#"{"text":"hi"}"#),
        &accept_inject,
        &no_env_lookup,
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(401));
}
