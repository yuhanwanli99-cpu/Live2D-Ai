//! rc.2：`mods.json` 的 `mods.desk-pet.config` 读取，以及「要不要开窗」的判定。
//!
//! 本模块**不引用 Mod crate**：缺省二进制工厂数是 0，没有这个运行时，也必须能编译。
//! 它只看三件事——
//! 1. 命令行是否要窗（`--desk-pet`）；
//! 2. 本进程 `AVAILABLE_MOD_FACTORIES` 里有没有 id = `desk-pet`；
//! 3. manifest 里 `mods.desk-pet.enabled` 是否为真。
//!
//! 失败语义：文件不存在 / 读失败 / 坏 JSON / 缺 id / 缺 config / 类型不对——一律
//! 当「没有这段配置」，由 `PetConfig` 逐字段回落合同缺省；**永不 Err、永不 panic**
//! （它跑在启动路径上，坏 manifest 不该拦住宿主启动）。
//!
//! 窗相关的行为（Win32 / 渲染 / 命中）一概不在这里。

use std::path::Path;

use serde_json::Value;

use super::config::PetConfig;

/// 本 Mod 的稳定 id（与 Mod 仓库 `live2d-ai-mod-desk-pet` 的 `DESCRIPTOR.id` 一致）。
pub const DESK_PET_ID: &str = "desk-pet";

/// 读 manifest 文件；任何毛病都退化成空对象（= 全缺省）。
pub fn read_manifest(path: &Path) -> Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

/// `mods.desk-pet.config` → `PetConfig`（缺键 / 坏值由 `PetConfig::from_json` 逐字段回落）。
pub fn config_from_manifest(manifest: &Value) -> PetConfig {
    manifest
        .get("mods")
        .and_then(|mods| mods.get(DESK_PET_ID))
        .and_then(|entry| entry.get("config"))
        .map(PetConfig::from_json)
        .unwrap_or_default()
}

/// 把当前宠物配置写回 `mods.desk-pet.config`，其它键保留。
/// 文件不存在或不是对象时，写成只含这一段的 JSON。写失败只忽略（菜单不能因此卡住）。
pub fn save_config(path: &Path, config: &PetConfig) {
    let mut root = read_manifest(path);
    if !root.is_object() {
        root = serde_json::json!({});
    }
    let mut mods = root.get("mods").cloned().unwrap_or_else(|| serde_json::json!({}));
    if !mods.is_object() {
        mods = serde_json::json!({});
    }
    let mut pet = mods
        .get(DESK_PET_ID)
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    if !pet.is_object() {
        pet = serde_json::json!({});
    }
    let mut cfg = pet
        .get("config")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    if !cfg.is_object() {
        cfg = serde_json::json!({});
    }
    if let Some(object) = cfg.as_object_mut() {
        object.insert("hit".to_string(), serde_json::json!(config.hit.as_str()));
        object.insert("opacity".to_string(), serde_json::json!(config.opacity));
        object.insert(
            "always_on_top".to_string(),
            serde_json::json!(config.always_on_top),
        );
        object.insert(
            "height_ratio".to_string(),
            serde_json::json!(config.height_ratio),
        );
    }
    if let Some(pet_object) = pet.as_object_mut() {
        pet_object.insert("config".to_string(), cfg);
    }
    if let Some(mods_object) = mods.as_object_mut() {
        mods_object.insert(DESK_PET_ID.to_string(), pet);
    }
    if let Some(root_object) = root.as_object_mut() {
        root_object.insert("mods".to_string(), mods);
    }
    if let Ok(text) = serde_json::to_string_pretty(&root) {
        let _ = std::fs::write(path, text);
    }
}

/// manifest 里 `mods.desk-pet.enabled` 是否为真（缺 id / 类型不对 → false）。
pub fn manifest_enabled(manifest: &Value) -> bool {
    manifest
        .get("mods")
        .and_then(|mods| mods.get(DESK_PET_ID))
        .and_then(|entry| entry.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// 开窗判定：命令行优先；否则要求「工厂里有 `desk-pet`」且「manifest enabled 为真」。
///
/// - 工厂为 0（缺省二进制）时，只有命令行能开窗；
/// - 命令行优先于 `enabled: false`（`enabled` 管的是启用，不是「命令行违不违规」）。
pub fn should_open(cli_requested: bool, factory_ids: &[&str], manifest: &Value) -> bool {
    cli_requested || (factory_ids.contains(&DESK_PET_ID) && manifest_enabled(manifest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desk_pet::config::{HEIGHT_RATIO_MIN, HitMode, OPACITY_MAX};
    use serde_json::json;

    #[test]
    fn read_manifest_falls_back_on_missing_file_and_bad_json() {
        let dir = std::env::temp_dir().join(format!("desk-pet-manifest-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let missing = dir.join("missing.json");
        assert_eq!(read_manifest(&missing), json!({}), "文件不存在 → 空对象");
        let bad = dir.join("bad.json");
        std::fs::write(&bad, "{ 这不是 JSON").expect("写坏文件");
        assert_eq!(read_manifest(&bad), json!({}), "坏 JSON → 空对象");
        assert_eq!(
            config_from_manifest(&read_manifest(&bad)),
            PetConfig::default(),
            "坏 JSON 的配置 = 合同缺省"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn config_falls_back_when_id_or_config_is_missing_or_wrong_typed() {
        for manifest in [
            json!({}),
            json!({"mods": {}}),
            json!({"mods": {"other": {"enabled": true, "config": {"hit": "through"}}}}),
            json!({"mods": {"desk-pet": {"enabled": true}}}),
            json!({"mods": {"desk-pet": {"config": null}}}),
            json!({"mods": {"desk-pet": {"config": [1, 2, 3]}}}),
            json!(null),
            json!([]),
        ] {
            assert_eq!(
                config_from_manifest(&manifest),
                PetConfig::default(),
                "{manifest}"
            );
        }
    }

    #[test]
    fn config_parses_fields_with_petconfig_fallbacks() {
        let manifest = json!({"mods": {"desk-pet": {"config": {
            "hit": "through",
            "opacity": 0.5,
            "always_on_top": false,
            "height_ratio": 0.4,
        }}}});
        let config = config_from_manifest(&manifest);
        assert_eq!(config.hit, HitMode::Through);
        assert_eq!(config.opacity, 0.5);
        assert!(!config.always_on_top);
        assert_eq!(config.height_ratio, 0.4);

        // 坏 hit（未知字符串）回落 opaque；越界 opacity/height_ratio 钳到边界，
        // 互不影响（这就是 PetConfig::from_json 的逐字段语义）。
        let bad = json!({"mods": {"desk-pet": {"config": {
            "hit": "click",
            "opacity": 9.0,
            "height_ratio": 0.01,
        }}}});
        let config = config_from_manifest(&bad);
        assert_eq!(config.hit, HitMode::Opaque);
        assert_eq!(config.opacity, OPACITY_MAX);
        assert_eq!(config.height_ratio, HEIGHT_RATIO_MIN);
    }

    #[test]
    fn cli_opens_even_with_empty_factory_and_enabled_false() {
        assert!(should_open(true, &[], &json!({})), "命令行是第一条通路");
        assert!(!should_open(false, &[], &json!({})), "两条都不满足 → 不开");
        assert!(
            should_open(
                true,
                &[],
                &json!({"mods": {"desk-pet": {"enabled": false}}})
            ),
            "命令行优先于 enabled: false"
        );
    }

    #[test]
    fn mounted_factory_requires_manifest_enabled() {
        let enabled = json!({"mods": {"desk-pet": {"enabled": true, "config": {}}}});
        let disabled = json!({"mods": {"desk-pet": {"enabled": false}}});
        let no_id = json!({"mods": {"other": {"enabled": true}}});
        let bad_bool = json!({"mods": {"desk-pet": {"enabled": "true"}}});
        assert!(should_open(false, &["desk-pet"], &enabled));
        assert!(should_open(false, &["other", "desk-pet"], &enabled));
        assert!(
            !should_open(false, &["other"], &enabled),
            "工厂里没有 → 不开"
        );
        assert!(
            !should_open(false, &[], &enabled),
            "工厂 0 时 enabled 不参与"
        );
        assert!(!should_open(false, &["desk-pet"], &disabled));
        assert!(!should_open(false, &["desk-pet"], &no_id));
        assert!(!should_open(false, &["desk-pet"], &bad_bool));
    }
}
