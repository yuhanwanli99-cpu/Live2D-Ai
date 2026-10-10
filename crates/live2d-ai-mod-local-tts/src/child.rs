//! 子进程抽象与**唯一**的真实 `Command` 拼装点。
//!
//! # 为什么要抽一层
//!
//! 单测**不 exec 真进程**（口径）：注入 `Spawner`，返回一个假句柄即可断言
//! argv / 停止调用 / 失败路径。真实实现 [`real_spawn`] 只在这里出现一次，
//! 「有没有走 shell」「stdio 怎么接」这类红线才有单一审计点。
//!
//! # 进程形态（口径逐条）
//!
//! - `argv` **逐个参数**传（`Command::new(argv[0])` + `args(argv[1..])`）：
//!   带空格的一项仍是一个 argv 元素，**禁止** `sh -c` / 拼一条命令行；
//! - `stdin = null`；`stdout = inherit`；
//! - `stderr = piped`，**由本模块立刻读干**并原样透传到宿主 stderr
//!   （2026-10-09 口径变更）：
//!   * 旧口径是 `stderr = inherit`，理由是「没人读的管道写满就把子进程堵死」；
//!     那条理由针对的是**没人读**，不是管道本身——这里有一条专用读线程，
//!     逐行读干、同时 `eprintln!` 转发，所以既不会堵，也没有丢掉可见性。
//!   * 必须留一份的原因只有一个：子进程**立刻非 0 退出**时，`start` 要把
//!     **脚本自己印的原因**放进 Err（宿主据此把 Mod 标 Failed）。透传只能让人
//!     在终端里看见，Err 里带不到。
//!   * 尾巴有上限（[`STDERR_TAIL_MAX`]），只留最后若干字节——它是错误消息的
//!     证据，不是日志文件。
//! - 读法参考 `crates/live2d-ai-mod-voice-input/src/sidecar.rs` 的「逐参数、不过
//!   shell」，但**不调用那个 crate、也不改它的行为**。

use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// stderr 尾巴的保留上限（**字节**）：超了就从头部裁到最近的字符边界。
///
/// 它只用来给 Err 附一句「脚本说了什么」，不需要全文。
pub const STDERR_TAIL_MAX: usize = 2048;

/// 一次非阻塞观察的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildProbe {
    /// 仍在跑（`try_wait` 回了 `Ok(None)`）。
    Running,
    /// 已退出；`code` 为 `None` = 被信号终止（拿不到退出码，**不编一个**）。
    Exited { code: Option<i32> },
}

/// 子进程句柄（生产实现 = `std::process::Child` 的薄包装）。
pub trait ChildProcess: Send {
    /// 进程号（拿不到时 `None`）。
    fn pid(&self) -> Option<u32>;
    /// **非阻塞**观察；出错原样上抛（快照把它记进 `wait_error`）。
    fn try_wait(&mut self) -> std::io::Result<ChildProbe>;
    /// 请求停止（生产实现 = `kill()`；停完还要 [`ChildProcess::wait`] 收尸）。
    fn stop(&mut self) -> std::io::Result<()>;
    /// 阻塞等待结束（收尸）。
    fn wait(&mut self) -> std::io::Result<()>;
    /// 子进程 **stderr 的尾巴**（2026-10-09）。
    ///
    /// 唯一用途：子进程立刻非 0 退出时，把脚本印出的原因带进 `start` 的 Err。
    /// 没有捕获（假句柄 / 未接管道 / 一个字都没输出）时返回 `None`——
    /// **不编**一句「未知错误」冒充脚本原话。
    fn stderr_tail(&self) -> Option<String>;
}

/// 一次 spawn 请求（argv + 可选工作目录）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnRequest {
    /// 完整 argv（含 `argv[0]`）。
    pub argv: Vec<String>,
    /// `current_dir`；`None` = 不设（继承宿主 cwd）。
    pub workdir: Option<std::path::PathBuf>,
}

/// 可注入的 spawner（单测注入假实现，不 exec 真进程）。
pub type Spawner = Box<dyn Fn(&SpawnRequest) -> std::io::Result<Box<dyn ChildProcess>> + Send>;

/// 让子进程的**生命周期与主程序一致**：父进程一消失，内核就把 `SIGKILL` 发给它。
///
/// # 为什么需要（真实来历，2026-10-10）
///
/// MeloTTS / CosyVoice 是本 Mod spawn 的**外部长驻进程**。桌面端若被 `SIGKILL`
/// 或被强杀（不经过 `shutdown()` 的路径），子进程会被 reparent 到 init 继续
/// 占着端口；下一次点火时新进程绑定失败、主链对着卡死的旧进程发请求
/// （本机实测：8091 上残留旧 melo → 新 melo `Address already in use` 立即退出
/// → `tts_upstream_502`）。`shutdown()` 的 `kill()` 只在父进程**能**优雅收尾时
/// 有效；`PR_SET_PDEATHSIG` 由内核兜底「父死 → 子收 SIGKILL」，不依赖那一刻。
///
/// # 边界（逐条）
///
/// - 只作用于**被 exec 的这一个进程**：`PDEATHSIG` **不被 fork 继承**
///   （`start.sh` 里为装依赖 fork 出的孙进程不会误伤），但**跨 `execve` 保留**
///   （`exec python serve.py` 之后依然生效）——正是我们要的「长驻那个进程」。
/// - 父进程在 `fork` 之后、`prctl` 之前若已退出，信号不会再投递（竞态）；
///   此处补一次 `getppid()`：已变成 1（被 init 收养）就主动报错退出，不留孤儿。
/// - 仅 Linux 编译：本项目的进程宿主是 WSL2/Linux，Windows 侧只跑浏览器。
#[cfg(target_os = "linux")]
#[allow(unsafe_code)] // 全仓唯一一处 unsafe：prctl(PDEATHSIG) 没有安全封装，见上说明。
fn arm_parent_death_signal(cmd: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    // SAFETY: `pre_exec` 在 fork 之后、exec 之前运行于**子进程**内。闭包只调用
    // async-signal-safe 的 `prctl` 与 `getppid`：不分配内存、不加锁、不做 IO。
    unsafe {
        cmd.pre_exec(|| {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() == 1 {
                return Err(std::io::Error::other("父进程在装上 PDEATHSIG 之前已退出"));
            }
            Ok(())
        });
    }
}

/// 生产实现：`std::process::Command`，逐参数、无 shell、stderr 接管道并读干。
pub fn real_spawn(req: &SpawnRequest) -> std::io::Result<Box<dyn ChildProcess>> {
    let Some((program, args)) = req.argv.split_first() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "argv 为空（连 argv[0] 都没有）",
        ));
    };
    let mut cmd = std::process::Command::new(program);
    cmd.args(args);
    if let Some(dir) = &req.workdir {
        cmd.current_dir(dir);
    }
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::inherit());
    cmd.stderr(std::process::Stdio::piped());
    // 生命周期 = 主程序：父进程一没，内核就把这个子进程 SIGKILL 掉（见函数头注）。
    #[cfg(target_os = "linux")]
    arm_parent_death_signal(&mut cmd);
    let mut child = cmd.spawn()?;
    let tail: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let reader = child.stderr.take().map(|err| {
        let sink = Arc::clone(&tail);
        // 读线程唯一职责：**读干 + 透传 + 留尾巴**。见模块头注的红线说明。
        std::thread::spawn(move || drain_stderr(err, sink))
    });
    Ok(Box::new(RealChild {
        child,
        tail,
        reader: Mutex::new(reader),
    }))
}

/// 把 stderr 读干：逐行透传到宿主 stderr，同时在内存里留最后 [`STDERR_TAIL_MAX`] 字节。
fn drain_stderr(source: impl std::io::Read, sink: Arc<Mutex<String>>) {
    let mut reader = BufReader::new(source);
    let mut buf: Vec<u8> = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(_) => {}
            // 读失败（含非 UTF-8 之外的 IO 错误）：退出读线程，不留半行。
            Err(_) => break,
        }
        // 非 UTF-8 用 lossy：宁可看见一个替换字符，也不要丢掉这行原因。
        let text = String::from_utf8_lossy(&buf);
        let text = text.trim_end_matches(['\n', '\r']);
        eprintln!("{text}");
        let mut guard = sink.lock().unwrap_or_else(|e| e.into_inner());
        guard.push_str(text);
        guard.push('\n');
        if guard.len() > STDERR_TAIL_MAX {
            let cut = guard.len() - STDERR_TAIL_MAX;
            // 按字符边界裁（错误消息里是中文）。
            let cut = guard
                .char_indices()
                .map(|(i, _)| i)
                .find(|i| *i >= cut)
                .unwrap_or(guard.len());
            guard.drain(..cut);
        }
    }
}

/// `std::process::Child` 的薄包装（把 `ExitStatus` 归一成 `Option<i32>`）。
struct RealChild {
    child: std::process::Child,
    tail: Arc<Mutex<String>>,
    /// 读线程句柄；`stderr_tail` 时取回收尾（**只能 join 一次**）。
    reader: Mutex<Option<JoinHandle<()>>>,
}

impl RealChild {
    /// 把读线程收回来。子进程已退出 → 管道 EOF → 线程立刻结束，不会卡住。
    fn join_reader(&self) {
        let handle = self.reader.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(h) = handle {
            let _ = h.join();
        }
    }
}

impl ChildProcess for RealChild {
    fn pid(&self) -> Option<u32> {
        Some(self.child.id())
    }

    fn try_wait(&mut self) -> std::io::Result<ChildProbe> {
        match self.child.try_wait()? {
            None => Ok(ChildProbe::Running),
            Some(status) => Ok(ChildProbe::Exited {
                code: status.code(),
            }),
        }
    }

    fn stop(&mut self) -> std::io::Result<()> {
        self.child.kill()
    }

    fn wait(&mut self) -> std::io::Result<()> {
        self.child.wait().map(|_| ())
    }

    fn stderr_tail(&self) -> Option<String> {
        // 先 join：进程刚死时最后一行可能还在读线程的缓冲里。
        self.join_reader();
        let guard = self.tail.lock().unwrap_or_else(|e| e.into_inner());
        if guard.trim().is_empty() {
            None
        } else {
            Some(guard.clone())
        }
    }
}
