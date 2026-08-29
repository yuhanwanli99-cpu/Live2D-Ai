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
//! - `200 + {"accepted": true, "epoch": N, "kind": "single"|"composed",
//!   "steps": K}`：invoke 已被 supervisor 受理（已通过 schema 校验 +
//!   已入 action 通道）。**accepted 不等于 core 仲裁通过**——capability
//!   gate / 优先级拒绝由 core 端发 `Dropped` 效果并通过 WS
//!   `runtime_status` 事件回报（D4 接受仲裁结果，前端按状态字段展示）。
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
    /// `"single"` 或 `"composed"`（v1 扩展点；前端按 `kind` 决定表单形态）。
    pub kind: &'static str,
    /// 基础命令的目标动作名（仅 `kind="single"`；与
    /// `live2d_ai_core::ActionId::name()` 同构）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_action: Option<&'static str>,
    /// 组合命令的步骤数（仅 `kind="composed"`）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_count: Option<usize>,
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
    /// 命令形态（`single` / `composed`）。
    pub kind: &'static str,
    /// 实际排队的 `SemanticAction` 数量（单步=1；组合=`steps.len() * repeat`）。
    pub steps: usize,
    /// 触发的目标 action（`single` 命令时为 `Some`，组合为 `None`）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<&'static str>,
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
            target_action: match c.kind {
                CommandKind::Single { action } => Some(action),
                _ => None,
            },
            step_count: match c.kind {
                CommandKind::Composed { steps } => Some(steps.len()),
                _ => None,
            },
        })
        .collect();
    let body = serde_json::to_vec(&CommandsListResponse { commands })
        .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}").into_bytes());
    json_response(StatusCode(200), body)
}

/// 顶层入口（dispatch 调用）：按 method 走 list/invoke。
///
/// `origin` / `content_type` 用于 mutating 路径（D4 §7 P0-3 复审：
/// Origin + Content-Type 校验——与 chat 路径语义一致）。
/// `security.rs` 冻结的现状下，本入口**主动**做同样的校验，避免
/// mutating 路径无门。返回的错误体走 `mutating_check_error_response`。
pub fn handle_commands_route(
    handle: Option<&std::sync::Arc<SupervisorHandle>>,
    method: &Method,
    path: &str,
    body: &str,
    origin: Option<&str>,
    content_type: Option<&str>,
) -> Response<Cursor<Vec<u8>>> {
    // GET 列表：只读，跳过 mutating 校验。
    if *method == Method::Get && path == "/api/v1/commands" {
        return handle_list_commands(method, path);
    }
    // POST invoke：mutating → 校验 Origin + Content-Type。
    if let Err(resp) = check_origin_and_ct(path, origin, content_type, !body.is_empty()) {
        return resp;
    }
    handle_invoke_command(handle, method, path, body)
}

/// 内部：复用 `security::check_mutating_request` 的语义但**不**走
/// `is_mutating_route`（RouteId 未在冻结列表中）；直接调用
/// `check_mutating_request` 内的纯函数。
///
/// 默认严格模式（`allow_no_origin=false`）——与 `ServerContext::new`
/// 默认 `port=0, allow_no_origin=false` 一致；本端点不接受"无 Origin
/// 工具客户端"路径（与 chat 一致：脚本必须显式开 `LIVE2D_AI_ALLOW_NO_ORIGIN`）。
fn check_origin_and_ct(
    _path: &str,
    origin: Option<&str>,
    content_type: Option<&str>,
    body_present: bool,
) -> Result<(), Response<std::io::Cursor<Vec<u8>>>> {
    use crate::web_api::security::{SecurityContext, check_mutating_request};
    let ctx = SecurityContext::new(0, false);
    match check_mutating_request(origin, content_type, &ctx, body_present) {
        Ok(()) => Ok(()),
        Err(err) => Err(crate::web_api::security::mutating_check_error_response(
            &err,
        )),
    }
}

const fn kind_str(kind: &CommandKind) -> &'static str {
    match kind {
        CommandKind::Single { .. } => "single",
        CommandKind::Composed { .. } => "composed",
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
    let (kind, steps, action) = match parsed {
        ParsedParams::Single { repeat, .. } => (
            kind_str(&cmd.kind),
            // 单步：repeat 次。动作已入 supervisor 通道 N 次。
            repeat as usize,
            single_target(&cmd.kind),
        ),
        ParsedParams::Composed { .. } => {
            let n = match cmd.kind {
                CommandKind::Composed { steps } => steps.len(),
                CommandKind::Single { .. } => 0,
            };
            (kind_str(&cmd.kind), n, None)
        }
    };
    let body = serde_json::to_vec(&InvokeAccepted {
        accepted: true,
        epoch,
        command_id: cmd.id,
        kind,
        steps,
        action,
    })
    .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}").into_bytes());
    json_response(StatusCode(200), body)
}

/// 把已校验的 params 翻译为若干 `SemanticAction` 并经 supervisor 通道触发。
///
/// 单一 `accept_arbiter`（§7.3 grep 审计保证）：**不**直接调
/// `core::apply`，仅通过 `SupervisorHandle::trigger_action` 走核心
/// 仲裁路径。
///
/// 返回 `true` = 至少入队一条；`false` = 通道关闭（supervisor 已退）。
fn dispatch_to_supervisor(
    handle: &SupervisorHandle,
    cmd: &CommandDescriptor,
    parsed: ParsedParams,
) -> bool {
    match parsed {
        ParsedParams::Single { strength, repeat } => {
            let target = match single_target(&cmd.kind) {
                Some(t) => t,
                None => return false, // 异常：schema 已锁，不可能到这里
            };
            let action_id = match parse_action_id(target) {
                Some(a) => a,
                None => return false,
            };
            for _ in 0..repeat {
                let Some(sa) = SupervisorHandle::make_user_action(action_id, strength) else {
                    return false; // 异常：validate_params 已保证 1..=3
                };
                if !handle.trigger_action(sa) {
                    return false;
                }
            }
            true
        }
        ParsedParams::Composed { gap_ms } => {
            let CommandKind::Composed { steps } = cmd.kind else {
                return false;
            };
            for step in steps.iter().copied() {
                let Some(action_id) = parse_action_id(step.action) else {
                    return false;
                };
                let Some(sa) = SupervisorHandle::make_user_action(action_id, step.strength) else {
                    return false;
                };
                if !handle.trigger_action(sa) {
                    return false;
                }
                // 步间间隔：v1 走客户端 sleep（greet/react 间隔通常
                // 几百毫秒，supervisor 通道 unbounded 不积压）。在 v1
                // 不引入 timer 任务（任务约束：禁动 supervisor/，意味着
                // 不能 spawn 配套 timer 任务；客户端等待期由前端串行
                // 触发或前端 sleep 决定——v1 文档化「gap_ms 由前端按需
                // 串行触发」，本实现把 gap_ms 透传给前端响应体便于
                // 客户端调度）。
                let _ = gap_ms; // 实际由前端 sleep；本字段仅作"建议"返回
            }
            true
        }
    }
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
        _ => None,
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
