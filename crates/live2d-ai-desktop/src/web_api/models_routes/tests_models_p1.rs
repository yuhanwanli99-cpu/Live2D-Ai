//! `models_routes` P1 里程碑回归测试（自 `tests_models_routes.rs` 拆出）。
//!
//! 场景：① P1-3 DisplayPatchBody 校验；② P1-4 registry 只存相对路径；
//! ③ P1-5 损坏 registry 备份；④ A1（rc.2）单一模型根 + 深一层布局 + 热换契约。

use std::path::PathBuf;

use tiny_http::Method;

use super::dto::{DisplayModelPatch, DisplayStagePatch};
use super::registry::{ModelRegistry, atomic_write_json};
use super::tests_models_common::{
    body_to_string, cleanup, make_minimal_package, sample_registry, tmp_dir, tmp_path,
};
use super::{ModelStore, dispatch};

// ============================================================================
// P1-3 — DisplayPatchBody 校验
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
// P1-4 — registry 只存相对路径
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
// P1-5 — 损坏 registry 备份
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

// ============================================================================
// A1（rc.2）— 单一模型根 + 深一层布局 + 热换契约
// ============================================================================

/// 造一个**深一层**的模型包：`assets/<id>/runtime/<id>.model3.json`。
///
/// 这正是本仓库的既有布局（`assets/models/bai/runtime/bai.model3.json`）——
/// 旧实现只扫顶层，于是「导入自家自带的模型」必然 400。
fn make_nested_package(id: &str) -> PathBuf {
    let assets_root = tmp_dir(&format!("nested_assets_{id}"));
    cleanup(&assets_root);
    let runtime = assets_root.join(id).join("runtime");
    std::fs::create_dir_all(&runtime).expect("create runtime dir");
    let mut moc3 = vec![0u8; 64];
    moc3[..4].copy_from_slice(b"MOC3");
    moc3[4] = 4;
    std::fs::write(runtime.join("bai.moc3"), &moc3).expect("write moc3");
    std::fs::write(runtime.join("tex_00.png"), b"\x89PNG-fake").expect("write tex");
    std::fs::write(
        runtime.join("bai.model3.json"),
        br#"{"Version":3,"FileReferences":{"Moc":"bai.moc3","Textures":["tex_00.png"]}}"#,
    )
    .expect("write model3");
    assets_root
}

/// 深一层布局：import 认得它，登记的相对路径含 `runtime/`，
/// activate 回的 `model_url` 能被静态路由**原样 GET** 到。
///
/// 这三条缺一条就是线上那句「激活成功但皮套不换」。
#[test]
fn a1_nested_layout_imports_and_activates_with_a_getable_url() {
    let assets_root = make_nested_package("deep");
    let reg = tmp_path("a1_nested");
    cleanup(&reg);
    let store = ModelStore::new(assets_root.clone(), reg.clone());

    let resp = dispatch(
        &store,
        &Method::Post,
        "/api/v1/models/import",
        r#"{"id":"deep"}"#,
    );
    assert_eq!(
        resp.status_code().0,
        200,
        "深一层布局必须能导入：{}",
        body_to_string(resp)
    );
    let loaded = ModelRegistry::load_from_path(&reg).expect("registry on disk");
    let entry = loaded.entries.get("deep").expect("entry");
    // 相对路径必须保留 runtime/ 段（旧实现拼 file_name 会丢掉它 → model_url 404）。
    assert_eq!(entry.model3_rel_path, "deep/runtime/bai.model3.json");

    // activate → model_url 必须与静态路由的口径一致，且文件真的在。
    let resp = dispatch(&store, &Method::Post, "/api/v1/models/deep/activate", "");
    assert_eq!(resp.status_code().0, 200);
    let body = body_to_string(resp);
    assert!(
        body.contains("\"model_url\":\"/models/deep/runtime/bai.model3.json\""),
        "body = {body}"
    );
    assert!(
        store
            .assets_root
            .join("deep/runtime/bai.model3.json")
            .is_file(),
        "model_url 指向的文件必须存在（否则前端拿到一个必然 404 的地址）"
    );

    cleanup(&reg);
    cleanup(&assets_root);
}

/// 热换契约（rc.2 冻结）：activate **恒** `requires_restart=false`。
///
/// 恒 `true` 会骗前端提示「需重启」，而渲染面本来就支持热换——这正是
/// 「激活了但没换皮」那一格的成因。真需要重启时应逐案返 `true` 并说明理由。
#[test]
fn a1_activate_reports_hot_swap_not_restart() {
    let (assets_root, _) = make_minimal_package("hot");
    let reg = tmp_path("a1_hot");
    cleanup(&reg);
    let store = ModelStore::new(assets_root.clone(), reg.clone());
    let _ = dispatch(
        &store,
        &Method::Post,
        "/api/v1/models/import",
        r#"{"id":"hot"}"#,
    );
    let resp = dispatch(&store, &Method::Post, "/api/v1/models/hot/activate", "");
    assert_eq!(resp.status_code().0, 200);
    let body = body_to_string(resp);
    assert!(body.contains("\"requires_restart\":false"), "body = {body}");
    assert!(body.contains("\"active_id\":\"hot\""), "body = {body}");

    cleanup(&reg);
    cleanup(&assets_root);
}

/// registry 的 `active_id` 与状态栏读数同源：activate 之后状态栏必须变。
#[test]
fn a1_active_model_id_follows_the_registry() {
    let (assets_root, _) = make_minimal_package("follow");
    let reg = tmp_path("a1_follow");
    cleanup(&reg);
    let store = ModelStore::new(assets_root.clone(), reg.clone());
    let _ = dispatch(
        &store,
        &Method::Post,
        "/api/v1/models/import",
        r#"{"id":"follow"}"#,
    );
    let _ = dispatch(&store, &Method::Post, "/api/v1/models/follow/activate", "");
    assert_eq!(super::active_model_id(&store), "follow");

    cleanup(&reg);
    cleanup(&assets_root);
}
