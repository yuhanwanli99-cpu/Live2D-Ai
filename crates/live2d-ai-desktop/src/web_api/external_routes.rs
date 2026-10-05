//! 外部文本注入端点（节点 E5-T3，2026-08-30；0.2.0-rc.1 加强）。
//!
//! 职责：暴露 `POST /api/v1/external/chat`，把外部事件（本地程序 / 直播
//! 弹幕 sidecar / 消息回调）经 `supervisor.say` 注入主链路
//! （text → say → LLM 主链路）。**仅此一个外部面向公网的文本入口**。
//!
//! 契约全文（含 curl / sidecar 说明）：`docs/external-input.md`。
//! **B 站协议抓取不在主仓**——它在 Windows sidecar（`docs/examples/bilibili-sidecar/`），
//! 清洗成纯文本后打本端点；主仓/Rust 侧不实现 blivedm / WSS / 开放平台 SDK。
//!
//! # 安全边界（P0 红线复审）
//!
//! 同 `wasm_assets` 模式一样，本 handler 是 `run_request_loop` 里
//! `wasm_assets::handle_assets` 之后、`dispatch_with_security` 之前的
//! **静态前置路由**：因此 `dispatch_with_security` 的 mutating
//! 校验（[`crate::web_api::security::check_mutating_request`]）**不会**
//! 自动套用在本端点上——本函数**自身**完成安全校验：
//!
//! - **方法**：仅 `POST`，否则 `405`。
//! - **Origin**：必须同源 loopback（`http://127.0.0.1:<port>` /
//!   `http://localhost:<port>`），校验复用
//!   [`crate::web_api::security::is_allowed_origin`]。无 Origin 时走
//!   `SecurityContext.allow_no_origin` 开关（工具客户端 / curl）。
//!   校验失败 → `403 origin_denied`。
//! - **Content-Type**：必须以 `application/json` 开头（前缀匹配，兼容
//!   `application/json; charset=utf-8`），复用
//!   [`crate::web_api::security::is_json_content_type`]；否则 `415`。
//! - **监听本身仅 loopback**（`start_server` 绑定 `127.0.0.1`），
//!   因此本端点天然防远程访问——仅本地进程可达。
//! - **不回 `Access-Control-Allow-Origin`**（默认拒绝跨域）。
//!
//! # 启停门禁（0.2.0-rc.1）
//!
//! `external-input` Mod 是**唯一真源**的开关：
//!
//! - Mod 在注册表里且 `enabled=false` → `403 mod_disabled`
//!   （**不静默吞掉**外部文本，发送方必须知道）。
//! - Mod 在注册表里且 `enabled=true` → 放行。
//! - 注册表里**没有**该 Mod（极简测试上下文 / 自定义装配）→ 不设门禁（放行）：
//!   本端点属于 web_api 的核心 say 能力，不应因一个可选 Mod 缺失而失效。
//!   生产装配始终经 `crate::AVAILABLE_MOD_FACTORIES`，故一定有它。
//!
//! # 文本模板 / 前缀（0.2.0-rc.1）
//!
//! 注入前用 Mod config 的 `prefix` + `text_template`（`{text}` 为占位符）
//! 渲染，纯逻辑在同名 Mod crate（`render_from_config`），handler 不重写一份。
//! 长度上限对**渲染后**文本生效（模板膨胀同样会 400 `text_too_long`）。
//!
//! # token 来源（env 优先，0.2.0-rc.1；W6 起 env 走 `secrets::lookup`）
//!
//! - env `EXTERNAL_INPUT_TOKEN` 非空 → 以它为准（部署期密钥，不落配置文件）。
//!   **值经 [`live2d_ai_runtime::secrets::lookup`] 读**（`.env` 快照 > 进程环境）——
//!   直接读进程环境会绕过 `.env`，于是「界面上刚写了 key、链路还说没配置」；
//! - env 未设/为空 → 回落到 Mod config 的 `token`（前端可填，secret 存储）；
//! - 两者都空 → 不鉴权（仅 loopback，向后兼容）。
//!
//! 优先级链**钉死不变**：env（`.env` > 进程环境）> Mod config > 不鉴权。
//!
//! 请求侧可用 body `token` 或 `Authorization: Bearer <token>`（二选一）。
//!
//! # 可观察计数（Wave 3，2026-09-14）
//!
//! 接受的注入 / 忙碌的丢弃 / 被策略拒绝都在本 handler 里记账（Mod crate 的进程级
//! `AtomicU64`，见 `live2d_ai_mod_external_input::counters`），并可经
//! `GET /api/v1/mods/external-input/state` 读到：
//!
//! - `accepts`：`ok:true` 分支；
//! - `busy`：`ok:false` 分支（code `busy`）；
//! - `rejects`：**策略拒绝**——`401 unauthorized` 与 `403 mod_disabled`。
//!   400（负载非法 / 超长）、405、415、503（未装配）**不计数**；
//! - `v2_ignored`：body 可选字段 `v2_ignored`（sidecar 上报的
//!   `SEND_GIFT_V2` 忽略累计值），**覆盖写**。
//!
//! 计数分支的回归用注入点 `inject` 驱动（见 `handle_external_chat_with`）：
//! 真实 `SupervisorHandle::say` 走容量 1 的通道，worker 是否已 recv 的时序
//! 不可复现，无法稳定造出「忙」。
//!
//! # 限制
//!
//! - **文本长度**：渲染后 ≤2000 字符，否则 `400 text_too_long`。
//! - **JSON 解析失败 / `text` 非字符串 / 空**：`400 invalid_payload`。
//! - **supervisor 未就绪**：`503 supervisor_unavailable`。
//! - **supervisor 忙碌（pending 缓冲已满）**：HTTP `200` + `{"ok":false,
//!   "error":{"code":"busy"}}` —— 文本已被丢弃，发送方应自行退避重试。

use std::io::Cursor;

use tiny_http::{Header, Method, Response, StatusCode};

use crate::web_api::ServerContext;

/// 鉴权 token 环境变量名（**优先于** Mod config 的 `token`）。
const TOKEN_ENV_VAR: &str = "EXTERNAL_INPUT_TOKEN";

/// 路由前缀（v1 固定一个外部文本端点）。
const EXTERNAL_CHAT_PATH: &str = "/api/v1/external/chat";

/// 提供启停门禁 / 模板 / token 的 Mod id。
const MOD_ID: &str = "external-input";

/// 最大允许文本长度（字符，作用于**渲染后**文本）。
const MAX_TEXT_LEN: usize = 2000;

/// 启停门禁 + Mod 配置快照。
struct ModGate {
    /// `true` = 放行（已启用，或注册表里没有该 Mod）。
    enabled: bool,
    /// Mod config（模板 / 前缀 / token）；
    config: serde_json::Value,
}

/// 读 `external-input` 的启停状态与配置（锁粒度：一次 clone）。
fn external_input_gate(ctx: &ServerContext) -> ModGate {
    let reg = match ctx.mod_registry.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(), // 锁中毒不 panic：门禁读快照即可。
    };
    match reg.config(MOD_ID) {
        Some(config) => ModGate {
            enabled: reg.is_enabled(MOD_ID),
            config: config.clone(),
        },
        // 未注册 = 无门禁（见头注「启停门禁」第三点）。
        None => ModGate {
            enabled: true,
            config: serde_json::json!({}),
        },
    }
}

/// 注入提供者：把一段文本交给 supervisor。
///
/// `Some(true)` = 已接受；`Some(false)` = 忙碌（pending 缓冲满）；
/// `None` = supervisor 未就绪（503）。
///
/// 抽成参数是为了让 handler 的**计数分支**可被单测确定性地驱动：真实
/// `SupervisorHandle::say` 走容量 1 的通道，毫秒级时序不可复现
/// （worker 是否已 recv），而计数回归要断言「3 成功 / 1 忙」。
type InjectFn<'a> = dyn Fn(&ServerContext, String) -> Option<bool> + 'a;

/// 密钥查找口径（W6）：生产路径传 [`live2d_ai_runtime::secrets::lookup`]，
/// 测试注入等价闭包——本文件不直接读进程环境。
type LookupFn<'a> = dyn Fn(&str) -> Option<String> + 'a;

/// 生产注入路径：借出 supervisor 并 `say`。
fn supervisor_inject(ctx: &ServerContext, text: String) -> Option<bool> {
    ctx.try_get_supervisor().map(|s| s.say(text))
}

/// 一次外部注入请求的原始字段。
///
/// 收进结构体而不是继续加形参：`handle_external_chat` 的 7 个形参已到
/// clippy `too_many_arguments` 上限，注入点不能再摊平。
struct ExternalRequest<'a> {
    method: &'a Method,
    path: &'a str,
    body: &'a str,
    origin: Option<&'a str>,
    content_type: Option<&'a str>,
    auth_header: Option<&'a str>,
}

/// 处理 `POST /api/v1/external/chat`（外部文本 → say → 主链路）。
///
/// - 路径/方法不匹配 → `None`（交给后续 `dispatch` 继续派发）。
/// - 其它校验/执行均在本函数完成。
pub fn handle_external_chat(
    ctx: &ServerContext,
    method: &Method,
    path: &str,
    body: &str,
    origin: Option<&str>,
    content_type: Option<&str>,
    auth_header: Option<&str>,
) -> Option<Response<Cursor<Vec<u8>>>> {
    handle_external_chat_with(
        ctx,
        ExternalRequest {
            method,
            path,
            body,
            origin,
            content_type,
            auth_header,
        },
        &supervisor_inject,
        &live2d_ai_runtime::secrets::lookup,
    )
}

/// [`handle_external_chat`] 的实现：把「怎么 say」抽成 [`InjectFn`] 注入点。
fn handle_external_chat_with(
    ctx: &ServerContext,
    req: ExternalRequest<'_>,
    inject: &InjectFn<'_>,
    lookup: &LookupFn<'_>,
) -> Option<Response<Cursor<Vec<u8>>>> {
    let ExternalRequest {
        method,
        path,
        body,
        origin,
        content_type,
        auth_header,
    } = req;
    // 路径不匹配：交还给 dispatch。
    if path != EXTERNAL_CHAT_PATH {
        return None;
    }
    // 方法校验。
    if *method != Method::Post {
        return Some(json_error(
            StatusCode(405),
            "method_not_allowed",
            "POST /api/v1/external/chat 仅支持 POST",
        ));
    }
    // Content-Type 校验（防表单 CSRF；curl/python 走 allow_no_origin 工具模式
    // 仍需显式 application/json）。
    if !crate::web_api::security::is_json_content_type(content_type) {
        return Some(json_error(
            StatusCode(415),
            "unsupported_media_type",
            "Content-Type 必须为 application/json",
        ));
    }
    // Origin 校验：同源 loopback，复用 security 判定。
    if !crate::web_api::security::is_allowed_origin(origin, &ctx.security) {
        // 复用 mutating_check_error 的 403 体风格（origin_denied / origin_required）。
        let err = match origin {
            Some(o) => crate::web_api::security::MutatingCheckError::BadOrigin {
                got: Some(o.to_string()),
            },
            None => crate::web_api::security::MutatingCheckError::NoOrigin,
        };
        return Some(crate::web_api::security::mutating_check_error_response(
            &err,
        ));
    }
    // 启停门禁：Mod 已注册但停用 → 不注入、明确告知发送方。
    let gate = external_input_gate(ctx);
    if !gate.enabled {
        live2d_ai_mod_external_input::record_reject();
        return Some(json_error(
            StatusCode(403),
            "mod_disabled",
            "external-input Mod 未启用：请在前端「Mod 管理」启用，或 POST /api/v1/mods/external-input/enable",
        ));
    }
    // JSON 解析（一次性抽取 text + 可选 token）。
    let parsed = match parse_payload(body) {
        Ok(p) => p,
        Err(msg) => {
            return Some(json_error(StatusCode(400), "invalid_payload", &msg));
        }
    };
    // token：env 优先，其次 Mod config（secret）；都空 = 不鉴权。
    // 优先级链**不变**：env（`.env` 快照 > 进程环境，经 `secrets::lookup`）
    // → Mod config → 不鉴权。
    let env_token = lookup(TOKEN_ENV_VAR)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let config_token = live2d_ai_mod_external_input::token_from_config(&gate.config);
    let effective = env_token.as_deref().or(config_token.as_deref());
    if !check_token(parsed.token.as_deref(), auth_header, effective) {
        live2d_ai_mod_external_input::record_reject();
        return Some(json_error(
            StatusCode(401),
            "unauthorized",
            "token 缺失或不匹配（env EXTERNAL_INPUT_TOKEN 或 Mod config token 已设）",
        ));
    }
    // sidecar 上报的 SEND_GIFT_V2 忽略累计值（可选字段）→ 暴露到 state_json。
    if let Some(total) = parsed.v2_ignored {
        live2d_ai_mod_external_input::record_v2_ignored(total);
    }
    // 模板 / 前缀渲染（纯逻辑在 Mod crate；handler 不重写一份）。
    let text = live2d_ai_mod_external_input::render_from_config(&gate.config, &parsed.text);
    // 文本长度（对**渲染后**文本判定：模板膨胀也要拦）。
    if text.chars().count() > MAX_TEXT_LEN {
        return Some(json_error(
            StatusCode(400),
            "text_too_long",
            &format!(
                "text 最多 {} 字符（渲染后收到 {}）",
                MAX_TEXT_LEN,
                text.chars().count()
            ),
        ));
    }
    // 执行：经注入点把文本交给 supervisor。
    let ok = match inject(ctx, text.clone()) {
        Some(ok) => ok,
        None => {
            return Some(json_error(
                StatusCode(503),
                "supervisor_unavailable",
                "supervisor 未就绪，无法注入文本",
            ));
        }
    };
    // 返回 {"ok": true/false, ...}；计数与分支一一对应（见 counters.rs 头注）。
    let body_json = if ok {
        live2d_ai_mod_external_input::record_accept();
        serde_json::json!({"ok": true, "endpoint": "external.chat"})
    } else {
        live2d_ai_mod_external_input::record_busy();
        serde_json::json!({
            "ok": false,
            "endpoint": "external.chat",
            "error": {"code": "busy", "message": "supervisor 忙碌（pending 缓冲已满），本条文本已被丢弃"}
        })
    };
    Some(ok_response(StatusCode(200), &body_json.to_string()))
}

/// 从 JSON body `{"text": "...", "token": "..."}` 抽取 text 与可选 token。
///
/// 要求：`text` 为非空字符串；`token` 如存在必须为字符串（鉴权用）。
fn parse_payload(body: &str) -> Result<Payload, String> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("JSON 解析失败: {e}"))?;
    let Some(text) = v.get("text") else {
        return Err("缺少 `text` 字段".to_string());
    };
    let Some(s) = text.as_str() else {
        return Err("`text` 必须为字符串".to_string());
    };
    let s = s.trim();
    if s.is_empty() {
        return Err("`text` 不得为空".to_string());
    }
    // 可选 token 字段（string）。
    let token = match v.get("token") {
        Some(t) if t.is_string() => t.as_str().map(|s| s.to_string()),
        Some(_) => return Err("`token` 必须为字符串".to_string()),
        None => None,
    };
    // 可选 v2_ignored 字段（u64，sidecar 上报的 SEND_GIFT_V2 忽略累计值）。
    let v2_ignored = match v.get("v2_ignored") {
        Some(n) => Some(
            n.as_u64()
                .ok_or_else(|| "`v2_ignored` 必须为非负整数".to_string())?,
        ),
        None => None,
    };
    Ok(Payload {
        text: s.to_string(),
        token,
        v2_ignored,
    })
}

/// 解析出的请求体（text + 可选 token）。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Payload {
    text: String,
    token: Option<String>,
    /// sidecar 上报的 `SEND_GIFT_V2` 忽略累计值（可选；覆盖写进计数）。
    v2_ignored: Option<u64>,
}

/// 可选 token 鉴权的纯函数。
///
/// - `env_token` 为 `None` 时 → 始终通过（未配置 token，向后兼容）。
/// - `env_token` 为 `Some(t)` 时 → 仅当 `body_token` 或
///   `auth_header`（`Authorization: Bearer <token>`）与之相等时放行。
///   二者均缺/不匹配 → 拒绝。
///
/// 该函数不读环境，不依赖全局状态，便于 `#[cfg(test)]` 注入。
pub fn check_token(
    body_token: Option<&str>,
    auth_header: Option<&str>,
    env_token: Option<&str>,
) -> bool {
    match env_token {
        None => true,
        Some(expected) => {
            // 优先 body token；其次 Authorization: Bearer。
            let body_ok = body_token.map(|b| b == expected).unwrap_or(false);
            let bearer_ok = auth_header
                .and_then(|h| bearer_token(Some(h)))
                .map(|t| t == expected)
                .unwrap_or(false);
            body_ok || bearer_ok
        }
    }
}

/// 从 `Authorization` 头值提取 bearer token。
///
/// 仅当前缀 `Bearer `（大小写不敏感）存在时返回值；否则 `None`。
fn bearer_token(auth_header: Option<&str>) -> Option<&str> {
    let h = auth_header?;
    let lower = h.to_ascii_lowercase();
    if !lower.starts_with("bearer ") {
        return None;
    }
    // 跳过 `bearer ` 前缀（7 字节），原串切片（大小写已校验）。
    let token = &h[7..];
    if token.is_empty() { None } else { Some(token) }
}

/// 构造一个 JSON 响应（status + body；Content-Type application/json）。
fn ok_response(status: StatusCode, body: &str) -> Response<Cursor<Vec<u8>>> {
    Response::from_data(body.as_bytes().to_vec())
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

/// 通用 JSON 错误响应（status + body；Content-Type application/json）。
fn json_error(status: StatusCode, code: &str, message: &str) -> Response<Cursor<Vec<u8>>> {
    let body = serde_json::json!({"error": {"code": code, "message": message}}).to_string();
    ok_response(status, &body)
}

// 回归测试拆到同目录 `external_routes_tests_*.rs`（保持本文件 ≤500 行；与
// `voice_routes_tests.rs` 同模式）。`#[path]` 让测试文件仍是本模块的子模块，
// 可直接 `use super::*` 访问私有项。
#[cfg(test)]
#[path = "external_routes_tests_gate.rs"]
mod tests_gate;
#[cfg(test)]
#[path = "external_routes_tests_handler.rs"]
mod tests_handler;
#[cfg(test)]
#[path = "external_routes_tests_support.rs"]
mod tests_support;
#[cfg(test)]
#[path = "external_routes_tests_token.rs"]
mod tests_token;
