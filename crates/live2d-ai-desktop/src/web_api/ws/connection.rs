//! Per-connection 处理（M0-7 + F3 简化）。
//!
//! ## 责任
//!
//! - 每条连接**一条**线程（actor，独占持有 [`WebSocket`]）：直接从
//!   `cmd_rx` 收广播帧 → 写 ws；空闲时周期发送 heartbeat（`{"type":"heartbeat"}`）。
//! - 连接建立后**立即**发 [`subscribe_ack`](Self::build_subscribe_ack) 首帧（契约 §4.4）。
//! - 服务端**不**读 socket、不处理客户端帧（WebSocket 是单向事件流；客户端
//!   命令全部走 HTTP）。前端若发文本 `subscribe` / `ping` 会被底层
//!   tungstenite 静默丢弃（auto-pong）——客户端不要依赖这些回包。
//! - 关闭路径：server shutdown → Broadcaster `tx` drop → actor `cmd_rx`
//!   `Disconnected` → actor 主动 close + `unsubscriber(conn_id)` 后退出。
//!
//! ## 线程模型
//!
//! ```text
//! Broadcaster (任意线程)
//!   │ broadcast(typed_event) → 遍历 ConnectionHandle → tx.try_send(String)
//! actor 线程 (per-connection, 持有 WebSocket + 持有 broadcast rx)
//!   │ loop {
//!   │   cmd_rx.recv_timeout(HEARTBEAT_INTERVAL) → Ok(WriteText(text))
//!   │     → 解析 + alloc_seq + 加 seq/ts → ws.write → 失败则 return
//!   │   Timeout → 构造 heartbeat 帧 + alloc_seq → ws.write
//!   │   Disconnected → ws.close → unsubscriber(conn_id) → return
//!   │ }
//! ```
//!
//! **为何是单线程**：tungstenite `read_message` 阻塞 socket read 且没有
//! read timeout API；tiny_http 升级后的 stream 无法 downcast 到 TcpStream
//! 设 read timeout。让 actor 独占持有 ws、把 broadcast 通道直接接到 actor
//! 是唯一干净路径——避免多线程争用 ws 写锁、避免转发线程无意义占 CPU。
//!
//! **不变量**：
//! - mpsc 写路径**不**阻塞：广播走 `try_send`，失败由 broadcaster 回收槽位。
//! - 关闭时显式 `unsubscriber(conn_id)`：actor 退出前调用，让 broadcaster
//!   立即回收槽位，不依赖下次 broadcast try_send 失败延迟回收。
//! - heartbeat（10s）保持连接活跃、防 NAT/代理超时切断：写失败即视为
//!   客户端断开，actor 退出。
//! - **自动 pong**：tungstenite 在底层自动排队 pong reply（v0.24.0 行为）
//!   ——我们**不**手动回 pong。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::web_api::ws::events::iso8601_now_ms;

/// 单连接写消息（**已**序列化的 JSON 帧，**不**含 seq/ts）。容量 256
/// ——慢客户端 backpressure 时 `try_send` 失败即丢弃（D1 §4.6 断线语义）。
///
/// **M0-7 变更**：actor 收到后**解析 → 用每连接 [`ConnectionState`]
/// alloc_seq → 加 seq/ts → 重新序列化 → 写**。每连接 seq 独立。
pub type WsWriter = mpsc::SyncSender<String>;

/// 服务端主动 heartbeat 间隔（10s，介于审查建议的 10–15s 区间）。
///
/// 目的：保持 NAT / 反向代理连接活跃、让客户端能感知 server 仍在运行；
/// 写失败即视为对端断开 → actor 退出。
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);

/// 稳定连接 id（进程级单调；唯一标识一条活跃连接）。
static NEXT_CONNECTION_ID: AtomicU64 = AtomicU64::new(1);

/// 分配下一条稳定连接 id。
pub fn next_connection_id() -> u64 {
    NEXT_CONNECTION_ID.fetch_add(1, Ordering::Relaxed)
}

/// 内部 broker：actor 退出时调用，把自身从 broadcaster 的槽位里剔除。
///
/// 拆分承载：避免循环依赖（broadcaster → connection → broadcaster）。
/// 由 [`crate::web_api::ws::Broadcaster`] 注入。
pub type Unsubscriber = Box<dyn Fn(u64) + Send + 'static>;

/// 单连接状态：仅保留 seq（无 last_frame_at —— 不再基于"客户端帧活跃度"关闭）。
struct ConnectionState {
    /// 每连接 next_seq（从 1 开始；契约 §4.2：每连接独立）。
    next_seq: u64,
}

impl ConnectionState {
    pub fn new() -> Self {
        Self { next_seq: 1 }
    }

    /// 分配下一个 seq 并推进。
    pub fn alloc_seq(&mut self) -> u64 {
        let s = self.next_seq;
        self.next_seq = self.next_seq.saturating_add(1);
        s
    }
}

/// 启动 per-connection 单线程 actor 并立即 return。
///
/// 线程模型：每个连接**一个** actor 线程，独占持有 `ws` + `cmd_rx`（broadcast
/// 通道的 receiver 端）。actor 周期 recv / 写帧 / 发 heartbeat；recv 断开
/// （server shutdown）→ close + unsubscriber → return。
///
/// `conn_id`：稳定连接 id（由 `next_connection_id()` 分配）。
/// `unsubscriber`：actor 退出时调用，让 broadcaster 立即回收槽位。
///
/// **返回值**：actor JoinHandle。设计选择故意**不**返回 spawn 线程的
/// 引用——server 进程持有 JoinHandle 无业务用途，丢弃即可；actor 自管理
/// 退出路径。
pub fn spawn_connection_threads(
    ws: tungstenite::WebSocket<Box<dyn tiny_http::ReadWrite + Send>>,
    rx: mpsc::Receiver<String>,
    conn_id: u64,
    unsubscriber: Unsubscriber,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name(format!("ws-actor-{conn_id}"))
        .spawn(move || actor_loop(ws, rx, conn_id, unsubscriber))
        .expect("spawn actor thread")
}

/// per-connection actor 线程：独占持有 `ws`，从 broadcast 通道收帧 +
/// 周期发 heartbeat。
///
/// 循环：
/// - `cmd_rx.recv_timeout(HEARTBEAT_INTERVAL)` → `Ok(text)` → 解析 →
///   alloc_seq → 加 seq/ts → 写 ws；写失败 return；
/// - `Err(Timeout)` → 构造 heartbeat 帧 → 写 ws；写失败 return；
/// - `Err(Disconnected)` → close ws + `unsubscriber(conn_id)` + return。
fn actor_loop(
    mut ws: tungstenite::WebSocket<Box<dyn tiny_http::ReadWrite + Send>>,
    cmd_rx: mpsc::Receiver<String>,
    conn_id: u64,
    unsubscriber: Unsubscriber,
) {
    let mut state = ConnectionState::new();
    // 1) 连接建立后**立即**发 subscribe_ack（契约 §4.4：服务端首帧）。
    let ack = build_subscribe_ack(&mut state, &[]);
    if write_text(&mut ws, &ack).is_err() {
        unsubscriber(conn_id);
        return;
    }
    // 2) 主循环：broadcast 帧 + heartbeat + 退出路径。
    loop {
        match cmd_rx.recv_timeout(HEARTBEAT_INTERVAL) {
            Ok(text) => {
                let wrapped = wrap_preserialized_with_seq(&text, &mut state);
                if write_text(&mut ws, &wrapped).is_err() {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // 周期唤醒：发 heartbeat 保持连接活跃。heartbeat 帧**也**
                // 走 alloc_seq + seq/ts 包装——前端可与业务事件统一处理。
                let hb = build_heartbeat(&mut state);
                if write_text(&mut ws, &hb).is_err() {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // server shutdown / broadcaster drop → 主动 close 后退出。
                let _ = ws.close(None);
                break;
            }
        }
    }
    // 3) 退出路径：close + 显式 unsubscribe 让 broadcaster 立即回收槽位。
    let _ = ws.close(None);
    unsubscriber(conn_id);
}

/// 写入一段文本帧，统一错误处理（写失败 → `Err(())`，actor 据此退出）。
fn write_text(
    ws: &mut tungstenite::WebSocket<Box<dyn tiny_http::ReadWrite + Send>>,
    text: &str,
) -> Result<(), ()> {
    if ws
        .write(tungstenite::Message::Text(text.to_string()))
        .is_err()
    {
        return Err(());
    }
    if ws.flush().is_err() {
        return Err(());
    }
    Ok(())
}

// -----------------------------------------------------------------------------
// 帧构造 helper（用 connection state 的 seq，每连接独立）
// -----------------------------------------------------------------------------

/// 把已序列化的 JSON 帧（**不含 seq/ts**）解析 → alloc_seq → 加 seq/ts
/// → 重序列化。
///
/// **M0-7 实现策略**：broadcaster 仍走 `String` mpsc（兼容 cli_entry
/// 等旧 caller），actor 收到后 parse → wrap with seq → re-serialize。
/// serde_json 单次 parse + dump 耗时 < 10μs（短帧），不构成热路径瓶颈。
///
/// **失败兜底**：若解析失败（broadcast 是非 JSON 字符串），回退为
/// 字符串原样发出，**不**阻断（log warn 便于排障）。
fn wrap_preserialized_with_seq(text: &str, state: &mut ConnectionState) -> String {
    let seq = state.alloc_seq();
    let ts = iso8601_now_ms();
    let mut frame: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "actor: broadcast 帧非 JSON，原样发出（不阻断）"
            );
            return text.to_string();
        }
    };
    if let Some(obj) = frame.as_object_mut() {
        frame_seq_ts(obj, seq, &ts);
    }
    serde_json::to_string(&frame).unwrap_or_else(|_| text.to_string())
}

/// 在 JSON object 上插入 seq/ts 字段。
fn frame_seq_ts(obj: &mut serde_json::Map<String, serde_json::Value>, seq: u64, ts: &str) {
    obj.insert("seq".to_string(), serde_json::Value::Number(seq.into()));
    obj.insert("ts".to_string(), serde_json::Value::String(ts.to_string()));
}

/// 构造 subscribe_ack 帧（连接建立后的首帧；契约 §4.4）。
fn build_subscribe_ack(state: &mut ConnectionState, topics: &[String]) -> String {
    let seq = state.alloc_seq();
    let ts = iso8601_now_ms();
    let topics_json = serde_json::to_string(topics).unwrap_or_else(|_| "[]".to_string());
    format!(
        r#"{{"type":"subscribe_ack","seq":{seq},"ts":"{ts}","data":{{"topics":{topics_json}}}}}"#
    )
}

/// 构造 heartbeat 帧（10s 周期保活；写失败视为对端断开 → actor 退出）。
fn build_heartbeat(state: &mut ConnectionState) -> String {
    let seq = state.alloc_seq();
    let ts = iso8601_now_ms();
    format!(r#"{{"type":"heartbeat","seq":{seq},"ts":"{ts}","data":{{}}}}"#)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 帧 helper：单连接 alloc_seq 从 1 起步且单调递增。
    #[test]
    fn connection_state_seq_monotonic_from_one() {
        let mut s = ConnectionState::new();
        assert_eq!(s.alloc_seq(), 1);
        assert_eq!(s.alloc_seq(), 2);
        assert_eq!(s.alloc_seq(), 3);
    }

    /// next_connection_id 单调。
    #[test]
    fn next_connection_id_monotonic() {
        let a = next_connection_id();
        let b = next_connection_id();
        let c = next_connection_id();
        assert!(b > a);
        assert!(c > b);
    }

    /// build_subscribe_ack 序列号由 connection state 分配，**不**用全局 static。
    #[test]
    fn subscribe_ack_seq_uses_connection_state() {
        let mut s = ConnectionState::new();
        let ack1 = build_subscribe_ack(&mut s, &[]);
        let ack2 = build_subscribe_ack(&mut s, &[]);
        assert!(ack1.contains("\"seq\":1"), "got: {ack1}");
        assert!(ack2.contains("\"seq\":2"), "got: {ack2}");
    }

    /// build_heartbeat 含 seq/ts 且 type=heartbeat。
    #[test]
    fn heartbeat_frame_shape() {
        let mut s = ConnectionState::new();
        let hb = build_heartbeat(&mut s);
        let v: serde_json::Value = serde_json::from_str(&hb).expect("heartbeat json");
        assert_eq!(v["type"], "heartbeat");
        assert!(v["seq"].is_u64(), "got: {hb}");
        assert!(v["ts"].is_string(), "got: {hb}");
        assert_eq!(v["data"], serde_json::json!({}));
    }

    /// wrap_preserialized_with_seq 给 broadcast 帧加 seq/ts（typed event 路径）。
    #[test]
    fn wrap_preserialized_with_seq_adds_seq_ts() {
        let mut s = ConnectionState::new();
        let wrapped = wrap_preserialized_with_seq(
            r#"{"type":"turn_state","data":{"epoch":7,"status":"completed"}}"#,
            &mut s,
        );
        let v: serde_json::Value = serde_json::from_str(&wrapped).expect("json");
        assert_eq!(v["type"], "turn_state");
        assert_eq!(v["data"]["epoch"], 7);
        assert!(v["seq"].is_u64(), "got: {wrapped}");
        assert!(v["ts"].is_string(), "got: {wrapped}");
    }

    /// wrap_preserialized_with_seq 对非 JSON 字符串原样回退（不阻断）。
    #[test]
    fn wrap_preserialized_with_seq_falls_back_on_non_json() {
        let mut s = ConnectionState::new();
        let wrapped = wrap_preserialized_with_seq("not json", &mut s);
        assert_eq!(wrapped, "not json");
    }
}
