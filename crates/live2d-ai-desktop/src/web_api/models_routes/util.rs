//! 模型路由内部工具函数（路径 / 时间 / 大小）。
//!
//! 拆出原因：`handlers.rs` 已接近 500 行；这些函数与 HTTP 业务无强耦合，
//! 独立模块便于复用 + 单测。

use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// 在 `dir` 顶层找一个 `*.model3.json` 文件（取第一个匹配的）。
pub(super) fn find_model3_json(dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for e in entries.flatten() {
        let p = e.path();
        if p.is_file()
            && p.extension().and_then(|s| s.to_str()) == Some("json")
            && p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|n| n.ends_with(".model3.json"))
        {
            return Some(p);
        }
    }
    None
}

/// 递归累计目录大小（符号链接用 `symlink_metadata`，不展开外指）。
pub(super) fn compute_dir_size(dir: &Path) -> io::Result<u64> {
    let mut total = 0u64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(p) = stack.pop() {
        let meta = match std::fs::symlink_metadata(&p) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.is_file() {
            total += meta.len();
        } else if meta.is_dir()
            && let Ok(entries) = std::fs::read_dir(&p)
        {
            for e in entries.flatten() {
                stack.push(e.path());
            }
        }
    }
    Ok(total)
}

/// ISO 8601 UTC 毫秒精度（与 `app_routes::system_time_iso8601` 同口径）。
pub(super) fn now_iso8601() -> String {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let total_ms = dur.as_millis();
    let secs = (total_ms / 1000) as i64;
    let ms = (total_ms % 1000) as u32;
    let (y, mo, d, h, mi, s) = unix_secs_to_ymdhms(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{ms:03}Z")
}

fn unix_secs_to_ymdhms(secs: i64) -> (i32, u32, u32, u32, u32, u32) {
    if secs < 0 {
        return (1970, 1, 1, 0, 0, 0);
    }
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let hour = (rem / 3600) as u32;
    let min = ((rem % 3600) / 60) as u32;
    let sec = (rem % 60) as u32;
    let mut y = 1970i32;
    let mut d = days;
    loop {
        let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
        let yd = if leap { 366 } else { 365 };
        if d < yd {
            break;
        }
        d -= yd;
        y += 1;
        if y > 2100 {
            return (1970, 1, 1, 0, 0, 0);
        }
    }
    let mdays: [u32; 12] = if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0usize;
    while m < 12 && d >= mdays[m] as i64 {
        d -= mdays[m] as i64;
        m += 1;
    }
    (y, (m as u32) + 1, (d as u32) + 1, hour, min, sec)
}

/// 当前 unix 秒（用于 P1-5 损坏备份文件名后缀）。
pub(super) fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
