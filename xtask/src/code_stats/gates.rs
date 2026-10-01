//! 硬门禁：棘轮阈值、四条门禁的判定与退出码。

use std::process::ExitCode;

use crate::{EXIT_BELOW_THRESHOLD, EXIT_PASS};

use super::cli::Options;
use super::*;

// ── 门禁 ──────────────────────────────────────────────────────────────────────

/// 可被 `--only` 选中的门禁分组。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// 文件行数：`crates/*/src` `.rs` > 500 与 Dart `lib` > 800。
    Lines,
    /// `crates/*/src` `.rs` > 1000（PLAN 目标 0）。
    Over1000,
    /// `live2d-ai-desktop` 依赖计数（PLAN 目标 ≤ 22）。
    Deps,
}

impl Gate {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "lines" | "行数" => Some(Gate::Lines),
            "over-1000" | "over1000" => Some(Gate::Over1000),
            "deps" | "依赖" => Some(Gate::Deps),
            _ => None,
        }
    }

    /// 门禁分组名（`--only` 的取值）。
    pub fn name(self) -> &'static str {
        match self {
            Gate::Lines => "lines",
            Gate::Over1000 => "over-1000",
            Gate::Deps => "deps",
        }
    }

    /// `--only` 未指定时的全集。
    pub fn all() -> Vec<Self> {
        vec![Gate::Lines, Gate::Over1000, Gate::Deps]
    }
}

/// 硬门禁阈值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub src_rs_over_500: u64,
    pub src_rs_over_1000: u64,
    pub dart_over_800: u64,
    pub desktop_deps: u64,
}

impl Limits {
    /// CI 默认：立档现状（棘轮）。
    pub fn ratchet() -> Self {
        Self {
            src_rs_over_500: RATCHET_SRC_RS_500,
            src_rs_over_1000: RATCHET_SRC_RS_1000,
            dart_over_800: RATCHET_DART_800,
            desktop_deps: RATCHET_DESKTOP_DEPS,
        }
    }

    /// PLAN §5 的目标值（现在仍是红的，留给 D2/D4 收紧）。
    pub fn plan() -> Self {
        Self {
            src_rs_over_500: PLAN_SRC_RS_500,
            src_rs_over_1000: PLAN_SRC_RS_1000,
            dart_over_800: PLAN_DART_800,
            desktop_deps: PLAN_DESKTOP_DEPS,
        }
    }
}

/// 单条门禁的判定结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateResult {
    /// 门禁 id（用于 `--only`）。
    pub id: &'static str,
    pub group: Gate,
    pub title: String,
    pub actual: u64,
    pub limit: u64,
    pub plan_target: u64,
    /// 是否参与「影响退出码」的判定（`--only` 选中的才算）。
    pub selected: bool,
    pub pass: bool,
}

// ── 门禁判定 ──────────────────────────────────────────────────────────────────

/// 计算四条门禁的当前值与判定；`--only` 选中的才参与退出码。
pub fn evaluate(stats: &CodeStats, opts: &Options) -> Vec<GateResult> {
    let selected = |group: Gate| opts.gates.is_empty() || opts.gates.contains(&group);
    let desktop_deps = stats
        .crates
        .iter()
        .find(|crate_stats| crate_stats.name == DESKTOP_CRATE)
        .map_or(0, |crate_stats| crate_stats.deps);

    let gate = |id: &'static str,
                group: Gate,
                title: String,
                actual: u64,
                limit: u64,
                plan_target: u64| {
        GateResult {
            id,
            group,
            title,
            actual,
            limit,
            plan_target,
            selected: selected(group),
            pass: actual <= limit,
        }
    };

    vec![
        gate(
            "src-rs-500",
            Gate::Lines,
            format!(
                "`crates/*/src` `.rs` > 500 行的文件数（上限 {}）",
                opts.limits.src_rs_over_500
            ),
            stats.src_over_500.len() as u64,
            opts.limits.src_rs_over_500,
            PLAN_SRC_RS_500,
        ),
        gate(
            "dart-800",
            Gate::Lines,
            format!(
                "Dart `lib` > 800 行的文件数（上限 {}）",
                opts.limits.dart_over_800
            ),
            stats.dart_lib_over_800.len() as u64,
            opts.limits.dart_over_800,
            PLAN_DART_800,
        ),
        gate(
            "src-rs-1000",
            Gate::Over1000,
            format!(
                "`crates/*/src` `.rs` > 1000 行的文件数（上限 {}）",
                opts.limits.src_rs_over_1000
            ),
            stats.src_over_1000.len() as u64,
            opts.limits.src_rs_over_1000,
            PLAN_SRC_RS_1000,
        ),
        gate(
            "deps-desktop",
            Gate::Deps,
            format!(
                "`{DESKTOP_CRATE}` 顶层 `[dependencies]` 条数（上限 {}）",
                opts.limits.desktop_deps
            ),
            desktop_deps,
            opts.limits.desktop_deps,
            PLAN_DESKTOP_DEPS,
        ),
    ]
}

/// 四条的 console 汇总（CI 日志末尾一眼看到结论）。
pub fn exit_code(results: &[GateResult], check: bool) -> ExitCode {
    if !check {
        return ExitCode::from(EXIT_PASS);
    }
    if results.iter().any(|result| result.selected && !result.pass) {
        ExitCode::from(EXIT_BELOW_THRESHOLD)
    } else {
        ExitCode::from(EXIT_PASS)
    }
}
