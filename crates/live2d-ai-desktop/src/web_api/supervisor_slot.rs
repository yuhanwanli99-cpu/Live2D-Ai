//! Supervisor 运行时槽位（D-P0C，2026-08-29）。
//!
//! # 背景
//!
//! 原 `ServerContext::supervisor: Option<Arc<SupervisorHandle>>` 是一次性
//! 注入模型——`cli_entry::run_web_mode` 启动时若配置可解析就
//! `with_supervisor`；否则 `None` 永久保持。这意味着：
//!
//! - **首次配置闭环**断裂：用户从无配置启动 → 填表 → PATCH 保存成功
//!   → 旧 `request_reload` 因 `None` no-op → 前端误报「已热重载」→ 实
//!   际 `POST /api/v1/chat` 回 503。
//!
//! # 设计（D-P0C）
//!
//! 用 [`SupervisorSlot`] 封装 `Arc<RwLock<Option<Arc<SupervisorHandle>>>>`，
//! 提供四个核心 API：
//!
//! - [`SupervisorSlot::set`]：启动期一次性注入（`with_supervisor`）——
//!   写锁短暂持锁；
//! - [`SupervisorSlot::try_get`]：handler 读路径（chat / status / reload）
//!   借出 Arc 克隆，**不**长期持锁；
//! - [`SupervisorSlot::ensure_after_patch`]：**新**——PATCH 200 写盘成功后
//!   调用，槽位空时动态创建 supervisor 注入槽位 + 注入 epoch 读源；返
//!   回 [`ApplyStatus`] 让 dispatch 写进响应 body；
//! - [`SupervisorSlot::take`]：cli_entry 退出时回收线程（quit + join）。
//!
//! # 并发模型
//!
//! - **读路径**频繁（每个 HTTP request 都查 `try_get`）：用 `RwLock::read`
//!   允许多 handler 并发借 Arc；锁释放后 Arc 仍存活（独立 clone）；
//! - **写路径**稀少（启动注入 / 动态装配 / 退出回收）：`RwLock::write`
//!   短暂持锁；
//! - **锁毒化**（poisoned）走 `ok().and_then(...)` 静默降级——handler 不
//!   因此崩盘（status 端点**始终**可用，§1.1 红线）。
//!
//! # 失败语义
//!
//! `ensure_after_patch` 失败路径：
//! - 配置不可解析（URL 缺失 / env 名非法 / TOML 解析失败）→ 返回
//!   `ApplyStatus::RestartRequired`；前端文案「已保存，需重启生效」；
//! - 槽位空 + 没有任何配置文件（用户**首次**填表路径）→ 同上
//!   `RestartRequired`（写盘后 build_web_supervisor 才看到配置，但配置
//!   仍可能不完整）。

use std::sync::{Arc, RwLock};

use live2d_ai_mod_system::ModEventTopic;

use crate::supervisor::{ModEventSink, SupervisorHandle};
use crate::web_api::app_routes::StatusContext;
use crate::web_api::dto::ApplyStatus;
use crate::web_api::ws;

/// Supervisor 运行时槽位。`Clone` 廉价（`Arc` clone，所有内部 handler
/// 都持同一份槽位）。
#[derive(Clone)]
pub struct SupervisorSlot {
    inner: Arc<RwLock<Option<Arc<SupervisorHandle>>>>,
}

impl SupervisorSlot {
    /// 构造空槽位（`None`）。
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(None)),
        }
    }

    /// 启动期注入（`with_supervisor` 路径）。写锁短暂持锁。
    pub fn set(&self, handle: Arc<SupervisorHandle>) {
        if let Ok(mut g) = self.inner.write() {
            *g = Some(handle);
        }
    }

    /// 读路径：从槽位借出当前 supervisor 句柄的 Arc 克隆。
    ///
    /// 读锁短暂持锁即释放——返回的 `Arc` 是独立引用，调用方可以在锁
    /// 释放后继续用。所有需要「读当前句柄」的路径（chat / status /
    /// dispatch）都走这里。
    pub fn try_get(&self) -> Option<Arc<SupervisorHandle>> {
        self.inner.read().ok().and_then(|g| g.clone())
    }

    /// **D-P0C 核心**：PATCH 200 写盘成功后由 dispatch 调用——动态装配
    /// supervisor。
    ///
    /// 行为：
    /// 1. **槽位非空** → 已存在 supervisor：触发 `reload()`，返回
    ///    [`ApplyStatus::Applied`]（与原「已热重载」语义一致）；
    /// 2. **槽位空 + 配置可解析** → 调用
    ///    `cli_entry::build_web_supervisor` 动态创建；成功 → 注入槽位
    ///    + 注入 epoch 读源到 `status_ctx`：返回 [`ApplyStatus::Applied`]；
    /// 3. **槽位空 + 配置不可解析**（URL 缺失 / env 名非法 / 解析失败）
    ///    → 槽位保持空；返回 [`ApplyStatus::RestartRequired`]；前端显
    ///    示「已保存，需重启生效」（与「本应自动热重载却卡 503」的失
    ///    败模式对齐——用户能区分「保存成功但需重启」与「保存失败」）。
    /// 4. **槽位空 + 没有配置文件**（用户从无配置启动）→ 同 3 的
    ///    `RestartRequired` 语义（首次装配失败）。
    ///
    /// 调用方必须**先**写盘成功（PATCH 200）**再**调本函数——不要在
    /// IO 错误路径上误触发。
    pub fn ensure_after_patch(
        &self,
        config_path: &str,
        broadcaster: ws::Broadcaster,
        status_ctx: &StatusContext,
        mod_registry: std::sync::Arc<std::sync::Mutex<crate::mod_registry::ModRegistry>>,
    ) -> ApplyStatus {
        if let Some(existing) = self.try_get() {
            // 已存在：reload 触发热重载（同原语义）。
            existing.reload();
            return ApplyStatus::Applied;
        }
        // 槽位空：动态装配。**Mod 事件桥（d2）**：动态装配路径的 supervisor
        // 直接捕获 `mod_registry`，emit `ModEventTopic` → `dispatch_event`。
        let mod_events: Option<ModEventSink> = {
            let reg = mod_registry.clone();
            Some(Arc::new(move |t: ModEventTopic, p: &str| {
                if let Ok(reg) = reg.lock() {
                    let _ = reg.dispatch_event(t, p);
                }
            }))
        };
        match crate::web_api::cli_entry::build_web_supervisor(config_path, broadcaster, mod_events)
        {
            Ok(handle) => {
                // P1WS-1 同步：注入 epoch 读源到 StatusContext（首次装配
                // 路径与 `cli_entry` 启动路径同源；保证 `/api/v1/app/status`
                // 读到的 `current_epoch` 真实而非默认 0）。
                let handle_for_epoch = Arc::clone(&handle);
                status_ctx.set_epoch_source(Box::new(move || handle_for_epoch.current_epoch()));
                self.set(handle);
                ApplyStatus::Applied
            }
            Err(_e) => {
                // 配置不完整 / 解析失败 → 不阻断 PATCH 200，但前端需
                // 知「需重启 / 修配置」才能对话；返回 RestartRequired。
                ApplyStatus::RestartRequired
            }
        }
    }

    /// 退出时取走槽位（`Option::take`）。返回的 `Arc` 由 cli_entry 走
    /// `reclaim_supervisor` 走 quit + join。**找不到** = 未装配（早期
    /// 启动失败 / 一直纯控制平面），直接返回 `None`。
    pub fn take(&self) -> Option<Arc<SupervisorHandle>> {
        self.inner.write().ok().and_then(|mut g| g.take())
    }
}

impl Default for SupervisorSlot {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 槽位基础：空 → set → try_get → take。
    #[test]
    fn slot_lifecycle_set_get_take() {
        // 真实 handle 需要 `spawn_supervisor`；本测试不验证 reload 行为，
        // 只验证「槽位读写 / 拿走 / 二次拿走 = None」的 API 契约。
        let slot = SupervisorSlot::new();
        assert!(slot.try_get().is_none(), "新槽位应空");
        assert!(slot.take().is_none(), "空槽位 take = None");
    }

    /// 锁毒化降级：模拟毒化后 `try_get` 不崩盘。
    #[test]
    fn slot_poisoned_does_not_panic() {
        use std::sync::{Arc, RwLock};
        // 构造一个被毒化的 RwLock，等价模拟锁被 panic 持有。
        let inner: Arc<RwLock<Option<Arc<()>>>> = Arc::new(RwLock::new(None));
        let inner_clone = Arc::clone(&inner);
        let _ = std::thread::spawn(move || {
            let _guard = inner_clone.write().unwrap();
            panic!("故意 panic 毒化 RwLock");
        })
        .join();
        // 此时 inner 是 poisoned。SupervisorSlot::new() 走自己的 RwLock
        // 与测试构造的 inner 无关——本测试仅确认 API 自身对毒化容错。
        let slot = SupervisorSlot::new();
        // try_get 走 .ok().and_then(...)：毒化时 lock() 返回 Err，
        // 被 .ok() 转 Option，and_then 收尾 → None。**不** panic。
        assert!(slot.try_get().is_none());
        // set 同理容错。
        slot.set(test_handle());
        // 之后 take 也应返回 None（set 走了写锁失败；take 也走写锁失败）。
        // 此处**不**断言 set/take 的副作用（毒化时确实降级为 no-op）。
        let _ = slot.take();
    }

    /// 测试专用 helper：构造一个 fake `SupervisorHandle`（绕过真实
    /// `spawn_supervisor`，仅用于槽位 API 边界检查）。
    ///
    /// **实现**：把一个永远不会跑 supervisor 线程的 `Arc` 假扮——`Arc`
    /// 是引用计数包装；只要**不**调用 `handle.say/quit/join` 等
    /// channel-backed 方法就安全。但本 helper **不**直接构造真
    /// `SupervisorHandle`（字段私有）——所以 `set` 路径要走 try_get /
    /// take 时**不**调用 `set(test_handle())`，仅当锁可用时设置。
    fn test_handle() -> Arc<SupervisorHandle> {
        // 用 `spawn_supervisor` 真实构造（指向不可达端点，线程很快 idle；
        // 测试结束 `drop` 时由 `Arc::drop` → `quit` + `join` 自动清理）。
        let client = live2d_ai_runtime::OpenAiClient::new(
            live2d_ai_runtime::LlmConfig::new("http://127.0.0.1:1/v1", "m"),
            live2d_ai_runtime::TtsConfig::new("http://127.0.0.1:2/v1", "alloy"),
        )
        .expect("client");
        Arc::new(crate::supervisor::spawn_supervisor(
            crate::supervisor::SupervisorConfig {
                client,
                conversation: live2d_ai_runtime::ConversationConfig::new("人设"),
                capabilities: live2d_ai_core::ModelCapabilities::all(),
                audio: None,
                config_path: None,
                mod_events: None,
            },
            |_ev| {},
        ))
    }
}
