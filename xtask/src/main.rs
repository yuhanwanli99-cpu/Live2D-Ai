//! `xtask` — Live2D-Ai 工程工具（第一批）。
//!
//! 目前仅实现 [`Command::RustRatio`]：
//! 统计仓库**第一方可执行源码**的 Rust 占比（RFC `docs/plans/RUST-REWRITE-RFC.md` D3）。
//!
//! 口径（清晰可审计的物理行数）：
//! - 统计扩展名：`rs / wgsl / py / ts / js / c / cc / cpp / h / hpp`；
//! - **豁免扩展名**：`dart` —— 前端/接口层（`shell/flutter/`）由 Flutter 工具链
//!   自行门禁（`flutter analyze` + `flutter test`），**不计入** Rust 占比分母；
//!   但仍单独统计并打印，保证「豁免 ≠ 不可见」（见 [`EXEMPT_EXTENSIONS`]）；
//! - 每文件按**物理行数**计数（与 `wc -l` 一致，可独立复核）；
//! - 排除目录（按目录名、任意层级）：`Live2D-Ai-pc`、`Live2D-Ai-Android`、`dist`、
//!   `target`、`build`、`node_modules`、`.venv`；
//! - 另跳过隐藏目录（`.` 开头，任意层级：`.git`、`.uv-cache` 等 VCS/托管缓存）；
//! - 不跟随目录符号链接（避免循环与逃出仓库根）。
//!
//! 占比 = Rust(`.rs`) 物理行数 ÷ 全部**被统计（非豁免）**文件物理行数 × 100%。
//! 低于门槛时退出码非 0（门槛默认 95，可用 `--threshold` 覆盖）。
//!
//! 许可：**AGPL-3.0-only**，以仓库根 `LICENSE` 为准。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;

/// 参与占比统计的第一方源码扩展名（RFC D3 口径）。
const COUNTED_EXTENSIONS: [&str; 10] =
    ["rs", "wgsl", "py", "ts", "js", "c", "cc", "cpp", "h", "hpp"];

/// **豁免**扩展名：单独统计、单独打印，但**不计入**占比分母。
///
/// `dart` = 前端/接口层（`shell/flutter/`）。豁免理由（2026-09-10 用户裁决，
/// 同步记录于 `AGENTS.md` §「前端层豁免的理由」）：
///
/// 1. 项目采用「Rust 核心 + Flutter 前端」**双主导**分层：Flutter Web 无法用
///    `dart:ffi` 渲染，Dart 只承担界面与调度，渲染核心仍是 Rust（`/render` wasm）；
/// 2. 前端质量门槛由 Flutter 自己的工具链承担
///    （`flutter analyze` + `flutter test`），不重复计入 Rust 占比；
/// 3. **豁免不等于不可见**：本列表会被统计并在报告中单独成段打印行数，
///    防止前端悄悄膨胀成为审计盲区。
const EXEMPT_EXTENSIONS: [&str; 1] = ["dart"];

/// 按目录名排除（任意层级）：旧双端项目 / 前端产物 / 构建产物 / 托管环境。
///
/// `build`：Flutter Web 等构建输出（`shell/flutter/build/web/main.dart.js` 单文件
/// 就有 ~9 万行 js，会把「第一方源码」占比口径打穿；构建产物不是源码）。
const EXCLUDED_DIR_NAMES: [&str; 7] = [
    "Live2D-Ai-pc",
    "Live2D-Ai-Android",
    "dist",
    "target",
    "build",
    "node_modules",
    ".venv",
];

/// 未显式传参时的占比门槛（百分比）。
const DEFAULT_THRESHOLD: f64 = 95.0;

/// 退出码：通过 / 未达门槛 / 用法或运行错误。
const EXIT_PASS: u8 = 0;
const EXIT_BELOW_THRESHOLD: u8 = 1;
const EXIT_USAGE: u8 = 2;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("xtask: 错误：{err}");
            eprintln!("用法：cargo run -p xtask -- rust-ratio [--threshold <0..100 的数值>]");
            ExitCode::from(EXIT_USAGE)
        }
    }
}

/// 解析并分发子命令。独立成函数便于单元测试（不经进程退出）。
///
/// # Errors
/// 返回用法错误（未知子命令、缺失/非法参数等）。
fn run(args: &[String]) -> Result<ExitCode, String> {
    let Some(first) = args.first() else {
        return Err("缺少子命令".to_string());
    };
    if first == "help" || first == "--help" || first == "-h" {
        print_help();
        return Ok(ExitCode::from(EXIT_PASS));
    }
    if first != "rust-ratio" {
        return Err(format!(
            "未知子命令 `{first}`（可用子命令：rust-ratio、help）"
        ));
    }

    let mut threshold: Option<f64> = None;
    let mut rest = &args[1..];
    while let Some(flag) = rest.first() {
        match flag.as_str() {
            "--threshold" => {
                let Some(value) = rest.get(1) else {
                    return Err("--threshold 缺少参数值".to_string());
                };
                threshold = Some(parse_threshold(value)?);
                rest = &rest[2..];
            }
            _ if flag.starts_with("--threshold=") => {
                let value = &flag["--threshold=".len()..];
                threshold = Some(parse_threshold(value)?);
                rest = &rest[1..];
            }
            other => return Err(format!("无法识别的参数 `{other}`")),
        }
    }
    let threshold = threshold.unwrap_or(DEFAULT_THRESHOLD);

    let root =
        locate_repo_root().ok_or("未能定位仓库根（含 [workspace] 的 Cargo.toml 所在目录）")?;
    let stats =
        collect_stats(&root).map_err(|err| format!("扫描 {} 失败：{err}", root.display()))?;
    report(&root, threshold, &stats);
    if stats.passes(threshold) {
        Ok(ExitCode::from(EXIT_PASS))
    } else {
        // 低于门槛退出码非 0 是 RFC D3 的预期行为（当前基线为第一方 Python 脚本/测试）。
        Ok(ExitCode::from(EXIT_BELOW_THRESHOLD))
    }
}

fn print_help() {
    println!(
        "xtask — Live2D-Ai 工程工具\n\
         \n\
         用法：cargo run -p xtask -- <子命令> [参数]\n\
         \n\
         子命令：\n  \
         rust-ratio [--threshold <f64>]  第一方源码 Rust 占比统计（RFC D3，默认门槛 95）\n  \
         help                            显示本帮助\n\
         \n\
         退出码：0 达标；1 未达门槛；2 用法/运行错误。"
    );
}

fn parse_threshold(value: &str) -> Result<f64, String> {
    let parsed =
        f64::from_str(value.trim()).map_err(|_| format!("--threshold 不是合法数值：`{value}`"))?;
    if !parsed.is_finite() || !(0.0..=100.0).contains(&parsed) {
        return Err(format!("--threshold 必须在 0..=100 内，得到 `{value}`"));
    }
    Ok(parsed)
}

/// 自当前工作目录向上查找含 `[workspace]` 的 `Cargo.toml`；
/// 找不到时回退到 xtask 自身所在 workspace 根（`CARGO_MANIFEST_DIR` 的父目录），
/// 保证无论从哪里调用都能得到同一仓库根。
fn locate_repo_root() -> Option<PathBuf> {
    let mut candidate = std::env::current_dir().ok()?;
    loop {
        if is_workspace_root(&candidate) {
            return Some(candidate);
        }
        if !candidate.pop() {
            break;
        }
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .to_path_buf();
    is_workspace_root(&manifest).then_some(manifest)
}

fn is_workspace_root(dir: &Path) -> bool {
    let Ok(manifest) = fs::read_to_string(dir.join("Cargo.toml")) else {
        return false;
    };
    manifest.contains("[workspace]")
}

/// 各扩展名的累计统计。
#[derive(Debug, Default, Clone, PartialEq)]
struct ExtensionStats {
    files: u64,
    lines: u64,
}

/// 一次扫描的总结果。
#[derive(Debug, Default, Clone, PartialEq)]
struct ScanStats {
    /// 扩展名（小写）→ 统计；只包含 [`COUNTED_EXTENSIONS`] 内的键（**计入**分母）。
    per_extension: Vec<(String, ExtensionStats)>,
    /// 扩展名（小写）→ 统计；只包含 [`EXEMPT_EXTENSIONS`] 内的键（**不计入**分母）。
    exempt_per_extension: Vec<(String, ExtensionStats)>,
}

impl ScanStats {
    fn record(&mut self, extension: &str, lines: u64) {
        record_into(&mut self.per_extension, extension, lines);
    }

    /// 记录一个**豁免**扩展名（计入可见性报告，但不进占比分母）。
    fn record_exempt(&mut self, extension: &str, lines: u64) {
        record_into(&mut self.exempt_per_extension, extension, lines);
    }

    fn lines_for(&self, extension: &str) -> u64 {
        self.per_extension
            .iter()
            .find(|(name, _)| name == extension)
            .map_or(0, |(_, stats)| stats.lines)
    }

    /// 豁免扩展名的物理行数（可见性报告用）。
    fn exempt_lines_for(&self, extension: &str) -> u64 {
        self.exempt_per_extension
            .iter()
            .find(|(name, _)| name == extension)
            .map_or(0, |(_, stats)| stats.lines)
    }

    fn exempt_total_files(&self) -> u64 {
        self.exempt_per_extension.iter().map(|(_, s)| s.files).sum()
    }

    fn exempt_total_lines(&self) -> u64 {
        self.exempt_per_extension.iter().map(|(_, s)| s.lines).sum()
    }

    fn total_files(&self) -> u64 {
        self.per_extension.iter().map(|(_, s)| s.files).sum()
    }

    fn total_lines(&self) -> u64 {
        self.per_extension.iter().map(|(_, s)| s.lines).sum()
    }

    /// Rust 物理行数占全部被统计物理行数的百分比；无数据时返回 `None`。
    fn rust_ratio_percent(&self) -> Option<f64> {
        let total = self.total_lines();
        (total > 0).then(|| self.lines_for("rs") as f64 * 100.0 / total as f64)
    }

    fn passes(&self, threshold: f64) -> bool {
        self.rust_ratio_percent()
            .is_some_and(|ratio| ratio >= threshold)
    }
}

/// 递归扫描 `root`，跳过排除目录 / 隐藏目录，不跟随符号链接。
fn collect_stats(root: &Path) -> io::Result<ScanStats> {
    let mut stats = ScanStats::default();
    walk(root, &mut stats)?;
    for bucket in [&mut stats.per_extension, &mut stats.exempt_per_extension] {
        bucket.sort_by(|a, b| b.1.lines.cmp(&a.1.lines).then(a.0.cmp(&b.0)));
    }
    Ok(stats)
}

fn walk(dir: &Path, stats: &mut ScanStats) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        // symlink_metadata：不穿透符号链接——链接本身不计入，目录链接不入栈。
        let file_type = entry.file_type()?;
        let path = entry.path();

        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if is_hidden_dir(&name) || EXCLUDED_DIR_NAMES.contains(&name.as_ref()) {
                continue;
            }
            walk(&path, stats)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        // 先试「计入分母」的集合，再试「豁免但可见」的集合；都不匹配则跳过。
        let lines = count_physical_lines(&path)?;
        if let Some(extension) = counted_extension(&path) {
            stats.record(extension, lines);
        } else if let Some(extension) = exempt_extension(&path) {
            stats.record_exempt(extension, lines);
        }
    }
    Ok(())
}

/// 隐藏目录：名字以 `.` 开头且不是当前/上级目录占位。
fn is_hidden_dir(name: &str) -> bool {
    name.starts_with('.') && name != "." && name != ".."
}

/// 文件扩展名若属于**计入分母**的统计集合则返回小写形式，否则 `None`。
fn counted_extension(path: &Path) -> Option<&str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    COUNTED_EXTENSIONS
        .iter()
        .find(|counted| **counted == extension.as_str())
        .copied()
}

/// 文件扩展名若属于**豁免**集合（[`EXEMPT_EXTENSIONS`]）则返回小写形式。
///
/// 豁免项单独统计并在报告中打印，**不参与**占比计算。
fn exempt_extension(path: &Path) -> Option<&str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    EXEMPT_EXTENSIONS
        .iter()
        .find(|exempt| **exempt == extension.as_str())
        .copied()
}

/// 把一次统计累加进指定桶（首次出现时建键）。
fn record_into(bucket: &mut Vec<(String, ExtensionStats)>, extension: &str, lines: u64) {
    match bucket.iter_mut().find(|(name, _)| name == extension) {
        Some((_, stats)) => {
            stats.files += 1;
            stats.lines += lines;
        }
        None => bucket.push((extension.to_string(), ExtensionStats { files: 1, lines })),
    }
}

/// 物理行数：`\n` 的个数；末尾无换行的残行补 1（与 `wc -l` 可对账）。
fn count_physical_lines(path: &Path) -> io::Result<u64> {
    let bytes = fs::read(path)?;
    Ok(count_bytes_as_lines(&bytes))
}

fn count_bytes_as_lines(bytes: &[u8]) -> u64 {
    let newlines = bytes.iter().filter(|byte| **byte == b'\n').count() as u64;
    if bytes.is_empty() || bytes[bytes.len() - 1] == b'\n' {
        newlines
    } else {
        newlines + 1
    }
}

/// 输出可审计报告：口径、范围、逐扩展明细、合计与判定。
/// 明细行数可与 `wc -l` 逐项复核。
fn report(root: &Path, threshold: f64, stats: &ScanStats) {
    println!("rust-ratio — 第一方可执行源码 Rust 占比（RFC docs/plans/RUST-REWRITE-RFC.md D3）");
    println!("仓库根    : {}", root.display());
    println!("统计口径  : 物理行数（每文件 \\n 计数，末尾残行补 1；可用 wc -l 复核）");
    println!("统计扩展名: {}", COUNTED_EXTENSIONS.join(" "));
    println!(
        "排除目录  : {} （按目录名，任意层级）",
        EXCLUDED_DIR_NAMES.join(" ")
    );
    println!("另跳过    : 隐藏目录（`.` 开头，任意层级）；目录符号链接不入栈");

    println!();
    println!("{:<10} {:>8} {:>12}", "扩展名", "文件数", "行数");
    for (extension, ext_stats) in &stats.per_extension {
        println!(
            "{:<10} {:>8} {:>12}",
            extension, ext_stats.files, ext_stats.lines
        );
    }
    println!("{:-<34}", "");
    println!(
        "{:<10} {:>8} {:>12}",
        "合计",
        stats.total_files(),
        stats.total_lines()
    );

    println!();
    match stats.rust_ratio_percent() {
        Some(ratio) => {
            println!(
                "Rust(rs) 占比: {:.4}%（{} / {} 物理行）",
                ratio,
                stats.lines_for("rs"),
                stats.total_lines()
            );
            println!("门槛        : {threshold:.4}%");
            if stats.passes(threshold) {
                println!("结论        : PASS — 达到或高于门槛");
            } else {
                println!(
                    "结论        : FAIL — 低于门槛（差 {:.4} 个百分点）",
                    threshold - ratio
                );
            }
        }
        None => {
            println!("未找到任何被统计文件，无法计算占比");
        }
    }

    // 豁免扩展名（前端/接口层）：**单独可见**，但不进上面的分母。
    // 目的：豁免 ≠ 不可见——前端膨胀必须在报告里看得见（见 AGENTS.md）。
    if !stats.exempt_per_extension.is_empty() {
        println!();
        println!(
            "豁免（不计入占比，前端/接口层）: {}",
            EXEMPT_EXTENSIONS.join(" ")
        );
        println!("{:<10} {:>8} {:>12}", "扩展名", "文件数", "行数");
        for (extension, ext_stats) in &stats.exempt_per_extension {
            println!(
                "{:<10} {:>8} {:>12}",
                extension, ext_stats.files, ext_stats.lines
            );
        }
        println!("{:-<34}", "");
        println!(
            "{:<10} {:>8} {:>12}",
            "豁免合计",
            stats.exempt_total_files(),
            stats.exempt_total_lines()
        );
        println!(
            "说明        : {} 行由 Flutter 工具链门禁（flutter analyze / test），不计入 rust-ratio",
            stats.exempt_lines_for("dart")
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

    /// 在系统临时目录下造一棵隔离测试树（用后删除）。
    struct TempTree {
        root: PathBuf,
    }

    impl TempTree {
        fn new(tag: &str) -> Self {
            let id = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "xtask-test-{}-{}-{tag}",
                std::process::id(),
                id
            ));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).expect("create temp root");
            Self { root }
        }

        fn write(&self, relative: &str, contents: &[u8]) -> PathBuf {
            let path = self.root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create temp parent dir");
            }
            fs::write(&path, contents).expect("write temp file");
            path
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn counted_extension_matches_case_insensitively() {
        assert_eq!(counted_extension(Path::new("a/b/lib.RS")), Some("rs"));
        assert_eq!(counted_extension(Path::new("shader.Wgsl")), Some("wgsl"));
        assert_eq!(counted_extension(Path::new("main.cpp")), Some("cpp"));
        assert_eq!(counted_extension(Path::new("README.md")), None);
        assert_eq!(counted_extension(Path::new("noext")), None);
        assert_eq!(counted_extension(Path::new("archive.tar.gz")), None);
    }

    #[test]
    fn physical_line_counts_match_wc_l_semantics() {
        assert_eq!(count_bytes_as_lines(b""), 0);
        assert_eq!(count_bytes_as_lines(b"a\n"), 1);
        assert_eq!(count_bytes_as_lines(b"a\nb\n"), 2);
        assert_eq!(count_bytes_as_lines(b"a\nb"), 2); // 末尾残行补 1
        assert_eq!(count_bytes_as_lines(b"\n"), 1);
        assert_eq!(count_bytes_as_lines("中文注释\n".as_bytes()), 1);
    }

    #[test]
    fn walk_skips_excluded_hidden_dirs_and_symlinks() {
        let tree = TempTree::new("walk");
        tree.write("crates/l2d/src/lib.rs", b"pub fn a() {}\n");
        tree.write("target/debug/out.rs", b"generated\n"); // 排除目录名
        tree.write(".git/hooks/pre.rs", b"hook\n"); // 隐藏目录
        tree.write("dist/bundle.js", b"console.log(1);\n"); // 排除目录名
        tree.write("Live2D-Ai-pc/x.py", b"print(1)\n"); // 排除目录名
        tree.write("docs/note.md", b"# doc\n"); // 不在统计集合
        tree.write("scripts/run.py", b"print('x')\n");
        #[cfg(unix)]
        std::os::unix::fs::symlink(tree.root.join("elsewhere"), tree.root.join("link.rs"))
            .expect("symlink");

        let stats = collect_stats(&tree.root).expect("scan");

        assert_eq!(stats.total_files(), 2);
        assert_eq!(stats.lines_for("rs"), 1);
        assert_eq!(stats.lines_for("py"), 1);
        assert_eq!(stats.total_lines(), 2);
        assert_eq!(stats.rust_ratio_percent(), Some(50.0));
    }

    #[test]
    fn ratio_guard_against_zero_total() {
        let empty = ScanStats::default();
        assert_eq!(empty.rust_ratio_percent(), None);
        assert!(!empty.passes(0.0)); // 无数据一律不判 PASS

        let mut stats = ScanStats::default();
        stats.record("rs", 19);
        stats.record("py", 1);
        assert!((stats.rust_ratio_percent().unwrap() - 95.0).abs() < f64::EPSILON);
        assert!(stats.passes(95.0));
        assert!(!stats.passes(96.0));
    }

    #[test]
    fn record_aggregates_by_extension() {
        let mut stats = ScanStats::default();
        stats.record("rs", 10);
        stats.record("rs", 5);
        stats.record("wgsl", 7);
        assert_eq!(stats.per_extension.len(), 2);
        assert_eq!(stats.lines_for("rs"), 15);
        assert_eq!(stats.total_files(), 3);
        assert_eq!(stats.total_lines(), 22);
    }

    /// **豁免契约（2026-09-10）**：`dart` 必须**可见**（单独统计）
    /// 且**不计入**占比分母——两者同时成立才是「豁免」，缺一即是漏洞。
    #[test]
    fn dart_is_visible_but_excluded_from_the_ratio() {
        let tree = TempTree::new("dart-exempt");
        tree.write("crates/l2d/src/lib.rs", b"pub fn a() {}\n");
        tree.write("shell/flutter/lib/main.dart", b"void main() {}\n// x\n");
        tree.write("shell/flutter/test/schedule_test.dart", b"// t\n");

        let stats = collect_stats(&tree.root).expect("scan");

        // 可见：单独成桶，行数必须在报告里能读出来。
        assert_eq!(stats.exempt_total_files(), 2);
        assert_eq!(stats.exempt_lines_for("dart"), 3);
        assert_eq!(stats.exempt_total_lines(), 3);

        // 不计入分母：占比仍由 rs/total(非豁免) 决定。
        assert_eq!(stats.total_files(), 1);
        assert_eq!(stats.total_lines(), 1);
        assert_eq!(stats.rust_ratio_percent(), Some(100.0));
    }

    #[test]
    fn exempt_extension_matches_case_insensitively() {
        assert_eq!(exempt_extension(Path::new("a/b/App.DART")), Some("dart"));
        assert_eq!(exempt_extension(Path::new("lib.rs")), None);
        // 计入分母的扩展名不得同时被判为豁免（两个集合必须互斥）。
        for counted in COUNTED_EXTENSIONS {
            assert!(
                !EXEMPT_EXTENSIONS.contains(&counted),
                "`{counted}` 同时出现在统计与豁免集合中"
            );
        }
    }

    #[test]
    fn threshold_parsing_validates_range() {
        assert_eq!(parse_threshold("95").unwrap(), 95.0);
        assert_eq!(parse_threshold(" 87.5 ").unwrap(), 87.5);
        assert_eq!(parse_threshold("0").unwrap(), 0.0);
        assert_eq!(parse_threshold("100").unwrap(), 100.0);
        assert!(parse_threshold("-1").is_err());
        assert!(parse_threshold("100.1").is_err());
        assert!(parse_threshold("abc").is_err());
        assert!(parse_threshold("nan").is_err());
        assert!(parse_threshold("inf").is_err());
    }

    #[test]
    fn run_reports_usage_errors() {
        assert!(run(&[]).is_err());
        assert!(run(&["unknown".to_string()]).is_err());
        assert!(run(&["rust-ratio".to_string(), "--threshold".to_string()]).is_err());
        assert!(
            run(&[
                "rust-ratio".to_string(),
                "--threshold=not-a-number".to_string()
            ])
            .is_err()
        );
    }

    #[test]
    fn run_help_succeeds() {
        assert_eq!(
            run(&["help".to_string()]).unwrap(),
            ExitCode::from(EXIT_PASS)
        );
    }
}
