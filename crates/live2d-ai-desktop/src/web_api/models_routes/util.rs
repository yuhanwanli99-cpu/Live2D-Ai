//! 模型路由内部工具函数（路径 / 时间 / 大小）。
//!
//! 拆出原因：`handlers.rs` 已接近 500 行；这些函数与 HTTP 业务无强耦合，
//! 独立模块便于复用 + 单测。

use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// 在 `dir` 下找一个 `*.model3.json` 文件：先看顶层，**再向下看一层**
/// （子目录按名字排序，取第一个命中）。
///
/// # 为什么要向下看一层（rc.2 2026-09-12）
///
/// 本仓库的既有布局就是深一层：`assets/models/bai/runtime/bai.model3.json`。
/// 原实现只扫顶层，于是 `POST /models/import {"id":"bai"}` 必然回
/// `400 invalid_model3_json`——「连自家自带的模型都导入失败」。
///
/// **只限一层**：更深会开始把临时目录、备份目录、另一个模型包也当成命中，
/// 「导入 id=X 却登记到 X 内部某个无关包」是很难查的错。只递归一层时，
/// 命中集合仍是「这个 id 自己」加「它的直接子目录」，语义可控。
///
/// 不展开符号链接目录（与 `compute_dir_size` 的保守口径一致）。
pub(super) fn find_model3_json(dir: &Path) -> Option<PathBuf> {
    if let Some(p) = find_model3_json_shallow(dir) {
        return Some(p);
    }
    let mut subdirs: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| !p.is_symlink() && p.is_dir())
        .collect();
    // 排序保证确定性：同一目录树每次导入得到同一个 model3.json。
    subdirs.sort();
    subdirs
        .into_iter()
        .find_map(|d| find_model3_json_shallow(&d))
}

/// 只在 `dir` 顶层找一个 `*.model3.json`（取第一个匹配的）。
fn find_model3_json_shallow(dir: &Path) -> Option<PathBuf> {
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

/// 把路径文本统一成 `/` 分隔，并剥掉 Windows verbatim / 设备前缀。
///
/// Windows 的 `canonicalize` 返回 `\\?\D:\...`（verbatim），而模型根来自
/// `cwd.join(...)`（普通盘符路径）；两者直接 `strip_prefix` 必然失败，于是旧
/// 实现把**绝对路径**写进 registry，模型 URL 变成 `/models///?/D:/...` → 404
/// → 换模静默回退 bai（2026-11-09 真机导入第三方皮肤实测）。
pub(super) fn normalize_path_text(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    strip_windows_prefix(&text)
}

fn strip_windows_prefix(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    // `\\?\UNC\server\share\x` → `//server/share/x`
    if lower.starts_with("//?/unc/") {
        return format!("//{}", &text[8..]);
    }
    // `\\?\C:\x` / `\\.\C:\x` → `C:/x`
    if lower.starts_with("//?/") || lower.starts_with("//./") {
        return text[4..].to_string();
    }
    text.to_string()
}

/// 从模型根与 model3 绝对路径算**相对模型根**的路径（纯函数，不读盘）。
///
/// 结果一律 `/` 分隔、非空、不以 `/` 开头、不含 `.` / `..` 段；否则 `Err`
/// （handler 映射 400，绝不把绝对路径写进 registry）。比较前缀时按 ASCII
/// 忽略大小写（Windows 盘符 / 目录名不区分大小写），非 ASCII 段原样保留。
pub(super) fn model3_rel_path_from_abs(
    assets_root: &Path,
    model3_abs: &Path,
) -> Result<String, String> {
    let root = normalize_path_text(assets_root);
    let file = normalize_path_text(model3_abs);
    let root = root.trim_end_matches('/');
    let file = file.trim_end_matches('/');
    let rel =
        strip_root_prefix(root, file).ok_or_else(|| format!("{file} 不在模型根 {root} 之下"))?;
    validate_model3_rel(&rel)?;
    Ok(rel)
}

fn strip_root_prefix(root: &str, file: &str) -> Option<String> {
    let root_bytes = root.as_bytes();
    let file_bytes = file.as_bytes();
    if file_bytes.len() <= root_bytes.len() {
        return None;
    }
    if !file_bytes[..root_bytes.len()].eq_ignore_ascii_case(root_bytes) {
        return None;
    }
    if file_bytes[root_bytes.len()] != b'/' {
        return None;
    }
    Some(file[root_bytes.len() + 1..].to_string())
}

/// 相对路径校验：非空、无前导 `/`、无 `.` / `..` / 空段（与
/// `normalize_relative_id` 同一口径，针对多段相对路径）。
pub(super) fn validate_model3_rel(rel: &str) -> Result<(), String> {
    if rel.is_empty() {
        return Err("相对路径为空".to_string());
    }
    if rel.starts_with('/') {
        return Err(format!("拒绝绝对路径: {rel}"));
    }
    for seg in rel.split('/') {
        if seg.is_empty() {
            return Err(format!("路径含空段: {rel}"));
        }
        if seg == "." || seg == ".." {
            return Err(format!("路径含 . / .. 段: {rel}"));
        }
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_windows_verbatim_and_unc_prefixes() {
        assert_eq!(
            normalize_path_text(Path::new(
                r"\\?\D:\live2d-ai\assets\models\skin\x.model3.json"
            )),
            "D:/live2d-ai/assets/models/skin/x.model3.json"
        );
        assert_eq!(
            normalize_path_text(Path::new(r"\\?\UNC\server\share\models\skin\x.model3.json")),
            "//server/share/models/skin/x.model3.json"
        );
        assert_eq!(
            normalize_path_text(Path::new(r"\\.\D:\x\y.json")),
            "D:/x/y.json"
        );
        assert_eq!(
            normalize_path_text(Path::new("/opt/live2d-ai/x.json")),
            "/opt/live2d-ai/x.json"
        );
    }

    /// 回归（2026-11-09 真机）：canonicalize 出来的 verbatim 绝对路径必须能
    /// 相对模型根；否则 registry 里会写成 `//?/D:/...`，URL 变 404。
    #[test]
    fn rel_path_from_verbatim_absolute_model3() {
        let root = Path::new(r"\\?\D:\live2d-ai\Live2D-Ai\assets\models");
        let abs =
            Path::new(r"\\?\D:\live2d-ai\Live2D-Ai\assets\models\skin\runtime\skin.model3.json");
        assert_eq!(
            model3_rel_path_from_abs(root, abs).expect("verbatim → 相对"),
            "skin/runtime/skin.model3.json"
        );

        // 混合形态：模型根是普通盘符，model3 是 verbatim（真实 Windows 主路径）。
        let plain_root = Path::new(r"D:\live2d-ai\assets\models");
        let verbatim_abs = Path::new(r"\\?\D:\live2d-ai\assets\models\skin\skin.model3.json");
        assert_eq!(
            model3_rel_path_from_abs(plain_root, verbatim_abs).expect("普通根 + verbatim 文件"),
            "skin/skin.model3.json"
        );

        // UNC verbatim。
        let unc_root = Path::new(r"\\server\share\models");
        let unc_abs = Path::new(r"\\?\UNC\server\share\models\skin\x.model3.json");
        assert_eq!(
            model3_rel_path_from_abs(unc_root, unc_abs).expect("UNC verbatim"),
            "skin/x.model3.json"
        );
    }

    #[test]
    fn rel_path_from_linux_absolute_model3() {
        let root = Path::new("/opt/live2d-ai/assets/models");
        let abs = Path::new("/opt/live2d-ai/assets/models/skin/x.model3.json");
        assert_eq!(
            model3_rel_path_from_abs(root, abs).expect("Linux 绝对路径"),
            "skin/x.model3.json"
        );
    }

    #[test]
    fn rel_path_rejects_outside_root_or_traversal() {
        let root = Path::new("/opt/live2d-ai/assets/models");
        assert!(
            model3_rel_path_from_abs(root, Path::new("/opt/live2d-ai/other/x.model3.json"))
                .is_err()
        );
        assert!(
            model3_rel_path_from_abs(root, root).is_err(),
            "根自身没有相对路径"
        );
        assert!(
            model3_rel_path_from_abs(root, Path::new("/opt/live2d-ai/assets/models/../x.json"))
                .is_err(),
            "含 .. 必须拒绝"
        );
    }

    /// 中文 / 空格路径（用户实际目录，如 `D:\皮套 测试\`）：按字节前缀比较必须
    /// 不破 UTF-8 边界。
    #[test]
    fn rel_path_preserves_non_ascii_segments() {
        let root = Path::new("/mnt/d/皮套 测试/models");
        let abs = Path::new("/mnt/d/皮套 测试/models/大肥鱼/x.model3.json");
        assert_eq!(
            model3_rel_path_from_abs(root, abs).expect("中文路径"),
            "大肥鱼/x.model3.json"
        );
    }
}
