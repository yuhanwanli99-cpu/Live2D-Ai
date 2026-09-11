//! `web_api::security` 单元测试（拆分承载：security.rs 主体 ≤500；
//! 本批新增 Origin/CSRF 单元测试单独提到本文件以保留主体可扩展性）。
//!
//! 入口：`web_api::mod tests_security;`（仅 `#[cfg(test)]` 下生效）——本
//! 文件作为 `web_api` 的子模块，可直接 `use super::*;` 访问私有项。
//!
//! # 覆盖
//!
//! - `check_ws_origin`：合法同源 / 端口不匹配 / 外部域名 / https / 无
//!   Origin / 大小写不敏感 / 带 path 拒 / 垃圾串拒。
//! - `check_mutating_request`：合法 loopback Origin + JSON 通过 / 外部
//!   Origin 拒 / 错端口拒 / 无 Origin 默认拒 / 无 Origin dev flag 放行
//!   / Content-Type 各分支 / 无 body 宽容。
//! - `is_truthy_env_value` / `extract_origin_and_ct` / `is_mutating_route` /
//!   错误体契约。

use tiny_http::{Header, Method};

use super::security::{
    MutatingCheckError, SecurityContext, check_mutating_request, check_ws_origin,
    extract_origin_and_ct, is_mutating_route, is_truthy_env_value, mutating_check_error_response,
};

// ============================================================================
// check_ws_origin
// ============================================================================

#[test]
fn ws_origin_loopback_ipv4_same_port_passes() {
    assert!(check_ws_origin(Some("http://127.0.0.1:18080"), 18080));
}

#[test]
fn ws_origin_loopback_localhost_same_port_passes() {
    assert!(check_ws_origin(Some("http://localhost:18080"), 18080));
}

#[test]
fn ws_origin_different_port_rejected() {
    assert!(!check_ws_origin(Some("http://127.0.0.1:9999"), 18080));
    assert!(!check_ws_origin(Some("http://localhost:9999"), 18080));
}

#[test]
fn ws_origin_external_domain_rejected() {
    assert!(!check_ws_origin(Some("http://evil.com:18080"), 18080));
    assert!(!check_ws_origin(
        Some("http://127.0.0.1.evil.com:18080"),
        18080
    ));
}

#[test]
fn ws_origin_https_rejected() {
    // v1 仅 http loopback；https 视为不匹配。
    assert!(!check_ws_origin(Some("https://127.0.0.1:18080"), 18080));
}

#[test]
fn ws_origin_no_origin_rejected_regardless_of_dev_flag() {
    // WS 路径不开 allow_no_origin 门——无 Origin 一律拒。
    assert!(!check_ws_origin(None, 18080));
}

#[test]
fn ws_origin_case_insensitive_scheme() {
    assert!(check_ws_origin(Some("HTTP://127.0.0.1:18080"), 18080));
    assert!(check_ws_origin(Some("Http://Localhost:18080"), 18080));
}

#[test]
fn ws_origin_with_path_or_query_rejected() {
    // Origin 头不应带 path；带视为异常。
    assert!(!check_ws_origin(Some("http://127.0.0.1:18080/"), 18080));
    assert!(!check_ws_origin(Some("http://127.0.0.1:18080/foo"), 18080));
}

#[test]
fn ws_origin_garbage_string_rejected() {
    assert!(!check_ws_origin(Some("not a url"), 18080));
    assert!(!check_ws_origin(Some(""), 18080));
}

// ============================================================================
// check_mutating_request: Origin
// ============================================================================

#[test]
fn mutating_request_loopback_origin_passes() {
    let ctx = SecurityContext::new(18080, false);
    assert_eq!(
        check_mutating_request(
            Some("http://127.0.0.1:18080"),
            Some("application/json"),
            &ctx,
            true,
        ),
        Ok(())
    );
    assert_eq!(
        check_mutating_request(
            Some("http://localhost:18080"),
            Some("application/json"),
            &ctx,
            true,
        ),
        Ok(())
    );
}

#[test]
fn mutating_request_external_origin_rejected() {
    let ctx = SecurityContext::new(18080, false);
    let r = check_mutating_request(
        Some("http://evil.com:18080"),
        Some("application/json"),
        &ctx,
        true,
    );
    assert!(matches!(r, Err(MutatingCheckError::BadOrigin { .. })));
}

#[test]
fn mutating_request_wrong_port_origin_rejected() {
    let ctx = SecurityContext::new(18080, false);
    let r = check_mutating_request(
        Some("http://127.0.0.1:9999"),
        Some("application/json"),
        &ctx,
        true,
    );
    assert!(matches!(r, Err(MutatingCheckError::BadOrigin { .. })));
}

#[test]
fn mutating_request_no_origin_rejected_by_default() {
    let ctx = SecurityContext::new(18080, false);
    let r = check_mutating_request(None, Some("application/json"), &ctx, true);
    assert_eq!(r, Err(MutatingCheckError::NoOrigin));
}

#[test]
fn mutating_request_no_origin_allowed_when_dev_flag_on() {
    let ctx = SecurityContext::new(18080, true);
    assert_eq!(
        check_mutating_request(None, Some("application/json"), &ctx, true),
        Ok(())
    );
}

// ============================================================================
// check_mutating_request: Content-Type
// ============================================================================

#[test]
fn mutating_request_application_json_passes() {
    let ctx = SecurityContext::new(18080, false);
    assert_eq!(
        check_mutating_request(
            Some("http://127.0.0.1:18080"),
            Some("application/json"),
            &ctx,
            true,
        ),
        Ok(())
    );
}

#[test]
fn mutating_request_application_json_with_charset_passes() {
    let ctx = SecurityContext::new(18080, false);
    assert_eq!(
        check_mutating_request(
            Some("http://127.0.0.1:18080"),
            Some("application/json; charset=utf-8"),
            &ctx,
            true,
        ),
        Ok(())
    );
}

#[test]
fn mutating_request_case_insensitive_json() {
    let ctx = SecurityContext::new(18080, false);
    assert_eq!(
        check_mutating_request(
            Some("http://127.0.0.1:18080"),
            Some("Application/JSON"),
            &ctx,
            true,
        ),
        Ok(())
    );
}

#[test]
fn mutating_request_text_plain_rejected() {
    let ctx = SecurityContext::new(18080, false);
    let r = check_mutating_request(
        Some("http://127.0.0.1:18080"),
        Some("text/plain"),
        &ctx,
        true,
    );
    assert!(matches!(r, Err(MutatingCheckError::BadContentType { .. })));
}

#[test]
fn mutating_request_form_urlencoded_rejected() {
    let ctx = SecurityContext::new(18080, false);
    let r = check_mutating_request(
        Some("http://127.0.0.1:18080"),
        Some("application/x-www-form-urlencoded"),
        &ctx,
        true,
    );
    assert!(matches!(r, Err(MutatingCheckError::BadContentType { .. })));
}

#[test]
fn mutating_request_multipart_rejected() {
    let ctx = SecurityContext::new(18080, false);
    let r = check_mutating_request(
        Some("http://127.0.0.1:18080"),
        Some("multipart/form-data; boundary=----abc"),
        &ctx,
        true,
    );
    assert!(matches!(r, Err(MutatingCheckError::BadContentType { .. })));
}

#[test]
fn mutating_request_no_content_type_with_body_rejected() {
    let ctx = SecurityContext::new(18080, false);
    let r = check_mutating_request(Some("http://127.0.0.1:18080"), None, &ctx, true);
    assert!(matches!(r, Err(MutatingCheckError::BadContentType { .. })));
}

#[test]
fn mutating_request_no_content_type_no_body_lenient() {
    // /chat/stop 等无 body mutating：宽容。
    let ctx = SecurityContext::new(18080, false);
    assert_eq!(
        check_mutating_request(Some("http://127.0.0.1:18080"), None, &ctx, false),
        Ok(())
    );
}

#[test]
fn mutating_request_text_plain_no_body_lenient() {
    // 无 body 时 CT 非法也宽容（前端 fetch 即便发 text/plain + body=""）
    let ctx = SecurityContext::new(18080, false);
    assert_eq!(
        check_mutating_request(
            Some("http://127.0.0.1:18080"),
            Some("text/plain"),
            &ctx,
            false,
        ),
        Ok(())
    );
}

// ============================================================================
// is_truthy_env_value
// ============================================================================

#[test]
fn env_value_truthy_recognizes_valid_forms() {
    assert!(is_truthy_env_value("1"));
    assert!(is_truthy_env_value("true"));
    assert!(is_truthy_env_value("TRUE"));
    assert!(is_truthy_env_value("Yes"));
    assert!(is_truthy_env_value(" yes \n"));
}

#[test]
fn env_value_truthy_rejects_others() {
    assert!(!is_truthy_env_value(""));
    assert!(!is_truthy_env_value("0"));
    assert!(!is_truthy_env_value("no"));
    assert!(!is_truthy_env_value("off"));
    assert!(!is_truthy_env_value("2"));
    assert!(!is_truthy_env_value("enabled"));
}

// ============================================================================
// extract_origin_and_ct
// ============================================================================

#[test]
fn extract_origin_and_ct_finds_both() {
    let headers = vec![
        Header::from_bytes(&b"Host"[..], &b"127.0.0.1:18080"[..]).unwrap(),
        Header::from_bytes(&b"Origin"[..], &b"http://127.0.0.1:18080"[..]).unwrap(),
        Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
    ];
    let (o, c) = extract_origin_and_ct(&headers);
    assert_eq!(o.as_deref(), Some("http://127.0.0.1:18080"));
    assert_eq!(c.as_deref(), Some("application/json"));
}

#[test]
fn extract_origin_and_ct_handles_missing() {
    let headers = vec![Header::from_bytes(&b"Host"[..], &b"127.0.0.1"[..]).unwrap()];
    let (o, c) = extract_origin_and_ct(&headers);
    assert!(o.is_none());
    assert!(c.is_none());
}

#[test]
fn extract_origin_and_ct_case_insensitive() {
    let headers = vec![
        Header::from_bytes(&b"origin"[..], &b"http://127.0.0.1:18080"[..]).unwrap(),
        Header::from_bytes(&b"content-type"[..], &b"application/json"[..]).unwrap(),
    ];
    let (o, c) = extract_origin_and_ct(&headers);
    assert!(o.is_some());
    assert!(c.is_some());
}

// ============================================================================
// is_mutating_route
// ============================================================================

#[test]
fn mutating_route_classification() {
    use super::RouteId;
    // 恒 mutating（任意 method 都 true；dispatch 入口已先做 method 校验）。
    assert!(is_mutating_route(RouteId::SettingsPatch, &Method::Patch));
    assert!(is_mutating_route(RouteId::SettingsTestLlm, &Method::Post));
    assert!(is_mutating_route(RouteId::SettingsTestTts, &Method::Post));
    assert!(is_mutating_route(RouteId::ChatPost, &Method::Post));
    assert!(is_mutating_route(RouteId::ChatStop, &Method::Post));
    // 恒只读。
    assert!(!is_mutating_route(RouteId::AppCapabilities, &Method::Get));
    assert!(!is_mutating_route(RouteId::AppStatus, &Method::Get));
    assert!(!is_mutating_route(RouteId::SettingsGet, &Method::Get));
    assert!(!is_mutating_route(RouteId::LogsGet, &Method::Get));
    assert!(!is_mutating_route(RouteId::LogsLevels, &Method::Get));
}

/// Models 路由按 method 区分：GET list/get 只读；POST import/activate、
/// PATCH display、DELETE delete 全部 mutating。
#[test]
fn mutating_route_models_by_method() {
    use super::RouteId;
    // 只读。
    assert!(!is_mutating_route(RouteId::Models, &Method::Get));
    // mutating：import / activate (POST), display (PATCH), delete (DELETE)。
    assert!(is_mutating_route(RouteId::Models, &Method::Post));
    assert!(is_mutating_route(RouteId::Models, &Method::Patch));
    assert!(is_mutating_route(RouteId::Models, &Method::Delete));
}

// ============================================================================
// 错误体 / 错误码契约
// ============================================================================

#[test]
fn mutating_error_code_is_stable() {
    assert_eq!(MutatingCheckError::NoOrigin.as_code(), "origin_required");
    assert_eq!(
        MutatingCheckError::BadOrigin { got: None }.as_code(),
        "origin_denied"
    );
    assert_eq!(
        MutatingCheckError::BadContentType { got: None }.as_code(),
        "content_type_required"
    );
}

#[test]
fn mutating_error_status_is_403() {
    assert_eq!(MutatingCheckError::NoOrigin.as_status(), 403);
    assert_eq!(MutatingCheckError::BadOrigin { got: None }.as_status(), 403);
    assert_eq!(
        MutatingCheckError::BadContentType { got: None }.as_status(),
        403
    );
}

#[test]
fn mutating_error_response_carries_code_and_status() {
    let resp = mutating_check_error_response(&MutatingCheckError::NoOrigin);
    assert_eq!(resp.status_code().0, 403);
    // Content-Type 已设。
    let ct = resp
        .headers()
        .iter()
        .find(|h| h.field.equiv("Content-Type"))
        .map(|h| h.value.as_str().to_string());
    assert_eq!(ct.as_deref(), Some("application/json; charset=utf-8"));
}

// 端到端集成测试（`dispatch_with_security` 全链路）已拆到
// `tests_security_e2e.rs` —— 保持本文件 ≤500。
