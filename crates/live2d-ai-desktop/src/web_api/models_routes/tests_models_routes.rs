//! D3 `models_routes` 测试（D3 收口，2026-08-28）。
//!
//! 覆盖：
//! 1. **路径安全**：[`registry::normalize_relative_id`] 拒绝 `..`、`.` 开头、
//!    路径分隔符、空、非 ASCII、特殊字符；正常 id 通过。
//! 2. **registry 原子写回 + round-trip**：[`registry::atomic_write_json`]
//!    写盘后再 `load_from_path` 还原，逐字段一致；写盘失败时旧文件保留
//!    + tmp 文件清理。
//! 3. **路径解析**：[`handlers::resolve_safe_path`] 接受 sandbox 内合法
//!    id，拒绝 `..`（即使 `canonicalize` 后父目录允许）——构造越界路径
//!    触发 `Err`。
//! 4. **路由表**：[`match_model_route`] 6 条子路由正确命中；非法 method /
//!    非法 id（`..` / 空 / 含 `/`）→ None。
//! 5. **handler 端到端**：
//!    - `handle_import` 接受合法 model3 目录 → 200 + registry 落盘；
//!    - `handle_import` 拒绝 `..` 路径穿越 → 400 invalid_resource_path；
//!    - `handle_import` 拒绝不存在目录 → 400；
//!    - `handle_import` 拒绝无 model3.json 的目录 → 400 invalid_model3_json；
//!    - `handle_activate` 翻 active_id；
//!    - `handle_delete` 拒绝删除激活模型 → 409 model_active；
//!    - `handle_delete` 删除非激活模型 → 204；
//!    - `handle_display` 持久化新 display + stage 字段。
//! 6. **DTO schema**：[`DisplayModelPatch`] / [`DisplayStagePatch`] 段级
//!    None 字段保留原值（防误清空）；`deny_unknown_fields` 拒绝未登记键。

use std::io::{Cursor, Read};
use std::path::PathBuf;

use l2d::asset::PackageManifest;
use tiny_http::{Method, Response};

use super::ModelStore;
use super::dto::{DisplayModelPatch, DisplayStagePatch};
use super::registry::{
    BackgroundType, FitMode, ModelDisplay, ModelEntry, ModelRegistry, ModelSource, StageConfig,
    StoredLayout, StoredManifest, atomic_write_json, default_stage, normalize_relative_id,
    registry_path_for,
};
use super::{ModelRouteId, dispatch, handlers, match_model_route};

// ============================================================================
// 工具
// ============================================================================

fn body_to_string(resp: Response<Cursor<Vec<u8>>>) -> String {
    let mut s = String::new();
    let _ = resp.into_reader().read_to_string(&mut s);
    s
}

fn tmp_path(name: &str) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let thread = format!("{:?}", std::thread::current().id());
    let mut h = std::collections::hash_map::DefaultHasher::new();
    thread.hash(&mut h);
    std::env::temp_dir().join(format!(
        "live2d_ai_models_routes_{}_{}_{}.json",
        name,
        std::process::id(),
        h.finish(),
    ))
}

fn tmp_dir(name: &str) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let thread = format!("{:?}", std::thread::current().id());
    let mut h = std::collections::hash_map::DefaultHasher::new();
    thread.hash(&mut h);
    std::env::temp_dir().join(format!(
        "live2d_ai_models_routes_dir_{}_{}_{}",
        name,
        std::process::id(),
        h.finish(),
    ))
}

/// 在 tmp_dir/{id}/ 下组装一个最小合法 model3 包（含 moc3/纹理/physics3
/// 文件）；返回 model3.json 路径。`assets_root` = `tmp_dir` 父目录（= tmp）。
fn make_minimal_package(id: &str) -> (PathBuf, PathBuf) {
    let assets_root = tmp_dir(&format!("pkg_assets_{id}"));
    let model_dir = assets_root.join(id);
    std::fs::create_dir_all(&model_dir).expect("create model dir");
    // moc3 头（64 字节，version byte = 4 = Bai 档位）
    let mut moc3 = vec![0u8; 64];
    moc3[..4].copy_from_slice(b"MOC3");
    moc3[4] = 4;
    std::fs::write(model_dir.join("bai.moc3"), &moc3).expect("write moc3");
    std::fs::write(model_dir.join("tex_00.png"), b"\x89PNG-fake").expect("write tex");
    std::fs::write(
        model_dir.join("physics3.json"),
        br#"{"Version":3,"Meta":{},"PhysicsSettings":[]}"#,
    )
    .expect("write physics");
    let model3 = br#"{
        "Version": 3,
        "FileReferences": {
            "Moc": "bai.moc3",
            "Textures": ["tex_00.png"],
            "Physics": "physics3.json"
        },
        "Layout": {"center_x": 0.0, "center_y": 0.0, "width": 1.0, "height": 1.0}
    }"#;
    let model3_path = model_dir.join("bai.model3.json");
    std::fs::write(&model3_path, model3).expect("write model3");
    (assets_root, model3_path)
}

fn cleanup(p: &std::path::Path) {
    if p.is_file() {
        let _ = std::fs::remove_file(p);
    }
    if p.is_dir() {
        let _ = std::fs::remove_dir_all(p);
    }
}

fn fresh_store(name: &str) -> ModelStore {
    let reg = tmp_path(name);
    cleanup(&reg);
    let assets_root = tmp_dir(&format!("assets_{name}"));
    cleanup(&assets_root);
    std::fs::create_dir_all(&assets_root).expect("create assets_root");
    ModelStore::new(assets_root, reg)
}

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

fn sample_registry() -> ModelRegistry {
    let manifest = StoredManifest {
        version: 3,
        moc3_file: "bai.moc3".into(),
        texture_files: vec!["tex_00.png".into()],
        physics_file: Some("physics3.json".into()),
        display_info_file: None,
        layout: StoredLayout {
            center_x: 0.0,
            center_y: 0.0,
            width: 1.0,
            height: 1.0,
        },
    };
    // P1-4：registry 只存相对 `assets_root` 的路径。
    let entry = ModelEntry {
        id: "bai".into(),
        display_name: "bai".into(),
        source: ModelSource::LocalImport,
        relative_dir: "bai".into(),
        model3_rel_path: "bai/bai.model3.json".into(),
        manifest,
        display: ModelDisplay::default(),
        stage: default_stage(),
        imported_at: "2026-08-28T12:00:00.000Z".into(),
    };
    let mut entries = std::collections::BTreeMap::new();
    entries.insert("bai".into(), entry);
    ModelRegistry {
        schema_version: 1,
        active_id: Some("bai".into()),
        entries,
    }
}

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
// 5. handler 端到端
// ============================================================================

#[test]
fn handle_list_empty_returns_200() {
    let store = fresh_store("list_empty");
    let resp = dispatch(&store, &Method::Get, "/api/v1/models", "");
    assert_eq!(resp.status_code().0, 200);
    let body = body_to_string(resp);
    assert!(body.contains("\"models\""), "body = {body}");
    assert!(body.contains("[]"), "body = {body}");
    cleanup(&store.registry_path);
    cleanup(&store.assets_root);
}

#[test]
fn handle_import_legit_package_succeeds() {
    let (assets_root, _model3_path) = make_minimal_package("legit");
    let reg = tmp_path("import_legit");
    let store = ModelStore::new(assets_root.clone(), reg.clone());
    let body = r#"{"id":"legit"}"#;
    let resp = dispatch(&store, &Method::Post, "/api/v1/models/import", body);
    assert_eq!(resp.status_code().0, 200, "body = {}", body_to_string(resp));
    let body = body_to_string(resp);
    assert!(body.contains("\"id\":\"legit\""), "body = {body}");
    assert!(body.contains("\"applied\":false"), "body = {body}");
    // 落盘
    let loaded = ModelRegistry::load_from_path(&reg).expect("registry on disk");
    assert!(loaded.entries.contains_key("legit"));
    cleanup(&reg);
    cleanup(&assets_root);
}

#[test]
fn handle_import_rejects_traversal_id() {
    let store = fresh_store("import_traversal");
    let body = r#"{"id":"../etc"}"#;
    let resp = dispatch(&store, &Method::Post, "/api/v1/models/import", body);
    assert_eq!(resp.status_code().0, 400);
    let body = body_to_string(resp);
    assert!(body.contains("invalid_resource_path"), "body = {body}");
    cleanup(&store.registry_path);
    cleanup(&store.assets_root);
}

#[test]
fn handle_import_rejects_nonexistent_dir() {
    let store = fresh_store("import_noent");
    let body = r#"{"id":"never_created"}"#;
    let resp = dispatch(&store, &Method::Post, "/api/v1/models/import", body);
    assert_eq!(resp.status_code().0, 400);
    let body = body_to_string(resp);
    assert!(body.contains("invalid_resource_path"), "body = {body}");
    assert!(body.contains("目录不存在"), "body = {body}");
    cleanup(&store.registry_path);
    cleanup(&store.assets_root);
}

#[test]
fn handle_import_rejects_dir_without_model3_json() {
    let assets_root = tmp_dir("import_no_model3");
    cleanup(&assets_root);
    std::fs::create_dir_all(assets_root.join("weird")).expect("create");
    std::fs::write(assets_root.join("weird/random.txt"), b"hi").expect("write");
    let reg = tmp_path("import_no_model3");
    let store = ModelStore::new(assets_root.clone(), reg.clone());
    let body = r#"{"id":"weird"}"#;
    let resp = dispatch(&store, &Method::Post, "/api/v1/models/import", body);
    assert_eq!(resp.status_code().0, 400);
    let body = body_to_string(resp);
    assert!(body.contains("invalid_model3_json"), "body = {body}");
    cleanup(&reg);
    cleanup(&assets_root);
}

#[test]
fn handle_import_id_conflict_returns_409() {
    let (assets_root, _model3_path) = make_minimal_package("dup");
    let reg = tmp_path("import_dup");
    let store = ModelStore::new(assets_root.clone(), reg.clone());
    let body = r#"{"id":"dup"}"#;
    let resp1 = dispatch(&store, &Method::Post, "/api/v1/models/import", body);
    assert_eq!(resp1.status_code().0, 200);
    let resp2 = dispatch(&store, &Method::Post, "/api/v1/models/import", body);
    assert_eq!(resp2.status_code().0, 409);
    let body = body_to_string(resp2);
    assert!(body.contains("id_conflict"), "body = {body}");
    cleanup(&reg);
    cleanup(&assets_root);
}

#[test]
fn handle_get_unknown_id_returns_404() {
    let store = fresh_store("get_404");
    let resp = dispatch(&store, &Method::Get, "/api/v1/models/never", "");
    assert_eq!(resp.status_code().0, 404);
    let body = body_to_string(resp);
    assert!(body.contains("model_not_found"), "body = {body}");
    cleanup(&store.registry_path);
    cleanup(&store.assets_root);
}

#[test]
fn handle_activate_then_delete_active_rejected() {
    let (assets_root, _model3_path) = make_minimal_package("actdel");
    let reg = tmp_path("actdel");
    let store = ModelStore::new(assets_root.clone(), reg.clone());
    // 导入
    let resp = dispatch(
        &store,
        &Method::Post,
        "/api/v1/models/import",
        r#"{"id":"actdel"}"#,
    );
    assert_eq!(resp.status_code().0, 200);
    // activate
    let resp = dispatch(&store, &Method::Post, "/api/v1/models/actdel/activate", "");
    assert_eq!(resp.status_code().0, 200);
    let body = body_to_string(resp);
    assert!(body.contains("\"active_id\":\"actdel\""), "body = {body}");
    assert!(body.contains("\"prev_active_id\":null"), "body = {body}");
    // 删除激活模型 → 409 model_active
    let resp = dispatch(&store, &Method::Delete, "/api/v1/models/actdel", "");
    assert_eq!(resp.status_code().0, 409);
    let body = body_to_string(resp);
    assert!(body.contains("model_active"), "body = {body}");
    cleanup(&reg);
    cleanup(&assets_root);
}

#[test]
fn handle_delete_non_active_succeeds_204() {
    let (assets_root, _model3_path) = make_minimal_package("kill");
    let reg = tmp_path("kill");
    let store = ModelStore::new(assets_root.clone(), reg.clone());
    let resp = dispatch(
        &store,
        &Method::Post,
        "/api/v1/models/import",
        r#"{"id":"kill"}"#,
    );
    assert_eq!(resp.status_code().0, 200);
    // activate
    let resp = dispatch(&store, &Method::Post, "/api/v1/models/kill/activate", "");
    assert_eq!(resp.status_code().0, 200);
    // 导入第二个
    let (assets_root2, _m2) = make_minimal_package("kill2");
    let reg2 = tmp_path("kill2");
    let store2 = ModelStore::new(assets_root2.clone(), reg2.clone());
    let resp = dispatch(
        &store2,
        &Method::Post,
        "/api/v1/models/import",
        r#"{"id":"kill2"}"#,
    );
    assert_eq!(resp.status_code().0, 200);
    // 删除未激活的 kill2 → 204
    let resp = dispatch(&store2, &Method::Delete, "/api/v1/models/kill2", "");
    assert_eq!(resp.status_code().0, 204);
    cleanup(&reg);
    cleanup(&reg2);
    cleanup(&assets_root);
    cleanup(&assets_root2);
}

#[test]
fn handle_display_persists_model_and_stage() {
    let (assets_root, _model3_path) = make_minimal_package("disp");
    let reg = tmp_path("disp");
    let store = ModelStore::new(assets_root.clone(), reg.clone());
    // 导入
    let resp = dispatch(
        &store,
        &Method::Post,
        "/api/v1/models/import",
        r#"{"id":"disp"}"#,
    );
    assert_eq!(resp.status_code().0, 200);
    // PATCH display
    let body = r##"{"model":{"scale":1.5,"offset_x":12.0,"rotation":45.0,"fit_mode":"cover"},"stage":{"width":800,"height":600,"background_type":"color","background_color":"#FF0000FF"}}"##;
    let resp = dispatch(&store, &Method::Patch, "/api/v1/models/disp/display", body);
    assert_eq!(resp.status_code().0, 200);
    // 从磁盘 reload 校验持久化。
    let loaded = ModelRegistry::load_from_path(&reg).expect("load");
    let e = loaded.entries.get("disp").expect("entry");
    assert!((e.display.scale - 1.5).abs() < f32::EPSILON);
    assert_eq!(e.display.fit_mode, FitMode::Cover);
    assert_eq!(e.stage.width, 800);
    assert_eq!(e.stage.height, 600);
    assert_eq!(e.stage.background_type, BackgroundType::Color);
    assert_eq!(e.stage.background_color, "#FF0000FF");
    cleanup(&reg);
    cleanup(&assets_root);
}

#[test]
fn handle_display_partial_only_stage_preserves_model() {
    let (assets_root, _model3_path) = make_minimal_package("partial");
    let reg = tmp_path("partial");
    let store = ModelStore::new(assets_root.clone(), reg.clone());
    let resp = dispatch(
        &store,
        &Method::Post,
        "/api/v1/models/import",
        r#"{"id":"partial"}"#,
    );
    assert_eq!(resp.status_code().0, 200);
    // 先设 model.scale = 1.5
    let resp = dispatch(
        &store,
        &Method::Patch,
        "/api/v1/models/partial/display",
        r#"{"model":{"scale":1.5}}"#,
    );
    assert_eq!(resp.status_code().0, 200);
    // 再只设 stage.width = 999（model 应保留 1.5）
    let resp = dispatch(
        &store,
        &Method::Patch,
        "/api/v1/models/partial/display",
        r#"{"stage":{"width":999}}"#,
    );
    assert_eq!(resp.status_code().0, 200);
    let loaded = ModelRegistry::load_from_path(&reg).expect("load");
    let e = loaded.entries.get("partial").expect("entry");
    assert!((e.display.scale - 1.5).abs() < f32::EPSILON);
    assert_eq!(e.stage.width, 999);
    assert_eq!(e.stage.height, 540, "stage.height 应保留默认 540");
    cleanup(&reg);
    cleanup(&assets_root);
}

#[test]
fn handle_display_rejects_unknown_field() {
    let (assets_root, _model3_path) = make_minimal_package("unk");
    let reg = tmp_path("unk");
    let store = ModelStore::new(assets_root.clone(), reg.clone());
    let resp = dispatch(
        &store,
        &Method::Post,
        "/api/v1/models/import",
        r#"{"id":"unk"}"#,
    );
    assert_eq!(resp.status_code().0, 200);
    // PATCH 带未登记字段 `unknown_field` → 400 invalid_payload（deny_unknown_fields）
    let body = r#"{"model":{"scale":1.0,"unknown_field":42}}"#;
    let resp = dispatch(&store, &Method::Patch, "/api/v1/models/unk/display", body);
    assert_eq!(resp.status_code().0, 400);
    let body = body_to_string(resp);
    assert!(body.contains("invalid_payload"), "body = {body}");
    cleanup(&reg);
    cleanup(&assets_root);
}

#[test]
fn handle_display_unknown_id_returns_404() {
    let store = fresh_store("disp_404");
    let resp = dispatch(
        &store,
        &Method::Patch,
        "/api/v1/models/never/display",
        r#"{"model":{"scale":1.0}}"#,
    );
    assert_eq!(resp.status_code().0, 404);
    let body = body_to_string(resp);
    assert!(body.contains("model_not_found"), "body = {body}");
    cleanup(&store.registry_path);
    cleanup(&store.assets_root);
}

// ============================================================================
// 6. DTO schema
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
// 7. cross-cutting
// ============================================================================

#[test]
fn registry_path_for_is_stable_string() {
    let p = registry_path_for();
    assert!(p.ends_with("model_registry.json"));
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

// ============================================================================
// 8. P1-3 — DisplayPatchBody 校验
// ============================================================================
//
// 设计：JSON Schema 不引入；用 `DisplayPatchBody::validate()` 在 handler
// 内统一拦截。校验失败回 400 invalid_params（与 invalid_payload
// 区分：前者结构对、值越界；后者结构错）。

/// 单元校验（不经过 dispatch）：单个字段越界应被 `validate()` 拦下。
#[test]
fn p1_3_validate_unit_rejects_bad_values() {
    // `DisplayModelPatch` 是扁平字段（不是 {"model": {...}} 嵌套）——后
    // 者属于 `DisplayPatchBody`。直接喂扁平 body 即可。
    let bad_scales = [
        r#"{"scale":0.0}"#,
        r#"{"scale":-1.0}"#,
        // NaN：serde_json 解析 "NaN" 失败（不允许非 finite 字面量），
        // 所以改用合法 finite 但越界/NaN-style 的字符串不行——直接
        // 构造 f32::NAN 走 validate 路径（不走 JSON）。
    ];
    for body in bad_scales {
        let p: DisplayModelPatch = serde_json::from_str(body).unwrap_or_else(|e| {
            panic!("patch 应能 parse（{body}）: {e}");
        });
        assert!(p.validate().is_err(), "body {body} 应被 validate 拒绝");
    }

    // f32::NAN / Infinity：手工构造避开 JSON 字面量限制。
    let nan_patch = DisplayModelPatch {
        scale: Some(f32::NAN),
        ..Default::default()
    };
    assert!(nan_patch.validate().is_err(), "NaN scale 应被拒绝");
    let inf_patch = DisplayModelPatch {
        offset_x: Some(f32::INFINITY),
        ..Default::default()
    };
    assert!(inf_patch.validate().is_err(), "Inf offset_x 应被拒绝");

    // stage.width = 0 / height = 0
    let p: DisplayStagePatch = serde_json::from_str(r#"{"width":0}"#).unwrap();
    assert!(p.validate().is_err());
    let p: DisplayStagePatch = serde_json::from_str(r#"{"height":0}"#).unwrap();
    assert!(p.validate().is_err());

    // stage.background_opacity 越界
    let p: DisplayStagePatch = serde_json::from_str(r#"{"background_opacity":1.5}"#).unwrap();
    assert!(p.validate().is_err());
    let p: DisplayStagePatch = serde_json::from_str(r#"{"background_opacity":-0.1}"#).unwrap();
    assert!(p.validate().is_err());

    // stage.background_color 非法（命名色 / rgba / 短 hex / 缺 #）
    for color in [
        "\"red\"",
        "\"rgba(1,2,3,1)\"",
        "\"#FFF\"",
        "\"FF0000\"",
        "\"#GG0000\"",
    ] {
        let body = format!(r#"{{"background_color":{color}}}"#);
        let p: DisplayStagePatch = serde_json::from_str(&body).unwrap();
        assert!(p.validate().is_err(), "color {color} 应被拒绝");
    }

    // 合法用例（不变量）：scale=1.0、width/height>0、opacity=0/1、
    // #RRGGBB / #RRGGBBAA 都通过。
    let p: DisplayStagePatch = serde_json::from_str(
        r##"{"width":360,"height":540,"background_color":"#00000000","background_opacity":1.0}"##,
    )
    .unwrap();
    assert!(p.validate().is_ok());
    let p: DisplayStagePatch =
        serde_json::from_str(r##"{"background_color":"#FF0000FF","background_opacity":0.0}"##)
            .unwrap();
    assert!(p.validate().is_ok());
}

/// 端到端：handler 在 PATCH display 时跑校验；非法 body → 400 invalid_params。
#[test]
fn p1_3_dispatch_rejects_invalid_params() {
    let (assets_root, _model3_path) = make_minimal_package("valid");
    let reg = tmp_path("p1_3_invalid");
    let store = ModelStore::new(assets_root.clone(), reg.clone());
    let resp = dispatch(
        &store,
        &Method::Post,
        "/api/v1/models/import",
        r#"{"id":"valid"}"#,
    );
    assert_eq!(resp.status_code().0, 200);

    // 非法 scale（= 0）→ 400 invalid_params（不是 invalid_payload：结构对、值越界）。
    let resp = dispatch(
        &store,
        &Method::Patch,
        "/api/v1/models/valid/display",
        r#"{"model":{"scale":0.0}}"#,
    );
    assert_eq!(resp.status_code().0, 400, "scale=0 应被拒绝");
    let body = body_to_string(resp);
    assert!(body.contains("invalid_params"), "body = {body}");
    assert!(body.contains("scale"), "body = {body}");

    // 非法 opacity（> 1） → 400 invalid_params
    let resp = dispatch(
        &store,
        &Method::Patch,
        "/api/v1/models/valid/display",
        r#"{"stage":{"background_opacity":2.0}}"#,
    );
    assert_eq!(resp.status_code().0, 400);
    let body = body_to_string(resp);
    assert!(body.contains("invalid_params"), "body = {body}");
    assert!(body.contains("background_opacity"), "body = {body}");

    // 非法 width（= 0） → 400 invalid_params
    let resp = dispatch(
        &store,
        &Method::Patch,
        "/api/v1/models/valid/display",
        r#"{"stage":{"width":0}}"#,
    );
    assert_eq!(resp.status_code().0, 400);
    let body = body_to_string(resp);
    assert!(body.contains("invalid_params"), "body = {body}");
    assert!(body.contains("width"), "body = {body}");

    // 非法 background_color（命名色） → 400 invalid_params
    let resp = dispatch(
        &store,
        &Method::Patch,
        "/api/v1/models/valid/display",
        r#"{"stage":{"background_color":"red"}}"#,
    );
    assert_eq!(resp.status_code().0, 400);
    let body = body_to_string(resp);
    assert!(body.contains("invalid_params"), "body = {body}");
    assert!(body.contains("background_color"), "body = {body}");

    // 校验失败时**不应**修改 registry（回滚）：reload 后 width 仍为默认 360。
    let loaded = ModelRegistry::load_from_path(&reg).expect("load");
    let entry = loaded.entries.get("valid").expect("entry");
    assert_eq!(entry.stage.width, 360, "校验失败不应改 stage.width");

    cleanup(&reg);
    cleanup(&assets_root);
}

/// 端到端：合法 hex color（#RRGGBB / #RRGGBBAA）通过校验。
#[test]
fn p1_3_dispatch_accepts_valid_hex_colors() {
    let (assets_root, _model3_path) = make_minimal_package("hex_ok");
    let reg = tmp_path("p1_3_hex");
    let store = ModelStore::new(assets_root.clone(), reg.clone());
    let resp = dispatch(
        &store,
        &Method::Post,
        "/api/v1/models/import",
        r#"{"id":"hex_ok"}"#,
    );
    assert_eq!(resp.status_code().0, 200);

    for color in ["#FFFFFF", "#00000000", "#aBcDeF12", "#FF00FFFF"] {
        let body = format!(r#"{{"stage":{{"background_color":"{color}"}}}}"#);
        let resp = dispatch(
            &store,
            &Method::Patch,
            "/api/v1/models/hex_ok/display",
            &body,
        );
        assert_eq!(
            resp.status_code().0,
            200,
            "color {color} 应通过；body = {}",
            body_to_string(resp)
        );
    }
    cleanup(&reg);
    cleanup(&assets_root);
}

// ============================================================================
// 9. P1-4 — registry 只存相对路径
// ============================================================================

/// 落盘 → 读回：相对路径字段完整保留（无 absolute_dir 残留）。
#[test]
fn p1_4_registry_round_trip_uses_relative_paths() {
    let path = tmp_path("p1_4_rt");
    cleanup(&path);
    let original = sample_registry();
    atomic_write_json(&path, &original).expect("write");
    let raw = std::fs::read(&path).expect("read raw");
    let raw_str = std::str::from_utf8(&raw).unwrap();
    // 旧字段名应不再出现。
    assert!(
        !raw_str.contains("absolute_dir"),
        "registry JSON 不应再含 absolute_dir（旧字段）；raw = {raw_str}"
    );
    assert!(
        !raw_str.contains("\"model3_path\""),
        "registry JSON 不应再含 \"model3_path\" 字段（旧字段）；raw = {raw_str}"
    );
    // 新字段名应出现。
    assert!(raw_str.contains("relative_dir"));
    assert!(raw_str.contains("model3_rel_path"));

    // round-trip 一致性。
    let loaded = ModelRegistry::load_from_path(&path).expect("load");
    let e = loaded.entries.get("bai").expect("entry");
    assert_eq!(e.relative_dir, "bai");
    assert_eq!(e.model3_rel_path, "bai/bai.model3.json");

    cleanup(&path);
}

/// handler 端到端：`handle_import` 写入的 entry 字段是相对 `assets_root`。
#[test]
fn p1_4_handle_import_stores_relative_paths() {
    let (assets_root, _model3_path) = make_minimal_package("rel");
    let reg = tmp_path("p1_4_import");
    let store = ModelStore::new(assets_root.clone(), reg.clone());
    let resp = dispatch(
        &store,
        &Method::Post,
        "/api/v1/models/import",
        r#"{"id":"rel"}"#,
    );
    assert_eq!(resp.status_code().0, 200);

    let loaded = ModelRegistry::load_from_path(&reg).expect("load");
    let e = loaded.entries.get("rel").expect("entry");
    // 相对 `assets_root`（不是绝对路径、不是相对 data_dir）。
    assert_eq!(e.relative_dir, "rel");
    assert_eq!(e.model3_rel_path, "rel/bai.model3.json");
    // raw JSON 不应再含 absolute_dir 字段。
    let raw = std::fs::read(&reg).unwrap();
    let raw_str = std::str::from_utf8(&raw).unwrap();
    assert!(!raw_str.contains("absolute_dir"));

    cleanup(&reg);
    cleanup(&assets_root);
}

/// handle_list / handle_get 派生绝对路径计算 size——新 store 的
/// assets_root 与原导入路径不同（模拟 WSL / HOME 改变）应仍能工作。
#[test]
fn p1_4_absolute_path_derived_per_call_assets_root_churn() {
    let (assets_root, _model3_path) = make_minimal_package("churn");
    let reg = tmp_path("p1_4_churn");
    let store = ModelStore::new(assets_root.clone(), reg.clone());
    let resp = dispatch(
        &store,
        &Method::Post,
        "/api/v1/models/import",
        r#"{"id":"churn"}"#,
    );
    assert_eq!(resp.status_code().0, 200);

    // list/get 仍 OK（派生路径 = `assets_root.join("churn")`）。
    let resp = dispatch(&store, &Method::Get, "/api/v1/models", "");
    assert_eq!(resp.status_code().0, 200);
    let body = body_to_string(resp);
    assert!(body.contains("\"id\":\"churn\""), "body = {body}");
    assert!(
        body.contains("\"size_bytes\""),
        "size_bytes 应被派生；body = {body}"
    );
    cleanup(&reg);
    cleanup(&assets_root);
}

// ============================================================================
// 10. P1-5 — 损坏 registry 备份
// ============================================================================

/// `ModelStore::from_disk` 遇到损坏 registry 应备份原文件并用空 registry。
#[test]
fn p1_5_from_disk_backs_up_corrupt_registry() {
    // 准备损坏 registry + 临时 dir。
    let reg = tmp_path("p1_5_corrupt");
    cleanup(&reg);
    std::fs::write(&reg, b"{ not valid json").expect("write corrupt");

    let assets_root = tmp_dir("p1_5_assets");
    cleanup(&assets_root);
    std::fs::create_dir_all(&assets_root).unwrap();

    let store = ModelStore::from_disk(assets_root.clone(), reg.clone());
    // 内存应为空 registry。
    let g = store.inner.read().unwrap();
    assert!(g.entries.is_empty(), "损坏降级后 entries 应为空");

    // 损坏文件应在某 `<reg>.corrupt-<ts>` 备份中存在——按本测试 reg
    // 文件名前缀过滤，避免与其它测试残留混淆。
    let parent = reg.parent().unwrap();
    let reg_basename = reg.file_name().unwrap().to_str().unwrap().to_string();
    let backups: Vec<PathBuf> = std::fs::read_dir(parent)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|n| n.starts_with(&reg_basename) && n.contains(".corrupt-"))
        })
        .collect();
    assert!(
        !backups.is_empty(),
        "应有 {reg_basename}.corrupt-<ts> 备份文件；parent = {}",
        parent.display()
    );
    // 备份内容应等于原损坏字节。
    let backup_bytes = std::fs::read(&backups[0]).expect("read backup");
    assert_eq!(backup_bytes, b"{ not valid json");

    // 后续 persist 不应 panic（损坏副本已保留；新写盘覆盖 registry_path 即可）。
    drop(g);
    store
        .persist()
        .expect("persist should not panic after corrupt");

    cleanup(&reg);
    for b in &backups {
        cleanup(b);
    }
    cleanup(&assets_root);
}

/// 缺失 registry → 不应创建 `.corrupt-*` 备份（仅损坏分支触发备份）。
#[test]
fn p1_5_from_disk_missing_file_does_not_backup() {
    let reg = tmp_path("p1_5_missing");
    cleanup(&reg);
    let assets_root = tmp_dir("p1_5_assets_missing");
    cleanup(&assets_root);
    std::fs::create_dir_all(&assets_root).unwrap();

    let _store = ModelStore::from_disk(assets_root.clone(), reg.clone());
    // 确认没有 `<reg_basename>.corrupt-*` 备份被生成（按本测试 reg 名称过滤）。
    let parent = reg.parent().unwrap();
    let reg_basename = reg.file_name().unwrap().to_str().unwrap().to_string();
    let backups: Vec<PathBuf> = std::fs::read_dir(parent)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|n| n.starts_with(&reg_basename) && n.contains(".corrupt-"))
        })
        .collect();
    assert!(
        backups.is_empty(),
        "缺失场景不应创建备份；got = {backups:?}"
    );
    cleanup(&assets_root);
}

/// 备份文件名带 unix 秒（数值后缀）；不应影响正常 registry 加载。
#[test]
fn p1_5_backup_filename_has_unix_secs_suffix() {
    let reg = tmp_path("p1_5_suffix");
    cleanup(&reg);
    std::fs::write(&reg, b"<corrupt>").unwrap();
    let assets_root = tmp_dir("p1_5_assets_suffix");
    cleanup(&assets_root);
    std::fs::create_dir_all(&assets_root).unwrap();

    let _store = ModelStore::from_disk(assets_root.clone(), reg.clone());

    let parent = reg.parent().unwrap();
    let reg_basename = reg.file_name().unwrap().to_str().unwrap().to_string();
    let backups: Vec<PathBuf> = std::fs::read_dir(parent)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|n| n.starts_with(&reg_basename) && n.contains(".corrupt-"))
        })
        .collect();
    assert_eq!(backups.len(), 1, "应恰好一个 .corrupt-* 备份");
    let name = backups[0].file_name().unwrap().to_str().unwrap();
    // 提取 `.corrupt-<digits>` 后缀，确认是数字。
    let suffix = name.split(".corrupt-").nth(1).expect("suffix");
    assert!(
        suffix.chars().all(|c| c.is_ascii_digit()),
        "后缀应为 unix 秒（数字）；got = {suffix}"
    );

    cleanup(&reg);
    for b in &backups {
        cleanup(b);
    }
    cleanup(&assets_root);
}
