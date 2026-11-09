//! 0.5.1 rc.1：出厂模型通用化（bai + `seed.local.json`）与 builtin 删除保护。

use tiny_http::Method;

use super::handlers::handle_list;
use super::registry::{ModelEntry, ModelRegistry};
use super::tests_models_common::{
    body_to_string, cleanup, fresh_store, minimal_model3_json, write_package_at,
};
use super::{ModelStore, dispatch};

/// bai 的小写 Layout（与仓库 `bai.model3.json` 同口径）。
fn bai_layout() -> &'static str {
    r#"{"center_x":0.0,"center_y":-0.07,"width":0.56,"height":0.56}"#
}

fn seed_bai_package(assets_root: &std::path::Path) {
    write_package_at(
        &assets_root.join("bai/runtime"),
        &minimal_model3_json(Some(bai_layout())),
    );
}

fn write_seed_file(assets_root: &std::path::Path, body: &str) {
    std::fs::write(assets_root.join("seed.local.json"), body).expect("write seed.local.json");
}

/// 0.5.1 之前的 registry 没有 `builtin` 字段：必须能反序列化（默认 false），
/// 不能因为加字段把用户 registry 打成「损坏」走备份降级。
#[test]
fn old_registry_entry_without_builtin_field_parses_as_false() {
    let entry = super::tests_models_import_path::legacy_entry("old", "old", "old/bai.model3.json");
    let mut value = serde_json::to_value(&entry).expect("serialize");
    value
        .as_object_mut()
        .expect("object")
        .remove("builtin")
        .expect("旧 registry 没有 builtin");
    let parsed: ModelEntry = serde_json::from_value(value).expect("旧 registry 必须能反序列化");
    assert!(!parsed.builtin, "缺 builtin 字段 → serde(default) = false");
    let with_true = ModelEntry {
        builtin: true,
        ..parsed
    };
    let round: ModelEntry =
        serde_json::from_value(serde_json::to_value(&with_true).expect("ser")).expect("de");
    assert!(round.builtin, "builtin=true 必须能往返");
}

/// 没有 `seed.local.json`：只 seed bai，且 bai 带 builtin 标记（不可删）。
#[test]
fn missing_seed_file_seeds_only_bai() {
    let store = fresh_store("seed_missing");
    seed_bai_package(&store.assets_root);
    let store = ModelStore::from_disk(store.assets_root.clone(), store.registry_path.clone());
    let registry = store.inner.read().unwrap().clone();
    assert_eq!(registry.entries.len(), 1, "只有 bai");
    let bai = &registry.entries["bai"];
    assert!(bai.builtin, "bai 必须是出厂模型");
    assert_eq!(
        bai.display_name, "白模（bai）",
        "展示名走数据，前端不再硬编码"
    );
    assert_eq!(bai.model3_rel_path, "bai/runtime/bai.model3.json");
    cleanup(&store.assets_root);
    cleanup(&store.registry_path);
}

/// 坏 JSON：warn + 跳过（不 panic、不影响 bai seed）。
#[test]
fn bad_seed_json_is_skipped_but_bai_still_seeded() {
    let store = fresh_store("seed_bad_json");
    seed_bai_package(&store.assets_root);
    write_seed_file(&store.assets_root, "{ this is not json");
    let store = ModelStore::from_disk(store.assets_root.clone(), store.registry_path.clone());
    let registry = store.inner.read().unwrap().clone();
    assert!(registry.entries.contains_key("bai"));
    assert_eq!(registry.entries.len(), 1, "坏 JSON 不产生条目");
    cleanup(&store.assets_root);
    cleanup(&store.registry_path);
}

/// 单条无效（穿越 id / 目录不存在 / 无 model3 / model3 坏）只跳过该条；
/// 同文件里的有效条目照常出厂。
#[test]
fn invalid_seed_entries_are_skipped_valid_one_is_seeded() {
    let store = fresh_store("seed_invalid");
    seed_bai_package(&store.assets_root);
    write_package_at(&store.assets_root.join("good"), &minimal_model3_json(None));
    std::fs::create_dir_all(store.assets_root.join("nomodel3")).expect("mkdir");
    std::fs::write(
        store.assets_root.join("nomodel3/readme.txt"),
        b"no model3 here",
    )
    .expect("write");
    write_package_at(&store.assets_root.join("brokenmodel3"), "{ not json");
    write_seed_file(
        &store.assets_root,
        r#"[
            {"id":"../evil","display_name":"越界"},
            {"id":"missing","display_name":"不存在"},
            {"id":"nomodel3","display_name":"没有清单"},
            {"id":"brokenmodel3","display_name":"坏清单"},
            {"id":"good","display_name":"大肥鱼"}
        ]"#,
    );
    let store = ModelStore::from_disk(store.assets_root.clone(), store.registry_path.clone());
    let registry = store.inner.read().unwrap().clone();
    assert!(registry.entries.contains_key("bai"), "bai 照常");
    for bad in ["../evil", "missing", "nomodel3", "brokenmodel3"] {
        assert!(
            !registry.entries.contains_key(bad),
            "无效条目不得入库: {bad}"
        );
    }
    let good = &registry.entries["good"];
    assert!(good.builtin, "有效条目必须是出厂模型");
    assert_eq!(good.display_name, "大肥鱼", "展示名来自 seed.local.json");
    assert_eq!(good.model3_rel_path, "good/bai.model3.json");
    cleanup(&store.assets_root);
    cleanup(&store.registry_path);
}

/// 旧 registry 里已有 bai entry（builtin=false）：载入时必须补成 true，
/// 否则老用户升级后 bai 会变成可删。
#[test]
fn old_bai_entry_gets_builtin_flag_on_load() {
    let store = fresh_store("seed_old_bai");
    seed_bai_package(&store.assets_root);
    let mut registry = ModelRegistry {
        schema_version: 1,
        active_id: Some("bai".into()),
        entries: std::collections::BTreeMap::new(),
    };
    registry.entries.insert(
        "bai".into(),
        super::tests_models_import_path::legacy_entry("bai", "bai", "bai/runtime/bai.model3.json"),
    );
    std::fs::write(
        &store.registry_path,
        serde_json::to_vec_pretty(&registry).expect("ser"),
    )
    .expect("write registry");
    let store = ModelStore::from_disk(store.assets_root.clone(), store.registry_path.clone());
    assert!(
        store.inner.read().unwrap().entries["bai"].builtin,
        "旧 registry 的 bai 必须在载入时补 builtin"
    );
    cleanup(&store.assets_root);
    cleanup(&store.registry_path);
}

/// builtin 条目不可删（409 model_builtin）；非 builtin 条目可删（204）。
#[test]
fn builtin_entry_cannot_be_deleted_non_builtin_can() {
    let store = fresh_store("seed_delete");
    seed_bai_package(&store.assets_root);
    write_package_at(&store.assets_root.join("skin"), &minimal_model3_json(None));
    write_package_at(&store.assets_root.join("plain"), &minimal_model3_json(None));
    write_seed_file(
        &store.assets_root,
        r#"[{"id":"skin","display_name":"大肥鱼"}]"#,
    );
    let store = ModelStore::from_disk(store.assets_root.clone(), store.registry_path.clone());

    let resp = dispatch(&store, &Method::Delete, "/api/v1/models/skin", "");
    assert_eq!(resp.status_code().0, 409, "{}", body_to_string(resp));
    // bai 也仍然不可删（id 兜底 + builtin 判据两条都在）。
    let resp = dispatch(&store, &Method::Delete, "/api/v1/models/bai", "");
    assert_eq!(resp.status_code().0, 409, "{}", body_to_string(resp));

    // 非 builtin：import 后可删。
    let resp = dispatch(
        &store,
        &Method::Post,
        "/api/v1/models/import",
        r#"{"id":"plain"}"#,
    );
    assert_eq!(resp.status_code().0, 200, "{}", body_to_string(resp));
    let resp = dispatch(&store, &Method::Delete, "/api/v1/models/plain", "");
    assert_eq!(resp.status_code().0, 204, "{}", body_to_string(resp));

    let registry = store.inner.read().unwrap().clone();
    assert!(registry.entries.contains_key("skin"), "builtin 不得被删掉");
    assert!(registry.entries.contains_key("bai"));
    assert!(!registry.entries.contains_key("plain"));
    cleanup(&store.assets_root);
    cleanup(&store.registry_path);
}

/// 列表暴露 builtin 与 display_name：前端据此画徽章/标题，不再硬编码 bai。
#[test]
fn list_exposes_builtin_and_display_name() {
    let store = fresh_store("seed_list");
    seed_bai_package(&store.assets_root);
    write_package_at(&store.assets_root.join("skin"), &minimal_model3_json(None));
    write_seed_file(
        &store.assets_root,
        r#"[{"id":"skin","display_name":"大肥鱼"}]"#,
    );
    let store = ModelStore::from_disk(store.assets_root.clone(), store.registry_path.clone());
    let body = body_to_string(handle_list(&store));
    assert!(body.contains(r#""id":"bai""#), "{body}");
    assert!(
        body.contains(r#""builtin":true"#),
        "builtin 必须透出: {body}"
    );
    assert!(body.contains(r#""display_name":"大肥鱼""#), "{body}");
    assert!(
        body.contains(r#""builtin":false"#) == false,
        "当前只有出厂模型: {body}"
    );
    cleanup(&store.assets_root);
    cleanup(&store.registry_path);
}

/// 用户先 import 过、之后写进 seed.local.json：升级为 builtin（不清用户配置）。
#[test]
fn seed_file_upgrades_imported_entry_to_builtin() {
    let store = fresh_store("seed_upgrade");
    seed_bai_package(&store.assets_root);
    write_package_at(&store.assets_root.join("skin"), &minimal_model3_json(None));
    let store = ModelStore::from_disk(store.assets_root.clone(), store.registry_path.clone());
    let resp = dispatch(
        &store,
        &Method::Post,
        "/api/v1/models/import",
        r#"{"id":"skin"}"#,
    );
    assert_eq!(resp.status_code().0, 200, "{}", body_to_string(resp));
    assert!(
        !store.inner.read().unwrap().entries["skin"].builtin,
        "import 进来的普通模型不是出厂"
    );
    write_seed_file(
        &store.assets_root,
        r#"[{"id":"skin","display_name":"大肥鱼"}]"#,
    );
    let store = ModelStore::from_disk(store.assets_root.clone(), store.registry_path.clone());
    let entry = store.inner.read().unwrap().entries["skin"].clone();
    assert!(entry.builtin, "seed.local.json 声明的 id 必须升级为出厂");
    assert_eq!(entry.display_name, "大肥鱼");
    cleanup(&store.assets_root);
    cleanup(&store.registry_path);
}
