//! 0.3.5 多 Live2D：active 校验 / bai seed / URL 同源 / 删除保护。
//!
//! 测试模型**不进仓库**：每个用例把仓库里已跟踪的 `assets/models/bai/runtime`
//! 复制到 tempdir；坏模型只改 JSON / 删文件。绝不在 assets 下生成测试资产。

use std::fs;
use std::path::Path;

use super::handlers::{handle_activate, handle_delete, handle_list};
use super::registry::{ModelEntry, ModelRegistry};
use super::tests_models_common::{body_to_string, cleanup, fresh_store, sample_registry};
use super::{ModelStore, active_model};

/// 把出厂 bai 的最小运行包复制到测试模型根（源 = 仓库内已跟踪文件）。
fn copy_bai_runtime(assets_root: &Path) {
    let src = crate::web_api::model_root::model_root().join("bai/runtime");
    assert!(
        src.join("bai.model3.json").is_file(),
        "仓库里应有出厂白模：{}",
        src.display()
    );
    copy_dir(&src, &assets_root.join("bai/runtime"));
}

fn copy_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("create dst");
    for entry in fs::read_dir(src).expect("read src") {
        let entry = entry.expect("dir entry");
        let to = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &to);
        } else {
            fs::copy(entry.path(), &to).expect("copy file");
        }
    }
}

fn write_registry(store: &ModelStore, reg: &ModelRegistry) {
    let bytes = serde_json::to_vec_pretty(reg).expect("serialize");
    fs::write(&store.registry_path, bytes).expect("write registry");
}

fn store_with_bai(name: &str) -> ModelStore {
    let store = fresh_store(name);
    copy_bai_runtime(&store.assets_root);
    store
}

#[test]
fn empty_registry_seeds_bai_and_url_matches_the_id() {
    let store = store_with_bai("seed");
    // from_disk 之前先放好 bai，模拟「首次启动、registry 还不存在」。
    let store = ModelStore::from_disk(store.assets_root.clone(), store.registry_path.clone());

    let active = active_model(&store);
    assert_eq!(active.id, "bai");
    assert_eq!(active.url, "/models/bai/runtime/bai.model3.json");
    assert!(!store.registry_path.exists(), "seed 不写盘");
    cleanup(&store.assets_root);
    cleanup(&store.registry_path);
}

#[test]
fn list_marks_the_fallback_bai_as_active() {
    let store = store_with_bai("list_bai");
    let store = ModelStore::from_disk(store.assets_root.clone(), store.registry_path.clone());
    let body = body_to_string(handle_list(&store));
    assert!(body.contains(r#""id":"bai""#), "{body}");
    assert!(body.contains(r#""active":true"#), "{body}");
    cleanup(&store.assets_root);
    cleanup(&store.registry_path);
}

#[test]
fn active_id_without_an_entry_falls_back_to_bai_without_persisting() {
    let store = store_with_bai("ghost");
    let mut reg = sample_registry();
    reg.active_id = Some("ghost".into());
    write_registry(&store, &reg);
    let store = ModelStore::from_disk(store.assets_root.clone(), store.registry_path.clone());

    let active = active_model(&store);
    assert_eq!(active.id, "bai");
    assert_eq!(active.url, "/models/bai/runtime/bai.model3.json");
    let on_disk = fs::read_to_string(&store.registry_path).expect("registry");
    assert!(
        on_disk.contains(r#""active_id": "ghost""#),
        "回退只发生在内存，不得改写 registry：{on_disk}"
    );
    cleanup(&store.assets_root);
    cleanup(&store.registry_path);
}

#[test]
fn active_entry_with_a_missing_model3_falls_back_to_bai() {
    let store = store_with_bai("missing_model3");
    let mut reg = sample_registry();
    let mut broken: ModelEntry = reg.entries.get("bai").expect("bai").clone();
    broken.id = "broken".into();
    broken.display_name = "broken".into();
    broken.relative_dir = "broken".into();
    broken.model3_rel_path = "broken/runtime/broken.model3.json".into();
    reg.entries.insert("broken".into(), broken);
    reg.active_id = Some("broken".into());
    write_registry(&store, &reg);
    let store = ModelStore::from_disk(store.assets_root.clone(), store.registry_path.clone());

    let active = active_model(&store);
    assert_eq!(active.id, "bai");
    assert_eq!(active.url, "/models/bai/runtime/bai.model3.json");
    cleanup(&store.assets_root);
    cleanup(&store.registry_path);
}

#[test]
fn activate_rejects_a_missing_model3_and_keeps_the_previous_active() {
    let store = store_with_bai("activate_broken");
    let mut reg = sample_registry();
    reg.entries.get_mut("bai").expect("bai").model3_rel_path = "bai/runtime/bai.model3.json".into();
    let mut broken: ModelEntry = reg.entries.get("bai").expect("bai").clone();
    broken.id = "broken".into();
    broken.display_name = "broken".into();
    broken.relative_dir = "broken".into();
    broken.model3_rel_path = "broken/runtime/broken.model3.json".into();
    reg.entries.insert("broken".into(), broken);
    reg.active_id = None;
    write_registry(&store, &reg);
    let store = ModelStore::from_disk(store.assets_root.clone(), store.registry_path.clone());

    let resp = handle_activate(&store, "broken");
    assert_eq!(resp.status_code().0, 400, "model3 不在盘上不得承诺可加载");
    let body = body_to_string(resp);
    assert!(body.contains("invalid_model3_json"), "{body}");
    assert_eq!(
        store.inner.read().unwrap().active_id,
        None,
        "失败不得写 active_id"
    );

    let ok = handle_activate(&store, "bai");
    assert_eq!(ok.status_code().0, 200);
    let body = body_to_string(ok);
    assert!(
        body.contains(r#""model_url":"/models/bai/runtime/bai.model3.json""#),
        "{body}"
    );
    cleanup(&store.assets_root);
    cleanup(&store.registry_path);
}

#[test]
fn factory_bai_cannot_be_deleted() {
    let store = store_with_bai("delete_bai");
    let store = ModelStore::from_disk(store.assets_root.clone(), store.registry_path.clone());
    let resp = handle_delete(&store, "bai");
    assert_eq!(resp.status_code().0, 409);
    let body = body_to_string(resp);
    assert!(body.contains("model_builtin"), "{body}");
    assert!(store.inner.read().unwrap().entries.contains_key("bai"));
    cleanup(&store.assets_root);
    cleanup(&store.registry_path);
}
