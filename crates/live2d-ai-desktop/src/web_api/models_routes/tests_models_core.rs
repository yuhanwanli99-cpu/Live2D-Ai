//! `models_routes` 核心逻辑测试（自 `tests_models_routes.rs` 拆出）。
//!
//! 场景：① 路径安全 `normalize_relative_id`；② registry 原子写回 round-trip；
//! ③ `resolve_safe_path`；④ 路由表 `match_model_route`；⑤ DTO schema；
//! ⑥ 单一模型根 / `StoredManifest` 跨层不变量。

use tiny_http::Method;

use l2d::asset::PackageManifest;

use super::dto::{DisplayModelPatch, DisplayStagePatch};
use super::registry::{
    BackgroundType, FitMode, ModelDisplay, ModelRegistry, StageConfig, StoredManifest,
    atomic_write_json, normalize_relative_id,
};
use super::tests_models_common::{
    cleanup, fresh_store, make_minimal_package, sample_registry, tmp_path,
};
use super::{ModelRouteId, handlers, match_model_route};

// ============================================================================
// 1. 路径安全：normalize_relative_id
// ============================================================================

#[test]
fn normalize_relative_id_accepts_snake_case() {
    assert_eq!(normalize_relative_id("bai").unwrap(), "bai");
    assert_eq!(normalize_relative_id("bai_001").unwrap(), "bai_001");
    assert_eq!(normalize_relative_id("BaiV2").unwrap(), "BaiV2");
    assert_eq!(normalize_relative_id("with-dash").unwrap(), "with-dash");
    assert_eq!(normalize_relative_id("abc123").unwrap(), "abc123");
}

#[test]
fn normalize_relative_id_rejects_empty_and_dot() {
    assert!(normalize_relative_id("").is_err());
    assert!(normalize_relative_id(".").is_err());
    assert!(normalize_relative_id(".hidden").is_err());
}

#[test]
fn normalize_relative_id_rejects_parent_escape() {
    assert!(normalize_relative_id("..").is_err());
    assert!(normalize_relative_id("../etc").is_err());
    assert!(normalize_relative_id("a..b").is_err());
    assert!(normalize_relative_id("..a").is_err());
    assert!(normalize_relative_id("a/../b").is_err());
}

#[test]
fn normalize_relative_id_rejects_path_separators() {
    assert!(normalize_relative_id("a/b").is_err());
    assert!(normalize_relative_id("a\\b").is_err());
    assert!(normalize_relative_id("/abs").is_err());
}

#[test]
fn normalize_relative_id_rejects_special_chars() {
    assert!(normalize_relative_id("a b").is_err());
    assert!(normalize_relative_id("a$b").is_err());
    assert!(normalize_relative_id("a;b").is_err());
    assert!(normalize_relative_id("a\nb").is_err());
    assert!(normalize_relative_id("a\0b").is_err());
    assert!(normalize_relative_id("中文").is_err());
}

// ============================================================================
// 2. registry 原子写回 + round-trip
// ============================================================================

#[test]
fn registry_atomic_write_round_trip() {
    let path = tmp_path("round_trip");
    cleanup(&path);
    let original = sample_registry();
    atomic_write_json(&path, &original).expect("write");
    let loaded = ModelRegistry::load_from_path(&path).expect("load");
    assert_eq!(loaded.schema_version, 1);
    assert_eq!(loaded.active_id.as_deref(), Some("bai"));
    let e = loaded.entries.get("bai").expect("entry");
    assert_eq!(e.id, "bai");
    assert_eq!(e.manifest.version, 3);
    assert_eq!(e.manifest.moc3_file, "bai.moc3");
    assert_eq!(e.manifest.texture_files, vec!["tex_00.png".to_string()]);
    assert_eq!(e.manifest.layout.width, 1.0);
    assert_eq!(e.display.scale, 1.0);
    assert_eq!(e.stage.width, 360);
    assert_eq!(e.stage.background_type, BackgroundType::Transparent);
    cleanup(&path);
}

#[test]
fn registry_load_missing_file_returns_default() {
    // 缺失 = Ok（ModelRegistry::default()）；不视作错误。
    let path = tmp_path("missing");
    cleanup(&path);
    let result = ModelRegistry::load_from_path(&path);
    assert!(matches!(
        result,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound
    ));
}

#[test]
fn registry_load_corrupted_returns_error() {
    let path = tmp_path("corrupt");
    cleanup(&path);
    std::fs::write(&path, b"{ not valid json").expect("write");
    let result = ModelRegistry::load_from_path(&path);
    assert!(result.is_err());
    cleanup(&path);
}

#[test]
fn registry_persist_writes_via_store() {
    let store = fresh_store("persist");
    store.inner.write().unwrap().active_id = Some("bai".into());
    store.persist().expect("persist");
    let loaded = ModelRegistry::load_from_path(&store.registry_path).expect("load");
    assert_eq!(loaded.active_id.as_deref(), Some("bai"));
    cleanup(&store.registry_path);
    cleanup(&store.assets_root);
}

// ============================================================================
// 3. resolve_safe_path 路径安全（handler 内部）
// ============================================================================

#[test]
fn resolve_safe_path_accepts_legitimate_id() {
    let (assets_root, model3_path) = make_minimal_package("resolve_ok");
    let resolved =
        handlers::resolve_safe_path(&assets_root, "resolve_ok").expect("safe path should resolve");
    assert!(resolved.is_dir());
    assert_eq!(
        resolved,
        model3_path.parent().unwrap().canonicalize().unwrap()
    );
    cleanup(&assets_root);
}

#[test]
fn resolve_safe_path_rejects_traversal() {
    let (assets_root, _model3_path) = make_minimal_package("ok");
    let cases = ["..", "../etc", "a/../b", ".", ""];
    for id in cases {
        let result = handlers::resolve_safe_path(&assets_root, id);
        assert!(result.is_err(), "id {id:?} should be rejected");
    }
    cleanup(&assets_root);
}

// ============================================================================
// 4. 路由表：match_model_route
// ============================================================================

#[test]
fn match_model_route_list_only_get() {
    assert_eq!(
        match_model_route(&Method::Get, "/api/v1/models"),
        Some(ModelRouteId::List)
    );
    assert_eq!(match_model_route(&Method::Post, "/api/v1/models"), None);
}

#[test]
fn match_model_route_import_only_post() {
    assert_eq!(
        match_model_route(&Method::Post, "/api/v1/models/import"),
        Some(ModelRouteId::Import)
    );
    assert_eq!(
        match_model_route(&Method::Get, "/api/v1/models/import"),
        None
    );
}

#[test]
fn match_model_route_get_and_delete() {
    assert_eq!(
        match_model_route(&Method::Get, "/api/v1/models/bai"),
        Some(ModelRouteId::Get)
    );
    assert_eq!(
        match_model_route(&Method::Delete, "/api/v1/models/bai"),
        Some(ModelRouteId::Delete)
    );
    assert_eq!(
        match_model_route(&Method::Patch, "/api/v1/models/bai"),
        None
    );
}

#[test]
fn match_model_route_activate_and_display() {
    assert_eq!(
        match_model_route(&Method::Post, "/api/v1/models/bai/activate"),
        Some(ModelRouteId::Activate)
    );
    assert_eq!(
        match_model_route(&Method::Get, "/api/v1/models/bai/activate"),
        None
    );
    assert_eq!(
        match_model_route(&Method::Patch, "/api/v1/models/bai/display"),
        Some(ModelRouteId::Display)
    );
    assert_eq!(
        match_model_route(&Method::Post, "/api/v1/models/bai/display"),
        None
    );
}

#[test]
fn match_model_route_rejects_traversal_id() {
    // 含 `..` 的 id 已被 mod 路由表层 `is_models_subpath` 拒（这里
    // match_model_route 仍做一次兜底，handler 内不再做第二遍）。
    assert_eq!(
        match_model_route(&Method::Get, "/api/v1/models/../etc"),
        None
    );
    assert_eq!(match_model_route(&Method::Get, "/api/v1/models/"), None);
}

// ============================================================================
// 5. DTO schema
// ============================================================================

#[test]
fn dto_segment_apply_preserves_unset_fields() {
    // DisplayModelPatch：未提供的字段不修改 target。
    let mut d = ModelDisplay::default();
    let patch: DisplayModelPatch = serde_json::from_str(r#"{"scale":2.0}"#).unwrap();
    patch.apply_into(&mut d);
    assert!((d.scale - 2.0).abs() < f32::EPSILON);
    assert_eq!(d.offset_x, 0.0);
    assert_eq!(d.rotation, 0.0);
    assert_eq!(d.fit_mode, FitMode::Contain);
}

#[test]
fn dto_segment_stage_apply_preserves_unset_fields() {
    let mut s = StageConfig::default();
    let patch: DisplayStagePatch =
        serde_json::from_str(r#"{"width":1000,"background_type":"color"}"#).unwrap();
    patch.apply_into(&mut s);
    assert_eq!(s.width, 1000);
    assert_eq!(s.height, 540);
    assert_eq!(s.background_type, BackgroundType::Color);
    assert_eq!(s.background_color, "#00000000");
}

#[test]
fn dto_rejects_unknown_fields_at_root() {
    let res: Result<DisplayModelPatch, _> = serde_json::from_str(r#"{"scale":1.0,"extra":1}"#);
    assert!(res.is_err());
}

#[test]
fn dto_fit_mode_parsing() {
    let p: DisplayModelPatch = serde_json::from_str(r#"{"fit_mode":"stretch"}"#).unwrap();
    assert_eq!(p.fit_mode, Some(FitMode::Stretch));
    // 容错：未知值（即便字符串合法）→ 反序列化失败。
    let bad: Result<DisplayModelPatch, _> = serde_json::from_str(r#"{"fit_mode":"unknown_mode"}"#);
    assert!(bad.is_err());
}

#[test]
fn dto_background_type_parsing() {
    let p: DisplayStagePatch = serde_json::from_str(r#"{"background_type":"image"}"#).unwrap();
    assert_eq!(p.background_type, Some(BackgroundType::Image));
}

// ============================================================================
// 6. cross-cutting：单一模型根 / StoredManifest
// ============================================================================

#[test]
fn registry_path_is_inside_the_single_model_root() {
    // rc.2 2026-09-12：registry 与静态 `/models/*` 必须共用**同一个根**
    // （`assets/models/`）。曾经 registry 走 XDG、静态服务走 cwd，于是
    // 「导入并激活后皮套不换」——这条守的就是那个回归。
    let root = crate::web_api::model_root::model_root();
    let p = crate::web_api::model_root::registry_path();
    assert!(p.ends_with("model_registry.json"), "{}", p.display());
    assert!(
        p.starts_with(&root),
        "registry 必须在模型根内部：{}",
        p.display()
    );
}

#[test]
fn stored_manifest_constructed_from_package_manifest() {
    // 构造一个 l2d::asset::PackageManifest 实例，确保 From 转换无字段丢失。
    let layout = l2d::asset::LayoutBox {
        center_x: 0.5,
        center_y: -0.25,
        width: 0.75,
        height: 1.5,
    };
    let pm = PackageManifest {
        version: 3,
        moc3_file: "bai.moc3".into(),
        texture_files: vec!["a.png".into(), "b.png".into()],
        physics_file: Some("phys.json".into()),
        display_info_file: None,
        layout,
    };
    let sm = StoredManifest::from(&pm);
    assert_eq!(sm.version, 3);
    assert_eq!(sm.moc3_file, "bai.moc3");
    assert_eq!(
        sm.texture_files,
        vec!["a.png".to_string(), "b.png".to_string()]
    );
    assert_eq!(sm.physics_file.as_deref(), Some("phys.json"));
    assert_eq!(sm.layout.center_x, 0.5);
    assert_eq!(sm.layout.width, 0.75);
}
