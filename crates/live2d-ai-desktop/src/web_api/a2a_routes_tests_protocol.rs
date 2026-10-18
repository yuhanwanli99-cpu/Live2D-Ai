//! a2a_routes 协议回归（rc.1）：agent card + message/send。
//!
//! 全部用注入假上游，不碰网络；真实 HTTP + mock 上游在 rc.2。
use std::sync::atomic::Ordering;

use serde_json::json;
use tiny_http::Method;

use super::tests_support::*;
use super::*;

const REPLY: &str = "你好，世界。";

// ---------------------------------------------------------------- agent card

/// 卡片是**请求时生成**的：version 取 Cargo 包版本，url 指向本服务自己的端点。
#[test]
fn agent_card_is_generated_with_cargo_version_and_self_url() {
    let (status, card) = get_card(&strict_ctx());
    assert_eq!(status, 200);
    assert_eq!(card["name"], "Live2D-Ai");
    assert_eq!(card["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(card["protocolVersion"], "0.3.0");
    assert_eq!(card["url"], "http://127.0.0.1:18080/api/v1/a2a");
    // 执行单 §3：supportedInterfaces 也只指向本服务端点。
    assert_eq!(card["supportedInterfaces"][0]["url"], card["url"]);
    assert_eq!(card["supportedInterfaces"][0]["protocolBinding"], "JSONRPC");
    // 流式与推送都是 false（本版不实现）。
    assert_eq!(card["capabilities"]["streaming"], false);
    assert_eq!(card["capabilities"]["pushNotifications"], false);
    assert_eq!(card["defaultInputModes"], json!(["text/plain"]));
    assert_eq!(card["defaultOutputModes"], json!(["text/plain"]));
}

/// 0.3.0 schema 的必填项一个都不能少（agent card 是发现契约）。
#[test]
fn agent_card_has_every_required_field_of_a2a_0_3_0() {
    let (_, card) = get_card(&strict_ctx());
    for key in [
        "protocolVersion",
        "name",
        "description",
        "url",
        "version",
        "capabilities",
        "defaultInputModes",
        "defaultOutputModes",
        "skills",
    ] {
        assert!(
            card.get(key).is_some(),
            "agent card 缺少必填项 {key}: {card}"
        );
    }
    let skills = card["skills"].as_array().expect("skills 数组");
    assert_eq!(skills.len(), 1, "本版只有一个 skill：{card}");
    let skill = &skills[0];
    assert_eq!(skill["id"], "llm-chat");
    assert_eq!(
        skill["description"],
        "把文本交给当前已配置的对话模型，回文本"
    );
    for key in ["id", "name", "description", "tags"] {
        assert!(skill.get(key).is_some(), "skill 缺少必填项 {key}");
    }
}

/// 卡片不泄露配置：不写上游 base_url、不写密钥、不写 .env。
#[test]
fn agent_card_never_contains_secrets_or_upstream_base_url() {
    let (_, card) = get_card(&strict_ctx());
    let text = card.to_string();
    assert!(!text.contains("api_key"), "卡片不得出现 api_key：{text}");
    assert!(!text.contains("sk-"), "卡片不得出现密钥：{text}");
    assert!(!text.contains(".env"), "卡片不得出现 .env：{text}");
    assert!(!text.contains("base_url"), "卡片不得出现 base_url：{text}");
}

/// card 是只读发现端点：非 GET → 405。
#[test]
fn agent_card_rejects_non_get() {
    let method = Method::Post;
    let (status, body) = read_json(handle_a2a(
        &strict_ctx(),
        A2aRequest {
            method: &method,
            path: AGENT_CARD_PATH,
            body: "",
            origin: None,
            content_type: None,
        },
    ));
    assert_eq!(status, 405);
    assert_eq!(body["error"]["code"], "method_not_allowed");
}

// ---------------------------------------------------------------- message/send

/// 一次 message/send：一条 completed Task + 只打一下上游。
#[test]
fn message_send_returns_completed_task_and_hits_upstream_once() {
    let (chat, hits) = counting_chat(REPLY);
    let (status, body) = call(
        &strict_ctx(),
        &rpc_body("message/send", send_params("你好")),
        &chat,
    );
    assert_eq!(status, 200);
    assert_eq!(body["jsonrpc"], "2.0");
    assert_eq!(body["id"], 7);
    let task = &body["result"];
    assert_eq!(task["kind"], "task");
    assert_eq!(task["status"]["state"], "completed");
    assert!(task["id"].as_str().is_some_and(|s| !s.is_empty()));
    assert!(task["contextId"].as_str().is_some_and(|s| !s.is_empty()));
    // 规范生成物位置：artifacts[0].parts[0].text。
    assert_eq!(task["artifacts"][0]["parts"][0]["kind"], "text");
    assert_eq!(task["artifacts"][0]["parts"][0]["text"], REPLY);
    // 执行单要求的「一条 agent 消息、一个 text part」。
    assert_eq!(task["status"]["message"]["kind"], "message");
    assert_eq!(task["status"]["message"]["role"], "agent");
    assert_eq!(task["status"]["message"]["parts"][0]["text"], REPLY);
    assert_eq!(
        task["status"]["message"]["parts"]
            .as_array()
            .expect("parts 数组")
            .len(),
        1
    );
    assert_eq!(hits.load(Ordering::Relaxed), 1, "一次 send 只打一上游");
}

/// A2A 收到的原文 → 原样交给一路 LLM；task/context id 回填到消息里。
#[test]
fn message_send_passes_the_user_text_to_the_single_llm_call() {
    let (chat, seen) = recording_chat();
    let (_, body) = call(
        &strict_ctx(),
        &rpc_body("message/send", send_params("请用一句话回答")),
        &chat,
    );
    let seen = seen.lock().expect("seen lock").clone();
    assert_eq!(seen, vec!["请用一句话回答".to_string()]);
    assert_eq!(
        body["result"]["status"]["message"]["taskId"],
        body["result"]["id"]
    );
    assert_eq!(
        body["result"]["status"]["message"]["contextId"],
        body["result"]["contextId"]
    );
}

/// 多条 text part 用换行拼成一段再送上游（顺序保持）。
#[test]
fn message_send_joins_multiple_text_parts_with_newlines() {
    let (chat, seen) = recording_chat();
    let params = json!({"message": {
        "kind": "message",
        "messageId": "m-1",
        "role": "user",
        "parts": [
            {"kind": "text", "text": "第一行"},
            {"kind": "text", "text": "第二行"},
        ],
    }});
    let (status, _) = call(&strict_ctx(), &rpc_body("message/send", params), &chat);
    assert_eq!(status, 200);
    assert_eq!(
        seen.lock().expect("seen lock").clone(),
        vec!["第一行\n第二行".to_string()]
    );
}

/// 客户端带来的 contextId 原样回填（多轮上下文的锚点）。
#[test]
fn message_send_echoes_client_context_id() {
    let (chat, _) = counting_chat(REPLY);
    let params = json!({"message": {
        "kind": "message",
        "messageId": "m-1",
        "role": "user",
        "contextId": "ctx-client-1",
        "parts": [{"kind": "text", "text": "你好"}],
    }});
    let (_, body) = call(&strict_ctx(), &rpc_body("message/send", params), &chat);
    assert_eq!(body["result"]["contextId"], "ctx-client-1");
}

/// 空文本 / 空白文本 → invalid params（-32602），且**不打上游**。
#[test]
fn message_send_rejects_blank_text_without_calling_upstream() {
    for text in ["", "   ", "\n\t "] {
        let (chat, hits) = counting_chat(REPLY);
        let (status, body) = call(
            &strict_ctx(),
            &rpc_body("message/send", send_params(text)),
            &chat,
        );
        assert_eq!(status, 200);
        assert_eq!(body["error"]["code"], -32602, "text={text:?}");
        assert_eq!(hits.load(Ordering::Relaxed), 0, "空文本不得打上游");
    }
}

/// 只接受 role=user（A2A 的 agent 角色回灌不算用户输入）。
#[test]
fn message_send_rejects_non_user_role() {
    let (chat, hits) = counting_chat(REPLY);
    let params = json!({"message": {
        "kind": "message",
        "messageId": "m-1",
        "role": "agent",
        "parts": [{"kind": "text", "text": "你好"}],
    }});
    let (_, body) = call(&strict_ctx(), &rpc_body("message/send", params), &chat);
    assert_eq!(body["error"]["code"], -32602);
    assert_eq!(hits.load(Ordering::Relaxed), 0);
}

/// parts 只收 kind=text：file / data part 明确 invalid params，不打上游。
#[test]
fn message_send_rejects_file_and_data_parts() {
    for part in [
        json!({"kind": "file", "file": {"uri": "file:///tmp/x"}}),
        json!({"kind": "data", "data": {"a": 1}}),
    ] {
        let (chat, hits) = counting_chat(REPLY);
        let params = json!({"message": {
            "kind": "message",
            "messageId": "m-1",
            "role": "user",
            "parts": [part],
        }});
        let (_, body) = call(&strict_ctx(), &rpc_body("message/send", params), &chat);
        assert_eq!(body["error"]["code"], -32602, "{body}");
        assert_eq!(hits.load(Ordering::Relaxed), 0);
    }
}

/// 上游失败 → JSON-RPC error（-32603），错误描述原样来自已脱敏层。
#[test]
fn message_send_maps_upstream_failure_to_jsonrpc_error() {
    let (chat, hits) = failing_chat("上游对话失败（llm_upstream_401）");
    let (status, body) = call(
        &strict_ctx(),
        &rpc_body("message/send", send_params("你好")),
        &chat,
    );
    assert_eq!(status, 200, "JSON-RPC 错误也是 HTTP 200");
    assert_eq!(body["error"]["code"], -32603);
    let message = body["error"]["message"].as_str().expect("message 字符串");
    assert!(message.contains("llm_upstream_401"), "got: {message}");
    assert_eq!(hits.load(Ordering::Relaxed), 1);
}

// ---------------------------------------------------------------- 范围外 / 非法请求

/// rc.1 未实现的方法 → method not found（-32601），不打上游。
#[test]
fn unknown_method_is_method_not_found() {
    let (chat, hits) = counting_chat(REPLY);
    let (status, body) = call(&strict_ctx(), &rpc_body("foo/bar", json!({})), &chat);
    assert_eq!(status, 200);
    assert_eq!(body["error"]["code"], -32601);
    assert_eq!(hits.load(Ordering::Relaxed), 0);
}

/// 坏 JSON → Parse error（-32700），id = null。
#[test]
fn malformed_json_is_parse_error() {
    let (chat, _) = counting_chat(REPLY);
    let (status, body) = call(&strict_ctx(), "这不是 JSON", &chat);
    assert_eq!(status, 200);
    assert_eq!(body["jsonrpc"], "2.0");
    assert_eq!(body["id"], serde_json::Value::Null);
    assert_eq!(body["error"]["code"], -32700);
}

/// jsonrpc 版本不对 / 缺 method → Invalid Request（-32600），id 原样回显。
#[test]
fn invalid_envelope_is_invalid_request() {
    let (chat, _) = counting_chat(REPLY);
    let (status, body) = call(
        &strict_ctx(),
        r#"{"jsonrpc":"1.0","id":3,"method":"message/send","params":{}}"#,
        &chat,
    );
    assert_eq!(status, 200);
    assert_eq!(body["id"], 3);
    assert_eq!(body["error"]["code"], -32600);

    let (_, body) = call(&strict_ctx(), r#"{"jsonrpc":"2.0","id":4}"#, &chat);
    assert_eq!(body["id"], 4);
    assert_eq!(body["error"]["code"], -32600);
}

// ---------------------------------------------------------------- 安全边界

/// POST 无 Origin + 严格模式 → 403 origin_required（与其它 mutating 路由同款）。
#[test]
fn post_without_origin_is_rejected_in_strict_mode() {
    let (chat, _) = counting_chat(REPLY);
    let (status, body) = call_with(
        &strict_ctx(),
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
        &strict_ctx(),
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
            &strict_ctx(),
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
    let (status, body) = call_with(
        &dev_ctx(),
        Method::Post,
        &rpc_body("message/send", send_params("你好")),
        None,
        Some("application/json"),
        &chat,
    );
    assert_eq!(status, 200);
    assert_eq!(body["result"]["status"]["state"], "completed");
    assert_eq!(hits.load(Ordering::Relaxed), 1);
}

/// JSON-RPC 端点只收 POST（GET → 405），且路由识别函数只认两条 A2A 路径。
#[test]
fn rpc_path_only_accepts_post_and_route_matching_is_exact() {
    let (chat, _) = counting_chat(REPLY);
    let (status, body) = call_with(&strict_ctx(), Method::Get, "", Some(ORIGIN), None, &chat);
    assert_eq!(status, 405);
    assert_eq!(body["error"]["code"], "method_not_allowed");

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

// ---------------------------------------------------------------- e2e：前置路由

/// 真起 run_request_loop（与生产同一段代码），证明 mod.rs 的前置路由真的接上了：
/// GET card 200、POST 范围外方法回 -32601（不打上游）、恶意 Origin 403。
#[test]
fn e2e_pre_route_serves_card_and_jsonrpc() {
    use std::thread;

    use live2d_ai_runtime::AppSettings;

    use crate::web_api::security::SecurityContext;
    use crate::web_api::{ServerContext, run_request_loop};

    let server = tiny_http::Server::http("127.0.0.1:0").expect("bind :0");
    let addr = server.server_addr().to_ip().expect("listen is ip");
    let ctx = ServerContext::new(
        AppSettings::default(),
        "/tmp/live2d-ai-a2a-e2e.toml".into(),
        None,
    )
    .with_security(SecurityContext::new(addr.port(), false));
    thread::spawn(move || run_request_loop(server, ctx));

    let card_resp = raw_http(
        addr,
        "GET /.well-known/agent-card.json HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
    );
    assert!(card_resp.starts_with("HTTP/1.1 200"), "got: {card_resp}");
    assert!(card_resp.contains("\"llm-chat\""), "got: {card_resp}");
    assert!(
        card_resp.contains("\"supportedInterfaces\""),
        "got: {card_resp}"
    );

    let rpc = r#"{"jsonrpc":"2.0","id":9,"method":"foo/bar","params":{}}"#;
    let req = format!(
        "POST /api/v1/a2a HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{rpc}",
        port = addr.port(),
        len = rpc.len(),
    );
    let resp = raw_http(addr, &req);
    assert!(resp.starts_with("HTTP/1.1 200"), "got: {resp}");
    assert!(resp.contains("-32601"), "未知方法应回 -32601：{resp}");

    let evil = format!(
        "POST /api/v1/a2a HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://evil.example\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{rpc}",
        port = addr.port(),
        len = rpc.len(),
    );
    let rejected = raw_http(addr, &evil);
    assert!(rejected.starts_with("HTTP/1.1 403"), "got: {rejected}");
    assert!(rejected.contains("origin_denied"), "got: {rejected}");
}
