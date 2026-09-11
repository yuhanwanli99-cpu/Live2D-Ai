//! Mod 工厂与运行时（E0 ADR §1：工厂/实例分离，`start` 返回 `Result`）。

use crate::descriptor::ModDescriptor;
use crate::error::ModError;
use crate::registry::ModRegistrar;
use crate::services::ModServices;

/// Mod 运行时请求返回给 host 的动作（host 决定是否执行）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModAction {
    /// 无。
    None,
    /// 请求重启本 Mod 实例（host 决定）。
    RequestRestart,
}

/// Mod 工厂（静态注册进 host 的 `AVAILABLE_MOD_FACTORIES`）。
pub trait ModFactory: Send + Sync {
    /// 静态描述符。
    fn descriptor(&self) -> &'static ModDescriptor;

    /// 构造一个 Mod 实例。
    ///
    /// `config` = 该 Mod 的 namespaced JSON 配置（`~/.config/live2d-ai/mods/<id>.json`
    /// 或主配置 `[mods.<id>]` 段解析结果）。
    fn create(
        &self,
        services: ModServices,
        config: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError>;
}

/// Mod 运行时实例（host 持有，`start` 注册能力，`shutdown` 收尾）。
pub trait ModRuntime: Send {
    /// 注册能力（settings schema / 事件订阅）。
    ///
    /// 返回 `Err` 表示注册失败 → host 置 `ModStatus::Failed` 并 disable。
    fn start(&mut self, registrar: &mut dyn ModRegistrar) -> Result<(), ModError>;

    /// 处理一次事件（由 host 的独立 worker 调用）。
    ///
    /// 返回 `Err` → host 记录 last_error 并停止投递（`ModStatus::Failed`）。
    fn on_event(
        &mut self,
        topic: crate::topics::ModEventTopic,
        payload: &str,
    ) -> Result<(), ModError> {
        let _ = (topic, payload);
        Ok(())
    }

    /// 收尾（release 资源）。
    fn shutdown(&mut self) -> Result<(), ModError> {
        Ok(())
    }
}
