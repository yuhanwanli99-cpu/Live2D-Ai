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

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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
    /// `sentence_seq` / `first_chunk` / `final_chunk` 由**引擎**给出
    /// （`EngineEvent::AudioChunk`）。
    ///
    /// **2026-09-11 修**：这里过去用 `audio_epoch_seen` 按 epoch 记账去推 `start`，
    /// 把「一轮语音的首片」当成了句子边界。一句音频本来就会被切成十几块，按
    /// epoch 记账永远推不出句界——实测 WS 上出现「0 个 start、13 个 end」，
    /// 前端按 `start`→`end` 攒句会攒错、播出来断断续续。现在句界直接来自引擎，
    /// 本方法**不再做任何推断**，那份 epoch 记账状态也随之删除。
    #[allow(dead_code)]
    pub fn broadcast_audio_frames(
        &self,
        epoch: u64,
        pcm: &[u8],
        sample_rate: u32,
        sentence_seq: u64,
        first_chunk: bool,
        final_chunk: bool,
    ) {
        let muted = self.muted.load(Ordering::Acquire);
        for frame in build_audio_frames(
            epoch,
            pcm,
            sample_rate,
            sentence_seq,
            first_chunk,
            final_chunk,
            muted,
        ) {
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
//   "slice_ms":20,"volume":0.3,"sentence_seq":2,"start":true,"end":false}}
//
// - 每片 = sample_rate * 2 bytes/sample * 20ms / 1000；
// - **句子边界**（2026-09-11 修）：`start=true` 只出现在**某一句第一块的首片**，
//   `end=true` 只出现在**该句末块的末片**；每句恰好各一个。一句的音频会被切成
//   十几块，所以这两个标志**必须按句**给，不能按块或按 epoch 给。
//   旧实现用 epoch 记账推 start（整轮只出现一次），实测产出
//   「0 个 start、13 个 end」——前端按 `start`→`end` 攒句会攒错、播出来断断续续。
//   前端据此把一句的 PCM 封成一个 WAV 交给 `<audio>`（见
//   `docs/plans/PLAN-audio-wav-path-2026-09-11.md`）；
// - `sentence_seq`：句子序号（本轮内从 1 严格递增），供前端做分片归组；
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
/// - `sentence_seq`：句子序号（本轮内从 1 严格递增，来自引擎
///   `EngineEvent::AudioChunk`）；
/// - `first_chunk`：本段 PCM 是否为**该句的第一块**——为 true 时首帧 `start=true`；
/// - `final_chunk`：本段 PCM 是否为**该句的最后一块**——为 true 时末帧 `end=true`。
///   两者都由引擎给出（`first_chunk` / `final_chunk`），**不由本函数推断**：
///   一句音频会被切成十几块，只有引擎知道哪块是首/末；
/// - `mute`：**静音输出**。为 true 时下发的 PCM 字节全部置零（长度不变），
///   但 `volume` **仍按原始样本计算** —— 于是「任何客户端都发不出声音」，
///   而口型驱动所需的信息完整保留；
/// - 返回值：空输入 → 空 Vec；非空 → ≥1 帧，`start`/`end` 按上表标注。
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
    sentence_seq: u64,
    first_chunk: bool,
    final_chunk: bool,
    mute: bool,
) -> Vec<String> {
    if sample_rate == 0 {
        return Vec::new();
    }
    // **空 PCM 的句界帧（2026-09-11 修，实测缺陷）**：引擎允许某句的末块
    // 样本数为 0——只要该句样本数正好是 `audio_chunk_samples` 的整数倍就会出现
    // （默认 4 800 样本 = 200 ms @24 kHz，所以 200/400/600 ms 的句子全都命中）。
    // `chunks()` 在空切片上**不产出任何片**，所以过去这里直接返回空表，于是：
    // 该句最后一个**非空**片带 `final_chunk=false` → `end=false`；而承载
    // `end=true` 的空块又被丢掉 → **前端永远等不到句尾闸门**，那一句就播不出来
    // （听感是「尾巴没了」，队列还会一直卡着）。
    //
    // 所以这里改为发**一帧只有边界、没有音频体**的帧（`audio` 为空串）：
    // `start`/`end` 本来就是**标记**，标记不需要带载荷。前端把空音频体解析成
    // 0 字节 PCM，照常在 `end=true` 上封口（`ws_frame.dart` 的 `_str` 对 `""`
    // 返回非 null，因此这一帧不会被「没有音频体就丢弃」那条规则吃掉）。
    if pcm.is_empty() {
        if !final_chunk {
            return Vec::new();
        }
        let frame = serde_json::json!({
            "type": "audio",
            "data": {
                "epoch": epoch,
                "audio": "",
                "sample_rate": sample_rate,
                "slice_ms": AUDIO_SLICE_MS,
                // 空块的 RMS 恒为 0：句尾闭嘴是真话，不是猜测。
                "volume": 0.0,
                "sentence_seq": sentence_seq,
                "start": first_chunk,
                "end": true,
                "muted": mute,
            }
        });
        return vec![frame.to_string()];
    }
    // 每片字节数 = sample_rate * 2 bytes/sample * slice_ms / 1000。
    let slice_bytes = (sample_rate as usize * S16LE_BYTES_PER_SAMPLE * AUDIO_SLICE_MS as usize)
        .div_ceil(1000)
        .max(S16LE_BYTES_PER_SAMPLE);
    let total = pcm.chunks(slice_bytes).count();
    let mut out = Vec::with_capacity(total);
    for (i, slice) in pcm.chunks(slice_bytes).enumerate() {
        // 句界：首块的首片 = start，末块的末片 = end（每句各恰好一个）。
        let start = first_chunk && i == 0;
        let end = final_chunk && i + 1 == total;
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
                "sentence_seq": sentence_seq,
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

    /// 空 PCM + `final_chunk` → **一帧只有边界、没有音频体**的句界帧
    /// （2026-09-11 修，见 [`super::build_audio_frames`] 的空 PCM 分支）。
    ///
    /// 来历：引擎允许某句末块样本数为 0（句子样本数正好是 `audio_chunk_samples`
    /// 整数倍时必然出现）。旧实现直接返回空表，于是那一句**永远没有 `end=true`**
    /// ——前端等不到句尾闸门，整句播不出来。这是本次修复的钉子。
    #[test]
    fn empty_pcm_with_final_chunk_emits_boundary_only_frame() {
        let frames = build_audio_frames(9, &[], 24000, 7, false, true, false);
        assert_eq!(
            frames.len(),
            1,
            "末块为空时仍须发一帧，否则该句没有 end 闸门"
        );
        let v: serde_json::Value = serde_json::from_str(&frames[0]).expect("json");
        assert_eq!(v["type"], "audio");
        assert_eq!(v["data"]["epoch"], 9);
        assert_eq!(v["data"]["sentence_seq"], 7);
        assert_eq!(v["data"]["end"], true, "句尾闸门必须送达");
        assert_eq!(v["data"]["start"], false, "末块不是首块");
        assert_eq!(v["data"]["audio"], "", "边界帧没有音频体");
        assert_eq!(v["data"]["volume"], 0.0, "空块 RMS 恒为 0");
        assert_eq!(
            v["data"]["sample_rate"], 24000,
            "仍带采样率（前端建 WAV 用）"
        );
    }

    /// 空 PCM 且**也不是末块** → 没有任何边界要报，空表（不造无用帧）。
    #[test]
    fn empty_pcm_without_final_chunk_yields_nothing() {
        let frames = build_audio_frames(9, &[], 24000, 7, false, false, false);
        assert!(frames.is_empty(), "非末块的空片没有边界语义，不该发帧");
    }

    /// 空 PCM 且是**单块成句**（首块也是末块）→ 边界帧同时带 start 与 end。
    #[test]
    fn empty_pcm_single_chunk_sentence_marks_both_boundaries() {
        let frames = build_audio_frames(9, &[], 24000, 7, true, true, false);
        assert_eq!(frames.len(), 1);
        let v: serde_json::Value = serde_json::from_str(&frames[0]).expect("json");
        assert_eq!(v["data"]["start"], true);
        assert_eq!(v["data"]["end"], true);
    }

    /// 零采样率一律空表（防除零），**优先于**上面的边界帧规则。
    #[test]
    fn zero_sample_rate_yields_empty() {
        let frames = build_audio_frames(1, &[0u8; 16], 0, 1, true, true, false);
        assert!(frames.is_empty(), "零采样率应产出 0 帧");
        let frames = build_audio_frames(1, &[], 0, 1, true, true, false);
        assert!(frames.is_empty(), "零采样率 + 空 PCM 也不该产出边界帧");
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

        let normal = build_audio_frames(0, &loud, sample_rate, 1, true, true, false);
        let muted = build_audio_frames(0, &loud, sample_rate, 1, true, true, true);

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

        let frames = build_audio_frames(0, &one_sec, sample_rate, 1, true, true, false);
        assert_eq!(frames.len(), 50, "24kHz 1s 应切 50 帧（每帧 20ms）");
    }

    /// 首帧 start=true，末帧 end=true，中间帧 start/end 均 false。
    #[test]
    fn start_and_end_flags_on_boundary_frames() {
        let sample_rate: u32 = 24000;
        let one_sec = i16_to_bytes(&vec![0i16; sample_rate as usize]);
        let frames = build_audio_frames(0, &one_sec, sample_rate, 1, true, true, false);
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
        let frames = build_audio_frames(0, &pcm, sample_rate, 1, true, true, false);
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
        let frames = build_audio_frames(0, &pcm, sample_rate, 1, true, true, false);
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
        let frames = build_audio_frames(42, &pcm, sample_rate, 1, true, true, false);
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
        let frames = build_audio_frames(0, &pcm, sample_rate, 1, true, true, false);
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
        bc.broadcast_audio_frames(1, &pcm, sample_rate, 1, true, true);

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
        bc.broadcast_audio_frames(0, &pcm, sample_rate, 1, true, true); // 不应 panic
        assert_eq!(bc.subscriber_count(), 0);
    }

    /// 句内**非末块**：所有帧 start/end 均为 false（句界不在这一块上）。
    ///
    /// 2026-09-11 重写（原测试名 `segment_start_false_marks_no_frame_as_start`）：
    /// 旧语义里「本段不是整轮首片」就意味着没有 start；现在 `start`/`end` 是
    /// **句子边界**，所以「句内中间块」两个标志都应为 false。
    #[test]
    fn mid_sentence_chunk_marks_neither_boundary() {
        let sample_rate: u32 = 24000;
        let two_sec = i16_to_bytes(&vec![0i16; sample_rate as usize * 2]);
        // 一句的第 2 块（first=false），且不是末块（final=false）。
        let frames = build_audio_frames(3, &two_sec, sample_rate, 1, false, false, false);
        assert_eq!(frames.len(), 100, "2 s / 20 ms = 100 帧");
        for (i, raw) in frames.iter().enumerate() {
            let v: serde_json::Value = serde_json::from_str(raw).expect("json");
            assert_eq!(v["data"]["start"], false, "第 {i} 帧 start 应为 false");
            assert_eq!(v["data"]["end"], false, "第 {i} 帧 end 应为 false");
        }
    }

    /// **句界回归钉子（2026-09-11）**：一句的音频被切成多块时，
    /// **整句只有一片 start=true、一片 end=true**，且它们可以落在**不同块**里。
    ///
    /// 这是「前端按句封 WAV」的前提。旧实现按 epoch 记账推 start，实测产出
    /// 「0 个 start、13 个 end」——前端攒句会攒错、播出来断断续续。
    #[test]
    fn sentence_boundaries_are_exactly_one_start_and_one_end() {
        let bc = Broadcaster::new();
        let (tx, rx) = std::sync::mpsc::sync_channel::<String>(64);
        bc.try_subscribe(tx).expect("subscribe");

        // 4_800 样本 @24 kHz = 200 ms = 10 片。
        let chunk = i16_to_bytes(&vec![100i16; 4_800]);
        // 收 10 帧，返回 (start, end) 列表。
        let flags = |rx: &std::sync::mpsc::Receiver<String>| -> Vec<(bool, bool)> {
            (0..10)
                .map(|_| {
                    let raw = rx
                        .recv_timeout(std::time::Duration::from_millis(500))
                        .expect("收到音频帧");
                    let v: serde_json::Value = serde_json::from_str(&raw).expect("json");
                    (
                        v["data"]["start"].as_bool().expect("start 是 bool"),
                        v["data"]["end"].as_bool().expect("end 是 bool"),
                    )
                })
                .collect()
        };

        // 第 1 句·第 1 块（首块，非末块）：首片 start，无 end。
        bc.broadcast_audio_frames(7, &chunk, 24_000, 1, true, false);
        let a = flags(&rx);
        assert!(a[0].0, "句首块的首片应为 start=true");
        assert!(
            a[1..].iter().all(|(s, _)| !s),
            "同一块内其余帧 start 应为 false"
        );
        assert!(a.iter().all(|(_, e)| !e), "非末块不得出现 end=true");

        // 第 1 句·第 2 块（末块）：末片 end，且**不得**再出现 start。
        bc.broadcast_audio_frames(7, &chunk, 24_000, 1, false, true);
        let b = flags(&rx);
        assert!(b.iter().all(|(s, _)| !s), "句内后续块不得再发 start=true");
        assert!(b[9].1, "句末块的末片应为 end=true");
        assert!(
            b[..9].iter().all(|(_, e)| !e),
            "末块内只有最后一片带 end=true"
        );

        // 第 2 句（新 seq）：句首块重新标记 start=true —— 句界按句，不按 epoch。
        bc.broadcast_audio_frames(7, &chunk, 24_000, 2, true, true);
        let c = flags(&rx);
        assert!(c[0].0, "新一句的首片应为 start=true");
        assert!(c[9].1, "单块成句时首末片同时成立");
    }

    /// 帧里带 `sentence_seq`（前端据此归组同一句的分片）。
    #[test]
    fn frames_carry_sentence_seq() {
        let sample_rate: u32 = 24000;
        let pcm = i16_to_bytes(&vec![0i16; 480]);
        let frames = build_audio_frames(5, &pcm, sample_rate, 9, true, true, false);
        let v: serde_json::Value = serde_json::from_str(&frames[0]).expect("json");
        assert_eq!(v["data"]["epoch"], 5);
        assert_eq!(v["data"]["sentence_seq"], 9);
    }
}
