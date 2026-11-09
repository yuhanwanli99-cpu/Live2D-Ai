//! 出厂模型 seed（0.5.1 rc.1）：bai + `assets/models/seed.local.json`。
//!
//! # 为什么通用化
//!
//! 0.3.5 以来「出厂模型」= 硬编码 id 的 bai。真实用户场景里还有**本地合法
//! 持有、作者禁止再分发**的皮套（例如作者免费配布的第三方皮肤）：模型文件与
//! 声明文件都不能入库，但用户希望它「和 bai 一样」——列表里出厂自带、有徽章、
//! 不能误删。0.5.1 起把这条机制做成数据驱动：
//!
//! - bai 仍由编译期常量 seed（文件不在则跳过）；
//! - 额外读模型根下的 `seed.local.json`（**gitignore**），格式
//!   `[{"id":"目录名","display_name":"大肥鱼"}]`；
//! - 每个条目先按 id 规范化 + 目录存在 + 找 *.model3.json + `ModelPackage::load`
//!   校验，全部通过才写入 registry（`builtin = true`）；
//! - 文件缺失 / 坏 JSON / 单条无效都只 warn，绝不阻断启动；
//! - 模型文件与 `seed.local.json` 都不入库、不随包分发（作者条约禁止重新分发）。
//!
//! 只改内存、不写盘（与既有 seed 纪律一致：启动路径不制造运行时文件）。

use std::path::Path;

use super::registry::{
    ModelDisplay, ModelEntry, ModelRegistry, ModelSource, StoredManifest, default_stage,
    normalize_relative_id,
};
use super::util::{find_model3_json, model3_rel_path_from_abs, now_iso8601};
use crate::web_api::model_root;

/// 本地出厂模型声明文件（相对模型根；`assets/models/seed.local.json`）。
///
/// 已在根 `.gitignore`：模型与声明都只属于用户本机，入库 / 随包分发都禁止。
pub(super) const SEED_LOCAL_FILE: &str = "seed.local.json";

/// `seed.local.json` 的单条声明。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SeedLocalEntry {
    id: String,
    #[serde(default)]
    display_name: String,
}

/// 载入期：bai + 本地出厂模型 seed（纯内存）。
pub(super) fn seed_builtin_entries(reg: &mut ModelRegistry, assets_root: &Path) {
    seed_bai(reg, assets_root);
    seed_local(reg, assets_root);
}

/// bai 仍由编译期常量决定；registry 里已有 bai entry 时**补 builtin 标记**
///（0.5.1 之前的 registry 没有这个字段，否则旧文件里的 bai 会变成可删）。
fn seed_bai(reg: &mut ModelRegistry, assets_root: &Path) {
    let id = model_root::builtin_model_id();
    if id.is_empty() {
        return;
    }
    if let Some(entry) = reg.entries.get_mut(id) {
        entry.builtin = true;
        return;
    }
    let rel = model_root::builtin_model3_rel_path();
    let model3_path = assets_root.join(rel);
    if !model3_path.is_file() {
        return;
    }
    let package = match l2d::asset::ModelPackage::load(&model3_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "[warn] 出厂模型 {} 解析失败（{e}）；模型库不自动收录它",
                model3_path.display()
            );
            return;
        }
    };
    reg.entries.insert(
        id.to_string(),
        ModelEntry {
            id: id.to_string(),
            // 0.5.1：出厂模型的展示名也走数据（前端不再硬编码 bai）。
            display_name: "白模（bai）".to_string(),
            source: ModelSource::LocalImport,
            relative_dir: id.to_string(),
            model3_rel_path: rel.to_string(),
            manifest: StoredManifest::from(package.manifest()),
            display: ModelDisplay::default(),
            stage: default_stage(),
            imported_at: now_iso8601(),
            builtin: true,
        },
    );
}

fn seed_local(reg: &mut ModelRegistry, assets_root: &Path) {
    let path = assets_root.join(SEED_LOCAL_FILE);
    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => {
            eprintln!(
                "[warn] 读取 {} 失败（{e}）；跳过本地出厂模型 seed",
                path.display()
            );
            return;
        }
    };
    let list: Vec<SeedLocalEntry> = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "[warn] {} JSON 解析失败（{e}）；跳过本地出厂模型 seed",
                path.display()
            );
            return;
        }
    };
    for item in list {
        if let Err(reason) = seed_one(reg, assets_root, &item) {
            eprintln!(
                "[warn] seed.local.json 条目 {:?} 无效（{reason}）；跳过",
                item.id
            );
        }
    }
}

/// 校验并写入一条本地出厂模型；任何一步失败都返回可读原因（调用方 warn）。
fn seed_one(
    reg: &mut ModelRegistry,
    assets_root: &Path,
    item: &SeedLocalEntry,
) -> Result<(), String> {
    let id = normalize_relative_id(&item.id)?;
    // bai 已由 seed_bai 处理：这里只补 builtin，避免按通用布局重复校验
    //（bai 的 model3 在 `bai/runtime/` 子目录，通用查找也能找到，但展示名与
    // 回退 id 都以编译期常量为准）。
    if id == model_root::builtin_model_id() {
        match reg.entries.get_mut(&id) {
            Some(entry) => entry.builtin = true,
            None => return Err("bai 的 model3 不在盘上（seed_bai 已跳过）".to_string()),
        }
        return Ok(());
    }
    let dir = assets_root.join(&id);
    if !dir.is_dir() {
        return Err(format!("目录不存在: {}", dir.display()));
    }
    let model3 = find_model3_json(&dir)
        .ok_or_else(|| format!("{} 下找不到 *.model3.json", dir.display()))?;
    let package = l2d::asset::ModelPackage::load(&model3)
        .map_err(|e| format!("ModelPackage::load 失败: {e}"))?;
    let model3_rel_path = model3_rel_path_from_abs(assets_root, &model3)?;
    let display_name = item.display_name.trim();
    let display_name = if display_name.is_empty() {
        id.clone()
    } else {
        display_name.to_string()
    };
    if let Some(entry) = reg.entries.get_mut(&id) {
        // 用户先 import 过、之后又写进 seed.local.json：升级为出厂（保留用户
        // 已有的 display / stage 配置，只改 builtin 与声明里的展示名）。
        entry.builtin = true;
        if !item.display_name.trim().is_empty() {
            entry.display_name = display_name;
        }
        return Ok(());
    }
    let entry = ModelEntry {
        id: id.clone(),
        display_name,
        source: ModelSource::LocalImport,
        relative_dir: id.clone(),
        model3_rel_path,
        manifest: StoredManifest::from(package.manifest()),
        display: ModelDisplay::default(),
        stage: default_stage(),
        imported_at: now_iso8601(),
        builtin: true,
    };
    reg.entries.insert(id, entry);
    Ok(())
}
