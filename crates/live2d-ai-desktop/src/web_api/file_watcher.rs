//! 配置文件热重载文件监听器（2026-08-31）。
//!
//! 用 `notify` 跨平台监听 `live2d-ai.toml`（以及 **`.env` 密钥文件**）的外部修改
//! （如用户手动编辑），检测到变更后触发 supervisor reload，实现真正的热重载。
//!
//! 2026-09-12（rc.2）：监视集加入 `.env`。密钥的真源是 `.env`（见
//! `live2d_ai_runtime::secrets`），用户手改它之后必须能生效；否则「改完 key
//! 还是 401」会以另一种形式回来。`post_reload` 钩子因此要**同时**刷新设置快照
//! 与密钥快照（调用方负责，见 `cli_entry`）。
//!
//! 2026-11-19（0.7.0 P1.4）：`.env` **不再**取「配置同目录」——配置可能在
//! exe 目录 / HOME / APPDATA，密钥文件按 `secrets::env_file_path()` 的锚定路径
//! 走，两者可以是不同目录。监听计划因此升级为「目录集合 + (目录, 文件名)」。
//!
//! 设计：
//! - 后台线程跑 `notify` 事件循环（`notify::recommended_watcher`）。
//! - 监听配置文件与密钥文件的**父目录**（notify 只能监听目录），过滤出目标文件事件。
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
///
/// 实参 = **本次事件发生时**槽位里是否真的有 supervisor（由
/// [`on_config_changed`] 现取后传入，不是启动时的静态值）。钩子据此分流：
/// 有 supervisor 的 reload 失败由 `apply_reload` 统一 emit error 帧；
/// **没有** supervisor 时那条出口不存在，钩子自己补发（S-007）。
pub type PostReloadHook = Box<dyn Fn(bool) + Send>;

/// **惰性** supervisor 来源：每次外部修改发生时**现取**当前句柄。
///
/// 为什么不是直接持 `Arc<SupervisorHandle>`：第一次运行（`live2d-ai.toml` 尚不存在）
/// 时槽位是空的——而用户正是在这个窗口里建配置、写 `.env`。以前干脆不装 watcher，
/// 于是这个窗口里改 `.env` 要等下次启动才生效，且没有任何提示（F-0002-01）。
/// 改成「装 watcher + 事件发生时现取」之后，槽位稍后被 PATCH 路径动态装配
/// （`SupervisorSlot::ensure_after_patch`）也能被同一条监听看见。
pub type SupervisorSource = Box<dyn Fn() -> Option<Arc<SupervisorHandle>> + Send + 'static>;

/// 监听计划：`notify` 只能监听目录，所以拆成「目录集合 + (目录, 文件名) 集合」。
///
/// 2026-11-19（0.7.0 P1.4）：配置目录与密钥目录可以不同，不再假设 `.env` 与
/// `live2d-ai.toml` 同目录。
#[derive(Debug, Clone, PartialEq, Eq)]
struct WatchPlan {
    /// 需要监听的目录（去重，按加入顺序）。
    dirs: Vec<PathBuf>,
    /// `(目录, 文件名)`：事件路径必须落在该目录且文件名命中才处理。
    targets: Vec<(PathBuf, std::ffi::OsString)>,
}

impl WatchPlan {
    /// 加入一个 `(目录, 文件名)`；同目录只监听一次，同目标不重复。
    fn push(&mut self, dir: PathBuf, name: std::ffi::OsString) {
        if !self.dirs.contains(&dir) {
            self.dirs.push(dir.clone());
        }
        let target = (dir, name);
        if !self.targets.contains(&target) {
            self.targets.push(target);
        }
    }
}

/// 相对路径没有非空父目录（如裸 `.env`）时用 `.`（= cwd）兜底。
fn non_empty_parent(path: &Path) -> Option<PathBuf> {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
}

/// 由配置路径 + 密钥文件路径算出监听计划（**纯函数**，可单测）。
///
/// 为什么不再硬编码「配置目录的 `.env`」：`.env` 的唯一真源是
/// [`live2d_ai_runtime::secrets::env_file_path`]（启动时按配置目录锚定，或
/// `LIVE2D_AI_ENV_FILE` 显式指定）。硬编码会在「配置在 HOME/APPDATA、密钥在
/// 别处」的组合下**监视错文件**——2026-11-19 真机观测到的「热重载读到旧值」
/// 正是这个分叉。
fn watch_plan(config_path: &Path, env_file: &Path) -> Result<WatchPlan, String> {
    let config_name = config_path
        .file_name()
        .ok_or_else(|| format!("配置文件无文件名: {}", config_path.display()))?
        .to_os_string();
    let config_dir = non_empty_parent(config_path)
        .ok_or_else(|| format!("配置文件无父目录: {}", config_path.display()))?;
    let env_name = env_file
        .file_name()
        .ok_or_else(|| format!("密钥文件无文件名: {}", env_file.display()))?
        .to_os_string();
    let env_dir = non_empty_parent(env_file).unwrap_or_else(|| PathBuf::from("."));

    let mut plan = WatchPlan {
        dirs: Vec::new(),
        targets: Vec::new(),
    };
    plan.push(config_dir, config_name);
    plan.push(env_dir, env_name);
    Ok(plan)
}

impl FileWatcher {
    /// 监听 `config_path` 与**密钥文件**（[`live2d_ai_runtime::secrets::env_file_path`]
    /// 的锚定路径）的外部修改，检测到变更时调用 `supervisor.reload()`。
    ///
    /// 密钥文件可能在**另一个目录**（配置在 HOME/APPDATA、`.env` 锚在别处），
    /// 所以内部按「目录集合 + (目录, 文件名)」监听，见 [`watch_plan`]。
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
        // **密钥文件路径不再猜「配置同目录」**（0.7.0 P1.4）：唯一真源是
        // `secrets::env_file_path()`——启动时已按 config_path 的目录锚定。
        let env_file = live2d_ai_runtime::secrets::env_file_path();
        // `LIVE2D_AI_ENV_FILE` 允许相对路径；notify 事件给的是绝对路径，不归一化
        // 会让过滤条件永远不命中（那样监听了也等于没监）。
        let env_file = match std::env::current_dir() {
            Ok(cwd) if !env_file.is_absolute() => cwd.join(&env_file),
            _ => env_file,
        };
        let plan = watch_plan(&config_path, &env_file)?;
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();

        let handle = std::thread::Builder::new()
            .name("config-file-watcher".into())
            .spawn(move || {
                Self::run(plan, supervisor, stop_flag_clone, debounce_ms, post_reload);
            })
            .map_err(|e| format!("启动文件监听线程失败: {e}"))?;

        Ok(Self {
            stop_flag,
            handle: Some(handle),
        })
    }

    fn run(
        plan: WatchPlan,
        supervisor: SupervisorSource,
        stop_flag: Arc<AtomicBool>,
        debounce_ms: u64,
        post_reload: Option<PostReloadHook>,
    ) {
        let WatchPlan { dirs, targets } = plan;
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let targets_for_cb = targets;
        let mut watcher = match notify::recommended_watcher(move |res: Result<notify::Event, _>| {
            if let Ok(event) = res {
                // 过滤：只关心监听计划里的**那个目录下那个文件名**。只看文件名
                // 不行（不同目录可以有同名文件），只看目录也不行（同目录其它文件
                // 与热重载无关）。
                let hit = event.paths.iter().any(|p| {
                    let Some(name) = p.file_name() else {
                        return false;
                    };
                    let parent = p.parent();
                    targets_for_cb.iter().any(|(dir, target)| {
                        target.as_os_str() == name && parent == Some(dir.as_path())
                    })
                });
                if hit {
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
        // notify 只能监听目录：计划里的每个目录各监听一次（同目录已去重）。
        for dir in &dirs {
            if let Err(e) =
                notify::Watcher::watch(&mut watcher, dir, notify::RecursiveMode::NonRecursive)
            {
                tracing::error!(error = %e, dir = %dir.display(), "监听目录失败");
                return;
            }
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
        // 传「本次事件时是否真的有 supervisor」（= 上面 reload 分支的判定结果），
        // 让钩子能在「无 supervisor + 解析失败」时补发 error 帧（S-007）。
        hook(reloaded);
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
        let hook: PostReloadHook = Box::new(move |_had_supervisor| {
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

    /// ③（0.7.0 P1.4）：watcher 目标 = 配置文件 + **锚定 `.env` 的父目录与文件名**，
    /// 不再硬编码「配置同目录的 `.env`」。纯函数注入路径，不碰全局锚定。
    #[test]
    fn watch_plan_uses_anchored_env_dir_and_name() {
        let config_dir = std::env::temp_dir().join("l2d-watch-plan-config");
        let env_dir = std::env::temp_dir().join("l2d-watch-plan-home");
        let config = config_dir.join("live2d-ai.toml");
        // 文件名也不假设必须是 `.env`（LIVE2D_AI_ENV_FILE 可显式指定任意名）。
        let env = env_dir.join("custom.keys");
        let plan = watch_plan(&config, &env).expect("监听计划");
        assert_eq!(
            plan.dirs,
            vec![config_dir.clone(), env_dir.clone()],
            "配置目录与密钥目录不同 → 两个目录都要监听"
        );
        assert!(
            plan.targets
                .contains(&(config_dir, std::ffi::OsString::from("live2d-ai.toml")))
        );
        assert!(
            plan.targets
                .contains(&(env_dir, std::ffi::OsString::from("custom.keys"))),
            "监视目标必须带锚定路径的父目录 + 文件名：{:?}",
            plan.targets
        );
    }

    /// 同目录时只监听一次目录（不重复 watch），但两个文件名都要在目标里。
    #[test]
    fn watch_plan_dedupes_shared_dir() {
        let dir = std::env::temp_dir().join("l2d-watch-plan-same");
        let plan = watch_plan(&dir.join("live2d-ai.toml"), &dir.join(".env")).expect("监听计划");
        assert_eq!(plan.dirs, vec![dir.clone()], "同目录只 watch 一次");
        assert_eq!(plan.targets.len(), 2);
    }
}
