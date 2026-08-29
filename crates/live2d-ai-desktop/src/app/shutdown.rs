//! 关机 handshake 与兜底 watchdog。
//!
//! - [`ShellApp::request_shutdown`]：统一入口（D13）——`/quit`、`CloseRequested`、
//!   `TrayExit` 共用；chat 模式向 supervisor 发 Quit 并启动 500ms 兜底计时，
//!   冒烟模式保留原语义直接退出。
//! - `tick_shutdown_watchdog`：`about_to_wait` 钩子里检查 `quitting_since` 是否
//!   超过 [`SHUTDOWN_WATCHDOG`]；超过即警告强退。

use std::time::Instant;

use tracing::{info, warn};
use winit::event_loop::ActiveEventLoop;

use crate::app::types::{SHUTDOWN_WATCHDOG, ShellApp};

impl ShellApp {
    /// 统一关机入口（D13）：`/quit`、`CloseRequested`、`TrayExit` 共用。
    ///
    /// - chat 模式：向 supervisor 发 Quit 并启动 500ms 兜底计时；
    ///   收到 [`AppEvent::ShutdownReady`] 即刻退出，超时则警告强退；
    /// - 冒烟模式：保留原语义直接退出（无 supervisor 可等）。
    pub(crate) fn request_shutdown(&mut self, event_loop: &ActiveEventLoop) {
        if self.quitting_since.is_some() {
            return; // 幂等：重复请求不重启计时
        }
        match &self.chat {
            Some(bridge) => {
                info!("请求关机：等待 supervisor 停机 handshake（上限 500ms）");
                self.quitting_since = Some(Instant::now());
                bridge.supervisor.quit();
            }
            None => event_loop.exit(),
        }
    }

    /// shutdown handshake 兜底（D13）：supervisor 500ms 内未回
    /// `ShutdownReady` 即警告强退——绝不让用户面对无响应的关闭按钮。
    ///
    /// 返回 `true` 表示已触发强退（事件循环已 `exit`），调用方应放弃后续
    /// 兜底逻辑直接 `return`。
    pub(crate) fn tick_shutdown_watchdog(&mut self, event_loop: &ActiveEventLoop) -> bool {
        if let Some(t) = self.quitting_since
            && t.elapsed() > SHUTDOWN_WATCHDOG
        {
            warn!("supervisor 停机超过 500ms，强制退出事件循环");
            event_loop.exit();
            return true;
        }
        false
    }
}
