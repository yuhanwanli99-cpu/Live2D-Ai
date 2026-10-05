//! `persona` 的静态 settings schema 与工厂单例（自 `lib.rs` 拆出）。
//!
//! schema **不含 `enabled`**：启停只由 Mod manifest 表达（见 `lib.rs` 模块头注）。
//! `persona_settings_spec` 按 `pub(super)` 开放给 `runtime`（`start` 时同样要 schema）。

use super::runtime::{PersonaConfig, PersonaRuntime};
use super::*;

/// 角色卡 Mod 的 settings schema（**静态**；factory 与 runtime.start 共用）。
///
/// **不含 `enabled`**：启停只由 Mod manifest 表达（模块头注「启停语义」）。
pub(super) fn persona_settings_spec() -> ModSettingsSpec {
    ModSettingsSpec {
        mod_id: DESCRIPTOR.id.to_string(),
        title: DESCRIPTOR.name.to_string(),
        version: 1,
        fields: vec![
            ModSettingField::String {
                key: "card_path".to_string(),
                label: "角色卡文件路径（.json / 内嵌 chara 的 .png）".to_string(),
                secret: false,
                default: None,
            },
            ModSettingField::String {
                key: "card_json".to_string(),
                label: "角色卡 JSON 文本（与路径二选一，优先）".to_string(),
                secret: false,
                default: None,
            },
            ModSettingField::Bool {
                key: "include_discipline".to_string(),
                label: "附加对话纪律模板".to_string(),
                default: true,
            },
            ModSettingField::Bool {
                key: "say_first_mes".to_string(),
                label: "启用时朗读开场白".to_string(),
                default: false,
            },
            ModSettingField::String {
                key: "name".to_string(),
                label: "覆盖：名称".to_string(),
                secret: false,
                default: None,
            },
            ModSettingField::String {
                key: "description".to_string(),
                label: "覆盖：描述".to_string(),
                secret: false,
                default: None,
            },
            ModSettingField::String {
                key: "personality".to_string(),
                label: "覆盖：性格".to_string(),
                secret: false,
                default: None,
            },
            ModSettingField::String {
                key: "scenario".to_string(),
                label: "覆盖：场景".to_string(),
                secret: false,
                default: None,
            },
        ],
    }
}

/// 静态工厂。
pub struct PersonaFactory;

impl ModFactory for PersonaFactory {
    fn descriptor(&self) -> &'static ModDescriptor {
        &DESCRIPTOR
    }

    /// M2：未启用也能拿到 schema（前端先填卡、再启用）。
    fn settings_spec(&self) -> Option<ModSettingsSpec> {
        Some(persona_settings_spec())
    }

    fn create(
        &self,
        services: ModServices,
        config: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        Ok(Box::new(PersonaRuntime::new(
            services,
            PersonaConfig::from_value(&config),
        )))
    }
}

/// 工厂单例。
pub const FACTORY: PersonaFactory = PersonaFactory;
