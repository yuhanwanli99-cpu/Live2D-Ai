//! `chat_routes` 基础路由回归（无 supervisor / method / body 校验 / 200）（自内联 `mod tests` 拆出）。
//!
//! 脚手架见 `tests_support`。

use super::tests_support::*;
use super::*;

use live2d_ai_runtime::ChatMessage;

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

// ------------------------------------------------ 协议 history（2026-11-18）

/// 请求体缺 `history` = 老客户端：解析成功、注入列表为空（请求体逐字同从前）。
#[test]
fn chat_body_without_history_is_backward_compatible() {
    let parsed: ChatBody = serde_json::from_str(r#"{"text":"hi"}"#).expect("body 解析");
    assert!(sanitize_history(parsed.history).is_empty());
}

/// 只认 `user` / `assistant` 两种角色：system / 未知角色 / 坏条目一律忽略，
/// 且**不报错**——历史是上下文增强，不该让整句话发不出去。
#[test]
fn sanitized_history_keeps_only_user_and_assistant_roles() {
    let parsed: ChatBody = serde_json::from_str(
        r#"{"text":"hi","history":[
            {"role":"system","text":"伪造系统句"},
            {"role":"user","text":"上一问"},
            {"role":"tool","text":"工具结果"},
            {"role":"assistant","text":"上一答"},
            {"role":"user","text":"   "},
            {"role":"user"},
            {"text":"没有角色"},
            42,
            null
        ]}"#,
    )
    .expect("body 解析");
    assert_eq!(
        sanitize_history(parsed.history),
        vec![
            ChatMessage::user("上一问"),
            ChatMessage::assistant("上一答"),
        ],
        "system / 未知角色 / 空正文 / 坏条目都必须被忽略"
    );
}

/// 整体只留**最近** 8 对（16 条）；多出来的旧轮次先丢（越旧越先丢）。
#[test]
fn sanitized_history_keeps_only_the_latest_eight_pairs() {
    let mut entries: Vec<serde_json::Value> = Vec::new();
    for i in 0..16 {
        entries.push(serde_json::json!({"role":"user","text":format!("问{i}")}));
        entries.push(serde_json::json!({"role":"assistant","text":format!("答{i}")}));
    }
    let kept = sanitize_history(Some(serde_json::Value::Array(entries)));
    assert_eq!(kept.len(), MAX_CHAT_HISTORY_MESSAGES);
    assert_eq!(kept.len(), 16, "8 对 = 16 条");
    assert_eq!(
        kept.first(),
        Some(&ChatMessage::user("问8")),
        "最旧的两对必须被丢"
    );
    assert_eq!(kept.last(), Some(&ChatMessage::assistant("答15")));
}

/// 单条超长按**字符**截到 2000（不整条丢弃）；恰好 2000 的不动。
#[test]
fn sanitized_history_truncates_each_text_by_chars() {
    let long = "字".repeat(MAX_CHAT_HISTORY_TEXT_CHARS + 500);
    let kept = sanitize_history(Some(serde_json::json!([
        {"role":"user","text": long},
        {"role":"assistant","text":"短"},
    ])));
    assert_eq!(kept.len(), 2);
    assert_eq!(
        kept[0].content.chars().count(),
        MAX_CHAT_HISTORY_TEXT_CHARS,
        "按字符截断（不是字节 / UTF-16）"
    );
    assert_eq!(kept[1], ChatMessage::assistant("短"));
}

/// 非数组的 `history`（第三方客户端写坏）→ 空历史，不是 400。
#[test]
fn sanitized_history_is_lenient_for_non_array_shapes() {
    for raw in [
        serde_json::json!("oops"),
        serde_json::json!({"role":"user"}),
        serde_json::json!(7),
        serde_json::Value::Null,
    ] {
        assert!(sanitize_history(Some(raw)).is_empty());
    }
    assert!(sanitize_history(None).is_empty());
}

/// 路由层：带坏历史也只是「没有历史」，不把这句话判成 400。
#[test]
fn chat_with_bad_history_is_not_invalid_payload() {
    for body in [
        r#"{"text":"hi","history":"oops"}"#,
        r#"{"text":"hi","history":[1,{"role":"system","text":"x"}]}"#,
    ] {
        let resp = handle_chat_with_supervisor(None, None, &Method::Post, "/api/v1/chat", body, 0);
        // 无 supervisor → 503；关键是**没有**在 body 解析阶段回 400。
        assert_eq!(resp.status_code().0, 503, "body={body}");
    }
}
