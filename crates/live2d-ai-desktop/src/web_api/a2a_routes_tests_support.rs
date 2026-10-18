//! a2a_routes 测试共享脚手架（0.3.8 rc.1）。
//!
//! 只给同目录 a2a_routes_tests_*.rs 用：最小 ServerContext、请求构造、假上游、
//! 响应读取。pub(super) 只为兄弟测试模块可见，不进产品 API。
#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use live2d_ai_runtime::AppSettings;
use serde_json::{Value, json};
use tiny_http::{Method, Response};

use super::*;
use crate::web_api::ServerContext;
use crate::web_api::security::SecurityContext;

pub(super) const PORT: u16 = 18080;
pub(super) const ORIGIN: &str = "http://127.0.0.1:18080";

/// 严格模式（无 Origin 拒绝）的最小上下文；端口固定 18080。
pub(super) fn strict_ctx() -> ServerContext {
    ServerContext::new(
        AppSettings::default(),
        "/tmp/live2d-ai-a2a-test.toml".into(),
        None,
    )
    .with_security(SecurityContext::new(PORT, false))
}

/// 工具客户端模式（allow_no_origin=true）。
pub(super) fn dev_ctx() -> ServerContext {
    ServerContext::new(
        AppSettings::default(),
        "/tmp/live2d-ai-a2a-test.toml".into(),
        None,
    )
    .with_security(SecurityContext::new(PORT, true))
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

/// 以「同源 Origin + application/json」调用 JSON-RPC 端点。
pub(super) fn call(ctx: &ServerContext, body: &str, chat: &ChatFn<'_>) -> (u16, Value) {
    call_with(
        ctx,
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

/// 最小 HTTP/1.1 客户端：发一段原始请求，读完整响应（e2e 回归用）。
pub(super) fn raw_http(addr: std::net::SocketAddr, request: &str) -> String {
    let mut stream = TcpStream::connect(addr).expect("连接测试服务");
    stream.write_all(request.as_bytes()).expect("写请求");
    stream.flush().expect("flush");
    let mut out = Vec::new();
    stream.read_to_end(&mut out).expect("读响应");
    String::from_utf8_lossy(&out).into_owned()
}
