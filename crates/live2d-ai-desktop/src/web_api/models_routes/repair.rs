//! registry 载入期修复（0.5.1 rc.1）：历史遗留的**绝对 / verbatim**
//! `model3_rel_path` 重算成相对模型根。
//!
//! 0.5.1 之前 Windows 上 `canonicalize` 的 verbatim 路径（`\\?\D:\...`）被原样
//! 写进 registry，拼出的 URL 是 `/models///?/D:/...` → 404 → 换模静默回退 bai
//!（2026-11-09 真机实测）。新写入已由 [`super::util::model3_rel_path_from_abs`]
//! 修掉；这里兜住**旧 registry**：按 `relative_dir` 重新找 model3 并重算相对路径。
//!
//! 只改内存 + warn：不动模型文件、不在启动路径写盘（下一次 persist 自然落好）。

use std::path::Path;

use super::registry::{ModelEntry, ModelRegistry};
use super::util::{find_model3_json, model3_rel_path_from_abs, normalize_path_text};

/// 修复所有 entry 的 model3_rel_path；返回修复条数（供测试与日志）。
pub(super) fn repair_legacy_model3_paths(reg: &mut ModelRegistry, assets_root: &Path) -> usize {
    let mut repaired = 0usize;
    for entry in reg.entries.values_mut() {
        if !is_absolute_or_verbatim(&entry.model3_rel_path) {
            continue;
        }
        repair_one(entry, assets_root, &mut repaired);
    }
    repaired
}

fn repair_one(entry: &mut ModelEntry, assets_root: &Path, repaired: &mut usize) {
    let dir = assets_root.join(&entry.relative_dir);
    let Some(model3) = find_model3_json(&dir) else {
        eprintln!(
            "[warn] registry entry {:?} 的 model3_rel_path 是绝对 / verbatim 路径，但 {} 下找不到 model3；保持原值",
            entry.id,
            dir.display()
        );
        return;
    };
    match model3_rel_path_from_abs(assets_root, &model3) {
        Ok(rel) => {
            eprintln!(
                "[warn] 修复 registry entry {:?} 的 model3_rel_path：{:?} → {rel:?}（历史绝对 / verbatim 路径）",
                entry.id, entry.model3_rel_path
            );
            entry.model3_rel_path = rel;
            *repaired += 1;
        }
        Err(e) => eprintln!(
            "[warn] registry entry {:?} 的 model3_rel_path 重算失败（{e}）；保持原值",
            entry.id
        ),
    }
}

/// 是否是绝对 / 相对非法形态（盘符、前导 `/`，含 verbatim / UNC 前缀）。
fn is_absolute_or_verbatim(rel: &str) -> bool {
    let text = normalize_path_text(Path::new(rel));
    if text.starts_with('/') {
        return true;
    }
    let bytes = text.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}
