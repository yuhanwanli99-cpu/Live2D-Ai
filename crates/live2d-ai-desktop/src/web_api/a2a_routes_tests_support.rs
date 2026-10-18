//! a2a_routes 测试共享脚手架（0.3.8）。
//!
//! 只给同目录 a2a_routes_tests_*.rs 用：最小 ServerContext、请求构造、假上游、
//! 本机 mock LLM HTTP、响应读取。pub(super) 只为兄弟测试模块可见，不进产品 API。
#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use live2d_ai_runtime::AppSettings;
use live2d_ai_runtime::settings::LlmMode;
use serde_json::{Value, json};
use tiny_http::{Method, Response};

use super::a2a_tasks::TaskStore;
use super::*;
use crate::web_api::ServerContext;
use crate::web_api::security::SecurityContext;

pub(super) const PORT: u16 = 18080;
pub(super) const ORIGIN: &str = "http://127.0.0.1:18080";

/// 严格模式（无 Origin 拒绝）的最小上下文；端口固定 18080。
pub(super) fn strict_ctx() -> ServerContext {
    ctx_with_settings(AppSettings::default())
}

/// 工具客户端模式（allow_no_origin=true）。
pub(super) fn dev_ctx() -> ServerContext {
    ctx_with_settings_and_security(AppSettings::default(), SecurityContext::new(PORT, true))
}

/// 用给定 settings 造严格模式上下文。
pub(super) fn ctx_with_settings(settings: AppSettings) -> ServerContext {
    ctx_with_settings_and_security(settings, SecurityContext::new(PORT, false))
}

fn ctx_with_settings_and_security(
    settings: AppSettings,
    security: SecurityContext,
) -> ServerContext {
    ServerContext::new(settings, "/tmp/live2d-ai-a2a-test.toml".into(), None)
        .with_security(security)
}

/// 配置了 [llm] 的上下文（mock HTTP / 真实上游回归用）。
pub(super) fn ctx_with_llm(
    base_url: &str,
    api_key_env: Option<&str>,
    mode: LlmMode,
) -> ServerContext {
    let mut settings = AppSettings::default();
    settings.llm.base_url = base_url.to_string();
    settings.llm.model = "mock-model".to_string();
    settings.llm.api_key_env = api_key_env.map(str::to_string);
    settings.llm.mode = mode;
    ctx_with_settings(settings)
}

/// 一次测试用的空任务表（任务表注入点；tasks/get 回归要跨两次调用共享）。
pub(super) fn store() -> Mutex<TaskStore> {
    Mutex::new(TaskStore::new(32))
}

/// 读一条响应：状态码 + JSON body（非 JSON 回 Value::Null）。
pub(super) fn read_json(resp: Response<std::io::Cursor<Vec<u8>>>) -> (u16, Value) {
    let status = resp.status_code().0;
    let mut reader = resp.into_reader();
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).expect("读响应体");
    let value = serde_json::from_slice(&buf).unwrap_or(Value::Null);
    (status, value)
}

/// GET agent card。
pub(super) fn get_card(ctx: &ServerContext) -> (u16, Value) {
    let method = Method::Get;
    let resp = handle_a2a(
        ctx,
        A2aRequest {
            method: &method,
            path: AGENT_CARD_PATH,
            body: "",
            origin: None,
            content_type: None,
        },
    );
    read_json(resp)
}

/// 构造 JSON-RPC 请求体（id 固定 7）。
pub(super) fn rpc_body(method: &str, params: Value) -> String {
    json!({"jsonrpc": "2.0", "id": 7, "method": method, "params": params}).to_string()
}

/// 一条最小 user 文本消息的 params。
pub(super) fn send_params(text: &str) -> Value {
    json!({
        "message": {
            "kind": "message",
            "messageId": "client-msg-1",
            "role": "user",
            "parts": [{"kind": "text", "text": text}],
        }
    })
}

/// 以「同源 Origin + application/json」调用 JSON-RPC 端点（内部自建任务表）。
pub(super) fn call(ctx: &ServerContext, body: &str, chat: &ChatFn<'_>) -> (u16, Value) {
    let tasks = store();
    call_in(ctx, &tasks, body, chat)
}

/// 指定任务表的调用（tasks/get 回归：send 与 get 必须共享同一张表）。
pub(super) fn call_in(
    ctx: &ServerContext,
    tasks: &Mutex<TaskStore>,
    body: &str,
    chat: &ChatFn<'_>,
) -> (u16, Value) {
    call_with(
        ctx,
        tasks,
        Method::Post,
        body,
        Some(ORIGIN),
        Some("application/json"),
        chat,
    )
}

/// 指定 method / Origin / Content-Type 的原始调用（安全边界回归用）。
pub(super) fn call_with(
    ctx: &ServerContext,
    tasks: &Mutex<TaskStore>,
    method: Method,
    body: &str,
    origin: Option<&str>,
    content_type: Option<&str>,
    chat: &ChatFn<'_>,
) -> (u16, Value) {
    let resp = handle_a2a_with(
        ctx,
        A2aRequest {
            method: &method,
            path: A2A_PATH,
            body,
            origin,
            content_type,
        },
        tasks,
        chat,
    );
    read_json(resp)
}

/// 注入点假上游：回复固定文本 + 调用计数。
pub(super) fn counting_chat(
    reply: &'static str,
) -> (
    impl Fn(&ServerContext, &str) -> Result<String, String>,
    Arc<AtomicUsize>,
) {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_for_chat = Arc::clone(&hits);
    let chat = move |_ctx: &ServerContext, _text: &str| -> Result<String, String> {
        hits_for_chat.fetch_add(1, Ordering::Relaxed);
        Ok(reply.to_string())
    };
    (chat, hits)
}

/// 注入点假上游：记录收到的文本（证明 A2A 把请求原文交给了 LLM）。
#[allow(clippy::type_complexity)]
pub(super) fn recording_chat() -> (
    impl Fn(&ServerContext, &str) -> Result<String, String>,
    Arc<Mutex<Vec<String>>>,
) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_for_chat = Arc::clone(&seen);
    let chat = move |_ctx: &ServerContext, text: &str| -> Result<String, String> {
        seen_for_chat
            .lock()
            .expect("seen lock")
            .push(text.to_string());
        Ok("收到。".to_string())
    };
    (chat, seen)
}

/// 注入点假上游：恒失败（错误描述按生产路径的脱敏口径）。
pub(super) fn failing_chat(
    message: &'static str,
) -> (
    impl Fn(&ServerContext, &str) -> Result<String, String>,
    Arc<AtomicUsize>,
) {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_for_chat = Arc::clone(&hits);
    let chat = move |_ctx: &ServerContext, _text: &str| -> Result<String, String> {
        hits_for_chat.fetch_add(1, Ordering::Relaxed);
        Err(message.to_string())
    };
    (chat, hits)
}

// ---------------------------------------------------------------- 本机 mock LLM

/// mock LLM 收下的原始请求（HTTP 头 + body 全文，断言用）。
pub(super) struct MockLlm {
    pub(super) addr: SocketAddr,
    pub(super) requests: Arc<Mutex<Vec<String>>>,
}

impl MockLlm {
    /// OpenAI-compatible base_url（join_endpoint 会拼 /chat/completions）。
    pub(super) fn base_url(&self) -> String {
        format!("http://{}/v1", self.addr)
    }

    /// 收到的第 0 条请求原文。
    pub(super) fn first_request(&self) -> String {
        self.requests
            .lock()
            .expect("mock requests lock")
            .first()
            .cloned()
            .unwrap_or_default()
    }

    /// 收到的请求条数。
    pub(super) fn request_count(&self) -> usize {
        self.requests.lock().expect("mock requests lock").len()
    }
}

enum MockKind {
    /// 正常 SSE 流（text/event-stream）。
    Sse(&'static str),
    /// 延迟一段时间再回 SSE（「HTTP 循环不被 A2A 调用冻住」回归用）。
    SseDelayed(&'static str, Duration),
    /// 非 2xx 状态 + JSON 响应体。
    Status(u16, &'static str),
}

/// 起一个只接**一个**连接的 mock LLM：收请求 → 回预设响应 → 关连接。
pub(super) fn mock_llm_sse(sse: &'static str) -> MockLlm {
    mock_llm(MockKind::Sse(sse))
}

/// 起一个回非 2xx 的 mock LLM（401/500 等失败路径回归）。
pub(super) fn mock_llm_status(status: u16, body: &'static str) -> MockLlm {
    mock_llm(MockKind::Status(status, body))
}

/// 起一个「延迟 delay 后再回 SSE」的 mock LLM（工作线程不冻 HTTP 循环回归用）。
pub(super) fn mock_llm_sse_after(sse: &'static str, delay: Duration) -> MockLlm {
    mock_llm(MockKind::SseDelayed(sse, delay))
}

fn mock_llm(kind: MockKind) -> MockLlm {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock llm");
    let addr = listener.local_addr().expect("mock addr");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let requests_for_thread = Arc::clone(&requests);
    std::thread::spawn(move || {
        let Ok((mut sock, _)) = listener.accept() else {
            return;
        };
        sock.set_read_timeout(Some(Duration::from_secs(5))).ok();
        let request = read_http_request(&mut sock);
        requests_for_thread
            .lock()
            .expect("mock requests lock")
            .push(request);
        let (status, content_type, body) = match kind {
            MockKind::Sse(body) => (200u16, "text/event-stream", body),
            MockKind::SseDelayed(body, delay) => {
                std::thread::sleep(delay);
                (200u16, "text/event-stream", body)
            }
            MockKind::Status(status, body) => (status, "application/json", body),
        };
        let head = format!(
            "HTTP/1.1 {status} OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        );
        let _ = sock.write_all(head.as_bytes());
        let _ = sock.write_all(body.as_bytes());
        let _ = sock.flush();
    });
    MockLlm { addr, requests }
}

/// 读一条完整 HTTP 请求（头 + content-length body）；返回原始文本。
fn read_http_request(sock: &mut TcpStream) -> String {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 2048];
    let header_end = loop {
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
        match sock.read(&mut tmp) {
            Ok(0) | Err(_) => return String::from_utf8_lossy(&buf).into_owned(),
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let content_length = head
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.trim()
                .eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())?
        })
        .unwrap_or(0);
    let mut body = buf[header_end..].to_vec();
    while body.len() < content_length {
        match sock.read(&mut tmp) {
            Ok(0) | Err(_) => break,
            Ok(n) => body.extend_from_slice(&tmp[..n]),
        }
    }
    let mut raw = head;
    raw.push_str(&String::from_utf8_lossy(&body));
    raw
}

/// mock SSE 的固定回复：先思考后正文（思考必须被丢弃）。
pub(super) const MOCK_SSE: &str = "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"内心独白\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"你好\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"，世界。\"}}]}\n\ndata: [DONE]\n\n";

/// 最小 HTTP/1.1 客户端：发一段原始请求，读完整响应（e2e 回归用）。
pub(super) fn raw_http(addr: SocketAddr, request: &str) -> String {
    let mut stream = TcpStream::connect(addr).expect("连接测试服务");
    stream.write_all(request.as_bytes()).expect("写请求");
    stream.flush().expect("flush");
    let mut out = Vec::new();
    stream.read_to_end(&mut out).expect("读响应");
    String::from_utf8_lossy(&out).into_owned()
}
