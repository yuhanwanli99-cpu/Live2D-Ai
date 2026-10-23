//! 本机 Pi 出站的操作系统管线（0.4.5 rc.1）：目录、PATH 查找、argv 环境
//! 白名单、管道读者、整组信号。HTTP 契约在 [`crate::web_api::peer_routes`]，
//! 状态机 / argv / 超时循环在 [`crate::web_api::peer_pi`]。
//!
//! 拆出来的唯一理由是文件行数棘轮（每文件 ≤500）：这里是 `std` + 既有
//! `folder_spawn` FFI 缝的薄封装，没有业务判断。
//!
//! # 安全边界（与 `peer_pi` 头注同源）
//!
//! - 可执行文件必须是**普通文件且有执行位**（PATH 检索不接受目录 / 644 文件）；
//! - 子进程环境只搬 `PATH` / `HOME` / `LANG` / `LC_ALL` + 父进程已有的
//!   `USER`（不读 `.env`、不搬 `*_API_KEY` / 代理变量）；
//! - 工作目录 0700，已存在则**收紧**到 0700；
//! - 结束子进程用**负 pid 的整组 SIGKILL**（FFI 缝复用 `folder_spawn::sys`）。

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;

use crate::web_api::peer_pi::ProgramSource;

/// 找的进程名（生产只从 PATH 找这个名字）。
const PI_PROGRAM: &str = "pi";

const SESSION_SUBDIR: &str = "sessions";
const TMP_SUBDIR: &str = "tmp";

/// 运行时根的默认落点（纯优先级判断；不创建目录）。
///
/// 优先级与日志目录同源（[`crate::log_paths::select_log_dir`]：XDG_DATA_HOME →
/// HOME/.local/share → exe 目录 → LOCALAPPDATA），取 `live2d-ai` 父目录再拼
/// `peers/pi`；全不可用才落 `std::env::temp_dir()`。
pub(crate) fn default_runtime_root() -> PathBuf {
    crate::log_paths::select_log_dir(std::env::current_exe().ok().as_deref(), |key| {
        std::env::var_os(key)
    })
    .ok()
    .and_then(|dir| dir.parent().map(|base| base.join("peers").join("pi")))
    .unwrap_or_else(|| {
        std::env::temp_dir()
            .join("live2d-ai")
            .join("peers")
            .join("pi")
    })
}

/// 环境白名单（顺序固定；父进程没有的键不补默认值）。
///
/// 只留 `PATH` / `HOME` / `LANG` / `LC_ALL` + 父进程已有的 `USER`。
/// **不**读 `.env`、**不**搬 `*_API_KEY` / 代理变量。
pub(crate) fn whitelisted_env(
    mut lookup: impl FnMut(&str) -> Option<OsString>,
) -> Vec<(OsString, OsString)> {
    const KEYS: [&str; 5] = ["PATH", "HOME", "LANG", "LC_ALL", "USER"];
    let mut env = Vec::with_capacity(KEYS.len());
    for key in KEYS {
        if let Some(value) = lookup(key) {
            env.push((OsString::from(key), value));
        }
    }
    env
}

/// 在 `path_env`（`:` 分隔的 PATH 值）里找名为 `name` 的可执行文件。
pub(crate) fn find_executable(name: &str, path_env: &OsStr) -> Option<PathBuf> {
    for dir in std::env::split_paths(path_env) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        let candidate = dir.join(name);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

pub(crate) fn resolve_program(source: &ProgramSource, path_env: &OsStr) -> Option<PathBuf> {
    match source {
        ProgramSource::Path => find_executable(PI_PROGRAM, path_env),
        ProgramSource::Explicit(path) => is_executable_file(path).then(|| path.clone()),
    }
}

/// 普通文件且可执行（Unix 看 `0o111`；非 Unix 只判普通文件）。
fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// 本机 Pi 的工作目录（`cwd` == `--session-dir`，0700）。
pub(crate) struct WorkDir {
    dir: PathBuf,
    ephemeral: bool,
}

impl WorkDir {
    pub(crate) fn prepare(root: &Path, session_id: &str) -> std::io::Result<Self> {
        if session_id.is_empty() {
            // 一次性会话：临时 0700 目录，跑完删（不落会话文件）。
            let dir = root.join(TMP_SUBDIR).join(unique_dir_name());
            create_private_dir(&dir)?;
            Ok(Self {
                dir,
                ephemeral: true,
            })
        } else {
            // 有会话：同一 id 复用同一目录（Pi 的会话文件住在这里，
            // 下一条仍是同一次对话）。
            let dir = root.join(SESSION_SUBDIR).join(session_id);
            create_private_dir(&dir)?;
            Ok(Self {
                dir,
                ephemeral: false,
            })
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.dir
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        if self.ephemeral {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

/// 创建 0700 目录（已存在则**收紧权限**到 0700）。
pub(crate) fn create_private_dir(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(path)
    }
}

fn unique_dir_name() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    format!("ephemeral-{}-{nanos}-{counter}", std::process::id())
}

/// 起一个读者线程把管道读干净（stdout 可能超过管道缓冲，不并发读会死锁）。
pub(crate) fn spawn_pipe_reader<R: std::io::Read + Send + 'static>(
    mut reader: R,
) -> mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = reader.read_to_end(&mut buf);
        let _ = tx.send(buf);
    });
    rx
}

/// 结束整个子进程组（负 pid）。只对本进程记录过的 pid 使用。
pub(crate) fn kill_process_group(pid: u32) {
    #[cfg(unix)]
    crate::folder_spawn::sys::kill_group(pid, crate::folder_spawn::sys::SIGKILL);
    #[cfg(not(unix))]
    {
        let _ = pid;
    }
}
