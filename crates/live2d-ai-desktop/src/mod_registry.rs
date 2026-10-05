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
//! # 动作：无 host 通道（2026-09-12，rc.2）
//!
//! `ModServices.action_tx` 仍在（Mod API 契约），但 **host 不再提供驱动方**：
//! 这里注入的是**固定的休眠 sender**——任何 `ActionRequest` 都只留一行 debug
//! 日志并被丢弃。理由：动作在产品路径上不存在（`core-chain-baseline.md` §3.3），
//! 而「一个只会静默吞掉请求的活通道」比没有通道更容易骗人。
//! 谁要唤醒它：先看 `docs/architecture/core-chain-baseline.md` §3.3 的三条理由，
//! 再决定是恢复 host 通道还是走步骤 2 的 Mod 能力。
//!
//! # 共享 runtime 槽位（E4b）
//!
//! `RegisteredMod.runtime` 字段已移除，改为 `ModRegistry.runtimes` 中以
//! `Arc<Mutex<Option<Box<dyn ModRuntime>>>>` 槽位共享。worker 线程持这些
//! Arc clone——**不持 registry 锁**，避免与 enable/disable/restart 竞争；
//! `try_lock` 失败（host 正在 restart）时跳过不阻塞；`on_event` 返回
//! `Err` 时清空槽位，待 host restart 恢复。

use std::collections::BTreeMap;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use live2d_ai_mod_system::*;

// ---------------------------------------------------------------- HostChannels

/// Host 真实通道（构造 registry 时注入）。
///
/// Mod → host 的真实回路只剩两条：`say` 直通 `supervisor.say`（主链路），
/// `apply_settings` 复用 settings PATCH 内核（写盘 + reload）。
///
/// **动作不在其中**：`action_tx` 是休眠 sender（见模块头注），因为动作在产品
/// 路径上不存在。**禁止**在这里重新接一条 `ActionRequest → core` 的线——
/// `supervisor` 侧的 `RootEvent::Action` 注入分支已整体删除（rc.2）。
#[derive(Clone)]
pub struct HostChannels {
    /// 文本 → supervisor.say（主链路）
    pub say: Arc<dyn Fn(String) -> bool + Send + Sync>,
    /// **配置写回（一等 API，rc.4 M4）**：Mod 需要改动 runtime settings 时走这里
    ///（现行使用者：`persona` 角色卡合成 `system_prompt` / 禁用时还原）。
    /// 参数是 namespaced JSON patch；写盘成功后 host 触发 `supervisor.reload()`。
    ///
    /// 历史：0.2.0-rc.1 前由 `local-llm` 用于「探活 → 写 base_url → 热重载」，
    /// 该 Mod 已废除启动（见其 lib.rs 头注），本通道本身仍是 Mod API 契约。
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
    /// **脱敏设置读取**（rc.4 M5）：Mod 读当前生效设置（无密钥、无变量名）。
    /// 角色卡 Mod 用它记住主链原本的 `system_prompt`，禁用时还原。
    pub read_settings: Arc<dyn Fn() -> serde_json::Value + Send + Sync>,
    /// **会话级 system_prompt 覆盖**（L1 基座，2026-09-15）。
    ///
    /// 由 host 从 supervisor 的 `SessionScopeStore` 包装而来（同一张表）。
    /// persona / memory 用它做按会话分桶，而不再整段覆写全局
    /// `persona.system_prompt`。未注入 = 不可用空实现。
    pub session_prompts: live2d_ai_mod_system::ModSessionPrompts,
    /// **Mod → host 动作 cue**（2026-09-16，P1-3）：导演产出的按句 cue → WS action_cue。
    /// 无 HostChannels（单测 / 无 supervisor）时由 ModCueSender::disabled() 顶替。
    pub cues: Arc<dyn Fn(serde_json::Value) -> bool + Send + Sync>,
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

/// `ModRegistry::command` 的失败分类（host 映射到 HTTP 状态码）。
///
/// 为什么不让 runtime 直接回 HTTP 语义：Mod 不该知道 host 的传输层
///（与 `state_json` 返回裸 JSON 同一立场）。分类发生在这里，是因为
/// 「未启用 / 正忙」只有 registry 知道，而「不认识这条命令」由 runtime 声明。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModCommandError {
    /// Mod 未启用 / runtime 槽位为空 / worker 正持锁 → **503**，可重试。
    Unavailable,
    /// Mod 不认识这条命令 → **409** `unsupported_command`。
    Unsupported(String),
    /// 命令执行失败 → **409** `command_failed`（带 runtime 的错误文案）。
    Failed(String),
}

impl std::fmt::Display for ModCommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModCommandError::Unavailable => write!(f, "Mod 未启用或正忙（可重试）"),
            ModCommandError::Unsupported(c) => write!(f, "Mod 不支持命令: {c}"),
            ModCommandError::Failed(m) => write!(f, "Mod 命令失败: {m}"),
        }
    }
}

/// 事件投递 channel 的消息。
///
/// - 前三条：话题 / payload / 会话 id（L1：`None` = 不带会话）；
/// - 第四条：**可选回执**。`Some` 时 worker 处理完这条事件后回一个信号，
///   发送方（[`ModRegistry::dispatch_event_and_flush`]）据此**等到**「这条
///   事件已被所有 Running Mod 处理完」。`None` = 非阻塞投递（默认）。
///
/// 为什么需要回执：memory / persona 在 `TurnPrompt` 里写会话注入槽，而
/// supervisor 紧接着就要读那张表去决议**本轮** system_prompt。没有回执时
/// 「Mod 写没写完」是竞态，注入就只能等下一轮（见 `dispatch_event_and_flush`）。
type EventMsg = (
    ModEventTopic,
    String,
    Option<String>,
    Option<std::sync::mpsc::SyncSender<()>>,
);

/// [`ModRegistry::dispatch_event_and_flush`] 等待 worker 回执的上限。
///
/// 取值理由：正常路径是「worker 处理一条事件」，量级在毫秒（memory 要读写一次
/// JSONL）。上限只用来兜住 **worker 已经不在了** 的情形（关机 / `on_event`
/// panic 把 worker 线程带走）——那种情况下宁可让本轮按「没有注入」继续，
/// 也不能把 supervisor 卡死。
const EVENT_FLUSH_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1000);

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
    /// `mods.json` 路径（rc.4 M1）；未注入 = 纯内存（单测）。
    manifest_path: Option<std::path::PathBuf>,
}

// 文件拆分（2026-10-05 债轮 R4-T1）：本文件只留类型、常量与模块头注；
// `impl ModRegistry` 在 `registry`，事件管道（sink / worker / manifest 解析）在
// `events`。内联 `mod tests` 拆成 `tests_support`（脚手架）+ `tests_lifecycle`
// + `tests_host` 三个场景文件，外加既有的 `tests_secret`——搬出去的每一份都必须
// <500 行，否则 >500 棘轮会变红（先例 `models_routes/tests_models_*`）。
mod events;
mod registry;

// 事件管道拆到 `events` 后，`crate::mod_registry::mod_event_sink` /
// `merge_mod_config` 的既有调用点（`supervisor` / `web_api`）路径不变。
pub use self::events::{merge_mod_config, mod_event_sink};

#[cfg(test)]
mod tests_host;
#[cfg(test)]
mod tests_lifecycle;
/// F-0062-01（2026-10-05）：保存 Mod 配置不抹 secret 的专项回归。
/// 单列文件：与生命周期 / HostChannels 场景分开，便于按缺陷追（先例 `tests_models_*`）。
#[cfg(test)]
mod tests_secret;
#[cfg(test)]
mod tests_support;
