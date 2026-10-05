//! `chat_routes` 测试共享脚手架（自内联 `mod tests` 拆出）。
//!
//! 仅供同目录 `chat_routes_tests_basic` / `_session` / `_baseline` 使用：
//! 响应体读取、带 supervisor 的 `ServerContext`、broadcaster + 订阅端、
//! 两条会话 baseline 常量。`pub(super)` 只为兄弟测试模块可见性，不进产品 API。

use std::io::Read as _;

use super::*;

pub(super) fn body(resp: Response<Cursor<Vec<u8>>>) -> String {
    let mut s = String::new();
    let _ = resp.into_reader().read_to_string(&mut s);
    s
}

/// 构造一个**带 supervisor** 的 ServerContext（会话表住在 supervisor 里）。
pub(super) fn ctx_with_supervisor() -> (
    crate::web_api::ServerContext,
    std::sync::Arc<SupervisorHandle>,
) {
    let client = live2d_ai_runtime::OpenAiClient::new(
        live2d_ai_runtime::LlmConfig::new("http://127.0.0.1:1/v1", "m"),
        live2d_ai_runtime::TtsConfig::new("http://127.0.0.1:2/v1", "alloy"),
    )
    .expect("client");
    let handle = std::sync::Arc::new(crate::supervisor::spawn_supervisor(
        crate::supervisor::SupervisorConfig {
            client,
            conversation: live2d_ai_runtime::ConversationConfig::new("人设"),
            capabilities: live2d_ai_core::ModelCapabilities::all(),
            audio: None,
            config_path: None,
            mod_events: None,
        },
        |_ev| {},
    ));
    let ctx = crate::web_api::ServerContext::new(
        live2d_ai_runtime::AppSettings::default(),
        "/tmp/l1-session-route.toml".into(),
        None,
    )
    // 真实端口白名单：`ServerContext::new` 出厂 port=0（严格模式），
    // 不显式给端口的话任何 Origin 都会被判 403——那是**测试夹具**的问题，
    // 不是产品行为（生产由 cli_entry 注入真实端口）。
    .with_security(crate::web_api::security::SecurityContext::new(18080, false))
    .with_supervisor(std::sync::Arc::clone(&handle));
    (ctx, handle)
}

/// 建一个 broadcaster + 订阅端，用来读**实际广播的帧原文**。
pub(super) fn broadcaster_with_rx() -> (
    crate::web_api::ws::Broadcaster,
    std::sync::mpsc::Receiver<String>,
) {
    let bc = crate::web_api::ws::Broadcaster::new();
    let (tx, rx) = std::sync::mpsc::sync_channel(256);
    bc.try_subscribe(tx).expect("订阅 broadcaster");
    (bc, rx)
}

/// 会话 A 的 baseline（三字段表达面之一）。
pub(super) const BASELINE_A: &str =
    r#"{"field":"expression","id":"smile","intensity":1,"at":"now","hold":true}"#;
/// 会话 B 的 baseline（另一条，保证 A/B 可区分）。
pub(super) const BASELINE_B: &str =
    r#"{"field":"head","y":-0.4,"intensity":2,"at":"now","hold":true}"#;
