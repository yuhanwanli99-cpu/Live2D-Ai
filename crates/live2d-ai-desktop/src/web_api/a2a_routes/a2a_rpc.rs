//! A2A JSON-RPC 方法（0.3.8）：request 解析 + dispatch + message/send + tasks/get。
//!
//! 本版只收两个方法（执行单 §3）：
//! - message/send：一条 role=user 的 text 消息 → 一路 LLM → completed Task；
//! - tasks/get：按 task id 从进程内任务表取回（上限 32，不落盘）。
//!
//! 其余方法一律回 JSON-RPC 错误、**不打上游**：
//! - message/stream、tasks/cancel、tasks/resubscribe、tasks/list、
//!   agent/getAuthenticatedExtendedCard → -32004 UnsupportedOperation；
//! - tasks/pushNotificationConfig/* → -32003 PushNotificationNotSupported；
//! - 其它未知方法 → -32601 Method not found。

use serde_json::{Value, json};

use crate::web_api::ServerContext;

use super::ChatFn;
use super::a2a_tasks::{TaskStore, lock_tasks};

const JSONRPC_PARSE_ERROR: i64 = -32700;
const JSONRPC_INVALID_REQUEST: i64 = -32600;
const JSONRPC_METHOD_NOT_FOUND: i64 = -32601;
const JSONRPC_INVALID_PARAMS: i64 = -32602;
const JSONRPC_INTERNAL_ERROR: i64 = -32603;
// A2A 专属错误码（spec §8）。
const A2A_TASK_NOT_FOUND: i64 = -32001;
const A2A_PUSH_NOTIFICATION_NOT_SUPPORTED: i64 = -32003;
const A2A_UNSUPPORTED_OPERATION: i64 = -32004;

/// 解析后的 JSON-RPC 请求（只保留本实现用到的三个字段）。
pub(super) struct RpcRequest {
    pub(super) id: Value,
    pub(super) method: String,
    pub(super) params: Value,
}

/// 解析 JSON-RPC 2.0 请求；失败返回可直接序列化的错误对象。
pub(super) fn parse_rpc_request(body: &str) -> Result<RpcRequest, Value> {
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

/// 方法派发。范围外方法在这**一层**就拒绝，因此不会触发任何上游请求。
pub(super) fn dispatch_rpc(
    ctx: &ServerContext,
    tasks: &std::sync::Mutex<TaskStore>,
    rpc: RpcRequest,
    chat: &ChatFn<'_>,
) -> Value {
    let method = rpc.method.as_str();
    match method {
        "message/send" => handle_message_send(ctx, tasks, rpc.id, &rpc.params, chat),
        "tasks/get" => handle_tasks_get(rpc.id, &rpc.params, tasks),
        "message/stream"
        | "tasks/cancel"
        | "tasks/resubscribe"
        | "tasks/list"
        | "agent/getAuthenticatedExtendedCard" => rpc_error(
            rpc.id,
            A2A_UNSUPPORTED_OPERATION,
            &format!(
                "Unsupported operation: {method}（本服务 streaming=false，只收 message/send 与 tasks/get）"
            ),
        ),
        // 推送配置族**整族**不支持（pushNotifications=false）。
        m if m.starts_with("tasks/pushNotificationConfig/") => rpc_error(
            rpc.id,
            A2A_PUSH_NOTIFICATION_NOT_SUPPORTED,
            "Push notifications not supported",
        ),
        other => rpc_error(
            rpc.id,
            JSONRPC_METHOD_NOT_FOUND,
            &format!("Method not found: {other}"),
        ),
    }
}

/// message/send：一条 role=user 的 text 消息 → 上游 → completed Task（并存进任务表）。
fn handle_message_send(
    ctx: &ServerContext,
    tasks: &std::sync::Mutex<TaskStore>,
    id: Value,
    params: &Value,
    chat: &ChatFn<'_>,
) -> Value {
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
            // 失败描述已在 a2a_llm 里脱敏（无 key / 无上游 body）。
            tracing::warn!(
                target: "a2a",
                code = "a2a_upstream_failed",
                task_id = %task_id,
                "A2A message/send 上游失败：{message}"
            );
            return rpc_error(id, JSONRPC_INTERNAL_ERROR, &message);
        }
    };

    let task = completed_task(&task_id, &context_id, &reply);
    lock_tasks(tasks).insert(task_id.clone(), task.clone());
    tracing::info!(
        target: "a2a",
        task_id = %task_id,
        input_chars = text.chars().count(),
        output_chars = reply.chars().count(),
        "A2A message/send 完成"
    );
    rpc_success(id, task)
}

/// tasks/get：按 id 从进程内任务表取回 Task。
fn handle_tasks_get(id: Value, params: &Value, tasks: &std::sync::Mutex<TaskStore>) -> Value {
    let Some(task_id) = params
        .get("id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    else {
        return rpc_error(id, JSONRPC_INVALID_PARAMS, "Invalid params: 缺少 params.id");
    };
    let task = lock_tasks(tasks).get(task_id);
    match task {
        Some(task) => rpc_success(id, task),
        None => rpc_error(id, A2A_TASK_NOT_FOUND, "Task not found"),
    }
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

/// JSON-RPC 成功响应。
pub(super) fn rpc_success(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

/// JSON-RPC 错误响应。
pub(super) fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}
