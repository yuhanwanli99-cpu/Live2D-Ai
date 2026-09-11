//! 模型路径解析（纯函数，便于单测）。
//!
//! 与 `mod.rs` 的 `parse_args` 解耦：本模块只关心「把 `Option<&str>` 解析为
//! 真实可读的 `model3.json` 路径」，不接触 CLI flag。

use std::path::{Path, PathBuf};

use super::DEFAULT_BAI_MODEL3_RELATIVE;

/// 解析 model smoke 的 model3.json 路径（纯函数；显式输入便于单测）。
///
/// - 显式路径（`--model-smoke <path>`）：按原样使用，不存在 → 错误；
/// - 缺省：依次尝试 `cwd_base/`[`DEFAULT_BAI_MODEL3_RELATIVE`] 与
///   `manifest_dir/../../`[`DEFAULT_BAI_MODEL3_RELATIVE`]（后者对应
///   `cargo run` 时仓库内相对布局，与 `crates/l2d` 测试同口径），
///   命中第一个存在的文件；都不存在 → 错误并列出尝试过的路径。
pub fn resolve_model3_path_in(
    explicit: Option<&str>,
    cwd_base: &Path,
    manifest_dir: &Path,
) -> Result<PathBuf, String> {
    if let Some(raw) = explicit {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err("--model-smoke 路径为空".to_owned());
        }
        let path = PathBuf::from(trimmed);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!("指定的模型清单不存在: {}", path.display()));
    }

    let relative = Path::new(DEFAULT_BAI_MODEL3_RELATIVE);
    let candidates = [
        cwd_base.join(relative),
        manifest_dir.join("../../").join(relative),
    ];
    for candidate in &candidates {
        if candidate.is_file() {
            return Ok(candidate.clone());
        }
    }
    let tried = candidates
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join("\n  ");
    Err(format!(
        "未找到缺省 Bai 皮套清单（{}），尝试过:\n  {tried}\n\
         请用 --model-smoke <model3.json> 显式指定路径",
        DEFAULT_BAI_MODEL3_RELATIVE
    ))
}

/// [`resolve_model3_path_in`] 的进程环境版（cwd + 本 crate 编译期 manifest 目录）。
pub fn resolve_model3_path(explicit: Option<&str>) -> Result<PathBuf, String> {
    resolve_model3_path_in(
        explicit,
        Path::new("."),
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
}
