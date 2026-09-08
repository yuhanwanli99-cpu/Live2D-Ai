//! D4 命令注册 HTTP 端点（节点 D §7 落地，2026-08-29）。
//!
//! # 端点
//!
//! - `GET /api/v1/commands`：列出 [`COMMANDS`]（不含 invoke 逻辑）。
//! - `POST /api/v1/commands/{id}/invoke`：触发命令。Body 由
//!   [`command_registry::validate_params`] 校验，**禁止**前端绕过
//!   schema 直接构造 action 参数。
//!
//! # 状态码语义
//!
//! - `200 + {"accepted": true, "epoch": N, "kind": "single",
//!   "steps": 1, "action": "..."}`：invoke 已被 supervisor 受理
//!   （已通过 schema 校验 + 已入 action 通道）。**accepted 不等于
//!   core 仲裁通过**——capability gate / 优先级拒绝由 core 端发
//!   `Dropped` 效果并通过 WS `runtime_status` 事件回报（D4 接受仲裁
//!   结果，前端按状态字段展示）。
//! - `400 invalid_params`：body 非 JSON、缺字段、字段值越界、附加字段
//!   拒绝。`details` 携带 `field` / `value`（便于前端 form 报错定位）。
//! - `404 command_not_found`：id 不在静态白名单。
//! - `405 method_not_allowed`：method 非 GET/POST。
//! - `503 no_supervisor`：web 模式未装配 supervisor（同 chat 路径）。
//!
//! # 严格仲裁（§7.3）
//!
//! 路径：handler 校验 schema → `SupervisorHandle::trigger_action`
//! → supervisor `action_tx` 通道 → supervisor 空闲态 `select!` →
//! `root_apply(RootEvent::Action)` → core 仲裁（capability gate / 优先级
//! / 幂等）→ `forward_effects` → `AppEvent::Render`。
//!
//! 全文**不**触碰 [`live2d_ai_core::apply`]、不构造
//! `AppEvent::Render`、不写 `ActionState::current`——
//! 这与 grep 审计 `forbidden_paths.rs` 同步（v1 §7.3 drift 防御）。
//!
//! # 安全
//!
//! Origin + Content-Type 校验统一在 [`crate::web_api::dispatch::dispatch_with_security`]
//! 入口按 `RouteId::Commands + POST` 自动触发；本文件**不**重复校验。

use std::io::Cursor;
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;
use tiny_http::{Method, Response, StatusCode};

use live2d_ai_core::ActionId;

use crate::supervisor::SupervisorHandle;
use crate::web_api::command_registry::{self, CommandDescriptor, CommandKind, ParsedParams};
use crate::web_api::dto::{ErrorDetail, ErrorResponse};

// ---------------------------------------------------------------- 响应 DTO

/// `GET /api/v1/commands` 响应。
#[derive(Debug, Serialize)]
pub struct CommandsListResponse {
    /// 命令清单（顺序 = `COMMANDS` 顺序）。
    pub commands: Vec<CommandSummary>,
}

/// 命令摘要（不含 invoke 实现）。
#[derive(Debug, Serialize)]
pub struct CommandSummary {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub params_schema: Value,
    pub allowed_sources: &'static [&'static str],
    /// v1 恒为 `"single"`（前端按 `kind` 决定表单形态）。
    pub kind: &'static str,
    /// 基础命令的目标动作名（与 `live2d_ai_core::ActionId::name()` 同构）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_action: Option<&'static str>,
    /// 该命令触发的 `SemanticAction` 数量（v1 Single 恒为 1）。
    pub step_count: usize,
}

/// `POST /api/v1/commands/{id}/invoke` 响应。
#[derive(Debug, Serialize)]
pub struct InvokeAccepted {
    /// 已被 supervisor 接受入队。
    pub accepted: bool,
    /// supervisor 当前 epoch。
    pub epoch: u64,
    /// 触发的命令 id（与请求路径一致）。
    pub command_id: &'static str,
    /// 命令形态（v1 恒为 `single`）。
    pub kind: &'static str,
    /// 实际排队的 `SemanticAction` 数量（v1 恒为 1）。
    pub steps: usize,
    /// 触发的目标 action。
    pub action: &'static str,
}

// ---------------------------------------------------------------- GET 端点

/// `GET /api/v1/commands` handler。
pub fn handle_list_commands(method: &Method, path: &str) -> Response<Cursor<Vec<u8>>> {
    if *method != Method::Get {
        return method_not_allowed(path, "GET");
    }
    let commands = command_registry::COMMANDS
        .iter()
        .map(|c| CommandSummary {
            id: c.id,
            name: c.name,
            description: c.description,
            params_schema: c.params_schema.clone(),
            allowed_sources: c.allowed_sources,
            kind: kind_str(&c.kind),
            target_action: single_target(&c.kind),
            step_count: step_count_of(&c.kind),
        })
        .collect();
    let body = serde_json::to_vec(&CommandsListResponse { commands })
        .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}").into_bytes());
    json_response(StatusCode(200), body)
}

/// 顶层入口（dispatch 调用）：按 method 走 list/invoke。
///
/// **mutating 校验**已上移到 [`dispatch_with_security`]（按
/// `RouteId::Commands + POST` 触发）；本入口只负责 method 路由 + 业务
/// 逻辑，不再重复 `Origin` / `Content-Type` 校验。
pub fn handle_commands_route(
    handle: Option<&std::sync::Arc<SupervisorHandle>>,
    method: &Method,
    path: &str,
    body: &str,
) -> Response<Cursor<Vec<u8>>> {
    // GET 列表：只读，跳过 mutating 校验（dispatch 已按 route 决策）。
    if *method == Method::Get && path == "/api/v1/commands" {
        return handle_list_commands(method, path);
    }
    handle_invoke_command(handle, method, path, body)
}

const fn kind_str(kind: &CommandKind) -> &'static str {
    match kind {
        CommandKind::Single { .. } => "single",
    }
}

/// 该命令触发的 `SemanticAction` 数量（v1 Single 恒为 1）。
const fn step_count_of(kind: &CommandKind) -> usize {
    match kind {
        CommandKind::Single { .. } => 1,
    }
}

// ---------------------------------------------------------------- POST 端点

/// `POST /api/v1/commands/{id}/invoke` handler。
///
/// `path` 必须是 `/api/v1/commands/{id}/invoke` 形式；`{id}` 经
/// `extract_command_id` 解析。`body` 已是完整 JSON 字符串（`""` 也
/// 视作空 `{}`）。
pub fn handle_invoke_command(
    handle: Option<&Arc<SupervisorHandle>>,
    method: &Method,
    path: &str,
    body: &str,
) -> Response<Cursor<Vec<u8>>> {
    if *method != Method::Post {
        return method_not_allowed(path, "POST");
    }
    let cmd_id = match extract_command_id(path) {
        Some(id) => id,
        None => return not_found(path, "command_not_found", "命令 id 缺失或路径非法"),
    };
    let cmd = match command_registry::lookup(cmd_id) {
        Some(c) => c,
        None => return not_found(path, "command_not_found", "命令 id 不在静态白名单"),
    };
    // Body 解析：空字符串 = 空 object（params 缺省）。
    let params_val: Value = if body.trim().is_empty() {
        Value::Object(Default::default())
    } else {
        match serde_json::from_str::<Value>(body) {
            Ok(v) => v,
            Err(e) => return invalid_params(path, format!("请求体不是合法 JSON：{e}"), None, None),
        }
    };
    let parsed = match command_registry::validate_params(cmd, &params_val) {
        Ok(p) => p,
        Err(msg) => return invalid_params(path, msg, None, None),
    };
    let Some(handle) = handle else {
        return no_supervisor(path);
    };
    // 经 supervisor 通道触发；core 仲裁在 supervisor 线程完成。
    let queued = dispatch_to_supervisor(handle, cmd, parsed);
    if !queued {
        return service_unavailable(path);
    }
    let epoch = handle.current_epoch();
    // v1：恒 single，steps 恒 1。
    let ParsedParams::Single { .. } = parsed;
    let action = single_target(&cmd.kind).unwrap_or("unknown");
    let body = serde_json::to_vec(&InvokeAccepted {
        accepted: true,
        epoch,
        command_id: cmd.id,
        kind: kind_str(&cmd.kind),
        steps: 1,
        action,
    })
    .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}").into_bytes());
    json_response(StatusCode(200), body)
}

/// 把已校验的 params 翻译为单个 `SemanticAction` 并经 supervisor 通道触发。
///
/// 单一 `accept_arbiter`（§7.3 grep 审计保证）：**不**直接调
/// `core::apply`，仅通过 `SupervisorHandle::trigger_action` 走核心
/// 仲裁路径。
///
/// 返回 `true` = 已入队一条；`false` = 通道关闭（supervisor 已退）。
fn dispatch_to_supervisor(
    handle: &SupervisorHandle,
    cmd: &CommandDescriptor,
    parsed: ParsedParams,
) -> bool {
    let ParsedParams::Single { strength } = parsed;
    let target = match single_target(&cmd.kind) {
        Some(t) => t,
        None => return false, // 异常：schema 已锁，不可能到这里
    };
    let action_id = match parse_action_id(target) {
        Some(a) => a,
        None => return false,
    };
    let Some(sa) = SupervisorHandle::make_user_action(action_id, strength) else {
        return false; // 异常：validate_params 已保证 1..=3
    };
    handle.trigger_action(sa)
}

fn parse_action_id(name: &str) -> Option<ActionId> {
    Some(match name {
        "nod" => ActionId::Nod,
        "shake_no" => ActionId::ShakeNo,
        "tilt" => ActionId::Tilt,
        "look_around" => ActionId::LookAround,
        "listen" => ActionId::Listen,
        "surprise" => ActionId::Surprise,
        _ => return None,
    })
}

fn single_target(kind: &CommandKind) -> Option<&'static str> {
    match kind {
        CommandKind::Single { action } => Some(*action),
    }
}

/// 从 `/api/v1/commands/{id}/invoke` 提取 `{id}`。
///
/// id 必须是 snake_case（与注册表一致）；非 ASCII 或非 snake 字符
/// → 404（避免路径猜测旁路）。
pub(crate) fn extract_command_id(path: &str) -> Option<&str> {
    const PREFIX: &str = "/api/v1/commands/";
    const SUFFIX: &str = "/invoke";
    let rest = path.strip_prefix(PREFIX)?;
    let id = rest.strip_suffix(SUFFIX)?;
    if id.is_empty() || id.contains('/') {
        return None;
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return None;
    }
    Some(id)
}

// ---------------------------------------------------------------- 响应 helper

fn json_response(status: StatusCode, body: Vec<u8>) -> Response<Cursor<Vec<u8>>> {
    Response::from_data(body)
        .with_status_code(status)
        .with_header(
            tiny_http::Header::from_bytes(
                &b"Content-Type"[..],
                &b"application/json; charset=utf-8"[..],
            )
            .expect("Content-Type header"),
        )
}

fn method_not_allowed(path: &str, expected: &str) -> Response<Cursor<Vec<u8>>> {
    let detail = ErrorDetail::new("method_not_allowed", format!("{path:?} 需 {expected} 方法"));
    json_response(
        StatusCode(405),
        serde_json::to_vec(&ErrorResponse::new(detail))
            .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}").into_bytes()),
    )
}

fn not_found(path: &str, code: &'static str, msg: impl Into<String>) -> Response<Cursor<Vec<u8>>> {
    let detail = ErrorDetail::with_details(code, msg, serde_json::json!({"path": path}));
    json_response(
        StatusCode(404),
        serde_json::to_vec(&ErrorResponse::new(detail))
            .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}").into_bytes()),
    )
}

fn invalid_params(
    path: &str,
    msg: String,
    field: Option<String>,
    value: Option<String>,
) -> Response<Cursor<Vec<u8>>> {
    let mut details = serde_json::Map::new();
    details.insert("path".into(), Value::String(path.into()));
    if let Some(f) = field {
        details.insert("field".into(), Value::String(f));
    }
    if let Some(v) = value {
        details.insert("value".into(), Value::String(v));
    }
    let detail = ErrorDetail::with_details("invalid_params", msg, Value::Object(details));
    json_response(
        StatusCode(400),
        serde_json::to_vec(&ErrorResponse::new(detail))
            .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}").into_bytes()),
    )
}

fn no_supervisor(path: &str) -> Response<Cursor<Vec<u8>>> {
    let detail = ErrorDetail::with_details(
        "no_supervisor",
        "web 模式未装配 supervisor；请先在 live2d-ai.toml 配 LLM/TTS 后重启",
        serde_json::json!({"path": path}),
    );
    json_response(
        StatusCode(503),
        serde_json::to_vec(&ErrorResponse::new(detail))
            .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}").into_bytes()),
    )
}

fn service_unavailable(path: &str) -> Response<Cursor<Vec<u8>>> {
    let detail = ErrorDetail::with_details(
        "supervisor_unavailable",
        "supervisor 已退出或通道关闭",
        serde_json::json!({"path": path}),
    );
    json_response(
        StatusCode(503),
        serde_json::to_vec(&ErrorResponse::new(detail))
            .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}").into_bytes()),
    )
}

// ---------------------------------------------------------------- 单元测试

// ---------------------------------------------------------------- 单元测试
//
// D4 路由测试独立到 `web_api::tests_commands_routes`（由 mod.rs
// 声明，保持本主体 ≤500）。
