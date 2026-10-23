//! 本机 Pi 出站子进程（0.4.5 rc.1）：单飞状态、会话目录、argv / env /
//! 超时与整组结束。HTTP 契约在 [`crate::web_api::peer_routes`]，stdout 解析在
//! [`crate::web_api::peer_pi_parse`]。
//!
//! 契约真源：`docs/versions/0.4.md`「0.4.5 把一条对话发给本机 Pi」§出站。
//!
//! # 硬边界（逐条对应执行单）
//!
//! - 执行 `PATH` 里名为 `pi` 的可执行文件（测试可显式注入路径）。**argv 数组、
//!   不经 shell**；原文只作为 `--` 后的**单独一个参数**。
//! - 有 `session_id`：
//!   `pi --print --mode json --no-extensions --no-tools --no-skills
//!    --no-prompt-templates --no-context-files --thinking off
//!    --session-dir <目录> --session-id <id> -- <原文>`；
//!   空 `session_id`：去掉 `--session-dir` / `--session-id`，换成 `--no-session`。
//!   不传 `--api-key` / `--provider` / `--model` / `--approve`。
//! - **环境白名单**：`PATH` / `HOME` / `LANG` / `LC_ALL` + 父进程已有的 `USER`，
//!   其余全部丢弃（`*_API_KEY`、代理变量都不进子进程）；**不读 `.env`**。
//! - **cwd = 宿主自建的 0700 目录**（本机 Pi 的会话目录；空会话 = 一次性临时目录，
//!   跑完删）。不是仓库、不是用户的家目录项目。
//! - 独立进程组（`process_group(0)`）；到点**整组 SIGKILL** 并 `wait` 回收。
//!   Linux 上另设 `PR_SET_PDEATHSIG`（宿主死则 Pi 不滞留）。
//! - 同一时刻只允许一个 Pi 子进程：[ `PeerState::try_acquire` ] 的 `AtomicBool`
//!   compare-exchange 是唯一闸门（第二个请求立即拿不到，不排队）。
//!
//! # 为什么不在这里碰 tiny_http
//!
//! `web_api::run_request_loop` 是**串行**循环：在这里同步等 120s 会把整个 HTTP
//! 面卡死。本模块只提供**同步**的 `run`；把等待搬到 worker 线程是
//! [`crate::web_api::peer_routes::PeerOutcome::complete`] 的职责。
//!
//! # 日志红线
//!
//! 只记**原因码**（`reason=`）与退出状态；**绝不**记原文 / stdout / stderr
//! （stdout 里可能有用户内容，stderr 可能含 Pi 的本地路径）。

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::web_api::peer_pi_spawn::{
    WorkDir, create_private_dir, default_runtime_root, kill_process_group, resolve_program,
    spawn_pipe_reader, whitelisted_env,
};

/// 默认超时（执行单锁死 120 秒）。
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// `session_id` 最大长度（执行单：最长 64）。
pub const MAX_SESSION_ID_LEN: usize = 64;

/// 子进程状态的轮询间隔（测试用短超时也够细）。
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// 读完 stdout / stderr 的上限等待：进程退出（整组结束）后管道应立刻 EOF；
/// 若孙进程在别的进程组里攥着管道，宁可判失败也不让 worker 永久卡住。
const DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

/// 会话 id 契约（执行单锁死）：只允许字母、数字、`_`、`-`，最长 64；
/// 空串 = 这一条不记会话。**按执行单原样**，不放宽 pi 自己更严的
/// 「首尾必须字母数字」规则——那种值由 pi 拒绝后落 `peer_failed`。
pub fn validate_session_id(id: &str) -> bool {
    id.len() <= MAX_SESSION_ID_LEN
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// 一次运行的配置（生产从 [`PeerState::resolve_config`] 解析，测试可注入）。
#[derive(Debug, Clone)]
pub struct RunConfig {
    /// 程序来源：生产 = PATH；测试 = 显式假二进制路径。
    pub program: ProgramSource,
    /// 查 PATH 用的环境值（只用于 [`ProgramSource::Path`]）。
    pub path_env: OsString,
    /// 子进程超时。
    pub timeout: Duration,
    /// 运行时目录根（其下 `sessions/` 与 `tmp/`）。
    pub root: PathBuf,
}

/// 程序来源。
#[derive(Debug, Clone)]
pub enum ProgramSource {
    /// 从 `PATH` 找 `pi`（生产唯一路径）。
    Path,
    /// 显式路径（**测试注入假二进制**；生产不会构造这个变体）。
    Explicit(PathBuf),
}

/// 运行结果（HTTP 映射见 `peer_routes`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PiRunOutcome {
    /// 成功：只含助手正文。
    Reply(String),
    /// PATH 里没有可执行的 `pi`（或显式路径不存在）→ 404 `peer_missing`。
    Missing,
    /// 超时，整组已结束 → 504 `peer_timeout`。
    Timeout,
    /// 其它失败（原因码只进日志）→ 502 `peer_failed`。
    Failed(&'static str),
}

/// 测试注入（生产恒为默认值）；只有 `#[cfg(test)]` 的 setter 能写。
#[derive(Debug, Clone, Default)]
pub struct RunOverrides {
    pub program: Option<PathBuf>,
    pub path_env: Option<OsString>,
    pub timeout: Option<Duration>,
    pub runtime_root: Option<PathBuf>,
}

/// 跨请求共享的 Pi 出站状态（住在 [`crate::web_api::ServerContext`]）。
pub struct PeerState {
    /// 单飞闸门：true = 已有一个 Pi 子进程在飞。
    busy: AtomicBool,
    /// 运行时目录根（首次使用时按 log_paths 的锚点优先级创建 0700）。
    runtime_root: Mutex<Option<PathBuf>>,
    /// 测试注入（生产为空）。
    overrides: Mutex<RunOverrides>,
}

impl Default for PeerState {
    fn default() -> Self {
        Self::new()
    }
}

impl PeerState {
    /// 构造（`ServerContext::new` 调用一次）。
    pub fn new() -> Self {
        Self {
            busy: AtomicBool::new(false),
            runtime_root: Mutex::new(None),
            overrides: Mutex::new(RunOverrides::default()),
        }
    }

    /// 尝试占用唯一的 Pi 子进程名额。
    ///
    /// **这是 409 `peer_busy` 的唯一判据**：compare-exchange 失败即「上一条还在
    /// 等 Pi」，调用方立即回 409，不排队、不打断皮套。
    pub fn try_acquire(self: &Arc<Self>) -> Option<BusyGuard> {
        self.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| BusyGuard {
                state: Arc::clone(self),
            })
    }

    /// 当前是否有 Pi 子进程在飞（诊断 / 测试用；生产由 try_acquire 的 CAS 判定）。
    #[allow(dead_code)]
    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::Acquire)
    }

    /// 解析本次运行的配置（含运行时根目录的创建）。
    pub fn resolve_config(&self) -> std::io::Result<RunConfig> {
        let overrides = self
            .overrides
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let root = match overrides.runtime_root {
            Some(path) => path,
            None => self.ensure_root()?,
        };
        Ok(RunConfig {
            program: match overrides.program {
                Some(path) => ProgramSource::Explicit(path),
                None => ProgramSource::Path,
            },
            path_env: overrides
                .path_env
                .unwrap_or_else(|| std::env::var_os("PATH").unwrap_or_default()),
            timeout: overrides.timeout.unwrap_or(DEFAULT_TIMEOUT),
            root,
        })
    }

    /// 测试注入（仅 `cfg(test)` 编译；生产没有写入口）。
    #[cfg(test)]
    pub fn set_run_overrides(&self, overrides: RunOverrides) {
        *self
            .overrides
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = overrides;
    }

    /// 首次使用时确定并创建运行时根（0700）。
    ///
    /// 锚点优先级复用 [`crate::log_paths::select_log_dir`]（XDG_DATA_HOME →
    /// HOME/.local/share → exe 目录 → LOCALAPPDATA），取其 `live2d-ai` 父目录
    /// 再拼 `peers/pi`；全不可用才落 `std::env::temp_dir()`。
    /// 会话文件跨请求要接着用，所以根目录**持久**，不随进程退出删除。
    fn ensure_root(&self) -> std::io::Result<PathBuf> {
        let mut guard = self
            .runtime_root
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(root) = guard.as_ref() {
            return Ok(root.clone());
        }
        let root = default_runtime_root();
        create_private_dir(&root)?;
        *guard = Some(root.clone());
        Ok(root)
    }
}

/// 单飞守卫：drop 时释放 busy 槽位（成功 / 失败 / 超时 / worker 线程 panic 都释放）。
pub struct BusyGuard {
    state: Arc<PeerState>,
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.state.busy.store(false, Ordering::Release);
    }
}

/// 跑一次 Pi：找程序 → 建目录 → spawn → 等（最多 `timeout`）→ 收 stdout →
/// 解析。**同步阻塞**，只能在 worker 线程里调用。
pub fn run(config: &RunConfig, text: &str, session_id: &str) -> PiRunOutcome {
    debug_assert!(
        validate_session_id(session_id),
        "session_id 必须由 HTTP 层先校验"
    );
    let Some(program) = resolve_program(&config.program, &config.path_env) else {
        return PiRunOutcome::Missing;
    };
    let work = match WorkDir::prepare(&config.root, session_id) {
        Ok(work) => work,
        Err(err) => {
            tracing::warn!(target: "peer_pi", reason = "work_dir", error = %err, "无法准备 Pi 会话目录");
            return PiRunOutcome::Failed("work_dir");
        }
    };

    let mut command = Command::new(&program);
    command.args(build_argv(text, session_id, work.path()));
    command.current_dir(work.path());
    // 环境白名单：先清空再把允许的键从**父进程**搬过去（不读 .env）。
    command.env_clear();
    for (key, value) in whitelisted_env(|key| std::env::var_os(key)) {
        command.env(key, value);
    }
    // stdin = null：不让 Pi 等 stdin（print 模式不需要交互，也不能继承我们的管道）。
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // 独立进程组：超时时可整组结束（与 folder_spawn 同一条纪律）。
        command.process_group(0);
        #[cfg(target_os = "linux")]
        #[allow(unsafe_code)]
        // pre_exec 是 unsafe（fork 后只做一次 prctl，见 folder_spawn）。
        unsafe {
            command.pre_exec(|| {
                crate::folder_spawn::sys::set_parent_death_signal();
                Ok(())
            });
        }
    }

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            tracing::warn!(target: "peer_pi", reason = "spawn", error = %err, "无法启动本机 pi");
            return if err.kind() == std::io::ErrorKind::NotFound {
                PiRunOutcome::Missing
            } else {
                PiRunOutcome::Failed("spawn")
            };
        }
    };
    let stdout_rx = child.stdout.take().map(spawn_pipe_reader);
    let stderr_rx = child.stderr.take().map(spawn_pipe_reader);

    let deadline = Instant::now() + config.timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    // 到点：整组 SIGKILL + 回收 leader。子进程即使忽略 SIGTERM
                    // 也躲不掉 SIGKILL；孙进程同组一起结束。
                    kill_process_group(child.id());
                    let _ = child.wait();
                    let _ = stderr_rx.and_then(|rx| rx.recv_timeout(DRAIN_TIMEOUT).ok());
                    tracing::warn!(target: "peer_pi", reason = "timeout", "本机 pi 超时，整组已结束");
                    return PiRunOutcome::Timeout;
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(err) => {
                tracing::warn!(target: "peer_pi", reason = "wait", error = %err, "等待本机 pi 失败");
                kill_process_group(child.id());
                let _ = child.wait();
                return PiRunOutcome::Failed("wait");
            }
        }
    };

    // leader 已退出；组里若还有孙进程攥着 stdout / stderr 管道，补一刀整组，
    // 让读者线程能 EOF（真实 pi 不开工具时没有孙进程，这里是保底）。
    kill_process_group(child.id());
    let stdout = stdout_rx
        .and_then(|rx| rx.recv_timeout(DRAIN_TIMEOUT).ok())
        .unwrap_or_default();
    // stderr 只排空、不入正文、不进日志（可能含本地路径 / 错误细节）。
    let stderr_len = stderr_rx
        .and_then(|rx| rx.recv_timeout(DRAIN_TIMEOUT).ok())
        .map(|bytes| bytes.len())
        .unwrap_or(0);

    if !status.success() {
        tracing::warn!(
            target: "peer_pi",
            reason = "exit_nonzero",
            code = ?status.code(),
            stderr_bytes = stderr_len,
            "本机 pi 非 0 退出"
        );
        return PiRunOutcome::Failed("exit_nonzero");
    }

    let stdout = String::from_utf8_lossy(&stdout);
    match crate::web_api::peer_pi_parse::parse_pi_session_stdout(&stdout) {
        Ok(reply) => PiRunOutcome::Reply(reply),
        Err(err) => {
            tracing::warn!(
                target: "peer_pi",
                reason = err.reason(),
                stderr_bytes = stderr_len,
                "本机 pi 的 stdout 没有可读助手正文"
            );
            PiRunOutcome::Failed(err.reason())
        }
    }
}

/// 构造 argv（**数组、不经 shell**；顺序即执行单原文）。
///
/// 原文永远是最后一个元素、且只出现一次（`--` 之后）；有会话时
/// `--session-dir` / `--session-id` 在 `--` 之前。
///
/// 唯一的文本改写是 [`prompt_arg`]（首字符 `@` 的 pi CLI 兼容垫片）。
pub fn build_argv(text: &str, session_id: &str, session_dir: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
        "--print",
        "--mode",
        "json",
        "--no-extensions",
        "--no-tools",
        "--no-skills",
        "--no-prompt-templates",
        "--no-context-files",
        "--thinking",
        "off",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    if session_id.is_empty() {
        args.push(OsString::from("--no-session"));
    } else {
        args.push(OsString::from("--session-dir"));
        args.push(session_dir.as_os_str().to_owned());
        args.push(OsString::from("--session-id"));
        args.push(OsString::from(session_id));
    }
    args.push(OsString::from("--"));
    args.push(OsString::from(prompt_arg(text)));
    args
}

/// 原文进 argv 前的唯一改写：**只**处理首字符 `@`。
///
/// pi 在 `--` 之后仍把 `@…` 当 `@file` 附件而不是 prompt
/// （`packages/coding-agent/src/cli/args.ts` 83-90：`startsWith("@")` →
/// `fileArgs.push(arg.slice(1))`），用户原文正好以 `@` 开头时这句话会被
/// 当成文件路径丢掉。加**一个前导空格**让 pi 按普通文本处理——只多一个空格，
/// 正文一字不动。这是 pi CLI 的兼容垫片，不是业务逻辑（其它字符不转换）。
fn prompt_arg(text: &str) -> String {
    if text.starts_with('@') {
        format!(" {text}")
    } else {
        text.to_string()
    }
}

#[cfg(all(test, unix))]
#[path = "peer_pi_tests.rs"]
mod tests;
