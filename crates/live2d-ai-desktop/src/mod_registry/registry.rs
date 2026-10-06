//! `ModRegistry` 的方法面（构造 / 启停 / 配置 / 只读查询 / 事件投递与回收）。
//!
//! 从 `mod_registry.rs` 拆出（该文件 1438 行，越 code-stats 的 >1000 棘轮档）。
//! 类型、常量与模块头注留在父模块 `crate::mod_registry`；本文件只放
//! `impl ModRegistry`，事件管道（sink / worker / manifest 解析）在 `super::events`。

use super::events::{HostRegistrar, event_worker_loop, parse_mod_config, secret_keys_of};
use super::*;

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

    /// 原子写回 `mods.json`（rc.4 M1）。
    ///
    /// 写入内容 = **当前全部注册 Mod** 的 `enabled` + `config`（含未启用者，
    /// 否则重新启用另一个 Mod 会把前一个的状态丢掉）。
    ///
    /// - 未注入路径（单测 / 纯内存）→ no-op；
    /// - 复用 `live2d-ai-runtime` 的 `plan_atomic_write`（tmp + fdatasync + rename，
    ///   与 `live2d-ai.toml` 同一条原子写纪律）；
    /// - **best-effort**：内存状态已变更，磁盘失败只记 `error` 日志并继续——
    ///   HTTP 不该因为磁盘只读就谎报「操作失败」（用户会看到开关没动）。
    fn persist_manifest(&self) {
        let Some(path) = self.manifest_path.as_deref() else {
            return;
        };
        // F-0013-01 / F-0644-01：以**磁盘当前内容**为基底（读不到 / 坏 JSON 时退回
        // 构造时那份原始文档），**只覆写**本进程真正拥有的 id。
        //
        // 从前的实现整份重建 {"mods": {...}}，于是用户按文档手写的「当前不存在的
        // Mod id」与其它顶层键，会在第一次写回时被静默抹掉——那是数据丢失，
        // 不是清理（docs/external-input.md 两处明确邀请用户改这个文件）。
        let mut root = std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .filter(serde_json::Value::is_object)
            .unwrap_or_else(|| self.raw_manifest.clone());
        if !root.is_object() {
            root = serde_json::Value::Object(serde_json::Map::new());
        }
        let root_obj = root.as_object_mut().expect("刚刚保证是对象");
        if !root_obj
            .get("mods")
            .is_some_and(serde_json::Value::is_object)
        {
            root_obj.insert(
                "mods".to_string(),
                serde_json::Value::Object(serde_json::Map::new()),
            );
        }
        let mods = root_obj
            .get_mut("mods")
            .and_then(|v| v.as_object_mut())
            .expect("刚刚保证是对象");
        for (id, e) in &self.entries {
            mods.insert(
                (*id).to_string(),
                serde_json::json!({ "enabled": e.enabled, "config": e.config }),
            );
        }
        let doc = root;
        let text = match serde_json::to_string_pretty(&doc) {
            Ok(t) => t,
            Err(e) => {
                tracing::error!(target: "mod", "mods.json 序列化失败: {e}");
                return;
            }
        };
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            tracing::error!(target: "mod", path = %path.display(), "mods.json 目录创建失败: {e}");
            return;
        }
        match live2d_ai_runtime::settings::patch::plan_atomic_write(path, &text) {
            Ok(tmp) => {
                if let Err(e) = std::fs::rename(&tmp, path) {
                    let _ = std::fs::remove_file(&tmp);
                    tracing::error!(target: "mod", path = %path.display(), "mods.json rename 失败: {e}");
                }
            }
            Err(e) => {
                tracing::error!(target: "mod", path = %path.display(), "mods.json 写盘失败: {e}");
            }
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
        // M3：启动时校验 API 版本；不兼容 → Failed，主链不崩（factory 不被 create）。
        if factory.descriptor().api_version != live2d_ai_mod_system::MOD_API_VERSION {
            let msg = format!(
                "Mod {id} 声明 api_version={} 与宿主 MOD_API_VERSION={} 不兼容",
                factory.descriptor().api_version,
                live2d_ai_mod_system::MOD_API_VERSION
            );
            tracing::warn!(target: "mod", "{msg}");
            if let Some(e) = self.entries.get_mut(id) {
                e.status = ModStatus::Failed { message: msg.clone() };
                e.last_error = Some(msg);
            }
            return;
        }
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
        let config_path = host.as_ref().map(|h| h.config_path.clone()).unwrap_or_default();
        // 脱敏设置读取（rc.4 M5）：无 HostChannels → 空快照。
        // 日志字段要跨进闭包：clone 一份，避免把 config_path 的所有权交出去。
        let config_path_for_log = config_path.clone();
        let reader = match host.as_ref().map(|h| h.read_settings.clone()) {
            Some(read) => ModSettingsReader::new(move || read()),
            None => ModSettingsReader::new(|| serde_json::json!({})),
        };
        // 一等配置写回（rc.4 M4）：host.apply_settings 包成 `ModSettingsApplier`。
        // 无 HostChannels（无 supervisor / 单测）→ 默认「拒绝一切 patch」，不静默写盘。
        let applier = match host.as_ref().map(|h| h.apply_settings.clone()) {
            Some(apply) => ModSettingsApplier::new(move |patch: serde_json::Value| {
                let ok = apply(patch.clone());
                tracing::info!(
                    target: "mod", mod_id, config_path = %config_path_for_log,
                    "apply_settings {}: patch={}",
                    if ok { "success" } else { "failed" }, patch
                );
                ok
            }),
            None => ModSettingsApplier::new(move |_patch| {
                tracing::info!(target: "mod", mod_id, "无 HostChannels：apply_settings 被拒绝");
                false
            }),
        };
        ModServices::new(
            // 动作：**休眠** sender（rc.2）。`ModServices.action_tx` 是 Mod API 契约，
            // 但 host 不再有驱动方——请求只留痕、被丢弃，返回 false（= 未被接受）。
            // 不要在这里接真通道：那会重建一条通往 `RootEvent::Action` 的路。
            ModActionSender::new(|req| {
                tracing::debug!(
                    target: "mod",
                    action = req.action,
                    mod_id = req.mod_id,
                    "动作子系统休眠中（rc.2 起无驱动方）：ActionRequest 被丢弃"
                );
                false
            }),
            // SaySender 保持现有 say(t)
            SaySender::new(move |t| say(t)),
            // Mod→host event：只透传日志。配置写回走一等的 `apply_settings`
            // （rc.4 M4）——旧的 `{"__apply_settings":true,...}` 事件走私已删除。
            ModEventSender::new(|topic, payload| {
                tracing::info!(target: "mod", topic = topic.as_str(), payload, "Mod→host event");
                true
            }),
            ModLogger::new(|_lvl, msg| tracing::info!(target: "mod", "{}", msg)),
        )
        .with_apply_settings(applier)
        .with_settings_reader(reader)
        .with_config_path(config_path)
        // L1 基座：会话能力。无 HostChannels（单测）→ 不可用空实现
        // （Mod 侧必须能靠 `enabled() == false` 察觉并退回全局写回）。
        .with_session_prompts(
            host.as_ref()
                .map(|h| h.session_prompts.clone())
                .unwrap_or_else(live2d_ai_mod_system::ModSessionPrompts::disabled),
        )
        // P1-3：Mod → host 动作 cue（导演按句投递；无 host 时为 disabled）。
        .with_cues(match host.as_ref().map(|h| h.cues.clone()) {
            Some(f) => live2d_ai_mod_system::ModCueSender::new(move |cue| f(cue)),
            None => live2d_ai_mod_system::ModCueSender::disabled(),
        })
    }

    #[rustfmt::skip]
    pub fn enable(&mut self, id: &'static str) -> Result<(), ModError> {
        let Some(e) = self.entries.get_mut(id) else { return Err(ModError::Other(format!("Mod {id} 不在注册表"))); };
        e.enabled = true;
        e.status = ModStatus::Starting;
        self.start_one(id);
        self.persist_manifest(); // M1：写回 mods.json，重启状态不丢。
        Ok(())
    }

    #[rustfmt::skip]
    pub fn disable(&mut self, id: &'static str) -> Result<(), ModError> {
        let Some(e) = self.entries.get_mut(id) else { return Err(ModError::Other(format!("Mod {id} 不在注册表"))); };
        if let Some(mut rt) = self.runtimes.get(id).and_then(|s| s.lock().unwrap().take()) { let _ = rt.shutdown(); }
        e.enabled = false;
        e.status = ModStatus::Disabled;
        self.persist_manifest(); // M1：写回 mods.json。
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
        self.persist_manifest(); // M1：config 也写回 mods.json。
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
    pub fn runtime_state(&self, id: &str) -> Option<serde_json::Value> {
        let slot = self.runtimes.get(id)?;
        let mut guard = slot.try_lock().ok()?;
        guard.as_mut()?.state_json()
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
