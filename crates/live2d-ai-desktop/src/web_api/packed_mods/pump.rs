//! 外部注入队列泵（**仅打包临时树**，0.4.6）。
//!
//! `POST /api/v1/external/chat` 只负责把一条原文 `push` 进
//! [`crate::web_api::external_queue::ExternalQueue`]；本线程负责在 ready 时
//! **一次 pump 至多一条**交给 `supervisor.say`：
//!
//! - ready = supervisor 存在 **且** 本轮生成已结束（`!turn_in_flight`）**且**
//!   播放不忙（`!playback_gate.is_busy()`）；
//! - 一条原文一次 say，不拼接、不重排；`say` 返回 false 时队头留着下次再试；
//! - 没发出去（未 ready / 队列空 / say 被拒）短睡 20 ms，避免空转；
//! - 服务停机时 `stop_and_join` 唤醒并 join，不留线程跨实例存活。
//!
//! 缺省树（`feature = "packed-mods"` 关）不编译本文件、也不起这个线程——
//! 队列在缺省 binary 里没有调用方（`external_routes` 仍是 404 存根）。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use crate::web_api::ServerContext;
use crate::web_api::external_queue::PumpOutcome;

/// 泵线程句柄：停机时置停止位并 join。
pub struct PumpHandle {
    stop: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<()>>,
}

impl PumpHandle {
    /// 通知泵线程退出并等它结束（服务 `incoming_requests` 循环结束后调用）。
    pub fn stop_and_join(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// 起常驻泵线程（每个 `run_request_loop` 一个）。
pub fn spawn(ctx: ServerContext) -> PumpHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    let join = thread::Builder::new()
        .name("external-queue-pump".to_string())
        .spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                let supervisor = ctx.try_get_supervisor();
                let ready = supervisor.as_ref().is_some_and(|s| !s.turn_in_flight())
                    && !ctx.playback_gate.is_busy();
                let outcome = ctx.external_queue.pump(ready, |text| {
                    supervisor.as_ref().is_some_and(|s| s.say(text.to_string()))
                });
                // 没发出去（未 ready / 队列空 / say 被拒）就短睡，避免空转。
                if outcome != PumpOutcome::Said {
                    thread::sleep(Duration::from_millis(20));
                }
            }
        })
        .expect("external-queue-pump 线程创建失败");
    PumpHandle {
        stop,
        join: Some(join),
    }
}
