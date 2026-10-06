//! markdown 报告：可直接粘进 release note 的体量快照与门禁判定。

use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::EXIT_PASS;

use super::cli::Options;
use super::gates::{Gate, evaluate, exit_code};
use super::*;

// ── 报告 ──────────────────────────────────────────────────────────────────────

/// 报告：可直接粘进 release note 的 markdown。
pub fn report(root: &Path, stats: &CodeStats, opts: &Options) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "<!-- code-stats：`cargo run -p xtask -- code-stats` 生成；口径见文末 §5 -->"
    );
    let _ = writeln!(out, "## 代码体量与门禁快照（D0）");
    let _ = writeln!(out);
    let _ = writeln!(out, "- 仓库根：`{}`", display_path(root));
    let _ = writeln!(out, "- 生成时间（UTC）：{}", now_utc_string());
    let _ = writeln!(
        out,
        "- 口径真源：`docs/plans/PLAN-debloat-and-closeout-2026-10-01.md` §2 / §5 D0"
    );
    let _ = writeln!(
        out,
        "- 行数口径：物理行数（`wc -l` 语义，末尾残行计 1）；超限门禁按文件总行数（含内联测试）"
    );
    let _ = writeln!(out);

    if !opts.quiet {
        write_totals(&mut out, stats);
        write_crates(&mut out, stats);
        write_over_limit(&mut out, stats);
        write_artifacts(&mut out, stats);
    }
    write_gates(&mut out, stats, opts);
    if !opts.quiet {
        write_methodology(&mut out, stats, opts);
    }
    out
}

fn write_totals(out: &mut String, stats: &CodeStats) {
    let _ = writeln!(out, "### 1. 总量");
    let _ = writeln!(out);
    let _ = writeln!(out, "| 类别 | 文件数 | 行数 |");
    let _ = writeln!(out, "|---|---:|---:|");
    let _ = writeln!(
        out,
        "| Rust 生产（不含内联 `#[cfg(test)]`） | {} | {} |",
        stats.rust_src_files,
        thousands(stats.rust_prod)
    );
    let _ = writeln!(
        out,
        "| Rust 内联测试（`#[cfg(test)]` 条目） | {}（含该块的文件） | {} |",
        stats.rust_inline_test_files,
        thousands(stats.rust_inline_test)
    );
    let _ = writeln!(
        out,
        "| Rust 集成测试 / 示例 | {} | {} |",
        stats.rust_integ_files,
        thousands(stats.rust_integ)
    );
    let _ = writeln!(
        out,
        "| Rust 合计 | — | {} |",
        thousands(stats.rust_prod + stats.rust_inline_test + stats.rust_integ)
    );
    let _ = writeln!(
        out,
        "| Dart `lib` | {} | {} |",
        stats.dart_lib_files,
        thousands(stats.dart_lib_lines)
    );
    let _ = writeln!(
        out,
        "| Dart `test` | {} | {} |",
        stats.dart_test_files,
        thousands(stats.dart_test_lines)
    );
    let _ = writeln!(
        out,
        "| docs `*.md`（含 `docs/legacy/`） | {} | {} |",
        stats.docs_files,
        thousands(stats.docs_lines)
    );
    let _ = writeln!(out);
}

fn write_crates(out: &mut String, stats: &CodeStats) {
    let _ = writeln!(out, "### 2. 逐 crate（Rust + Cargo 依赖数）");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "| crate | 生产 | 内联测试 | 集成/示例 | 合计 | `[dependencies]` |"
    );
    let _ = writeln!(out, "|---|---:|---:|---:|---:|---:|");
    for crate_stats in &stats.crates {
        let _ = writeln!(
            out,
            "| `{name}` | {prod} | {inline} | {integ} | {total} | {deps} |",
            name = crate_stats.name,
            prod = thousands(crate_stats.prod),
            inline = thousands(crate_stats.inline_test),
            integ = thousands(crate_stats.integ),
            total = thousands(crate_stats.total()),
            deps = crate_stats.deps,
        );
    }
    let _ = writeln!(
        out,
        "| **合计** | **{}** | **{}** | **{}** | **{}** | — |",
        thousands(stats.rust_prod),
        thousands(stats.rust_inline_test),
        thousands(stats.rust_integ),
        thousands(stats.rust_prod + stats.rust_inline_test + stats.rust_integ)
    );
    let _ = writeln!(out);
}

fn write_over_limit(out: &mut String, stats: &CodeStats) {
    let _ = writeln!(out, "### 3. 超限清单");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "#### 3.1 `crates/*/src` 生产 `.rs` > 1000 行（{} 个，PLAN 目标 0）",
        stats.src_over_1000.len()
    );
    let _ = writeln!(out);
    if stats.src_over_1000.is_empty() {
        let _ = writeln!(out, "（无）");
    } else {
        let _ = writeln!(out, "| 文件 | 总行数 | 其中内联测试 | 生产 |");
        let _ = writeln!(out, "|---|---:|---:|---:|");
        for file in &stats.src_over_1000 {
            let _ = writeln!(
                out,
                "| `{}` | {} | {} | {} |",
                file.rel,
                file.total,
                file.inline_test,
                file.prod()
            );
        }
    }
    let _ = writeln!(out);

    let _ = writeln!(
        out,
        "#### 3.2 `crates/*/src` 生产 `.rs` > 500 行（{} 个，PLAN 目标 ≤15）",
        stats.src_over_500.len()
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "| 文件 | 总行数 | 其中内联测试 | 生产 |");
    let _ = writeln!(out, "|---|---:|---:|---:|");
    for file in &stats.src_over_500 {
        let _ = writeln!(
            out,
            "| `{}` | {} | {} | {} |",
            file.rel,
            file.total,
            file.inline_test,
            file.prod()
        );
    }
    let _ = writeln!(out);

    let _ = writeln!(
        out,
        "#### 3.3 Dart `lib` > 800 行（{} 个，PLAN 目标 ≤2）",
        stats.dart_lib_over_800.len()
    );
    let _ = writeln!(out);
    if stats.dart_lib_over_800.is_empty() {
        let _ = writeln!(out, "（无）");
    } else {
        let _ = writeln!(out, "| 文件 | 行数 |");
        let _ = writeln!(out, "|---|---:|");
        for (path, lines) in &stats.dart_lib_over_800 {
            let _ = writeln!(out, "| `{path}` | {lines} |");
        }
    }
    let _ = writeln!(out);

    if !stats.dart_test_over_800.is_empty() {
        let _ = writeln!(
            out,
            "#### 3.4 Dart `test` > 800 行（{} 个，仅披露；D6 治理对象）",
            stats.dart_test_over_800.len()
        );
        let _ = writeln!(out);
        let _ = writeln!(out, "| 文件 | 行数 |");
        let _ = writeln!(out, "|---|---:|");
        for (path, lines) in &stats.dart_test_over_800 {
            let _ = writeln!(out, "| `{path}` | {lines} |");
        }
        let _ = writeln!(out);
    }
}

fn write_artifacts(out: &mut String, stats: &CodeStats) {
    let _ = writeln!(out, "### 4. 产物体积（PLAN §2.5 / §D4 预算）");
    let _ = writeln!(out);
    let _ = writeln!(out, "| 产物 | 大小 | 预算 | 状态 |");
    let _ = writeln!(out, "|---|---:|---:|---|");
    for artifact in &stats.artifacts {
        match artifact.bytes {
            Some(bytes) => {
                let budget = artifact.budget_mib * 1024 * 1024;
                let status = if bytes <= budget {
                    "在预算内"
                } else {
                    "超预算"
                };
                let _ = writeln!(
                    out,
                    "| `{}` | {}（{} B） | ≤ {} MiB | {} |",
                    artifact.rel,
                    human_size(bytes),
                    bytes,
                    artifact.budget_mib,
                    status
                );
            }
            None => {
                let _ = writeln!(
                    out,
                    "| `{}` | 缺 | ≤ {} MiB | 未构建 |",
                    artifact.rel, artifact.budget_mib
                );
            }
        }
    }
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "预算口径（不要混读）：`FLUTTER_WEB_BUDGET_MIB` 仍是 PLAN §D4 目标（47M → ≤35M）；\
         `WASM_DIST_BUDGET_MIB` 已按 2026-10-06 实测重定为 **4.2 MiB**（**当前接受值**，\
         不是 PLAN 目标：本机无 `wasm-opt` ⇒ 真减未实测、不估数）。\
         上表「超预算」是如实打印，`--strict-plan` 也不会因此变绿；\
         真源 `docs/architecture/artifact-budget.md`。"
    );
}

fn write_gates(out: &mut String, stats: &CodeStats, opts: &Options) {
    let results = evaluate(stats, opts);
    let scope = if opts.gates.is_empty() {
        format!(
            "全部（{}）",
            Gate::all()
                .iter()
                .map(|gate| gate.name())
                .collect::<Vec<_>>()
                .join(" / ")
        )
    } else {
        opts.gates
            .iter()
            .map(|gate| gate.name())
            .collect::<Vec<_>>()
            .join(" / ")
    };

    let _ = writeln!(out, "### 5. 硬门禁（`--check`）");
    let _ = writeln!(out);
    let _ = writeln!(out, "| 门禁 | 当前值 | 上限（棘轮） | PLAN 目标 | 判定 |");
    let _ = writeln!(out, "|---|---:|---:|---:|---|");
    for result in &results {
        let mark = if result.pass { "PASS" } else { "FAIL" };
        let _ = writeln!(
            out,
            "| {} | {} | ≤ {} | {} | {} |",
            result.title,
            thousands(result.actual),
            thousands(result.limit),
            thousands(result.plan_target),
            mark
        );
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "- 判定范围（影响退出码）：{scope}");
    if !opts.check {
        let _ = writeln!(
            out,
            "- 结论：PASS（未加 `--check`：门禁只作展示，不影响退出码）"
        );
    } else if exit_code(&results, true) == ExitCode::from(EXIT_PASS) {
        let _ = writeln!(out, "- 结论：PASS —— 判定范围内无超限；退出码 0");
    } else {
        let _ = writeln!(out, "- 结论：FAIL —— 判定范围有超限；退出码 1");
        for result in results.iter().filter(|r| r.selected && !r.pass) {
            let _ = writeln!(
                out,
                "  - `{}`：当前 {} ＞ 上限 {}（PLAN 目标 {}）",
                result.id, result.actual, result.limit, result.plan_target
            );
            if result.group == Gate::Over1000 {
                for file in &stats.src_over_1000 {
                    let _ = writeln!(out, "    - `{}`：{} 行", file.rel, file.total);
                }
            }
        }
    }
    let _ = writeln!(out);
}

fn write_methodology(out: &mut String, stats: &CodeStats, opts: &Options) {
    let _ = writeln!(out, "### 6. 口径与已知边界");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "- Rust：`crates/*/**/*.rs` + `xtask/src/**/*.rs`；排除 `target/ build/ dist/ node_modules/ .venv/` 与隐藏目录，不跟随符号链接；"
    );
    let _ = writeln!(
        out,
        "- 「生产」= `src/` 内不属于 `#[cfg(test)]` 花括号配对条目的行；「内联测试」= 该条目（含属性行）；"
    );
    let _ = writeln!(
        out,
        "- 「集成测试 / 示例」= `crates/*/tests/**`、`crates/*/examples/**`，以及 `src/` 内 `tests/` 目录或 `tests.rs` 文件；"
    );
    let _ = writeln!(
        out,
        "- 超限门禁按**文件总行数**（含内联测试）计——与 PLAN §2.4 的 58 / 4 一致；"
    );
    let _ = writeln!(
        out,
        "- 依赖数 = 顶层 `[dependencies]` 键数（不含 `[target.*.dependencies]` / dev / build）；"
    );
    let _ = writeln!(out, "- 「缺」= 产物目录不存在（未构建）；不伪造体积。");
    let _ = writeln!(out);
    let test_module_lines: u64 = stats.test_module_files.iter().map(|f| f.total).sum();
    let _ = writeln!(
        out,
        "- **披露**：仅被 `#[cfg(test)] mod x;` 引用的 `src/` 测试模块文件 `{}` 个 / {} 行——它们住在 `src/`，上表按物理位置计入「生产」；严格口径下应从生产里扣除。",
        stats.test_module_files.len(),
        thousands(test_module_lines)
    );
    let _ = writeln!(
        out,
        "- **已知偏差（与冻结基线同口径）**：无花括号的 `#[cfg(test)] mod x;` / `use ...;` 之后的行一律计为内联测试（PLAN §2 的手工脚本同口径，prod 相差 −0.29%）；对「测试模块声明在前、生产代码在后」的文件（典型 `live2d-ai-runtime/src/settings.rs`：826 行里 793 行被判为内联测试）会把生产算少。这是刻度口径，不是逐行语义判断。"
    );
    if opts.verbose && !stats.test_module_files.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(out, "| 测试模块文件（`src/` 内） | 行数 |");
        let _ = writeln!(out, "|---|---:|");
        for file in &stats.test_module_files {
            let _ = writeln!(out, "| `{}` | {} |", file.rel, file.total);
        }
    }
    let _ = writeln!(out);
}

/// 千分位（release note 要读）。
pub fn thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// 人读体积（MiB/KiB）。
pub fn human_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    let value = bytes as f64;
    if value >= MIB {
        format!("{:.1} MiB", value / MIB)
    } else {
        format!("{:.1} KiB", value / KIB)
    }
}

/// 当前 UTC 时间（`YYYY-MM-DD HH:MM UTC`）——不引第三方时间库。
pub fn now_utc_string() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    format_utc(seconds)
}

/// epoch 秒 → `YYYY-MM-DD HH:MM UTC`。
pub fn format_utc(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let rem = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        rem / 3600,
        (rem % 3600) / 60
    )
}

/// Howard Hinnant 的 `civil_from_days`（1970-01-01 为第 0 天）。
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}
