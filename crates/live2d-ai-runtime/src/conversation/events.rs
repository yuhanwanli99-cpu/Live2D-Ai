//! 事件投递辅助：与取消竞争；接收端已关闭或已取消 → `false`（调用方据此停机）。
//!
//! 终态事件（`Terminal`）走 [`send_terminal`]，有独立的有界兜底投递
//! （`TERMINAL_SEND_TIMEOUT`），supervisor 的权威终态是 [`super::TurnReport`]，
//! 不依赖本事件送达。

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::EngineEvent;
use super::TERMINAL_SEND_TIMEOUT;

/// 向外发一个普通事件：与取消竞争；接收端已关闭或已取消 → `false`（调用方据此停机）。
pub(super) async fn send_event(
    event_tx: &mpsc::Sender<EngineEvent>,
    cancel: &CancellationToken,
    event: EngineEvent,
) -> bool {
    tokio::select! {
        _ = cancel.cancelled() => false,
        sent = event_tx.send(event) => sent.is_ok(),
    }
}

/// 发送终态事件：消费端卡死时最多等 [`TERMINAL_SEND_TIMEOUT`]，之后放弃——
/// supervisor 的权威终态是 [`super::TurnReport`]，不依赖本事件送达。
pub(super) async fn send_terminal(event_tx: &mpsc::Sender<EngineEvent>, event: EngineEvent) {
    let _ = tokio::time::timeout(TERMINAL_SEND_TIMEOUT, event_tx.send(event)).await;
}
