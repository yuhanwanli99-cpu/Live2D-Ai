//! 文件夹 Mod 的**子进程生命周期**（0.3.13）。
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
//!   环境变量（env_clear 之后只回填 PATH / HOME / LANG / LC_ALL）；
//! - 子进程不阻塞对话回合：spawn 之后不 wait（只在结束时才等它退出）。
//!
//! # 平台
//!
//! Linux 上给子进程设 PR_SET_PDEATHSIG = SIGTERM（父进程退出 → 子进程收到
//! SIGTERM），并把子进程放进**独立进程组**；结束时按记录的 pid 杀**整个组**，
//! 不碰用户另外启动的同名程序。prctl / kill 没有 std 封装，而本 crate 的顶层
//! 依赖已顶格（18/18，执行单 §37）——所以这里用一段**最小 FFI**（文件末尾
//! sys 模块）。工作区 unsafe_code = deny，用函数级 allow(unsafe_code) 显式
//! 豁免；全文件只有 pre_exec 一处、sys 内部两处。
//!
//! # 失败
//!
//! 启动失败（文件不可执行 / 路径消失 / fork 失败…）只进返回值的 failed，
//! 不影响 HTTP 200：勾选已经写盘成功（执行单 §17）。本模块不 panic。

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::{Duration, Instant};

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
/// 正常关停时会顺手结束自己起过的子进程；进程被直接杀死时靠 PDEATHSIG。
#[derive(Default)]
pub struct FolderProcesses {
    children: Mutex<HashMap<String, Child>>,
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
            if let Some(mut child) = children.remove(&id) {
                stop_child(&mut child);
                report.stopped.push(id);
            }
        }

        for spec in desired {
            let alive = match children.get_mut(&spec.id) {
                Some(child) => matches!(child.try_wait(), Ok(None)),
                None => false,
            };
            if alive {
                continue;
            }
            children.remove(&spec.id);
            match spawn_one(spec) {
                Ok(child) => {
                    children.insert(spec.id.clone(), child);
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

    /// 当前**还活着**的文件夹 id（只读；仅测试用它断言进程真的起了/停了）。
    #[cfg(test)]
    pub fn running_ids(&self) -> Vec<String> {
        let mut children = self.lock();
        let mut out: Vec<String> = Vec::new();
        for (id, child) in children.iter_mut() {
            if matches!(child.try_wait(), Ok(None)) {
                out.push(id.clone());
            }
        }
        out.sort();
        out
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Child>> {
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
        for (_, mut child) in children.drain() {
            stop_child(&mut child);
        }
    }
}

/// 起一个子进程：cwd = Mod 目录，程序 = 规范化后的绝对路径。
///
/// 不加参数（执行单 §23）、不 chmod（执行单 §24）、不传 .env / 密钥环境变量
/// （执行单 §25）。失败时返回一句可直接展示的原因。
#[allow(unsafe_code)] // pre_exec 是 unsafe：子进程只做一次 prctl（见文件头）。
fn spawn_one(spec: &DesiredSpawn) -> Result<Child, String> {
    if !spec.program.is_absolute() {
        return Err("程序路径不是绝对路径（清单预检之后磁盘被改动？）".to_string());
    }
    let mut command = Command::new(&spec.program);
    command.current_dir(&spec.dir);
    command.env_clear();
    for key in ["PATH", "HOME", "LANG", "LC_ALL"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
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
    command.spawn().map_err(|err| match err.kind() {
        std::io::ErrorKind::PermissionDenied => format!("启动失败：文件不可执行（{err}）"),
        std::io::ErrorKind::NotFound => format!("启动失败：程序不存在（{err}）"),
        _ => format!("启动失败：{err}"),
    })
}

/// 结束一个自己记录过的子进程**组**：先 SIGTERM，宽限后 SIGKILL。
///
/// 只对记录过的 pid 发信号（负 pid = 该组）；不按名字杀，所以用户另外启动的
/// 同名程序不会被误伤。leader 先退出时仍会补一次整组 SIGKILL，避免它拉起的
/// 孙进程留在组里。
fn stop_child(child: &mut Child) {
    #[cfg(unix)]
    sys::kill_group(child.id(), sys::SIGTERM);
    #[cfg(not(unix))]
    let _ = child.kill();

    let deadline = Instant::now() + Duration::from_millis(200);
    while Instant::now() < deadline {
        if matches!(child.try_wait(), Ok(Some(_)) | Err(_)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    #[cfg(unix)]
    sys::kill_group(child.id(), sys::SIGKILL);
    let _ = child.wait();
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
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// 临时目录（Drop 时删干净）。
    struct Tree {
        root: PathBuf,
    }

    impl Tree {
        fn new(tag: &str) -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "l2d-folder-spawn-{}-{}-{tag}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            ));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).expect("临时目录");
            Self { root }
        }

        fn join(&self, rel: &str) -> PathBuf {
            self.root.join(rel)
        }

        /// 写一份可执行脚本（测试夹具自己 chmod；产品不 chmod）。
        fn write_exec(&self, rel: &str, body: &str) -> PathBuf {
            let path = self.join(rel);
            self.write(&path, body);
            let mut perms = std::fs::metadata(&path).expect("metadata").permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).expect("chmod");
            path
        }

        fn write(&self, path: &Path, body: &str) {
            std::fs::create_dir_all(path.parent().expect("父目录")).expect("父目录");
            std::fs::write(path, body).expect("写夹具");
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn desired(tree: &Tree, id: &str, rel: &str) -> DesiredSpawn {
        DesiredSpawn {
            id: id.to_string(),
            dir: tree.join(id),
            program: tree.join(id).join(rel),
        }
    }

    fn wait_for(path: &Path) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if path.exists() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).expect("读夹具输出")
    }

    #[test]
    fn checked_process_starts_in_its_mod_dir_and_stop_kills_it() {
        let tree = Tree::new("spawn");
        tree.write_exec(
            "demo/run.sh",
            "#!/bin/sh\npwd > cwd.txt\ntouch ran\nsleep 30\n",
        );
        let processes = FolderProcesses::new();
        let spec = desired(&tree, "demo", "run.sh");
        let report = processes.reconcile(std::slice::from_ref(&spec));
        assert_eq!(report.started, vec!["demo".to_string()]);
        assert!(report.failed.is_empty(), "{report:?}");
        assert!(wait_for(&tree.join("demo/ran")), "子进程必须真的跑起来");
        assert_eq!(
            read(&tree.join("demo/cwd.txt")).trim(),
            tree.join("demo").display().to_string(),
            "工作目录必须是该 Mod 目录"
        );
        assert_eq!(processes.running_ids(), vec!["demo".to_string()]);

        let report = processes.reconcile(&[]);
        assert_eq!(report.stopped, vec!["demo".to_string()]);
        assert!(processes.running_ids().is_empty(), "取消后不得还在跑");
    }

    #[test]
    fn second_enter_does_not_start_a_second_copy() {
        let tree = Tree::new("dup");
        tree.write_exec("dup/run.sh", "#!/bin/sh\necho x >> starts\nsleep 30\n");
        let processes = FolderProcesses::new();
        let spec = desired(&tree, "dup", "run.sh");
        let _ = processes.reconcile(std::slice::from_ref(&spec));
        assert!(wait_for(&tree.join("dup/starts")));
        let _ = processes.reconcile(std::slice::from_ref(&spec));
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(read(&tree.join("dup/starts")).lines().count(), 1);
        let _ = processes.reconcile(&[]);
    }

    #[test]
    fn exited_child_can_start_again() {
        let tree = Tree::new("restart");
        tree.write_exec("quick/run.sh", "#!/bin/sh\necho x >> runs\n");
        let processes = FolderProcesses::new();
        let spec = desired(&tree, "quick", "run.sh");
        let _ = processes.reconcile(std::slice::from_ref(&spec));
        assert!(wait_for(&tree.join("quick/runs")));
        std::thread::sleep(Duration::from_millis(200));
        let _ = processes.reconcile(std::slice::from_ref(&spec));
        let deadline = Instant::now() + Duration::from_secs(5);
        while read(&tree.join("quick/runs")).lines().count() < 2 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(read(&tree.join("quick/runs")).lines().count(), 2);
        let _ = processes.reconcile(&[]);
    }

    #[test]
    fn non_executable_program_is_reported_not_panicked() {
        let tree = Tree::new("noexec");
        tree.write(&tree.join("bad/run.sh"), "#!/bin/sh\nexit 0\n");
        let processes = FolderProcesses::new();
        let report = processes.reconcile(&[desired(&tree, "bad", "run.sh")]);
        assert!(report.started.is_empty());
        assert_eq!(report.failed.len(), 1, "{report:?}");
        assert_eq!(report.failed[0].id, "bad");
        assert!(
            report.failed[0].reason.contains("启动失败"),
            "{}",
            report.failed[0].reason
        );
        assert!(processes.running_ids().is_empty());
    }

    #[test]
    fn no_desired_entries_starts_nothing() {
        let processes = FolderProcesses::new();
        let report = processes.reconcile(&[]);
        assert_eq!(report, ReconcileReport::default());
        assert!(processes.running_ids().is_empty());
    }

    /// 父进程里的 LIVE2D_AI_SENTINEL_SECRET 不得出现在子进程环境。
    ///
    /// 用「测试二进制把自己当子进程再跑一次」的写法，避免在测试进程里
    /// set_var（edition 2024 里它是 unsafe，而本 crate deny(unsafe_code)）。
    #[test]
    fn child_environment_is_scrubbed() {
        const CHILD_MARK: &str = "L2D_FOLDER_SPAWN_ENV_CHILD";
        if std::env::var(CHILD_MARK).ok().as_deref() != Some("1") {
            let exe = std::env::current_exe().expect("当前测试二进制");
            let status = Command::new(exe)
                .args([
                    "--exact",
                    "folder_spawn::tests::child_environment_is_scrubbed",
                    "--nocapture",
                ])
                .env(CHILD_MARK, "1")
                .env("LIVE2D_AI_SENTINEL_SECRET", "do-not-pass")
                .status()
                .expect("跑子测试");
            assert!(status.success(), "子测试进程失败");
            return;
        }

        let tree = Tree::new("env");
        tree.write_exec("env/run.sh", "#!/bin/sh\nenv > env.txt\n");
        let processes = FolderProcesses::new();
        let _ = processes.reconcile(&[desired(&tree, "env", "run.sh")]);
        let out = tree.join("env/env.txt");
        assert!(wait_for(&out), "子进程应留下环境快照");
        // 文件先被创建、内容随后写入：等到非空再断言，避免读到半截。
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut text = read(&out);
        while text.is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
            text = read(&out);
        }
        assert!(!text.is_empty(), "环境快照为空");
        assert!(
            !text.contains("LIVE2D_AI_SENTINEL_SECRET"),
            "密钥环境变量不得传给子进程：{text}"
        );
        assert!(text.lines().any(|line| line.starts_with("PATH=")), "{text}");
        for line in text.lines() {
            let key = line.split('=').next().unwrap_or("");
            for banned in ["KEY", "TOKEN", "SECRET"] {
                assert!(
                    !key.contains(banned),
                    "环境变量 {key} 不许传给子进程：{text}"
                );
            }
        }
        let _ = processes.reconcile(&[]);
    }

    /// 结束时杀的是整组：脚本拉起的孙进程也不能留在 /proc 里。
    #[cfg(target_os = "linux")]
    #[test]
    fn stop_kills_the_whole_process_group() {
        let tree = Tree::new("group");
        tree.write_exec(
            "grp/run.sh",
            "#!/bin/sh\nsleep 30 &\necho $! > grand.pid\nwait\n",
        );
        let processes = FolderProcesses::new();
        let _ = processes.reconcile(&[desired(&tree, "grp", "run.sh")]);
        assert!(wait_for(&tree.join("grp/grand.pid")), "孙进程应已出现");
        let pid: u32 = read(&tree.join("grp/grand.pid"))
            .trim()
            .parse()
            .expect("pid");
        assert!(Path::new("/proc").join(pid.to_string()).exists());
        let _ = processes.reconcile(&[]);
        let deadline = Instant::now() + Duration::from_secs(5);
        while Path::new("/proc").join(pid.to_string()).exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            !Path::new("/proc").join(pid.to_string()).exists(),
            "孙进程 {pid} 必须随组一起结束"
        );
    }
}
