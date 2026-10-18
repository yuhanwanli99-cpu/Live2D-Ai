//! a2a_routes 上游回归（0.3.8 rc.2）：真实 OpenAiClient + 本机 mock HTTP。
//!
//! 执行单 §3：不要求本机有 Ollama / 真远端 agent；用本机 mock 守协议形状与
//! 密钥边界。真实 OpenAI-compatible 服务标「未验」。
use std::time::{Duration, Instant};

use super::tests_support::*;
use super::*;

const SECRET: &str = "sk-super-secret-value";

/// 一次 message/send，走**生产上游**（真实 reqwest + SSE 解析 + 正文收集）。
fn send_through_real_client(
    ctx: &ServerContext,
    lookup: &dyn Fn(&str) -> Option<String>,
    text: &str,
) -> (u16, serde_json::Value) {
    let chat =
        |ctx: &ServerContext, text: &str| super::a2a_llm::llm_chat_upstream_with(ctx, text, lookup);
    call(ctx, &rpc_body("message/send", send_params(text)), &chat)
}

/// 真实客户端路径：SSE 正文收集 + 思考丢弃 + 请求体无 tools。
#[test]
fn real_client_collects_text_drops_reasoning_and_sends_no_tools() {
    let mock = mock_llm_sse(MOCK_SSE);
    let ctx = ctx_with_llm(
        &mock.base_url(),
        None,
        live2d_ai_runtime::settings::LlmMode::Cloud,
    );
    let no_keys = |_: &str| None::<String>;
    let (status, body) = send_through_real_client(&ctx, &no_keys, "你好");
    assert_eq!(status, 200, "{body}");
    let reply = &body["result"]["artifacts"][0]["parts"][0]["text"];
    assert_eq!(reply, "你好，世界。", "思考不得混进正文：{body}");
    assert_eq!(body["result"]["status"]["state"], "completed");

    let raw = mock.first_request();
    assert!(raw.starts_with("POST /v1/chat/completions"), "got: {raw}");
    assert!(raw.contains("\"stream\":true"), "A2A 上游仍是流式：{raw}");
    assert!(raw.contains("\"model\":\"mock-model\""), "got: {raw}");
    assert!(
        raw.contains("\"thinking\":{\"type\":\"disabled\"}"),
        "缺省关思考必须显式上线：{raw}"
    );
    assert!(
        raw.contains(&format!(
            "\"max_tokens\":{}",
            live2d_ai_runtime::DEFAULT_MAX_TOKENS
        )),
        "输出上限必须来自设置（缺省 DEFAULT_MAX_TOKENS）：{raw}"
    );
    assert!(!raw.contains("\"tools\""), "A2A 请求体不得含 tools：{raw}");
    assert!(
        !raw.to_ascii_lowercase().contains("authorization"),
        "没配密钥时不得发 Authorization：{raw}"
    );
}

/// 上游 401：请求确实带了 Bearer 密钥（只进头），但**响应与错误描述都不回显**
/// 密钥或上游 body（执行单 §3）。
#[test]
fn upstream_401_never_echoes_key_or_upstream_body() {
    let mock = mock_llm_status(
        401,
        "{\"error\":{\"message\":\"invalid api key sk-super-secret-value\"}}",
    );
    let ctx = ctx_with_llm(
        &mock.base_url(),
        Some("A2A_TEST_KEY"),
        live2d_ai_runtime::settings::LlmMode::Cloud,
    );
    let secret_for_lookup = SECRET.to_string();
    let lookup = move |name: &str| {
        (name == "A2A_TEST_KEY")
            .then(|| secret_for_lookup.clone())
            .filter(|value| !value.is_empty())
    };
    let (status, body) = send_through_real_client(&ctx, &lookup, "你好");
    assert_eq!(status, 200, "JSON-RPC 错误也是 HTTP 200");
    assert_eq!(body["error"]["code"], -32603);
    let message = body["error"]["message"].as_str().expect("message");
    assert!(
        message.contains("llm_upstream_401"),
        "只回稳定错误码：{message}"
    );

    let text = body.to_string();
    assert!(!text.contains(SECRET), "响应不得回显密钥：{text}");
    assert!(
        !text.contains("invalid api key"),
        "响应不得回显上游 body：{text}"
    );

    // 密钥只允许进 Authorization 头（且确实发出去了），不进请求体。
    let raw = mock.first_request();
    let lower = raw.to_ascii_lowercase();
    assert!(
        lower.contains(&format!("bearer {}", SECRET.to_ascii_lowercase())),
        "密钥必须只走 Authorization 头：{raw}"
    );
    let body_part = raw
        .split_once("\r\n\r\n")
        .map_or_else(String::new, |(_, body)| body.to_string());
    assert!(!body_part.contains(SECRET), "请求体不得含密钥：{body_part}");
}

/// local 模式失败**不回退 cloud**：只打一次已配置的 local 端点，错误如实上报。
#[test]
fn local_mode_failure_does_not_fall_back_to_cloud() {
    let mock = mock_llm_status(500, "{\"error\":\"local engine down\"}");
    let ctx = ctx_with_llm(
        &mock.base_url(),
        None,
        live2d_ai_runtime::settings::LlmMode::Local,
    );
    let no_keys = |_: &str| None::<String>;
    let (status, body) = send_through_real_client(&ctx, &no_keys, "你好");
    assert_eq!(status, 200);
    assert_eq!(body["error"]["code"], -32603);
    let message = body["error"]["message"].as_str().expect("message");
    assert!(
        message.contains("llm_upstream_500"),
        "local 失败必须如实上报：{message}"
    );
    assert_eq!(
        mock.request_count(),
        1,
        "local 失败不得重试 / 不得回退到另一个端点"
    );
}

// ---------------------------------------------------------------- e2e 工作线程

/// 在飞一次慢 A2A 调用时，HTTP 循环仍然服务其它请求（POST 派生工作线程）。
#[test]
fn slow_a2a_call_does_not_freeze_the_http_loop() {
    use std::sync::mpsc;
    use std::thread;

    use live2d_ai_runtime::AppSettings;

    use crate::web_api::security::SecurityContext;
    use crate::web_api::{ServerContext, run_request_loop};

    let mock = mock_llm_sse_after(MOCK_SSE, Duration::from_millis(1000));
    let mut settings = AppSettings::default();
    settings.llm.base_url = mock.base_url();
    settings.llm.model = "mock-model".to_string();

    let server = tiny_http::Server::http("127.0.0.1:0").expect("bind :0");
    let addr = server.server_addr().to_ip().expect("listen is ip");
    let ctx = ServerContext::new(settings, "/tmp/live2d-ai-a2a-slow.toml".into(), None)
        .with_security(SecurityContext::new(addr.port(), false));
    thread::spawn(move || run_request_loop(server, ctx));

    let body = r#"{"jsonrpc":"2.0","id":1,"method":"message/send","params":{"message":{"kind":"message","messageId":"m-1","role":"user","parts":[{"kind":"text","text":"你好"}]}}}"#;
    let a2a_req = format!(
        "POST /api/v1/a2a HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{body}",
        port = addr.port(),
        len = body.len(),
    );
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let response = raw_http(addr, &a2a_req);
        tx.send(response).expect("回传 A2A 响应");
    });

    // 让 A2A 请求先落到工作线程上，再量一次普通 GET 的耗时。
    thread::sleep(Duration::from_millis(150));
    let started = Instant::now();
    let status_resp = raw_http(
        addr,
        &format!(
            "GET /api/v1/app/status HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
            addr.port()
        ),
    );
    let elapsed = started.elapsed();
    assert!(
        status_resp.starts_with("HTTP/1.1 200"),
        "got: {status_resp}"
    );
    assert!(
        elapsed < Duration::from_millis(700),
        "慢 A2A 调用把 HTTP 循环冻住了：普通 GET 用了 {elapsed:?}（mock 上游要 1000ms）"
    );

    let a2a_resp = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("等 A2A 响应");
    assert!(a2a_resp.starts_with("HTTP/1.1 200"), "got: {a2a_resp}");
    assert!(
        a2a_resp.contains("\"state\":\"completed\""),
        "A2A 应回 completed Task：{a2a_resp}"
    );
}
