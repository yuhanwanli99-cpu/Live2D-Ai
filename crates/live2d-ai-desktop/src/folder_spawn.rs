//! 文件夹 Mod 的**子进程生命周期**（0.3.13；0.4.0 rc.1 增加 Windows 支持）。
//!
//! 执行单：docs/plans/PLAN-0.3.13-folder-spawn-2026-10-21.md。
//!
//! 边界（与 0.3.12 的只读预检严格分开）：
//!
//! - 只有 POST /api/v1/mods/discovered 在**勾选写盘成功之后**才调用这里；
//!   GET 清单、「刷新」、扫描函数都不碰本模块；
//! - 启动的是清单里**已经校验过**的 entry（kind = process）或
//!   launch.program（kind = launcher 且 mode = with_app）；on_demand 永远
//!   不在这里启动，只留在 mods.json；
//! - 工作目录 = 该 Mod 目录；不经过 shell、不加参数、不读 .env、不传密钥
//!   环境变量（env_clear 之后只回填白名单：PATH / HOME / LANG / LC_ALL；
//!   0.4.0 rc.1 起 Windows 上再带上**已存在**的 SYSTEMROOT / USERPROFILE /
//!   PATHEXT；0.4.7 rc.2 起再补 TEMP / TMP / APPDATA / LOCALAPPDATA / COMSPEC
//!   ——python / 本地引擎这类子进程要靠 %TEMP% 写临时文件，见 [`crate::host_layout::collect_child_env`]）；
//! - 子进程不阻塞对话回合：spawn 之后不 wait（只在结束时才等它退出）。
//!
//! # 平台（0.4.0 rc.1）
//!
//! - **Windows 入口必须就是 `.exe` 文件本身**：`.sh` / `.bat` 要 shell，
//!   CreateProcess 不会执行它们——预检阶段就给出可读原因，spawn 前再兜一刀
//!   （[`crate::host_layout::entry_problem`]）。
//! - **父进程退出时结束自己记下的子进程**：Linux 用 PR_SET_PDEATHSIG = SIGTERM
//!   + 独立进程组；Windows 用**一个子进程一个 Job Object**
//!   （`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`，见 `crate::folder_job`），
//!   父进程被硬杀也不会留孤儿。
//! - **挂起启动再挂 Job（0.4.7 rc.2 / W5）**：Windows 子进程用 CREATE_SUSPENDED
//!   创建，先把进程树拉进 Job 再 ResumeThread——0.4.6 以前是「spawn 之后才
//!   AssignProcessToJobObject」，子进程在那段窗口里拉起的孙进程会逃出 Job。
//! - **温和终止（0.4.7 rc.2 / W6）**：取消勾选 / 关停时先给顶层窗口 WM_CLOSE，
//!   再给子进程独占的控制台进程组发 CTRL_BREAK_EVENT，宽限 2s 才
//!   TerminateJobObject 动硬的；Linux 仍是 SIGTERM → 200ms → SIGKILL。
//! - **失败人话（0.4.7 rc.2 / W21）**：Windows 的 NTSTATUS / Win32 错误码
//!   （0xC0000135 缺 DLL、0xC000007B 位数不符、5 拒绝访问…）翻成可执行中文，
//!   原始错误文本一律保留（见 [`spawn_failure_message`]）。
//! - prctl / kill / kernel32 没有 std 封装，而本 crate 的顶层依赖已顶格
//!   （18/18，执行单 §37）——所以两处都用**最小 FFI**（文件末尾 `sys` 模块 +
//!   `folder_job`）。工作区 unsafe_code = deny，用文件级 allow 显式豁免。
//!
//! # 失败
//!
//! 启动失败（文件不可执行 / 路径消失 / fork 失败 / Windows 入口不是 exe…）只进
//! 返回值的 failed，不影响 HTTP 200：勾选已经写盘成功（执行单 §17）。不 panic。

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::host_layout;

/// 一次「进入」希望处于运行状态的文件夹 Mod。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesiredSpawn {
    /// 清单 id（= 目录名），用于和 mods.json / 页面行对应。
    pub id: String,
    /// 子进程工作目录（该 Mod 的目录，规范化后的绝对路径）。
    pub dir: PathBuf,
    /// 已校验过的程序文件（绝对路径；扫描阶段做过规范化）。
    pub program: PathBuf,
}

/// 一次 reconcile 的结果（POST 响应里的 started / stopped / failed）。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReconcileReport {
    pub started: Vec<String>,
    pub stopped: Vec<String>,
    pub failed: Vec<StartFailure>,
}

/// 单个 id 启动失败的原因（页面日志直接显示）。
#[derive(Debug, PartialEq, Eq)]
pub struct StartFailure {
    pub id: String,
    pub reason: String,
}

/// 本进程为**文件夹 Mod** 起过的子进程表（一个 id 至多一个）。
///
/// 放在 ServerContext 的字段里（Arc 共享）；ServerContext 整份克隆只复制
/// 这个 Arc，子进程表仍是同一份。Drop 只在最后一个引用消失时发生：服务
/// 正常关停时会顺手结束自己起过的子进程；进程被直接杀死时靠 PDEATHSIG
/// （Linux）/ Job Object（Windows）。
#[derive(Default)]
pub struct FolderProcesses {
    children: Mutex<HashMap<String, TrackedProcess>>,
}

/// 一个自己起过的子进程（Windows 上带自己的 Job Object）。
struct TrackedProcess {
    child: Child,
    /// Windows：KILL_ON_JOB_CLOSE 的 Job；指派失败为 None（显式结束兜底）。
    #[cfg(windows)]
    job: Option<crate::folder_job::Job>,
}

impl TrackedProcess {
    #[cfg(not(windows))]
    fn new(child: Child) -> Self {
        Self { child }
    }

    /// Windows：Job 与创建标志由 [`spawn_one`] 管（挂起创建 → attach → resume），
    /// 这里只把两样装起来。
    #[cfg(windows)]
    fn with_job(child: Child, job: Option<crate::folder_job::Job>) -> Self {
        Self { child, job }
    }
}

impl FolderProcesses {
    pub fn new() -> Self {
        Self {
            children: Mutex::new(HashMap::new()),
        }
    }

    /// 把运行集合对齐到 desired（执行单 §14 / §27）。
    ///
    /// - 已记录且还活着的 id：不动（再次「进入」不会起第二个）；
    /// - 已记录但已退出：移出表，按 desired 重新起；
    /// - 不在 desired 里的 id：结束**本进程**记录过的那一组进程；
    /// - 启动失败只记 failed，继续处理其它 id。
    pub fn reconcile(&self, desired: &[DesiredSpawn]) -> ReconcileReport {
        let mut children = self.lock();
        let mut report = ReconcileReport::default();

        let keep: Vec<&str> = desired.iter().map(|spec| spec.id.as_str()).collect();
        let mut stop_ids: Vec<String> = children
            .keys()
            .filter(|id| !keep.contains(&id.as_str()))
            .cloned()
            .collect();
        stop_ids.sort();
        for id in stop_ids {
            if let Some(mut process) = children.remove(&id) {
                stop_child(&mut process);
                report.stopped.push(id);
            }
        }

        for spec in desired {
            let alive = match children.get_mut(&spec.id) {
                Some(process) => matches!(process.child.try_wait(), Ok(None)),
                None => false,
            };
            if alive {
                continue;
            }
            children.remove(&spec.id);
            match spawn_one(spec) {
                Ok(process) => {
                    children.insert(spec.id.clone(), process);
                    report.started.push(spec.id.clone());
                }
                Err(reason) => report.failed.push(StartFailure {
                    id: spec.id.clone(),
                    reason,
                }),
            }
        }
        report
    }

    /// 当前**还活着**的文件夹 id（只读；仅 unix 测试用它断言进程真的起了/停了）。
    #[cfg(all(test, unix))]
    pub fn running_ids(&self) -> Vec<String> {
        let mut children = self.lock();
        let mut out: Vec<String> = Vec::new();
        for (id, process) in children.iter_mut() {
            if matches!(process.child.try_wait(), Ok(None)) {
                out.push(id.clone());
            }
        }
        out.sort();
        out
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, TrackedProcess>> {
        self.children
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Drop for FolderProcesses {
    fn drop(&mut self) {
        let mut children = self
            .children
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for (_, mut process) in children.drain() {
            stop_child(&mut process);
        }
    }
}

/// Windows 子进程创建标志（0.4.7 rc.2 / W5 + W6）。
///
/// - `CREATE_SUSPENDED`（0x0000_0004）：创建后主线程挂起，Job 指派完成后才
///   `ResumeThread`——0.4.6 以前是「spawn 之后才 assign」，子进程在那段窗口里
///   拉起的孙进程不在 Job 里，父进程被硬杀时会逃逸（W5 竞态）。
/// - `CREATE_NEW_PROCESS_GROUP`（0x0000_0200）：独占一个控制台进程组，取消
///   勾选时先给这个组发 `CTRL_BREAK_EVENT` 温和终止（W6）。
///
/// 非 Windows 目标上没有消费者；Linux 单测钉住这两个位（防有人「顺手」删掉挂起）。
#[cfg_attr(not(windows), allow(dead_code))]
const CHILD_CREATION_FLAGS: u32 = 0x0000_0004 | 0x0000_0200;

/// spawn 失败的一句可执行中文（0.4.7 rc.2 / W21）。
///
/// 只透传 OS 原文时，用户看到的是 `os error -1073741515` 这类天书。这里把
/// Windows 最常见的几类失败翻成人话，但**原始错误一律保留**（诊断靠它）：
/// - `0xC0000135`（STATUS_DLL_NOT_FOUND）：缺 DLL / 运行库；
/// - `0xC000007B`（STATUS_INVALID_IMAGE_FORMAT）/ 193 / 216：位数或架构不符；
/// - `5`（ERROR_ACCESS_DENIED）：杀软 / 安全策略拦截或权限不足。
fn spawn_failure_message(err: std::io::Error) -> String {
    let hint = spawn_error_hint(err.raw_os_error()).or_else(|| match err.kind() {
        std::io::ErrorKind::PermissionDenied => Some("文件不可执行或权限不足"),
        std::io::ErrorKind::NotFound => Some("程序不存在（路径变了或被删）"),
        _ => None,
    });
    match (hint, err.raw_os_error()) {
        (Some(hint), Some(raw)) => {
            let status = raw as u32;
            if status & 0x8000_0000 != 0 {
                format!("启动失败：{hint}（NTSTATUS 0x{status:08x}；原始错误：{err}）")
            } else {
                format!("启动失败：{hint}（错误码 {raw}；原始错误：{err}）")
            }
        }
        (Some(hint), None) => format!("启动失败：{hint}（原始错误：{err}）"),
        (None, _) => format!("启动失败：{err}"),
    }
}

/// Win32 / NTSTATUS 错误码 → 人话（只收最常见的几类，未命中的原样透传）。
fn spawn_error_hint(raw: Option<i32>) -> Option<&'static str> {
    Some(match raw? as u32 {
        0xC000_0135 => {
            "缺少 DLL 或运行库（STATUS_DLL_NOT_FOUND）：安装 Microsoft Visual C++ 运行库（x64），或换用带运行库的构建"
        }
        0xC000_007B => {
            "镜像格式 / 位数不符（STATUS_INVALID_IMAGE_FORMAT）：换与系统架构一致的 64 位 .exe"
        }
        0xC000_0142 => "DLL 初始化失败（STATUS_DLL_INIT_FAILED）：运行库缺文件或被杀毒软件拦截",
        5 => "拒绝访问（权限不足，或被杀毒软件 / 安全策略拦截）",
        193 => "不是有效的 Win32 应用程序（位数 / 格式不符）",
        216 => "程序与系统架构不符（ERROR_EXE_MACHINE_TYPE_MISMATCH）",
        267 => "目录名无效（工作目录路径有问题）",
        740 => "需要以管理员身份运行（ERROR_ELEVATION_REQUIRED）",
        1260 => "被组策略禁止运行（ERROR_ACCESS_DISABLED_BY_POLICY）",
        _ => return None,
    })
}

/// 起一个子进程：cwd = Mod 目录，程序 = 规范化后的绝对路径。
///
/// 不加参数（执行单 §23）、不 chmod（执行单 §24）、不传 .env / 密钥环境变量
/// （执行单 §25）。失败时返回一句可直接展示的原因。
#[allow(unsafe_code)] // pre_exec 是 unsafe：子进程只做一次 prctl（见文件头）。
fn spawn_one(spec: &DesiredSpawn) -> Result<TrackedProcess, String> {
    if !spec.program.is_absolute() {
        return Err("程序路径不是绝对路径（清单预检之后磁盘被改动？）".to_string());
    }
    // 0.4.0 rc.1：Windows 上入口必须就是 .exe 本身（预检已拦，这里再兜一刀——
    // 清单可能在预检之后被换掉）。失败原因必须可读。
    if let Some(problem) = host_layout::entry_problem(&spec.program, cfg!(windows)) {
        return Err(format!("启动失败：{problem}"));
    }
    let mut command = Command::new(&spec.program);
    command.current_dir(&spec.dir);
    command.env_clear();
    for (key, value) in host_layout::collect_child_env(cfg!(windows), |key| std::env::var_os(key)) {
        command.env(key, value);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // 独立进程组：结束时可以整组结束，不碰同名的外部进程。
        command.process_group(0);
        #[cfg(target_os = "linux")]
        unsafe {
            command.pre_exec(|| {
                sys::set_parent_death_signal();
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // W5/W6：挂起创建（attach 前不跑）+ 独占控制台进程组（CTRL_BREAK 目标）。
        command.creation_flags(CHILD_CREATION_FLAGS);
    }
    let child = command.spawn().map_err(spawn_failure_message)?;
    #[cfg(windows)]
    {
        // W5：先 attach（子进程仍挂起，拉不起孙进程）→ 再 resume。assign 失败时
        // Job 为 None，退化到「显式结束兜底」（已知限制，见 folder_job）。
        let job = crate::folder_job::Job::attach(&child);
        if let Err(err) = crate::win32_proc::resume_primary_thread(&child) {
            // 不能把子进程永远挂在 CREATE_SUSPENDED 里：结束 + 回收后如实报错。
            let mut child = child;
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("启动失败：子进程无法恢复运行（{err}）"));
        }
        return Ok(TrackedProcess::with_job(child, job));
    }
    #[cfg(not(windows))]
    Ok(TrackedProcess::new(child))
}

/// 结束一个自己记录过的子进程**组**：先温和终止，宽限后才动硬的。
///
/// 只对记录过的 pid 发信号（负 pid = 该组）/ 只对自己的 Job；不按名字杀，所以
/// 用户另外启动的同名程序不会被误伤。leader 先退出时仍会补一次整组强杀，避免
/// 它拉起的孙进程留在组里。
///
/// - Linux：SIGTERM → 200ms 宽限 → SIGKILL（0.3.13 语义不变）；
/// - Windows（0.4.7 rc.2 / W6）：WM_CLOSE + CTRL_BREAK_EVENT → 2s 宽限 →
///   TerminateJobObject（整棵进程树）+ leader 补一刀。
fn stop_child(process: &mut TrackedProcess) {
    // 先温和：Linux 整组 SIGTERM；Windows 顶层窗口 WM_CLOSE + 控制台组 CTRL_BREAK。
    #[cfg(unix)]
    sys::kill_group(process.child.id(), sys::SIGTERM);
    #[cfg(windows)]
    crate::win32_proc::request_graceful_stop(process.child.id());

    // 宽限：Linux 保持 0.3.13 的 200ms；Windows 给 Mod 2s 落盘（WM_CLOSE /
    // CTRL_BREAK 只是请求，不保证响应），超时才动硬的。
    let grace_ms = if cfg!(windows) { 2_000 } else { 200 };
    let deadline = Instant::now() + Duration::from_millis(grace_ms);
    while Instant::now() < deadline {
        if matches!(process.child.try_wait(), Ok(Some(_)) | Err(_)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    // 硬的：Linux 整组 SIGKILL；Windows 先 TerminateJobObject（整棵进程树）再对
    // leader 补一刀（Job 没建成的退化路径）。
    #[cfg(unix)]
    sys::kill_group(process.child.id(), sys::SIGKILL);
    #[cfg(windows)]
    if let Some(job) = &process.job {
        job.terminate();
    }
    #[cfg(not(unix))]
    let _ = process.child.kill();
    let _ = process.child.wait();
}

#[cfg(unix)]
#[allow(unsafe_code)] // 唯一 FFI 缝：prctl / kill（std 无封装，且不能再加依赖）。
mod sys {
    use std::os::raw::{c_int, c_ulong};

    const PR_SET_PDEATHSIG: c_int = 1;
    pub(super) const SIGTERM: c_int = 15;
    pub(super) const SIGKILL: c_int = 9;

    unsafe extern "C" {
        fn prctl(option: c_int, ...) -> c_int;
        fn kill(pid: c_int, sig: c_int) -> c_int;
    }

    /// 只能在 pre_exec（fork 之后、exec 之前）里调用；失败不改行为。
    pub(super) fn set_parent_death_signal() {
        unsafe {
            let _ = prctl(PR_SET_PDEATHSIG, SIGTERM as c_ulong);
        }
    }

    /// 杀整个进程组（负 pid）。只用于本进程记录过的 pid。
    pub(super) fn kill_group(pid: u32, sig: c_int) {
        let Ok(pid) = c_int::try_from(pid) else {
            return;
        };
        unsafe {
            let _ = kill(-pid, sig);
        }
    }
}

#[cfg(all(test, unix))]
#[path = "folder_spawn_tests.rs"]
mod tests;
