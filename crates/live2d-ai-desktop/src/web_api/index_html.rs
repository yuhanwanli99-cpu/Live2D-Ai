//! 单页配置面板 + 静态资源（W8 任务）。
//!
//! 责任：
//! - 编译期通过 [`include_str!`] 把 [`INDEX_HTML`] / [`STYLE_CSS`] / [`APP_JS`]
//!   嵌入二进制（**离线可用**——P0 红线：任何模式都自带，不引 CDN/框架）；
//! - [`handle_index`] / [`handle_static`] 由 [`super::dispatch`] 路由；
//!   HTML 走 200 + `text/html; charset=utf-8`，CSS 走 `text/css`、JS 走
//!   `application/javascript`。
//!
//! 安全：
//! - 静态资源路径锁白名单：只服务 [`STATIC_PATHS`] 内置的三条（`/`,
//!   `/index.html`, `/static/style.css`, `/static/app.js`）——**不**按路径
//!   任意读盘（P0-4 兜底：避免路径猜测旁路）。
//! - `Cache-Control: no-cache` 让浏览器每次都拉最新版本（D1 阶段频繁迭代；
//!   上线时可换 immutable）。
//! - **不**回 `Access-Control-Allow-Origin`（同源策略，loopback 单端口）。

/// 编译期嵌入的单页 HTML（≤ 200 行；CSS / JS 拆为外部 `static/*`）。
pub const INDEX_HTML: &str = include_str!("index.html");

/// 编译期嵌入的样式表（≤ 200 行）。
pub const STYLE_CSS: &str = include_str!("style.css");

/// 编译期嵌入的前端脚本（≤ 500 行）。
pub const APP_JS: &str = include_str!("app.js");

/// 编译期嵌入的对话区 + WebSocket 客户端脚本（D5-W2 任务）。
pub const CHAT_JS: &str = include_str!("chat.js");

/// 静态资源 MIME 类型（路由分发时按文件后缀查表）。
///
/// 单一事实来源：避免路由层和测试层各自硬编码不一致。
pub const STYLE_CSS_MIME: &str = "text/css; charset=utf-8";
pub const APP_JS_MIME: &str = "application/javascript; charset=utf-8";
pub const CHAT_JS_MIME: &str = "application/javascript; charset=utf-8";
pub const INDEX_HTML_MIME: &str = "text/html; charset=utf-8";

/// 静态资源白名单（路径 → (内容, MIME)）。
///
/// 顺序与 `match_route` 一致；新增资源需同步扩展 [`super::RouteId`]。
pub fn lookup_static(path: &str) -> Option<(&'static str, &'static str)> {
    match path {
        "/" | "/index.html" => Some((INDEX_HTML, INDEX_HTML_MIME)),
        "/static/style.css" => Some((STYLE_CSS, STYLE_CSS_MIME)),
        "/static/app.js" => Some((APP_JS, APP_JS_MIME)),
        "/static/chat.js" => Some((CHAT_JS, CHAT_JS_MIME)),
        _ => None,
    }
}

/// 构造 HTML/CSS/JS 响应。
///
/// 走 tiny_http 的 `Response::from_data` + `Content-Type` 头。状态码统一 200。
pub fn build_response(
    body: &'static str,
    mime: &'static str,
) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    use tiny_http::{Header, Response};
    Response::from_data(body.as_bytes().to_vec())
        .with_status_code(tiny_http::StatusCode(200))
        .with_header(
            Header::from_bytes(&b"Content-Type"[..], mime.as_bytes()).expect("Content-Type 头常量"),
        )
        .with_header(
            Header::from_bytes(&b"Cache-Control"[..], &b"no-cache"[..])
                .expect("Cache-Control 头常量"),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 模板字符串非空。
    #[test]
    fn index_html_is_non_empty() {
        assert!(!INDEX_HTML.is_empty());
        assert!(INDEX_HTML.len() > 200);
    }

    /// 模板含 doctype 与关键 DOM id（确保我们服务的就是配置面板）。
    #[test]
    fn index_html_contains_critical_anchors() {
        // 文档骨架。
        assert!(INDEX_HTML.starts_with("<!doctype html>"));
        assert!(INDEX_HTML.contains("<title>Live2D-Ai"));
        // 关键字段。
        assert!(INDEX_HTML.contains("id=\"llmBaseUrl\""));
        assert!(INDEX_HTML.contains("id=\"ttsBaseUrl\""));
        assert!(INDEX_HTML.contains("id=\"systemPrompt\""));
        assert!(INDEX_HTML.contains("id=\"devMode\""));
        // 静态资源引用（保证浏览器会再拉 style.css + app.js）。
        assert!(INDEX_HTML.contains("/static/style.css"));
        assert!(INDEX_HTML.contains("/static/app.js"));
        // 密钥语义提示。
        assert!(INDEX_HTML.contains("环境变量名（非密钥）"));
    }

    /// 样式表非空 + 含媒体查询。
    #[test]
    fn style_css_is_non_empty_and_uses_vars() {
        assert!(!STYLE_CSS.is_empty());
        assert!(STYLE_CSS.contains(":root"));
        // UI 重做规格 C2：品牌蓝 token（NEKO #40C5F1）取代旧 --accent。
        assert!(STYLE_CSS.contains("--color-main"));
        assert!(STYLE_CSS.contains("@media"));
    }

    /// JS 含 fetch 路径——保证前端真在用 API。
    #[test]
    fn app_js_uses_endpoints() {
        assert!(APP_JS.contains("/api/v1/app/capabilities"));
        assert!(APP_JS.contains("/api/v1/app/status"));
        assert!(APP_JS.contains("/api/v1/settings"));
        assert!(APP_JS.contains("/api/v1/logs"));
        assert!(APP_JS.contains("/api/v1/settings/test/llm"));
        assert!(APP_JS.contains("/api/v1/settings/test/tts"));
        // 关键逻辑：PATCH + clear_api_key + dev_mode 三态。
        assert!(APP_JS.contains("PATCH"));
        assert!(APP_JS.contains("clear_api_key"));
        assert!(APP_JS.contains("dev_mode"));
        // 「已保存并热重载」用户反馈。
        assert!(APP_JS.contains("已保存并热重载"));
        // v2.5 迁移锚点：设置 rail/分区切换 + 动作网格委托 + actFire。
        assert!(APP_JS.contains("SETTINGS_SECTIONS"));
        assert!(APP_JS.contains("setActiveSection"));
        assert!(APP_JS.contains("actFire"));
        assert!(APP_JS.contains(".act-grid .act-btn[data-act]"));
        assert!(APP_JS.contains("openSettings"));
    }

    /// v2.5 迁移：设置模态 + 遮罩必须可开（overlay class 缺失会致 backdrop 无样式且点击异常）。
    #[test]
    fn index_html_settings_modal_bridge_ok() {
        // 遮罩必须带 .overlay（preview 是 class="overlay" id="ov"，迁移成 id="settings-overlay" 时丢了类）。
        assert!(INDEX_HTML.contains("id=\"settings-overlay\" class=\"overlay\""));
        // 模态必须带 .settings-modal 类（CSS 是类选择器；只有 id 会导致整个模态无样式不可见）。
        assert!(INDEX_HTML.contains("id=\"settings-modal\" class=\"settings-modal\""));
        // 7 分区 rail（data-s）——模型管理为 v2.5 接线新增分区。
        assert_eq!(
            INDEX_HTML.matches("data-s=\"").count(),
            7,
            "left rail 应恰有 7 个 data-s 分区项（角色/外观/AI/动作/模型/开发者/Mod）"
        );
        assert!(INDEX_HTML.contains("data-s=\"models\""));
        // 状态栏（statusbar 用 class，无 id）。
        assert!(INDEX_HTML.contains("class=\"statusbar\""));
        // 动作浮窗。
        assert!(INDEX_HTML.contains("id=\"actPanel\""));
        assert!(INDEX_HTML.contains("id=\"btnActions\""));
    }

    /// 设置接线：角色卡姓名/描述 + 互动开关 + 背景图控件必须存在（spec web-ui-settings-wiring-v1）。
    #[test]
    fn index_html_settings_wiring_controls_ok() {
        assert!(INDEX_HTML.contains("id=\"f-p-name\""));
        assert!(INDEX_HTML.contains("id=\"f-p-desc\""));
        assert!(INDEX_HTML.contains("id=\"f-idle\""));
        assert!(INDEX_HTML.contains("id=\"f-click\""));
        assert!(INDEX_HTML.contains("id=\"f-bgimg-btn\""));
        assert!(INDEX_HTML.contains("id=\"f-bgimg-input\""));
        assert!(INDEX_HTML.contains("id=\"f-bgimg-clear\""));
        // app.js 侧接线锚点。
        assert!(APP_JS.contains("personaName"));
        assert!(APP_JS.contains("bindStageInteractions"));
        assert!(APP_JS.contains("idleEnabled"));
        assert!(APP_JS.contains("clickEnabled"));
        assert!(APP_JS.contains("stage-bg"));
        // 行为测试等价覆盖：persona 五字段 loop 化后（persona[k] = v === "" ? null : v），
        // 空输入=null 清除语义等价（webapp_behavior.rs::SCENE_SETTINGS "set/persona-null" 校验）。
        assert!(APP_JS.contains("persona[k] = v === \"\" ? null : v"));
    }

    /// D5-W2 任务：chat.js 含 WebSocket 连接、text_delta 处理、重连逻辑。
    /// 通过字符串锚点确保前端真在消费核心链路。
    #[test]
    fn chat_js_handles_ws_and_reconnect() {
        assert!(!CHAT_JS.is_empty());
        // WebSocket 连接端点。
        assert!(CHAT_JS.contains("/ws/state"));
        assert!(CHAT_JS.contains("new WebSocket("));
        assert!(CHAT_JS.contains("onmessage"));
        // 事件类型 + 收口逻辑。
        assert!(CHAT_JS.contains("turn_state"));
        assert!(CHAT_JS.contains("text_delta"));
        assert!(CHAT_JS.contains("runtime_status"));
        assert!(CHAT_JS.contains("appendAssistantDelta"));
        // 指数退避 + 30s 上限（P1WS-2 对齐契约 §4.5：1s → 2s → 4s → 8s → 30s）。
        assert!(CHAT_JS.contains("scheduleReconnect"));
        assert!(CHAT_JS.contains("wsBackoffMs"));
        assert!(CHAT_JS.contains("30000"));
        // P1WS-2 新增：subscribe_ack 处理 + shutdown_ready 不重连。
        assert!(CHAT_JS.contains("subscribe_ack"));
        assert!(CHAT_JS.contains("noReconnect"));
        // 发送/停止端点。
        assert!(CHAT_JS.contains("/api/v1/chat"));
        assert!(CHAT_JS.contains("/api/v1/chat/stop"));
        // 状态码分支：429 busy / 503 no_supervisor。
        assert!(CHAT_JS.contains("429"));
        assert!(CHAT_JS.contains("503"));
        // 关键 DOM 锚点（防止重构改名时悄悄破坏；与 HTML id 完全一致）。
        assert!(CHAT_JS.contains("chat-input"));
        assert!(CHAT_JS.contains("chat-send"));
        assert!(CHAT_JS.contains("chat-stop"));
        assert!(CHAT_JS.contains("chat-messages"));
    }

    /// P1WS-1：chat.js 真实 LLM 流式 text 追加路径（`payload.text` 守卫）。
    /// 前端在收到 `{type:"text_delta", data:{epoch, text, ts_ms?}}` 时
    /// **必须**把 `text` 追加到当前 assistant 气泡 body——这是闭环关键。
    #[test]
    fn chat_js_appends_text_delta_text_payload() {
        // appendAssistantDelta 是 text 追加函数（重构改名时本测试即 fail）。
        assert!(CHAT_JS.contains("function appendAssistantDelta"));
        // text_delta 分支必须读 `payload.text`（`typeof === "string"` 守卫）。
        assert!(CHAT_JS.contains("payload.text"));
        // 流式累加方式：textContent += text（不破坏既有内容）。
        assert!(CHAT_JS.contains("body.textContent += text"));
        // 兜底：text_delta 先于 turn_state 到达时开空气泡承接。
        assert!(CHAT_JS.contains("beginAssistantBubble(null)"));
    }

    /// D5-W2 任务：index.html 含对话区 + Live2D 展示区占位。
    #[test]
    fn index_html_contains_chat_panel_anchors() {
        // 对话区核心 DOM id（kebab-case；与 chat.js / 验收 grep "chat-input" 一致）。
        assert!(INDEX_HTML.contains("id=\"chat-input\""));
        assert!(INDEX_HTML.contains("id=\"chat-send\""));
        assert!(INDEX_HTML.contains("id=\"chat-stop\""));
        assert!(INDEX_HTML.contains("id=\"chat-messages\""));
        assert!(INDEX_HTML.contains("id=\"chat-status\""));
        // Live2D 展示区占位。
        assert!(INDEX_HTML.contains("id=\"live2d-display\""));
        // 加载 chat.js（顺序在 app.js 之后无关紧要，但存在性必须）。
        assert!(INDEX_HTML.contains("/static/chat.js"));
    }

    /// lookup_static 路径分发。
    #[test]
    fn lookup_static_table_is_exact() {
        assert!(lookup_static("/").is_some());
        assert!(lookup_static("/index.html").is_some());
        assert!(lookup_static("/static/style.css").is_some());
        assert!(lookup_static("/static/app.js").is_some());
        assert!(lookup_static("/static/chat.js").is_some());
        // 路径猜测旁路兜底。
        assert!(lookup_static("/../etc/passwd").is_none());
        assert!(lookup_static("/static/../etc/passwd").is_none());
        assert!(lookup_static("/static/").is_none());
        assert!(lookup_static("/index").is_none());
        assert!(lookup_static("/static/chat.js.map").is_none());
    }

    /// MIME 与资源正确配对。
    #[test]
    fn mime_is_correct_for_each_resource() {
        assert_eq!(lookup_static("/").unwrap().1, INDEX_HTML_MIME);
        assert_eq!(lookup_static("/index.html").unwrap().1, INDEX_HTML_MIME);
        assert_eq!(
            lookup_static("/static/style.css").unwrap().1,
            STYLE_CSS_MIME
        );
        assert_eq!(lookup_static("/static/app.js").unwrap().1, APP_JS_MIME);
        assert_eq!(lookup_static("/static/chat.js").unwrap().1, CHAT_JS_MIME);
    }

    /// build_response 返回 200 + 正确头。
    #[test]
    fn build_response_200_with_mime_and_no_cache() {
        let resp = build_response(INDEX_HTML, INDEX_HTML_MIME);
        assert_eq!(resp.status_code().0, 200);
        let headers = resp.headers();
        let ct = headers
            .iter()
            .find(|h| h.field.equiv("Content-Type"))
            .expect("Content-Type 头");
        assert_eq!(ct.value.as_str(), INDEX_HTML_MIME);
        let cc = headers
            .iter()
            .find(|h| h.field.equiv("Cache-Control"))
            .expect("Cache-Control 头");
        assert_eq!(cc.value.as_str(), "no-cache");
    }
}
