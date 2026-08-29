//! WebSocket 桥（P1WS-2 拆出 `ws/connection.rs` + `ws/events.rs`）。
//!
//! # 责任
//!
//! - `/ws/runtime` 与 `/ws/state`：tungstenite WS 升级 + 文本帧广播。
//! - `AppEvent` → D1 §4.3 事件 JSON 投影（v1: `turn_state` / `runtime_status` /
//!   `text_delta` / `error` / `tool_action`；其余 P1 pending）——**纯函数**已迁
//!   到 [`events::app_event_to_ws_frame`]，本文件仅 `pub use` 转发以保持调用方
//!   路径不变。
//! - 断线语义（D1 §4.6）：WS 断开不影响 supervisor。
//!
//! # 线程模型（2026-08-29 D5 修复 + P1WS-2 升级）
//!
//! ```text
//! start_server 主 accept 线程 (tiny_http)
//!   │ recv → req.upgrade() (写 101) + derive_accept_key 头
//!   │ spawn "ws-conn-{id}" → 立即 return
//! ws-conn 主线程 (per-connection, 持有 mpsc rx + cmd_tx)
//!   │ rx.recv_timeout 周期: 等广播帧 → cmd_tx.send(WriteText)
//!   │ rx 断开 / 30s 主动 unsubscriber: cmd_tx.send(Close) → return
//! ws-actor 线程 (per-connection, 独占 WebSocket)
//!   │ 阻塞 ws.read_message → 处理 close/ping/subscribe
//!   │ 处理 cmd_rx (WriteText/Close)
//!   │ 自动 pong (tungstenite 内置)
//! Broadcaster (任意线程)
//!   │ broadcast_typed(event) → 遍历 ConnectionHandle → 包装 seq/ts → mpsc try_send
//! ```
//!
//! **关键不变量**：
//! - `handle_ws_request` **不**阻塞主 accept 循环（写 101 + 派生 ws-conn
//!   后立即 return）；HTTP/chat 请求可与 WS 收发真正并发。
//! - 每连接 `next_seq` 由 actor 维护，**不**用全局 static（契约 §4.2）。
//! - 连接退出时 `unsubscriber(conn_id)` 显式回收 broadcaster 槽位，
//!   不依赖下次广播的 try_send 失败延迟回收。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use serde_json::Value;
use tiny_http::{Method, Request, Response};

use crate::web_api::dto::{ErrorDetail, ErrorResponse};

/// WS 投影纯函数子模块（P1WS-1 拆分承载）。
pub mod events;

/// per-connection 处理子模块（P1WS-2 拆分承载）。
pub mod connection;

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

/// WS 连接并发上限（防线程泄漏 + DoS）。32 = 前端 ≤ 3 标签 + chat overlay；
/// 再高通常是滥用。超额时 `handle_ws_request` 回 503 不进入升级流程。
pub const WS_CONN_LIMIT: usize = 32;

/// 单连接在 broadcaster 侧的句柄。
///
/// `unsubscriber` 在连接线程退出时被调用（**先**于 actor 退出），让
/// broadcaster 立即回收槽位，**不**依赖下次 broadcast try_send 失败。
#[derive(Clone)]
struct ConnectionHandle {
    conn_id: u64,
    /// 文本帧 sender（actor 收 + 包装 seq/ts 后写入 ws）。
    tx: WsWriter,
}

/// 广播器：所有活跃 WS 连接的 writer 集合 + 订阅计数 + 稳定 conn_id。
///
/// **P1WS-2 改进**：
/// - `Vec<ConnectionHandle>` 携带稳定 `conn_id`，`unsubscribe(id)` 按 id
///   移除（O(n) 但 n ≤ 32 可接受）；
/// - `broadcast(typed_event)` 广播**未**含 seq/ts 的 typed event，seq/ts
///   由各连接的 actor 包装（每连接独立 seq）。
/// - 死 sender 由下次 broadcast try_send 失败回收（仍保留向后兼容）。
#[derive(Clone, Default)]
pub struct Broadcaster {
    inner: Arc<Mutex<Vec<ConnectionHandle>>>,
    subscribed_count: Arc<AtomicUsize>,
}

impl Broadcaster {
    /// 新建空广播器（订阅上限 = [`WS_CONN_LIMIT`]）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 尝试注册一个新连接；超额时返回 `Err(())`。
    ///
    /// 成功时把 sender 推入内部 vec，计数 +1，并返回稳定 conn_id。
    /// 失败时 vec/计数都不变——caller 决定是否回 503。
    pub fn try_subscribe(&self, ws: WsWriter) -> Result<u64, ()> {
        let mut guard = self.inner.lock().expect("broadcaster mutex");
        let prev = self.subscribed_count.load(Ordering::Acquire);
        if prev >= WS_CONN_LIMIT {
            return Err(());
        }
        let conn_id = connection::next_connection_id();
        guard.push(ConnectionHandle { conn_id, tx: ws });
        self.subscribed_count.store(prev + 1, Ordering::Release);
        Ok(conn_id)
    }

    /// 按稳定 conn_id 注销订阅（**新**：P1WS-2）。
    ///
    /// 显式回收槽位，**不**等下次 broadcast try_send 失败。返回是否真的
    /// 移除了某条（false = id 不存在，可能是双重 unsubscribe）。
    pub fn unsubscribe(&self, conn_id: u64) -> bool {
        let mut guard = self.inner.lock().expect("broadcaster mutex");
        let before = guard.len();
        guard.retain(|h| h.conn_id != conn_id);
        let removed = before != guard.len();
        if removed {
            let prev = self.subscribed_count.load(Ordering::Acquire);
            if prev > 0 {
                self.subscribed_count.store(prev - 1, Ordering::Release);
            }
        }
        removed
    }

    /// 当前订阅者数量（仅供测试与观测；生产不开销）。
    pub fn subscriber_count(&self) -> usize {
        self.subscribed_count.load(Ordering::Acquire)
    }

    /// 广播一条**已序列化**的 JSON 帧（**不**含 seq/ts）。seq/ts 由
    /// 各连接 actor 在 [`connection::wrap_preserialized_with_seq`] 处
    /// 解析 + 包装（每连接独立 seq）。
    ///
    /// **不**阻塞：每个订阅者用 `try_send`（失败即移除该订阅者，计数 -1）。
    /// 死 sender 由下一次 broadcast try_send 失败回收（延迟回收）。
    pub fn broadcast(&self, frame: &str) {
        let mut guard = self.inner.lock().expect("broadcaster mutex");
        let mut idx = 0;
        while idx < guard.len() {
            if guard[idx].tx.try_send(frame.to_string()).is_err() {
                let removed = guard.swap_remove(idx);
                let prev = self.subscribed_count.load(Ordering::Acquire);
                if prev > 0 {
                    self.subscribed_count.store(prev - 1, Ordering::Release);
                }
                tracing::debug!(
                    conn_id = removed.conn_id,
                    "broadcaster: 慢订阅者 try_send 失败 → 移除槽位"
                );
            } else {
                idx += 1;
            }
        }
    }

    /// 测试用：强制清空广播器（模拟所有订阅者掉线）。
    #[cfg(test)]
    pub fn clear(&self) {
        self.inner.lock().expect("broadcaster mutex").clear();
        self.subscribed_count.store(0, Ordering::Release);
    }
}

impl Drop for Broadcaster {
    /// broadcaster drop 时所有 ConnectionHandle 的 tx 被 drop → 各 ws-conn
    /// 主线程 rx 断开 → 发 Close 命令 → actor 优雅退出 → 槽位回收。
    ///
    /// **不**在 Drop 里**主动**调 unsubscribe（unsubscribe 闭包本身依赖
    /// broadcaster 状态——已经 drop，再调会死锁）。
    fn drop(&mut self) {
        // 故意 no-op：依赖 mpsc 断链传播。日志便于排障。
        let n = self.subscribed_count.load(Ordering::Acquire);
        if n > 0 {
            tracing::debug!(
                remaining = n,
                "Broadcaster drop：等待剩余 ws-conn 线程自行退出（rx 断链传播）"
            );
        }
    }
}

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
/// - 派生 `ws-conn-{id}` 双线程（主线程 + actor 线程，详见
///   [`connection::spawn_connection_threads`]）。
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
    let conn_id = match broadcaster.try_subscribe(tx.clone()) {
        Ok(id) => id,
        Err(()) => {
            drop(ws);
            return Err(ws_too_many_connections_response());
        }
    };

    // 6) 派生 per-connection 双线程。
    let bc_for_unsub = broadcaster.clone();
    let unsubscriber: connection::Unsubscriber = Box::new(move |id: u64| {
        bc_for_unsub.unsubscribe(id);
    });
    let _join = connection::spawn_connection_threads(ws, tx, rx, conn_id, unsubscriber);

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

// 序列号 wrapper helper 已迁到 [`connection::wrap_preserialized_with_seq`]——
// 每连接 actor 持有自己的 ConnectionState，避免全局 static SEQ。
