//! `models_routes` 测试共享脚手架（自 `tests_models_routes.rs` 拆出）。
//!
//! 临时路径 / 最小合法模型包 / 清理 / 空 store / 样例 registry。
//! 仅供同目录的 `tests_models_core` / `tests_models_handlers` / `tests_models_p1` 使用。

use std::io::{Cursor, Read};
use std::path::PathBuf;

use tiny_http::Response;

use super::ModelStore;
use super::registry::{
    ModelDisplay, ModelEntry, ModelRegistry, ModelSource, StoredLayout, StoredManifest,
    default_stage,
};

pub(super) fn body_to_string(resp: Response<Cursor<Vec<u8>>>) -> String {
    let mut s = String::new();
    let _ = resp.into_reader().read_to_string(&mut s);
    s
}

pub(super) fn tmp_path(name: &str) -> PathBuf {
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

pub(super) fn tmp_dir(name: &str) -> PathBuf {
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
pub(super) fn make_minimal_package(id: &str) -> (PathBuf, PathBuf) {
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

pub(super) fn cleanup(p: &std::path::Path) {
    if p.is_file() {
        let _ = std::fs::remove_file(p);
    }
    if p.is_dir() {
        let _ = std::fs::remove_dir_all(p);
    }
}

pub(super) fn fresh_store(name: &str) -> ModelStore {
    let reg = tmp_path(name);
    cleanup(&reg);
    let assets_root = tmp_dir(&format!("assets_{name}"));
    cleanup(&assets_root);
    std::fs::create_dir_all(&assets_root).expect("create assets_root");
    ModelStore::new(assets_root, reg)
}

pub(super) fn sample_registry() -> ModelRegistry {
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
