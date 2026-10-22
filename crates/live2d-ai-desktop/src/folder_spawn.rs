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
//!   PATHEXT，见 [`crate::host_layout::collect_child_env`]）；
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

    #[cfg(windows)]
    fn new(child: Child) -> Self {
        let job = crate::folder_job::Job::attach(&child);
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
    command
        .spawn()
        .map(TrackedProcess::new)
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::PermissionDenied => format!("启动失败：文件不可执行（{err}）"),
            std::io::ErrorKind::NotFound => format!("启动失败：程序不存在（{err}）"),
            _ => format!("启动失败：{err}"),
        })
}

/// 结束一个自己记录过的子进程**组**：先 SIGTERM，宽限后 SIGKILL。
///
/// 只对记录过的 pid 发信号（负 pid = 该组）；不按名字杀，所以用户另外启动的
/// 同名程序不会被误伤。leader 先退出时仍会补一次整组 SIGKILL，避免它拉起的
/// 孙进程留在组里。Windows 上先 `TerminateJobObject`（整棵进程树），再对
/// leader 补一刀后 wait。
fn stop_child(process: &mut TrackedProcess) {
    #[cfg(unix)]
    sys::kill_group(process.child.id(), sys::SIGTERM);
    #[cfg(windows)]
    if let Some(job) = &process.job {
        job.terminate();
    }

    let deadline = Instant::now() + Duration::from_millis(200);
    while Instant::now() < deadline {
        if matches!(process.child.try_wait(), Ok(Some(_)) | Err(_)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    #[cfg(unix)]
    sys::kill_group(process.child.id(), sys::SIGKILL);
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
