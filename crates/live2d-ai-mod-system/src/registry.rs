//! Mod 注册器（Mod 通过它注册能力；host 实现）。

use crate::error::ModError;
use crate::settings::ModSettingsSpec;
use crate::topics::ModEventTopic;

/// 订阅 id（Mod 可取消订阅）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(pub u64);

/// Mod 注册器（`start` 期间可用；host 实现存储）。
///
/// **限制**（E0 ADR）：
/// - 不暴露宽泛 `reload`（Mod 只能返回 `ModAction::RequestRestart`）。
/// - 不暴露 `register_action_source`（动作来源由 host 固定优先级映射）。
pub trait ModRegistrar {
    /// 注册设置 schema（前端统一渲染，禁 Mod 注入 HTML/JS）。
    fn register_settings(&mut self, spec: ModSettingsSpec) -> Result<(), ModError>;

    /// 订阅一个事件主题（返回可取消 id）。
    fn subscribe(&mut self, topic: ModEventTopic) -> Result<SubscriptionId, ModError>;

    /// 取消订阅。
    fn unsubscribe(&mut self, id: SubscriptionId) -> Result<(), ModError>;
}
