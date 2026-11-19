//! `ModRegistry` 的启停生命周期内核（P1.5，0.7.0 收口）。
//!
//! 从 `registry.rs` 拆出：该文件已 447 行，而 P1.5 要在启停路径上补
//! 「先停旧实例 / 失败清槽位 / 状态与真实行为一致 / 停用即 teardown」，
//! 继续堆会越 `crates/*/src` 单文件 500 行硬门禁。
//!
//! # P1.5 取舍（逐条对应验收项）
//!
//! 1. **enable 幂等**：`status == Running` 且槽位里确实有 runtime → no-op
//!    （不重启、不重复 `create`）。要「保存并应用」请走 `restart`：它 =
//!    `disable`（停掉旧实例）+ 以 `Apply` 启动新实例。选 no-op 而不是
//!    「先停再起」：用户重复点开关不该有副作用（旧实现会 create 第二个实例
//!    覆盖槽位，旧实例只被 drop、不跑 `shutdown` ⇒ Mod 的子进程泄漏）。
//! 2. **启动前先停旧实例**：`start_one` 先 `stop_old_runtime`；`shutdown`
//!    失败就如实上抛（不假装停掉，也不硬起第二个）。
//! 3. **失败与真实行为一致**：create / start 失败 ⇒ 槽位显式清成 `None`、
//!    状态 `Failed`、`last_error` 写明原因；成功 ⇒ `Running` + 清 `last_error`。
//! 4. **停用即 teardown**：`disable` 调 trait 的 `shutdown()`（失败把 runtime
//!    放回槽位），成功后 runtime 在本函数返回时 drop；该 Mod 的订阅一并清掉，
//!    worker 之后不会再路由事件给它（槽位 `None` 是第二道闸）。
//!
//! 消息面语义不变：`enable` / `disable` 的 `Err` 仍只表示「没做成」（写盘失败 /
//! 停不掉）；`start` 失败只落状态（`Failed`）而不把 `enable` 变 `Err`——主链
//! 不因为一个 Mod 起不来而崩（M3 的既有口径）。

use super::events::HostRegistrar;
use super::*;
use live2d_ai_mod_system::{StartCause, set_start_cause};

impl ModRegistry {
    /// 启动一个 Mod（`start_all` / `enable` / `restart` 的内核）。
    ///
    /// 顺序是刻意的：**先停旧实例、再校验 API、最后 create + start**。
    /// create / start 失败时显式清空槽位（`None`）并置 `Failed`，保证「状态」
    /// 与「槽位里的真实行为」一致。
    #[rustfmt::skip]
    pub(super) fn start_one(&mut self, id: &'static str) {
        let Some(&factory) = self.factories.iter().find(|f| f.descriptor().id == id) else { return };

        // P1.5（2）：启动前先把槽位里的旧实例**停掉再清空**。直接覆盖槽位只是
        // drop，Mod 的 `shutdown` 不会跑（子进程泄漏）；上一次失败也可能把状态
        // 写成 Failed、旧 runtime 却还躺在槽位里。
        if let Err(err) = self.stop_old_runtime(id) {
            let msg = format!("Mod {id} 启动前停掉旧实例失败：{err}");
            tracing::warn!(target: "mod", "{msg}");
            self.mark_failed(id, msg);
            return;
        }
        // 旧订阅随旧实例一起作废；新实例的 `start` 会重新订阅。
        self.clear_subscriptions(id);

        // M3：启动时校验 API 版本；不兼容 → Failed，主链不崩（factory 不被 create）。
        if factory.descriptor().api_version != live2d_ai_mod_system::MOD_API_VERSION {
            let msg = format!(
                "Mod {id} 声明 api_version={} 与宿主 MOD_API_VERSION={} 不兼容",
                factory.descriptor().api_version,
                live2d_ai_mod_system::MOD_API_VERSION
            );
            tracing::warn!(target: "mod", "{msg}");
            self.mark_failed(id, msg);
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
                    subscriptions: &mut self.subscriptions,
                    routes: &self.subscription_routes,
                    next_id: &self.next_sub_id };
                match runtime.start(&mut registrar) {
                    Ok(()) => {
                        // W1（2026-10-16）：槽位锁毒化不 panic（旧 `.unwrap()` 会让
                        // 此后每一次 enable/disable 都 panic，用户再也停不掉那个 Mod）。
                        if let Some(slot) = self.runtimes.get(id) {
                            *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(runtime);
                        }
                        if let Some(e) = self.entries.get_mut(id) {
                            e.status = ModStatus::Running;
                            e.last_error = None;
                        }
                    }
                    Err(err) => {
                        // P1.5（3）：失败 ⇒ 槽位显式 None + Failed + last_error 写清。
                        self.clear_runtime_slot(id);
                        self.clear_subscriptions(id);
                        // 实例没启动成功，不调 shutdown；这里显式 drop 收尾。
                        drop(runtime);
                        self.mark_failed(id, format!("Mod {id} 启动失败：{err}"));
                    }
                }
            }
            Err(err) => {
                // P1.5（3）：create 失败同样清槽位（旧实现只改状态，旧 runtime 可能
                // 还在槽位里 → 状态 Failed 与「其实还跑着」互相矛盾）。
                self.clear_runtime_slot(id);
                self.clear_subscriptions(id);
                self.mark_failed(id, format!("Mod {id} 创建失败：{err}"));
            }
        }
    }

    /// 槽位里是否**真的**躺着 runtime。
    ///
    /// 用 `try_lock` 而不是 `lock`：worker 正在 `on_event` 时持有该锁，而 enable
    /// 跑在 web_api 线程上——「锁被占 = runtime 在 worker 手里」是安全结论，不该
    /// 为一个幂等判断等慢 Mod 处理完一条事件。
    #[rustfmt::skip]
    fn runtime_present(&self, id: &str) -> bool {
        let Some(slot) = self.runtimes.get(id) else { return false };
        match slot.try_lock() {
            Ok(guard) => guard.is_some(),
            Err(std::sync::TryLockError::WouldBlock) => true,
            Err(std::sync::TryLockError::Poisoned(poison)) => poison.into_inner().is_some(),
        }
    }

    /// 停掉并清空槽位里的旧 runtime（启动前清理；与 `disable` 同一条纪律：
    /// 停不掉就把句柄放回槽位，绝不假装停掉）。
    #[rustfmt::skip]
    fn stop_old_runtime(&self, id: &'static str) -> Result<(), ModError> {
        // 先只在锁内「取出」，shutdown 放到锁外跑（慢 Mod 收尾不该挡 worker）。
        let rt = {
            let Some(slot) = self.runtimes.get(id) else { return Ok(()) };
            let mut guard = slot.lock().unwrap_or_else(|e| e.into_inner());
            guard.take()
        };
        let Some(mut rt) = rt else { return Ok(()) };
        match rt.shutdown() {
            // shutdown 成功后 `rt` 在本函数返回时 drop（资源收尾的第二道）。
            Ok(()) => Ok(()),
            // 停不掉 ⇒ 放回槽位，别假装停了。
            Err(err) => { self.put_runtime(id, rt); Err(err) }
        }
    }

    /// 把槽位明确清成 `None`（失败路径；实例没启动成功，不调 shutdown）。
    fn clear_runtime_slot(&self, id: &str) {
        if let Some(slot) = self.runtimes.get(id) {
            *slot.lock().unwrap_or_else(|e| e.into_inner()) = None;
        }
    }

    /// 取出槽位里的 runtime（锁毒化不 panic，W1）。
    #[rustfmt::skip]
    fn take_runtime(&self, id: &str) -> Option<Box<dyn ModRuntime>> {
        self.runtimes.get(id)
            .and_then(|slot| slot.lock().unwrap_or_else(|e| e.into_inner()).take())
    }

    /// 把没停掉的 runtime 放回槽位（`disable` 失败路径）。
    fn put_runtime(&self, id: &str, rt: Box<dyn ModRuntime>) {
        if let Some(slot) = self.runtimes.get(id) {
            *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(rt);
        }
    }

    /// 清掉该 Mod 的全部订阅（registry 私有 id 表 + worker 读的路由表）。
    ///
    /// 停用 / 重启 / 启动失败都要清：否则「状态 Failed、没有 runtime」的 Mod 在
    /// 路由表里仍留着 topic，看起来像还在收事件。
    #[rustfmt::skip]
    fn clear_subscriptions(&mut self, id: &'static str) {
        let _ = self.subscriptions.remove(id);
        let _ = self.subscription_routes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id);
    }

    /// 失败统一落账：状态 `Failed` + `last_error` 同一份文案。
    fn mark_failed(&mut self, id: &'static str, message: String) {
        if let Some(e) = self.entries.get_mut(id) {
            e.status = ModStatus::Failed {
                message: message.clone(),
            };
            e.last_error = Some(message);
        }
    }

    /// 用户按下启用开关 → 启动原因恒为 [`StartCause::Enable`]。
    #[rustfmt::skip]
    pub fn enable(&mut self, id: &'static str) -> Result<(), ModError> {
        self.enable_caused(id, StartCause::Enable)
    }

    /// `enable` 的内核：`cause` 由调用方给定。
    ///
    /// 分开的理由只有一条：`restart` 成功停掉旧实例之后要以 `Apply` 启动，
    /// 而**公开** `enable` 必须始终是 `Enable`（`enable_with_config` 也走它）。
    ///
    /// P1.5 幂等：已 `Running` 且槽位里确实有 runtime → no-op。要热应用新配置
    /// 请走 [`Self::restart`]（HTTP `POST /api/v1/mods/{id}/restart`）。
    #[rustfmt::skip]
    fn enable_caused(&mut self, id: &'static str, cause: StartCause) -> Result<(), ModError> {
        if !self.contains(id) {
            return Err(ModError::Other(format!("Mod {id} 不在注册表")));
        }
        // 用 runtime_present 而不是只信状态：worker 在 on_event 失败时会清空
        // 槽位但碰不到 entries（它不持 registry 锁），这种「Running 但没实例」
        // 的状态必须能被下一次 enable 自愈。反过来「有实例」也可能顶着 Failed
        // （上一次写盘失败）——所以两个条件都要看。
        if self.entries.get(id).is_some_and(|e| e.status == ModStatus::Running)
            && self.runtime_present(id)
        {
            return Ok(());
        }
        if let Some(e) = self.entries.get_mut(id) {
            e.enabled = true;
            e.status = ModStatus::Starting;
        }
        set_start_cause(cause);
        self.start_one(id);
        // M1：写回 mods.json，重启状态不丢。**S-003**：写盘失败不许吞——
        // 内存里已经启用了（这一进程是真的），但持久化没成，必须让调用方拿到
        // Err（HTTP 409 / 后台 op 把状态标 Failed，界面不写「已启用」）。
        if let Err(err) = self.persist_manifest() {
            let msg = format!("Mod {id} 已启用，但 {err}");
            self.mark_failed(id, msg.clone());
            return Err(ModError::Other(msg));
        }
        Ok(())
    }

    /// 停用：真正 teardown（`shutdown` + drop），失败时**不吞**。
    ///
    /// `shutdown` 失败 → 把 runtime 放回槽位并返回 `Err`；失败路径什么都不改：
    /// `enabled` 仍为 true、不写 `mods.json`、runtime 还在。
    /// （旧实现是 `let _ = rt.shutdown()`：失败后 runtime 被丢掉，子进程成了没人
    /// `wait` 的孤儿，而 `enabled` 照样被写成 `false`——下次启用会**再拉一个**。）
    ///
    /// P1.5：成功路径在 shutdown 之后 drop runtime，并清掉该 Mod 的订阅；
    /// worker 从此不再给它投递事件。
    #[rustfmt::skip]
    pub fn disable(&mut self, id: &'static str) -> Result<(), ModError> {
        if !self.contains(id) {
            return Err(ModError::Other(format!("Mod {id} 不在注册表")));
        }
        // take_runtime 在返回前就释放槽位锁；shutdown 不在锁内跑（慢 Mod 收尾
        // 不该挡 worker——worker 此时看到槽位 None，直接跳过）。
        if let Some(mut rt) = self.take_runtime(id)
            && let Err(err) = rt.shutdown()
        {
            self.put_runtime(id, rt);
            return Err(err);
        }
        if let Some(e) = self.entries.get_mut(id) {
            e.enabled = false;
            e.status = ModStatus::Disabled;
        }
        // P1.5：订阅随实例一起作废（槽位 None 是第二道闸）。
        self.clear_subscriptions(id);
        // S-003：同上——停用是**真的**（进程里已经停了），但写盘失败要让调用方
        // 拿到 Err，界面不写「已停用」，也不会以为「重启后还是停用」。
        if let Err(err) = self.persist_manifest() {
            let msg = format!("Mod {id} 已停用，但 {err}");
            self.mark_failed(id, msg.clone());
            return Err(ModError::Other(msg));
        }
        Ok(())
    }

    /// 保存并应用：先停，**成功停掉之后**再以 `Apply` 启动。
    ///
    /// `disable` 失败（停不掉）时直接返回 `Err` ⇒ 这里不会走到 `start`：
    /// 「停不掉还硬起一个」正是孤儿进程与双份进程的来源。
    /// P1.5：`disable` 已负责真正 teardown（shutdown + drop + 清订阅），
    /// 所以这里就是「stop + start」。
    pub fn restart(&mut self, id: &'static str) -> Result<(), ModError> {
        self.disable(id)?;
        self.enable_caused(id, StartCause::Apply)
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
}
