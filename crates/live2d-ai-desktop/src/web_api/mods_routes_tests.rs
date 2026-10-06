//! `mods_routes` 场景回归（自内联 `mod tests` 拆出，2026-10-06：本文件原 1009 行，
//! 触到 code-stats 的 `>1000` 棘轮）。脚手架见 `tests_support`。

use super::tests_support::*;
use super::*;
use crate::web_api::security::SecurityContext;
use std::io::Read;

/// M2：`GET /api/v1/mods` 带出 `settings_spec` + `config`，且 secret 字段脱敏。
#[test]
fn mods_list_includes_spec_and_redacts_secret() {
    let ctx = ctx_with_mod(serde_json::json!({
        "mods": {"specmod": {"enabled": true, "config": {"on": true, "token": "SECRET", "path": "/x"}}}
    }));
    let resp = handle_mods_route(&ctx, &Method::Get, "/api/v1/mods", "", None, None).unwrap();
    assert_eq!(resp.status_code(), StatusCode(200));
    let body = body_string(resp);
    assert!(body.contains("\"settings_spec\""), "应带 schema: {body}");
    assert!(
        body.contains("\"kind\":\"string\""),
        "字段种类应可见: {body}"
    );
    assert!(body.contains("\"kind\":\"bool\""), "字段种类应可见: {body}");
    assert!(
        body.contains("\"secret\":true"),
        "secret 标记应在 schema 里: {body}"
    );
    assert!(!body.contains("SECRET"), "secret 字段值不得进 GET: {body}");
    assert!(body.contains("\"/x\""), "非 secret 字段应回值: {body}");
}

/// M2：`GET /api/v1/mods/{id}/config` 只读返回；未知 id → 404；secret 不回值。
#[test]
fn mod_config_get_returns_redacted_config() {
    let ctx = ctx_with_mod(serde_json::json!({
        "mods": {"specmod": {"enabled": true, "config": {"on": true, "token": "SECRET", "path": "/x"}}}
    }));
    let ok = handle_mods_route(
        &ctx,
        &Method::Get,
        "/api/v1/mods/specmod/config",
        "",
        None,
        None,
    )
    .unwrap();
    assert_eq!(ok.status_code(), StatusCode(200));
    let body = body_string(ok);
    assert!(body.contains("\"/x\""), "body = {body}");
    assert!(!body.contains("SECRET"), "secret 不得回值: {body}");

    let missing = handle_mods_route(
        &ctx,
        &Method::Get,
        "/api/v1/mods/nope/config",
        "",
        None,
        None,
    )
    .unwrap();
    assert_eq!(missing.status_code(), StatusCode(404));
}

/// **F-0062-01（路由层）**：POST `…/config` 省略声明的 secret 字段（= 前端从 GET
/// 的**脱敏**配置出发的形状）时**保留**旧密钥。显式清除（空串）见
/// `mod_registry::tests_secret`；端到端「密钥还在 ⇒ 端点仍 401」见
/// `external_routes::tests::mod_config_save_without_secret_keeps_endpoint_auth`。
#[test]
fn config_save_preserves_omitted_secret() {
    let ctx = ctx_with_mod(serde_json::json!({
        "mods": {"specmod": {"enabled": true, "config": {"on": true, "token": "SECRET", "path": "/x"}}}
    }));
    let resp = handle_mods_route(
        &ctx,
        &Method::Post,
        "/api/v1/mods/specmod/config",
        r#"{"config":{"path":"/y"}}"#,
        Some("http://127.0.0.1:18099"),
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(200));
    let reg = ctx.mod_registry.lock().unwrap();
    let cfg = reg.config("specmod").unwrap();
    assert_eq!(
        cfg["token"],
        serde_json::json!("SECRET"),
        "省略 secret 必须保留旧值：{cfg}"
    );
    assert_eq!(cfg["path"], serde_json::json!("/y"));
}

/// 非本族路径 → None。
#[test]
fn non_mods_path_returns_none() {
    let sec = SecurityContext::new(18099, true);
    let ctx = dummy_ctx(sec);
    assert!(
        handle_mods_route(
            &ctx,
            &Method::Get,
            "/api/v1/chat",
            "",
            None,
            Some("application/json"),
        )
        .is_none()
    );
}

/// 空 registry，enable 未知 id → 404。
#[test]
fn enable_unknown_id_returns_404() {
    let sec = SecurityContext::new(18099, true);
    let ctx = dummy_ctx(sec);
    let resp = handle_mods_route(
        &ctx,
        &Method::Post,
        "/api/v1/mods/nope/enable",
        "{}",
        Some("http://127.0.0.1:18099"),
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(404));
}

/// **回归（stabilize）**：无 body 的 enable/disable **不要求 Content-Type**。
///
/// 触发场景（无头浏览器实测）：Flutter `ModsApi.setEnabled` 发的是
/// `fetch(POST)` **不带 body、不带 Content-Type**；旧实现在此处无条件要
/// `application/json` → 恒 415，前端「Mod 管理」的启停整个点不动。
/// 口径与 `security::check_mutating_request` 一致：只有带 body 才校验 CT。
#[test]
fn bodyless_enable_disable_skip_content_type_check() {
    let ctx = ctx_with_mod(serde_json::json!({
        "mods": {"specmod": {"enabled": true, "config": {}}}
    }));
    let origin = Some("http://127.0.0.1:18099");
    for action in ["disable", "enable"] {
        let path = format!("/api/v1/mods/specmod/{action}");
        let resp = handle_mods_route(&ctx, &Method::Post, &path, "", origin, None)
            .expect("mods 路由应命中");
        assert_eq!(
            resp.status_code(),
            StatusCode(200),
            "无 body 的 {action} 必须 200（旧实现会 415）"
        );
    }
}

/// 带 body 的 mutating 仍必须 `application/json`（防表单 CSRF 不回退）。
#[test]
fn body_with_non_json_content_type_is_415() {
    let ctx = ctx_with_mod(serde_json::json!({
        "mods": {"specmod": {"enabled": true, "config": {}}}
    }));
    let resp = handle_mods_route(
        &ctx,
        &Method::Post,
        "/api/v1/mods/specmod/enable",
        r#"{"config":{"on":true}}"#,
        Some("http://127.0.0.1:18099"),
        Some("text/plain"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(415));
    assert!(body_string(resp).contains("unsupported_media_type"));
}

/// 无 body + 缺 Origin + 不允许无 Origin → 403（**不是** 415）：
/// Origin 才是闸门，错误面要给对，工具客户端才知道去开 allow_no_origin。
#[test]
fn bodyless_without_origin_is_403_not_415() {
    let sec = SecurityContext::new(18099, false);
    let ctx = dummy_ctx(sec);
    let resp = handle_mods_route(
        &ctx,
        &Method::Post,
        "/api/v1/mods/specmod/enable",
        "",
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        resp.status_code(),
        StatusCode(403),
        "缺 Origin 应回 403（origin_required），不能是 415"
    );
}

/// GET /api/v1/mods 在空 registry → 200 + `{"mods":[]}`。
#[test]
fn get_mods_list_empty() {
    let sec = SecurityContext::new(18099, true);
    let ctx = dummy_ctx(sec);
    let resp = handle_mods_route(&ctx, &Method::Get, "/api/v1/mods", "", None, None).unwrap();
    assert_eq!(resp.status_code(), StatusCode(200));
    let mut reader = resp.into_reader();
    let mut s = String::new();
    let _ = reader.read_to_string(&mut s);
    assert!(s.contains("\"mods\""), "body = {s}");
    assert!(s.contains("[]"), "body = {s}");
}

/// POST /api/v1/mods/{id}/config 路由匹配测试。
/// - 空 registry 时 reload_config 返回 Other → 404。
#[test]
fn config_route_match_unknown_id_returns_404() {
    let sec = SecurityContext::new(18099, true);
    let ctx = dummy_ctx(sec);
    let resp = handle_mods_route(
        &ctx,
        &Method::Post,
        "/api/v1/mods/nope/config",
        r#"{"config":{"a":1}}"#,
        Some("http://127.0.0.1:18099"),
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(404));
}

/// POST /api/v1/mods/{id}/config：无效 JSON → 400。
#[test]
fn config_route_bad_json_returns_400() {
    let sec = SecurityContext::new(18099, true);
    let ctx = dummy_ctx(sec);
    let resp = handle_mods_route(
        &ctx,
        &Method::Post,
        "/api/v1/mods/nope/config",
        r#"not json"#,
        Some("http://127.0.0.1:18099"),
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(400));
}

/// POST /api/v1/mods/{id}/config：缺少 config 字段 → 400。
#[test]
fn config_route_missing_config_returns_400() {
    let sec = SecurityContext::new(18099, true);
    let ctx = dummy_ctx(sec);
    let resp = handle_mods_route(
        &ctx,
        &Method::Post,
        "/api/v1/mods/nope/config",
        r#"{"other":"value"}"#,
        Some("http://127.0.0.1:18099"),
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(400));
}

/// 启用中的 Mod：200 + `{id, enabled, state}`。
#[test]
fn state_route_returns_runtime_snapshot() {
    let ctx = ctx_with_state_mod(true);
    let resp = handle_mods_route(
        &ctx,
        &Method::Get,
        "/api/v1/mods/stateful/state",
        "",
        None,
        None,
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(200));
    let body = body_string(resp);
    assert!(body.contains("\"id\":\"stateful\""), "{body}");
    assert!(body.contains("\"enabled\":true"), "{body}");
    assert!(body.contains("\"counter\":7"), "应带出运行态: {body}");
}

/// **404 vs 503 的分界**：不在注册表 = 404；在册但没在跑 = 503。
#[test]
fn state_route_distinguishes_unknown_from_unavailable() {
    let ctx = ctx_with_state_mod(true);
    let unknown = handle_mods_route(
        &ctx,
        &Method::Get,
        "/api/v1/mods/nope/state",
        "",
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        unknown.status_code(),
        StatusCode(404),
        "不存在的 Mod 必须 404（前端显示「没有这个 Mod」）"
    );
    let body = body_string(unknown);
    assert!(body.contains("not_found"), "{body}");

    // 同一个 Mod，停用 → 有 runtime 槽位为空 → 503。
    let mut registry = ctx.mod_registry.lock().unwrap();
    registry.disable("stateful").unwrap();
    drop(registry);
    let unavailable = handle_mods_route(
        &ctx,
        &Method::Get,
        "/api/v1/mods/stateful/state",
        "",
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        unavailable.status_code(),
        StatusCode(503),
        "在册但取不到快照必须 503（前端显示「暂时读不到」）"
    );
    assert!(body_string(unavailable).contains("state_unavailable"));
}

/// 在册、在跑、但**没实现** `state_json` → 也是 503（不是 500）。
#[test]
fn state_route_without_state_json_is_503() {
    let ctx = ctx_with_mod(serde_json::json!({"mods": {"specmod": {"enabled": true}}}));
    let resp = handle_mods_route(
        &ctx,
        &Method::Get,
        "/api/v1/mods/specmod/state",
        "",
        None,
        None,
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(503));
    assert!(body_string(resp).contains("state_unavailable"));
}

/// **F-0006-03 回归**：`mod_registry` 锁被毒化后，Mod 路由不得 panic。
///
/// 生产路径上那次 panic 落在 run_request_loop 的 accept 线程上（无
/// catch_unwind）⇒ 一次 Mod 侧 panic 会永久打死 HTTP 面。这里把**同一把**
/// 锁真的毒化，再打列表路由：把 lock_registry 改回 .expect(...) 就变红。
#[test]
fn poisoned_registry_lock_degrades_instead_of_panicking() {
    let ctx = ctx_with_mod(serde_json::json!({}));
    let reg = std::sync::Arc::clone(&ctx.mod_registry);
    let _ = std::thread::spawn(move || {
        let _guard = reg.lock().expect("fresh lock");
        panic!("故意 panic：毒化 mod_registry");
    })
    .join();
    assert!(ctx.mod_registry.is_poisoned(), "前提：这把锁真的被毒化了");

    let resp = handle_mods_list(&ctx);
    assert!(
        resp.status_code().0 == 200,
        "毒化后仍应给出 200 快照；panic 或 5xx 都说明降级失效"
    );
}
