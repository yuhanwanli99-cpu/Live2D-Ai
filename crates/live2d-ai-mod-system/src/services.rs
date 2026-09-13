//! Host services（注入 Mod 的能力，打包而非任意 Box<dyn Fn> 堆叠）。
//!
//! E0 ADR §2：Mod 通过 `ModServices` 驱动主链路。动作由 host **固定优先级映射**
//! （不暴露自定优先级，不破坏 core 仲裁边界）。

use crate::descriptor::ModId;

/// 主动作请求 sender（host 端映射为固定 Mod 优先级）。
#[derive(Clone)]
pub struct ModActionSender {
    inner: std::sync::Arc<dyn Fn(ActionRequest) -> bool + Send + Sync>,
}

/// Mod 主动作请求（host 端把 mod_id 记入审计，动作走 core 仲裁）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionRequest {
    pub mod_id: ModId,
    /// 动作（6 基础动作为主；任意 core 支持的动作名）。
    pub action: &'static str,
    /// 强度 1..=3。
    pub strength: u8,
}

impl ModActionSender {
    /// 由 host 注入实现。
    pub fn new(f: impl Fn(ActionRequest) -> bool + Send + Sync + 'static) -> Self {
        Self {
            inner: std::sync::Arc::new(f),
        }
    }

    /// 提交一个动作请求；返回是否被 host 接受。
    pub fn request(&self, req: ActionRequest) -> bool {
        (self.inner)(req)
    }
}

/// 外部文本 sender（进 LLM/TTS 主链路）。
#[derive(Clone)]
pub struct SaySender {
    inner: std::sync::Arc<dyn Fn(String) -> bool + Send + Sync>,
}

impl SaySender {
    pub fn new(f: impl Fn(String) -> bool + Send + Sync + 'static) -> Self {
        Self {
            inner: std::sync::Arc::new(f),
        }
    }

    /// 发送一句话给主链路（走 LLM/TTS 对话）。
    pub fn say(&self, text: impl Into<String>) -> bool {
        (self.inner)(text.into())
    }
}

/// 事件投递回调类型（host → Mod 的有界 event channel）。
pub type ModEventFn = dyn Fn(crate::topics::ModEventTopic, &str) -> bool + Send + Sync;

/// 日志回调类型。
pub type LogFn = dyn Fn(log::Level, &str) + Send + Sync;

/// 事件投递 sender（host → Mod 的有界 event channel）。
#[derive(Clone)]
pub struct ModEventSender {
    inner: std::sync::Arc<ModEventFn>,
}

impl ModEventSender {
    pub fn new(
        f: impl Fn(crate::topics::ModEventTopic, &str) -> bool + Send + Sync + 'static,
    ) -> Self {
        Self {
            inner: std::sync::Arc::new(f),
        }
    }

    /// 投递一个事件（payload 为 JSON 字符串）；返回是否入队成功。
    pub fn try_emit(&self, topic: crate::topics::ModEventTopic, payload: &str) -> bool {
        (self.inner)(topic, payload)
    }
}

/// Mod logger（带 mod_id 前缀，脱敏）。
#[derive(Clone)]
pub struct ModLogger {
    inner: std::sync::Arc<LogFn>,
}

impl ModLogger {
    pub fn new(f: impl Fn(log::Level, &str) + Send + Sync + 'static) -> Self {
        Self {
            inner: std::sync::Arc::new(f),
        }
    }

    pub fn info(&self, msg: &str) {
        (self.inner)(log::Level::Info, msg);
    }
    pub fn warn(&self, msg: &str) {
        (self.inner)(log::Level::Warn, msg);
    }
    pub fn error(&self, msg: &str) {
        (self.inner)(log::Level::Error, msg);
    }
}

/// Mod → host 配置写回（**一等 API**，rc.4 M4）。
///
/// 取代旧的 `__apply_settings` 事件走私：Mod 直接拿到一个
/// `Fn(namespaced JSON patch) -> bool`，host 内部复用 settings PATCH 内核
/// （apply_patch → 原子写盘 → `supervisor.reload()`）。返回 `true` = 写盘成功
/// （或无变更），`false` = 写盘失败。
///
/// **边界**：patch 是 `live2d-ai.toml` 的 namespaced JSON 形态（如
/// `{"llm":{"base_url":"..."}}`）；Mod **不得**借此写主链语义之外的键
/// （写不存在的键由 host 的 SettingsPatch 反序列化拒绝）。
#[derive(Clone)]
pub struct ModSettingsApplier {
    inner: std::sync::Arc<dyn Fn(serde_json::Value) -> bool + Send + Sync>,
}

impl ModSettingsApplier {
    /// 由 host 注入实现。
    pub fn new(f: impl Fn(serde_json::Value) -> bool + Send + Sync + 'static) -> Self {
        Self {
            inner: std::sync::Arc::new(f),
        }
    }

    /// 应用一个 namespaced JSON patch；返回写盘 + reload 是否成功。
    pub fn apply(&self, patch: serde_json::Value) -> bool {
        (self.inner)(patch)
    }
}

/// Mod → host 配置**读取**（脱敏快照；rc.4 M5）。
///
/// 与 `apply_settings` 对称：返回当前生效的 namespaced settings JSON
/// （由 host 用脱敏视图构造——**不含任何密钥明文，也不含 `api_key_env` 变量名**）。
/// 角色卡 Mod 用它记住「主链原本的 system_prompt」，从而在禁用时还原。
#[derive(Clone)]
pub struct ModSettingsReader {
    inner: std::sync::Arc<dyn Fn() -> serde_json::Value + Send + Sync>,
}

impl ModSettingsReader {
    /// 由 host 注入实现。
    pub fn new(f: impl Fn() -> serde_json::Value + Send + Sync + 'static) -> Self {
        Self {
            inner: std::sync::Arc::new(f),
        }
    }

    /// 读取当前设置快照（namespaced JSON）。
    pub fn read(&self) -> serde_json::Value {
        (self.inner)()
    }
}

/// Host 注入 Mod 的服务集合。
#[derive(Clone)]
pub struct ModServices {
    pub action_tx: ModActionSender,
    pub say_tx: SaySender,
    pub event_tx: ModEventSender,
    pub logger: ModLogger,
    /// 一等配置写回（rc.4 M4）；未注入时为「拒绝一切 patch」的默认实现。
    pub apply_settings: ModSettingsApplier,
    /// 脱敏设置读取（rc.4 M5）；未注入时返回空对象。
    pub settings: ModSettingsReader,
    /// 真实 `live2d-ai.toml` 路径（host 注入；失败时用于精确报错，不再自行探测）。
    pub config_path: String,
}

impl ModServices {
    pub fn new(
        action_tx: ModActionSender,
        say_tx: SaySender,
        event_tx: ModEventSender,
        logger: ModLogger,
    ) -> Self {
        Self {
            action_tx,
            say_tx,
            event_tx,
            logger,
            // 默认实现拒绝写入：显式注入才开通（避免单测/无 supervisor 环境静默写盘）。
            apply_settings: ModSettingsApplier::new(|_| false),
            settings: ModSettingsReader::new(|| serde_json::json!({})),
            config_path: String::new(),
        }
    }

    /// builder：注入一等配置写回（host 的 `make_services` 用）。
    pub fn with_apply_settings(mut self, applier: ModSettingsApplier) -> Self {
        self.apply_settings = applier;
        self
    }

    /// builder：注入脱敏设置读取。
    pub fn with_settings_reader(mut self, reader: ModSettingsReader) -> Self {
        self.settings = reader;
        self
    }

    /// builder：注入配置文件路径。
    pub fn with_config_path(mut self, path: impl Into<String>) -> Self {
        self.config_path = path.into();
        self
    }
}
