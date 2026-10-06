//! 仓库扫描：Rust（逐 crate 三桶）/ Dart / docs / 产物体积。
//!
//! 目录排除、隐藏目录、符号链接的处理与 `rust-ratio` 同一份纪律。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::measure::{count_dependencies, inline_test_lines, physical_lines};
use super::*;

// ── 扫描 ──────────────────────────────────────────────────────────────────────

/// 扫描仓库根，产出全部体量统计。
///
/// # Errors
/// 任何目录/文件读失败（不吞错误：度量工具说谎比没有工具更坏）。
pub fn collect(root: &Path) -> io::Result<CodeStats> {
    let mut stats = CodeStats::default();

    let mut crate_dirs: Vec<(String, PathBuf)> = Vec::new();
    let crates_dir = root.join("crates");
    if crates_dir.is_dir() {
        for entry in fs::read_dir(&crates_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            crate_dirs.push((name, entry.path()));
        }
    }
    let xtask_dir = root.join("xtask");
    if xtask_dir.is_dir() {
        crate_dirs.push(("xtask".to_string(), xtask_dir));
    }
    crate_dirs.sort_by(|a, b| a.0.cmp(&b.0));

    for (name, dir) in &crate_dirs {
        let mut crate_stats = CrateStats {
            name: name.clone(),
            deps: count_dependencies(&dir.join("Cargo.toml")),
            ..CrateStats::default()
        };
        for file in collect_files(dir, "rs")? {
            let text = fs::read_to_string(&file)?;
            let total = physical_lines(&text);
            let inline = if text.is_empty() {
                0
            } else {
                inline_test_lines(&text)
            };
            let rel_inside = file.strip_prefix(dir).unwrap_or(&file);
            let kind = rust_kind(rel_inside);
            // 超限门禁的对象是 `crates/*/src` 的**全部** `.rs`（含只被 cfg(test) 引用的
            // 测试文件与块）——与 PLAN §2.4 的 58 / 4 同口径；xtask 不在门禁内。
            if name != "xtask" && kind != RustKind::OtherIntegration {
                let entry = FileLines {
                    rel: relative(root, &file),
                    total,
                    // `tests/` 目录 / `tests.rs` 文件整份只被 `#[cfg(test)]` 引用：
                    // 内联列记满，生产列因此为 0（不让测试文件冒充生产）。
                    inline_test: if kind == RustKind::SrcIntegration {
                        total
                    } else {
                        inline
                    },
                };
                if total > 500 {
                    stats.src_over_500.push(entry.clone());
                }
                if total > 1000 {
                    stats.src_over_1000.push(entry.clone());
                }
            }
            match kind {
                RustKind::Src => {
                    crate_stats.prod += total - inline;
                    crate_stats.inline_test += inline;
                    stats.rust_src_files += 1;
                    if inline > 0 {
                        stats.rust_inline_test_files += 1;
                    }
                    if name != "xtask" && is_named_test_module(rel_inside) {
                        stats.test_module_files.push(FileLines {
                            rel: relative(root, &file),
                            total,
                            inline_test: inline,
                        });
                    }
                }
                RustKind::SrcIntegration | RustKind::OtherIntegration => {
                    crate_stats.integ += total;
                    stats.rust_integ_files += 1;
                }
            }
        }
        stats.rust_prod += crate_stats.prod;
        stats.rust_inline_test += crate_stats.inline_test;
        stats.rust_integ += crate_stats.integ;
        stats.crates.push(crate_stats);
    }

    stats
        .src_over_500
        .sort_by(|a, b| b.total.cmp(&a.total).then(a.rel.cmp(&b.rel)));
    stats
        .src_over_1000
        .sort_by(|a, b| b.total.cmp(&a.total).then(a.rel.cmp(&b.rel)));
    stats
        .test_module_files
        .sort_by(|a, b| b.total.cmp(&a.total).then(a.rel.cmp(&b.rel)));

    let lib = collect_dart(root, &root.join("shell/flutter/lib"))?;
    stats.dart_lib_files = lib.files;
    stats.dart_lib_lines = lib.lines;
    stats.dart_lib_over_800 = lib.over;
    let test = collect_dart(root, &root.join("shell/flutter/test"))?;
    stats.dart_test_files = test.files;
    stats.dart_test_lines = test.lines;
    stats.dart_test_over_800 = test.over;

    // docs：总量 + 「不含 docs/audit/**」的减量账（判据对象）。一次扫描两数，避免漂移。
    let docs = collect_files(&root.join("docs"), "md")?;
    stats.docs_files = docs.len() as u64;
    let mut docs_total = 0_u64;
    let mut audit_files = 0_u64;
    let mut audit_total = 0_u64;
    for path in &docs {
        let lines = physical_lines(&fs::read_to_string(path)?);
        docs_total += lines;
        let rel = path.strip_prefix(root).unwrap_or(path.as_path());
        if rel.starts_with("docs/audit/") {
            audit_files += 1;
            audit_total += lines;
        }
    }
    stats.docs_lines = docs_total;
    stats.docs_audit_files = audit_files;
    stats.docs_audit_lines = audit_total;
    stats.docs_worktree_files = stats.docs_files - audit_files;
    stats.docs_worktree_lines = docs_total - audit_total;

    for (rel, budget) in [
        (FLUTTER_WEB_DIST, FLUTTER_WEB_BUDGET_MIB),
        (WASM_DIST, WASM_DIST_BUDGET_MIB),
    ] {
        stats.artifacts.push(Artifact {
            rel: rel.to_string(),
            bytes: dir_size(&root.join(rel))?,
            budget_mib: budget,
        });
    }

    Ok(stats)
}

/// Rust 文件的归属分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustKind {
    /// `src/` 内的生产文件（可能含内联 `#[cfg(test)]` 块）。
    Src,
    /// `src/` 内只被 `#[cfg(test)] mod x;` 引用的测试文件（`tests/` 目录或 `tests.rs`）。
    SrcIntegration,
    /// `crates/*/tests/**`、`crates/*/examples/**` 等 src 之外的文件。
    OtherIntegration,
}

/// 判断（crate 目录内的）相对路径属于哪一类。
pub fn rust_kind(rel_inside_crate: &Path) -> RustKind {
    let parts: Vec<&str> = rel_inside_crate
        .iter()
        .filter_map(|part| part.to_str())
        .collect();
    let Some((file, dirs)) = parts.split_last() else {
        return RustKind::OtherIntegration;
    };
    if !dirs.contains(&"src") {
        return RustKind::OtherIntegration;
    }
    if dirs.contains(&"tests") || *file == "tests.rs" {
        RustKind::SrcIntegration
    } else {
        RustKind::Src
    }
}

/// 文件名像测试模块吗（`tests_x.rs` / `x_tests.rs`）——披露用，不参与门禁。
pub fn is_named_test_module(rel_inside_crate: &Path) -> bool {
    match rel_inside_crate.file_name().and_then(|name| name.to_str()) {
        Some(name) => name != "tests.rs" && name.contains("test"),
        None => false,
    }
}

/// 递归收集 `dir` 下指定扩展名的文件（跳过排除目录 / 隐藏目录 / 符号链接）。
pub fn collect_files(dir: &Path, extension: &str) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in fs::read_dir(&current)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if file_type.is_dir() {
                if name.starts_with('.') || SKIPPED_DIR_NAMES.contains(&name.as_str()) {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let matches = path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case(extension));
            if matches {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// 一次 Dart 目录扫描的结果。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DartStats {
    pub files: u64,
    pub lines: u64,
    /// > 800 行的文件（降序）。
    pub over: Vec<(String, u64)>,
}

/// Dart 目录统计：文件数、行数、> 800 行的文件（降序）。
fn collect_dart(root: &Path, dir: &Path) -> io::Result<DartStats> {
    let files = collect_files(dir, "dart")?;
    let mut lines_total = 0_u64;
    let mut over = Vec::new();
    for file in &files {
        let lines = physical_lines(&fs::read_to_string(file)?);
        lines_total += lines;
        if lines > 800 {
            over.push((relative(root, file), lines));
        }
    }
    over.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    Ok(DartStats {
        files: files.len() as u64,
        lines: lines_total,
        over,
    })
}

/// 目录体积：常规文件字节数之和；目录不存在返回 `None`。
pub fn dir_size(dir: &Path) -> io::Result<Option<u64>> {
    if !dir.is_dir() {
        return Ok(None);
    }
    let mut total = 0_u64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in fs::read_dir(&current)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                stack.push(entry.path());
            } else if file_type.is_file() {
                total += entry.metadata()?.len();
            }
        }
    }
    Ok(Some(total))
}

/// 展示路径：仓库内相对路径统一用 `/`；不在仓库内则用原路径。
fn relative(root: &Path, path: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
        Err(_) => display_path(path),
    }
}
