//! 配置文件热重载文件监听器（2026-08-31）。
//!
//! 用 `notify` 跨平台监听 `live2d-ai.toml`（**以及同目录的 `.env`**）的外部修改
//! （如用户手动编辑），检测到变更后触发 supervisor reload，实现真正的热重载。
//!
//! 2026-09-12（rc.2）：监视集加入 `.env`。密钥的真源是 `.env`（见
//! `live2d_ai_runtime::secrets`），用户手改它之后必须能生效；否则「改完 key
//! 还是 401」会以另一种形式回来。`post_reload` 钩子因此要**同时**刷新设置快照
//! 与密钥快照（调用方负责，见 `cli_entry`）。
//!
//! 设计：
//! - 后台线程跑 `notify` 事件循环（`notify::recommended_watcher`）。
//! - 监听配置文件**父目录**（notify 只能监听目录），过滤出目标文件事件。
//! - 防抖：事件触发后等待 `debounce_ms`，期间的事件合并为一次 reload。
//! - 通过 `SupervisorHandle::reload()` 触发热重载（线程安全：原子标志 + channel）。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::supervisor::SupervisorHandle;

/// 文件监听器句柄（drop 时停止监听线程）。
pub struct FileWatcher {
    /// 停止标志：true = 线程退出。
    stop_flag: Arc<AtomicBool>,
    /// 监听线程 join handle（drop 时 join）。
    handle: Option<std::thread::JoinHandle<()>>,
}

/// 外部修改被接受后要做的**副作用**（在 reload 之后、防抖之前调用）。
///
/// 存在的理由：`supervisor.reload()` 只重建 LLM/TTS client，**不**刷新
/// `StatusContext` 里的设置快照——那会让「界面上显示的值」与「磁盘上的值」
/// 静默分叉（详见 `StatusContext::refresh_from_disk` 的说明）。
pub type PostReloadHook = Box<dyn Fn() + Send>;

/// **惰性** supervisor 来源：每次外部修改发生时**现取**当前句柄。
///
/// 为什么不是直接持 `Arc<SupervisorHandle>`：第一次运行（`live2d-ai.toml` 尚不存在）
/// 时槽位是空的——而用户正是在这个窗口里建配置、写 `.env`。以前干脆不装 watcher，
/// 于是这个窗口里改 `.env` 要等下次启动才生效，且没有任何提示（F-0002-01）。
/// 改成「装 watcher + 事件发生时现取」之后，槽位稍后被 PATCH 路径动态装配
/// （`SupervisorSlot::ensure_after_patch`）也能被同一条监听看见。
pub type SupervisorSource = Box<dyn Fn() -> Option<Arc<SupervisorHandle>> + Send + 'static>;

impl FileWatcher {
    /// 监听 `config_path`（及同目录 `.env`）的外部修改，检测到变更时调用
    /// `supervisor.reload()`。
    ///
    /// `debounce_ms`：防抖窗口（毫秒），避免编辑器一次保存触发多次 reload。
    /// `post_reload`：reload 之后的额外副作用（刷新设置快照 / 密钥快照）；
    /// `None` = 只 reload。
    pub fn watch_config(
        config_path: impl AsRef<Path>,
        supervisor: SupervisorSource,
        debounce_ms: u64,
        post_reload: Option<PostReloadHook>,
    ) -> Result<Self, String> {
        let config_path = config_path.as_ref().to_path_buf();
        // notify 监听目录，所以取父目录。
        let watch_dir = config_path
            .parent()
            .ok_or_else(|| format!("配置文件无父目录: {}", config_path.display()))?
            .to_path_buf();
        // 监视集：配置文件 + 同目录的 `.env`（密钥真源，rc.2）。
        // 两者都在仓库根，`notify` 又只能监听目录，所以一个目录 + 两个文件名。
        let mut targets = vec![
            config_path
                .file_name()
                .ok_or_else(|| format!("配置文件无文件名: {}", config_path.display()))?
                .to_os_string(),
        ];
        targets.push(std::ffi::OsString::from(".env"));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();

        let handle = std::thread::Builder::new()
            .name("config-file-watcher".into())
            .spawn(move || {
                Self::run(
                    watch_dir,
                    targets,
                    supervisor,
                    stop_flag_clone,
                    debounce_ms,
                    post_reload,
                );
            })
            .map_err(|e| format!("启动文件监听线程失败: {e}"))?;

        Ok(Self {
            stop_flag,
            handle: Some(handle),
        })
    }

    fn run(
        watch_dir: PathBuf,
        targets: Vec<std::ffi::OsString>,
        supervisor: SupervisorSource,
        stop_flag: Arc<AtomicBool>,
        debounce_ms: u64,
        post_reload: Option<PostReloadHook>,
    ) {
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let mut watcher = match notify::recommended_watcher(move |res: Result<notify::Event, _>| {
            if let Ok(event) = res {
                // 过滤：只关心监视集里的文件的写入/创建/重命名事件。
                if event.paths.iter().any(|p| {
                    p.file_name()
                        .is_some_and(|n| targets.iter().any(|t| t == n))
                }) {
                    match event.kind {
                        notify::EventKind::Modify(_) | notify::EventKind::Create(_) => {
                            let _ = tx.send(());
                        }
                        _ => {}
                    }
                }
            }
        }) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!(error = %e, "创建 notify watcher 失败");
                return;
            }
        };
        if let Err(e) = notify::Watcher::watch(
            &mut watcher,
            &watch_dir,
            notify::RecursiveMode::NonRecursive,
        ) {
            tracing::error!(error = %e, dir = %watch_dir.display(), "监听目录失败");
            return;
        }

        let debounce = Duration::from_millis(debounce_ms);
        let mut pending = false;
        loop {
            if stop_flag.load(Ordering::Relaxed) {
                break;
            }
            // 等待事件或超时（100ms 轮询停止标志）。
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(()) => {
                    pending = true;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // 超时：检查是否需要触发 reload。
                    if pending {
                        pending = false;
                        on_config_changed(&supervisor, post_reload.as_ref());
                        // 防抖：等待期间忽略新事件。
                        std::thread::sleep(debounce);
                        // 清空可能累积的事件。
                        while rx.try_recv().is_ok() {}
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        // Drop watcher 自动停止监听。
        drop(watcher);
    }
}

/// 外部修改被接受后要做的动作（从 `run` 的事件循环里抽出来，**可单测**）。
///
/// 返回是否真的触发了 reload：
/// - 槽位里有 supervisor → `reload()` + 返回 `true`；
/// - **首跑窗口**（配置刚建、supervisor 还没装配）→ 只刷新快照并返回 `false`
///   （F-0002-01：以前这种情况下 watcher 压根没装，改 `.env` 要等下次启动）。
fn on_config_changed(supervisor: &SupervisorSource, post_reload: Option<&PostReloadHook>) -> bool {
    let reloaded = match supervisor() {
        Some(sup) => {
            tracing::info!("配置文件外部修改 detected，触发 supervisor reload");
            sup.reload();
            true
        }
        None => {
            tracing::info!(
                "配置文件外部修改 detected，但当前没有 supervisor（首次配置窗口）——快照照常刷新，reload 待下次启动"
            );
            false
        }
    };
    // **快照也要刷新**（2026-09-11 修）：只 reload client 会让
    // `GET /api/v1/settings` 与磁盘分叉，且用户手改的值会在下一次界面
    // 「保存」时被旧快照覆盖回去。没有 supervisor 时这一步**照样要做**——
    // 首跑窗口里改的 `.env` 至少必须立刻进快照（F-0002-01）。
    if let Some(hook) = post_reload {
        hook();
    }
    reloaded
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;

    /// F-0002-01：**首跑窗口**（槽位空）下，外部修改仍必须刷新快照，只是不 reload。
    ///
    /// 这条测试就是那个缺陷的守门人：把 `on_config_changed` 改回「没有 supervisor
    /// 就整个跳过」（= 旧行为：干脆不装 watcher），它必须变红。
    ///
    /// 有 supervisor 的那条分支这里**不重复**覆盖：它只是「调一次
    /// `SupervisorHandle::reload`」，reload 语义的回归在
    /// `supervisor/tests_reload.rs`；本文件新增的契约只有「空槽位也要刷新快照」。
    #[test]
    fn on_config_changed_without_supervisor_still_refreshes_snapshots() {
        let calls = Arc::new(AtomicU64::new(0));
        let calls_in_hook = Arc::clone(&calls);
        let source: SupervisorSource = Box::new(|| None);
        let hook: PostReloadHook = Box::new(move || {
            calls_in_hook.fetch_add(1, Ordering::SeqCst);
        });

        assert!(
            !on_config_changed(&source, Some(&hook)),
            "没有 supervisor 时不得报告「已 reload」"
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "首跑窗口也必须刷新设置/密钥快照（F-0002-01 的修复点）"
        );
    }
}
