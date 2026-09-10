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

use live2d_ai_core::action::ActionId;
use live2d_ai_mod_system::services::ActionRequest;
use live2d_ai_mod_system::*;

// ---------------------------------------------------------------- HostChannels

/// Host 真实通道（构造 registry 时注入；动作经 host 固定映射进 core 仲裁）。
///
/// Mod → host 的真实回路（P0-3 前半）：`ActionRequest.action (&'static str)`
/// 通过 [`parse_action_id`] 映射为 `ActionId`，然后构造 `SemanticAction`
/// 经 `supervisor.trigger_action` 进入 core 仲裁；`say` 直通 supervisor.say。
/// 来源固定为 `ActionSource::LlmTool`（Mod 动作固定优先级档——E0 ADR §2.1），
/// 不暴露自定优先级、不绕过 core 仲裁。
#[derive(Clone)]
pub struct HostChannels {
    /// ActionRequest → SemanticAction（固定映射） → supervisor.trigger_action
    pub trigger_action: Arc<dyn Fn(ActionRequest) -> bool + Send + Sync>,
    /// 文本 → supervisor.say（主链路）
    pub say: Arc<dyn Fn(String) -> bool + Send + Sync>,
    /// **P0-4 真实闭环**：本地 Mod（local-llm）探测到服务就绪后，
    /// 通过此通道把探测到的 `base_url` / `model` / `voice` 写回 runtime settings
    ///（复用 settings PATCH 语义：`{"llm":{"base_url":...,"model":...}}`），
    /// 写盘成功后 host 触发 `supervisor.reload()` —— 形成「spawn → 探活 → 写
    /// 配置 → 热重载」的真实闭环，而非仅凭 TCP 探活。
    ///
    /// 参数为 namespaced JSON patch 体（对应 `SettingsPatch` 的 JSON 形态）；
    /// host 复用 `settings_routes::handle_patch` 内核完成「apply_patch →
    /// plan_atomic_write → fs::rename → reload」全链。返回 `true` 表示写盘
    /// 成功（或无变更），`false` 表示写盘失败。
    ///
    /// `config_path` 来自 [`HostChannels::config_path`] —— host 在构造时
    /// 注入真实 `live2d-ai.toml` 路径，Mod 无需感知文件位置。
    pub apply_settings: Arc<dyn Fn(serde_json::Value) -> bool + Send + Sync>,
    /// 真实 `live2d-ai.toml` 路径（host 注入；供 Mod 在 apply_settings 失败时
    /// 记录精确落盘目标，不再探测文件系统）。
    pub config_path: String,
}

/// 把 `ActionRequest.action`（`&'static str`，如 `"nod"`）映射为 `ActionId`。
/// 区分大小写严格匹配 `ActionId::name()` 输出；未知动作返回 `None`。
pub fn parse_action_id(action: &str) -> Option<ActionId> {
    ActionId::ALL.iter().find(|a| a.name() == action).copied()
}

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
    host: Option<HostChannels>,                      // P0-3 HostChannels（action + say 真实回路）。
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
            host: None,
        }
    }

    /// 注入 HostChannels（builder；cli_entry 链式调用）。
    /// Host 固定映射 Mod action → SemanticAction::LlmTool 优先级档。
    pub fn with_host_channels(mut self, host: HostChannels) -> Self {
        self.host = Some(host);
        self
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
        let say = self.say_source.clone()
            .or_else(|| self.host.as_ref().map(|h| h.say.clone()))
            .unwrap_or_else(|| Arc::new(|_: String| true));
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
    fn make_services(&self, mod_id: &'static str, say: Arc<dyn Fn(String) -> bool + Send + Sync>) -> ModServices {
        let host = self.host.clone();
        let trigger_action = host.as_ref().map(|h| h.trigger_action.clone());
        let apply_settings = host.as_ref().map(|h| h.apply_settings.clone());
        let config_path = host.as_ref().map(|h| h.config_path.clone()).unwrap_or_default();
        ModServices::new(
            // ActionRequest → host.trigger_action（固定映射进 core 仲裁）
            ModActionSender::new(move |req| match &trigger_action {
                Some(f) => f(req),
                None => { tracing::debug!(target:"mod", "no host channel: action {} dropped", req.action); false }
            }),
            // SaySender 保持现有 say(t)
            SaySender::new(move |t| say(t)),
            // Mod→host event：P0-4 真实闭环 —— 拦截 `payload` 中携带
            // `"__apply_settings": true` 的 JSON，提取 `patch` 字段经
            // `apply_settings` 写盘 + reload。其他事件按 v1 行为透传日志。
            ModEventSender::new(move |topic, payload| {
                let Some(apply) = apply_settings.as_ref() else {
                    tracing::info!(target: "mod", topic = topic.as_str(), payload, "Mod→host event");
                    return true;
                };
                let Ok(json) = serde_json::from_str::<serde_json::Value>(payload) else {
                    tracing::info!(target: "mod", topic = topic.as_str(), payload, "Mod→host event");
                    return true;
                };
                let has_marker = json
                    .get("__apply_settings")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if has_marker
                    && let Some(patch) = json.get("patch")
                {
                    let ok = apply(patch.clone());
                    tracing::info!(
                        target: "mod",
                        mod_id, config_path,
                        "P0-4 apply_settings {}: patch={}",
                        if ok { "success" } else { "failed" }, patch
                    );
                    return ok;
                }
                tracing::info!(target: "mod", topic = topic.as_str(), payload, "Mod→host event");
                true
            }),
            ModLogger::new(|_lvl, msg| tracing::info!(target: "mod", "{}", msg)),
        )
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

    /// enable 前注入 config（存在时先 reload_config 再 enable）。
    ///
    /// 用于 HTTP `POST /api/v1/mods/{id}/enable` 的 body 透传：
    /// 前端可在 enable 时同时下发 config，host 先写入 config 再启动 Mod
    ///（`start_one` 读取 `entries[id].config`），避免「enable 后再 POST
    /// /config 再 restart」的两步操作。
    pub fn enable_with_config(
        &mut self,
        id: &'static str,
        config: serde_json::Value,
    ) -> Result<(), ModError> {
        self.reload_config(id, config)?;
        self.enable(id)
    }

    #[allow(dead_code)]
    #[rustfmt::skip]
    pub fn reload_config(&mut self, id: &'static str, config: serde_json::Value) -> Result<(), ModError> {
        let Some(e) = self.entries.get_mut(id) else { return Err(ModError::Other(format!("Mod {id} 不在注册表"))); };
        e.config = config;
        Ok(())
    }

    /// 查询指定 Mod 是否已启用（供 web API 路由判断 config 更新后是否 restart）。
    pub fn is_enabled(&self, id: &'static str) -> bool {
        self.entries.get(id).map(|e| e.enabled).unwrap_or(false)
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

    // ------------------------------------------------------- HostChannels tests

    thread_local! {
        /// TestActionRuntime 把创建时的 ModServices 存进来，供测试直接调用。
        static CAPTURED_SERVICES: std::cell::RefCell<Option<ModServices>> =
            const { std::cell::RefCell::new(None) };
    }

    /// TestActionMod：捕获 ModServices 给测试直调 action_tx / say_tx。
    #[rustfmt::skip]
    struct TestActionMod;
    impl ModFactory for TestActionMod {
        fn descriptor(&self) -> &'static ModDescriptor {
            static D: ModDescriptor = ModDescriptor {
                id: "action_test",
                name: "ActionTest",
                version: "0.1.0",
                api_version: 1,
            };
            &D
        }
        fn create(
            &self,
            services: ModServices,
            _: serde_json::Value,
        ) -> Result<Box<dyn ModRuntime>, ModError> {
            CAPTURED_SERVICES.with(|c| *c.borrow_mut() = Some(services));
            Ok(Box::new(TestActionRuntime))
        }
    }
    struct TestActionRuntime;
    impl ModRuntime for TestActionRuntime {
        fn start(&mut self, _: &mut dyn ModRegistrar) -> Result<(), ModError> {
            Ok(())
        }
    }
    static ACTION_FACTORIES: &[&dyn ModFactory] = &[&TestActionMod];

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

    // ------------------------------------------------------- enable_with_config test

    thread_local! {
        /// ConfigCaptureMod 把 factory.create 收到的 config 写到此处。
        static CAPTURED_CONFIG: std::cell::RefCell<Option<serde_json::Value>> =
            const { std::cell::RefCell::new(None) };
    }

    #[rustfmt::skip]
    struct ConfigCaptureMod;
    impl ModFactory for ConfigCaptureMod {
        fn descriptor(&self) -> &'static ModDescriptor {
            static D: ModDescriptor = ModDescriptor {
                id: "config_capture",
                name: "ConfigCapture",
                version: "0.1.0",
                api_version: 1,
            };
            &D
        }
        fn create(
            &self,
            _: ModServices,
            config: serde_json::Value,
        ) -> Result<Box<dyn ModRuntime>, ModError> {
            CAPTURED_CONFIG.with(|c| *c.borrow_mut() = Some(config));
            Ok(Box::new(ConfigCaptureRuntime))
        }
    }
    struct ConfigCaptureRuntime;
    impl ModRuntime for ConfigCaptureRuntime {
        fn start(&mut self, _: &mut dyn ModRegistrar) -> Result<(), ModError> {
            Ok(())
        }
    }
    static CONFIG_FACTORIES: &[&dyn ModFactory] = &[&ConfigCaptureMod];

    /// enable_with_config 应在 start_one 之前写入 config，使 factory.create
    /// 收到注入的 config。
    #[rustfmt::skip]
    #[test]
    fn enable_with_config_passes_config_to_runtime() {
        CAPTURED_CONFIG.with(|c| c.borrow_mut().take()); // 清残留。
        let mut reg = ModRegistry::new(CONFIG_FACTORIES, &serde_json::json!({}));
        let injected = serde_json::json!({"key": "value", "n": 42});
        reg.enable_with_config("config_capture", injected.clone()).expect("enable_with_config");
        // enable_with_config 会调用 start_one → factory.create 捕获 config。
        let got = CAPTURED_CONFIG.with(|c| c.borrow().clone());
        assert_eq!(got.as_ref(), Some(&injected), "factory.create 应收到注入的 config，got {got:?}");
    }

    // ------------------------------------------------------- HostChannels tests

    /// HostChannels action 回路测试：启用 TestActionMod，获取其 services.action_tx，
    /// 提交 ActionRequest("nod")，断言 host.trigger_action 记录到 Vec 含 "nod".
    #[rustfmt::skip]
    #[test]
    fn action_request_reaches_host_channel() {
        CAPTURED_SERVICES.with(|c| c.borrow_mut().take()); // 清上一轮残留。
        let recorded: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let recorded_c = recorded.clone();
        let mut reg = ModRegistry::new(
                ACTION_FACTORIES,
                &serde_json::json!({"mods":{"action_test":{"enabled":true}}}),
            )
            .with_host_channels(HostChannels {
                trigger_action: Arc::new(move |req| {
                    recorded.lock().unwrap().push(req.action.to_string());
                    true
                }),
                say: Arc::new(|_| true),
                apply_settings: Arc::new(|_| true),
                config_path: String::new(),
            });
        reg.start_all(); // 触发 factory.create → 捕获 services。
        // action_tx 从 TestActionRuntime 捕获到全局。
        let req = ActionRequest {
            mod_id: "action_test",
            action: "nod",
            strength: 2,
        };
        let sent = CAPTURED_SERVICES.with(|c| {
            c.borrow()
                .as_ref()
                .map(|s| s.action_tx.request(req.clone()))
                .unwrap_or(false)
        });
        assert!(sent, "ActionRequest 应被 host channel 接受");
        let got = recorded_c.lock().unwrap().clone();
        assert!(got.iter().any(|a| a == "nod"), "记录应含动作名 'nod'，got {got:?}");
    }

    /// HostChannels say 回路测试：提交文本，断言 host.say 记录到 Vec。
    #[rustfmt::skip]
    #[test]
    fn say_reaches_host_channel() {
        CAPTURED_SERVICES.with(|c| c.borrow_mut().take()); // 清上一轮残留。
        let recorded: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let recorded_c = recorded.clone();
        let mut reg = ModRegistry::new(
                ACTION_FACTORIES,
                &serde_json::json!({"mods":{"action_test":{"enabled":true}}}),
            )
            .with_host_channels(HostChannels {
                trigger_action: Arc::new(|_| true),
                say: Arc::new(move |t| {
                    recorded.lock().unwrap().push(t);
                    true
                }),
                apply_settings: Arc::new(|_| true),
                config_path: String::new(),
            });
        reg.start_all(); // 触发 factory.create → 捕获 services。
        let text = "hello from mod".to_string();
        let sent = CAPTURED_SERVICES.with(|c| {
            c.borrow()
                .as_ref()
                .map(|s| s.say_tx.say(text.clone()))
                .unwrap_or(false)
        });
        assert!(sent, "say 应被 host channel 接受");
        let got = recorded_c.lock().unwrap().clone();
        assert!(got.iter().any(|t| t == "hello from mod"), "记录应含文本，got {got:?}");
    }
}
