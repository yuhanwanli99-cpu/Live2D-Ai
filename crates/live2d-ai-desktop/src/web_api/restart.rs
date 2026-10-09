//! 设置保存的「必须重启」落点（2026-10-09）。
//!
//! # 为什么需要它
//!
//! 计划书 §3：设置保存要做成**一步**——写盘、能热重载就热重载、必须重启就
//! **只重启 18080 上的 `live2d-ai-desktop`**，然后前端刷新 `/app/`。
//! 从前 PATCH 回来一个 `restart_required` 就完事了，界面弹一句「已保存，
//! 需重启生效」，用户只能自己回去重新点火。
//!
//! # 它做什么 / 不做什么
//!
//! - 只重启**本进程**：用 `current_exe` + 原 argv + 原 cwd 起一个替身，
//!   然后自己 `exit`。**不动别的端口**（8091 上的 MeloTTS 是另一个进程，
//!   见下一条）；
//! - 退出前先调 `before_exit`（生产传 `ModRegistry::shutdown_all`）：
//!   `std::process::exit` **不跑析构**，子进程会被留成孤儿，新进程再拉一个
//!   就撞端口。「已有子进程就不再起一个」只在本进程内有效，所以这里必须
//!   显式停干净，让新进程**自己拉一个**；
//! - 替身起不来（`spawn` 失败）→ **不退出**，旧进程继续服务，只记 error：
//!   「重启失败还把服务关了」比不重启更坏；
//! - 替身拿端口：父进程要等自己 `exit` 才释放，所以给替身带上
//!   [`RESTART_WAIT_ENV`]，`start_server` 据此在启动时**重试绑定**。

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// **自重启开关**（进程级）。
///
/// 只有真服务进程（`cli_entry::run_web_mode`）会 [`arm`] 它。测试与库调用默认
/// **关**——否则任何「dispatch 走到 `RestartRequired`」的单测都会去 spawn
/// 一次 `current_exe()`（在测试里那就是**测试二进制自己**），于是测试进程
/// 无限自我复制。这不是假想：第一版就踩了，跑 `cargo test --workspace` 时
/// 看见同一个测试二进制被反复拉起。
static ARMED: AtomicBool = AtomicBool::new(false);

/// 打开自重启（只在真服务进程启动路径上调一次）。
pub fn arm() {
    ARMED.store(true, Ordering::SeqCst);
}

/// 当前进程是否允许自重启。
pub fn is_armed() -> bool {
    ARMED.load(Ordering::SeqCst)
}

/// 替身进程的重试绑定窗口（毫秒）。
///
/// 由 [`spawn_replacement_after`] 注入；`start_server` 读到它时会在
/// 「bind 失败」上重试到窗口耗尽——正常启动（无此变量）仍然**立刻失败**，
/// 「端口被别的程序占了」不会被这里掩饰成慢启动。
pub const RESTART_WAIT_ENV: &str = "LIVE2D_AI_RESTART_WAIT_MS";

/// 默认窗口：够父进程 `exit` + 内核回收监听 socket。
pub const RESTART_WAIT_MS: u64 = 8000;

/// 起一个替身（原 exe + 原 argv + 原 cwd），随后**退出本进程**。
///
/// `before_exit` 在退出前、`spawn` 之后调用一次（生产 = 停掉所有 Mod 子进程）。
/// `delay` 是给调用方把 HTTP 响应写完的时间。
///
/// 返回 `()`：真正的退出发生在后台线程里（本函数立刻返回，handler 才能
/// 正常把 200 + `apply_status` 写回浏览器）。
pub fn spawn_replacement_after(delay: Duration, before_exit: impl FnOnce() + Send + 'static) {
    let exe = std::env::current_exe();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cwd = std::env::current_dir();
    let spawned = std::thread::Builder::new()
        .name("self-restart".into())
        .spawn(move || {
            std::thread::sleep(delay);
            let Ok(exe) = exe else {
                tracing::error!("自重启放弃：拿不到 current_exe，旧进程继续服务");
                return;
            };
            let mut cmd = std::process::Command::new(&exe);
            cmd.args(&args);
            if let Ok(dir) = cwd {
                cmd.current_dir(dir);
            }
            cmd.env(RESTART_WAIT_ENV, RESTART_WAIT_MS.to_string());
            match cmd.spawn() {
                Ok(child) => {
                    tracing::info!(pid = child.id(), "自重启：替身已起，本进程退出");
                    // 先停干净 Mod 子进程（不留孤儿、不让新进程抢端口），
                    // 再退出——`exit` 不跑析构，这一步不能省。
                    before_exit();
                    std::process::exit(0);
                }
                Err(e) => {
                    tracing::error!("自重启失败（{e}）：旧进程继续服务，不做任何回滚");
                }
            }
        });
    if spawned.is_err() {
        tracing::error!("自重启放弃：起不了后台线程，旧进程继续服务");
    }
}
