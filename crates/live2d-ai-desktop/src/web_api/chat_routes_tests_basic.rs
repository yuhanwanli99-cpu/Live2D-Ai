//! `chat_routes` 基础路由回归（无 supervisor / method / body 校验 / 200）（自内联 `mod tests` 拆出）。
//!
//! 脚手架见 `tests_support`。

use super::tests_support::*;
use super::*;

/// 无 supervisor 时 chat 返回 503。
#[test]
fn chat_without_supervisor_returns_503() {
    let resp = handle_chat_with_supervisor(
        None,
        None,
        &Method::Post,
        "/api/v1/chat",
        r#"{"text":"hi"}"#,
        0,
    );
    assert_eq!(resp.status_code().0, 503);
    let b = body(resp);
    assert!(b.contains("no_supervisor"), "got: {b}");
}

/// 无 supervisor 时 stop 仍 200（幂等）。
#[test]
fn stop_without_supervisor_returns_200_idempotent() {
    let resp = handle_stop_with_supervisor(None, None, &Method::Post, "/api/v1/chat/stop");
    assert_eq!(resp.status_code().0, 200);
}

/// 错 method → 405。
#[test]
fn chat_wrong_method_returns_405() {
    let resp = handle_chat_with_supervisor(
        None,
        None,
        &Method::Get,
        "/api/v1/chat",
        r#"{"text":"hi"}"#,
        0,
    );
    assert_eq!(resp.status_code().0, 405);
}

/// body 非 JSON → 400。
#[test]
fn chat_invalid_body_returns_400() {
    let resp =
        handle_chat_with_supervisor(None, None, &Method::Post, "/api/v1/chat", "not json", 0);
    assert_eq!(resp.status_code().0, 400);
}

/// 缺 text 字段 → 400。
#[test]
fn chat_missing_text_returns_400() {
    let resp = handle_chat_with_supervisor(None, None, &Method::Post, "/api/v1/chat", r#"{}"#, 0);
    assert_eq!(resp.status_code().0, 400);
}

/// text 空串 → 400。
#[test]
fn chat_empty_text_returns_400() {
    let resp = handle_chat_with_supervisor(
        None,
        None,
        &Method::Post,
        "/api/v1/chat",
        r#"{"text":""}"#,
        0,
    );
    assert_eq!(resp.status_code().0, 400);
}

/// 有 supervisor 时 say 被接受 → 200 + accepted=true。
/// 用真实 supervisor（指向不可达端点，只走"send 成功"路径）。
#[test]
fn chat_with_supervisor_returns_200() {
    // 用 thread id 拼唯一文件名（cargo test 并发安全）。
    use std::hash::{Hash, Hasher};
    let thread_id = format!("{:?}", std::thread::current().id());
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    thread_id.hash(&mut hasher);
    let tmp = std::env::temp_dir().join(format!(
        "live2d_ai_test_chat_routes_{}_{}.toml",
        std::process::id(),
        hasher.finish()
    ));
    let _ = std::fs::remove_file(&tmp);
    std::fs::write(
        &tmp,
        live2d_ai_runtime::AppSettings::default().to_toml_string(),
    )
    .expect("write tmp");
    let client = live2d_ai_runtime::OpenAiClient::new(
        live2d_ai_runtime::LlmConfig::new("http://127.0.0.1:1/v1", "m"),
        live2d_ai_runtime::TtsConfig::new("http://127.0.0.1:2/v1", "alloy"),
    )
    .expect("client");
    let supervisor = std::sync::Arc::new(crate::supervisor::spawn_supervisor(
        crate::supervisor::SupervisorConfig {
            client,
            conversation: live2d_ai_runtime::ConversationConfig::new("人设"),
            capabilities: live2d_ai_core::ModelCapabilities::all(),
            audio: None,
            config_path: Some(tmp.to_string_lossy().into_owned()),
            mod_events: None,
        },
        |_ev| {},
    ));

    let resp = handle_chat_with_supervisor(
        Some(&supervisor),
        None,
        &Method::Post,
        "/api/v1/chat",
        r#"{"text":"hi"}"#,
        // 末位参数 v1 占位；P1WS-1 后被忽略，handler 改用
        // `handle.current_epoch()`（启动瞬间 = 0）。
        42,
    );
    assert_eq!(resp.status_code().0, 200, "first say should accept");
    let b = body(resp);
    assert!(b.contains("\"accepted\":true"), "got: {b}");
    // P1WS-1：epoch 来自 supervisor.current_epoch()；启动瞬间 = 0。
    assert!(b.contains("\"epoch\":0"), "got: {b}");

    // 第二次 say → 通道满（容量 1）→ 429。
    let resp2 = handle_chat_with_supervisor(
        Some(&supervisor),
        None,
        &Method::Post,
        "/api/v1/chat",
        r#"{"text":"hi again"}"#,
        42,
    );
    assert_eq!(
        resp2.status_code().0,
        429,
        "second say while pending should be busy"
    );
    let b2 = body(resp2);
    assert!(b2.contains("busy"), "got: {b2}");

    // stop → 200。
    let resp3 =
        handle_stop_with_supervisor(Some(&supervisor), None, &Method::Post, "/api/v1/chat/stop");
    assert_eq!(resp3.status_code().0, 200);

    // 收尾。
    supervisor.quit();
    let _ = std::fs::remove_file(&tmp);
}
