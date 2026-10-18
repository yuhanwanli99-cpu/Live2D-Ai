//! A2A（Agent2Agent）对外接口：agent card + JSON-RPC 2.0 端点（0.3.8 rc.1）。
//!
//! - GET /.well-known/agent-card.json → 发现文档（请求时生成：version 取 Cargo 包版本；
//!   url 只指向本服务 POST /api/v1/a2a；不写上游 base_url / 密钥 / .env）；
//! - POST /api/v1/a2a → JSON-RPC 2.0，本 rc 只收 message/send（tasks/get 与范围外方法
//!   的拒绝在 rc.2，见执行单 §3）。
//!
//! 边界（执行单 §1）：不认识 supervisor / say / 句子装配 / TTS / 口型 / action_cue，也
//! 不往皮套回合请求体加 tools；思考（ReasoningDelta）不进正文。
//! 协议：A2A 0.3.0 的 JSON-RPC 绑定（kind / status.state / parts）；卡片另带执行单要求
//! 的 supportedInterfaces（这是 1.0 字段，版本冲突与取舍见 rc.3 发布说明）。
//! 安全：POST 与其它 mutating 路由同一套 CT + 同源 Origin；响应/日志/卡片不含密钥，
//! 上游失败只回稳定错误码、不回上游 body。

use std::io::Cursor;

use serde_json::{Value, json};
use tiny_http::{Header, Method, Response, StatusCode};

use crate::web_api::ServerContext;
use crate::web_api::security::{
    MutatingCheckError, is_allowed_origin, is_json_content_type, mutating_check_error_response,
};

/// 发现文档路径（A2A 0.3.0 起；**不是**旧的 /.well-known/agent.json）。
pub const AGENT_CARD_PATH: &str = "/.well-known/agent-card.json";

/// JSON-RPC 端点路径。
pub const A2A_PATH: &str = "/api/v1/a2a";

/// 本实现声明的 A2A 协议版本（JSON-RPC 绑定）。
const PROTOCOL_VERSION: &str = "0.3.0";

/// 唯一的 skill id（执行单 §3）。
const SKILL_ID: &str = "llm-chat";

// ---------------------------------------------------------------- JSON-RPC 错误码

const JSONRPC_PARSE_ERROR: i64 = -32700;
const JSONRPC_INVALID_REQUEST: i64 = -32600;
const JSONRPC_METHOD_NOT_FOUND: i64 = -32601;
const JSONRPC_INVALID_PARAMS: i64 = -32602;
const JSONRPC_INTERNAL_ERROR: i64 = -32603;

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
type ChatFn<'a> = dyn Fn(&ServerContext, &str) -> Result<String, String> + 'a;

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
    handle_a2a_with(ctx, req, &llm_chat_upstream)
}

/// handle_a2a 的实现：把「怎么打上游」抽成 ChatFn 注入点（测试注入计数假上游，
/// 证明一次 message/send **只打一下 LLM**；生产路径传 llm_chat_upstream）。
pub fn handle_a2a_with(
    ctx: &ServerContext,
    req: A2aRequest<'_>,
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

    let response = match parse_rpc_request(body) {
        Ok(rpc) => dispatch_rpc(ctx, rpc, chat),
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

/// 解析后的 JSON-RPC 请求（只保留本实现用到的三个字段）。
struct RpcRequest {
    id: Value,
    method: String,
    params: Value,
}

/// 解析 JSON-RPC 2.0 请求；失败返回可直接序列化的错误对象。
fn parse_rpc_request(body: &str) -> Result<RpcRequest, Value> {
    let value: Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(_) => {
            return Err(rpc_error(Value::Null, JSONRPC_PARSE_ERROR, "Parse error"));
        }
    };
    let Some(obj) = value.as_object() else {
        return Err(rpc_error(
            Value::Null,
            JSONRPC_INVALID_REQUEST,
            "Invalid Request: 请求体必须是 JSON 对象",
        ));
    };
    let id = obj.get("id").cloned().unwrap_or(Value::Null);
    if !matches!(id, Value::Null | Value::String(_) | Value::Number(_)) {
        return Err(rpc_error(
            Value::Null,
            JSONRPC_INVALID_REQUEST,
            "Invalid Request: id 必须是字符串 / 数字 / null",
        ));
    }
    if obj.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(rpc_error(
            id,
            JSONRPC_INVALID_REQUEST,
            "Invalid Request: jsonrpc 必须为 2.0",
        ));
    }
    let Some(method) = obj.get("method").and_then(Value::as_str) else {
        return Err(rpc_error(
            id,
            JSONRPC_INVALID_REQUEST,
            "Invalid Request: 缺少 method",
        ));
    };
    Ok(RpcRequest {
        id,
        method: method.to_string(),
        params: obj.get("params").cloned().unwrap_or(Value::Null),
    })
}

/// 方法派发（rc.1 只收 message/send；rc.2 补 tasks/get 与范围外方法拒绝）。
fn dispatch_rpc(ctx: &ServerContext, rpc: RpcRequest, chat: &ChatFn<'_>) -> Value {
    match rpc.method.as_str() {
        "message/send" => handle_message_send(ctx, rpc.id, &rpc.params, chat),
        other => rpc_error(
            rpc.id,
            JSONRPC_METHOD_NOT_FOUND,
            &format!("Method not found: {other}"),
        ),
    }
}

/// message/send：一条 role=user 的 text 消息 → 上游 → completed Task。
fn handle_message_send(ctx: &ServerContext, id: Value, params: &Value, chat: &ChatFn<'_>) -> Value {
    let text = match extract_user_text(params) {
        Ok(text) => text,
        Err(message) => return rpc_error(id, JSONRPC_INVALID_PARAMS, &message),
    };
    let context_id = params
        .get("message")
        .and_then(|m| m.get("contextId"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map_or_else(|| new_id("ctx"), str::to_string);
    let task_id = new_id("task");

    let reply = match chat(ctx, &text) {
        Ok(reply) => reply,
        Err(message) => {
            // 失败描述已在 llm_chat_upstream 里脱敏（无 key / 无上游 body）。
            tracing::warn!(
                target: "a2a",
                code = "a2a_upstream_failed",
                task_id = %task_id,
                "A2A message/send 上游失败：{message}"
            );
            return rpc_error(id, JSONRPC_INTERNAL_ERROR, &message);
        }
    };

    tracing::info!(
        target: "a2a",
        task_id = %task_id,
        input_chars = text.chars().count(),
        output_chars = reply.chars().count(),
        "A2A message/send 完成"
    );
    rpc_success(id, completed_task(&task_id, &context_id, &reply))
}

/// 从 params 里抽一条 user 文本消息（多段 text 用换行拼成一段）。本版只收
/// kind = text；file / data part 明确 invalid params，不静默丢内容、也不打上游。
fn extract_user_text(params: &Value) -> Result<String, String> {
    let message = params
        .get("message")
        .ok_or_else(|| "Invalid params: 缺少 message".to_string())?;
    let role = message
        .get("role")
        .and_then(Value::as_str)
        .ok_or_else(|| "Invalid params: message.role 缺失".to_string())?;
    if role != "user" {
        return Err("Invalid params: 只接受 role=user 的消息".to_string());
    }
    let parts = message
        .get("parts")
        .and_then(Value::as_array)
        .ok_or_else(|| "Invalid params: message.parts 缺失".to_string())?;
    if parts.is_empty() {
        return Err("Invalid params: message.parts 不得为空".to_string());
    }
    let mut texts: Vec<&str> = Vec::with_capacity(parts.len());
    for part in parts {
        match part.get("kind").and_then(Value::as_str) {
            Some("text") => {
                let text = part
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "Invalid params: text part 缺少 text 字段".to_string())?;
                texts.push(text);
            }
            Some(other) => {
                return Err(format!(
                    "Invalid params: 不支持的 part kind {other:?}（本版只收 kind=text）"
                ));
            }
            None => return Err("Invalid params: part 缺少 kind".to_string()),
        }
    }
    let joined = texts.join("\n");
    if joined.trim().is_empty() {
        return Err("Invalid params: 消息文本为空".to_string());
    }
    Ok(joined)
}

/// 构造一个 completed Task：正文同时放进 artifacts[0].parts[0]（A2A 0.3.0
/// 规范的生成物位置）与 status.message（执行单要求的「一条 agent 消息、一个
/// text part」），两处逐字相同（回归断言）。
fn completed_task(task_id: &str, context_id: &str, text: &str) -> Value {
    let message_id = new_id("msg");
    json!({
        "kind": "task",
        "id": task_id,
        "contextId": context_id,
        "status": {
            "state": "completed",
            "message": {
                "kind": "message",
                "role": "agent",
                "messageId": message_id,
                "taskId": task_id,
                "contextId": context_id,
                "parts": [{"kind": "text", "text": text}],
            },
        },
        "artifacts": [{
            "artifactId": new_id("art"),
            "name": "reply",
            "parts": [{"kind": "text", "text": text}],
        }],
    })
}

/// 进程内唯一 id（无 uuid 依赖）：纳秒时戳 + 单调序号 + 前缀。
fn new_id(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(1);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("{prefix}-{nanos:x}-{seq}")
}

// ---------------------------------------------------------------- 上游

/// 生产上游：读当前配置快照 → [llm] → OpenAiClient → 流式收集正文。
fn llm_chat_upstream(ctx: &ServerContext, text: &str) -> Result<String, String> {
    llm_chat_upstream_with(ctx, text, &live2d_ai_runtime::secrets::lookup)
}

/// llm_chat_upstream 的实现：密钥查找抽成注入点（测试不污染进程环境）。
fn llm_chat_upstream_with(
    ctx: &ServerContext,
    text: &str,
    lookup: &dyn Fn(&str) -> Option<String>,
) -> Result<String, String> {
    let settings = ctx.status_ctx.settings_snapshot();
    let llm = llm_config_for(&settings, lookup)?;
    let client =
        live2d_ai_runtime::OpenAiClient::new(llm, live2d_ai_runtime::TtsConfig::new("", "unused"))
            .map_err(|_| "构建 LLM 客户端失败（错误码 llm_client_build）".to_string())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "构建异步运行时失败（错误码 llm_runtime_build）".to_string())?;
    let text = text.to_string();
    runtime.block_on(async move {
        let stream = client
            .chat_stream(&[live2d_ai_runtime::ChatMessage::user(text)])
            .await
            .map_err(|e| sanitize_llm_error(&e))?;
        live2d_ai_runtime::collect_chat_text(stream)
            .await
            .map_err(|e| sanitize_llm_error(&e))
    })
}

/// 从当前 settings 里构造**只有一路 LLM** 的 LlmConfig。
///
/// 不走 AppSettings::resolve_with：那条路会顺带校验 TTS（A2A 不碰 TTS），
/// 用户 TTS 配错不该让 A2A 端点失效。
fn llm_config_for(
    settings: &live2d_ai_runtime::AppSettings,
    lookup: &dyn Fn(&str) -> Option<String>,
) -> Result<live2d_ai_runtime::LlmConfig, String> {
    use live2d_ai_runtime::{ApiSecret, LlmConfig};
    let llm = &settings.llm;
    if llm.base_url.trim().is_empty() {
        return Err("LLM 未配置：[llm].base_url 为空（错误码 llm_unconfigured）".to_string());
    }
    let mut config = LlmConfig::new(llm.base_url.trim(), llm.model.trim());
    config.max_tokens = llm.effective_max_tokens();
    config.thinking = llm.effective_show_reasoning();
    if let Some(name) = llm
        .api_key_env
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        if !live2d_ai_runtime::secrets::is_valid_key(name) {
            return Err(
                "LLM 配置有误：[llm].api_key_env 不是合法环境变量名（错误码 llm_api_key_env_invalid）"
                    .to_string(),
            );
        }
        if let Some(value) = lookup(name) {
            config = config.with_api_key(ApiSecret::new(value));
        }
    }
    Ok(config)
}

/// 上游失败 → **已脱敏**的稳定描述：只带状态码 / 错误码，不带上游 body、不带 key。
fn sanitize_llm_error(error: &live2d_ai_runtime::Error) -> String {
    match error.upstream_status() {
        Some(status) => format!("上游对话失败（llm_upstream_{status}）"),
        None => format!("上游对话失败（llm_{}）", error.code_suffix()),
    }
}

// ---------------------------------------------------------------- 响应 helper

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

/// JSON-RPC 成功响应。
fn rpc_success(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

/// JSON-RPC 错误响应。
fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

// 回归测试拆到同目录 a2a_routes_tests_*.rs（保持本文件 ≤500 行）。
#[cfg(test)]
#[path = "a2a_routes_tests_protocol.rs"]
mod tests_protocol;
#[cfg(test)]
#[path = "a2a_routes_tests_support.rs"]
mod tests_support;
