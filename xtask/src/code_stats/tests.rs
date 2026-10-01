//! `code-stats` 单测：**给定 fixture 目录树 → 期望的 prod/test/超限计数**。
//!
//! 关键边界（planner 指定）：`#[cfg(test)]` 内联块**不得**计入生产；
//! 与门禁红绿双向对应的判定函数 [`evaluate`] / [`exit_code`] 也在这里锁死。

use super::cli::{Options, parse_args};
use super::collect::{RustKind, collect, dir_size, rust_kind};
use super::gates::{Gate, Limits, evaluate, exit_code};
use super::measure::{count_dependencies_in, inline_test_lines, physical_lines};
use super::report::{format_utc, human_size, report, thousands};
use crate::{EXIT_BELOW_THRESHOLD, EXIT_PASS};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// 隔离的临时 fixture 树（用后删除）。
struct TempTree {
    root: PathBuf,
}

impl TempTree {
    fn new(tag: &str) -> Self {
        let id = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "xtask-code-stats-{}-{}-{tag}",
            std::process::id(),
            id
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create temp root");
        Self { root }
    }

    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dir");
        }
        fs::write(&path, contents).expect("write fixture file");
        path
    }

    /// 写一个「首行是 `// filler`，其余 N-1 行是 `// x`」的占位文件（行数精确可控）。
    fn write_lines(&self, relative: &str, lines: u64, symbol: &str) -> PathBuf {
        let mut text = String::new();
        for index in 0..lines {
            text.push_str(&format!("{symbol} line {index}\n"));
        }
        self.write(relative, &text)
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// 造一棵最小但覆盖全部分类的 fixture 树。
fn fixture(tag: &str) -> TempTree {
    let tree = TempTree::new(tag);
    // Rust：生产 3 行 + 内联测试块 5 行（边界用例）。
    tree.write(
        "crates/demo/src/lib.rs",
        "pub fn a() {}\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\npub fn b() {}\n",
    );
    // 超限文件：501 行 / 1001 行。
    tree.write_lines("crates/demo/src/big.rs", 501, "//");
    tree.write_lines("crates/demo/src/huge.rs", 1001, "//");
    // src 内测试文件（`tests/` 目录、`tests.rs`）→ 集成桶。
    tree.write_lines("crates/demo/src/tests.rs", 10, "//");
    tree.write_lines("crates/demo/src/parts/tests/one.rs", 20, "//");
    // 兄弟测试模块文件（住在 src/，本工具披露口径：计入生产）。
    tree.write_lines("crates/demo/src/alpha_tests.rs", 30, "//");
    // 集成测试 / 示例。
    tree.write_lines("crates/demo/tests/integration.rs", 40, "//");
    tree.write_lines("crates/demo/examples/demo.rs", 7, "//");
    // 依赖：顶层 2 条 + 子表 1 条；target/dev 不数。
    tree.write(
        "crates/demo/Cargo.toml",
        "[package]\nname = \"demo\"\n\n[dependencies]\nserde = \"1\"\n\n[dependencies.tokio]\nversion = \"1\"\n\n[dev-dependencies]\nproptest = \"1\"\n\n[target.'cfg(unix)'.dependencies]\nlibc = \"0.2\"\n",
    );
    // 排除目录：不得进统计。
    tree.write_lines("crates/demo/target/ignored.rs", 99, "//");
    tree.write_lines("crates/demo/src/build/ignored2.rs", 99, "//");
    tree.write_lines("crates/demo/.hidden/ignored3.rs", 99, "//");
    // Dart。
    tree.write("shell/flutter/lib/main.dart", "void main() {}\n");
    tree.write_lines("shell/flutter/lib/big.dart", 801, "//");
    tree.write_lines("shell/flutter/test/main_test.dart", 12, "//");
    tree.write_lines("shell/flutter/test/huge_test.dart", 901, "//");
    // docs（含 legacy）。
    tree.write("docs/a.md", "# a\n# b\n");
    tree.write("docs/legacy/b.md", "# c\n");
    tree
}

#[test]
fn inline_cfg_test_block_is_not_counted_as_production() {
    let text =
        "pub fn a() {}\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\npub fn b() {}\n";
    assert_eq!(physical_lines(text), 7);
    // 5 行内联块（属性行 + mod + 用例 3 行），生产 = 2 行。
    assert_eq!(inline_test_lines(text), 5);
}

/// **判别力自查（2026-10-01 verifier 发现）**：无花括号的 `#[cfg(test)] mod x;` 必须
/// **吃到文件末尾**。旧 fixture 把这个条目放在**最后一行**，于是「吃到末尾」与
/// 「扫到下一处括号归零」同解 ⇒ 恒真断言（假绿灯）。这里在后面接一行
/// `use serde::{Deserialize, Serialize};`（一行内 `{` `}` 同现 ⇒ 收支归零，正是旧实现
/// 的提前收尾触发点）再跟生产函数，两条规则才分得开：旧实现得 3，正确实现得 4。
#[test]
fn inline_cfg_test_without_braces_runs_to_end_of_file() {
    let text = "pub fn a() {}\n#[cfg(test)]\nmod tests_x;\nuse serde::{Deserialize, Serialize};\npub fn b() {}\n";
    assert_eq!(physical_lines(text), 5);
    // attr 行 + `mod tests_x;` + `use ...` + `pub fn b()` = 4（attr 起，吃到文件末尾）。
    assert_eq!(inline_test_lines(text), 4);
}

/// 无花括号条目的另一种形态：`#[cfg(test)] use ...;`（同样吃到文件末尾）。
#[test]
fn inline_cfg_test_without_braces_use_form_runs_to_end_of_file() {
    let text =
        "pub fn a() {}\n#[cfg(test)]\nuse super::*;\nuse serde::{Deserialize};\npub fn b() {}\n";
    assert_eq!(physical_lines(text), 5);
    assert_eq!(inline_test_lines(text), 4);
}

#[test]
fn inline_cfg_test_counts_multiple_blocks() {
    let text = "#[cfg(test)]\nmod a {\n    fn x() {}\n}\n\npub fn prod() {}\n\n#[cfg(test)]\nmod b {\n    fn y() {}\n}\n";
    assert_eq!(physical_lines(text), 11);
    assert_eq!(inline_test_lines(text), 8);
}

#[test]
fn physical_lines_matches_wc_l_semantics() {
    assert_eq!(physical_lines(""), 0);
    assert_eq!(physical_lines("a\n"), 1);
    assert_eq!(physical_lines("a\nb\n"), 2);
    assert_eq!(physical_lines("a\nb"), 2); // 末尾残行补 1
    assert_eq!(physical_lines("\n"), 1);
}

#[test]
fn collect_classifies_src_tests_and_examples_per_fixture() {
    let tree = fixture("classify");
    let stats = collect(&tree.root).expect("scan fixture");

    let demo = stats
        .crates
        .iter()
        .find(|crate_stats| crate_stats.name == "demo")
        .expect("demo crate present");

    // 生产：lib.rs 2 + big.rs 501 + huge.rs 1001 + alpha_tests.rs 30 = 1534。
    assert_eq!(demo.prod, 2 + 501 + 1001 + 30);
    // 内联测试：lib.rs 的 5 行块。
    assert_eq!(demo.inline_test, 5);
    // 集成 / 示例：tests.rs 10 + parts/tests/one.rs 20 + tests/integration.rs 40 + examples/demo.rs 7。
    assert_eq!(demo.integ, 10 + 20 + 40 + 7);
    assert_eq!(demo.deps, 2); // serde + tokio 子表；dev / target 不数

    // 排除目录没被算进来。
    assert_eq!(demo.total(), 2 + 501 + 1001 + 30 + 5 + 77);
}

#[test]
fn collect_over_limit_lists_match_fixture() {
    let tree = fixture("limits");
    let stats = collect(&tree.root).expect("scan fixture");

    let over_500: Vec<&str> = stats
        .src_over_500
        .iter()
        .map(|file| file.rel.as_str())
        .collect();
    // fixture 里 > 500 的只有 big.rs(501) / huge.rs(1001)；排除目录里的 99 行不进统计。
    assert_eq!(
        over_500,
        vec!["crates/demo/src/huge.rs", "crates/demo/src/big.rs"]
    );
    let over_1000: Vec<&str> = stats
        .src_over_1000
        .iter()
        .map(|file| file.rel.as_str())
        .collect();
    assert_eq!(over_1000, vec!["crates/demo/src/huge.rs"]);
    // 超限行的「内联测试」列必须是 0（fixture 里这两个文件没有 cfg(test) 块）。
    assert_eq!(stats.src_over_500[0].inline_test, 0);
    assert_eq!(stats.src_over_500[0].prod(), 1001);
}

#[test]
fn collect_dart_and_docs_and_missing_artifacts_per_fixture() {
    let tree = fixture("dart-docs");
    let stats = collect(&tree.root).expect("scan fixture");

    assert_eq!(stats.dart_lib_files, 2);
    assert_eq!(stats.dart_lib_lines, 1 + 801);
    let over_800: Vec<&str> = stats
        .dart_lib_over_800
        .iter()
        .map(|(path, _)| path.as_str())
        .collect();
    assert_eq!(over_800, vec!["shell/flutter/lib/big.dart"]);
    // `test/` 超限只披露、不进门禁。
    assert_eq!(stats.dart_test_over_800.len(), 1);
    assert_eq!(stats.dart_test_files, 2);
    assert_eq!(stats.dart_test_lines, 12 + 901);

    assert_eq!(stats.docs_files, 2);
    assert_eq!(stats.docs_lines, 3);

    // 产物目录不存在 → 记「缺」，不猜体积。
    assert_eq!(stats.artifacts.len(), 2);
    assert!(
        stats
            .artifacts
            .iter()
            .all(|artifact| artifact.bytes.is_none())
    );
}

#[test]
fn rust_kind_splits_src_tests_and_examples() {
    assert_eq!(rust_kind(Path::new("src/lib.rs")), RustKind::Src);
    assert_eq!(
        rust_kind(Path::new("src/tests.rs")),
        RustKind::SrcIntegration
    );
    assert_eq!(
        rust_kind(Path::new("src/parts/tests/one.rs")),
        RustKind::SrcIntegration
    );
    assert_eq!(
        rust_kind(Path::new("tests/integration.rs")),
        RustKind::OtherIntegration
    );
    assert_eq!(
        rust_kind(Path::new("examples/demo.rs")),
        RustKind::OtherIntegration
    );
}

#[test]
fn dependencies_count_ignores_target_and_dev_sections() {
    let manifest = "[dependencies]\na = \"1\"\n\n[dependencies.b]\nversion = \"1\"\n\n[dev-dependencies]\nc = \"1\"\n\n[build-dependencies]\nd = \"1\"\n\n[target.'cfg(unix)'.dependencies]\ne = \"1\"\n";
    assert_eq!(count_dependencies_in(manifest), 2);
    // 重复键 / 点号键只数一次、只数顶层。
    assert_eq!(
        count_dependencies_in("[dependencies]\na = \"1\"\na.b = \"2\"\n"),
        1
    );
    // 注释不影响计数。
    assert_eq!(
        count_dependencies_in("[dependencies]\n# a = \"1\"\na = \"1\" # 行尾注释\n"),
        1
    );
}

#[test]
fn gate_evaluation_is_red_when_fixture_exceeds_a_zero_limit() {
    let tree = fixture("gate-red");
    let stats = collect(&tree.root).expect("scan fixture");

    // 红：>1000 上限设 0，fixture 有 1 个 → FAIL，退出码 1。
    let opts = Options {
        check: true,
        gates: vec![Gate::Over1000],
        limits: Limits {
            src_rs_over_1000: 0,
            ..Limits::ratchet()
        },
        ..Options::default()
    };
    let results = evaluate(&stats, &opts);
    let over = results
        .iter()
        .find(|result| result.group == Gate::Over1000)
        .expect("over-1000 gate present");
    assert_eq!(over.actual, 1);
    assert!(!over.pass);
    assert_eq!(
        exit_code(&results, true),
        ExitCode::from(EXIT_BELOW_THRESHOLD)
    );
    // 未选中 over-1000 时，同一条 FAIL 不参与退出码判定。
    let lines_only = Options {
        gates: vec![Gate::Lines],
        limits: Limits {
            src_rs_over_500: 2,
            dart_over_800: 1,
            ..Limits::ratchet()
        },
        ..opts.clone()
    };
    let results = evaluate(&stats, &lines_only);
    assert_eq!(exit_code(&results, true), ExitCode::from(EXIT_PASS));

    // 绿：>1000 上限放到 1（fixture 只有 1 个）→ PASS，退出码 0。
    let green = Options {
        limits: Limits {
            src_rs_over_1000: 1,
            ..Limits::ratchet()
        },
        ..opts
    };
    let results = evaluate(&stats, &green);
    assert_eq!(exit_code(&results, true), ExitCode::from(EXIT_PASS));
}

#[test]
fn gate_evaluation_covers_lines_and_deps() {
    let tree = fixture("gate-lines-deps");
    let stats = collect(&tree.root).expect("scan fixture");
    let opts = Options {
        check: true,
        gates: vec![Gate::Lines, Gate::Deps],
        limits: Limits {
            src_rs_over_500: 1,
            dart_over_800: 0,
            desktop_deps: 0,
            src_rs_over_1000: 0,
        },
        ..Options::default()
    };
    let results = evaluate(&stats, &opts);
    // >500：fixture 有 2 个，上限 1 → FAIL。
    let lines = results
        .iter()
        .find(|result| result.id == "src-rs-500")
        .expect("src-rs-500 present");
    assert_eq!(lines.actual, 2);
    assert!(!lines.pass);
    // Dart >800：fixture 有 1 个，上限 0 → FAIL。
    let dart = results
        .iter()
        .find(|result| result.id == "dart-800")
        .expect("dart-800 present");
    assert_eq!(dart.actual, 1);
    assert!(!dart.pass);
    // deps：fixture 里没有 live2d-ai-desktop → 0 ≤ 0 PASS。
    let deps = results
        .iter()
        .find(|result| result.id == "deps-desktop")
        .expect("deps gate present");
    assert_eq!(deps.actual, 0);
    assert!(deps.pass);
    assert_eq!(
        exit_code(&results, true),
        ExitCode::from(EXIT_BELOW_THRESHOLD)
    );
}

#[test]
fn parse_args_accepts_check_scopes_and_limits() {
    let opts = parse_args(&[
        "--check".to_string(),
        "--only".to_string(),
        "deps".to_string(),
    ])
    .expect("parse");
    assert!(opts.check);
    assert_eq!(opts.gates, vec![Gate::Deps]);
    assert_eq!(opts.limits, Limits::ratchet());

    let opts = parse_args(&[
        "--check".to_string(),
        "--only=lines".to_string(),
        "--max-src-rs-500=15".to_string(),
        "--max-dart-800".to_string(),
        "2".to_string(),
        "--max-deps".to_string(),
        "live2d-ai-desktop=22".to_string(),
    ])
    .expect("parse");
    assert_eq!(opts.gates, vec![Gate::Lines]);
    assert_eq!(opts.limits.src_rs_over_500, 15);
    assert_eq!(opts.limits.dart_over_800, 2);
    assert_eq!(opts.limits.desktop_deps, 22);

    let strict = parse_args(&["--strict-plan".to_string()]).expect("parse");
    assert_eq!(strict.limits, Limits::plan());

    assert!(parse_args(&["--nope".to_string()]).is_err());
    assert!(parse_args(&["--only".to_string(), "oops".to_string()]).is_err());
    assert!(parse_args(&["--max-src-rs-500".to_string(), "abc".to_string()]).is_err());
    assert!(parse_args(&["--max-deps".to_string(), "other=1".to_string()]).is_err());
    assert!(parse_args(&["--max-src-rs-500".to_string()]).is_err());
}

#[test]
fn report_renders_markdown_with_gate_verdict() {
    let tree = fixture("report");
    let stats = collect(&tree.root).expect("scan fixture");
    let opts = Options {
        check: true,
        gates: vec![Gate::Deps],
        limits: Limits {
            desktop_deps: 0,
            ..Limits::ratchet()
        },
        verbose: true,
        ..Options::default()
    };
    let text = report(&tree.root, &stats, &opts);
    assert!(text.contains("## 代码体量与门禁快照（D0）"));
    assert!(text.contains("### 3. 超限清单"));
    assert!(text.contains("#### 3.1 `crates/*/src` 生产 `.rs` > 1000 行（1 个，PLAN 目标 0）"));
    assert!(text.contains("结论：PASS"));
    assert!(text.contains("测试模块文件"));
}

#[test]
fn thousands_and_human_size_read_well() {
    assert_eq!(thousands(0), "0");
    assert_eq!(thousands(999), "999");
    assert_eq!(thousands(1000), "1,000");
    assert_eq!(thousands(67_806), "67,806");
    assert_eq!(thousands(96_899), "96,899");
    assert_eq!(human_size(47 * 1024 * 1024), "47.0 MiB");
    assert_eq!(human_size(512), "0.5 KiB");
}

#[test]
fn utc_formatting_matches_known_epochs() {
    assert_eq!(format_utc(0), "1970-01-01 00:00 UTC");
    assert_eq!(format_utc(1_735_689_600), "2025-01-01 00:00 UTC");
    assert_eq!(format_utc(1_767_225_600), "2026-01-01 00:00 UTC");
}

#[test]
fn dir_size_returns_none_for_missing_directory() {
    let tree = TempTree::new("dir-size");
    assert_eq!(dir_size(&tree.root.join("nope")).expect("size"), None);
    tree.write("has/file.bin", "12345");
    assert_eq!(dir_size(&tree.root.join("has")).expect("size"), Some(5));
}
