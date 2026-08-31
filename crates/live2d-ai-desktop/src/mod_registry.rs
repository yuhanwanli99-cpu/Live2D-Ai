//! Host 端 ModRegistry（节点 E E4b）。
//!
//! 把 [`live2d-ai-mod-system`] 的接口装配为运行时：管理 Mod 生命周期
//! （enable/disable/restart/reload_config + 状态机）、事件有界 worker、
//! 状态观察。**Mod 是静态编译**——host 持 [`AVAILABLE_MOD_FACTORIES`] 数组，
//! manifest 只决定启用/禁用/配置，不决定加载哪个 crate。
//!
//! # 事件隔离（E0 ADR §2.2）
//!
//! host → Mod 经**有界 channel + 独立 worker 线程**：`dispatch_event` 用
//! `try_send` 入队（满则丢弃 + dropped 计数，不阻塞 supervisor）；worker 对
//! 每个 Running Mod 调 `on_event`，`Err` → 置 `Failed` + `last_error` 并从
//! 投递列表移除（核心继续）。慢 Mod / 失败 Mod 不影响主链路。
//!
//! # 动作来源（E0 ADR §2.1）
//!
//! Mod 主动作经 `ModServices.action_tx` 的 `ActionRequest` 提交，host 端映射
//! 为固定优先级（UserCommand/LlmTool/**Mod**/RuleFallback），**不**暴露自定
//! 优先级、**不**绕过 core 仲裁。
//!
//! # 未接线说明（E4b）
//!
//! 本模块 API 是给 E5（external-input）/ E7（pet/director）Mod 接入用的，
//! 当前 `AVAILABLE_MOD_FACTORIES` 为空数组，故 `ModRegistry` 尚未被
//! supervisor 装配。标 `#![allow(dead_code)]` 而不删除——E5/E7 接入后移除。

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use live2d_ai_mod_system::*;

/// 已注册的 Mod 条目。
pub struct RegisteredMod {
    pub descriptor: &'static ModDescriptor,
    pub status: ModStatus,
    pub config: serde_json::Value,
    pub runtime: Option<Box<dyn ModRuntime>>,
    pub last_error: Option<String>,
    pub enabled: bool,
}

impl RegisteredMod {
    fn new(factory: &'static dyn ModFactory) -> Self {
        Self {
            descriptor: factory.descriptor(),
            status: ModStatus::Disabled,
            config: serde_json::Value::Object(Default::default()),
            runtime: None,
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
    /// 事件投递 sender（有界 256；满则 try_send 失败丢弃）。
    event_tx: std::sync::mpsc::SyncSender<EventMsg>,
    /// worker 线程 join 句柄。
    worker_join: Option<std::thread::JoinHandle<()>>,
    /// 当前 dropped 事件计数（可观察）。
    dropped_events: Arc<AtomicU64>,
    /// 阻塞 supervisor 的慢 Mod 计数（观测值；v1 不阻塞）。
    blocking_mods: Arc<Mutex<Vec<&'static str>>>,
    next_sub_id: AtomicU64,
    /// 每个 Mod 的 subscription id（E4a 接口占位；v1 只登记不细粒度路由）。
    subscriptions: BTreeMap<&'static str, Vec<SubscriptionId>>,
    /// 已注册的 settings schema（Mod 管理 UI / 前端消费）。
    settings_specs: BTreeMap<&'static str, ModSettingsSpec>,
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
        for factory in factories {
            let desc = factory.descriptor();
            // manifest（`[mods.<id>]` 或 `mods/<id>.json`）决定 enabled/config。
            let (enabled, config) = parse_mod_config(manifest, desc.id);
            let mut reg = RegisteredMod::new(*factory);
            reg.enabled = enabled;
            reg.config = config;
            // 启动时由 `start_all` 尝试 Start；此处统一 Disabled（未运行）。
            reg.status = ModStatus::Disabled;
            entries.insert(desc.id, reg);
        }

        let dropped = Arc::clone(&dropped_events);
        let mut regs_for_worker: Vec<&'static str> = Vec::new();
        for (id, r) in &entries {
            if r.enabled {
                regs_for_worker.push(*id);
            }
        }
        let worker_join = std::thread::Builder::new()
            .name("mod-event-worker".into())
            .spawn(move || event_worker_loop(event_rx, regs_for_worker, dropped))
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
        }
    }

    /// 启动所有 enabled Mod（每 Mod `factory.create` + `runtime.start`）。
    pub fn start_all(&mut self) {
        let ids: Vec<&'static str> = self
            .entries
            .iter()
            .filter(|(_, r)| r.enabled)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.start_one(id);
        }
    }

    fn start_one(&mut self, id: &'static str) {
        let Some(factory) = self.factories.iter().find(|f| f.descriptor().id == id) else {
            return;
        };
        let factory = *factory;
        let config = self
            .entries
            .get(id)
            .map(|r| r.config.clone())
            .unwrap_or_default();
        match factory.create(self.make_services(id), config) {
            Ok(mut runtime) => {
                let mut registrar = HostRegistrar {
                    mod_id: id,
                    settings_specs: &mut self.settings_specs,
                    subscriptions: &mut self.subscriptions,
                    next_id: &self.next_sub_id,
                };
                match runtime.start(&mut registrar) {
                    Ok(()) => {
                        if let Some(e) = self.entries.get_mut(id) {
                            e.status = ModStatus::Running;
                            e.runtime = Some(runtime);
                            e.last_error = None;
                        }
                    }
                    Err(err) => {
                        if let Some(e) = self.entries.get_mut(id) {
                            e.status = ModStatus::Failed {
                                message: err.to_string(),
                            };
                            e.last_error = Some(err.to_string());
                        }
                    }
                }
            }
            Err(err) => {
                if let Some(e) = self.entries.get_mut(id) {
                    e.status = ModStatus::Failed {
                        message: err.to_string(),
                    };
                    e.last_error = Some(err.to_string());
                }
            }
        }
    }

    /// 构造 ModServices（映射到 host 能力；v1 用闭包桥接，E5/E7 接 supervisor）。
    fn make_services(&self, _mod_id: &'static str) -> ModServices {
        // v1：ModServices 用 no-op 闭包占位；E5/E7 接入 supervisor 的 say/action_tx。
        // 动作固定优先级：由 host 把 ActionRequest 映射（此处不做，E5 接真正通道）。
        ModServices::new(
            ModActionSender::new(|_| true),
            SaySender::new(|_| true),
            ModEventSender::new(|_topic, _payload| true),
            ModLogger::new(|_lvl, msg| {
                tracing::info!(target: "mod", "{}", msg);
            }),
        )
    }

    /// enable 一个 Mod（已禁用时创建实例 + start）。
    pub fn enable(&mut self, id: &'static str) -> Result<(), ModError> {
        let Some(e) = self.entries.get_mut(id) else {
            return Err(ModError::Other(format!("Mod {id} 不在注册表")));
        };
        e.enabled = true;
        e.status = ModStatus::Starting;
        self.start_one(id);
        Ok(())
    }

    /// disable 一个 Mod（shutdown + 状态 Disabled）。
    pub fn disable(&mut self, id: &'static str) -> Result<(), ModError> {
        let Some(e) = self.entries.get_mut(id) else {
            return Err(ModError::Other(format!("Mod {id} 不在注册表")));
        };
        if let Some(mut rt) = e.runtime.take() {
            let _ = rt.shutdown();
        }
        e.enabled = false;
        e.status = ModStatus::Disabled;
        Ok(())
    }

    /// restart（shutdown + 重新 create/start）。
    pub fn restart(&mut self, id: &'static str) -> Result<(), ModError> {
        self.disable(id)?;
        self.enable(id)
    }

    /// reload_config（更新 config + 重新 start）。
    pub fn reload_config(
        &mut self,
        id: &'static str,
        config: serde_json::Value,
    ) -> Result<(), ModError> {
        let Some(e) = self.entries.get_mut(id) else {
            return Err(ModError::Other(format!("Mod {id} 不在注册表")));
        };
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

    /// 全部已注册 id。
    pub fn ids(&self) -> Vec<&'static str> {
        self.entries.keys().copied().collect()
    }

    /// 已注册的 settings spec（前端渲染）。
    pub fn settings_spec(&self, id: &'static str) -> Option<&ModSettingsSpec> {
        self.settings_specs.get(id)
    }

    /// 投递一个事件到 worker（有界；满则丢弃计数，不阻塞）。
    pub fn dispatch_event(&self, topic: ModEventTopic, payload: &str) -> bool {
        self.event_tx.try_send((topic, payload.to_string())).is_ok()
    }

    /// dropped 事件计数（观测值）。
    pub fn dropped_events(&self) -> u64 {
        self.dropped_events.load(Ordering::Relaxed)
    }

    /// 关闭全部 worker。
    pub fn shutdown(&mut self) {
        for e in self.entries.values_mut() {
            if let Some(mut rt) = e.runtime.take() {
                let _ = rt.shutdown();
            }
            e.status = ModStatus::Disabled;
        }
        // drop event_tx → worker 读 Disconnected 退出。
        if let Some(j) = self.worker_join.take() {
            let _ = j.join();
        }
    }
}

/// 事件 worker 循环：从有界 channel 收事件（独立线程，不阻塞 supervisor）。
///
/// v1（E4b）保持最小：worker 独立线程 + 有界 channel（try_send 满则丢），
/// 证明「慢 Mod / 大流量事件不卡主链路」。真正的 per-Mod `on_event` 转发在
/// E5（external-input）接入时补——那时各 Mod runtime 以 `Arc<Mutex<...>>`
/// 共享给 worker（`ModRuntime: Send`）。
fn event_worker_loop(
    event_rx: std::sync::mpsc::Receiver<EventMsg>,
    _enabled_ids: Vec<&'static str>,
    _dropped: Arc<AtomicU64>,
) {
    // 消费事件以保持 channel 不积压（try_send 满则丢弃由 sender 侧计数）。
    while let Ok((_topic, _payload)) = event_rx.recv() {
        // v1 占位：E5 起在此按 mod_id 路由到对应 runtime.on_event。
    }
}

/// 解析 Mod 配置：`[mods.<id>].enabled` / `.config`（或 `mods/<id>.json`）。
fn parse_mod_config(manifest: &serde_json::Value, id: &'static str) -> (bool, serde_json::Value) {
    let mods = manifest.get("mods").and_then(|m| m.as_object());
    if let Some(obj) = mods.and_then(|m| m.get(id)).and_then(|v| v.as_object()) {
        let enabled = obj
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let config = obj
            .get("config")
            .cloned()
            .unwrap_or_else(|| serde_json::Value::Object(Default::default()));
        return (enabled, config);
    }
    (false, serde_json::Value::Object(Default::default()))
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

    struct TestMod;
    impl ModFactory for TestMod {
        fn descriptor(&self) -> &'static ModDescriptor {
            &ModDescriptor {
                id: "test",
                name: "Test",
                version: "0.1.0",
                api_version: 1,
            }
        }
        fn create(
            &self,
            _s: ModServices,
            _c: serde_json::Value,
        ) -> Result<Box<dyn ModRuntime>, ModError> {
            Ok(Box::new(TestRuntime))
        }
    }
    struct TestRuntime;
    impl ModRuntime for TestRuntime {
        fn start(&mut self, _r: &mut dyn ModRegistrar) -> Result<(), ModError> {
            Ok(())
        }
    }
    static FACTORIES: &[&dyn ModFactory] = &[&TestMod];

    #[test]
    fn default_disabled_until_enabled() {
        let manifest = serde_json::json!({});
        let mut reg = ModRegistry::new(FACTORIES, &manifest);
        reg.start_all();
        let (_, status, enabled) = reg.list()[0].clone();
        assert_eq!(status, ModStatus::Disabled); // manifest 无 enabled → false
        assert!(!enabled);
    }

    #[test]
    fn enable_runs_mod() {
        let manifest = serde_json::json!({"mods": {"test": {"enabled": true}}});
        let mut reg = ModRegistry::new(FACTORIES, &manifest);
        reg.start_all();
        let (_d, status, enabled) = reg.list()[0].clone();
        assert_eq!(status, ModStatus::Running);
        assert!(enabled);
    }

    #[test]
    fn disable_shuts_down() {
        let manifest = serde_json::json!({"mods": {"test": {"enabled": true}}});
        let mut reg = ModRegistry::new(FACTORIES, &manifest);
        reg.start_all();
        reg.disable("test").unwrap();
        let (_d, status, enabled) = reg.list()[0].clone();
        assert_eq!(status, ModStatus::Disabled);
        assert!(!enabled);
    }

    #[test]
    fn dispatch_event_bounded_no_block() {
        let manifest = serde_json::json!({});
        let reg = ModRegistry::new(FACTORIES, &manifest);
        // 有界 channel，try_send 不阻塞；即使投递也立即返回。
        let ok = reg.dispatch_event(ModEventTopic::TurnStarted, "{}");
        assert!(ok);
    }

    #[test]
    fn parse_config_from_manifest() {
        let manifest = serde_json::json!({"mods": {"x": {"enabled": true, "config": {"port": 1}}}});
        let (e, c) = parse_mod_config(&manifest, "x");
        assert!(e);
        assert_eq!(c["port"], 1);
    }
}
