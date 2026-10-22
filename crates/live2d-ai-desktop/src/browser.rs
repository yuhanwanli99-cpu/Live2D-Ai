//! Windows 包：监听成功后用系统默认浏览器打开启动器页（0.4.0 rc.1）。
//!
//! 只在 **Windows 目标 + 生产 `--web` 进程**里动作：
//! - 非 Windows 不编译本模块——Linux 仍由 `scripts/ignite.sh` 打开，服务自己
//!   不再开一次；
//! - 测试 / 库调用不 [`arm`]（与 `web_api::restart` 同一条纪律）：否则 e2e
//!   测试在 Windows 上跑时会弹浏览器。
//!
//! 打开方式 = 系统 `cmd /C start`（`std::process::Command`，不新增依赖）。
//! 用 `start "" <url>`：`start` 会把**第一个带引号的参数**当成窗口标题，
//! 空标题才能保证 URL 被当成目标。

use std::sync::atomic::{AtomicBool, Ordering};

/// 进程级开关：只有生产 `--web` 启动路径会打开它。
static ARMED: AtomicBool = AtomicBool::new(false);

/// 打开「监听成功后自开浏览器」（只在真服务进程的启动路径上调一次）。
pub fn arm() {
    ARMED.store(true, Ordering::SeqCst);
}

/// 监听成功后用默认浏览器打开启动器页（未 arm → 什么都不做）。
pub fn open_launcher_after_listen(port: u16) {
    if !ARMED.load(Ordering::SeqCst) {
        return;
    }
    let url = crate::host_layout::launcher_url(port);
    match std::process::Command::new("cmd")
        .args(["/C", "start", "", &url])
        .spawn()
    {
        Ok(child) => {
            tracing::info!(pid = child.id(), %url, "已用系统默认浏览器打开启动器页");
        }
        Err(err) => {
            tracing::warn!(%url, "打开默认浏览器失败（{err}）；请手动访问该地址");
        }
    }
}
