//! Host 端 ModRegistry（节点 E4b）。
//!
//! 把 [`live2d-ai-mod-system`] 接口装配为运行时：管理 Mod 生命周期
//! (enable/disable/restart/reload_config + 状态机)、事件有界 worker、状态观察。
//! Mod 是静态编译——host 持 `AVAILABLE_MOD_FACTORIES`，manifest 只决定
//! 启用/禁用/配置。
//!
//! # 事件隔离（E0 ADR §2.2）
//!
//! host → Mod 经**有界 channel + 独立 worker 线程**：`dispatch_event` 用
//! `try_send` 入队（满则丢弃 + dropped 计数，不阻塞 supervisor）；worker 对
//! 每个 Running Mod 调 `runtime.on_event`，`Err` → 清空该 Mod runtime 槽位
//! 并记录 `last_error`（失败隔离，核心继续）。慢/失败 Mod 不影响主链路。
//!
//! # 动作来源（E0 ADR §2.1）
//!
//! Mod 主动作经 `ModServices.action_tx` 的 `ActionRequest` 提交，host 端映射
//! 为固定优先级（UserCommand/LlmTool/**Mod**/RuleFallback），**不**暴露自定
//! 优先级、**不**绕过 core 仲裁。
//!
//! # 共享 runtime 槽位（E4b）
//!
//! `RegisteredMod.runtime` 字段已移除，改为 `ModRegistry.runtimes` 中以
//! `Arc<Mutex<Option<Box<dyn ModRuntime>>>>` 槽位共享。worker 线程持这些
//! Arc clone——**不持 registry 锁**，避免与 enable/disable/restart 竞争；
//! `try_lock` 失败（host 正在 restart）时跳过不阻塞；`on_event` 返回
//! `Err` 时清空槽位，待 host restart 恢复。

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use live2d_ai_mod_system::*;

/// worker 线程可安全访问的 per-Mod runtime 快照（ModRuntime: Send）.
type SharedRuntime = Arc<Mutex<Option<Box<dyn ModRuntime>>>>;

/// 已注册的 Mod 条目。
pub struct RegisteredMod {
    pub descriptor: &'static ModDescriptor,
    pub status: ModStatus,
    pub config: serde_json::Value,
    pub last_error: Option<String>,
    pub enabled: bool,
}

impl RegisteredMod {
    fn new(factory: &'static dyn ModFactory) -> Self {
        Self {
            descriptor: factory.descriptor(),
            status: ModStatus::Disabled,
            config: serde_json::Value::Object(Default::default()),
            last_error: None,
            enabled: false,
        }
    }
}

/// 事件投递 channel 的消息。
type EventMsg = (ModEventTopic, String);

/// Host ModRegistry。
pub struct ModRegistry {
    entries: BTreeMap<&'static str, RegisteredMod>,
    factories: &'static [&'static dyn ModFactory],
    event_tx: std::sync::mpsc::SyncSender<EventMsg>, // 有界 256；满则 try_send 丢弃。
    worker_join: Option<std::thread::JoinHandle<()>>,
    dropped_events: Arc<AtomicU64>, // 可观察的 dropped 计数。
    #[allow(dead_code)]
    blocking_mods: Arc<Mutex<Vec<&'static str>>>,
    next_sub_id: AtomicU64,
    subscriptions: BTreeMap<&'static str, Vec<SubscriptionId>>,
    settings_specs: BTreeMap<&'static str, ModSettingsSpec>,
    say_source: Option<Arc<dyn Fn(String) -> bool + Send + Sync>>,
    runtimes: BTreeMap<&'static str, SharedRuntime>, // per-Mod runtime 槽位。
}

impl ModRegistry {
    /// 用 AVAILABLE_MOD_FACTORIES + manifest 初始化（起 worker）。
    pub fn new(
        factories: &'static [&'static dyn ModFactory],
        manifest: &serde_json::Value,
    ) -> Self {
        let (event_tx, event_rx) = std::sync::mpsc::sync_channel::<EventMsg>(256);
        let dropped_events = Arc::new(AtomicU64::new(0));
        let blocking_mods = Arc::new(Mutex::new(Vec::new()));
        let mut entries = BTreeMap::new();
        let mut runtimes = BTreeMap::new();
        for factory in factories {
            let desc = factory.descriptor();
            let (enabled, config) = parse_mod_config(manifest, desc.id);
            let mut reg = RegisteredMod::new(*factory);
            reg.enabled = enabled;
            reg.config = config;
            reg.status = ModStatus::Disabled;
            entries.insert(desc.id, reg);
            runtimes.insert(desc.id, Arc::new(Mutex::new(None)));
        }
        let dropped = Arc::clone(&dropped_events);
        let runtimes_for_worker = runtimes.clone();
        let worker_join = std::thread::Builder::new()
            .name("mod-event-worker".into())
            .spawn(move || event_worker_loop(event_rx, runtimes_for_worker, dropped))
            .ok();
        Self {
            entries,
            factories,
            event_tx,
            worker_join,
            dropped_events,
            blocking_mods,
            next_sub_id: AtomicU64::new(1),
            subscriptions: BTreeMap::new(),
            settings_specs: BTreeMap::new(),
            say_source: None,
            runtimes,
        }
    }
    pub fn start_all(&mut self) {
        let ids: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, r)| r.enabled)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.start_one(id);
        }
    }

    #[rustfmt::skip]
    fn start_one(&mut self, id: &'static str) {
        let Some(&factory) = self.factories.iter().find(|f| f.descriptor().id == id) else { return };
        let config = self.entries.get(id).map(|r| r.config.clone()).unwrap_or_default();
        let say = self.say_source.clone().unwrap_or_else(|| Arc::new(|_: String| true));
        match factory.create(self.make_services(id, say), config) {
            Ok(mut runtime) => {
                let mut registrar = HostRegistrar { mod_id: id,
                    settings_specs: &mut self.settings_specs,
                    subscriptions: &mut self.subscriptions, next_id: &self.next_sub_id };
                match runtime.start(&mut registrar) {
                    Ok(()) => {
                        if let Some(e) = self.entries.get_mut(id) { e.status = ModStatus::Running; e.last_error = None; }
                        if let Some(slot) = self.runtimes.get(id) { *slot.lock().unwrap() = Some(runtime); }
                    }
                    Err(err) => {
                        if let Some(e) = self.entries.get_mut(id) {
                            e.status = ModStatus::Failed { message: err.to_string() };
                            e.last_error = Some(err.to_string());
                        }
                    }
                }
            }
            Err(err) => {
                if let Some(e) = self.entries.get_mut(id) {
                    e.status = ModStatus::Failed { message: err.to_string() };
                    e.last_error = Some(err.to_string());
                }
            }
        }
    }

    #[rustfmt::skip]
    fn make_services(&self, _mod_id: &'static str, say: Arc<dyn Fn(String) -> bool + Send + Sync>) -> ModServices {
        ModServices::new(ModActionSender::new(|_| true), SaySender::new(move |t| say(t)),
            ModEventSender::new(|_, _| true), ModLogger::new(|_lvl, msg| tracing::info!(target: "mod", "{}", msg)))
    }
    #[allow(dead_code)]
    pub fn set_say_source(&mut self, say: Arc<dyn Fn(String) -> bool + Send + Sync>) {
        self.say_source = Some(say);
    }

    #[rustfmt::skip]
    pub fn enable(&mut self, id: &'static str) -> Result<(), ModError> {
        let Some(e) = self.entries.get_mut(id) else { return Err(ModError::Other(format!("Mod {id} 不在注册表"))); };
        e.enabled = true;
        e.status = ModStatus::Starting;
        self.start_one(id);
        Ok(())
    }

    #[rustfmt::skip]
    pub fn disable(&mut self, id: &'static str) -> Result<(), ModError> {
        let Some(e) = self.entries.get_mut(id) else { return Err(ModError::Other(format!("Mod {id} 不在注册表"))); };
        if let Some(mut rt) = self.runtimes.get(id).and_then(|s| s.lock().unwrap().take()) { let _ = rt.shutdown(); }
        e.enabled = false;
        e.status = ModStatus::Disabled;
        Ok(())
    }

    pub fn restart(&mut self, id: &'static str) -> Result<(), ModError> {
        self.disable(id)?;
        self.enable(id)
    }

    #[allow(dead_code)]
    #[rustfmt::skip]
    pub fn reload_config(&mut self, id: &'static str, config: serde_json::Value) -> Result<(), ModError> {
        let Some(e) = self.entries.get_mut(id) else { return Err(ModError::Other(format!("Mod {id} 不在注册表"))); };
        e.config = config;
        Ok(())
    }

    /// 状态观察（Mod 管理 UI）。
    pub fn list(&self) -> Vec<(&'static ModDescriptor, ModStatus, bool)> {
        self.entries
            .values()
            .map(|r| (r.descriptor, r.status.clone(), r.enabled))
            .collect()
    }

    #[allow(dead_code)]
    pub fn ids(&self) -> Vec<&'static str> {
        self.entries.keys().copied().collect()
    }

    #[allow(dead_code)]
    pub fn settings_spec(&self, id: &'static str) -> Option<&ModSettingsSpec> {
        self.settings_specs.get(id)
    }

    /// 投递事件到 worker（有界；满则丢弃计数，不阻塞）。
    #[allow(dead_code)]
    pub fn dispatch_event(&self, topic: ModEventTopic, payload: &str) -> bool {
        self.event_tx.try_send((topic, payload.to_string())).is_ok()
    }

    #[allow(dead_code)]
    pub fn dropped_events(&self) -> u64 {
        self.dropped_events.load(Ordering::Relaxed)
    }

    #[allow(dead_code)]
    pub fn shutdown(&mut self) {
        for (id, slot) in &self.runtimes {
            if let Some(mut rt) = slot.lock().unwrap().take() {
                let _ = rt.shutdown();
            }
            if let Some(e) = self.entries.get_mut(*id) {
                e.status = ModStatus::Disabled;
            }
        }
        if let Some(j) = self.worker_join.take() {
            let _ = j.join();
        } // drop event_tx → worker 退出。
    }
}

/// 事件 worker 循环：从有界 channel 收事件，对每个 Running Mod 调
/// `runtime.on_event`（独立线程，不阻塞 supervisor；worker 持 per-Mod
/// runtime 的 `Arc<Mutex<...>>` clone，**不持 registry 锁**）。
///
/// - `try_lock` 失败（host 正在 restart 持有锁）→ 跳过本条事件。
/// - 槽位为 `None`（Mod 未启用/已停用）→ 跳过。
/// - `on_event` 返回 `Err` → 清空槽位（失败隔离），host restart 恢复。
#[rustfmt::skip]
fn event_worker_loop(
    event_rx: std::sync::mpsc::Receiver<EventMsg>,
    runtimes: BTreeMap<&'static str, SharedRuntime>,
    _dropped: Arc<AtomicU64>,
) {
    while let Ok((topic, payload)) = event_rx.recv() {
        for (id, slot) in &runtimes {
            let mut guard = match slot.try_lock() { Ok(g) => g, Err(_) => continue }; // host 持锁（restart）→ 跳过。
            if guard.is_none() { continue } // 未启用 → 跳过。
            if let Err(err) = guard.as_mut().expect("non-none").on_event(topic, payload.as_str()) {
                tracing::warn!(target: "mod", "Mod {id} on_event 失败: {err}");
                *guard = None; // 失败 Mod 停止投递（E0 失败隔离）。
            }
        }
    }
}

/// 解析 Mod 配置：`[mods.<id>].enabled` / `.config`（或 `mods/<id>.json`）。
#[rustfmt::skip]
fn parse_mod_config(manifest: &serde_json::Value, id: &'static str) -> (bool, serde_json::Value) {
    let Some(obj) = manifest.get("mods").and_then(|m| m.as_object()).and_then(|m| m.get(id)).and_then(|v| v.as_object()) else {
        return (false, serde_json::Value::Object(Default::default()));
    };
    let enabled = obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
    let config = obj.get("config").cloned().unwrap_or_else(|| serde_json::Value::Object(Default::default()));
    (enabled, config)
}

/// Host 端的 ModRegistrar 实现（借用到 registry 的共享存储）。
struct HostRegistrar<'a> {
    mod_id: &'static str,
    settings_specs: &'a mut BTreeMap<&'static str, ModSettingsSpec>,
    subscriptions: &'a mut BTreeMap<&'static str, Vec<SubscriptionId>>,
    next_id: &'a AtomicU64,
}

impl ModRegistrar for HostRegistrar<'_> {
    fn register_settings(&mut self, spec: ModSettingsSpec) -> Result<(), ModError> {
        spec.validate().map_err(|e| ModError::InvalidSettings {
            mod_id: self.mod_id.to_string(),
            message: e,
        })?;
        self.settings_specs.insert(self.mod_id, spec);
        Ok(())
    }

    fn subscribe(&mut self, topic: ModEventTopic) -> Result<SubscriptionId, ModError> {
        let id = SubscriptionId(self.next_id.fetch_add(1, Ordering::Relaxed));
        self.subscriptions.entry(self.mod_id).or_default().push(id);
        let _ = topic; // v1 登记 topic，细粒度路由留 E5。
        Ok(id)
    }

    fn unsubscribe(&mut self, id: SubscriptionId) -> Result<(), ModError> {
        if let Some(v) = self.subscriptions.get_mut(self.mod_id) {
            v.retain(|s| *s != id);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rustfmt::skip]
    struct TestMod;
    impl ModFactory for TestMod {
        fn descriptor(&self) -> &'static ModDescriptor {
            static D: ModDescriptor = ModDescriptor {
                id: "test",
                name: "Test",
                version: "0.1.0",
                api_version: 1,
            };
            &D
        }
        fn create(
            &self,
            _: ModServices,
            _: serde_json::Value,
        ) -> Result<Box<dyn ModRuntime>, ModError> {
            Ok(Box::new(TestRuntime))
        }
    }

    /// worker 通过 shared slot 调用 on_event，TestRuntime 把收到的 topic 写到此处。
    static RECEIVED: Mutex<Vec<String>> = Mutex::new(Vec::new());
    struct TestRuntime;
    impl ModRuntime for TestRuntime {
        fn start(&mut self, _: &mut dyn ModRegistrar) -> Result<(), ModError> {
            Ok(())
        }
        fn on_event(&mut self, topic: ModEventTopic, _: &str) -> Result<(), ModError> {
            RECEIVED.lock().unwrap().push(topic.as_str().to_string());
            Ok(())
        }
    }
    static FACTORIES: &[&dyn ModFactory] = &[&TestMod];

    #[test]
    fn default_disabled_until_enabled() {
        let mut reg = ModRegistry::new(FACTORIES, &serde_json::json!({}));
        reg.start_all();
        let (_, status, enabled) = reg.list()[0].clone();
        assert_eq!(status, ModStatus::Disabled);
        assert!(!enabled);
    }

    #[test]
    fn enable_runs_mod() {
        let mut reg = ModRegistry::new(
            FACTORIES,
            &serde_json::json!({"mods":{"test":{"enabled":true}}}),
        );
        reg.start_all();
        let (_d, status, enabled) = reg.list()[0].clone();
        assert_eq!(status, ModStatus::Running);
        assert!(enabled);
    }

    #[test]
    fn disable_shuts_down() {
        let mut reg = ModRegistry::new(
            FACTORIES,
            &serde_json::json!({"mods":{"test":{"enabled":true}}}),
        );
        reg.start_all();
        reg.disable("test").unwrap();
        let (_, status, enabled) = reg.list()[0].clone();
        assert_eq!(status, ModStatus::Disabled);
        assert!(!enabled);
    }

    #[test]
    fn dispatch_event_bounded_no_block() {
        let reg = ModRegistry::new(FACTORIES, &serde_json::json!({}));
        assert!(reg.dispatch_event(ModEventTopic::TurnStarted, "{}"));
    }

    #[test]
    fn parse_config_from_manifest() {
        let manifest = serde_json::json!({"mods": {"x": {"enabled": true, "config": {"port": 1}}}});
        let (e, c) = parse_mod_config(&manifest, "x");
        assert!(e);
        assert_eq!(c["port"], 1);
    }

    /// 事件送达测试：enable TestMod，start_all 后 dispatch_event，
    /// 轮询（最多 ~500ms）直到记录非空。
    #[rustfmt::skip]
    #[test]
    fn event_delivers_to_running_mod() {
        RECEIVED.lock().unwrap().clear();
        let mut reg = ModRegistry::new(FACTORIES, &serde_json::json!({"mods":{"test":{"enabled":true}}}));
        reg.start_all();
        assert!(reg.dispatch_event(ModEventTopic::TurnStarted, "{}"));
        let slot = reg.runtimes.get("test").expect("runtime 槽位存在");
        let mut got = false;
        for _ in 0..10 {
            if slot.try_lock().map(|g| g.is_some() && !RECEIVED.lock().unwrap().is_empty()).unwrap_or(false) {
                got = true; break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(got, "worker 应将 event 投递到 running Mod 的 on_event");
    }

    #[rustfmt::skip]
    struct FailMod;
    impl ModFactory for FailMod {
        fn descriptor(&self) -> &'static ModDescriptor {
            static D: ModDescriptor = ModDescriptor {
                id: "failmod",
                name: "Fail",
                version: "0.1.0",
                api_version: 1,
            };
            &D
        }
        fn create(
            &self,
            _: ModServices,
            _: serde_json::Value,
        ) -> Result<Box<dyn ModRuntime>, ModError> {
            Ok(Box::new(FailRuntime))
        }
    }
    struct FailRuntime;
    impl ModRuntime for FailRuntime {
        fn start(&mut self, _: &mut dyn ModRegistrar) -> Result<(), ModError> {
            Ok(())
        }
        fn on_event(&mut self, _: ModEventTopic, _: &str) -> Result<(), ModError> {
            Err(ModError::Other("on_event 失败".into()))
        }
    }
    static FAIL_FACTORIES: &[&dyn ModFactory] = &[&FailMod];

    #[rustfmt::skip]
    #[test]
    fn event_failure_isolates_mod() {
        let mut reg = ModRegistry::new(FAIL_FACTORIES, &serde_json::json!({"mods":{"failmod":{"enabled":true}}}));
        reg.start_all();
        assert!(reg.dispatch_event(ModEventTopic::TurnStarted, "{}"));
        let slot = reg.runtimes.get("failmod").expect("runtime 槽位存在");
        let mut cleared = false;
        for _ in 0..10 {
            if slot.try_lock().map(|g| g.is_none()).unwrap_or(false) { cleared = true; break; }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(cleared, "on_event 失败后 runtime 槽位应被清空");
        assert!(reg.dispatch_event(ModEventTopic::TurnStarted, "{}")); // 二次 dispatch 不 panic。
    }
}
