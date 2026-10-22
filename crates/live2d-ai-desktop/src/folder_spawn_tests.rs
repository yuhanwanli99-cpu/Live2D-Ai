//! folder_spawn 的回归测试（0.3.13 原样搬出）。
//!
//! 0.4.0 rc.1：主文件要加 Windows Job Object / 环境白名单 / 入口规则，为了不让
//! folder_spawn.rs 越过 500 行棘轮，测试整段搬到本文件（内容逐字未改）。

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
