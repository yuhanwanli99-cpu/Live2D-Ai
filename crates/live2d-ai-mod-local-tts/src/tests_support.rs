//! `local-tts` 单测共享脚手架（假子进程 / 假 spawner / 计数间谍 / 注册器）。
//!
//! 拆出来的理由：`crates/*/src` 下的 `.rs` **按文件总行数**计（>500 会顶高
//! `RATCHET_SRC_RS_500` 的计数），所以测试按场景拆成 `tests` / `tests_command`
//! 两个文件，共享件放这里（先例 `mod_registry/tests_support.rs`）。
//!
//! 全程**不 exec 真进程**：假句柄由 [`SpawnerLog::spawner`] 产出。

use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::*;

/// 假子进程的内部状态（测试直接改它来制造「已退出 / 探测失败 / 停不掉」）。
#[derive(Debug)]
pub struct FakeState {
    pub pid: u32,
    pub running: bool,
    pub exit_code: Option<i32>,
    pub probe_error: bool,
    pub stop_error: bool,
    pub wait_error: bool,
    pub stops: usize,
    pub waits: usize,
    /// 假句柄的 stderr 尾巴（[ChildProcess::stderr_tail] 的回话）。
    pub stderr: String,
}

impl FakeState {
    fn running(pid: u32) -> Self {
        Self {
            pid,
            running: true,
            exit_code: None,
            probe_error: false,
            stop_error: false,
            wait_error: false,
            stops: 0,
            waits: 0,
            stderr: String::new(),
        }
    }
}

/// 假句柄：`stop()` 成功即置「已退出（码 0）」，并计数。
///
/// 句柄与测试共享同一个 `Arc<Mutex<FakeState>>`——测试因此能在 runtime 持有
/// 句柄的同时制造「已退出 / 探测失败 / 停不掉」。
struct FakeChild {
    state: Arc<Mutex<FakeState>>,
}

impl ChildProcess for FakeChild {
    fn pid(&self) -> Option<u32> {
        Some(self.state.lock().unwrap().pid)
    }

    fn try_wait(&mut self) -> io::Result<ChildProbe> {
        let s = self.state.lock().unwrap();
        if s.probe_error {
            return Err(io::Error::other("probe 失败"));
        }
        if s.running {
            Ok(ChildProbe::Running)
        } else {
            Ok(ChildProbe::Exited { code: s.exit_code })
        }
    }

    fn stop(&mut self) -> io::Result<()> {
        let mut s = self.state.lock().unwrap();
        s.stops += 1;
        if s.stop_error {
            return Err(io::Error::other("kill 失败"));
        }
        s.running = false;
        if s.exit_code.is_none() {
            s.exit_code = Some(0);
        }
        Ok(())
    }

    fn wait(&mut self) -> io::Result<()> {
        let mut s = self.state.lock().unwrap();
        s.waits += 1;
        if s.wait_error {
            return Err(io::Error::other("wait 失败"));
        }
        Ok(())
    }

    /// 假句柄的「脚本输出」：空串按「没有」处理（与真实现同口径）。
    fn stderr_tail(&self) -> Option<String> {
        let s = self.state.lock().unwrap();
        if s.stderr.trim().is_empty() {
            None
        } else {
            Some(s.stderr.clone())
        }
    }
}

/// 记录 spawner 调用：每次调用产出一个**新的**假句柄，并把它的共享状态留下。
#[derive(Default)]
pub struct SpawnerLog {
    requests: Mutex<Vec<SpawnRequest>>,
    children: Mutex<Vec<Arc<Mutex<FakeState>>>>,
    /// `Some(code)` = 新假句柄**一出生就是已退出**（`None` = 一直 Running）。
    ///
    /// 用来复现「脚本起来就失败」：真子进程在 spawn 后立刻退出，假句柄用这个
    /// 开关在同一个位置制造同样的观察结果。
    spawn_exit: Mutex<Option<Option<i32>>>,
    /// 新假句柄出生时带的 stderr 尾巴（模拟脚本印出的原因）。
    spawn_stderr: Mutex<Option<String>>,
}

impl SpawnerLog {
    /// 可注入的 spawner（不 exec 真进程）。
    pub fn spawner(self: &Arc<Self>) -> Spawner {
        let log = Arc::clone(self);
        Box::new(move |req: &SpawnRequest| {
            let mut fresh = FakeState::running(4242);
            if let Some(code) = *log.spawn_exit.lock().unwrap() {
                fresh.running = false;
                fresh.exit_code = code;
            }
            if let Some(text) = log.spawn_stderr.lock().unwrap().clone() {
                fresh.stderr = text;
            }
            let state = Arc::new(Mutex::new(fresh));
            log.children.lock().unwrap().push(Arc::clone(&state));
            log.requests.lock().unwrap().push(req.clone());
            Ok(Box::new(FakeChild { state }) as Box<dyn ChildProcess>)
        })
    }

    /// spawner 被调用了几次。
    pub fn calls(&self) -> usize {
        self.requests.lock().unwrap().len()
    }

    /// 最近一次 spawn 的请求（argv / workdir）。
    pub fn last_request(&self) -> SpawnRequest {
        self.requests
            .lock()
            .unwrap()
            .last()
            .cloned()
            .expect("没有 spawn 过")
    }

    /// 对最近一次 spawn 出来的假子进程状态做一次读/改。
    pub fn with_state<T>(&self, f: impl FnOnce(&mut FakeState) -> T) -> T {
        let state = Arc::clone(self.children.lock().unwrap().last().expect("没有 spawn 过"));
        let mut guard = state.lock().unwrap();
        f(&mut guard)
    }

    /// 让**之后** spawn 出来的假句柄一出生就已退出（退出码 + 脚本输出）。
    pub fn child_exits_on_spawn(&self, code: Option<i32>, stderr: &str) {
        *self.spawn_exit.lock().unwrap() = Some(code);
        *self.spawn_stderr.lock().unwrap() = Some(stderr.to_string());
    }
}

/// `apply_settings` 计数间谍（口径：本 Mod 的调用次数恒为 0）。
pub fn spy_services(applied: &Arc<AtomicUsize>) -> ModServices {
    let counter = Arc::clone(applied);
    ModServices::new(
        ModActionSender::new(|_| false),
        SaySender::new(|_| true),
        ModEventSender::new(|_, _| true),
        ModLogger::new(|_, _| {}),
    )
    .with_apply_settings(ModSettingsApplier::new(move |_| {
        counter.fetch_add(1, Ordering::SeqCst);
        true
    }))
}

/// 记录 `register_settings` 的测试注册器。
#[derive(Default)]
pub struct RecordingRegistrar {
    pub specs: Vec<ModSettingsSpec>,
}

impl ModRegistrar for RecordingRegistrar {
    fn register_settings(&mut self, spec: ModSettingsSpec) -> Result<(), ModError> {
        self.specs.push(spec);
        Ok(())
    }
    fn subscribe(&mut self, _: ModEventTopic) -> Result<SubscriptionId, ModError> {
        Ok(SubscriptionId(1))
    }
    fn unsubscribe(&mut self, _: SubscriptionId) -> Result<(), ModError> {
        Ok(())
    }
}

/// 造一个注入假 spawner 的 runtime（并把线程局部 `StartCause` 设好）。
pub fn runtime_with(
    config: serde_json::Value,
    cause: StartCause,
    log: &Arc<SpawnerLog>,
) -> (LocalTtsRuntime, Arc<AtomicUsize>) {
    let applied = Arc::new(AtomicUsize::new(0));
    set_start_cause(cause);
    let parsed = LaunchConfig::from_json(&config).expect("配置应可解析");
    let rt = LocalTtsRuntime::with_spawner(spy_services(&applied), parsed, log.spawner());
    (rt, applied)
}

/// 走一次 `ModRuntime::start`（用一次性注册器）。
pub fn start(rt: &mut LocalTtsRuntime) -> Result<(), ModError> {
    let mut reg = RecordingRegistrar::default();
    rt.start(&mut reg)
}

/// 同 [runtime_with]，但**显式给观察窗口**（复现「子进程立刻退出」要它非 0）。
pub fn runtime_with_grace(
    config: serde_json::Value,
    cause: StartCause,
    log: &Arc<SpawnerLog>,
    grace: std::time::Duration,
) -> (LocalTtsRuntime, Arc<AtomicUsize>) {
    let applied = Arc::new(AtomicUsize::new(0));
    set_start_cause(cause);
    let parsed = LaunchConfig::from_json(&config).expect("配置应可解析");
    let rt = LocalTtsRuntime::with_spawner(spy_services(&applied), parsed, log.spawner())
        .with_immediate_exit_grace(grace);
    (rt, applied)
}
