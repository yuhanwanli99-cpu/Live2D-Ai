//! A2A 宿主门回归（0.3.9 rc.3）：停用两路径 / 安全边界 / 405。
//!
//! 协议形状（JSON-RPC 方法表、任务表、card 字段）由第二仓 crate 自己的测试守；
//! 这里守**宿主只留门**这一半：停用则不调用、CT / Origin / 405、协议核真的接上。

use std::sync::atomic::Ordering;

use serde_json::{Value, json};
use tiny_http::Method;

use super::tests_support::*;
use super::*;

const REPLY: &str = "你好，世界。";

// ---------------------------------------------------------------- 停用门禁

/// 停用时**两条路径同一套**：GET card 与 POST /api/v1/a2a 都 403 mod_disabled。
#[test]
fn disabled_mod_rejects_both_paths_with_mod_disabled() {
    let ctx = disabled_ctx();
    let (chat, hits) = counting_chat(REPLY);

    let (card_status, card_body) = get_card(&ctx);
    assert_eq!(card_status, 403, "停用时 card 也必须 403：{card_body}");
    assert_eq!(card_body["error"]["code"], "mod_disabled");

    let (rpc_status, rpc_body) = call(&ctx, &rpc_body("message/send", send_params("你好")), &chat);
    assert_eq!(rpc_status, 403, "停用时 JSON-RPC 必须 403：{rpc_body}");
    assert_eq!(rpc_body["error"]["code"], "mod_disabled");

    assert_eq!(
        hits.load(Ordering::Relaxed),
        0,
        "Mod 停用时不得打上游（协议核一次都不调）"
    );
}

/// 停用是**在册但未启用**：启用后同一上下文即可服务（对照，证明停用门不是恒 403）。
#[test]
fn enabled_mod_serves_the_same_requests() {
    let ctx = enabled_ctx();
    let (status, card) = get_card(&ctx);
    assert_eq!(status, 200, "{card}");
    let (chat, hits) = counting_chat(REPLY);
    let (status, body) = call(&ctx, &rpc_body("message/send", send_params("你好")), &chat);
    assert_eq!(status, 200);
    assert_eq!(body["result"]["status"]["state"], "completed");
    assert_eq!(hits.load(Ordering::Relaxed), 1);
}

// ---------------------------------------------------------------- card 与 405

/// card 请求时生成：version = Cargo 包版本、url 指向本服务端口，且不泄露配置。
#[test]
fn agent_card_is_generated_and_never_leaks_config() {
    let (status, card) = get_card(&enabled_ctx());
    assert_eq!(status, 200);
    assert_eq!(card["name"], "Live2D-Ai");
    assert_eq!(card["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(card["protocolVersion"], "0.3.0");
    assert_eq!(card["url"], "http://127.0.0.1:18080/api/v1/a2a");
    assert_eq!(card["supportedInterfaces"][0]["url"], card["url"]);
    assert_eq!(card["capabilities"]["streaming"], false);
    assert_eq!(card["capabilities"]["pushNotifications"], false);
    assert_eq!(card["defaultInputModes"], json!(["text/plain"]));
    let text = card.to_string();
    assert!(!text.contains("api_key"), "{text}");
    assert!(!text.contains("sk-"), "{text}");
    assert!(!text.contains(".env"), "{text}");
    assert!(!text.contains("base_url"), "{text}");
}

/// card 只读：非 GET → 405（并带 Allow: GET）。
#[test]
fn agent_card_rejects_non_get() {
    for method in [Method::Post, Method::Put] {
        let (status, body) = call_card(&enabled_ctx(), method);
        assert_eq!(status, 405, "{body}");
        assert_eq!(body["error"]["code"], "method_not_allowed");
    }
}

/// JSON-RPC 端点只收 POST：GET → 405。
#[test]
fn rpc_endpoint_rejects_get() {
    let (chat, _) = counting_chat(REPLY);
    let (status, body) = call_with(
        &enabled_ctx(),
        Method::Get,
        "",
        Some(ORIGIN),
        Some("application/json"),
        &chat,
    );
    assert_eq!(status, 405);
    assert_eq!(body["error"]["code"], "method_not_allowed");
}

/// 路由识别只认两条 A2A 路径（精确匹配，无前缀魔术）。
#[test]
fn route_matching_is_exact() {
    assert!(is_a2a_path(AGENT_CARD_PATH));
    assert!(is_a2a_path(A2A_PATH));
    for other in [
        "/",
        "/api/v1/a2a/",
        "/api/v1/chat",
        "/.well-known/agent.json",
    ] {
        assert!(!is_a2a_path(other), "{other} 不是 A2A 路径");
    }
}

// ---------------------------------------------------------------- 安全边界

/// POST 无 Origin + 严格模式 → 403 origin_required（与其它 mutating 路由同款）。
#[test]
fn post_without_origin_is_rejected_in_strict_mode() {
    let (chat, _) = counting_chat(REPLY);
    let (status, body) = call_with(
        &enabled_ctx(),
        Method::Post,
        &rpc_body("message/send", send_params("你好")),
        None,
        Some("application/json"),
        &chat,
    );
    assert_eq!(status, 403);
    assert_eq!(body["error"]["code"], "origin_required");
}

/// 非 loopback 同源 Origin → 403 origin_denied。
#[test]
fn post_with_foreign_origin_is_rejected() {
    let (chat, _) = counting_chat(REPLY);
    let (status, body) = call_with(
        &enabled_ctx(),
        Method::Post,
        &rpc_body("message/send", send_params("你好")),
        Some("http://evil.example:18080"),
        Some("application/json"),
        &chat,
    );
    assert_eq!(status, 403);
    assert_eq!(body["error"]["code"], "origin_denied");
}

/// Content-Type 不是 application/json → 415（防表单 CSRF）。
#[test]
fn post_without_json_content_type_is_rejected() {
    let (chat, _) = counting_chat(REPLY);
    for ct in [None, Some("text/plain")] {
        let (status, body) = call_with(
            &enabled_ctx(),
            Method::Post,
            &rpc_body("message/send", send_params("你好")),
            Some(ORIGIN),
            ct,
            &chat,
        );
        assert_eq!(status, 415, "ct={ct:?}");
        assert_eq!(body["error"]["code"], "unsupported_media_type");
    }
}

/// allow_no_origin=true（工具客户端模式）时无 Origin 也放行。
#[test]
fn post_without_origin_is_allowed_for_tool_clients() {
    let (chat, hits) = counting_chat(REPLY);
    let (status, body) = call(
        &enabled_dev_ctx(),
        &rpc_body("message/send", send_params("你好")),
        &chat,
    );
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["result"]["status"]["state"], "completed");
    assert_eq!(hits.load(Ordering::Relaxed), 1);
}

// ---------------------------------------------------------------- 协议核接线

/// 宿主门把请求原样交给协议核：一次 message/send → completed Task + 只打一下上游。
#[test]
fn gate_wires_message_send_through_the_protocol_core() {
    let (chat, hits) = counting_chat(REPLY);
    let (status, body) = call(
        &enabled_ctx(),
        &rpc_body("message/send", send_params("你好")),
        &chat,
    );
    assert_eq!(status, 200);
    assert_eq!(body["jsonrpc"], "2.0");
    assert_eq!(body["id"], 7);
    let task = &body["result"];
    assert_eq!(task["kind"], "task");
    assert_eq!(task["status"]["state"], "completed");
    // 规范生成物位置与「一条 agent 消息」两处逐字相同。
    assert_eq!(task["artifacts"][0]["parts"][0]["text"], REPLY);
    assert_eq!(task["status"]["message"]["parts"][0]["text"], REPLY);
    assert_eq!(hits.load(Ordering::Relaxed), 1, "一次 send 只打一上游");
}

/// A2A 收到的原文原样交给一路 LLM（宿主闭包只转发文本）。
#[test]
fn gate_passes_the_user_text_to_the_injected_upstream() {
    let (chat, seen) = recording_chat();
    let (_, body) = call(
        &enabled_ctx(),
        &rpc_body("message/send", send_params("请用一句话回答")),
        &chat,
    );
    assert_eq!(
        seen.lock().expect("seen lock").clone(),
        vec!["请用一句话回答".to_string()]
    );
    assert_eq!(body["result"]["contextId"], body["result"]["contextId"]);
}

/// 上游失败 → JSON-RPC -32603，描述按注入的已脱敏文本原样回。
#[test]
fn gate_maps_upstream_failure_to_internal_error() {
    let (chat, hits) = failing_chat("上游对话失败（llm_upstream_401）");
    let (status, body) = call(
        &enabled_ctx(),
        &rpc_body("message/send", send_params("你好")),
        &chat,
    );
    assert_eq!(status, 200, "JSON-RPC 错误也是 HTTP 200");
    assert_eq!(body["error"]["code"], -32603);
    let message = body["error"]["message"].as_str().expect("message 字符串");
    assert!(message.contains("llm_upstream_401"), "got: {message}");
    assert_eq!(hits.load(Ordering::Relaxed), 1);
}

/// 范围外方法由协议核拒绝：宿主门不额外打上游。
#[test]
fn gate_does_not_hit_upstream_for_out_of_scope_methods() {
    let (chat, hits) = counting_chat(REPLY);
    let cases = [("foo/bar", -32601), ("tasks/cancel", -32004)];
    for (method, expected) in cases {
        let (status, body) = call(&enabled_ctx(), &rpc_body(method, json!({})), &chat);
        assert_eq!(status, 200);
        assert_eq!(body["error"]["code"], expected, "{method}: {body}");
    }
    assert_eq!(hits.load(Ordering::Relaxed), 0, "范围外方法不得打上游");
}

/// 坏 JSON → -32700（协议核解析层，宿主门原样翻 HTTP 200）。
#[test]
fn gate_returns_parse_error_for_malformed_json() {
    let (chat, _) = counting_chat(REPLY);
    let (status, body) = call(&enabled_ctx(), "这不是 JSON", &chat);
    assert_eq!(status, 200);
    assert_eq!(body["id"], Value::Null);
    assert_eq!(body["error"]["code"], -32700);
}
