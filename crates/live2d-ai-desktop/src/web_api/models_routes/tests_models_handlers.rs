//! `models_routes` handler 端到端测试（自 `tests_models_routes.rs` 拆出）。
//!
//! 场景：`/api/v1/models*` 的 list / import / get / activate / delete / display
//! 六条子路由的 HTTP 状态码与落盘行为。

use tiny_http::Method;

use super::registry::{BackgroundType, FitMode, ModelRegistry};
use super::tests_models_common::{
    body_to_string, cleanup, fresh_store, make_minimal_package, tmp_dir, tmp_path,
};
use super::{ModelStore, dispatch};

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
