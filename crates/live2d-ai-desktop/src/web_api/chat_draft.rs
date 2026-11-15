//! `POST /api/v1/chat/draft`：草稿一变就换热卡（2026-11-16 热槽记忆 M3）。
//!
//! # 它做什么 / 不做什么
//!
//! - **做**：把该会话的热卡整块换成「由草稿检索出来的那一张」；
//! - **不做**：不创建 turn、不调 TTS、不写 `live2d-ai.toml`、不产生任何 WS 帧。
//!   这条端点是**纯检索**，唯一副作用是内存里那张热卡。
//!
//! # 防抖在客户端
//!
//! 客户端停顿 [DRAFT_DEBOUNCE_MS] 毫秒再发；**服务端不自己防抖**。连发以最后一次
//! 为准——见 [`HotSlot::retrieve`]：每次请求整块替换，旧结果不会追加。
//!
//! # 安全
//!
//! Origin / Content-Type 与 `/api/v1/chat` 同一套（否则 403/415）：这是一条
//! mutating 端点（它改了热卡），口径不许比主链松。
//!
//! # 为什么没有 `wait` 的 1500 ms
//!
//! 计划 §3 说「发送时若查还在跑，最多等 1500 ms」。当前实现里「查」是**同步完成**
//! 的（候选集来自内存事实库，没有向量网络调用），所以这条等待**今天不需要**；
//! 常量 [`RETRIEVE_WAIT_MS`] 留在 `hot_slot` 里，向量检索接上时由它兜底。

use std::io::Cursor;

use serde::{Deserialize, Serialize};
use tiny_http::{Method, Response, StatusCode};

use crate::hot_slot::HotSlot;

/// 客户端建议的停顿时长（毫秒）。服务端**不**据此防抖，只回给前端做默认值。
///
/// 生产代码不读它（防抖在 Flutter 侧）；它在这里是**契约数字**：计划 §6 M3
/// 写「客户端停顿 400 ms 再发」，测试按这个常量断言，前端改值时两边一起改。
#[allow(dead_code)]
pub const DRAFT_DEBOUNCE_MS: u64 = 400;

/// 草稿端点路径。
pub const CHAT_DRAFT_PATH: &str = "/api/v1/chat/draft";

/// `POST /api/v1/chat/draft` 请求体。
#[derive(Debug, Default, Deserialize)]
pub struct DraftBody {
    /// 草稿文本。空串 = 清掉该会话的热卡（没有检索依据）。
    #[serde(default)]
    pub text: Option<String>,
    /// 会话 id（缺省 = 全局桶）。非法 id 视为不带会话（与 chat 同一条口径）。
    #[serde(default)]
    pub session_id: Option<String>,
}

/// `POST /api/v1/chat/draft` 响应。
///
/// `Deserialize` 只为测试读回自己的响应（wire 契约只有 `Serialize` 方向）。
#[derive(Debug, Serialize, serde::Deserialize)]
pub struct DraftAccepted {
    /// 恒为 `true`：草稿不是命令，没有「忙」这种失败态。
    pub accepted: bool,
    /// 归一化后的会话 id（`null` = 全局桶）。
    pub session_id: Option<String>,
    /// 本张热卡的行数（0 = 空热卡）。
    pub lines: usize,
    /// 本张热卡的估算 token（与预算同一口径）。
    pub tokens: usize,
    /// 热卡正文（`- [清楚] …` 三行制；空 = 没有命中）。
    pub card: String,
    /// 本张卡里 `[资料]` 行的条数（0 = 本轮没有外挂资料进出）。
    pub material_lines: usize,
}

/// 处理 `POST /api/v1/chat/draft`。
///
/// 返回 `None` = 路径不匹配（交还 dispatch）；其余一律是完整响应。
///
/// 外挂资料（M5）在这里读：`material` 由调用方从 `[hotmem].material_path` 读出
/// （空 = 不查资料）。读盘失败不报错，按「没有资料」处理。
///
/// 参数多（8 个）是**刻意**的：这条 handler 与 `handle_chat_session` 同款，
/// 把 method / path / body / origin / content-type / security 逐个显式传入，
/// 而不是收一个上下文对象——测试要能逐项构造，且「这一层不许自己去读文件之外的
/// 任何东西」在签名上就看得见。放行 `too_many_arguments` 而不是塞一个 struct。
#[allow(clippy::too_many_arguments)]
pub fn handle_chat_draft(
    hot_slot: Option<&HotSlot>,
    material: &[String],
    method: &Method,
    path: &str,
    body: &str,
    origin: Option<&str>,
    content_type: Option<&str>,
    security: &crate::web_api::security::SecurityContext,
) -> Option<Response<Cursor<Vec<u8>>>> {
    if path != CHAT_DRAFT_PATH {
        return None;
    }
    if *method != Method::Post {
        return Some(method_not_allowed(path, "POST"));
    }
    // mutating 端点：CT 与 Origin 与 /api/v1/chat 同一条口径。
    if !crate::web_api::security::is_json_content_type(content_type) {
        return Some(unsupported_media_type());
    }
    if !crate::web_api::security::is_allowed_origin(origin, security) {
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
    let parsed: DraftBody = match serde_json::from_str(body) {
        Ok(p) => p,
        Err(_) => {
            return Some(json_response(
                StatusCode(400),
                error_body("请求体不是合法 JSON"),
            ));
        }
    };
    // 缺 key 是客户端 bug；空串是「清掉热卡」的合法语义（两者必须分开）。
    let Some(text) = parsed.text else {
        return Some(json_response(
            StatusCode(400),
            error_body("字段 text 必填（空串 = 清掉热卡）"),
        ));
    };
    let session = parsed
        .session_id
        .as_deref()
        .and_then(live2d_ai_mod_system::sanitize_session_id);
    // 没有热槽（理论上不会发生：热槽总是存在）→ 200 空卡，**不报错**：
    // 草稿是尽力而为的辅助，不该让前端多一条只在特定装配下才出现的失败路径。
    let session_id = session.clone();
    let card = match hot_slot {
        Some(slot) => slot.retrieve_with_material(session_id.as_deref(), &text, material, &[]),
        None => live2d_ai_runtime::hotmem::HotCard::empty(),
    };
    let rendered = live2d_ai_runtime::hotmem::render_hot_card(Some(&card));
    let payload = DraftAccepted {
        accepted: true,
        session_id,
        lines: card.lines().len(),
        tokens: card.tokens(),
        card: rendered,
        material_lines: card
            .lines()
            .iter()
            .filter(|line| line.tier == live2d_ai_runtime::hotmem::CardTier::Material)
            .count(),
    };
    Some(json_response(
        StatusCode(200),
        serde_json::to_vec(&payload).unwrap_or_default(),
    ))
}

fn error_body(message: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "error": "invalid_payload",
        "message": message,
    }))
    .unwrap_or_default()
}

fn method_not_allowed(path: &str, expected: &str) -> Response<Cursor<Vec<u8>>> {
    let body = serde_json::to_vec(&serde_json::json!({
        "error": "method_not_allowed",
        "path": path,
        "expected": expected,
    }))
    .unwrap_or_default();
    crate::web_api::responses::json_error(StatusCode(405), body)
}

fn unsupported_media_type() -> Response<Cursor<Vec<u8>>> {
    let body = serde_json::to_vec(&serde_json::json!({
        "error": "unsupported_media_type",
        "message": "Content-Type 必须是 application/json",
    }))
    .unwrap_or_default();
    crate::web_api::responses::json_error(StatusCode(415), body)
}

fn json_response(status: StatusCode, body: Vec<u8>) -> Response<Cursor<Vec<u8>>> {
    crate::web_api::responses::json_error(status, body)
}
