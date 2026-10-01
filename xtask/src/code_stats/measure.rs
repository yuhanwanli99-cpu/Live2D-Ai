//! 行数口径（`wc -l` 语义）、`#[cfg(test)]` 边界与 Cargo 依赖计数。
//!
//! 这些是「尺子」本身：纯函数、可直接单测，不碰文件系统遍历。

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

// ── 行数与 `#[cfg(test)]` 边界 ────────────────────────────────────────────────

/// 物理行数（`wc -l` 语义）：`\n` 个数；末尾无换行的残行补 1。
pub fn physical_lines(text: &str) -> u64 {
    if text.is_empty() {
        return 0;
    }
    let newlines = text.bytes().filter(|byte| *byte == b'\n').count() as u64;
    if text.ends_with('\n') {
        newlines
    } else {
        newlines + 1
    }
}

/// 文件里被 `#[cfg(test)]` 条目占掉的行数（块精确口径）。
///
/// 边界规则：从 `#[cfg(test)]` 属性行起，按花括号计数走到**配对的 `}`** 行（含）；
/// 若该条目没有花括号（`#[cfg(test)] mod x;` / `use ...;`），则算到文件末尾。
/// 属性行本身计入内联测试。
pub fn inline_test_lines(text: &str) -> u64 {
    let mut lines: Vec<&str> = text.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    let mut masked = vec![false; lines.len()];
    let mut index = 0;
    while index < lines.len() {
        if is_cfg_test_attr(lines[index]) {
            let end = block_end(&lines, index);
            for flag in masked.iter_mut().take(end + 1).skip(index) {
                *flag = true;
            }
            index = end + 1;
        } else {
            index += 1;
        }
    }
    masked.iter().filter(|flag| **flag).count() as u64
}

fn is_cfg_test_attr(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed == "#[cfg(test)]" || trimmed.starts_with("#[cfg(test)]")
}

/// 从属性行出发找条目结束行。
///
/// 两条形态（**与 `mod.rs` 头注写的口径逐字一致**）：
/// - **无花括号条目**（`#[cfg(test)] mod x;` / `use ...;`）：自 attr 起，在遇到 `{` 之前
///   先遇到 `;` ⇒ 没有块体 ⇒ **吃到文件末尾**（`lines.len() - 1`）。注意不能「往后继续
///   扫到第一行括号归零就收尾」——那样会被后续的 `use serde::{Deserialize};`
///   （一行里同时有 `{` `}` ⇒ 收支归零）提前截断，把内联测试算少、生产算多
///   （verifier 2026-10-01 实测：`settings.rs` inline 9 / 应为 793，全仓 7 文件 2,605 行）；
/// - **有花括号条目**：花括号配对到 `depth <= 0` 的那一行（含）。
fn block_end(lines: &[&str], start: usize) -> usize {
    let mut depth: i64 = 0;
    let mut seen_brace = false;
    let mut index = start;
    while index < lines.len() {
        let line = lines[index];
        if !seen_brace {
            match line.find('{') {
                // 同一行里 `;` 在 `{` 之前 ⇒ 这条声明没有块体。
                Some(brace_at) if line.find(';').is_some_and(|semi| semi < brace_at) => {
                    return lines.len().saturating_sub(1);
                }
                Some(_) => seen_brace = true,
                // 整行无 `{` 但有 `;` ⇒ 无花括号声明（`mod x;`）。
                None if line.contains(';') => return lines.len().saturating_sub(1),
                None => {}
            }
        }
        depth += brace_delta(line);
        if seen_brace && depth <= 0 {
            return index;
        }
        index += 1;
    }
    lines.len().saturating_sub(1)
}

fn brace_delta(line: &str) -> i64 {
    let mut delta = 0_i64;
    for byte in line.bytes() {
        match byte {
            b'{' => delta += 1,
            b'}' => delta -= 1,
            _ => {}
        }
    }
    delta
}

// ── Cargo 依赖 ────────────────────────────────────────────────────────────────

/// 顶层 `[dependencies]` 的键数（`[dependencies.x]` 子表也算 1 条）。
///
/// 不数 `[target.*.dependencies]` / `[dev-dependencies]` / `[build-dependencies]`：
/// PLAN §2.5 的口径是「运行依赖预算」，与 `cargo tree --depth 1` 的直觉一致。
pub fn count_dependencies(manifest: &Path) -> u64 {
    let Ok(text) = fs::read_to_string(manifest) else {
        return 0;
    };
    count_dependencies_in(&text)
}

/// [`count_dependencies`] 的纯函数版本（便于单测）。
pub fn count_dependencies_in(text: &str) -> u64 {
    let mut section = String::new();
    let mut keys: BTreeSet<String> = BTreeSet::new();
    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(header) = line.strip_prefix('[') {
            section = header.trim_end_matches(']').trim().to_string();
            if let Some(sub) = section.strip_prefix("dependencies.") {
                keys.insert(sub.to_string());
            }
            continue;
        }
        if section != "dependencies" {
            continue;
        }
        let Some((key, _)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        // `foo.bar = ...` 这类点号键只数顶层 `foo`。
        keys.insert(key.split('.').next().unwrap_or(key).to_string());
    }
    keys.len() as u64
}

/// 去掉 `#` 之后的注释（TOML 的 `#` 不出现在不带引号的键里）。
fn strip_comment(line: &str) -> &str {
    match line.split_once('#') {
        Some((head, _)) => head,
        None => line,
    }
}
