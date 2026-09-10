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

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
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
/// `unsubscriber` 在 actor 退出时被调用（关闭 ws 后），让 broadcaster
/// 立即回收槽位，**不**依赖下次 broadcast try_send 失败。
#[derive(Clone)]
struct ConnectionHandle {
    conn_id: u64,
    /// 文本帧 sender（actor 收 + 包装 seq/ts 后写入 ws；M0-7 单线程直连）。
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
#[derive(Clone)]
pub struct Broadcaster {
    inner: Arc<Mutex<Vec<ConnectionHandle>>>,
    subscribed_count: Arc<AtomicUsize>,
    /// **输出静音**（默认 `false` = 出声，2026-09-10 用户裁决）。
    ///
    /// 为 true 时下发给所有客户端的 PCM 全部置零、`volume` 仍为真实值 ——
    /// 即「谁都发不出声音，但口型照常」。语义与理由见
    /// [`build_audio_frames`] 的 `mute` 参数。
    ///
    /// **默认出声**，但保留这个服务端开关：跑测试 / 无人值守时用
    /// `LIVE2D_AI_MUTE_AUDIO=1` 打开它，任何客户端都发不出声，不必依赖前端自觉。
    /// 放在服务端而不是客户端：客户端侧的静音绕不过 Service Worker 缓存的
    /// 旧前端、遗留 JS 前端与第三方客户端。
    muted: Arc<AtomicBool>,
    /// 已经广播过音频的 epoch（存 epoch + 1，0 = 尚未广播任何音频）。
    ///
    /// 用来把 audio.start 还原成它文档里的语义：**本轮语音的首片**。
    /// 修复前它是「每个 TTS 流式分片（约 200 ms）的首片」，一轮 8.6 s 语音会
    /// 产出 44 个 start=true；浏览器端据此把播放时间轴拽回 now，导致一轮音频
    /// 最多 17 片同时发声（削波杂音，实测 529 对重叠、累计 9.79 s）。
    ///
    /// 与 inner / subscribed_count 一样用 Arc 共享：Broadcaster 的 clone 是
    /// 同一实例的别名。
    audio_epoch_seen: Arc<AtomicU64>,
}

impl Default for Broadcaster {
    /// 默认**出声**（见 [`Self::muted`]）。
    ///
    /// 手写而不是 `derive`：`AtomicBool` 的 `Default` 恰好也是 `false`，但两者
    /// 语义不同（一个是「类型默认值」，一个是「产品默认出声」），不靠巧合。
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Vec::new())),
            subscribed_count: Arc::new(AtomicUsize::new(0)),
            muted: Arc::new(AtomicBool::new(false)),
            audio_epoch_seen: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl Broadcaster {
    /// 新建空广播器（订阅上限 = [`WS_CONN_LIMIT`]）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前是否静音输出。
    pub fn is_muted(&self) -> bool {
        self.muted.load(Ordering::Acquire)
    }

    /// 设置静音输出（服务端权威，立即对后续分片生效）。
    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Release);
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

    /// 构造音频帧并逐帧广播（封装 [`build_audio_frames`] + [`broadcast`]）。
    ///
    /// **接线便利**：调用方（如 cli_entry emit 闭包）在拿到
    /// broadcaster + 一段 s16le PCM 时可直接调。注意：当前 v1 路径下 PCM
    /// 不经 AppEvent（详见 supervisor.rs 文档约束），因此**需要在 PCM 流经处**
    /// 显式调用本方法——见报告“接线点结论”。
    ///
    /// audio.start 只在本 epoch 的**首片**上为 true（见 audio_epoch_seen）。
    #[allow(dead_code)]
    pub fn broadcast_audio_frames(&self, epoch: u64, pcm: &[u8], sample_rate: u32) {
        let token = epoch.wrapping_add(1);
        let previous = self.audio_epoch_seen.swap(token, Ordering::AcqRel);
        let segment_start = previous != token;
        let muted = self.muted.load(Ordering::Acquire);
        for frame in build_audio_frames(epoch, pcm, sample_rate, segment_start, muted) {
            self.broadcast(&frame);
        }
    }
}

impl Drop for Broadcaster {
    /// broadcaster drop 时所有 ConnectionHandle 的 tx 被 drop → 各 ws-actor
    /// 线程 rx 断开 → actor 优雅退出（close + unsubscriber）→ 槽位回收。
    ///
    /// **不**在 Drop 里**主动**调 unsubscribe（unsubscribe 闭包本身依赖
    /// broadcaster 状态——已经 drop，再调会死锁）。
    fn drop(&mut self) {
        // 故意 no-op：依赖 mpsc 断链传播。日志便于排障。
        let n = self.subscribed_count.load(Ordering::Acquire);
        if n > 0 {
            tracing::debug!(
                remaining = n,
                "Broadcaster drop：等待剩余 ws-actor 线程自行退出（rx 断链传播）"
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

// 序列号 wrapper helper 已迁到 [`connection::wrap_preserialized_with_seq`]——
// 每连接 actor 持有自己的 ConnectionState，避免全局 static SEQ。

// --------------------------------------------------------------------------
// WS 音频帧（W2+ / F6-T2 接入，v1 最小可播）
// --------------------------------------------------------------------------
//
// 把 s16le PCM 切成 ~20 ms base64 音频帧，协议参照 py 版 `prepare_audio_payload`。
// v1 简化为单声道、单 `volume` 标量（RMS/32768，保留 3 位），不下发 viseme。
//
// 帧 JSON 形如：
// {"type":"audio","data":{"epoch":1,"audio":"<base64 s16le>","sample_rate":24000,
//   "slice_ms":20,"volume":0.3,"start":true,"end":false}}
//
// - 每片 = sample_rate * 2 bytes/sample * 20ms / 1000；
// - **本轮首片** start=true（由 Broadcaster 按 epoch 判定，不是每个分片首片），
//   end=true 标在**本段 PCM** 的末帧——一轮语音会被切成多个 200 ms 段分别调用，
//   故 end 一轮出现多次，**不可**用它判定「本轮语音结束」（轮次边界看 epoch）；
// - volume = 本片 RMS，归一化到 [0,1]，保留 3 位小数。

/// 每片毫秒数（v1 固定 20 ms，与 RMS_WINDOW_MS / TTS chunk 同尺度）。
#[allow(dead_code)]
pub const AUDIO_SLICE_MS: u32 = 20;
/// s16le 每样本字节数。
#[allow(dead_code)]
const S16LE_BYTES_PER_SAMPLE: usize = 2;
/// s16 最大值（用于归一化）：32768.0。
#[allow(dead_code)]
const S16_NORM: f32 = 32_768.0;

/// 由两个环境变量开关决定**输出静音**状态（纯函数，便于单测）。
///
/// 这是「产品默认是否出声」的唯一落点，所以从 `cli_entry` 的启动分支里抽出来，
/// 让它能被离线回归——默认值翻转是很容易被无意改回去的那类行为。
///
/// 规则（优先级从高到低）：
/// 1. `force_unmute`（`LIVE2D_AI_UNMUTE_AUDIO=1`）→ **出声**。保留的旧开关，
///    让旧脚本/旧文档不会因为默认值翻转而静默变成静音；
/// 2. `force_mute`（`LIVE2D_AI_MUTE_AUDIO=1`）→ **静音**（跑测试 / 无人值守用）；
/// 3. 都不设 → **出声**（2026-09-10 用户裁决的默认值）。
pub fn resolve_muted(force_unmute: bool, force_mute: bool) -> bool {
    !force_unmute && force_mute
}

/// 把一段 s16le PCM 按 ~20 ms 切片，产出 WS 音频帧 JSON 字符串列表。
///
/// - `epoch`：所属业务轮次；
/// - `pcm`：s16le 交错样本字节（mono 视为单声道；多声道时仅首声道参与 RMS，
///   其它声道字节原样 Base64 透传）；
/// - `sample_rate`：PCM 采样率（Hz）；
/// - `segment_start`：本段 PCM 是否**一轮语音的首片**——为 true 时首帧
///   `start=true`，否则所有帧 `start=false`。一轮语音会被切成多个 200 ms
///   分片分别调用本函数，只有第一次调用该传 true（见
///   [`Broadcaster::broadcast_audio_frames`]）；
/// - `mute`：**静音输出**。为 true 时下发的 PCM 字节全部置零（长度不变），
///   但 `volume` **仍按原始样本计算** —— 于是「任何客户端都发不出声音」，
///   而口型驱动所需的信息完整保留；
/// - 返回值：空输入 → 空 Vec；非空 → ≥1 帧，**本段**末帧 `end=true`。
///
/// # 为什么在服务端静音（2026-09-10 用户裁决）
///
/// 用户要求「关掉声音但保留口型」。若只在某个客户端做静音，就绕不过：
/// 浏览器 Service Worker 缓存下来的**旧前端**、遗留的原生 JS 前端 `/`、
/// 以及任何第三方客户端。静音属于**产品语义**，必须在音频的**源头**强制，
/// 而不是指望每个消费者自觉。
///
/// 置零而不是「不发帧」：分片节奏与时间轴保持不变（客户端仍按同样节奏排片），
/// 口型的到期释放不会漂移；旧客户端「照常播放」——只是播的是静音。
pub fn build_audio_frames(
    epoch: u64,
    pcm: &[u8],
    sample_rate: u32,
    segment_start: bool,
    mute: bool,
) -> Vec<String> {
    if pcm.is_empty() || sample_rate == 0 {
        return Vec::new();
    }
    // 每片字节数 = sample_rate * 2 bytes/sample * slice_ms / 1000。
    let slice_bytes = (sample_rate as usize * S16LE_BYTES_PER_SAMPLE * AUDIO_SLICE_MS as usize)
        .div_ceil(1000)
        .max(S16LE_BYTES_PER_SAMPLE);
    let total = pcm.chunks(slice_bytes).count();
    let mut out = Vec::with_capacity(total);
    for (i, slice) in pcm.chunks(slice_bytes).enumerate() {
        let start = segment_start && i == 0;
        let end = i + 1 == total;
        // volume 永远取自**原始**样本：静音不能把口型一起关掉。
        let volume = rms_s16le_mono(slice);
        let b64 = if mute {
            // 零填充到相同字节数（s16le 全零 = 绝对静音）。
            base64_encode(&vec![0u8; slice.len()])
        } else {
            base64_encode(slice)
        };
        let frame = serde_json::json!({
            "type": "audio",
            "data": {
                "epoch": epoch,
                "audio": b64,
                "sample_rate": sample_rate,
                "slice_ms": AUDIO_SLICE_MS,
                "volume": volume,
                "start": start,
                "end": end,
                "muted": mute,
            }
        });
        out.push(frame.to_string());
    }
    out
}

/// s16le 单声道片的 RMS 音量归一化标量 ∈ [0, 1]，保留 3 位。
///
/// 对多声道输入（>2 字节/样本），仅首声道用于 RMS；其它声道忽略。
/// 样本数不足整取时，余数样本参与 RMS 但不做填充。
#[allow(dead_code)]
fn rms_s16le_mono(slice: &[u8]) -> f32 {
    let n = slice.len() / S16LE_BYTES_PER_SAMPLE;
    if n == 0 {
        return 0.0;
    }
    let mut sum_sq: f64 = 0.0;
    for i in 0..n {
        let off = i * S16LE_BYTES_PER_SAMPLE;
        let sample = i16::from_le_bytes([slice[off], slice[off + 1]]);
        let f = (sample as f32 / S16_NORM).clamp(-1.0, 1.0);
        let fv = f as f64;
        sum_sq += fv * fv;
    }
    let rms = (sum_sq / n as f64).sqrt();
    // rms 已经 [0,1] 归一化，保留 3 位小数。
    let normalized = rms as f32;
    (normalized * 1000.0).round() / 1000.0
}

/// 把字节片 Base64（标准表）编码 —— 自包含实现（避免新增 crate 依赖）。
///
/// **注**：若日后 desktop Cargo.toml 加入 `base64 = "0.22"`，可替换为
/// `base64::engine::general_purpose::STANDARD.encode(slice)`。
#[allow(dead_code)]
fn base64_encode(slice: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(slice.len().div_ceil(3) * 4);
    for chunk in slice.as_chunks::<3>().0 {
        let b0 = chunk[0] as u32;
        let b1 = chunk[1] as u32;
        let b2 = chunk[2] as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 6) & 0x3F) as usize] as char);
        out.push(TABLE[(n & 0x3F) as usize] as char);
    }
    let rem = slice.len() % 3;
    if rem == 1 {
        let b0 = slice[slice.len() - 1] as u32;
        let n = b0 << 16;
        out.push(TABLE[((n >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3F) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let b0 = slice[slice.len() - 2] as u32;
        let b1 = slice[slice.len() - 1] as u32;
        let n = (b0 << 16) | (b1 << 8);
        out.push(TABLE[((n >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 6) & 0x3F) as usize] as char);
        out.push('=');
    }
    out
}

// --------------------------------------------------------------------------
// WS 音频帧测试
// --------------------------------------------------------------------------

#[cfg(test)]
mod audio_frame_tests {
    use super::*;

    /// 把 Base64（标准表）字符串解码回字节 —— 自包含实现（测试用）。
    fn decode_b64(s: &str) -> Vec<u8> {
        const REV: [u8; 256] = {
            let mut rev = [255u8; 256];
            let table = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut i = 0;
            while i < 64 {
                rev[table[i] as usize] = i as u8;
                i += 1;
            }
            rev
        };
        let mut out = Vec::with_capacity(s.len() * 3 / 4);
        let mut buf = [0u8; 4];
        let mut n = 0usize;
        for &b in s.as_bytes() {
            if b == b'=' {
                break;
            }
            let v = REV[b as usize];
            if v == 255 {
                continue; // 跳过非法/空白字符
            }
            buf[n] = v;
            n += 1;
            if n == 4 {
                out.push(((buf[0] as u32) << 2 | (buf[1] as u32) >> 4) as u8);
                out.push(((buf[1] as u32 & 0x0F) << 4 | (buf[2] as u32) >> 2) as u8);
                out.push(((buf[2] as u32 & 0x03) << 6 | (buf[3] as u32)) as u8);
                n = 0;
            }
        }
        if n == 2 {
            out.push(((buf[0] as u32) << 2 | (buf[1] as u32) >> 4) as u8);
        } else if n == 3 {
            out.push(((buf[0] as u32) << 2 | (buf[1] as u32) >> 4) as u8);
            out.push(((buf[1] as u32 & 0x0F) << 4 | (buf[2] as u32) >> 2) as u8);
        }
        out
    }

    /// 每片字节数计算 helper。
    fn slice_bytes_for_rate(sample_rate: u32) -> usize {
        (sample_rate as usize * S16LE_BYTES_PER_SAMPLE * AUDIO_SLICE_MS as usize)
            .div_ceil(1000)
            .max(S16LE_BYTES_PER_SAMPLE)
    }

    /// i16 向量转为 s16le 字节。
    fn i16_to_bytes(samples: &[i16]) -> Vec<u8> {
        samples.iter().flat_map(|&s| s.to_le_bytes()).collect()
    }

    /// 空输入 → 空帧列表。
    #[test]
    fn empty_input_yields_empty() {
        let frames = build_audio_frames(0, &[], 24000, true, false);
        assert!(frames.is_empty(), "空 PCM 应产出 0 帧");
    }

    /// 零采样率 → 空帧列表（防除零）。
    #[test]
    fn zero_sample_rate_yields_empty() {
        let frames = build_audio_frames(1, &[0u8; 16], 0, true, false);
        assert!(frames.is_empty(), "零采样率应产出 0 帧");
    }

    /// **静音契约（2026-09-10 用户裁决）**：`mute=true` 时
    /// ① 下发的 PCM 必须**全零**（谁都发不出声，与客户端缓存/第三方客户端无关）；
    /// ② `volume` 必须**仍是真实值**（口型不能跟着被关掉）；
    /// ③ 分片数量与长度不变（时间轴与排片节奏不漂移）。
    #[test]
    fn muted_frames_carry_silence_but_real_volume() {
        let sample_rate: u32 = 24000;
        // 非零样本：若静音被实现成「整帧丢弃」或「volume 也清零」，本测试必红。
        let loud = i16_to_bytes(&vec![16_000i16; sample_rate as usize]);

        let normal = build_audio_frames(0, &loud, sample_rate, true, false);
        let muted = build_audio_frames(0, &loud, sample_rate, true, true);

        assert_eq!(
            muted.len(),
            normal.len(),
            "静音不得改变分片数量（时间轴必须一致）"
        );

        // 静音帧的 audio 必须恰好等于「同长度全零」的 base64。
        let silent_b64 = base64_encode(&vec![0u8; slice_bytes_for_rate(sample_rate)]);

        for (i, (n, m)) in normal.iter().zip(muted.iter()).enumerate() {
            let nv: serde_json::Value = serde_json::from_str(n).expect("normal json");
            let mv: serde_json::Value = serde_json::from_str(m).expect("muted json");

            // ② volume 一致且非零（真实电平被保留）。
            let nvol = nv["data"]["volume"].as_f64().expect("volume");
            let mvol = mv["data"]["volume"].as_f64().expect("volume");
            assert!(nvol > 0.0, "前置：原始样本应有非零音量");
            assert_eq!(mvol, nvol, "静音不得改变 volume（第 {i} 帧）");

            // ① 静音帧 PCM 全零。
            let naudio = nv["data"]["audio"].as_str().expect("audio");
            let maudio = mv["data"]["audio"].as_str().expect("audio");
            assert_eq!(maudio, silent_b64, "静音帧 PCM 必须全零（第 {i} 帧）");
            assert_ne!(naudio, silent_b64, "前置：正常帧应含非零样本（第 {i} 帧）");

            // 观测字段：前端可据此提示「已静音」。
            assert_eq!(mv["data"]["muted"], serde_json::Value::Bool(true));
            assert_eq!(nv["data"]["muted"], serde_json::Value::Bool(false));
        }
    }

    /// **默认出声**，且 `set_muted` 立即生效；clone 共享同一开关。
    #[test]
    fn broadcaster_is_unmuted_by_default() {
        let bc = Broadcaster::new();
        assert!(
            !bc.is_muted(),
            "默认必须出声（2026-09-10 用户裁决）；静音需显式 LIVE2D_AI_MUTE_AUDIO=1"
        );
        bc.set_muted(true);
        assert!(bc.is_muted(), "set_muted(true) 应即时生效");
        let alias = bc.clone();
        alias.set_muted(false);
        assert!(!bc.is_muted(), "clone 应共享同一 muted 状态");
    }

    /// 环境变量 → 输出静音的优先级：默认出声；显式 UNMUTE 压过 MUTE。
    #[test]
    fn resolve_muted_defaults_to_audible() {
        assert!(
            !resolve_muted(false, false),
            "都不设时必须出声（产品默认值）"
        );
        assert!(resolve_muted(false, true), "LIVE2D_AI_MUTE_AUDIO=1 应静音");
        assert!(
            !resolve_muted(true, false),
            "LIVE2D_AI_UNMUTE_AUDIO=1 应出声"
        );
        assert!(
            !resolve_muted(true, true),
            "两个开关冲突时以出声为准（旧开关优先级最高）"
        );
    }

    /// 24 kHz 1 秒 s16le mono PCM → 50 帧（20 ms / 帧）。
    #[test]
    fn one_second_24khz_yields_50_frames() {
        let sample_rate: u32 = 24000;
        let one_sec = i16_to_bytes(&vec![0i16; sample_rate as usize]);

        let frames = build_audio_frames(0, &one_sec, sample_rate, true, false);
        assert_eq!(frames.len(), 50, "24kHz 1s 应切 50 帧（每帧 20ms）");
    }

    /// 首帧 start=true，末帧 end=true，中间帧 start/end 均 false。
    #[test]
    fn start_and_end_flags_on_boundary_frames() {
        let sample_rate: u32 = 24000;
        let one_sec = i16_to_bytes(&vec![0i16; sample_rate as usize]);
        let frames = build_audio_frames(0, &one_sec, sample_rate, true, false);
        assert!(!frames.is_empty());

        let first: serde_json::Value = serde_json::from_str(&frames[0]).expect("json");
        let last: serde_json::Value =
            serde_json::from_str(&frames[frames.len() - 1]).expect("json");
        assert_eq!(first["data"]["start"], true, "首帧 start 应为 true");
        assert_eq!(last["data"]["end"], true, "末帧 end 应为 true");

        if frames.len() > 2 {
            let mid: serde_json::Value = serde_json::from_str(&frames[1]).expect("json");
            assert_eq!(mid["data"]["start"], false, "中间帧 start 应为 false");
            assert_eq!(mid["data"]["end"], false, "中间帧 end 应为 false");
        }
    }

    /// volume ∈ [0, 1]，保留 3 位。
    #[test]
    fn volume_in_unit_range() {
        let sample_rate: u32 = 24000;
        // 填充 0.5 振幅正弦（i16 = 32767 * 0.5 ≈ 16383）。
        let samples: Vec<i16> = (0..sample_rate as usize)
            .map(|i| {
                let phase = i as f32 / sample_rate as f32 * 2.0 * std::f32::consts::PI * 440.0;
                (phase.sin() * 16383.0) as i16
            })
            .collect();
        let pcm = i16_to_bytes(&samples);
        let frames = build_audio_frames(0, &pcm, sample_rate, true, false);
        for raw in &frames {
            let v: serde_json::Value = serde_json::from_str(raw).expect("json");
            let vol = v["data"]["volume"].as_f64().expect("volume f64");
            assert!((0.0..=1.0).contains(&vol), "volume 超出 [0,1]: {vol}");
        }
    }

    /// base64 可解码回原字节（round-trip）。
    #[test]
    fn base64_round_trips_original_bytes() {
        let sample_rate: u32 = 24000;
        let raw_samples: Vec<i16> = (0..1920).map(|i| (i as i16) * 10).collect();
        let pcm = i16_to_bytes(&raw_samples);
        let frames = build_audio_frames(0, &pcm, sample_rate, true, false);
        assert!(!frames.is_empty());

        let first: serde_json::Value = serde_json::from_str(&frames[0]).expect("json");
        let b64 = first["data"]["audio"].as_str().expect("audio b64 str");
        let decoded = decode_b64(b64);
        let expected_len = pcm.len().min(slice_bytes_for_rate(sample_rate));
        assert_eq!(decoded.len(), expected_len);
        assert_eq!(&decoded[..], &pcm[..decoded.len()]);
    }

    /// 帧 schema 校验：type/audio/sample_rate/slice_ms/volume/start/end。
    #[test]
    fn frame_schema_shape() {
        let sample_rate: u32 = 24000;
        // 480 i16 样本 = 960 字节 = 1 片（24kHz 20ms）。
        let pcm = i16_to_bytes(&vec![0i16; 480]);
        let frames = build_audio_frames(42, &pcm, sample_rate, true, false);
        assert_eq!(frames.len(), 1, "960 字节 = 1 片（24kHz 20ms）");
        let v: serde_json::Value = serde_json::from_str(&frames[0]).expect("json");
        assert_eq!(v["type"], "audio");
        assert_eq!(v["data"]["epoch"], 42);
        assert_eq!(v["data"]["sample_rate"], 24000);
        assert_eq!(v["data"]["slice_ms"], 20);
        assert_eq!(v["data"]["start"], true);
        assert_eq!(v["data"]["end"], true);
        assert!(v["data"]["volume"].as_f64().is_some());
        assert!(v["data"]["audio"].as_str().is_some());
    }

    /// silent（全零）PCM → volume == 0.0。
    #[test]
    fn silent_pcm_zero_volume() {
        let sample_rate: u32 = 24000;
        let pcm = i16_to_bytes(&vec![0i16; 960]);
        let frames = build_audio_frames(0, &pcm, sample_rate, true, false);
        let v: serde_json::Value = serde_json::from_str(&frames[0]).expect("json");
        assert_eq!(v["data"]["volume"], 0.0, "静音应 volume=0");
    }

    /// broadcaster.broadcast_audio_frames 在有订阅者时能收到音频帧。
    #[test]
    fn broadcast_audio_frames_delivers_to_subscriber() {
        let bc = Broadcaster::new();
        let (tx, rx) = std::sync::mpsc::sync_channel::<String>(32);
        bc.try_subscribe(tx).expect("subscribe");

        let sample_rate: u32 = 24000;
        // 480 i16 样本 = 960 字节 = 1 片（24kHz 20ms）。
        let pcm = i16_to_bytes(&vec![100i16; 480]);
        bc.broadcast_audio_frames(1, &pcm, sample_rate);

        let received = rx
            .recv_timeout(std::time::Duration::from_millis(200))
            .expect("收到音频帧");
        let v: serde_json::Value = serde_json::from_str(&received).expect("json");
        assert_eq!(v["type"], "audio");
        assert_eq!(v["data"]["epoch"], 1);
        assert_eq!(v["data"]["start"], true);
        assert_eq!(v["data"]["end"], true);
    }

    /// 无订阅者时广播音频帧也是安全的 no-op。
    #[test]
    fn broadcast_audio_frames_no_subscribers_is_safe() {
        let bc = Broadcaster::new();
        let sample_rate: u32 = 24000;
        let pcm = i16_to_bytes(&vec![0i16; 480]);
        bc.broadcast_audio_frames(0, &pcm, sample_rate); // 不应 panic
        assert_eq!(bc.subscriber_count(), 0);
    }

    /// 非本轮首片（segment_start=false）：**所有**帧 start 均为 false。
    ///
    /// 这是「一轮语音的 200 ms 分片」中第 2..n 片的正确标记。
    #[test]
    fn segment_start_false_marks_no_frame_as_start() {
        let sample_rate: u32 = 24000;
        let two_sec = i16_to_bytes(&vec![0i16; sample_rate as usize * 2]);
        let frames = build_audio_frames(3, &two_sec, sample_rate, false, false);
        assert_eq!(frames.len(), 100, "2 s / 20 ms = 100 帧");
        for (i, raw) in frames.iter().enumerate() {
            let v: serde_json::Value = serde_json::from_str(raw).expect("json");
            assert_eq!(v["data"]["start"], false, "第 {i} 帧 start 应为 false");
        }
        // end 只标在末帧，与 segment_start 无关。
        let last: serde_json::Value =
            serde_json::from_str(frames.last().expect("非空")).expect("json");
        assert_eq!(last["data"]["end"], true);
    }

    /// **回归钉子（音质）**：一轮语音被切成多个 200 ms 分片时，只有**首片**带
    /// start=true；换一轮（epoch 变化）后首片再次为 true。
    ///
    /// 修复前每个分片都带 start=true，浏览器端每片都把播放时间轴拽回 now，
    /// 而上一片通常仍在播——一轮音频最多 17 片同时发声（削波杂音）。
    #[test]
    fn broadcaster_marks_only_first_chunk_of_epoch_as_start() {
        let bc = Broadcaster::new();
        let (tx, rx) = std::sync::mpsc::sync_channel::<String>(64);
        bc.try_subscribe(tx).expect("subscribe");

        // 4_800 样本 @24 kHz = 200 ms = 10 片。
        let chunk = i16_to_bytes(&vec![100i16; 4_800]);
        let starts = |rx: &std::sync::mpsc::Receiver<String>| -> Vec<bool> {
            (0..10)
                .map(|_| {
                    let raw = rx
                        .recv_timeout(std::time::Duration::from_millis(500))
                        .expect("收到音频帧");
                    let v: serde_json::Value = serde_json::from_str(&raw).expect("json");
                    v["data"]["start"].as_bool().expect("start 是 bool")
                })
                .collect()
        };
        let expect_one_start = |flags: &[bool]| {
            assert_eq!(flags.len(), 10);
            assert!(flags[0], "分片首帧应为 start=true");
            assert!(
                flags[1..].iter().all(|f| !f),
                "分片内其余帧 start 应为 false"
            );
        };

        // 第 1 片：本轮首片 → 唯一一次 start=true。
        bc.broadcast_audio_frames(7, &chunk, 24_000);
        expect_one_start(&starts(&rx));

        // 第 2/3 片：同一 epoch 的后续分片 → 全 false。
        bc.broadcast_audio_frames(7, &chunk, 24_000);
        let second = starts(&rx);
        assert!(second.iter().all(|f| !f), "同轮后续分片不得再发 start=true");
        bc.broadcast_audio_frames(7, &chunk, 24_000);
        let third = starts(&rx);
        assert!(third.iter().all(|f| !f), "同轮后续分片不得再发 start=true");

        // 新一轮：首片重新标记 start=true。
        bc.broadcast_audio_frames(8, &chunk, 24_000);
        expect_one_start(&starts(&rx));
    }
}
