//! 外部文本注入端点（**打包临时树接线**，0.4.6）。
//!
//! 本文件只在 `feature = "packed-mods"` 的打包临时树里参与编译：七个 Mod crate
//! 被静态链进本 binary，`external-input` 才可启用。缺省提交用
//! `web_api::external_routes` 的未挂载存根（404 `mod_disabled`）。
//!
//! 职责：`POST /api/v1/external/chat` 校验后把**一条**原文 `push` 进宿主侧
//! FIFO 队列（[`crate::web_api::external_queue::ExternalQueue`]）；真正的注入由
//! `run_request_loop` 的 `external-queue-pump` 线程在 ready（supervisor 存在 &&
//! 本轮生成已结束 && 播放不忙）时**逐条** `supervisor.say` 打主链路。
//!
//! - **一条原文一次 say**：FIFO，禁止用换行把后来的几条拼成同一次 say
//!   （0.3.1 的忙时合并窗口已废弃，本版不恢复——见 `docs/versions/0.4.md` §0.4.6）。
//! - **待发预算** [`crate::web_api::external_queue::PENDING_CHAR_BUDGET`] = 2000
//!   字符，放不下只丢**当条**；播放忙 120 秒超时只影响泵的 ready 判据，
//!   **不清队列**。
//! - **未挂载或未启用**：404 `mod_disabled`，不入队、不记 accept。
//!
//! 安全边界（与 6a0a1ef 的真实 handler 逐条同源）：本 handler 是
//! `run_request_loop` 的静态前置路由，**自身**完成方法 / Content-Type /
//! Origin 校验（[crate::web_api::security]）；监听仅 loopback，不回
//! `Access-Control-Allow-Origin`。
//!
//! token 优先级**钉死**：env `EXTERNAL_INPUT_TOKEN`（经 `secrets::lookup`，
//! `.env` 快照 > 进程环境）→ Mod config `token` → 不鉴权。请求侧可用 body
//! `token` 或 `Authorization: Bearer <token>`。
//!
//! 可观察计数（Mod crate 的进程级 `AtomicU64`）：`accepts` = 入队成功；
//! `busy` = 待发预算满丢弃当条；`rejects` = 401 / 404 `mod_disabled`；
//! `v2_ignored` = sidecar 上报的 `SEND_GIFT_V2` 忽略累计值（覆盖写）。
//! 400 / 405 / 415 不计数。值不进日志。
//!
//! 本文件 ≤500 行；队列语义回归在 `external_queue.rs`（缺省树可跑）。

use std::io::Cursor;

use tiny_http::{Header, Method, Response, StatusCode};

use crate::web_api::ServerContext;
use crate::web_api::external_queue::{PENDING_CHAR_BUDGET, PushOutcome};

/// 鉴权 token 环境变量名（**优先于** Mod config 的 `token`）。
const TOKEN_ENV_VAR: &str = "EXTERNAL_INPUT_TOKEN";

/// 路由前缀（v1 固定一个外部文本端点）。
const EXTERNAL_CHAT_PATH: &str = "/api/v1/external/chat";

/// 提供启停门禁 / 模板 / token 的 Mod id。
const MOD_ID: &str = "external-input";

/// 最大允许文本长度（字符，作用于**渲染后**文本）。
const MAX_TEXT_LEN: usize = 2000;

/// 读 `external-input` 的配置；**未挂载或未启用都返回 None**（→ 404）。
///
/// 0.4.6 口径：未挂载（缺省树 / 该 crate 没链进来）与在册但 `enabled=false`
/// 都是 404 `mod_disabled`——不再区分 403，也不再有「未注册 = 无门禁」的放行档。
fn enabled_config(ctx: &ServerContext) -> Option<serde_json::Value> {
    let reg = match ctx.mod_registry.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(), // 锁中毒不 panic：门禁读快照即可。
    };
    if !reg.is_enabled(MOD_ID) {
        return None;
    }
    Some(
        reg.config(MOD_ID)
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
    )
}

/// 密钥查找口径（W6）：生产路径传 [`live2d_ai_runtime::secrets::lookup`]，
/// 测试注入等价闭包——本文件不直接读进程环境。
type LookupFn<'a> = dyn Fn(&str) -> Option<String> + 'a;

/// 一次外部注入请求的原始字段。
///
/// 收进结构体而不是继续加形参：`handle_external_chat` 的 7 个形参已到
/// clippy `too_many_arguments` 上限，不能再摊平。
struct ExternalRequest<'a> {
    method: &'a Method,
    path: &'a str,
    body: &'a str,
    origin: Option<&'a str>,
    content_type: Option<&'a str>,
    auth_header: Option<&'a str>,
}

/// 处理 `POST /api/v1/external/chat`（外部文本 → 入队 → 泵线程 say → 主链路）。
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
        &live2d_ai_runtime::secrets::lookup,
    )
}

/// [`handle_external_chat`] 的实现：把密钥查找抽成 [`LookupFn`] 注入点。
fn handle_external_chat_with(
    ctx: &ServerContext,
    req: ExternalRequest<'_>,
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
    // 启停门禁：未挂载 / 未启用 → 404 mod_disabled（不打主链、不入队）。
    let Some(config) = enabled_config(ctx) else {
        live2d_ai_mod_external_input::record_reject();
        return Some(json_error(
            StatusCode(404),
            "mod_disabled",
            "external-input Mod 未挂载或未启用：本端点只在带 Mod 的打包树里可用；请先在「Mod 管理」启用",
        ));
    };
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
    let config_token = live2d_ai_mod_external_input::token_from_config(&config);
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
    let text = live2d_ai_mod_external_input::render_from_config(&config, &parsed.text);
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
    // 入队（FIFO）：一条原文一次 say。真正的 say 由 `run_request_loop` 的
    // `external-queue-pump` 线程在 ready（生成结束 && 播放不忙）时逐条执行；
    // 本函数**不直接 say**、也**不拼接**。
    match ctx.external_queue.push(text) {
        PushOutcome::Queued => {
            live2d_ai_mod_external_input::record_accept();
            let body = serde_json::json!({"ok": true, "endpoint": "external.chat"});
            Some(ok_response(StatusCode(200), &body.to_string()))
        }
        // 待发预算满：只丢**当条**（已在队列里的原样保留），发送方退避重试。
        PushOutcome::DroppedFull => {
            live2d_ai_mod_external_input::record_busy();
            let body = serde_json::json!({
                "ok": false,
                "endpoint": "external.chat",
                "error": {
                    "code": "busy",
                    "message": format!(
                        "待发队列已满（{PENDING_CHAR_BUDGET} 字符上限），本条文本已被丢弃；请退避重试"
                    )
                }
            });
            Some(ok_response(StatusCode(200), &body.to_string()))
        }
    }
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
