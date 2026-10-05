//! Mod 事件管道：事件投递、有界 worker、host 事件 sink、`mods.json` 解析/合并。
//!
//! 从 `mod_registry.rs` 拆出（该文件 1438 行，越 code-stats 的 >1000 棘轮档）。
//! 类型与常量在父模块 `crate::mod_registry`；`ModRegistry` 的其它方法面在
//! `super::registry`。私有项按 `pub(super)` 开放给兄弟模块，不改对外 API。

use std::sync::atomic::Ordering;

use super::*;

impl ModRegistry {
    /// 投递事件到 worker（有界；满则丢弃计数，不阻塞）。
    ///
    /// `session` 是 L1 基座加上的**会话 id**（`None` = 不带会话）；worker 会把它
    /// 经 `ModRuntime::on_scoped_event` 交给 Mod。
    pub fn dispatch_event(
        &self,
        topic: ModEventTopic,
        payload: &str,
        session: Option<&str>,
    ) -> bool {
        self.send_event(topic, payload, session, None)
    }

    /// **同步投递**：入队后等 worker 处理完这条事件再返回。
    ///
    /// 只给 turn 提交点的 `TurnPrompt` 用（[`crate::mod_registry::mod_event_sink`]
    /// 是唯一调用方）：它是唯一「Mod 刚写完、本轮请求体马上就要读到」的话题。
    ///
    /// 为什么不能靠「先发事件、再读表」的调序：`dispatch_event` 是 `try_send`，
    /// worker 是另一个线程，supervisor 完全可能先跑到读表那一步——于是注入又变成
    /// 「只对下一轮生效」（这正是 2026-09-15 之前的缺陷）。这里的回执把时序钉死：
    /// 同一话题的写入在**本轮** system_prompt 决议之前一定已经落地。
    ///
    /// 返回 `false` = 队列满/已关（事件根本没入队，回执不等待）。
    pub fn dispatch_event_and_flush(
        &self,
        topic: ModEventTopic,
        payload: &str,
        session: Option<&str>,
    ) -> bool {
        let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(0);
        let queued = self.send_event(topic, payload, session, Some(ack_tx));
        if queued {
            // 有界等待：worker 已退出（关机 / panic）时不把 supervisor 卡死。
            let _ = ack_rx.recv_timeout(EVENT_FLUSH_TIMEOUT);
        }
        queued
    }

    /// 入队（`try_send`，满则丢弃计数、不阻塞）。
    fn send_event(
        &self,
        topic: ModEventTopic,
        payload: &str,
        session: Option<&str>,
        ack: Option<std::sync::mpsc::SyncSender<()>>,
    ) -> bool {
        self.event_tx
            .try_send((topic, payload.to_string(), session.map(str::to_string), ack))
            .is_ok()
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

/// 装配用：把 [`ModRegistry`] 包成 supervisor 的 Mod 事件 sink
///（`SupervisorConfig::mod_events`）。
///
/// **`TurnPrompt` 走同步投递**（入队后等 worker 回执），其余话题保持非阻塞
/// `try_send`。理由：`TurnPrompt` 是唯一「Mod 刚写完、supervisor 马上就要读」
/// 的话题（memory 的会话注入槽 / persona 的会话卡），而 `TextDelta` 之类的高频
/// 话题不该被 Mod worker 拖住。
///
/// 启动路径（`cli_entry`）与动态装配路径（`supervisor_slot`）共用这一个函数，
/// 免得两处各写一份「什么时候要 flush」的判断（它们分叉过一次，代价是
/// 注入时序在两条路径上不一致）。
pub fn mod_event_sink(registry: Arc<Mutex<ModRegistry>>) -> crate::supervisor::ModEventSink {
    Arc::new(move |t: ModEventTopic, p: &str, s: Option<&str>| {
        let Ok(reg) = registry.lock() else { return };
        if t == ModEventTopic::TurnPrompt {
            let _ = reg.dispatch_event_and_flush(t, p, s);
        } else {
            let _ = reg.dispatch_event(t, p, s);
        }
    })
}

/// 事件 worker 循环：从有界 channel 收事件，对每个 Running Mod 调
/// `runtime.on_event`（独立线程，不阻塞 supervisor；worker 持 per-Mod
/// runtime 的 `Arc<Mutex<...>>` clone，**不持 registry 锁**）。
///
/// - `try_lock` 失败（host 正在 restart 持有锁）→ 跳过本条事件。
/// - 槽位为 `None`（Mod 未启用/已停用）→ 跳过。
/// - `on_event` 返回 `Err` → 清空槽位（失败隔离），host restart 恢复。
#[rustfmt::skip]
pub(super) fn event_worker_loop(
    event_rx: std::sync::mpsc::Receiver<EventMsg>,
    runtimes: BTreeMap<&'static str, SharedRuntime>,
    _dropped: Arc<AtomicU64>,
) {
    while let Ok((topic, payload, session, ack)) = event_rx.recv() {
        for (id, slot) in &runtimes {
            let mut guard = match slot.try_lock() { Ok(g) => g, Err(_) => continue }; // host 持锁（restart）→ 跳过。
            if guard.is_none() { continue } // 未启用 → 跳过。
            // L1：走**带会话**入口；缺省实现原样转发给 on_event，
            // 所以不关心会话的 Mod（director / voice-input）行为一字不变。
            if let Err(err) = guard.as_mut().expect("non-none")
                .on_scoped_event(topic, payload.as_str(), session.as_deref())
            {
                tracing::warn!(target: "mod", "Mod {id} on_event 失败: {err}");
                *guard = None; // 失败 Mod 停止投递（E0 失败隔离）。
            }
        }
        // 回执放在**所有 Mod 都处理完之后**：`dispatch_event_and_flush` 的语义是
        // 「这条事件已经被处理完」，而不是「worker 收到了」。
        if let Some(ack) = ack {
            let _ = ack.send(());
        }
    }
}

/// 解析 Mod 配置：`[mods.<id>].enabled` / `.config`（或 `mods/<id>.json`）。
#[rustfmt::skip]
pub(super) fn parse_mod_config(manifest: &serde_json::Value, id: &'static str) -> (bool, serde_json::Value) {
    let Some(obj) = manifest.get("mods").and_then(|m| m.as_object()).and_then(|m| m.get(id)).and_then(|v| v.as_object()) else {
        return (false, serde_json::Value::Object(Default::default()));
    };
    let enabled = obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
    let config = obj.get("config").cloned().unwrap_or_else(|| serde_json::Value::Object(Default::default()));
    (enabled, config)
}

/// 一次 Mod 配置提交的**合并**规则（F-0062-01 修复，2026-10-05）。
///
/// # 规则（逐条可测）
///
/// 1. **按键合并**：提交里出现的键覆盖旧值；提交里**没有**的键保留旧值。
///    前端从脱敏配置出发只渲染 spec 字段，spec 之外的既有键同样不该被顺手抹掉。
/// 2. **secret 字段显式空串 = 清除**：`secret_keys`（= 该 Mod
///    `settings_spec` 里 `secret=true` 的字段名）中的键若提交为 `""` / 纯空白，
///    则从配置里**删除该键**（「空 = 没配」，与
///    `live2d_ai_mod_external_input::token_from_config` 对空串的判定同口径）。
///    这是**唯一**的删除语法：前端留空 = 不提交 = 保留；要清除就得显式发空串。
/// 3. 非 secret 字段的空串**照存**：它只是这个字段的值，不是删除语法。
///    ——与 `PUT /api/v1/env` 的「空值 = 清除该键」**刻意区分**：那条是密钥文件
///    `.env` 的写入口，这里改的是 Mod 配置；两套语义不通用（把 env 的
///    「空 = 清除」照搬过来，会让「清空一个普通文本框」变成删键）。
pub fn merge_mod_config(
    old: &serde_json::Value,
    submitted: &serde_json::Value,
    secret_keys: &[String],
) -> serde_json::Value {
    let mut merged = old.as_object().cloned().unwrap_or_default();
    if let Some(obj) = submitted.as_object() {
        for (key, value) in obj {
            let blank_secret = secret_keys.iter().any(|k| k == key)
                && value.as_str().is_some_and(|s| s.trim().is_empty());
            if blank_secret {
                merged.remove(key);
            } else {
                merged.insert(key.clone(), value.clone());
            }
        }
    }
    serde_json::Value::Object(merged)
}

/// 从 `settings_spec` 取出 `secret=true` 的字段名（供 [`merge_mod_config`]）。
///
/// 用**静态** schema（`ModRegistry::new` 从 factory 预填，未启用也有），
/// 所以停用的 Mod 保存配置时同样受保护。
pub(super) fn secret_keys_of(spec: Option<&ModSettingsSpec>) -> Vec<String> {
    spec.map(|spec| {
        spec.fields
            .iter()
            .filter_map(|field| match field {
                ModSettingField::String {
                    key, secret: true, ..
                } => Some(key.clone()),
                _ => None,
            })
            .collect()
    })
    .unwrap_or_default()
}

/// Host 端的 ModRegistrar 实现（借用到 registry 的共享存储）。
pub(super) struct HostRegistrar<'a> {
    pub(super) mod_id: &'static str,
    pub(super) settings_specs: &'a mut BTreeMap<&'static str, ModSettingsSpec>,
    pub(super) subscriptions: &'a mut BTreeMap<&'static str, Vec<SubscriptionId>>,
    pub(super) next_id: &'a AtomicU64,
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
