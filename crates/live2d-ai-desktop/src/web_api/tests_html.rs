//! W8 任务「单页配置面板 + 静态资源」端到端测试。
//!
//! 拆分承载：`mod.rs` 主体已贴 500 上限；本批新增 6 条 W8 测试单独提到本文件，
//! 与既有 `tests_dev_mode.rs` / `tests_reload.rs` 同模式（`mod.rs` 中
//! `mod tests_html;`，仅 `#[cfg(test)]` 下生效，可直接 `use super::*;`）。
//!
//! 覆盖：
//! - `GET /` / `GET /index.html` → 200 + text/html（白名单静态资源）
//! - `GET /static/style.css` → 200 + text/css
//! - `GET /static/app.js` → 200 + application/javascript
//! - 未知静态路径 → 404（路径白名单兜底）
//! - 模板含全部关键 API 端点字符串（编译期 include_str! 兜底）

use std::io::Read;

use tiny_http::{Method, Response};

use super::*;

/// 工具：把 Response body 拉成 String（与既有 `tests_reload` / `tests_dev_mode` 同实现）。
fn body_to_string(resp: Response<std::io::Cursor<Vec<u8>>>) -> String {
    let mut reader = resp.into_reader();
    let mut s = String::new();
    let _ = reader.read_to_string(&mut s);
    s
}

/// `GET /` 返回 200 + `text/html` + body 以 doctype 起始。
#[test]
fn dispatch_index_html_returns_200_with_text_html() {
    let ctx = ServerContext::new(AppSettings::default(), "/tmp/cfg.toml".into(), None);
    let resp = dispatch(&ctx, &Method::Get, "/", "");
    assert_eq!(resp.status_code().0, 200);
    let headers = resp.headers();
    let ct = headers
        .iter()
        .find(|h| h.field.equiv("Content-Type"))
        .expect("Content-Type header");
    assert!(
        ct.value.as_str().starts_with("text/html"),
        "got: {:?}",
        ct.value.as_str()
    );
    let body = body_to_string(resp);
    assert!(body.starts_with("<!doctype html>"));
}

/// `GET /index.html` 同上（301 任务允许同页返回）。
#[test]
fn dispatch_index_html_alias_returns_200() {
    let ctx = ServerContext::new(AppSettings::default(), "/tmp/cfg.toml".into(), None);
    let resp = dispatch(&ctx, &Method::Get, "/index.html", "");
    assert_eq!(resp.status_code().0, 200);
    let body = body_to_string(resp);
    assert!(body.contains("Live2D-Ai"));
}

/// `GET /static/style.css` 返回 200 + text/css。
#[test]
fn dispatch_static_css_returns_text_css() {
    let ctx = ServerContext::new(AppSettings::default(), "/tmp/cfg.toml".into(), None);
    let resp = dispatch(&ctx, &Method::Get, "/static/style.css", "");
    assert_eq!(resp.status_code().0, 200);
    let headers = resp.headers();
    let ct = headers
        .iter()
        .find(|h| h.field.equiv("Content-Type"))
        .expect("Content-Type header");
    assert!(ct.value.as_str().starts_with("text/css"));
}

/// `GET /static/app.js` 返回 200 + application/javascript。
#[test]
fn dispatch_static_js_returns_javascript_mime() {
    let ctx = ServerContext::new(AppSettings::default(), "/tmp/cfg.toml".into(), None);
    let resp = dispatch(&ctx, &Method::Get, "/static/app.js", "");
    assert_eq!(resp.status_code().0, 200);
    let headers = resp.headers();
    let ct = headers
        .iter()
        .find(|h| h.field.equiv("Content-Type"))
        .expect("Content-Type header");
    assert!(ct.value.as_str().starts_with("application/javascript"));
}

/// `GET /static/chat.js` 返回 200 + application/javascript（D5-W2 任务）。
#[test]
fn dispatch_static_chat_js_returns_javascript_mime() {
    let ctx = ServerContext::new(AppSettings::default(), "/tmp/cfg.toml".into(), None);
    let resp = dispatch(&ctx, &Method::Get, "/static/chat.js", "");
    assert_eq!(resp.status_code().0, 200);
    let headers = resp.headers();
    let ct = headers
        .iter()
        .find(|h| h.field.equiv("Content-Type"))
        .expect("Content-Type header");
    assert!(ct.value.as_str().starts_with("application/javascript"));
}

/// 未知静态路径不命中白名单 → 404（路径猜测旁路兜底）。
#[test]
fn dispatch_unknown_static_path_returns_404() {
    let ctx = ServerContext::new(AppSettings::default(), "/tmp/cfg.toml".into(), None);
    let resp = dispatch(&ctx, &Method::Get, "/static/../etc/passwd", "");
    assert_eq!(resp.status_code().0, 404);
}

/// 模板字符串（include_str! 嵌入）含全部关键 API 端点。
#[test]
fn index_html_template_embeds_key_endpoints() {
    // 模板引用了同源静态资源（浏览器会拉 style.css + app.js + chat.js）。
    assert!(index_html::INDEX_HTML.contains("/static/app.js"));
    assert!(index_html::INDEX_HTML.contains("/static/style.css"));
    assert!(index_html::INDEX_HTML.contains("/static/chat.js"));
    // app.js 含全部 5 个 API 端点（W8 任务要求 fetch capabilities/settings）。
    assert!(index_html::APP_JS.contains("/api/v1/app/capabilities"));
    assert!(index_html::APP_JS.contains("/api/v1/app/status"));
    assert!(index_html::APP_JS.contains("/api/v1/settings"));
    assert!(index_html::APP_JS.contains("/api/v1/logs"));
    assert!(index_html::APP_JS.contains("/api/v1/settings/test/llm"));
    assert!(index_html::APP_JS.contains("/api/v1/settings/test/tts"));
    // 「已保存并热重载」用户反馈（PATCH 成功提示）。
    assert!(index_html::APP_JS.contains("已保存并热重载"));
    // dev_mode 三态支持。
    assert!(index_html::APP_JS.contains("dev_mode"));
    // P0-2 清除密钥语义。
    assert!(index_html::APP_JS.contains("clear_api_key"));
}

/// D5-W2 任务：HTML 含对话区 + Live2D 展示区占位的关键 DOM id。
#[test]
fn index_html_template_contains_chat_panel_dom() {
    // 对话区输入/按钮/消息容器（kebab-case 与 chat.js / 验收 grep 一致）。
    assert!(index_html::INDEX_HTML.contains("id=\"chat-input\""));
    assert!(index_html::INDEX_HTML.contains("id=\"chat-send\""));
    assert!(index_html::INDEX_HTML.contains("id=\"chat-stop\""));
    assert!(index_html::INDEX_HTML.contains("id=\"chat-messages\""));
    assert!(index_html::INDEX_HTML.contains("id=\"chat-status\""));
    // 状态徽章（WS 连接状态）。
    assert!(index_html::INDEX_HTML.contains("id=\"wsBadge\""));
    // Live2D 展示区：真实舞台 /render iframe（v2.5 迁移替换了原占位串 "Live2D 展示区"）。
    assert!(index_html::INDEX_HTML.contains("id=\"live2d-display\""));
    assert!(index_html::INDEX_HTML.contains("id=\"live2d-frame\""));
    assert!(index_html::INDEX_HTML.contains("src=\"/render\""));
}

/// 设置线条迁移/接线锁：模型管理与外观控件必须存在于 HTML，且 app.js 确实消费它们。
///
/// 此处锁住的是「前端 DOM ↔ JS 逻辑」的对应关系——此前 app.js 有完整
/// `loadModels/renderModels/activateModel` 与 `bindAppearance`，但 v2.5 迁移
/// 丢了 `modelsList/modelsStatus/btnRefreshModels/appScale/appBgDark` 等元素，
/// 导致模型管理区空转、`bindAppearance` 因 appScale 缺失提前 return 使口型开关失效。
#[test]
fn index_html_template_contains_model_and_appearance_dom() {
    // 模型管理区（rail `data-s="models"` + section `data-p="models"`）。
    assert!(index_html::INDEX_HTML.contains("data-s=\"models\""));
    assert!(index_html::INDEX_HTML.contains("data-p=\"models\""));
    assert!(index_html::INDEX_HTML.contains("id=\"modelsList\""));
    assert!(index_html::INDEX_HTML.contains("id=\"modelsStatus\""));
    assert!(index_html::INDEX_HTML.contains("id=\"btnRefreshModels\""));
    // 外观与舞台（appScale 缩放 / appBgDark 暗色）。
    assert!(index_html::INDEX_HTML.contains("id=\"appScale\""));
    assert!(index_html::INDEX_HTML.contains("id=\"appBgDark\""));
    // app.js 消费上述元素（证明不是死 DOM）。
    assert!(index_html::APP_JS.contains("els.modelsList"));
    assert!(index_html::APP_JS.contains("els.appScale"));
    assert!(index_html::APP_JS.contains("els.appBgDark"));
    // 修复回归锁：`bindAppearance` 不再因缺元素整体提前 return（此前
    // `if (!els.appScale) return;` 会让 appLipSyncToggle 监听绑不上）。
    assert!(index_html::APP_JS.contains("if (els.appScale)"));
    assert!(index_html::APP_JS.contains("if (els.appBgDark)"));
    assert!(index_html::APP_JS.contains("if (els.appLipSyncToggle)"));
}

/// 日志分级 UI（Phase 4：RUST_LOG 分级进 logs/levels UI）。
///
/// 后端 `/api/v1/logs/levels` 已存在并测试（返回 5 级，dev_mode 门控）；
/// 本测试锁住前端消费——`id="logLevel"` 下拉存在，且 app.js 有级别过滤逻辑
/// （`LOG_LEVEL_RANK` / `filterLogLines` / `populateLogLevels`），避免"后端就绪、
/// 前端空白"的空转。
#[test]
fn index_html_template_has_log_level_filter_ui() {
    assert!(index_html::INDEX_HTML.contains("id=\"logLevel\""));
    // 消费点：level 下拉触发重滤 + 分级常驻过滤逻辑。
    assert!(index_html::APP_JS.contains("els.logLevel"));
    assert!(index_html::APP_JS.contains("LOG_LEVEL_RANK"));
    assert!(index_html::APP_JS.contains("filterLogLines"));
    assert!(index_html::APP_JS.contains("populateLogLevels"));
    // 级别下拉监听 change → refreshLogs 重滤。
    assert!(index_html::APP_JS.contains("els.logLevel.addEventListener(\"change\", refreshLogs)"));
}

/// D5-W2 任务：chat.js 含 WebSocket 端点 + 关键事件类型 + 重连逻辑。
#[test]
fn chat_js_contains_ws_event_handlers_and_reconnect() {
    // WS 端点。
    assert!(index_html::CHAT_JS.contains("/ws/state"));
    // 事件类型（来自 web_api::ws::app_event_to_ws_frame）。
    assert!(index_html::CHAT_JS.contains("turn_state"));
    assert!(index_html::CHAT_JS.contains("text_delta"));
    assert!(index_html::CHAT_JS.contains("runtime_status"));
    // WebSocket 原生 API + 重连退避。
    assert!(index_html::CHAT_JS.contains("new WebSocket("));
    assert!(index_html::CHAT_JS.contains("onmessage"));
    assert!(index_html::CHAT_JS.contains("scheduleReconnect"));
    // P1WS-2：30s 上限（契约 §4.5：1s → 2s → 4s → 8s → 30s）。
    assert!(index_html::CHAT_JS.contains("30000"));
    // P1WS-2：subscribe_ack 处理 + shutdown_ready 不重连。
    assert!(index_html::CHAT_JS.contains("subscribe_ack"));
    assert!(index_html::CHAT_JS.contains("noReconnect"));
    // 发送/停止端点。
    assert!(index_html::CHAT_JS.contains("/api/v1/chat"));
    assert!(index_html::CHAT_JS.contains("/api/v1/chat/stop"));
    // HTTP 状态码分支。
    assert!(index_html::CHAT_JS.contains("429"));
    assert!(index_html::CHAT_JS.contains("503"));
}
