//! WebSocket 桥（拆分为 ws/events + ws/connection + ws/broadcaster + ws/audio）。
//!
//! # 责任
//!
//! - `/ws/runtime` 与 `/ws/state`：tungstenite WS 升级 + 文本帧广播。
//! - `AppEvent` → D1 §4.3 事件 JSON 投影（v1: `turn_state` / `runtime_status` /
//!   `text_delta` / `reasoning_delta` / `error`；其余 P1 pending）——**纯函数**在
//!   [`events::app_event_to_ws_frame`]，本文件仅 `pub use` 转发以保持调用方路径不变。
//! - **音频帧构造**（PCM 切片 + base64 + 句界 + 服务端静音）在 [`audio`]。
//! - **连接生命周期**：注册表 / 广播在 [`broadcaster`]，per-connection actor 在
//!   [`connection`]。
//! - 断线语义（D1 §4.6）：WS 断开不影响 supervisor。
//!
//! # 线程模型（M0-7 + F3 简化）
//!
//! ```text
//! start_server 主 accept 线程 (tiny_http)
//!   │ recv → req.upgrade() (写 101) + derive_accept_key 头
//!   │ spawn "ws-actor-{id}" → 立即 return
//! ws-actor 线程 (per-connection, 独占 WebSocket + 持有 broadcast rx)
//!   │ cmd_rx.recv_timeout(HEARTBEAT_INTERVAL) → 收广播帧 → 写 ws
//!   │ Timeout → 发 heartbeat 帧（保活）→ 写失败退出
//!   │ Disconnected → close + unsubscriber(conn_id) → return
//! Broadcaster (任意线程)
//!   │ broadcast(typed_event) → 遍历 ConnectionHandle → tx.try_send(String)
//! ```
//!
//! **关键不变量**：
//! - `handle_ws_request` **不**阻塞主 accept 循环（写 101 + 派生 ws-actor
//!   后立即 return）；HTTP/chat 请求可与 WS 收发真正并发。
//! - 每连接**一个** actor 线程，独占持有 `ws` —— 没有 read/write 线程锁争用。
//! - 每连接 `next_seq` 由 actor 维护，**不**用全局 static（契约 §4.2）。
//! - 连接退出时 `unsubscriber(conn_id)` 显式回收 broadcaster 槽位，
//!   不依赖下次广播的 try_send 失败延迟回收。
//! - WebSocket 是**单向事件流**：客户端命令全部走 HTTP，WS 不读 socket。
//!   前端**不**应发文本 `subscribe` / `ping` —— 这些会被底层 auto-pong
//!   静默丢弃（无业务回包）。

use std::sync::mpsc;

use serde_json::Value;
use tiny_http::{Method, Request, Response};

use crate::web_api::dto::{ErrorDetail, ErrorResponse};

/// WS 投影纯函数子模块（P1WS-1 拆分承载）。
pub mod events;

/// per-connection 处理子模块（P1WS-2 拆分承载）。
pub mod connection;

/// 连接注册表 / 广播子模块（P1WS-2 拆分承载）。
pub mod broadcaster;

/// WS 音频帧构造子模块（W2+ / F6-T2 拆分承载）。
mod audio;

#[allow(unused_imports)]
pub use connection::WsWriter;

/// 把 [`crate::app_event::AppEvent`] 投影为 D1 §4.3 WS 帧 JSON（`None` = P1 暂不实现）。
#[allow(unused_imports)]
pub use events::app_event_to_ws_frame;

/// 1970-01-01 起累计秒 → (年, 月, 日, 时, 分, 秒)。`pub(crate)` 供测试用。
#[allow(unused_imports)]
pub(crate) use events::epoch_secs_to_ymdhms;
/// ISO 8601 UTC 毫秒精度（无 chrono 依赖）。`pub(crate)` 供 `tests_ws.rs` 调用。
#[allow(unused_imports)]
pub(crate) use events::iso8601_now_ms;

/// 音频帧构造 / 服务端静音决策（调用方：`cli_entry`）——保持原 `ws::` 路径不变。
#[allow(unused_imports)]
pub use audio::{build_audio_frames, resolve_muted};

/// 连接注册表与并发上限（调用方：`web_api` / `supervisor_slot` / 测试）——
/// 保持原 `ws::` 路径不变。
pub use broadcaster::{Broadcaster, WS_CONN_LIMIT};

/// `/ws/runtime` 与 `/ws/state` 端点匹配（共享升级路径；语义差异见 D1 §4.4）。
pub fn is_ws_path(path: &str) -> bool {
    path == "/ws/runtime" || path == "/ws/state"
}

/// 在 WS 升级**前**校验 `Origin` 头（同源白名单）。
pub fn check_request_origin(
    req: &Request,
    port: u16,
) -> Result<(), Response<std::io::Cursor<Vec<u8>>>> {
    let origin = req
        .headers()
        .iter()
        .find(|h| h.field.equiv("Origin"))
        .map(|h| h.value.as_str().to_string());
    if crate::web_api::security::check_ws_origin(origin.as_deref(), port) {
        Ok(())
    } else {
        tracing::debug!(
            origin = ?origin,
            port,
            "WS 升级被拒：Origin 校验失败（403）"
        );
        Err(ws_origin_denied_response(origin.as_deref()))
    }
}

/// 升级 HTTP 请求到 WebSocket 并把 per-connection 处理派生到独立线程。
///
/// **线程模型**：
/// - 当前调用方 = **主 accept 循环**（`start_server`）：写 101 + 构造
///   `WebSocket::from_partially_read` 跳过 tungstenite::accept（避免双 101
///   协议错）+ 计数门控；然后**立刻 return**——主循环继续。
/// - **派生** `ws-actor-{id}` **单线程**（独占 ws + broadcast rx；详见
///   [`connection::spawn_connection_threads`]）。M0-7 简化：移除主转发线程，
///   actor 直接 recv broadcast 通道——消除 ws 写锁争用与无意义 CPU tick。
pub fn handle_ws_request(
    req: Request,
    broadcaster: Broadcaster,
) -> Result<(), Response<std::io::Cursor<Vec<u8>>>> {
    // 0) 计数门控：超额直接拒绝（在升级前）。
    if broadcaster.subscriber_count() >= WS_CONN_LIMIT {
        return Err(ws_too_many_connections_response());
    }

    // 1) 取 `Sec-WebSocket-Key` 头，缺失 → 426。
    let key = req
        .headers()
        .iter()
        .find(|h| h.field.equiv("Sec-WebSocket-Key"))
        .map(|h| h.value.to_string());
    let key = match key {
        Some(k) if !k.is_empty() => k,
        _ => {
            tracing::debug!("WS 升级失败：缺 Sec-WebSocket-Key");
            return Err(ws_protocol_error_response());
        }
    };

    // 2) 计算 `Sec-WebSocket-Accept`。
    let accept = tungstenite::handshake::derive_accept_key(key.as_bytes());

    // 3) 构造**完整**的 101 响应。
    let upgrade_resp = Response::empty(101)
        .with_header(
            tiny_http::Header::from_bytes(&b"Upgrade"[..], &b"websocket"[..])
                .expect("Upgrade header"),
        )
        .with_header(
            tiny_http::Header::from_bytes(&b"Connection"[..], &b"Upgrade"[..])
                .expect("Connection header"),
        )
        .with_header(
            tiny_http::Header::from_bytes(&b"Sec-WebSocket-Accept"[..], accept.as_bytes())
                .expect("Sec-WebSocket-Accept header"),
        );
    let stream = req.upgrade("websocket", upgrade_resp);

    // 4) 用 `WebSocket::from_partially_read` 构造 WS。
    let ws = tungstenite::WebSocket::from_partially_read(
        stream,
        Vec::new(),
        tungstenite::protocol::Role::Server,
        None,
    );

    // 5) 计数门控（升级成功后双重保险）+ 分配 conn_id。
    let (tx, rx) = mpsc::sync_channel::<String>(256);
    let conn_id = match broadcaster.try_subscribe(tx) {
        Ok(id) => id,
        Err(()) => {
            drop(ws);
            return Err(ws_too_many_connections_response());
        }
    };

    // 6) 派生 per-connection actor 线程（M0-7 单线程模型：actor 直连 broadcast rx）。
    let bc_for_unsub = broadcaster.clone();
    let unsubscriber: connection::Unsubscriber = Box::new(move |id: u64| {
        bc_for_unsub.unsubscribe(id);
    });
    let _join = connection::spawn_connection_threads(ws, rx, conn_id, unsubscriber);

    Ok(())
}

// -----------------------------------------------------------------------------
// 错误响应
// -----------------------------------------------------------------------------

fn ws_error_response(
    code: &'static str,
    message: impl Into<String>,
    status: u16,
    details: Option<Value>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let detail = match details {
        Some(d) => ErrorDetail::with_details(code, message, d),
        None => ErrorDetail::new(code, message),
    };
    let body = serde_json::to_vec(&ErrorResponse::new(detail))
        .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}").into_bytes());
    Response::from_data(body)
        .with_status_code(tiny_http::StatusCode(status))
        .with_header(
            tiny_http::Header::from_bytes(
                &b"Content-Type"[..],
                &b"application/json; charset=utf-8"[..],
            )
            .expect("Content-Type header"),
        )
}

/// 把 method+path 直接路由为 405 / 404（`is_ws_path` 命中但 method 非 GET）。
pub fn ws_method_or_path_error(_method: &Method, path: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    if path == "/ws/runtime" || path == "/ws/state" {
        ws_error_response(
            "method_not_allowed",
            format!("WS 端点 {path:?} 仅支持 GET（升级握手）"),
            405,
            None,
        )
    } else {
        ws_error_response("not_found", "WS 端点不存在", 404, None)
    }
}

/// WS 协议不兼容（升级后 tungstenite::accept 失败）→ 426 Upgrade Required。
fn ws_protocol_error_response() -> Response<std::io::Cursor<Vec<u8>>> {
    ws_error_response(
        "ws_upgrade_failed",
        "WS 升级失败：客户端协议不兼容或握手异常",
        426,
        None,
    )
}

/// 并发 WS 连接超过 [`WS_CONN_LIMIT`] → 503 Service Unavailable。
fn ws_too_many_connections_response() -> Response<std::io::Cursor<Vec<u8>>> {
    ws_error_response(
        "ws_too_many_connections",
        format!("WS 并发连接数超过上限 {}", WS_CONN_LIMIT),
        503,
        Some(serde_json::json!({"limit": WS_CONN_LIMIT})),
    )
}

/// D-P0B：WS 升级时 Origin 校验失败 → 403 Forbidden（**不**写 101）。
fn ws_origin_denied_response(origin: Option<&str>) -> Response<std::io::Cursor<Vec<u8>>> {
    let (code, message) = match origin {
        Some(o) => (
            "origin_denied",
            format!("WS 升级失败：Origin {o:?} 不在 loopback 同源白名单"),
        ),
        None => (
            "origin_required",
            "WS 升级失败：缺 Origin 头（浏览器 WS 必带；非浏览器视为异常）".to_string(),
        ),
    };
    ws_error_response(code, message, 403, Some(serde_json::json!({"got": origin})))
}

#[cfg(test)]
mod audio_tests;
