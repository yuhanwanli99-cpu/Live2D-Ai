//! 日志目录选择（0.4.2 rc.3「日志目录不跟 cwd」）。
//!
//! # 约定（真源 `docs/versions/0.4.md`「0.4.2」缺陷 2）
//!
//! 按优先级取**绝对目录**，**永不返回相对的 `logs`**：
//!
//! 1. `XDG_DATA_HOME` → `$XDG_DATA_HOME/live2d-ai/logs`；
//! 2. 否则 `HOME` → `$HOME/.local/share/live2d-ai/logs`——脚本 / 沙箱若把
//!    `HOME` 指向临时目录，日志就落在那个目录下（不再回退 cwd 相对 `logs`）；
//! 3. 否则 exe 在本地（不是 WSL 网络路径）→ `<exe 目录>/logs`，
//!    例：`C:\pack\live2d-ai-desktop.exe` → `C:\pack\logs`；
//! 4. 否则 `%LOCALAPPDATA%\live2d-ai\logs`（从注入环境取）；
//! 5. 都没有 → `Err`（中文说明），**绝不**回退相对路径。
//!
//! # 纯函数
//!
//! [`select_log_dir`] 只吃显式入参：exe 路径 + 环境查找闭包，**不读进程真实环境**。
//! 生产 wrapper 是 `logging::default_log_dir`（唯一读 `current_exe()` / `var_os`
//! 的地方）。Windows 风格路径先经 [`portable`] 统一成 `/` 分隔，所以同一份判据能在
//! Linux 上单测（与 `host_layout` 的 portable 同一口径；那个函数是私有的，本轮又不改
//! `host_layout.rs`，故这里保留一份同语义的小实现）。

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::host_layout;

/// 统一分隔符：Windows 的 `\` → `/`（与 `host_layout` 内部 portable 同口径）。
fn portable(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if text.contains('\\') {
        PathBuf::from(text.replace('\\', "/"))
    } else {
        path.to_path_buf()
    }
}

/// 绝对锚点：Unix 绝对路径，或 Windows 盘符路径（`C:/` / `c:\`）。
///
/// 相对路径一律不算锚点——否则拼出来的 `logs` 还是相对的，等于把缺陷换个名字。
fn is_absolute_anchor(path: &Path) -> bool {
    if path.is_absolute() {
        return true;
    }
    let text = path.to_string_lossy();
    let bytes = text.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'/' || bytes[2] == b'\\')
}

/// 取一个**非空且绝对**的环境目录；空串 / 相对值按未设置处理。
fn env_dir(lookup: &mut impl FnMut(&str) -> Option<OsString>, key: &str) -> Option<PathBuf> {
    lookup(key)
        .filter(|value| !value.is_empty())
        .map(|value| portable(Path::new(&value)))
        .filter(|path| is_absolute_anchor(path))
}

/// 选择日志目录（纯函数）：XDG → HOME → exe 目录（非 WSL 网络路径）→
/// `%LOCALAPPDATA%`；无锚点返回 `Err`（中文说明），**永不返回相对的 `logs`**。
///
/// `exe` 传 `Some` 即视为「exe 路径存在」（本函数不探文件系统）；`lookup`
/// 由调用方注入（生产 = `std::env::var_os`，测试 = 固定表）。
pub fn select_log_dir(
    exe: Option<&Path>,
    mut lookup: impl FnMut(&str) -> Option<OsString>,
) -> Result<PathBuf, String> {
    if let Some(xdg) = env_dir(&mut lookup, "XDG_DATA_HOME") {
        return Ok(xdg.join("live2d-ai").join("logs"));
    }
    if let Some(home) = env_dir(&mut lookup, "HOME") {
        return Ok(home
            .join(".local")
            .join("share")
            .join("live2d-ai")
            .join("logs"));
    }
    if let Some(dir) = exe
        .filter(|exe| !host_layout::is_wsl_network_path(exe))
        .and_then(host_layout::exe_dir)
        .filter(|dir| is_absolute_anchor(dir))
    {
        return Ok(dir.join("logs"));
    }
    if let Some(local_app_data) = env_dir(&mut lookup, "LOCALAPPDATA") {
        return Ok(local_app_data.join("live2d-ai").join("logs"));
    }
    Err("无法确定日志目录：XDG_DATA_HOME / HOME / exe 目录 / LOCALAPPDATA 都不是可用的绝对锚点；拒绝回退到相对的 logs（请设置 HOME 或 LOCALAPPDATA）"
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// 注入式环境：只含给定键值（未列出的键 = 未设置）。
    fn env_of(pairs: &[(&str, &str)]) -> impl FnMut(&str) -> Option<OsString> {
        let map: HashMap<String, OsString> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), OsString::from(value)))
            .collect();
        move |key| map.get(key).cloned()
    }

    fn resolve(exe: Option<&Path>, pairs: &[(&str, &str)]) -> Result<PathBuf, String> {
        select_log_dir(exe, env_of(pairs))
    }

    /// 「永不相对」：Unix 绝对路径，或 Windows 盘符路径。
    fn assert_absolute_anchor(path: &Path) {
        let text = path.to_string_lossy();
        let bytes = text.as_bytes();
        let drive = bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && (bytes[2] == b'/' || bytes[2] == b'\\');
        assert!(
            path.is_absolute() || drive,
            "日志目录必须是绝对路径（不得相对）: {text}"
        );
        assert_ne!(path, Path::new("logs"), "禁止返回相对的 logs");
    }

    #[test]
    fn xdg_data_home_wins() {
        let exe = Path::new(r"C:\pack\live2d-ai-desktop.exe");
        let dir = resolve(
            Some(exe),
            &[
                ("XDG_DATA_HOME", "/tmp/xdg-data"),
                ("HOME", "/home/skystar"),
                ("LOCALAPPDATA", r"C:\Users\skystar\AppData\Local"),
            ],
        )
        .unwrap();
        assert_eq!(dir, Path::new("/tmp/xdg-data/live2d-ai/logs"));
        assert_absolute_anchor(&dir);
    }

    #[test]
    fn home_is_the_fallback_without_xdg() {
        let dir = resolve(
            None,
            &[
                ("HOME", "/home/skystar"),
                ("LOCALAPPDATA", r"C:\Users\skystar\AppData\Local"),
            ],
        )
        .unwrap();
        assert_eq!(dir, Path::new("/home/skystar/.local/share/live2d-ai/logs"));
        assert_absolute_anchor(&dir);
    }

    /// Windows 双击常见形态：没有 XDG_DATA_HOME / HOME，exe 在本地目录。
    #[test]
    fn exe_directory_is_the_windows_desktop_fallback() {
        let exe = Path::new(r"C:\pack\live2d-ai-desktop.exe");
        let dir = resolve(
            Some(exe),
            &[("LOCALAPPDATA", r"C:\Users\skystar\AppData\Local")],
        )
        .unwrap();
        assert_eq!(dir, Path::new("C:/pack/logs"));
        assert_absolute_anchor(&dir);
    }

    /// 从 WSL 网络位置直接运行 exe：exe 目录不是锚点 → 落到 %LOCALAPPDATA%。
    #[test]
    fn wsl_network_exe_falls_back_to_local_appdata() {
        let exe = Path::new(r"\\wsl$\Ubuntu\home\skystar\pack\live2d-ai-desktop.exe");
        let dir = resolve(
            Some(exe),
            &[("LOCALAPPDATA", r"C:\Users\skystar\AppData\Local")],
        )
        .unwrap();
        assert_eq!(
            dir,
            Path::new("C:/Users/skystar/AppData/Local/live2d-ai/logs")
        );
        assert_absolute_anchor(&dir);
        // 连 LOCALAPPDATA 也没有：必须 Err，不能悄悄落到相对 logs。
        assert!(resolve(Some(exe), &[]).is_err());
    }

    /// 没有任何绝对锚点 → Err（中文说明），而不是相对 logs。
    #[test]
    fn no_anchor_is_an_error_not_a_relative_fallback() {
        let err = resolve(None, &[]).unwrap_err();
        assert!(err.contains("LOCALAPPDATA"), "错误要指明缺哪个变量: {err}");
        assert!(err.contains("logs"), "错误要说明拒绝相对的 logs: {err}");
        // exe 是裸文件名 / 相对路径 / 空串：都不是锚点。
        for exe in ["live2d-ai-desktop.exe", "sub/live2d-ai-desktop.exe", ""] {
            let err = resolve(Some(Path::new(exe)), &[]).unwrap_err();
            assert!(err.contains("LOCALAPPDATA"), "{exe}: {err}");
        }
    }

    /// 空串环境变量不是锚点（老实现会把 XDG="" 拼成相对 live2d-ai/logs）。
    #[test]
    fn empty_environment_values_are_ignored() {
        let exe = Path::new("/opt/live2d-ai/live2d-ai-desktop.exe");
        let dir = resolve(
            Some(exe),
            &[("XDG_DATA_HOME", ""), ("HOME", ""), ("LOCALAPPDATA", "")],
        )
        .unwrap();
        assert_eq!(dir, Path::new("/opt/live2d-ai/logs"));
        assert_absolute_anchor(&dir);
        assert!(resolve(None, &[("XDG_DATA_HOME", ""), ("HOME", "")]).is_err());
    }

    /// 所有成功结果都是绝对锚点；无锚点一律 Err。
    #[test]
    fn resolutions_are_never_relative() {
        let exe = Path::new(r"C:\pack\live2d-ai-desktop.exe");
        let wsl = Path::new(r"\\wsl.localhost\Ubuntu\home\x\live2d-ai-desktop.exe");
        let cases = [
            resolve(Some(exe), &[("XDG_DATA_HOME", "/tmp/xdg")]),
            resolve(Some(exe), &[("HOME", "/home/u")]),
            resolve(Some(exe), &[]),
            resolve(Some(wsl), &[("LOCALAPPDATA", r"C:\Users\u\AppData\Local")]),
        ];
        for dir in cases {
            assert_absolute_anchor(&dir.unwrap());
        }
        assert!(resolve(None, &[]).is_err(), "无锚点时禁止返回相对 logs");
    }
}
