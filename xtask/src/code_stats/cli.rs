//! `code-stats` 参数解析（`--x=y` 与 `--x y` 两种写法都收）。

use super::gates::{Gate, Limits};
use super::*;

/// 命令行选项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// 是否把门禁结果落到退出码。
    pub check: bool,
    /// 只打印门禁段（CI 日志友好）。
    pub quiet: bool,
    /// 打印测试模块文件等长清单。
    pub verbose: bool,
    /// `--check` 时实际参与判定的门禁（空 = 全集）。
    pub gates: Vec<Gate>,
    pub limits: Limits,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            check: false,
            quiet: false,
            verbose: false,
            gates: Vec::new(),
            limits: Limits::ratchet(),
        }
    }
}

/// 解析 `code-stats` 的参数（`--x=y` 与 `--x y` 两种写法都收）。
///
/// # Errors
/// 未知参数 / 缺参数值 / 非法数值。
pub fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut opts = Options::default();
    let mut strict_plan = false;
    let mut over_500: Option<u64> = None;
    let mut over_1000: Option<u64> = None;
    let mut dart_800: Option<u64> = None;
    let mut desktop_deps: Option<u64> = None;

    let normalized = normalize_flags(args);
    let mut index = 0;
    while index < normalized.len() {
        let flag = normalized[index].clone();
        // 取参数值：`normalize_flags` 已把 `--flag=value` 拆成两个元素，这里只看下一个。
        let take_value = |name: &str| -> Result<(String, usize), String> {
            match normalized.get(index + 1) {
                Some(value) if !value.starts_with("--") => Ok((value.clone(), index + 2)),
                _ => Err(format!("{name} 缺少参数值")),
            }
        };
        let next = match flag.as_str() {
            "--check" => {
                opts.check = true;
                index + 1
            }
            "--quiet" => {
                opts.quiet = true;
                index + 1
            }
            "--verbose" => {
                opts.verbose = true;
                index + 1
            }
            "--strict-plan" => {
                strict_plan = true;
                index + 1
            }
            "--only" | "--gate" => {
                let (value, used) = take_value("--only")?;
                let gate = Gate::parse(value.trim()).ok_or_else(|| {
                    format!("--only 只认 lines / over-1000 / deps，得到 `{value}`")
                })?;
                if !opts.gates.contains(&gate) {
                    opts.gates.push(gate);
                }
                used
            }
            "--max-src-rs-500" => {
                let (value, used) = take_value("--max-src-rs-500")?;
                over_500 = Some(parse_count("--max-src-rs-500", &value)?);
                used
            }
            "--max-src-rs-1000" => {
                let (value, used) = take_value("--max-src-rs-1000")?;
                over_1000 = Some(parse_count("--max-src-rs-1000", &value)?);
                used
            }
            "--max-dart-800" => {
                let (value, used) = take_value("--max-dart-800")?;
                dart_800 = Some(parse_count("--max-dart-800", &value)?);
                used
            }
            "--max-deps" => {
                let (value, used) = take_value("--max-deps")?;
                desktop_deps = Some(parse_deps_limit(&value)?);
                used
            }
            other => return Err(format!("无法识别的参数 `{other}`")),
        };
        index = next;
    }

    let base = if strict_plan {
        Limits::plan()
    } else {
        opts.limits
    };
    opts.limits = Limits {
        src_rs_over_500: over_500.unwrap_or(base.src_rs_over_500),
        src_rs_over_1000: over_1000.unwrap_or(base.src_rs_over_1000),
        dart_over_800: dart_800.unwrap_or(base.dart_over_800),
        desktop_deps: desktop_deps.unwrap_or(base.desktop_deps),
    };
    Ok(opts)
}

/// 把 `--flag=value` 拆成 `--flag` + `value` 两个元素；其余原样。
fn normalize_flags(args: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len());
    for arg in args {
        match arg.split_once('=') {
            Some((flag, value)) if flag.starts_with("--") && !value.is_empty() => {
                out.push(flag.to_string());
                out.push(value.to_string());
            }
            _ => out.push(arg.clone()),
        }
    }
    out
}

fn parse_count(flag: &str, value: &str) -> Result<u64, String> {
    value
        .trim()
        .parse::<u64>()
        .map_err(|_| format!("{flag} 需要非负整数，得到 `{value}`"))
}

/// `--max-deps live2d-ai-desktop=34`（只认预算点名的那个 crate）。
fn parse_deps_limit(value: &str) -> Result<u64, String> {
    let (crate_name, count) = value
        .split_once('=')
        .ok_or_else(|| format!("--max-deps 需要 `<crate>=<n>`，得到 `{value}`"))?;
    if crate_name.trim() != DESKTOP_CRATE {
        return Err(format!(
            "--max-deps 目前只支持 `{DESKTOP_CRATE}`（PLAN §2.5 点名的预算点），得到 `{crate_name}`"
        ));
    }
    parse_count("--max-deps", count)
}
