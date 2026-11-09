//! 0.5.1 rc.1：import 的 `model3_rel_path` 必须是**相对模型根**（Windows
//! verbatim 路径回归）、官方 PascalCase Layout 解析、无 Layout 默认取景、
//! 以及 registry 载入期对历史绝对 / verbatim 路径的修复。

use tiny_http::Method;

use super::registry::{
    ModelDisplay, ModelEntry, ModelRegistry, ModelSource, StoredLayout, StoredManifest,
    default_stage,
};
use super::tests_models_common::{
    body_to_string, cleanup, make_minimal_package, minimal_model3_json, tmp_dir, tmp_path,
    write_package_at,
};
use super::util::model3_rel_path_from_abs;
use super::{ModelStore, dispatch};

/// 纯函数回归：Windows 真机实测的 verbatim / UNC 前缀必须能剥成相对路径。
#[test]
fn model3_rel_path_from_verbatim_strings_is_relative() {
    assert_eq!(
        model3_rel_path_from_abs(
            std::path::Path::new(r"\\?\D:\live2d-ai\assets\models"),
            std::path::Path::new(r"\\?\D:\live2d-ai\assets\models\skin\skin.model3.json"),
        )
        .expect("verbatim 前缀必须能剥"),
        "skin/skin.model3.json"
    );
    // 真实 Windows 主路径：模型根是普通盘符，model3 是 canonicalize 后的 verbatim。
    assert_eq!(
        model3_rel_path_from_abs(
            std::path::Path::new(r"D:\live2d-ai\assets\models"),
            std::path::Path::new(r"\\?\D:\live2d-ai\assets\models\skin\runtime\skin.model3.json"),
        )
        .expect("普通根 + verbatim 文件"),
        "skin/runtime/skin.model3.json"
    );
}

/// import 写进 registry 的必须是相对路径（旧 bug：verbatim 绝对路径 → URL 404）。
#[test]
fn import_writes_a_relative_model3_path() {
    let (assets_root, _model3) = make_minimal_package("relimport");
    let reg = tmp_path("relimport");
    cleanup(&reg);
    let store = ModelStore::new(assets_root.clone(), reg.clone());
    let resp = dispatch(
        &store,
        &Method::Post,
        "/api/v1/models/import",
        r#"{"id":"relimport"}"#,
    );
    assert_eq!(resp.status_code().0, 200, "{}", body_to_string(resp));
    let loaded = ModelRegistry::load_from_path(&reg).expect("registry on disk");
    let entry = loaded.entries.get("relimport").expect("entry");
    assert_eq!(entry.model3_rel_path, "relimport/bai.model3.json");
    assert!(
        !entry.model3_rel_path.starts_with('/'),
        "{}",
        entry.model3_rel_path
    );
    assert!(
        !entry.model3_rel_path.contains(":/"),
        "不得写盘符绝对路径: {}",
        entry.model3_rel_path
    );
    assert!(
        !entry.model3_rel_path.contains("//?/"),
        "不得写 verbatim 前缀: {}",
        entry.model3_rel_path
    );
    cleanup(&reg);
    cleanup(&assets_root);
}

/// Bug B 端到端：官方 PascalCase `Layout` 经 import → registry 存的是真实值
///（旧实现整块忽略 → 1.0×1.0 撑满裁切）。
#[test]
fn import_parses_official_pascal_case_layout() {
    let assets_root = tmp_dir("layout_pascal");
    cleanup(&assets_root);
    write_package_at(
        &assets_root.join("pascal"),
        &minimal_model3_json(Some(
            r#"{"CenterX":0.1,"CenterY":-0.07,"Width":0.56,"Height":0.56}"#,
        )),
    );
    let reg = tmp_path("layout_pascal");
    cleanup(&reg);
    let store = ModelStore::new(assets_root.clone(), reg.clone());
    let resp = dispatch(
        &store,
        &Method::Post,
        "/api/v1/models/import",
        r#"{"id":"pascal"}"#,
    );
    assert_eq!(resp.status_code().0, 200, "{}", body_to_string(resp));
    let loaded = ModelRegistry::load_from_path(&reg).expect("registry");
    let layout = loaded.entries["pascal"].manifest.layout;
    assert_eq!(layout.center_x, 0.1);
    assert_eq!(layout.center_y, -0.07);
    assert_eq!(layout.width, 0.56);
    assert_eq!(layout.height, 0.56);
    cleanup(&reg);
    cleanup(&assets_root);
}

/// Bug C 端到端：无 `Layout` 的模型 import 后拿到产品默认取景（与 bai 同口径）。
#[test]
fn import_without_layout_uses_product_framing() {
    let assets_root = tmp_dir("layout_missing");
    cleanup(&assets_root);
    write_package_at(&assets_root.join("nolayout"), &minimal_model3_json(None));
    let reg = tmp_path("layout_missing");
    cleanup(&reg);
    let store = ModelStore::new(assets_root.clone(), reg.clone());
    let resp = dispatch(
        &store,
        &Method::Post,
        "/api/v1/models/import",
        r#"{"id":"nolayout"}"#,
    );
    assert_eq!(resp.status_code().0, 200, "{}", body_to_string(resp));
    let loaded = ModelRegistry::load_from_path(&reg).expect("registry");
    let layout = loaded.entries["nolayout"].manifest.layout;
    assert_eq!(layout.center_x, 0.0);
    assert_eq!(layout.center_y, -0.07, "竖长画布不得再被 1.0 撑满裁头");
    assert_eq!(layout.width, 0.56);
    assert_eq!(layout.height, 0.56);
    cleanup(&reg);
    cleanup(&assets_root);
}

/// 旧 registry 里的 verbatim 绝对路径在载入时被修复成相对路径（只改内存，不写盘）。
#[test]
fn load_repairs_legacy_verbatim_model3_path() {
    let root = tmp_dir("legacy_repair");
    cleanup(&root);
    write_package_at(&root.join("skin"), &minimal_model3_json(None));
    let reg = tmp_path("legacy_repair");
    cleanup(&reg);
    let mut registry = ModelRegistry {
        schema_version: 1,
        active_id: None,
        entries: std::collections::BTreeMap::new(),
    };
    registry.entries.insert(
        "skin".to_string(),
        legacy_entry("skin", "skin", "//?/D:/old/abs/skin.model3.json"),
    );
    std::fs::write(&reg, serde_json::to_vec_pretty(&registry).expect("ser"))
        .expect("write registry");
    let before = std::fs::read_to_string(&reg).expect("read registry");
    let store = ModelStore::from_disk(root.clone(), reg.clone());
    let fixed = store
        .inner
        .read()
        .unwrap()
        .entries
        .get("skin")
        .expect("entry")
        .model3_rel_path
        .clone();
    assert_eq!(fixed, "skin/bai.model3.json");
    assert!(root.join(&fixed).is_file());
    let after = std::fs::read_to_string(&reg).expect("read registry");
    assert_eq!(before, after, "载入修复只改内存，不在启动路径写盘");
    cleanup(&root);
    cleanup(&reg);
}

#[test]
fn load_keeps_valid_relative_model3_path_untouched() {
    let root = tmp_dir("legacy_ok");
    cleanup(&root);
    write_package_at(&root.join("skin"), &minimal_model3_json(None));
    let reg = tmp_path("legacy_ok");
    cleanup(&reg);
    let mut registry = ModelRegistry {
        schema_version: 1,
        active_id: None,
        entries: std::collections::BTreeMap::new(),
    };
    registry.entries.insert(
        "skin".into(),
        legacy_entry("skin", "skin", "skin/bai.model3.json"),
    );
    std::fs::write(&reg, serde_json::to_vec_pretty(&registry).expect("ser"))
        .expect("write registry");
    let store = ModelStore::from_disk(root.clone(), reg.clone());
    assert_eq!(
        store.inner.read().unwrap().entries["skin"].model3_rel_path,
        "skin/bai.model3.json"
    );
    cleanup(&root);
    cleanup(&reg);
}

/// 造一条 0.5.1 之前的 registry 记录（builtin 字段由 serde(default) 补 false）。
pub(super) fn legacy_entry(id: &str, relative_dir: &str, model3_rel_path: &str) -> ModelEntry {
    ModelEntry {
        id: id.to_string(),
        display_name: id.to_string(),
        source: ModelSource::LocalImport,
        relative_dir: relative_dir.to_string(),
        model3_rel_path: model3_rel_path.to_string(),
        manifest: StoredManifest {
            version: 3,
            moc3_file: "bai.moc3".to_string(),
            texture_files: vec!["tex_00.png".to_string()],
            physics_file: Some("physics3.json".to_string()),
            display_info_file: None,
            layout: StoredLayout {
                center_x: 0.0,
                center_y: -0.07,
                width: 0.56,
                height: 0.56,
            },
        },
        display: ModelDisplay::default(),
        stage: default_stage(),
        imported_at: "2026-11-09T00:00:00.000Z".to_string(),
        builtin: false,
    }
}
