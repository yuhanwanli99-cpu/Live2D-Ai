//! `ModRegistry` 的方法面（构造 / 启停 / 配置 / 只读查询 / 事件投递与回收）。
//!
//! 从 `mod_registry.rs` 拆出（该文件 1438 行，越 code-stats 的 >1000 棘轮档）。
//! 类型、常量与模块头注留在父模块 `crate::mod_registry`；本文件只放
//! `impl ModRegistry` 的构造与只读查询面，事件管道（sink / worker / manifest
//! 解析）在 `super::events`，启停生命周期（start/enable/disable/restart）在
//! `super::lifecycle`。
//!
//! 2026-11-19（P1.5）：启停路径再拆出去——P1.5 要补「先停旧实例 / 失败清槽 /
//! 订阅随实例作废」，本文件当时已 447 行，继续堆会越 `crates/*/src` 单文件
//! 500 行硬门禁。

use super::events::{event_worker_loop, parse_mod_config, secret_keys_of};
use super::*;
// 启动原因（2026-10-09）：`local-tts` 按它决定要不要 spawn。
use live2d_ai_mod_system::{StartCause, set_start_cause};

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
        // M2：**静态** settings schema（factory 提供）——未启用的 Mod 也有表单，
        // 前端可以「先填配置、再启用」。运行时 register_settings 仍会覆盖它
        // （后者可携带动态字段）。
        let mut settings_specs = BTreeMap::new();
        for factory in factories {
            if let Some(spec) = factory.settings_spec()
                && spec.validate().is_ok()
            {
                settings_specs.insert(factory.descriptor().id, spec);
            }
        }
        // F-0644-01：文件里当前不存在的 Mod id 从前是**静默**丢弃的——用户按文档
        // 手写 mods.json 后完全不知道自己写的键被忽略了。启动时点名列一次；
        // 这些键本身在写回时会被原样保留（见 persist_manifest）。
        let known: std::collections::BTreeSet<&str> =
            factories.iter().map(|f| f.descriptor().id).collect();
        let unknown: Vec<String> = manifest
            .get("mods")
            .and_then(|m| m.as_object())
            .map(|m| {
                m.keys()
                    .filter(|k| !known.contains(k.as_str()))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        if !unknown.is_empty() {
            tracing::warn!(
                target: "mod",
                ids = ?unknown,
                "mods.json 里有 {} 个当前不存在的 Mod id：本次启动忽略它们，但写回时会原样保留（不再抹掉）",
                unknown.len()
            );
        }

        // P1.5：per-Mod 订阅路由（HostRegistrar 写、worker 读）。worker 只关心
        // topic 集合，不需要 registry 锁，也不需要订阅 id。
        let subscription_routes: SharedSubscriptions = Arc::new(Mutex::new(BTreeMap::new()));

        let dropped = Arc::clone(&dropped_events);
        let runtimes_for_worker = runtimes.clone();
        let routes_for_worker = Arc::clone(&subscription_routes);
        let worker_join = std::thread::Builder::new()
            .name("mod-event-worker".into())
            .spawn(move || {
                event_worker_loop(event_rx, runtimes_for_worker, routes_for_worker, dropped)
            })
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
            subscription_routes,
            settings_specs,
            say_source: None,
            runtimes,
            host: None,
            manifest_path: None,
            raw_manifest: manifest.clone(),
        }
    }

    /// 注入 `mods.json` 路径（builder；rc.4 M1 起 enable/disable/config 会原子写回）。
    pub fn with_manifest_path(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.manifest_path = Some(path.into());
        self
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
            // 开机启动：`on_apply` 的 Mod（local-tts）**不得**在这里 spawn。
            set_start_cause(StartCause::Boot);
            self.start_one(id);
        }
    }

    /// 保存一份 Mod 配置（`POST /api/v1/mods/{id}/config` 的内核）。
    ///
    /// **合并，不是整份替换**（F-0062-01，2026-10-05）。必要性：`GET /api/v1/mods`
    /// 对声明了 `secret=true` 的字段**脱敏**（值不进响应），前端表单因此拿不到
    /// 密钥、提交里也就没有它；旧实现 `e.config = config` 是整份替换 ⇒ 保存任意
    /// 一个普通字段都会把已存密钥抹掉，注入端点的鉴权**静默失效**，而界面回的是
    /// 成功文案。规则逐条见 [`merge_mod_config`]。
    ///
    /// [`Self::enable_with_config`] 走同一条路径，所以 `POST …/enable` 带 config
    /// 时同样不会抹密钥（同一处修复同时覆盖两个入口）。
    pub fn reload_config(
        &mut self,
        id: &'static str,
        config: serde_json::Value,
    ) -> Result<(), ModError> {
        // 先取 secret 键名（不可变借用在此结束），再取 entries 的可变借用。
        let secret_keys = secret_keys_of(self.settings_specs.get(id));
        let Some(e) = self.entries.get_mut(id) else {
            return Err(ModError::Other(format!("Mod {id} 不在注册表")));
        };
        // 合并基准是**当前条目**的配置（= 内存里的既有值），不是 manifest 原文：
        // 中途 enable/disable 过的键也一样在。
        e.config = merge_mod_config(&e.config, &config, &secret_keys);
        // M1：config 也写回 mods.json。S-003：写盘失败上抛（配置已进内存，
        // 但重启后会回退——调用方据此回 409，界面不写「已保存」）。
        self.persist_manifest()?;
        Ok(())
    }

    /// 查询指定 Mod 是否已启用（供 web API 路由判断 config 更新后是否 restart）。
    ///
    /// 收 `&str` 而不是 `&'static str`（Wave 2）：只读查询没有理由要求调用方
    /// 把 id 泄漏成 `'static`——`mods_routes` 的 GET 路径只有借来的 `&str`。
    pub fn is_enabled(&self, id: &str) -> bool {
        self.entries.get(id).map(|e| e.enabled).unwrap_or(false)
    }

    /// 该 Mod 的当前配置（rc.4 M2：给 `GET /api/v1/mods` 与 config 子路由用）。
    pub fn config(&self, id: &str) -> Option<&serde_json::Value> {
        self.entries.get(id).map(|e| &e.config)
    }

    /// 该 Mod 是否**在注册表里**（不论启用与否）。
    ///
    /// 与 [`Self::is_enabled`] 的差别：`is_enabled` 对「不存在」也回 `false`，
    /// 路由因而分不清「没这个 Mod」（404）与「有但停用」（403）。
    pub fn contains(&self, id: &str) -> bool {
        self.entries.contains_key(id)
    }

    /// **只读运行态快照**（Wave 2）：取一次 [`ModRuntime::state_json`]。
    ///
    /// 返回 `None` 的三种情况（调用方**一律**按「暂时取不到」处理，不要当错误
    /// 写进日志刷屏）：
    /// 1. `id` 不在注册表 —— 先查 [`Self::contains`] 再决定 404 / 503；
    /// 2. Mod 未启用 / 已 Failed（runtime 槽位是 `None`）；
    /// 3. Mod worker 正持着 runtime 锁（`try_lock` 失败）或本 Mod 没实现
    ///    `state_json`。
    ///
    /// 刻意用 `try_lock`：web_api 线程**绝不**为一个可选 Mod 的状态等锁——
    /// 那会让「看一眼桌宠状态」把整个 HTTP 请求循环卡在 Mod worker 上。
    ///
    /// **S-006（2026-10-16）**：返回值里**额外**带一个 `dropped_events`——宿主
    /// 丢掉 / worker 跳过的事件条数（registry 级，不是本 Mod 私有字段）。跑在
    /// 这一层而不是改各 Mod 的 `state_json`：没新增路由、没新增 WS 帧，Mod 的
    /// 快照一字未改（合并只发生在对象形态的快照上）。
    pub fn runtime_state(&self, id: &str) -> Option<serde_json::Value> {
        let slot = self.runtimes.get(id)?;
        let mut guard = slot.try_lock().ok()?;
        let mut state = guard.as_mut()?.state_json()?;
        if let Some(obj) = state.as_object_mut() {
            obj.insert(
                "dropped_events".to_string(),
                serde_json::Value::Number(self.dropped_events().into()),
            );
        }
        Some(state)
    }

    /// **一次性命令**（产品级加强波次）：把用户按下按钮的动作转达给 Mod runtime。
    ///
    /// 与 `runtime_state` 共用同一把锁和同一条「不阻塞 HTTP」纪律：
    /// - `id` 不在注册表 → 调用方**先**用 `contains` 判 404，本函数不区分；
    /// - Mod 未启用 / worker 正持锁 → 返回 `ModCommandError::Unavailable`（503，可重试）；
    /// - runtime 返回 `ModError::UnsupportedCommand` → `Unsupported`（409）；
    /// - 其它 `Err` → `Failed`（409，带 runtime 的错误文案）。
    pub fn command(
        &self,
        id: &str,
        command: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, ModCommandError> {
        let Some(slot) = self.runtimes.get(id) else {
            return Err(ModCommandError::Unavailable);
        };
        let Ok(mut guard) = slot.try_lock() else {
            return Err(ModCommandError::Unavailable);
        };
        let Some(rt) = guard.as_mut() else {
            return Err(ModCommandError::Unavailable);
        };
        match rt.command(command, args) {
            Ok(value) => Ok(value),
            Err(ModError::UnsupportedCommand { .. }) => {
                Err(ModCommandError::Unsupported(command.to_string()))
            }
            Err(e) => Err(ModCommandError::Failed(e.to_string())),
        }
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

    /// 该 Mod 注册的设置 schema（rc.4 M2：`GET /api/v1/mods` 带出）。
    pub fn settings_spec(&self, id: &str) -> Option<&ModSettingsSpec> {
        self.settings_specs.get(id)
    }
}
