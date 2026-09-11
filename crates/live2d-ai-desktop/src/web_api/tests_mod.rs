//! `web_api::mod` 主体路由/派发测试（拆分承载：mod.rs 已贴 500 上限，
//! 现有 `tests` 块整体迁移到本文件以释放主体可扩展空间）。
//!
//! 入口：`web_api::mod tests_mod;`（仅 `#[cfg(test)]` 下生效）——
//! 本文件作为 `web_api` 的子模块，可直接 `use super::*;` 访问私有项。
//!
//! 覆盖（保持与原 `mod.rs` 内联测试一致）：
//! - `route_dispatch_table_is_stable`：10 条稳定 method+path → RouteId 映射
//! - `route_id_as_str_is_stable`：10 个路由 ID 字符串契约
//! - `dispatch_capabilities_returns_200` / `dispatch_status_returns_200_unconfigured`
//!   / `dispatch_settings_get_returns_200`：常规 200 路径
//! - `dispatch_models_returns_501_with_endpoint`：NotImplemented → 501
//! - `dispatch_unknown_path_returns_404`：未知路径 → 404
//! - `dispatch_logs_returns_403_when_dev_mode_off` / `dispatch_logs_levels_returns_403_when_dev_mode_off`：
//!   dev_mode=false 时日志端点 → 403

use std::io::Read;

use tiny_http::{Method, Response};

use super::*;

/// 工具：构造 ServerContext（默认 settings + 临时 cfg 路径）。
fn ctx_with(settings: AppSettings) -> ServerContext {
    ServerContext::new(settings, "/tmp/cfg.toml".into(), None)
}

/// 工具：把 Response body 拉成 String。
fn body_to_string(resp: Response<std::io::Cursor<Vec<u8>>>) -> String {
    let mut reader = resp.into_reader();
    let mut s = String::new();
    let _ = reader.read_to_string(&mut s);
    s
}

/// 全表 method+path → RouteId 映射（含 W2 新增 chat 端点）。
/// **查询串必须先被剥掉**（2026-09-11 修：Flutter 诊断面板从来没拿到过日志）。
///
/// 症状：`GET /api/v1/logs?limit=200` → **501**（`NotImplemented`），
/// 因为 `tiny_http` 的 `request.url()` 带着 `?...`，而路由是精确匹配。
/// Flutter 前端恰好就是那么调的，所以诊断面板的日志区在成品里是坏的。
#[test]
fn strip_query_removes_search_only() {
    assert_eq!(super::strip_query("/api/v1/logs?limit=200"), "/api/v1/logs");
    assert_eq!(
        super::strip_query("/api/v1/logs?level=warn&limit=100"),
        "/api/v1/logs"
    );
    // 没有查询串时**原样返回**（不能把正常路径切坏）。
    assert_eq!(super::strip_query("/api/v1/logs"), "/api/v1/logs");
    assert_eq!(super::strip_query("/"), "/");
    assert_eq!(super::strip_query("/render"), "/render");
    // 空查询串（`/x?`）也剥掉。
    assert_eq!(super::strip_query("/x?"), "/x");
}

/// 剥过之后，那些「前端带查询参数」的端点落回正确路由。
#[test]
fn query_strings_do_not_break_route_matching() {
    for (raw, want) in [
        ("/api/v1/logs?limit=200", RouteId::LogsGet),
        ("/api/v1/logs?level=warn", RouteId::LogsGet),
        ("/api/v1/app/status?x=1", RouteId::AppStatus),
        ("/api/v1/settings?a=b", RouteId::SettingsGet),
    ] {
        assert_eq!(
            match_route(&Method::Get, super::strip_query(raw)),
            want,
            "{raw} 剥掉查询串后应落到 {want:?}"
        );
    }
}

#[test]
fn route_dispatch_table_is_stable() {
    assert_eq!(
        match_route(&Method::Get, "/api/v1/app/capabilities"),
        RouteId::AppCapabilities
    );
    assert_eq!(
        match_route(&Method::Get, "/api/v1/app/status"),
        RouteId::AppStatus
    );
    assert_eq!(
        match_route(&Method::Get, "/api/v1/settings"),
        RouteId::SettingsGet
    );
    assert_eq!(
        match_route(&Method::Patch, "/api/v1/settings"),
        RouteId::SettingsPatch
    );
    assert_eq!(
        match_route(&Method::Post, "/api/v1/settings/test/llm"),
        RouteId::SettingsTestLlm
    );
    assert_eq!(
        match_route(&Method::Post, "/api/v1/settings/test/tts"),
        RouteId::SettingsTestTts
    );
    // W2：HTTP 对话入口（chat / chat/stop）。
    assert_eq!(
        match_route(&Method::Post, "/api/v1/chat"),
        RouteId::ChatPost
    );
    assert_eq!(
        match_route(&Method::Post, "/api/v1/chat/stop"),
        RouteId::ChatStop
    );
    // W3：日志端点（dev_mode 门控在 dispatch 层做；match 仍命中）。
    assert_eq!(match_route(&Method::Get, "/api/v1/logs"), RouteId::LogsGet);
    assert_eq!(
        match_route(&Method::Get, "/api/v1/logs/levels"),
        RouteId::LogsLevels
    );
    // W8 的原生 JS 前端（`/` + `/static/*`）已于 2026-09-11 删除。
    // `/` 现在由 `flutter_app::handle_flutter_app` 在**请求循环里**接管
    // （302 → `/app/`），和 WS 路径一样**不出现在 `match_route` 的结果里**。
    // 所以这里断言的是「dispatch 不认它」——它不该悄悄落回某个旧路由。
    assert_eq!(match_route(&Method::Get, "/"), RouteId::NotFound);
    assert_eq!(match_route(&Method::Get, "/index.html"), RouteId::NotFound);
    assert_eq!(
        match_route(&Method::Get, "/static/app.js"),
        RouteId::NotFound
    );
    // 冻结但未实现。
    // D3（2026-08-28）：`/api/v1/models` 现在走 `RouteId::Models`（统一
    // 由 models_routes::dispatch 派发）；保留旧断言注释便于理解演化。
    assert_eq!(match_route(&Method::Get, "/api/v1/models"), RouteId::Models);
    // 2026-09-11 用户裁决：动作命令端点（`/api/v1/commands*`）已随动作系统
    // 整体删除 → 它现在是「未实现前缀」501，不再是专用路由。
    assert_eq!(
        match_route(&Method::Get, "/api/v1/commands"),
        RouteId::NotImplemented
    );
    assert_eq!(
        match_route(
            &Method::Post,
            "/api/v1/commands/live2d_perform_action_nod/invoke"
        ),
        RouteId::NotImplemented
    );
    // 不存在路径。
    assert_eq!(match_route(&Method::Get, "/nope"), RouteId::NotFound);
    // method 错配。
    assert_eq!(
        match_route(&Method::Post, "/api/v1/settings"),
        RouteId::NotFound
    );
}

/// 路由 ID 字符串契约（错误体 `endpoint` 字段 + 日志依赖）。
#[test]
fn route_id_as_str_is_stable() {
    assert_eq!(RouteId::AppCapabilities.as_str(), "app.capabilities");
    assert_eq!(RouteId::AppStatus.as_str(), "app.status");
    assert_eq!(RouteId::SettingsGet.as_str(), "settings.get");
    assert_eq!(RouteId::SettingsPatch.as_str(), "settings.patch");
    assert_eq!(RouteId::SettingsTestLlm.as_str(), "settings.test_llm");
    assert_eq!(RouteId::SettingsTestTts.as_str(), "settings.test_tts");
    assert_eq!(RouteId::ChatPost.as_str(), "chat.post");
    assert_eq!(RouteId::ChatStop.as_str(), "chat.stop");
    assert_eq!(RouteId::LogsGet.as_str(), "logs.get");
    assert_eq!(RouteId::LogsLevels.as_str(), "logs.levels");
    assert_eq!(RouteId::Models.as_str(), "models");
    assert_eq!(RouteId::NotImplemented.as_str(), "not_implemented");
    assert_eq!(RouteId::NotFound.as_str(), "not_found");
}

#[test]
fn dispatch_capabilities_returns_200() {
    let ctx = ctx_with(AppSettings::default());
    let resp = dispatch(&ctx, &Method::Get, "/api/v1/app/capabilities", "");
    assert_eq!(resp.status_code().0, 200);
}

#[test]
fn dispatch_status_returns_200_unconfigured() {
    let ctx = ctx_with(AppSettings::default());
    let resp = dispatch(&ctx, &Method::Get, "/api/v1/app/status", "");
    assert_eq!(resp.status_code().0, 200);
}

#[test]
fn dispatch_settings_get_returns_200() {
    let ctx = ctx_with(AppSettings::default());
    let resp = dispatch(&ctx, &Method::Get, "/api/v1/settings", "");
    assert_eq!(resp.status_code().0, 200);
}

/// D3（2026-08-28）：`GET /api/v1/models` 现在走 `models_routes::dispatch`。
/// 空 registry → 200 + `{"models":[]}`。
#[test]
fn dispatch_models_list_returns_200_empty() {
    let ctx = ctx_with(AppSettings::default());
    let resp = dispatch(&ctx, &Method::Get, "/api/v1/models", "");
    assert_eq!(resp.status_code().0, 200);
    let body = body_to_string(resp);
    assert!(body.contains("\"models\""), "body = {body}");
    assert!(body.contains("[]"), "body = {body}");
}

#[test]
fn dispatch_unknown_path_returns_404() {
    let ctx = ctx_with(AppSettings::default());
    // 用不存在的路径断言 404。
    let resp = dispatch(&ctx, &Method::Get, "/nope", "");
    assert_eq!(resp.status_code().0, 404);
}

#[test]
fn dispatch_logs_returns_403_when_dev_mode_off() {
    let ctx = ctx_with(AppSettings::default());
    // 默认 dev_mode=false。
    let resp = dispatch(&ctx, &Method::Get, "/api/v1/logs", "");
    assert_eq!(resp.status_code().0, 403);
    let body = body_to_string(resp);
    assert!(body.contains("dev_mode_required"), "got: {body}");
}

#[test]
fn dispatch_logs_levels_returns_403_when_dev_mode_off() {
    let ctx = ctx_with(AppSettings::default());
    let resp = dispatch(&ctx, &Method::Get, "/api/v1/logs/levels", "");
    assert_eq!(resp.status_code().0, 403);
}

/// W2：`POST /api/v1/chat` 无 supervisor → 503。
#[test]
fn dispatch_chat_without_supervisor_returns_503() {
    let ctx = ctx_with(AppSettings::default());
    let resp = dispatch(&ctx, &Method::Post, "/api/v1/chat", r#"{"text":"hi"}"#);
    assert_eq!(resp.status_code().0, 503);
    let body = body_to_string(resp);
    assert!(body.contains("no_supervisor"), "got: {body}");
}

/// W2：`POST /api/v1/chat/stop` 无 supervisor 仍 200（幂等）。
#[test]
fn dispatch_chat_stop_without_supervisor_returns_200() {
    let ctx = ctx_with(AppSettings::default());
    let resp = dispatch(&ctx, &Method::Post, "/api/v1/chat/stop", "");
    assert_eq!(resp.status_code().0, 200);
}

/// W2：`POST /api/v1/chat` body 非法 → 400（无 supervisor 也走 chat handler
/// 的解析阶段）。
#[test]
fn dispatch_chat_invalid_body_returns_400() {
    let ctx = ctx_with(AppSettings::default());
    let resp = dispatch(&ctx, &Method::Post, "/api/v1/chat", "not json");
    assert_eq!(resp.status_code().0, 400);
}

/// W2：`GET /api/v1/chat` → 405（chat 路由 method 校验在 handler 内）。
#[test]
fn dispatch_chat_wrong_method_returns_405() {
    let ctx = ctx_with(AppSettings::default());
    let resp = dispatch(&ctx, &Method::Get, "/api/v1/chat", "");
    assert_eq!(resp.status_code().0, 405);
}
