//! 语音转写注入端点：**已退休**（2026-11-16）。
//!
//! # 现在是什么
//!
//! 路径命中 `POST /api/v1/voice/transcript` → **`410 Gone`** +
//! `{"error":{"code":"voice_retired","message":"请改用 /api/v1/voice/asr"}}`；
//! 路径不命中 → `None`（交还 `run_request_loop` 继续派发）。
//!
//! # 为什么是 410 而不是 404 `mod_disabled`
//!
//! 2026-10-19「一 Mod 一本地仓库」之后本端点回的是 `404 mod_disabled`
//! （`voice-input` Mod 的实现移出主仓）。那个码**会骗人**：它读起来像
//! 「把 Mod 打开就好」，而这条路已经**整体作废**——识别现在是核心的
//! `POST /api/v1/voice/asr`（一次转发到 DeepSeek），与 Mod 启停无关。
//! `410 Gone` 说的是「这里曾经有过，现在永远没有了」，并直接给出新地址。
//!
//! # 随它一起退休的（**不恢复**）
//!
//! 唤醒词、sidecar、`run_sidecar`、`backend = mock|sidecar`、
//! `clean_transcript`（不搬进主仓）、locale 归一化、token 门禁、
//! 以及「转写即 `supervisor.say`」那条落点。历史实现见 git 历史与
//! `voice-input` 的本地仓库；`docs/voice-input.md` **已过时**，本计划未改 docs。

use std::io::Cursor;

use tiny_http::{Header, Method, Response, StatusCode};

use crate::web_api::ServerContext;

/// 端点路径（**已退休**；只为让老三方客户端拿到一句人话而不是 404）。
const VOICE_TRANSCRIPT_PATH: &str = "/api/v1/voice/transcript";

/// 退休响应：**总是** 410 `voice_retired`（不看方法 / body / Origin）。
///
/// 刻意不读 `ctx` / body / headers / auth：这条路已经没有实现，**任何**
/// 请求都该得到同一个答案（包括 GET——那也曾经是一条「打开 Mod 就好」的误会）。
pub fn handle_voice_transcript(
    _ctx: &ServerContext,
    _method: &Method,
    path: &str,
    _body: &str,
    _origin: Option<&str>,
    _content_type: Option<&str>,
    _auth_header: Option<&str>,
) -> Option<Response<Cursor<Vec<u8>>>> {
    if path != VOICE_TRANSCRIPT_PATH {
        return None;
    }
    Some(ok_response(
        StatusCode(410),
        r#"{"error":{"code":"voice_retired","message":"请改用 /api/v1/voice/asr"}}"#,
    ))
}

/// 构造 JSON 响应（status + body；Content-Type application/json）。
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

// 回归测试：410 + 稳定 error.code + 路径不命中回 None（同目录 `*_tests.rs`，
// 保持源文件 <500 行）。
#[cfg(test)]
#[path = "voice_routes_tests.rs"]
mod tests;
