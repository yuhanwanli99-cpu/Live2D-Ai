//! Live2D-Ai Mod 系统（节点 E E4）。
//!
//! # 定位
//!
//! 主仓库核心链路（TTS + LLM + Live2D + Web UI）通过本 crate 的 trait 接口
//! 暴露给 Mod。**Mod 是静态编译模块**——编译进当前 binary，enable/disable /
//! restart / reload_config 是运行时开关；**不支持运行期安装新 crate**（新增
//! Mod 需重新构建应用）。
//!
//! # 设计要点（对齐 E0 边界裁决 ADR）
//!
//! - **ModFactory + ModRuntime**（工厂/实例分离）：`factory.create(services,
//!   config)` 构造实例，`runtime.start(registrar)` 注册能力，`shutdown` 收尾。
//! - **ModRegistrar**：Mod 注册 settings schema / 订阅事件（返回可取消的
//!   [`SubscriptionId`]）。**不暴露**宽泛 `reload`、**不暴露**
//!   `register_action_source`（动作来源由 host 固定优先级映射）。
//! - **ModServices**：注入的 host 能力（say / action_tx / event_tx / logger），
//!   打包而非任意 `Box<dyn Fn>` 字段堆叠。
//! - **事件测试隔离**：host → Mod 经有界 channel + 独立 worker，慢 Mod 不卡
//!   supervisor（失败仅 disable，不影响主链路）。
//! - **SettingsSpec 是纯数据 schema**：前端统一渲染，禁 Mod 注入 HTML/JS。
//!
//! # 版本
//!
//! `api_version` 随本 crate SemVer 主版本递增；Mod 兼容性由它对齐。

pub mod descriptor;
pub mod error;
pub mod factory;
pub mod registry;
pub mod services;
pub mod settings;
pub mod status;
pub mod topics;

pub use descriptor::{ModDescriptor, ModId};
pub use error::ModError;
pub use factory::{ModAction, ModFactory, ModRuntime};
pub use registry::{ModRegistrar, SubscriptionId};
pub use services::{
    ModActionSender, ModEventSender, ModLogger, ModServices, ModSettingsApplier, SaySender,
};
pub use settings::{ModSettingField, ModSettingsSpec, SelectOption};
pub use status::ModStatus;
pub use topics::ModEventTopic;

/// Mod 系统 API 版本（对齐 [`ModDescriptor::api_version`]）。
pub const MOD_API_VERSION: u32 = 1;
