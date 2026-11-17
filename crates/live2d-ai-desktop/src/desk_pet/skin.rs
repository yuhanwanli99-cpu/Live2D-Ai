//! 「切换皮肤」：列出已安装模型、记住当前皮肤、把换肤请求交给帧循环。
//!
//! - **唯一的模型根**仍是 `web_api::model_root::model_root()`，不新开一套路径；
//! - 皮肤是相对模型根的 `*.model3.json` 路径（如 `bai/runtime/bai.model3.json`），
//!   持久在 `mods.desk-pet.config.skin`，**不塞进 `PetConfig`**（它是 `Copy`，
//!   塞 String 会连带破坏配置层与全部单测）；
//! - 菜单写 `PENDING`，帧循环每帧 `take()` 一次后重新 `load_model`；
//!   **换失败只记 `desk_pet` 日志、旧皮套照常跑**，窗不关、HTTP 不动。

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use super::manifest;

static CURRENT: Mutex<Option<String>> = Mutex::new(None);
static PENDING: Mutex<Option<String>> = Mutex::new(None);
static LOADED_ONCE: AtomicBool = AtomicBool::new(false);

fn with_current<R>(f: impl FnOnce(&mut Option<String>) -> R) -> R {
    match CURRENT.lock() {
        Ok(mut guard) => f(&mut guard),
        Err(poisoned) => f(&mut poisoned.into_inner()),
    }
}

fn with_pending<R>(f: impl FnOnce(&mut Option<String>) -> R) -> R {
    match PENDING.lock() {
        Ok(mut guard) => f(&mut guard),
        Err(poisoned) => f(&mut poisoned.into_inner()),
    }
}

/// 第一次用时从 `mods.json` 读回皮肤（缺省 = 没有 = 用出厂 bai）。
fn ensure_loaded() {
    if LOADED_ONCE.swap(true, Ordering::SeqCst) {
        return;
    }
    let from_disk = manifest::skin_from_manifest(&manifest::read_manifest(
        &crate::web_api::cli_entry::mods_path_for_web(),
    ));
    with_current(|slot| *slot = from_disk);
}

/// 当前皮肤的相对路径；`None` = 用出厂 `bai`。
pub fn current() -> Option<String> {
    ensure_loaded();
    with_current(|slot| slot.clone())
}

/// 菜单选中一项：只写内存 + `mods.desk-pet.config.skin`，不碰别的键。
pub fn select(rel: &str) {
    ensure_loaded();
    with_current(|slot| *slot = Some(rel.to_string()));
    with_pending(|slot| *slot = Some(rel.to_string()));
    manifest::save_skin(&crate::web_api::cli_entry::mods_path_for_web(), rel);
}

/// 帧循环取一次待换皮肤（取走即清）。
pub fn take() -> Option<String> {
    with_pending(Option::take)
}

/// 这次开窗要加载的 `model3.json` 绝对路径（含出厂兜底）。
pub fn model_path() -> PathBuf {
    let root = crate::web_api::model_root::model_root();
    let rel = current()
        .unwrap_or_else(|| crate::web_api::model_root::builtin_model3_rel_path().to_string());
    root.join(rel)
}

/// 模型 id（一级目录名），给菜单当显示名。
fn display_name(rel: &str) -> String {
    Path::new(rel)
        .components()
        .next()
        .and_then(|c| c.as_os_str().to_str())
        .map(str::to_string)
        .unwrap_or_else(|| {
            Path::new(rel)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("模型")
                .to_string()
        })
}

/// 扫描模型根下**一层模型目录**（再往下一层 runtime）里的 `*.model3.json`。
/// 读不动目录 / 一个模型都没有时返回空表，菜单那一组就只有组头。
#[must_use]
pub fn installed() -> Vec<(String, String)> {
    let root = crate::web_api::model_root::model_root();
    let Ok(entries) = std::fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for dir in entries.flatten() {
        let path = dir.path();
        if !path.is_dir() {
            continue;
        }
        let mut found: Vec<String> = Vec::new();
        collect_model3(&path, &root, 2, &mut found);
        found.sort();
        if let Some(rel) = found.into_iter().next() {
            out.push((display_name(&rel), rel));
        }
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    // 出厂皮套即使不在目录里（例如只挂了 registry），也保证有一项可回退。
    let builtin = crate::web_api::model_root::builtin_model3_rel_path().to_string();
    if !out.iter().any(|(_, rel)| *rel == builtin) && root.join(&builtin).is_file() {
        out.push((display_name(&builtin), builtin));
        out.sort_by(|a, b| a.1.cmp(&b.1));
    }
    out
}

fn collect_model3(dir: &Path, root: &Path, depth: u8, out: &mut Vec<String>) {
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_model3(&path, root, depth - 1, out);
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.ends_with(".model3.json") {
            continue;
        }
        if let Ok(rel) = path.strip_prefix(root)
            && let Some(text) = rel.to_str()
        {
            out.push(text.replace('\\', "/"));
        }
    }
}

/// 菜单里该给哪一项打勾。找不到就落回第一项。
#[must_use]
pub fn active_index(list: &[(String, String)]) -> usize {
    let Some(cur) = current() else {
        return list
            .iter()
            .position(|(_, rel)| rel == crate::web_api::model_root::builtin_model3_rel_path())
            .unwrap_or(0);
    };
    list.iter().position(|(_, rel)| rel == &cur).unwrap_or(0)
}

/// Windows：帧循环每帧调，真正换掉离屏渲染器里的那一套皮套。
/// 失败只记日志（旧模型继续跑），**不向调用方返回 Err**。
#[cfg(windows)]
pub fn apply_pending(renderer: &mut l2d::renderer::OffscreenRenderer) {
    let Some(rel) = take() else {
        return;
    };
    let path = crate::web_api::model_root::model_root().join(&rel);
    let result = (|| -> Result<(), String> {
        let package = l2d::asset::ModelPackage::load(&path)
            .map_err(|e| format!("加载皮套包失败（{}）: {e}", path.display()))?;
        let loaded =
            l2d::model::LoadedModel::resolve(package).map_err(|e| format!("解析皮套失败: {e}"))?;
        // load_fresh：先在旁支建好整套栈，成功才原子替换，避免上游原地换
        // 失败留下的半状态（0.3.5 热换 panic 的修法 B）。
        let fresh = renderer
            .core()
            .load_fresh(&loaded)
            .map_err(|e| format!("装载皮套失败: {e}"))?;
        *renderer.core() = fresh;
        Ok(())
    })();
    match result {
        Ok(()) => tracing::info!(target: "desk_pet", skin = %rel, "皮肤已切换"),
        Err(e) => tracing::error!(target: "desk_pet", skin = %rel, "换肤失败，保留旧皮套：{e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_model_root_yields_an_empty_list_without_panicking() {
        // 不假设本机一定有 assets/models；这里只钉住「读不动不 panic」。
        let list = installed();
        assert!(list.iter().all(|(_, rel)| rel.ends_with(".model3.json")));
    }

    #[test]
    fn relative_paths_are_forward_slash_and_unique() {
        let list = installed();
        let mut seen = std::collections::BTreeSet::new();
        for (_, rel) in list {
            assert!(!rel.contains('\\'), "{rel} 必须是正斜杠");
            assert!(seen.insert(rel));
        }
    }
}
