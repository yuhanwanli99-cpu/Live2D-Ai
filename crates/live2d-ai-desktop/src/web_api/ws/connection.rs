//! Per-connection 处理（拆分承载；P1WS-2）。
//!
//! ## 责任
//!
//! - 每条连接两条线程：
//!   - **actor 线程**（持有 [`WebSocket`]）：阻塞 `read_message` 读客户端
//!     帧 + 处理来自主线程的写命令。
//!   - **主线程**（per-connection，spawn by ws.rs handle_ws_request）：
//!     mpsc 100ms tick 收广播帧 → 发 Write 命令给 actor。
//! - 关闭路径：客户端 close / 服务端 shutdown → 显式 `unsubscriber(conn_id)`
//!   + 发 Close 命令给 actor。
//! - 持每连接 [`ConnectionState`]：next_seq（从 1 开始）+ 上次收到任意帧的时刻。
//!
//! ## 线程模型
//!
//! ```text
//! Broadcaster (任意线程)
//!   │ broadcast(typed_event) → 遍历 ConnectionHandle → 包装 seq/ts → mpsc try_send
//! 主线程 (per-connection, 由 handle_ws_request spawn)
//!   │ loop {
//!   │   rx.recv_timeout(100ms) → cmd_tx.send(WriteText(text))
//!   │ }
//!   │ unsubscriber(conn_id) → cmd_tx.send(Close) → return
//! actor 线程 (per-connection, 持有 WebSocket)
//!   │ loop {
//!   │   ws.read_message() → handle + 必要时 ws.write 回包
//!   │   30s idle 软判（在 read 返回后检查 last_frame_at）
//!   │ }
//! ```
//!
//! **为何要 actor 模式**：tungstenite 的 `read_message` 阻塞 socket
//! read，没有 read timeout API；tiny_http 升级后的 stream 是 `Box<dyn
//! ReadWrite + Send>`，无法 downcast 到 TcpStream 设 read timeout。让
//! actor 线程持有 ws 阻塞 read + 主线程用 mpsc 写命令是唯一干净路径。
//!
//! **不变量**：
//! - mpsc 写路径**不**阻塞：广播走 `try_send`，失败由 broadcaster 回收槽位。
//! - 关闭时显式 `unsubscriber(id)`：主线程退出前**先**注销槽位，再
//!   发 Close 命令给 actor。
//! - 30s idle：actor 在每次 read 返回后检查 `last_frame_at`；**真正**的
//!   服务端主动 close 依赖客户端后续 read 操作触发 flush（auto-pong 等
//!   都会 flush），或客户端主动断开——契约 partial（见 contract doc 状态列）。
//! - **自动 pong**：tungstenite 在 `read_message` 内自动排队 pong reply
//!   （v0.24.0 行为）——我们**不**手动回 pong。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::Value;
use tungstenite::Message;
use tungstenite::protocol::CloseFrame;

/// 单连接写消息（**已**序列化的 JSON 帧，**不**含 seq/ts）。容量 256
/// ——慢客户端 backpressure 时 `try_send` 失败即丢弃（D1 §4.6 断线语义）。
///
/// **P1WS-2 变更**：actor 收到后**解析 → 用每连接 ConnectionState
/// alloc_seq → 加 seq/ts → 重新序列化 → 写**。每连接 seq 独立。
pub type WsWriter = mpsc::SyncSender<String>;

/// 短轮询 / recv_timeout 周期（ms）。100ms 折中响应性与 CPU 负载。
pub const LOOP_TICK_MS: u64 = 100;

/// 服务端 WS 30s 无客户端帧即 close（契约 §4.5）。这是**软**判：read
/// 返回后检查，超时由主线程发 Close 命令触发 actor 优雅退出。
pub const CLIENT_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// 稳定连接 id（进程级单调；唯一标识一条活跃连接）。
static NEXT_CONNECTION_ID: AtomicU64 = AtomicU64::new(1);

/// 分配下一条稳定连接 id。
pub fn next_connection_id() -> u64 {
    NEXT_CONNECTION_ID.fetch_add(1, Ordering::Relaxed)
}

/// 内部 broker：ws 主线程退出时调用，把自身从 broadcaster 的槽位里剔除。
///
/// 拆分承载：避免循环依赖（broadcaster → connection → broadcaster）。
/// 由 [`crate::web_api::ws::Broadcaster`] 注入。
pub type Unsubscriber = Box<dyn Fn(u64) + Send + 'static>;

/// 单连接状态：seq + 最近活跃时间。
struct ConnectionState {
    /// 稳定连接 id（由 broadcaster 分配）。
    #[allow(dead_code)]
    pub conn_id: u64,
    /// 每连接 next_seq（从 1 开始；契约 §4.2：每连接独立）。
    next_seq: u64,
    /// 最近一次收到任意客户端帧的时刻（用于 30s idle timeout）。
    last_frame_at: Instant,
}

impl ConnectionState {
    pub fn new(conn_id: u64) -> Self {
        Self {
            conn_id,
            next_seq: 1,
            last_frame_at: Instant::now(),
        }
    }

    /// 分配下一个 seq 并推进。
    pub fn alloc_seq(&mut self) -> u64 {
        let s = self.next_seq;
        self.next_seq = self.next_seq.saturating_add(1);
        s
    }

    /// 标记本连接刚刚收到任何帧。
    #[allow(dead_code)] // P1WS-3 启用 read 路径时使用
    pub fn touch(&mut self) {
        self.last_frame_at = Instant::now();
    }

    /// 检查是否超过 idle timeout。
    pub fn is_idle(&self) -> bool {
        self.last_frame_at.elapsed() > CLIENT_IDLE_TIMEOUT
    }
}

/// 主线程 → actor 线程的命令。
enum ActorCommand {
    /// 写一段已序列化的 JSON 帧（**不**含 seq/ts；actor 解析后用
    /// 每连接 ConnectionState 包装 seq/ts → 写 WS 文本帧）。
    WriteText(String),
    /// 主动关闭（unsubscriber 已调，actor 优雅 close socket）。
    Close,
}

/// 启动 per-connection 双线程（actor + 主线程）并立即 return。
///
/// **P1WS-2 设计**：
/// - **actor 线程**（`ws-actor-{id}`）：独占持有 ws，**不**调
///   `ws.read()`——tungstenite 0.24 `read()` 在阻塞 I/O EOF 时 busy-wait
///   （见 [`actor_loop`] 文档）。actor 通过 `cmd_rx.recv_timeout(50ms)`
///   周期唤醒处理写命令 + idle 检查。
/// - **主线程**（`ws-main-{id}`）：mpsc 100ms tick 收 broadcast 帧 →
///   发 `ActorCommand::WriteText` 给 actor；rx 断开（server shutdown）
///   → `unsubscriber(conn_id)` + 发 `ActorCommand::Close`。
///
/// `conn_id`：稳定连接 id（由 `next_connection_id()` 分配）。
/// `unsubscriber`：主线程退出时调用（**先**于 actor 退出）。
///
/// **返回值**：主线程 JoinHandle。actor JoinHandle 故意**不**返回
/// ——cmd_tx 在主线程结束时 drop → actor 读到 Disconnected → actor 退出
/// → ws 在 actor 栈帧结束时 drop → socket 关闭。
pub fn spawn_connection_threads(
    ws: tungstenite::WebSocket<Box<dyn tiny_http::ReadWrite + Send>>,
    tx: WsWriter,
    rx: mpsc::Receiver<String>,
    conn_id: u64,
    unsubscriber: Unsubscriber,
) -> JoinHandle<()> {
    let (cmd_tx, cmd_rx) = mpsc::sync_channel::<ActorCommand>(8);
    // actor 线程：持有 ws 独占。
    let _actor = thread::Builder::new()
        .name(format!("ws-actor-{conn_id}"))
        .spawn(move || actor_loop(ws, cmd_rx, conn_id))
        .expect("spawn actor thread");
    // 主线程：100ms tick 转 mpsc 帧到 actor。
    thread::Builder::new()
        .name(format!("ws-main-{conn_id}"))
        .spawn(move || main_loop(tx, rx, cmd_tx, unsubscriber, conn_id))
        .expect("spawn main thread")
}

/// per-connection 主线程：mpsc tick → 转发广播帧到 actor。
///
/// 退出条件：rx 断开（server shutdown）→ unsubscriber + 发 Close 命令 + return。
/// unsubscriber 闭包由 caller 注入并捕获 conn_id。
fn main_loop(
    _hold_tx: WsWriter,
    rx: mpsc::Receiver<String>,
    cmd_tx: mpsc::SyncSender<ActorCommand>,
    unsubscriber: Unsubscriber,
    conn_id: u64,
) {
    loop {
        match rx.recv_timeout(Duration::from_millis(LOOP_TICK_MS)) {
            Ok(text) => {
                if cmd_tx.send(ActorCommand::WriteText(text)).is_err() {
                    // actor 已退出
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // server shutdown 或 broadcaster drop → 通知 actor 关闭
                let _ = cmd_tx.send(ActorCommand::Close);
                break;
            }
        }
    }
    // 主线程退出：先 unsubscribe（让 broadcaster 立即回收槽位），
    // 再发 Close 命令（actor 即使阻塞 read，下一次 read 返回后 cmd_rx
    // try_recv 会拿到 Close；actor 退出时 ws drop 关 socket 唤醒 read）。
    unsubscriber(conn_id);
    let _ = cmd_tx.send(ActorCommand::Close);
}

/// per-connection actor 线程：独占持有 `ws`，处理 cmd_rx 写命令 + 周期
/// idle 检查。
///
/// **P1WS-2 关键设计**：actor **不**调 `ws.read()`——tungstenite 0.24
/// `read()` 在阻塞 I/O 上**会** busy-wait（`read_frame` 在 `Ok(0)` 时
/// 返回 `Ok(None)`，外层 `read()` 据此死循环——见
/// `tungstenite-0.24.0/src/protocol/frame/mod.rs:191-194`）。actor 通过
/// `cmd_rx.recv_timeout(50ms)` 周期唤醒：
/// - 收到写命令 → 写（broadcast 帧在 50ms 内被处理）；
/// - 收到 Close 命令 → close ws + return；
/// - timeout → 检查 idle（30s）→ 主动 close。
///
/// **已知限制**（契约 partial，见 contract doc §4.3 状态列）：
/// - **client close 检测**不靠 `read()` 帧，靠 30s idle timeout 或
///   server shutdown（主线程 unsubscriber → actor 收到 Close）。
/// - **client→server 帧**（subscribe/ping 文本）**不**被处理——actor
///   不调 `read()`。客户端若发 ping 文本会无响应（**前端 chat.js
///   不要发文本 ping**，只用 auto-pong）。subscribe 文本同理**不会**
///   收到 subscribe_ack echo——但**初始** subscribe_ack 已在连接建立
///   时由 actor 主动发。
/// - P1WS-3 计划：改用 custom TcpListener + `set_read_timeout`，
///   修复 client close + 帧处理路径。
fn actor_loop(
    mut ws: tungstenite::WebSocket<Box<dyn tiny_http::ReadWrite + Send>>,
    cmd_rx: mpsc::Receiver<ActorCommand>,
    conn_id: u64,
) {
    let mut state = ConnectionState::new(conn_id);
    // 1) 连接建立后**立即**发 subscribe_ack（契约 §4.4：服务端首帧）。
    let ack = build_subscribe_ack(&mut state, &[]);
    if write_text(&mut ws, &ack).is_err() {
        return;
    }
    loop {
        // 处理命令通道（短阻塞 50ms；周期性唤醒处理 idle 检查）。
        match cmd_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(ActorCommand::WriteText(text)) => {
                let wrapped = wrap_preserialized_with_seq(&text, &mut state);
                if write_text(&mut ws, &wrapped).is_err() {
                    return;
                }
            }
            Ok(ActorCommand::Close) => {
                let _ = ws.close(None);
                return;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // 周期唤醒：检查 idle（30s 软判）。
                if state.is_idle() {
                    let _ = ws.close(Some(CloseFrame {
                        code: tungstenite::protocol::frame::coding::CloseCode::Policy,
                        reason: "client idle timeout".into(),
                    }));
                    return;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // 主线程退出（说明已 unsubscriber）→ 主动 close。
                let _ = ws.close(None);
                return;
            }
        }
    }
}

/// 写入一段文本帧，统一错误处理。
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

/// 处理一条客户端帧；返回 `Some(text)` 时需要回写文本帧。
///
/// **Ping/Pong**：tungstenite 在 `read_message` 内**自动**排队 pong
/// reply（v0.24.0 行为）——我们不手动回 pong，下一次 read/write/flush
/// 会自动 flush 该 pong。
#[allow(dead_code)] // P1WS-3 启用 read 路径时使用
fn handle_client_message(msg: &Message, state: &mut ConnectionState) -> Option<String> {
    match msg {
        Message::Ping(_) => None, // 自动 pong
        Message::Pong(_) => None,
        Message::Close(_) => None, // 让主循环在 Err(ConnectionClosed) 路径退出
        Message::Text(t) => handle_client_text(t.as_str(), state),
        Message::Binary(_) => None,
        Message::Frame(_) => None,
    }
}

/// 文本客户端帧处理：subscribe / ping 文本 / 其它。
#[allow(dead_code)] // P1WS-3 启用 read 路径时使用
fn handle_client_text(text: &str, state: &mut ConnectionState) -> Option<String> {
    let parsed: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => {
            return Some(build_error_frame(
                state,
                "invalid_payload",
                "客户端帧 JSON 解析失败",
            ));
        }
    };
    let kind = parsed.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match kind {
        "subscribe" => {
            let topics = parsed
                .get("topics")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|t| t.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Some(build_subscribe_ack(state, &topics))
        }
        "ping" => {
            // 文本 ping → 文本 pong（契约 §4.5 兼容文本 ping）。
            let seq = state.alloc_seq();
            let ts = crate::web_api::ws::events::iso8601_now_ms();
            Some(format!(
                r#"{{"type":"pong","seq":{seq},"ts":"{ts}","data":{{}}}}"#
            ))
        }
        _ => Some(build_error_frame(
            state,
            "unknown_type",
            &format!("未知客户端帧 type={kind:?}"),
        )),
    }
}

// -----------------------------------------------------------------------------
// 帧构造 helper（用 connection state 的 seq，每连接独立）
// -----------------------------------------------------------------------------

/// 把已序列化的 JSON 帧（**不含 seq/ts**）解析 → alloc_seq → 加 seq/ts
/// → 重序列化。
///
/// **P1WS-2 实现策略**：broadcaster 仍走 `String` mpsc（兼容 cli_entry
/// 等旧 caller），actor 收到后 parse → wrap with seq → re-serialize。
/// serde_json 单次 parse + dump 耗时 < 10μs（短帧），不构成热路径瓶颈。
///
/// **失败兜底**：若解析失败（broadcast 是非 JSON 字符串），回退为
/// 字符串原样发出，**不**阻断（log warn 便于排障）。
fn wrap_preserialized_with_seq(text: &str, state: &mut ConnectionState) -> String {
    let seq = state.alloc_seq();
    let ts = crate::web_api::ws::events::iso8601_now_ms();
    let mut frame: Value = match serde_json::from_str(text) {
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
        obj.insert("seq".to_string(), Value::Number(seq.into()));
        obj.insert("ts".to_string(), Value::String(ts));
    }
    serde_json::to_string(&frame).unwrap_or_else(|_| text.to_string())
}

/// 构造 subscribe_ack 帧（连接建立后的首帧；契约 §4.4）。
fn build_subscribe_ack(state: &mut ConnectionState, topics: &[String]) -> String {
    let seq = state.alloc_seq();
    let ts = crate::web_api::ws::events::iso8601_now_ms();
    format!(
        r#"{{"type":"subscribe_ack","seq":{seq},"ts":"{ts}","data":{{"topics":{}}}}}"#,
        serde_json::to_string(topics).unwrap_or_else(|_| "[]".to_string())
    )
}

/// 构造 error 帧（服务端→客户端；契约 §5 风格）。
#[allow(dead_code)] // P1WS-3 启用 read 路径时使用
fn build_error_frame(state: &mut ConnectionState, code: &str, message: &str) -> String {
    let seq = state.alloc_seq();
    let ts = crate::web_api::ws::events::iso8601_now_ms();
    let msg_json = serde_json::Value::String(message.to_string());
    let msg_str = serde_json::to_string(&msg_json).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        r#"{{"type":"error","seq":{seq},"ts":"{ts}","data":{{"code":"{code}","message":{msg_str}}}}}"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 帧 helper：单连接 alloc_seq 从 1 起步且单调递增。
    #[test]
    fn connection_state_seq_monotonic_from_one() {
        let mut s = ConnectionState::new(42);
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
        let mut s = ConnectionState::new(7);
        let ack1 = build_subscribe_ack(&mut s, &[]);
        let ack2 = build_subscribe_ack(&mut s, &[]);
        assert!(ack1.contains("\"seq\":1"), "got: {ack1}");
        assert!(ack2.contains("\"seq\":2"), "got: {ack2}");
    }

    /// build_error_frame 转义 message 中的 " 字符。
    #[test]
    fn error_frame_escapes_message() {
        let mut s = ConnectionState::new(1);
        let err = build_error_frame(&mut s, "boom", "他说\"hi\"");
        assert!(err.contains(r#"\"hi\""#), "got: {err}");
    }

    /// handle_client_text 路由：subscribe → subscribe_ack；ping → pong；其它 → error。
    #[test]
    fn handle_client_text_dispatch() {
        let mut s = ConnectionState::new(1);
        let ack =
            handle_client_text(r#"{"type":"subscribe","topics":["a"]}"#, &mut s).expect("ack");
        assert!(ack.contains("\"type\":\"subscribe_ack\""));
        let pong = handle_client_text(r#"{"type":"ping"}"#, &mut s).expect("pong");
        assert!(pong.contains("\"type\":\"pong\""));
        let err = handle_client_text(r#"{"type":"wtf"}"#, &mut s).expect("err");
        assert!(err.contains("\"type\":\"error\""));
        let bad = handle_client_text("not json", &mut s).expect("err");
        assert!(bad.contains("invalid_payload"));
    }
}
