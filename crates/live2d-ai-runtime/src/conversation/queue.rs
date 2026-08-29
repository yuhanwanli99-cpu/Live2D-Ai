//! 句子入队与背压处理：满则等待（背压），可选超时升级为
//! [`super::ErrorKind::Backpressure`]；取消随时打断。

use std::time::Duration;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::worker::TtsJob;

/// 句子入队结果。
pub(super) enum PushOutcome {
    /// 已进入有界队列。
    Accepted,
    /// 队列已关闭：TTS worker 已异常退出（其 Error 事件已自行发出）。
    QueueDead,
    /// 取消或事件消费者消失：安静收场。
    Stopped,
    /// 背压超限：队列长期满且未被消费。
    TimedOut(String),
}

/// 把一个句子推进有界队列：满则等待（背压），可选超时升级为
/// [`super::ErrorKind::Backpressure`]；取消随时打断。
pub(super) async fn push_job(
    job_tx: &mpsc::Sender<TtsJob>,
    cancel: &CancellationToken,
    timeout: Option<Duration>,
    job: TtsJob,
) -> PushOutcome {
    let push = job_tx.send(job);
    tokio::select! {
        _ = cancel.cancelled() => PushOutcome::Stopped,
        sent = push => match sent {
            Ok(()) => PushOutcome::Accepted,
            Err(_) => PushOutcome::QueueDead,
        },
        // 仅在配置了超时时参与竞争：等待超过时限仍未入队 → 下游卡死。
        _ = sleep_for(timeout), if timeout.is_some() => PushOutcome::TimedOut(format!(
            "句子在 {timeout:?} 内未能进入 TTS 队列（容量 {}）：下游持续不消费",
            job_tx.max_capacity()
        )),
    }
}

/// `Some(d)` → 睡 d；`None` → 永不完成（供 `select!` 的 `if` 前置条件短路）。
async fn sleep_for(timeout: Option<Duration>) {
    match timeout {
        Some(d) => tokio::time::sleep(d).await,
        None => std::future::pending::<()>().await,
    }
}
