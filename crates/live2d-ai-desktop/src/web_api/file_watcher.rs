//! 配置文件热重载文件监听器（2026-08-31）。
//!
//! 用 `notify` 跨平台监听 `live2d-ai.toml` 的外部修改（如用户手动编辑），
//! 检测到变更后触发 supervisor reload，实现真正的热重载。
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

impl FileWatcher {
    /// 监听 `config_path` 的外部修改，检测到变更时调用 `supervisor.reload()`。
    ///
    /// `debounce_ms`：防抖窗口（毫秒），避免编辑器一次保存触发多次 reload。
    /// `post_reload`：reload 之后的额外副作用（刷新设置快照）；`None` = 只 reload。
    pub fn watch_config(
        config_path: impl AsRef<Path>,
        supervisor: Arc<SupervisorHandle>,
        debounce_ms: u64,
        post_reload: Option<PostReloadHook>,
    ) -> Result<Self, String> {
        let config_path = config_path.as_ref().to_path_buf();
        // notify 监听目录，所以取父目录。
        let watch_dir = config_path
            .parent()
            .ok_or_else(|| format!("配置文件无父目录: {}", config_path.display()))?
            .to_path_buf();
        let target_name = config_path
            .file_name()
            .ok_or_else(|| format!("配置文件无文件名: {}", config_path.display()))?
            .to_os_string();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();

        let handle = std::thread::Builder::new()
            .name("config-file-watcher".into())
            .spawn(move || {
                Self::run(
                    watch_dir,
                    target_name,
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
        target_name: std::ffi::OsString,
        supervisor: Arc<SupervisorHandle>,
        stop_flag: Arc<AtomicBool>,
        debounce_ms: u64,
        post_reload: Option<PostReloadHook>,
    ) {
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let mut watcher = match notify::recommended_watcher(move |res: Result<notify::Event, _>| {
            if let Ok(event) = res {
                // 过滤：只关心目标文件的写入/创建/重命名事件。
                if event
                    .paths
                    .iter()
                    .any(|p| p.file_name().map(|n| n == target_name).unwrap_or(false))
                {
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
                        tracing::info!("配置文件外部修改 detected，触发 supervisor reload");
                        supervisor.reload();
                        // **快照也要刷新**（2026-09-11 修）：只 reload client 会让
                        // `GET /api/v1/settings` 与磁盘分叉，且用户手改的值会在
                        // 下一次界面「保存」时被旧快照覆盖回去。
                        if let Some(hook) = &post_reload {
                            hook();
                        }
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

impl Drop for FileWatcher {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}
