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

/// Host 注入 Mod 的服务集合。
#[derive(Clone)]
pub struct ModServices {
    pub action_tx: ModActionSender,
    pub say_tx: SaySender,
    pub event_tx: ModEventSender,
    pub logger: ModLogger,
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
        }
    }
}
