//! A2A（Agent2Agent）对外接口：agent card + JSON-RPC 2.0 端点（0.3.8）。
//!
//! - GET /.well-known/agent-card.json → 发现文档（请求时生成：version 取 Cargo 包版本；
//!   url 只指向本服务 POST /api/v1/a2a；不写上游 base_url / 密钥 / .env）；
//! - POST /api/v1/a2a → JSON-RPC 2.0，只收 message/send 与 tasks/get（rc.2 起）。
//!
//! 边界（执行单 §1）：不认识 supervisor / say / 句子装配 / TTS / 口型 / action_cue，也
//! 不往皮套回合请求体加 tools；思考（ReasoningDelta）不进正文。
//! 协议：A2A 0.3.0 的 JSON-RPC 绑定（kind / status.state / parts）；卡片另带执行单要求
//! 的 supportedInterfaces（这是 1.0 字段，版本冲突与取舍见 rc.3 发布说明）。
//! 安全：POST 与其它 mutating 路由同一套 CT + 同源 Origin；响应/日志/卡片不含密钥，
//! 上游失败只回稳定错误码、不回上游 body。
//!
//! 文件拆分（每个文件守住 500 行棘轮）：
//! - 本文件：入口 / 安全边界 / agent card / 响应 helper；
//! - a2a_routes/a2a_rpc.rs：JSON-RPC 解析 + dispatch + 两个方法；
//! - a2a_routes/a2a_tasks.rs：进程内任务表（上限 32，不落盘）；
//! - a2a_routes/a2a_llm.rs：一路 [llm] 上游 + 失败脱敏。

use std::io::Cursor;
use std::sync::Mutex;

use serde_json::{Value, json};
use tiny_http::{Header, Method, Response, StatusCode};

use crate::web_api::ServerContext;
use crate::web_api::security::{
    MutatingCheckError, is_allowed_origin, is_json_content_type, mutating_check_error_response,
};

mod a2a_llm;
mod a2a_rpc;
mod a2a_tasks;

use a2a_tasks::{TaskStore, global_tasks};

/// 发现文档路径（A2A 0.3.0 起；**不是**旧的 /.well-known/agent.json）。
pub const AGENT_CARD_PATH: &str = "/.well-known/agent-card.json";

/// JSON-RPC 端点路径。
pub const A2A_PATH: &str = "/api/v1/a2a";

/// 本实现声明的 A2A 协议版本（JSON-RPC 绑定）。
const PROTOCOL_VERSION: &str = "0.3.0";

/// 唯一的 skill id（执行单 §3）。
const SKILL_ID: &str = "llm-chat";

/// true = 这条路径属于本模块（card 或 JSON-RPC 端点）。
///
/// run_request_loop 用它把请求**前置于** dispatch；不命中就交还给后续路由。
pub fn is_a2a_path(path: &str) -> bool {
    path == AGENT_CARD_PATH || path == A2A_PATH
}

/// 一次 A2A 请求的原始字段。
pub struct A2aRequest<'a> {
    /// HTTP 方法。
    pub method: &'a Method,
    /// 已剥查询串的路径（只可能是 AGENT_CARD_PATH / A2A_PATH）。
    pub path: &'a str,
    /// 请求体（GET 时为空串）。
    pub body: &'a str,
    /// Origin 头。
    pub origin: Option<&'a str>,
    /// Content-Type 头。
    pub content_type: Option<&'a str>,
}

/// 上游对话函数：一段文本 → 一段正文（Err 是**已脱敏**的失败描述）。
pub(crate) type ChatFn<'a> = dyn Fn(&ServerContext, &str) -> Result<String, String> + 'a;

/// 在独立工作线程里处理一条 A2A 请求。
///
/// message/send 要等上游 LLM 整段回复；tiny_http 的请求循环是**单线程**的，
/// 在循环里直接等会把 /app/ 与其它 API 一起冻住。GET card 不走这里。
pub fn spawn_a2a<F>(
    ctx: ServerContext,
    method: Method,
    path: String,
    body: String,
    origin: Option<String>,
    content_type: Option<String>,
    respond: F,
) where
    F: FnOnce(Response<Cursor<Vec<u8>>>) + Send + 'static,
{
    std::thread::spawn(move || {
        let req = A2aRequest {
            method: &method,
            path: &path,
            body: &body,
            origin: origin.as_deref(),
            content_type: content_type.as_deref(),
        };
        respond(handle_a2a(&ctx, req));
    });
}

/// 生产入口：mod.rs 前置路由调用（GET card 在循环内直答；POST 派生线程）。
pub fn handle_a2a(ctx: &ServerContext, req: A2aRequest<'_>) -> Response<Cursor<Vec<u8>>> {
    handle_a2a_with(ctx, req, global_tasks(), &a2a_llm::llm_chat_upstream)
}

/// handle_a2a 的实现：任务表与上游都抽成注入点（测试确定性驱动）。
pub(crate) fn handle_a2a_with(
    ctx: &ServerContext,
    req: A2aRequest<'_>,
    tasks: &Mutex<TaskStore>,
    chat: &ChatFn<'_>,
) -> Response<Cursor<Vec<u8>>> {
    let A2aRequest {
        method,
        path,
        body,
        origin,
        content_type,
    } = req;

    // ---- 发现文档：只读 GET，不套 mutating 校验（loopback 绑定即边界）。
    if path == AGENT_CARD_PATH {
        if *method != Method::Get {
            return json_response(
                StatusCode(405),
                &api_error("method_not_allowed", "agent card 只支持 GET"),
            );
        }
        return json_response(StatusCode(200), &agent_card(ctx.security.port));
    }

    // ---- JSON-RPC 端点：与其它 mutating 路由同一套 CT + Origin 校验。
    if *method != Method::Post {
        return json_response(
            StatusCode(405),
            &api_error("method_not_allowed", "POST /api/v1/a2a 仅支持 POST"),
        );
    }
    if !is_json_content_type(content_type) {
        return json_response(
            StatusCode(415),
            &api_error(
                "unsupported_media_type",
                "Content-Type 必须为 application/json",
            ),
        );
    }
    if !is_allowed_origin(origin, &ctx.security) {
        let err = match origin {
            Some(got) => MutatingCheckError::BadOrigin {
                got: Some(got.to_string()),
            },
            None => MutatingCheckError::NoOrigin,
        };
        return mutating_check_error_response(&err);
    }

    let response = match a2a_rpc::parse_rpc_request(body) {
        Ok(rpc) => a2a_rpc::dispatch_rpc(ctx, tasks, rpc, chat),
        Err(error) => error,
    };
    json_response(StatusCode(200), &response)
}

/// 请求时生成的 agent card。
///
/// - version = Cargo 包版本（当前 0.3.1，rc 不改版本串）；
/// - url / supportedInterfaces **只**指向本服务的 POST /api/v1/a2a；
/// - streaming / pushNotifications 都是 false（本版不支持）；
/// - 一个 skill：llm-chat。
fn agent_card(port: u16) -> Value {
    let url = format!("http://127.0.0.1:{port}{A2A_PATH}");
    json!({
        "name": "Live2D-Ai",
        "description": "Live2D-Ai 本地服务：把当前已配置的 OpenAI-compatible 对话模型暴露成 A2A agent（纯文本进、纯文本出）。",
        "version": env!("CARGO_PKG_VERSION"),
        "protocolVersion": PROTOCOL_VERSION,
        "url": url.clone(),
        "supportedInterfaces": [{
            "url": url,
            "protocolBinding": "JSONRPC",
            "protocolVersion": PROTOCOL_VERSION,
        }],
        "capabilities": {
            "streaming": false,
            "pushNotifications": false,
        },
        "defaultInputModes": ["text/plain"],
        "defaultOutputModes": ["text/plain"],
        "skills": [{
            "id": SKILL_ID,
            "name": "LLM chat",
            "description": "把文本交给当前已配置的对话模型，回文本",
            "tags": ["chat", "llm", "text"],
        }],
    })
}

/// 普通 JSON 响应（200 / 405 / 415 等）。
fn json_response(status: StatusCode, value: &Value) -> Response<Cursor<Vec<u8>>> {
    Response::from_data(value.to_string().into_bytes())
        .with_status_code(status)
        .with_header(
            Header::from_bytes(
                &b"Content-Type"[..],
                &b"application/json; charset=utf-8"[..],
            )
            .expect("Content-Type header"),
        )
        .with_header(
            Header::from_bytes(&b"Cache-Control"[..], &b"no-cache"[..])
                .expect("Cache-Control header"),
        )
}

/// 传输层（非 JSON-RPC）错误体：与外部注入端点同款。
fn api_error(code: &str, message: &str) -> Value {
    json!({"error": {"code": code, "message": message}})
}

// 回归测试拆到同目录 a2a_routes_tests_*.rs（保持本文件 ≤500 行）。
#[cfg(test)]
#[path = "a2a_routes_tests_protocol.rs"]
mod tests_protocol;
#[cfg(test)]
#[path = "a2a_routes_tests_support.rs"]
mod tests_support;
#[cfg(test)]
#[path = "a2a_routes_tests_tasks.rs"]
mod tests_tasks;
#[cfg(test)]
#[path = "a2a_routes_tests_upstream.rs"]
mod tests_upstream;
