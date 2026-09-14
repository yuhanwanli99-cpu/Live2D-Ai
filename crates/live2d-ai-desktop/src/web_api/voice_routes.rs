//! 语音转写注入端点（Wave 2 A 轨，2026-09-14）。
//!
//! 职责：暴露 `POST /api/v1/voice/transcript`，把外部 sidecar（本地 ASR 进程）
//! 产出的转写文本经 `supervisor.say` 注入主链路
//! （transcript → 清洗 → say → LLM → TTS → 口型）。
//!
//! 契约全文（含 curl / sidecar 用法 / 与 external 端点的差异）：
//! `docs/voice-input.md`。**ASR 本体不在主仓**——它在用户机器上的独立
//! sidecar 进程（示例：`docs/examples/voice-sidecar/`）；主仓只收转写文本。
//!
//! # 为什么新增专用端点（而不是复用 `/api/v1/external/chat`）
//!
//! 见 `docs/voice-input.md` §8：voice 侧有自己的 `token` / `locale` / `backend`
//! 语义，且「语音 → 文本 → say」这条链要在**主链路上**证明
//! `live2d_ai_mod_voice_input::clean_transcript` 真的跑了（复用 external
//! 只会让 voice-input Mod 继续当摆设）。二者落点相同（都是 `supervisor.say`），
//! 差别只在输入侧的语义与鉴权来源。
//!
//! # 安全边界
//!
//! 本 handler 是 `run_request_loop` 里 `wasm_assets` / `flutter_app` 之后、
//! `dispatch_with_security` 之前的**静态前置路由**：`dispatch_with_security`
//! 的 mutating 校验**不会**自动套用在本端点上——本函数**自身**完成校验：
//!
//! - **方法**：仅 `POST`，否则 `405 method_not_allowed`。
//! - **Content-Type**：必须 `application/json` 前缀，否则 `415`
//!   （复用 [`crate::web_api::security::is_json_content_type`]）。
//! - **Origin**：同源 loopback，复用
//!   [`crate::web_api::security::is_allowed_origin`]；无 Origin 时走
//!   `SecurityContext.allow_no_origin`（curl / 工具客户端）。失败 →
//!   `403 origin_denied` / `origin_required`（复用 security 错误体）。
//! - **监听本身仅 loopback**（`start_server` 绑 `127.0.0.1`）且不回
//!   `Access-Control-Allow-Origin`。
//!
//! # 启停门禁
//!
//! `voice-input` Mod 的 manifest `enabled` 是**唯一真源**：
//!
//! - 在册且 `enabled=false` → `403 mod_disabled`（**不静默吞**转写）；
//! - 在册且 `enabled=true` → 放行；
//! - **不在册**（极简测试上下文 / 自定义装配）→ 不设门禁（与
//!   `external_routes` 同口径）：端点是 web_api 的核心 say 能力，
//!   不因一个可选 Mod 缺失而失效。
//!
//! # token（env 优先）
//!
//! 优先级：env `VOICE_INPUT_TOKEN` → voice-input Mod config 的 `token` →
//! 不鉴权。请求侧用 body `token` 或 `Authorization: Bearer <token>`（二选一）。
//! 不匹配 → `401 unauthorized`。**任何路径都不回显 token 明文**，也不写日志。
//!
//! # 清洗（复用，不重写）
//!
//! 清洗**必须**走 [`live2d_ai_mod_voice_input::clean_transcript`]：去零宽字符 /
//! 控制字符、折叠空白、去首尾空白。本 handler **不得**内联一份等价实现——
//! 那会让「专用端点证明 Mod 真的在主链路上」这个理由失效。清洗后为空 →
//! `400 empty_transcript`（空转写不得进 `say`，与「空句不进 TTS」同一条纪律）。
//!
//! # 其它限制
//!
//! - 清洗后长度 > 2000 字符 → `400 text_too_long`；
//! - supervisor 未就绪 → `503 supervisor_unavailable`；
//! - 主链忙碌（`say` 缓冲已满）→ **HTTP 200** + `{"ok":false,"error":
//!   {"code":"busy"}}`——刻意不用 5xx：发送方自行退避，不要重试到刷屏。

use std::io::Cursor;

use tiny_http::{Header, Method, Response, StatusCode};

use crate::web_api::ServerContext;

/// 鉴权 token 环境变量名（**优先于** Mod config 的 `token`）。
const TOKEN_ENV_VAR: &str = "VOICE_INPUT_TOKEN";

/// 端点路径（v1 固定一个语音转写入口）。
const VOICE_TRANSCRIPT_PATH: &str = "/api/v1/voice/transcript";

/// 提供启停门禁 / token 的 Mod id。
const MOD_ID: &str = "voice-input";

/// 最大允许长度（字符，作用于**清洗后**文本）。
const MAX_TEXT_LEN: usize = 2000;

/// 启停门禁 + Mod 配置快照。
struct ModGate {
    /// `true` = 放行（已启用，或注册表里没有该 Mod）。
    enabled: bool,
    /// Mod config（token 来源）。
    config: serde_json::Value,
}

/// 读 `voice-input` 的启停状态与配置（锁粒度：一次 clone）。
fn voice_input_gate(ctx: &ServerContext) -> ModGate {
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

/// 处理 `POST /api/v1/voice/transcript`（语音转写 → 清洗 → say → 主链路）。
///
/// - 路径不匹配 → `None`（交还给后续 `dispatch` 继续派发）；
/// - 其它校验/执行均在本函数完成（前置路由，不经过 `dispatch_with_security`）。
pub fn handle_voice_transcript(
    ctx: &ServerContext,
    method: &Method,
    path: &str,
    body: &str,
    origin: Option<&str>,
    content_type: Option<&str>,
    auth_header: Option<&str>,
) -> Option<Response<Cursor<Vec<u8>>>> {
    // 路径不匹配：交还给 dispatch。
    if path != VOICE_TRANSCRIPT_PATH {
        return None;
    }
    // 方法校验。
    if *method != Method::Post {
        return Some(json_error(
            StatusCode(405),
            "method_not_allowed",
            "POST /api/v1/voice/transcript 仅支持 POST",
        ));
    }
    // Content-Type 校验（防表单 CSRF）。
    if !crate::web_api::security::is_json_content_type(content_type) {
        return Some(json_error(
            StatusCode(415),
            "unsupported_media_type",
            "Content-Type 必须为 application/json",
        ));
    }
    // Origin 校验：同源 loopback，复用 security 判定（错误体也复用）。
    if !crate::web_api::security::is_allowed_origin(origin, &ctx.security) {
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
    let gate = voice_input_gate(ctx);
    if !gate.enabled {
        return Some(json_error(
            StatusCode(403),
            "mod_disabled",
            "voice-input Mod 未启用：请在前端「Mod 管理」启用，或 POST /api/v1/mods/voice-input/enable",
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
    let env_token = std::env::var(TOKEN_ENV_VAR).ok();
    let config_token = live2d_ai_mod_voice_input::token_from_config(&gate.config);
    let effective = effective_token(env_token.as_deref(), config_token.as_deref());
    if !check_token(parsed.token.as_deref(), auth_header, effective.as_deref()) {
        return Some(json_error(
            StatusCode(401),
            "unauthorized",
            "token 缺失或不匹配（env VOICE_INPUT_TOKEN 或 Mod config token 已设）",
        ));
    }
    // 清洗：**复用 Mod crate 的纯函数**（头注「清洗」）。
    let Some(text) = live2d_ai_mod_voice_input::clean_transcript(&parsed.text) else {
        return Some(json_error(
            StatusCode(400),
            "empty_transcript",
            "转写清洗后为空（空白 / 零宽字符 / 控制字符）：不发空回合",
        ));
    };
    // 长度：对**清洗后**文本判定。
    if text.chars().count() > MAX_TEXT_LEN {
        return Some(json_error(
            StatusCode(400),
            "text_too_long",
            &format!(
                "转写最多 {} 字符（清洗后收到 {}）",
                MAX_TEXT_LEN,
                text.chars().count()
            ),
        ));
    }
    // 执行：借出 supervisor say（与 external 端点同一落点）。
    let supervisor = match ctx.try_get_supervisor() {
        Some(s) => s,
        None => {
            return Some(json_error(
                StatusCode(503),
                "supervisor_unavailable",
                "supervisor 未就绪，无法注入语音转写",
            ));
        }
    };
    let accepted = supervisor.say(text.clone());
    // 忙碌刻意 200 + ok:false（见头注）：发送方退避，不重试到刷屏。
    let body_json = if accepted {
        serde_json::json!({"ok": true, "text": text})
    } else {
        serde_json::json!({
            "ok": false,
            "error": {
                "code": "busy",
                "message": "supervisor 忙碌（pending 缓冲已满），本条转写已被丢弃"
            }
        })
    };
    Some(ok_response(StatusCode(200), &body_json.to_string()))
}

/// 从 JSON body `{"text": "...", "token": "..."}` 抽取 text 与可选 token。
///
/// 只判**结构**：`text` 必须是字符串（可以是空白——「清洗后为空」由
/// [`live2d_ai_mod_voice_input::clean_transcript`] 判定并映射
/// `400 empty_transcript`，与 `400 invalid_payload` 刻意分开）。
fn parse_payload(body: &str) -> Result<Payload, String> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("JSON 解析失败: {e}"))?;
    let Some(text) = v.get("text") else {
        return Err("缺少 `text` 字段".to_string());
    };
    let Some(s) = text.as_str() else {
        return Err("`text` 必须为字符串".to_string());
    };
    // 可选 token 字段（string）。
    let token = match v.get("token") {
        Some(t) if t.is_string() => t.as_str().map(|s| s.to_string()),
        Some(_) => return Err("`token` 必须为字符串".to_string()),
        None => None,
    };
    Ok(Payload {
        text: s.to_string(),
        token,
    })
}

/// 解析出的请求体（text + 可选 token）。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Payload {
    text: String,
    token: Option<String>,
}

/// 可选 token 鉴权的纯函数。
///
/// - `expected` 为 `None` → 始终通过（未配置 token，向后兼容）；
/// - `expected` 为 `Some(t)` → 仅当 `body_token` 或 `auth_header`
///   （`Authorization: Bearer <token>`）与之相等时放行。
///
/// 不读环境、不依赖全局状态，便于 `#[cfg(test)]` 注入。
pub fn check_token(
    body_token: Option<&str>,
    auth_header: Option<&str>,
    expected: Option<&str>,
) -> bool {
    match expected {
        None => true,
        Some(expected) => {
            let body_ok = body_token.map(|b| b == expected).unwrap_or(false);
            let bearer_ok = auth_header
                .and_then(|h| bearer_token(Some(h)))
                .map(|t| t == expected)
                .unwrap_or(false);
            body_ok || bearer_ok
        }
    }
}

/// token 生效顺序（**钉死的契约**，纯函数便于断言优先级）：
/// env 非空 → env；否则 Mod config；都空/纯空白 → `None`（不鉴权）。
///
/// 空值语义是「未配置」而不是「用空 token 鉴权」——空 token 若算「已配置」，
/// 任何不带 token 的请求都会 401，等于把「不鉴权」这个兼容档悄悄删掉。
pub fn effective_token(env: Option<&str>, config: Option<&str>) -> Option<String> {
    let non_blank = |s: &str| !s.trim().is_empty();
    env.filter(|s| non_blank(s))
        .or_else(|| config.filter(|s| non_blank(s)))
        .map(|s| s.trim().to_string())
}

/// 从 `Authorization` 头值提取 bearer token（前缀大小写不敏感）。
fn bearer_token(auth_header: Option<&str>) -> Option<&str> {
    let h = auth_header?;
    if !h.to_ascii_lowercase().starts_with("bearer ") {
        return None;
    }
    // 跳过 `bearer ` 前缀（7 字节）。
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

/// 通用 JSON 错误响应（status + `{"error":{"code","message"}}`）。
fn json_error(status: StatusCode, code: &str, message: &str) -> Response<Cursor<Vec<u8>>> {
    let body = serde_json::json!({"error": {"code": code, "message": message}}).to_string();
    ok_response(status, &body)
}

// 回归测试拆到同目录 `voice_routes_tests.rs`（保持本文件 ≤500 行，
// 与 `tests_*` 系列同模式）。`#[path]` 让测试文件仍是本模块的子模块，
// 可直接 `use super::*` 访问私有项。
#[cfg(test)]
#[path = "voice_routes_tests.rs"]
mod tests;
