//! `web_api::dispatch_with_security` 端到端集成测试（D-P0B，2026-08-29）。
//!
//! 验证：
//! - 恶意 Origin mutating → 403 origin_denied
//! - no-cors 风格（无 Origin + 非 JSON CT）→ 403 origin_required
//! - 合法同源 + application/json → 200
//! - 合法同源但缺 CT → 403 content_type_required
//! - GET / 只读端点不受影响
//! - dev flag（allow_no_origin=true）下工具客户端放行
//!
//! 入口：`web_api::mod tests_security_e2e;`（仅 `#[cfg(test)]` 下生效）。

use std::io::Read;

use live2d_ai_runtime::AppSettings;
use tiny_http::Method;

/// 严格模式 SecurityContext（port=18080，allow_no_origin=false）。
fn strict_sec() -> super::security::SecurityContext {
    super::security::SecurityContext::new(18080, false)
}

/// dev flag 模式 SecurityContext（port=18080，allow_no_origin=true）。
fn dev_sec() -> super::security::SecurityContext {
    super::security::SecurityContext::new(18080, true)
}

fn strict_ctx() -> super::ServerContext {
    super::ServerContext::new(AppSettings::default(), "/tmp/cfg.toml".into(), None)
        .with_security(strict_sec())
}

fn dev_flag_ctx() -> super::ServerContext {
    super::ServerContext::new(AppSettings::default(), "/tmp/cfg.toml".into(), None)
        .with_security(dev_sec())
}

fn body_to_string(resp: tiny_http::Response<std::io::Cursor<Vec<u8>>>) -> String {
    let mut reader = resp.into_reader();
    let mut s = String::new();
    let _ = reader.read_to_string(&mut s);
    s
}

// ---------- mutating：Origin 失败 ----------

/// PATCH /api/v1/settings：恶意 Origin → 403 origin_denied。
#[test]
fn dispatch_patch_malicious_origin_returns_403() {
    let resp = super::dispatch_with_security(
        &strict_ctx(),
        &Method::Patch,
        "/api/v1/settings",
        r#"{"llm":{"model":"x"}}"#,
        Some("http://evil.com:18080"),
        Some("application/json"),
    );
    assert_eq!(resp.status_code().0, 403);
    let body = body_to_string(resp);
    assert!(body.contains("origin_denied"), "got: {body}");
}

/// POST /api/v1/chat：无 Origin 严格模式 → 403 origin_required。
#[test]
fn dispatch_chat_post_no_origin_strict_rejects() {
    let resp = super::dispatch_with_security(
        &strict_ctx(),
        &Method::Post,
        "/api/v1/chat",
        r#"{"text":"hi"}"#,
        None,
        Some("application/json"),
    );
    assert_eq!(resp.status_code().0, 403);
    let body = body_to_string(resp);
    assert!(body.contains("origin_required"), "got: {body}");
}

// ---------- mutating：Content-Type 失败 ----------

/// PATCH /api/v1/settings：无 Origin + 非 JSON CT（no-cors 风格）→ 403 origin_required。
#[test]
fn dispatch_patch_no_origin_and_text_plain_returns_403() {
    let resp = super::dispatch_with_security(
        &strict_ctx(),
        &Method::Patch,
        "/api/v1/settings",
        r#"{"llm":{"model":"x"}}"#,
        None,
        Some("text/plain"),
    );
    assert_eq!(resp.status_code().0, 403);
    let body = body_to_string(resp);
    assert!(body.contains("origin_required"), "got: {body}");
}

/// PATCH /api/v1/settings：合法同源但 Content-Type 缺省 → 403 content_type_required。
#[test]
fn dispatch_patch_same_origin_without_ct_returns_403() {
    let resp = super::dispatch_with_security(
        &strict_ctx(),
        &Method::Patch,
        "/api/v1/settings",
        r#"{"dev_mode":false}"#,
        Some("http://127.0.0.1:18080"),
        None,
    );
    assert_eq!(resp.status_code().0, 403);
    let body = body_to_string(resp);
    assert!(body.contains("content_type_required"), "got: {body}");
}

/// PATCH /api/v1/settings：合法同源 + text/plain → 403 content_type_required。
#[test]
fn dispatch_patch_same_origin_with_text_plain_returns_403() {
    let resp = super::dispatch_with_security(
        &strict_ctx(),
        &Method::Patch,
        "/api/v1/settings",
        r#"{"dev_mode":false}"#,
        Some("http://127.0.0.1:18080"),
        Some("text/plain"),
    );
    assert_eq!(resp.status_code().0, 403);
    let body = body_to_string(resp);
    assert!(body.contains("content_type_required"), "got: {body}");
}

// ---------- mutating：通过路径 ----------

/// PATCH /api/v1/settings：合法同源 + application/json → 200。
#[test]
fn dispatch_patch_same_origin_with_json_returns_200() {
    let resp = super::dispatch_with_security(
        &strict_ctx(),
        &Method::Patch,
        "/api/v1/settings",
        r#"{"dev_mode":false}"#,
        Some("http://127.0.0.1:18080"),
        Some("application/json"),
    );
    assert_eq!(resp.status_code().0, 200);
}

/// PATCH /api/v1/settings：合法 localhost + application/json → 200。
#[test]
fn dispatch_patch_localhost_with_json_returns_200() {
    let resp = super::dispatch_with_security(
        &strict_ctx(),
        &Method::Patch,
        "/api/v1/settings",
        r#"{"dev_mode":false}"#,
        Some("http://localhost:18080"),
        Some("application/json"),
    );
    assert_eq!(resp.status_code().0, 200);
}

/// POST /api/v1/chat/stop：无 body mutating，CT 宽容 → 200。
#[test]
fn dispatch_chat_stop_no_body_lenient() {
    let resp = super::dispatch_with_security(
        &strict_ctx(),
        &Method::Post,
        "/api/v1/chat/stop",
        "",
        Some("http://127.0.0.1:18080"),
        Some("text/plain"),
    );
    // 无 supervisor 时 stop 仍 200（幂等）。
    assert_eq!(resp.status_code().0, 200);
}

// ---------- GET / 只读端点不受影响 ----------

/// GET /api/v1/app/capabilities：无 Origin + 任意 CT → 200（GET 不校验）。
#[test]
fn dispatch_get_capabilities_unaffected_by_security() {
    let resp = super::dispatch_with_security(
        &strict_ctx(),
        &Method::Get,
        "/api/v1/app/capabilities",
        "",
        None,
        Some("text/plain"),
    );
    assert_eq!(resp.status_code().0, 200);
}

/// GET /api/v1/app/status：无 Origin → 200。
#[test]
fn dispatch_get_status_unaffected() {
    let resp = super::dispatch_with_security(
        &strict_ctx(),
        &Method::Get,
        "/api/v1/app/status",
        "",
        None,
        None,
    );
    assert_eq!(resp.status_code().0, 200);
}

// ---------- dev flag 工具模式 ----------

/// POST /api/v1/chat：dev flag 下无 Origin + 无 CT → 通过安全校验（503 因无 supervisor）。
#[test]
fn dispatch_chat_post_dev_flag_allows_no_origin() {
    let resp = super::dispatch_with_security(
        &dev_flag_ctx(),
        &Method::Post,
        "/api/v1/chat",
        r#"{"text":"hi"}"#,
        None,
        None,
    );
    assert_eq!(resp.status_code().0, 503, "应通过安全校验落到 503");
    let body = body_to_string(resp);
    assert!(
        !body.contains("origin_required") && !body.contains("content_type_required"),
        "got: {body}"
    );
}

/// PATCH /api/v1/settings：dev flag 下无 Origin + text/plain → 200。
#[test]
fn dispatch_patch_dev_flag_allows_no_origin_and_non_json() {
    let resp = super::dispatch_with_security(
        &dev_flag_ctx(),
        &Method::Patch,
        "/api/v1/settings",
        r#"{"dev_mode":false}"#,
        None,
        Some("text/plain"),
    );
    assert_eq!(resp.status_code().0, 200);
}

// ---------- is_mutating_route 行为 ----------

/// `is_mutating_route` 对所有 mutating 端点返回 true；GET / 只读返回 false。
/// （已在 tests_security.rs 覆盖；此处保留端到端 sanity。）
#[test]
fn mutating_routes_classification_sanity() {
    use super::RouteId;
    use super::security::is_mutating_route;
    use tiny_http::Method;
    assert!(is_mutating_route(RouteId::SettingsPatch, &Method::Patch));
    assert!(is_mutating_route(RouteId::ChatPost, &Method::Post));
    assert!(is_mutating_route(RouteId::Models, &Method::Post));
    assert!(is_mutating_route(RouteId::Models, &Method::Delete));
    assert!(!is_mutating_route(RouteId::SettingsGet, &Method::Get));
    assert!(!is_mutating_route(RouteId::Models, &Method::Get));
}
