//! 宿主锚点的纯函数（0.4.0 rc.1「Windows 包」）。
//!
//! 打包后的目录布局（真源 `docs/versions/0.4.md`）：
//!
//! ```text
//! live2d-ai-desktop.exe
//! live2d-ai.toml
//! web/                 Flutter 构建产物（index.html 在这一层）
//! web/render/          渲染面（wasm）的 hashed js / wasm（pack 从 dist/ 放这里）
//! assets/models/
//! assets/actions/
//! mods/<id>/mod.toml + 入口 exe
//! ```
//!
//! 所以资源查找在**现有 cwd / 仓库根**之外，再增加「exe 所在目录」这一层锚点。
//! 本模块只吃显式入参（exe 路径、cwd、环境变量查找闭包），**不读进程环境**，
//! 因此全部可在 Linux 上单测；调用点负责把 `std::env::current_exe()` 等喂进来。
//!
//! # 红线：运行期不读 WSL 网络路径
//!
//! 包里没有 WSL。若 exe 落在 `\\\\wsl$\\` / `\\\\wsl.localhost\\` 这种 UNC 路径上，
//! 说明用户是从 WSL 网络位置直接运行的——此时**不把该目录当资源锚点**
//! （网络路径会在 WSL 关闭时失效，正是本版要消灭的形态）。cwd 锚点保持既有
//! 行为不变（那是用户显式选择的当前目录）。
//!
//! # 分隔符
//!
//! [`portable`] 把 `\\` 统一成 `/`：这样同一份纯函数在 Linux 上也能对
//! `C:\\dist\\...` 这类路径做单测（Linux 的 `Path::parent` 不认 `\\`）；Windows
//! 本身接受 `/`，候选路径照常 `join` / `is_dir`。

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// 统一分隔符后的路径（Windows 的 `\\` → `/`）。
fn portable(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if text.contains('\\') {
        PathBuf::from(text.replace('\\', "/"))
    } else {
        path.to_path_buf()
    }
}

/// 是否 WSL 网络路径（`\\\\wsl$\\...` / `\\\\wsl.localhost\\...`，两种分隔符都认）。
pub fn is_wsl_network_path(path: &Path) -> bool {
    let text = portable(path).to_string_lossy().to_ascii_lowercase();
    text.starts_with("//wsl$/") || text.starts_with("//wsl.localhost/")
}

/// exe 所在目录（打包布局的锚点）；裸文件名 / 根目录没有可用父目录时 `None`。
pub fn exe_dir(exe: &Path) -> Option<PathBuf> {
    let exe = portable(exe);
    let parent = exe.parent()?;
    if parent.as_os_str().is_empty() {
        return None;
    }
    Some(parent.to_path_buf())
}

/// 打包锚点：`<exe_dir>/<rel>`；exe 是 WSL 网络路径时 `None`（红线）。
fn packaged(exe: &Path, rel: &str) -> Option<PathBuf> {
    if is_wsl_network_path(exe) {
        return None;
    }
    Some(exe_dir(exe)?.join(rel))
}

/// Flutter Web 构建目录候选：`<exe_dir>/web`。
pub fn packaged_web_dir(exe: &Path) -> Option<PathBuf> {
    packaged(exe, "web")
}

/// 渲染面 wasm 产物候选：`<exe_dir>/web/render`。
pub fn packaged_wasm_dist_dir(exe: &Path) -> Option<PathBuf> {
    packaged(exe, "web/render")
}

/// 模型根候选：`<exe_dir>/assets/models`。
pub fn packaged_models_dir(exe: &Path) -> Option<PathBuf> {
    packaged(exe, "assets/models")
}

/// 动作表候选：`<exe_dir>/assets/actions`。
pub fn packaged_actions_dir(exe: &Path) -> Option<PathBuf> {
    packaged(exe, "assets/actions")
}

/// 文件夹 Mod 扫描锚点：**cwd 的 `mods/` 在前**，exe 目录的在后（同一 id 以 cwd 为准）。
///
/// exe 是 WSL 网络路径时不追加（红线）；两处指向同一目录时去重。
pub fn mods_scan_roots(cwd: &Path, exe: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = vec![portable(cwd).join("mods")];
    if let Some(exe) = exe
        && let Some(dir) = exe_dir(exe)
        && !is_wsl_network_path(exe)
    {
        let root = dir.join("mods");
        if roots.first() != Some(&root) {
            roots.push(root);
        }
    }
    roots
}

/// 文件夹子进程的环境白名单：Linux 四项；Windows 再加三枚**已存在**的系统变量。
///
/// 名单之外一律不传（`.env` 快照 / 密钥 / 代理都不进子进程，见 `folder_spawn`）。
pub fn child_env_names(windows: bool) -> &'static [&'static str] {
    if windows {
        &[
            "PATH",
            "HOME",
            "LANG",
            "LC_ALL",
            "SYSTEMROOT",
            "USERPROFILE",
            "PATHEXT",
        ]
    } else {
        &["PATH", "HOME", "LANG", "LC_ALL"]
    }
}

/// 按白名单收集**存在**的环境变量（缺失的键不补空值）。
pub fn collect_child_env(
    windows: bool,
    mut lookup: impl FnMut(&str) -> Option<OsString>,
) -> Vec<(String, OsString)> {
    child_env_names(windows)
        .iter()
        .filter_map(|key| lookup(key).map(|value| ((*key).to_string(), value)))
        .collect()
}

/// 入口可启动性：**Windows 上必须就是 `.exe` 文件本身**。
///
/// `.sh` / `.bat` 交给 `CreateProcess` 必定失败（它们要 shell）；在扫描阶段就
/// 给一句可读原因，而不是让「进入」时冒出一条看不懂的系统错误。
/// 非 Windows 不做限制（0.3.12 / 0.3.13 的行为不变）。
pub fn entry_problem(path: &Path, windows: bool) -> Option<String> {
    if !windows {
        return None;
    }
    let is_exe = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"));
    if is_exe {
        return None;
    }
    Some(format!(
        "Windows 上入口必须是 .exe 文件本身（{} 不是 .exe；.sh / .bat 需要 shell，不会被 CreateProcess 执行）",
        path.display()
    ))
}

/// 启动器页 URL（Windows 包在监听成功后自开浏览器用；非 Windows 只有测试用它）。
#[cfg_attr(not(windows), allow(dead_code))]
pub fn launcher_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/")
}

/// 配置文件锚点候选：**cwd 的 `live2d-ai.toml` 在前**，exe 目录的在后。
///
/// 只给候选，「哪个存在」由调用方判：快捷方式起进程、Start-in 不对时，cwd 里
/// 没有 toml 也能用 exe 目录那份兜底（同目录的 `mods.json` 一并受益）。
/// `.env` 的查找**不**走这里（仍是既有口径，本版不动密钥真源）。
pub fn config_candidates(cwd: Option<&Path>, exe: Option<&Path>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(cwd) = cwd {
        out.push(portable(cwd).join("live2d-ai.toml"));
    }
    if let Some(exe) = exe
        && let Some(dir) = exe_dir(exe)
        && !is_wsl_network_path(exe)
    {
        let path = dir.join("live2d-ai.toml");
        if out.first() != Some(&path) {
            out.push(path);
        }
    }
    out
}

/// cwd / exe 目录里第一个**存在**的配置；都没有 → `None`（调用方回退 HOME）。
pub fn existing_config_path(cwd: Option<&Path>, exe: Option<&Path>) -> Option<PathBuf> {
    config_candidates(cwd, exe)
        .into_iter()
        .find(|path| path.is_file())
}

/// 配置目录 / exe 目录里第一个**存在**的 `mods.json`；都没有 → `None`（写配置目录）。
pub fn existing_mods_manifest(config_dir: &Path, exe: Option<&Path>) -> Option<PathBuf> {
    mods_manifest_candidates(config_dir, exe)
        .into_iter()
        .find(|path| path.is_file())
}

/// 生产入口：从**进程环境**取 cwd / current_exe，再调 [`mods_scan_roots`]。
///
/// 本模块唯一读进程环境的地方（其余函数保持纯）；调用点因此不必各自重复
/// `current_dir()` / `current_exe()` 的失败分支。
pub fn current_mods_scan_roots() -> Vec<PathBuf> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let exe = std::env::current_exe().ok();
    mods_scan_roots(&cwd, exe.as_deref())
}

/// `mods.json` 候选：配置目录的在前，exe 目录的在后。
///
/// 读与写取同一个路径：第一个存在的胜出；都不存在时调用方写第一个（配置目录），
/// 即 0.3.x 的既有行为。
pub fn mods_manifest_candidates(config_dir: &Path, exe: Option<&Path>) -> Vec<PathBuf> {
    let mut out = vec![portable(config_dir).join("mods.json")];
    if let Some(exe) = exe
        && let Some(dir) = exe_dir(exe)
        && !is_wsl_network_path(exe)
    {
        let path = dir.join("mods.json");
        if out.first() != Some(&path) {
            out.push(path);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIN_EXE: &str = r"C:\dist\windows\live2d-ai-desktop.exe";

    #[test]
    fn wsl_network_paths_are_recognized_in_both_separator_forms() {
        for path in [
            r"\\wsl$\Ubuntu\home\skystar\Live2D-Ai-win",
            r"\\WSL$\Ubuntu\dist",
            r"\\wsl.localhost\Ubuntu\home\x",
            r"\\WSL.LOCALHOST\Ubuntu\home\x",
            "//wsl$/Ubuntu/home/skystar/dist",
            "//wsl.localhost/Ubuntu/home/skystar/dist",
        ] {
            assert!(
                is_wsl_network_path(Path::new(path)),
                "{path} 应被认作 WSL 网络路径"
            );
        }
        for path in [
            r"C:\dist\windows",
            "/home/skystar/dist",
            r"\\server\share\dist",
            "relative/web",
        ] {
            assert!(
                !is_wsl_network_path(Path::new(path)),
                "{path} 不是 WSL 网络路径"
            );
        }
    }

    #[test]
    fn exe_dir_is_the_packaging_anchor() {
        assert_eq!(
            exe_dir(Path::new(WIN_EXE)).as_deref(),
            Some(Path::new("C:/dist/windows"))
        );
        // 裸文件名 / 根目录没有可用父目录。
        assert_eq!(exe_dir(Path::new("live2d-ai-desktop.exe")), None);
        assert_eq!(exe_dir(Path::new("/")), None);
    }

    #[test]
    fn packaged_resource_candidates_hang_off_the_exe_dir() {
        let exe = Path::new(WIN_EXE);
        assert_eq!(
            packaged_web_dir(exe).as_deref(),
            Some(Path::new("C:/dist/windows/web"))
        );
        assert_eq!(
            packaged_wasm_dist_dir(exe).as_deref(),
            Some(Path::new("C:/dist/windows/web/render"))
        );
        assert_eq!(
            packaged_models_dir(exe).as_deref(),
            Some(Path::new("C:/dist/windows/assets/models"))
        );
        assert_eq!(
            packaged_actions_dir(exe).as_deref(),
            Some(Path::new("C:/dist/windows/assets/actions"))
        );
    }

    #[test]
    fn packaged_candidates_refuse_wsl_network_exe() {
        let exe = Path::new(r"\\wsl$\Ubuntu\home\skystar\dist\live2d-ai-desktop.exe");
        assert_eq!(packaged_web_dir(exe), None);
        assert_eq!(packaged_wasm_dist_dir(exe), None);
        assert_eq!(packaged_models_dir(exe), None);
        assert_eq!(packaged_actions_dir(exe), None);
    }

    /// cwd 在前、exe 目录在后；同目录去重；WSL exe 锚点不追加。
    #[test]
    fn mods_scan_roots_put_cwd_first() {
        let cwd = Path::new("/home/skystar/pkg");
        let exe = Path::new("/opt/live2d-ai/live2d-ai-desktop.exe");
        assert_eq!(
            mods_scan_roots(cwd, Some(exe)),
            vec![
                PathBuf::from("/home/skystar/pkg/mods"),
                PathBuf::from("/opt/live2d-ai/mods")
            ]
        );
        // cwd 就是 exe 目录：只扫一次。
        assert_eq!(
            mods_scan_roots(Path::new("/opt/live2d-ai"), Some(exe)),
            vec![PathBuf::from("/opt/live2d-ai/mods")]
        );
        // 无 exe = 只有 cwd（0.3.12 行为）。
        assert_eq!(
            mods_scan_roots(cwd, None),
            vec![PathBuf::from("/home/skystar/pkg/mods")]
        );
        // WSL 网络路径上的 exe 不追加锚点（红线）。
        let wsl = Path::new(r"\\wsl$\Ubuntu\home\skystar\dist\live2d-ai-desktop.exe");
        assert_eq!(
            mods_scan_roots(cwd, Some(wsl)),
            vec![PathBuf::from("/home/skystar/pkg/mods")]
        );
    }

    #[test]
    fn child_env_allowlist_has_the_system_vars_only_on_windows() {
        assert_eq!(child_env_names(false), &["PATH", "HOME", "LANG", "LC_ALL"]);
        assert_eq!(
            child_env_names(true),
            &[
                "PATH",
                "HOME",
                "LANG",
                "LC_ALL",
                "SYSTEMROOT",
                "USERPROFILE",
                "PATHEXT"
            ]
        );
    }

    #[test]
    fn collect_child_env_keeps_only_present_allowlisted_names() {
        let lookup = |key: &str| match key {
            "PATH" => Some(OsString::from("/usr/bin")),
            "SYSTEMROOT" => Some(OsString::from(r"C:\Windows")),
            "USERPROFILE" => Some(OsString::from(r"C:\Users\skystar")),
            // 不在白名单：密钥 / 代理 / 其它一切都不许进子进程。
            "LIVE2D_AI_SENTINEL_SECRET" => Some(OsString::from("do-not-pass")),
            "HTTPS_PROXY" => Some(OsString::from("http://proxy")),
            _ => None,
        };
        let linux = collect_child_env(false, lookup);
        assert_eq!(
            linux,
            vec![("PATH".to_string(), OsString::from("/usr/bin"))]
        );
        let windows = collect_child_env(true, lookup);
        let keys: Vec<&str> = windows.iter().map(|(key, _)| key.as_str()).collect();
        assert_eq!(keys, vec!["PATH", "SYSTEMROOT", "USERPROFILE"]);
        for (key, _) in &windows {
            assert!(!key.contains("SECRET") && !key.contains("PROXY"), "{key}");
        }
    }

    #[test]
    fn windows_entry_must_be_the_exe_itself() {
        assert_eq!(entry_problem(Path::new(r"C:\mods\x\run.exe"), false), None);
        assert_eq!(
            entry_problem(Path::new("/mods/x/run.sh"), false),
            None,
            "非 Windows 不限制入口扩展名（0.3.12/0.3.13 行为不变）"
        );
        assert_eq!(entry_problem(Path::new(r"C:\mods\x\RUN.EXE"), true), None);
        for bad in [
            r"C:\mods\x\start.sh",
            r"C:\mods\x\start.bat",
            r"C:\mods\x\start",
        ] {
            let reason = entry_problem(Path::new(bad), true)
                .unwrap_or_else(|| panic!("{bad} 在 Windows 上不应可启动"));
            assert!(reason.contains(".exe"), "失败原因必须可读：{reason}");
            assert!(reason.contains("Windows"), "失败原因必须说明平台：{reason}");
        }
    }

    #[test]
    fn launcher_url_is_the_launcher_page() {
        assert_eq!(launcher_url(18080), "http://127.0.0.1:18080/");
    }

    /// 配置锚点：cwd 在前、exe 目录兜底；同目录去重；WSL exe 不追加。
    #[test]
    fn config_anchors_prefer_cwd_then_exe_dir() {
        let cwd = Path::new("/home/skystar/pkg");
        let exe = Path::new("/opt/live2d-ai/live2d-ai-desktop.exe");
        assert_eq!(
            config_candidates(Some(cwd), Some(exe)),
            vec![
                PathBuf::from("/home/skystar/pkg/live2d-ai.toml"),
                PathBuf::from("/opt/live2d-ai/live2d-ai.toml")
            ]
        );
        assert_eq!(
            config_candidates(Some(Path::new("/opt/live2d-ai")), Some(exe)),
            vec![PathBuf::from("/opt/live2d-ai/live2d-ai.toml")]
        );
        assert_eq!(
            config_candidates(Some(cwd), None),
            vec![PathBuf::from("/home/skystar/pkg/live2d-ai.toml")]
        );
        let wsl = Path::new(r"\\wsl$\Ubuntu\home\skystar\dist\live2d-ai-desktop.exe");
        assert_eq!(
            config_candidates(Some(cwd), Some(wsl)),
            vec![PathBuf::from("/home/skystar/pkg/live2d-ai.toml")]
        );
    }

    /// mods.json 锚点：配置目录在前、exe 目录兜底（读/写同路径）。
    #[test]
    fn mods_manifest_anchors_prefer_config_dir_then_exe_dir() {
        let config_dir = Path::new("/home/skystar/.config/live2d-ai");
        let exe = Path::new(r"C:\dist\windows\live2d-ai-desktop.exe");
        assert_eq!(
            mods_manifest_candidates(config_dir, Some(exe)),
            vec![
                PathBuf::from("/home/skystar/.config/live2d-ai/mods.json"),
                PathBuf::from("C:/dist/windows/mods.json")
            ]
        );
        // exe 目录就是配置目录：只有一份。
        assert_eq!(
            mods_manifest_candidates(Path::new("C:\\dist\\windows"), Some(exe)),
            vec![PathBuf::from("C:/dist/windows/mods.json")]
        );
        assert_eq!(
            mods_manifest_candidates(config_dir, None),
            vec![PathBuf::from("/home/skystar/.config/live2d-ai/mods.json")]
        );
    }
}
