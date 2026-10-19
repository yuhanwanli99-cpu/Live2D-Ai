//! A2A **宿主门**（0.3.9 rc.3）：协议核已迁进第二仓 Mod \`live2d-ai-mod-a2a\`。
//!
//! # 宿主只留门（执行单 §3）
//!
//! 本文件是「宿主只留 loopback / Origin / Content-Type 这道门，以及『Mod 停用则
//! 不调用』」的落点。协议（agent card / JSON-RPC 2.0 / 任务表 / 上游收集）都在
//! 第二仓的 crate 里：
//!
//! - 本文件：路径匹配、停用门禁、CT + Origin 校验、405、协议核调用、
//!   tiny_http 响应翻译、POST 派生工作线程；
//! - \`a2a_gate/llm.rs\`：宿主注入的上游闭包（读 \`[llm]\` 快照 + \`secrets::lookup\`
//!   + \`OpenAiClient\`，失败只回稳定错误码）。**密钥值只用在这一跳，不进 Mod、
//!   不进响应、不进日志。**
//!
//! # 停用语义（两处路径同一套）
//!
//! \`a2a\` 是**缺省停用**的 Mod。不在注册表 / 未启用时，\`GET
//! /.well-known/agent-card.json\` 与 \`POST /api/v1/a2a\` **都**回 \`403
//! mod_disabled\`，且**不打上游**（协议核一次都不调）。这与 \`external_routes\` /
//! \`voice_routes\` 的停用门禁同款。

use std::io::Cursor;

use live2d_ai_mod_a2a::{A2A_PATH, AGENT_CARD_PATH, ChatFn, LogFn, LogLevel, core};
use serde_json::{Value, json};
use tiny_http::{Header, Method, Response, StatusCode};

use crate::web_api::ServerContext;
use crate::web_api::security::{
    MutatingCheckError, is_allowed_origin, is_json_content_type, mutating_check_error_response,
};

mod llm;

/// 本 Mod 的注册 id（与 \`AVAILABLE_MOD_FACTORIES\` 里的工厂描述符同一字符串）。
const MOD_ID: &str = "a2a";

/// true = 这条路径属于 A2A（发现文档或 JSON-RPC 端点）。
///
/// \`run_request_loop\` 用它把请求**前置于** dispatch；不命中就交还给后续路由。
pub fn is_a2a_path(path: &str) -> bool {
    path == AGENT_CARD_PATH || path == A2A_PATH
}

/// 一次 A2A 请求的原始字段。
pub struct A2aRequest<'a> {
    pub method: &'a Method,
    pub path: &'a str,
    pub body: &'a str,
    pub origin: Option<&'a str>,
    pub content_type: Option<&'a str>,
}

/// 在独立工作线程里处理一条 A2A 请求。
///
/// \`message/send\` 要等上游 LLM 整段回复；tiny_http 的请求循环是**单线程**的，
/// 在循环里直接等会把 \`/app/\` 与其它 API 一起冻住。GET card 不走这里。
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
    let chat = |text: &str| llm::llm_chat_upstream(ctx, text);
    handle_a2a_with(ctx, req, &chat)
}

/// \`handle_a2a\` 的实现：上游抽成注入点（测试确定性驱动，不碰网络）。
pub(crate) fn handle_a2a_with(
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

    // ---- 停用门禁：两条路径同一套，先于任何协议处理与上游调用。
    if !mod_enabled(ctx) {
        return json_response(
            StatusCode(403),
            &api_error(
                "mod_disabled",
                "a2a Mod 未启用：请在前端「Mod 管理」启用，或 POST /api/v1/mods/a2a/enable",
            ),
        );
    }

    // ---- 发现文档：只读 GET，不套 mutating 校验（loopback 绑定即边界）。
    if path == AGENT_CARD_PATH {
        if *method != Method::Get {
            return json_response(
                StatusCode(405),
                &api_error("method_not_allowed", "agent card 只支持 GET"),
            );
        }
    } else {
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
    }

    // ---- 协议核（第二仓 Mod）：解析 / dispatch / 任务表 / 上游都在那里。
    let log: &LogFn<'_> = &|level, line| match level {
        LogLevel::Info => tracing::info!(target: "a2a", "{line}"),
        LogLevel::Warn => tracing::warn!(target: "a2a", "{line}"),
    };
    let outcome = core().handle(method.as_str(), path, body, ctx.security.port, chat, log);
    let mut response = json_response(StatusCode(outcome.status), &outcome.json);
    if outcome.allow_get {
        response = response
            .with_header(Header::from_bytes(&b"Allow"[..], &b"GET"[..]).expect("Allow header"));
    }
    response
}

/// \`a2a\` 在注册表里且已启用。
fn mod_enabled(ctx: &ServerContext) -> bool {
    ctx.mod_registry
        .lock()
        .map(|registry| registry.is_enabled(MOD_ID))
        .unwrap_or(false)
}

/// 普通 JSON 响应（200 / 403 / 405 / 415 等）。
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

// 回归测试拆到同目录 a2a_gate_tests_*.rs（保持本文件 ≤500 行）。
#[cfg(test)]
#[path = "a2a_gate_tests_gate.rs"]
mod tests_gate;
#[cfg(test)]
#[path = "a2a_gate_tests_route.rs"]
mod tests_route;
#[cfg(test)]
#[path = "a2a_gate_tests_support.rs"]
mod tests_support;
