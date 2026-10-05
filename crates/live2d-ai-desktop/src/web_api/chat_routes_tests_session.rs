//! `chat_routes` L1 会话 id 路由回归（opt-in / mutating 契约 / 往返）（自内联 `mod tests` 拆出）。
//!
//! 脚手架见 `tests_support`。

use super::tests_support::*;
use super::*;

// ------------------------------------------------ L1 会话 id 宿主能力

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
