//! `code-stats` — D0 体量度量与硬门禁。
//!
//! 真源：`docs/plans/PLAN-debloat-and-closeout-2026-10-01.md` §2（体量实测基线）
//! 与 §5 D0（度量与门禁）。本子命令的职责只有两条：
//!
//! 1. **一把尺子**：把 §2 的数字变成一条可复算的命令（`cargo run -p xtask -- code-stats`），
//!    输出可直接粘进 release note 的 markdown；
//! 2. **一道棘轮**：`--check` 在任一硬门禁超限时退出码非 0，供 CI 拦「新增超限」。
//!
//! # 口径（可复核，勿在别处另立）
//!
//! - **行数**＝物理行数（`\n` 计数；末尾无换行的残行计 1）＝ `wc -l` 语义，可逐项复核；
//! - **Rust 范围**＝`crates/*/**/*.rs` + `xtask/src/**/*.rs`；排除 `target/ build/ dist/
//!   node_modules/ .venv/`（任意层级）与隐藏目录，不跟随符号链接；
//! - **生产**＝`src/` 内**不**属于 `#[cfg(test)]` 花括号配对条目的行；
//! - **内联测试**＝上述 `#[cfg(test)]` 条目的行（含属性行）；
//! - **集成测试 / 示例**＝`crates/*/tests/**`、`crates/*/examples/**`，以及 `src/` 内
//!   位于 `tests/` 目录或以 `tests.rs` 命名的文件（这些文件只被 `#[cfg(test)] mod x;` 引用）；
//! - **超限门禁按文件总行数（含内联测试）计**——与 §2.4 的 `>500 = 58` / `>1000 = 4` 一致
//!   （AGENTS.md「源码 ≤500 行」约束的是文件本身，不是文件的「生产段」）；
//! - **Dart**＝`shell/flutter/lib/**/*.dart` 与 `shell/flutter/test/**/*.dart`（不含 `build/`、
//!   `.dart_tool/`）；
//! - **docs**＝`docs/**/*.md`（含 `docs/legacy/`）；
//! - **依赖数**＝每个 crate `Cargo.toml` 顶层 `[dependencies]` 的键数——**不含**
//!   `[target.*.dependencies]` / `[dev-dependencies]` / `[build-dependencies]`
//!   （与 §2.5 的 `desktop 34 · runtime 12 · l2d 8` 对齐）；
//! - **产物体积**＝目录内常规文件字节数之和；目录不存在时记「缺」（不猜、不跳过）。
//!
//! # 已知边界（诚实栏）
//!
//! - `#[cfg(test)]` 条目边界用**花括号计数**判定，不解析字符串 / 注释里的 `{` `}`：
//!   对现有代码给出与 §2.4 一致的结论（4 个 >1000、58 个 >500），但对「测试块之后
//!   还有生产条目」的文件（如 `runtime/src/settings.rs`）会把块后内容算作生产；
//! - 仅被 `#[cfg(test)] mod x;` 引用的兄弟测试模块文件（`tests_*.rs` / `*_tests.rs`）
//!   物理上住在 `src/`，本工具按**物理位置**把它们计入「生产」，并在报告里单列这一桶
//!   （`--verbose` 打印清单），供严格口径读者扣除；
//! - 计划 §2.1 的手工脚本未落盘，其「内联测试 16,231」与本工具的块精确口径存在差
//!   （见报告 §5 对账），本工具**不改数字去凑**。
//!
//! 许可：**AGPL-3.0-only**，以仓库根 `LICENSE` 为准。

use std::path::Path;
use std::process::ExitCode;

use cli::parse_args;
use collect::collect;
use gates::{evaluate, exit_code};
use report::report;

#[cfg(test)]
mod tests;

/// 按目录名排除（任意层级）：构建产物 / 托管环境。与 `rust-ratio` 同一份纪律。
const SKIPPED_DIR_NAMES: [&str; 5] = ["target", "build", "dist", "node_modules", ".venv"];

/// **棘轮**默认阈值 = 立档时的现状（PLAN §2.1 / §2.4 / §2.5）。
///
/// 理由（用户 2026-10-01 口径 + D0 任务书）：门禁先按现状设上限，
/// **既不放宽，也不设成当前必红**——它拦的是「新增超限」，不是存量。
/// PLAN §5 的目标值另存 [`Limits::plan`]，用 `--strict-plan` 复核（现在仍是红的）。
const RATCHET_SRC_RS_500: u64 = 58;
const RATCHET_SRC_RS_1000: u64 = 4;
const RATCHET_DART_800: u64 = 7;
const RATCHET_DESKTOP_DEPS: u64 = 34;

/// PLAN §5 量化目标（`--strict-plan`）。
const PLAN_SRC_RS_500: u64 = 15;
const PLAN_SRC_RS_1000: u64 = 0;
const PLAN_DART_800: u64 = 2;
const PLAN_DESKTOP_DEPS: u64 = 22;

/// 依赖预算点名的 crate（PLAN §2.5 / §D4）。
const DESKTOP_CRATE: &str = "live2d-ai-desktop";

/// Flutter Web 产物目录（相对仓库根）。
const FLUTTER_WEB_DIST: &str = "shell/flutter/build/web";
/// Flutter Web 产物预算（PLAN §D4：47M → ≤35M）。
const FLUTTER_WEB_BUDGET_MIB: u64 = 35;
/// wasm 渲染面产物目录（相对仓库根）。
const WASM_DIST: &str = "crates/l2d-wasm-demo/dist";
/// wasm 产物预算（PLAN §D4：5.4M → ≤4M）。
const WASM_DIST_BUDGET_MIB: u64 = 4;

// ── 文件行数（`wc -l` 语义）────────────────────────────────────────────────────

/// 单个文件的可见统计：总行数 + 其中被 `#[cfg(test)]` 条目占掉的行数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileLines {
    /// 相对仓库根的路径（展示用，永远用 `/` 分隔）。
    pub rel: String,
    pub total: u64,
    pub inline_test: u64,
}

impl FileLines {
    /// 生产行数（总行数减内联测试段）。
    pub fn prod(&self) -> u64 {
        self.total.saturating_sub(self.inline_test)
    }
}

/// 一次 `code-stats` 扫描的全部结果。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CodeStats {
    /// 逐 crate（含 `xtask`，按名字排序）。
    pub crates: Vec<CrateStats>,
    /// `crates/*/src` 总行数 > 500 的文件（降序）。
    pub src_over_500: Vec<FileLines>,
    /// `crates/*/src` 总行数 > 1000 的文件（降序）。
    pub src_over_1000: Vec<FileLines>,
    /// 仅被 `#[cfg(test)] mod x;` 引用的 `src/` 测试模块文件（披露用）。
    pub test_module_files: Vec<FileLines>,
    /// `shell/flutter/lib` 的 dart 文件数与行数。
    pub dart_lib_files: u64,
    pub dart_lib_lines: u64,
    /// `shell/flutter/test` 的 dart 文件数与行数。
    pub dart_test_files: u64,
    pub dart_test_lines: u64,
    /// `shell/flutter/lib` 内 > 800 行的 dart 文件（门禁对象，降序）。
    pub dart_lib_over_800: Vec<(String, u64)>,
    /// `shell/flutter/test` 内 > 800 行的 dart 文件（仅披露，降序）。
    pub dart_test_over_800: Vec<(String, u64)>,
    /// `docs/**/*.md`。
    pub docs_files: u64,
    pub docs_lines: u64,
    /// 产物体积（缺失即 `None`）。
    pub artifacts: Vec<Artifact>,
    /// Rust 三桶合计（冗余保存，便于报告与断言）。
    pub rust_prod: u64,
    pub rust_inline_test: u64,
    pub rust_integ: u64,
    /// 文件数：生产桶（`src/` 内非测试模块文件）。
    pub rust_src_files: u64,
    /// 文件数：含 `#[cfg(test)]` 条目的文件。
    pub rust_inline_test_files: u64,
    /// 文件数：集成测试 / 示例桶。
    pub rust_integ_files: u64,
}

/// 逐 crate 的 Rust 统计 + Cargo 依赖数。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CrateStats {
    pub name: String,
    pub prod: u64,
    pub inline_test: u64,
    pub integ: u64,
    pub deps: u64,
}

impl CrateStats {
    fn total(&self) -> u64 {
        self.prod + self.inline_test + self.integ
    }
}

/// 产物体积。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    pub rel: String,
    /// `None` = 目录不存在（报告里写「缺」）。
    pub bytes: Option<u64>,
    /// PLAN 预算（MiB）。
    pub budget_mib: u64,
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

mod cli;
mod collect;
mod gates;
mod measure;
mod report;

pub fn run(args: &[String]) -> Result<ExitCode, String> {
    let opts = parse_args(args)?;
    let root = crate::locate_repo_root()
        .ok_or("未能定位仓库根（含 [workspace] 的 Cargo.toml 所在目录）")?;
    let stats = collect(&root).map_err(|err| format!("扫描 {} 失败：{err}", root.display()))?;
    print!("{}", report(&root, &stats, &opts));
    let results = evaluate(&stats, &opts);
    Ok(exit_code(&results, opts.check))
}

/// 供 `print_help` 使用的一行用法。
pub const USAGE: &str = "code-stats [--check] [--only <lines|over-1000|deps>]... [--strict-plan] [--quiet] [--verbose] [--max-* <n>]";
