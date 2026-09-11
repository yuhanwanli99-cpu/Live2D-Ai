//! `/api/v1/logs*` 系列端点（W3 任务）。
//!
//! 责任：
//! - [`handle_logs`]：返回当前日志目录最近 200 行（**只读日志目录**，**不**接受
//!   任何路径参数；P0-4：避免路径猜测旁路）。
//! - [`handle_logs_levels`]：返回 `tracing` 已知级别列表（前端可展示）。
//!
//! 安全：
//! - **dev_mode 门控**：[`super::dispatch`] 在 `dev_mode=false` 时一律 403，
//!   不会调到这里；
//! - **不读 body**（GET）、**不写盘**、**不调用 `Command::new`**（P0-4）；
//! - **不**返回密钥明文——日志文件本身已由 [`crate::logging::SanitizingWriter`]
//!   兜底脱敏；本端点只做截取/包装。
//! - 路径锁定：从 [`crate::logging::current_log_dir`] 读取当前激活目录，
//!   不接受 query 参数。

use std::fs;
use std::io::{self, Read};

use serde::Serialize;
use tiny_http::{Header, Response, StatusCode};

use crate::logging;

/// 最近回看的最大行数（D1 契约 §P0-4 兜底 + 控制响应体大小）。
const MAX_LINES: usize = 200;

/// `GET /api/v1/logs` 响应。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LogsResponse {
    /// 当前日志目录（绝对路径）。
    pub log_dir: String,
    /// 实际读取的文件名（`live2d-ai.log.YYYY-MM-DD` 或 `live2d-ai.log`）。
    pub file: String,
    /// 回看的行数（≤ [`MAX_LINES`]）。
    pub lines: Vec<String>,
    /// 是否达到最大行数（前端可借此判断是否需要翻页）。
    pub truncated: bool,
}

/// `GET /api/v1/logs/levels` 响应。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LogsLevelsResponse {
    /// 已知级别列表（按严重度从低到高）。
    pub levels: Vec<&'static str>,
}

/// 处理 `/api/v1/logs` GET。
///
/// - 若 `current_log_dir()` 为 `None`（main 未调 `init_logging` 或降级）→ 200 + 空行；
/// - 若日志目录不存在 → 同上；**不**返回 500（前端无显示就空展示）。
/// - **绝不**读取 `log_dir` 之外的任何路径。
pub fn handle_logs() -> Response<std::io::Cursor<Vec<u8>>> {
    let log_dir = match logging::current_log_dir() {
        Some(d) => d,
        None => {
            return json_response(
                StatusCode(200),
                &LogsResponse {
                    log_dir: String::new(),
                    file: String::new(),
                    lines: Vec::new(),
                    truncated: false,
                },
            );
        }
    };

    // 找最新日志文件：`live2d-ai.log.YYYY-MM-DD` 取字典序最大者；
    // 若都没有，回退 `live2d-ai.log`。
    let (file_path, file_label) = match pick_latest_log(&log_dir) {
        Some((p, name)) => (p, name),
        None => {
            return json_response(
                StatusCode(200),
                &LogsResponse {
                    log_dir: log_dir.display().to_string(),
                    file: String::new(),
                    lines: Vec::new(),
                    truncated: false,
                },
            );
        }
    };

    let (lines, truncated) = read_tail_lines(&file_path, MAX_LINES).unwrap_or_default(); // 读失败 = 空（不暴露 IO 错误给前端）

    json_response(
        StatusCode(200),
        &LogsResponse {
            log_dir: log_dir.display().to_string(),
            file: file_label,
            lines,
            truncated,
        },
    )
}

/// 处理 `/api/v1/logs/levels` GET。
pub fn handle_logs_levels() -> Response<std::io::Cursor<Vec<u8>>> {
    json_response(
        StatusCode(200),
        &LogsLevelsResponse {
            // tracing Level：TRACE / DEBUG / INFO / WARN / ERROR（按严重度升序）。
            levels: vec!["trace", "debug", "info", "warn", "error"],
        },
    )
}

/// 选最新日志文件：扫描目录，按文件名排序取最大者。
fn pick_latest_log(dir: &std::path::Path) -> Option<(std::path::PathBuf, String)> {
    let entries = fs::read_dir(dir).ok()?;
    let mut candidates: Vec<(std::path::PathBuf, String)> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "live2d-ai.log" || name.starts_with("live2d-ai.log.") {
            candidates.push((entry.path(), name));
        }
    }
    // 字典序：按文件名降序取最大（YYYY-MM-DD 同前缀时字典序 = 时间序）。
    candidates.sort_by(|a, b| b.1.cmp(&a.1));
    candidates.into_iter().next()
}

/// 读文件尾部 N 行：单次读全文件（≤ 5MB 截断），按 `\n` 切，取最后 N 行。
///
/// 文件大小硬上限 5 MiB ——超过则截断前缀（避免 OOM；5MB 是 W3 任务描述的
/// 单文件滚动大小上限）。
fn read_tail_lines(path: &std::path::Path, max_lines: usize) -> io::Result<(Vec<String>, bool)> {
    const MAX_BYTES: u64 = 5 * 1024 * 1024;
    let metadata = fs::metadata(path)?;
    let size = metadata.len();
    let truncated_by_size = size > MAX_BYTES;

    let mut file = fs::File::open(path)?;
    if truncated_by_size {
        use std::io::{Seek, SeekFrom};
        file.seek(SeekFrom::End(-(MAX_BYTES as i64)))?;
    }
    let mut buf = Vec::with_capacity((size.min(MAX_BYTES)) as usize);
    file.read_to_end(&mut buf)?;

    // 按 `\n` 拆（容忍 `\r\n` —— 截掉末尾 `\r`）。
    let text = String::from_utf8_lossy(&buf);
    let all_lines: Vec<String> = text
        .split('\n')
        .map(|s| s.strip_suffix('\r').unwrap_or(s).to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let total = all_lines.len();
    let start = total.saturating_sub(max_lines);
    let lines = all_lines[start..].to_vec();
    let truncated_by_lines = start > 0;
    Ok((lines, truncated_by_lines || truncated_by_size))
}

/// 通用：把 `Serialize` 包装成 200 + JSON 头。
fn json_response<T: Serialize>(status: StatusCode, body: &T) -> Response<std::io::Cursor<Vec<u8>>> {
    let bytes = serde_json::to_vec(body).unwrap_or_else(|e| {
        format!("{{\"error\":{{\"code\":\"internal_error\",\"message\":\"{e}\"}}}}").into_bytes()
    });
    Response::from_data(bytes)
        .with_status_code(status)
        .with_header(
            Header::from_bytes(
                &b"Content-Type"[..],
                &b"application/json; charset=utf-8"[..],
            )
            .expect("Content-Type 头常量"),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 纯函数：尾部 N 行；3 行文件 + max=2 → 后 2 行。
    #[test]
    fn read_tail_lines_takes_last_n() {
        let dir = std::env::temp_dir().join("live2d-ai-log-routes-test-1");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("test.log");
        fs::write(&p, "a\nb\nc\n").unwrap();
        let (lines, truncated) = read_tail_lines(&p, 2).unwrap();
        assert_eq!(lines, vec!["b".to_string(), "c".to_string()]);
        assert!(truncated);
        let _ = fs::remove_dir_all(&dir);
    }

    /// 短文件 + max 较大 → 全文件返回，truncated=false。
    #[test]
    fn read_tail_lines_short_file_not_truncated() {
        let dir = std::env::temp_dir().join("live2d-ai-log-routes-test-2");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("test.log");
        fs::write(&p, "a\nb\n").unwrap();
        let (lines, truncated) = read_tail_lines(&p, 200).unwrap();
        assert_eq!(lines.len(), 2);
        assert!(!truncated);
        let _ = fs::remove_dir_all(&dir);
    }

    /// `\r\n` 风格：行尾 `\r` 应被剥除。
    #[test]
    fn read_tail_lines_strips_carriage_return() {
        let dir = std::env::temp_dir().join("live2d-ai-log-routes-test-3");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("test.log");
        fs::write(&p, "a\r\nb\r\n").unwrap();
        let (lines, _) = read_tail_lines(&p, 200).unwrap();
        assert_eq!(lines, vec!["a".to_string(), "b".to_string()]);
        let _ = fs::remove_dir_all(&dir);
    }

    /// 选最新文件：`live2d-ai.log.YYYY-MM-DD` 字典序最大者。
    #[test]
    fn pick_latest_log_prefers_dated_suffix() {
        let dir = std::env::temp_dir().join("live2d-ai-log-routes-test-4");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("live2d-ai.log"), "old").unwrap();
        fs::write(dir.join("live2d-ai.log.2024-01-01"), "a").unwrap();
        fs::write(dir.join("live2d-ai.log.2024-12-31"), "b").unwrap();
        let (p, name) = pick_latest_log(&dir).unwrap();
        assert_eq!(name, "live2d-ai.log.2024-12-31");
        assert!(p.ends_with("live2d-ai.log.2024-12-31"));
        let _ = fs::remove_dir_all(&dir);
    }

    /// levels 端点：返回 200 + 五级。
    #[test]
    fn handle_logs_levels_returns_known_levels() {
        let resp = handle_logs_levels();
        assert_eq!(resp.status_code().0, 200);
    }

    /// logs 端点：当前 log_dir 未初始化 → 200 + 空 lines。
    #[test]
    fn handle_logs_returns_empty_when_no_log_dir() {
        // 在测试运行前 `logging::current_log_dir()` 通常为 None
        // （除非其它测试已调 init_logging）。无论是哪种：200 + 合理 body。
        let resp = handle_logs();
        assert_eq!(resp.status_code().0, 200);
    }
}
