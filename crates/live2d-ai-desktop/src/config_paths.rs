//! 配置目录选择（0.4.7 rc.1「配置 / registry 路径不再只认 HOME」，W4）。
//!
//! # 约定（与 log_paths 同一条「先环境、再 HOME、再 exe 目录、再 Windows 用户
//! 目录、**绝不相对**」的思路）
//!
//! 配置是 **config** 不是 data，所以第一级用 XDG_CONFIG_HOME 而不是
//! log_paths 的 XDG_DATA_HOME：Linux 上必须继续落 ~/.config/live2d-ai/，
//! 否则老用户的 live2d-ai.toml 会在升级后「消失」（这是回归，不是修复）。
//!
//! 按优先级取**绝对目录**，**永不返回相对路径**：
//!
//! 1. XDG_CONFIG_HOME → $XDG_CONFIG_HOME/live2d-ai/；
//! 2. 否则 HOME → $HOME/.config/live2d-ai/（0.4.x 以来的既有口径）；
//! 3. 否则 exe 在本地（不是 WSL 网络路径）→ <exe 目录>/——便携包：配置贴着 exe；
//! 4. 否则 %APPDATA%\live2d-ai（Windows 双击：没有 HOME 也能落到用户目录）；
//! 5. 否则 %LOCALAPPDATA%\live2d-ai；
//! 6. 都没有 → Err（中文说明），**绝不**回退相对的 live2d-ai.toml。
//!
//! # 缺陷（0.4.7 rc.1 修）
//!
//! 旧实现只认 HOME：Windows 双击（进程环境里没有 HOME）时把
//! live2d-ai.toml / mods.json 锚回**相对的 cwd**——cwd 只读时勾选写盘
//! 409 persist_failed，而且同目录派生物（memory.jsonl 等）解析到空目录。
//! 另外旧实现**不检查目录可写**：exe 目录只读（Program Files 类安装）时同样
//! 写不进去。所以生产入口取**第一个可写**候选：探针由调用方注入，
//! config_dir_candidates 仍是纯函数，Linux 单测可覆盖「只读目录 → 跳过 →
//! 落 APPDATA」。
//!
//! # 纯函数
//!
//! config_dir_candidates 只吃显式入参（exe 路径 + 环境查找闭包），不读进程
//! 真实环境；choose_config_dir 再用调用方注入的可写探针挑第一个可写候选。
//! 生产 wrapper production_config_dir 是唯一读 current_exe() / var_os /
//! 探磁盘的地方（与 logging::default_log_dir 的边界划分同一条纪律）。

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::host_layout;

/// 统一分隔符：Windows 的 \ 换成 /（与 host_layout / log_paths 同口径）。
fn portable(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if text.contains('\\') {
        PathBuf::from(text.replace('\\', "/"))
    } else {
        path.to_path_buf()
    }
}

/// 绝对锚点：Unix 绝对路径，或 Windows 盘符路径（C:/ 或 c:\）。
///
/// 相对路径一律不算锚点——否则拼出来的配置目录还是相对的，等于把缺陷换个名字。
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

/// 去重追加（多个来源可能指向同一目录，例如 HOME 与 XDG 同值）。
fn push_unique(out: &mut Vec<PathBuf>, dir: PathBuf) {
    if !out.contains(&dir) {
        out.push(dir);
    }
}

/// 配置目录候选（纯函数，按优先级；**每一项都是绝对路径**）。
///
/// 空表 = 没有任何绝对锚点（调用方必须当失败处理，不得回退相对路径）。
pub fn config_dir_candidates(
    exe: Option<&Path>,
    mut lookup: impl FnMut(&str) -> Option<OsString>,
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(xdg) = env_dir(&mut lookup, "XDG_CONFIG_HOME") {
        push_unique(&mut out, xdg.join("live2d-ai"));
    }
    if let Some(home) = env_dir(&mut lookup, "HOME") {
        push_unique(&mut out, home.join(".config").join("live2d-ai"));
    }
    if let Some(dir) = exe
        .filter(|exe| !host_layout::is_wsl_network_path(exe))
        .and_then(host_layout::exe_dir)
        .filter(|dir| is_absolute_anchor(dir))
    {
        // 便携包（zip 解到 D 盘）：配置就放 exe 旁边，与 host_layout 的
        // live2d-ai.toml / mods.json 候选同口径。
        push_unique(&mut out, dir);
    }
    if let Some(app_data) = env_dir(&mut lookup, "APPDATA") {
        push_unique(&mut out, app_data.join("live2d-ai"));
    }
    if let Some(local_app_data) = env_dir(&mut lookup, "LOCALAPPDATA") {
        push_unique(&mut out, local_app_data.join("live2d-ai"));
    }
    out
}

/// 按优先级取**第一个可写**候选（0.4.7 rc.1：只读 exe 目录不能当配置目录）。
///
/// - writable 由调用方注入（生产 = dir_is_writable，测试 = 固定表 / 临时目录）：
///   纯函数不探磁盘，Linux 上也能把「只读目录 → 跳过 → 落 APPDATA」钉住；
/// - 候选全都不「可写」→ 退回**第一个候选**（仍是绝对路径；写盘失败由上层给出
///   可读错误，不在选路阶段 panic，也不静默退化成相对路径）；
/// - 一个候选都没有 → Err（中文说明），**绝不**回退相对路径。
pub fn choose_config_dir(
    exe: Option<&Path>,
    lookup: impl FnMut(&str) -> Option<OsString>,
    mut writable: impl FnMut(&Path) -> bool,
) -> Result<PathBuf, String> {
    let candidates = config_dir_candidates(exe, lookup);
    if candidates.is_empty() {
        return Err(
            "无法确定配置目录：XDG_CONFIG_HOME / HOME / exe 目录 / APPDATA / \
             LOCALAPPDATA 都不是可用的绝对锚点；拒绝回退到相对的 live2d-ai.toml（请设置 \
             HOME 或 APPDATA）"
                .to_string(),
        );
    }
    if let Some(dir) = candidates.iter().find(|dir| writable(dir)) {
        return Ok(dir.clone());
    }
    Ok(candidates[0].clone())
}

/// 生产 wrapper：读进程环境 / current_exe，并用真实磁盘探针挑可写目录。
pub fn production_config_dir() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().ok();
    choose_config_dir(exe.as_deref(), |key| std::env::var_os(key), dir_is_writable)
}

/// 目录可写探针：目录不存在先 create_dir_all；再建一个探针文件、随即删掉。
///
/// 「只读目录 → 不 panic / 不 409 persist_failed」的判据就在这里：
/// create_dir_all 对**已存在**的只读目录会成功，所以必须真的尝试写文件
/// （与 logging::init_logging 用 appender build() 判定不可写同一条纪律）。
fn dir_is_writable(dir: &Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(format!(".live2d-ai-write-probe-{}", std::process::id()));
    match std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&probe)
    {
        Ok(file) => {
            drop(file);
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
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

    fn candidates(exe: Option<&Path>, pairs: &[(&str, &str)]) -> Vec<PathBuf> {
        config_dir_candidates(exe, env_of(pairs))
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
            "配置目录必须是绝对路径（不得相对）: {text}"
        );
        assert_ne!(path, Path::new("live2d-ai"), "禁止返回相对的配置目录");
    }

    #[test]
    fn xdg_config_home_wins_over_home() {
        let exe = Path::new(r"C:\pack\live2d-ai-desktop.exe");
        let out = candidates(
            Some(exe),
            &[
                ("XDG_CONFIG_HOME", "/tmp/xdg-config"),
                ("HOME", "/home/skystar"),
                ("APPDATA", r"C:\Users\skystar\AppData\Roaming"),
            ],
        );
        assert_eq!(out[0], Path::new("/tmp/xdg-config/live2d-ai"));
        assert_eq!(out[1], Path::new("/home/skystar/.config/live2d-ai"));
        assert_eq!(out[2], Path::new("C:/pack"));
        for dir in &out {
            assert_absolute_anchor(dir);
        }
    }

    /// Linux 既有口径不变：没有 XDG_CONFIG_HOME 时仍是 ~/.config/live2d-ai。
    #[test]
    fn home_config_dir_is_the_linux_default_without_xdg() {
        let out = candidates(
            None,
            &[
                ("HOME", "/home/skystar"),
                ("APPDATA", r"C:\Users\skystar\AppData\Roaming"),
            ],
        );
        assert_eq!(out[0], Path::new("/home/skystar/.config/live2d-ai"));
        assert_eq!(
            out[1],
            Path::new("C:/Users/skystar/AppData/Roaming/live2d-ai")
        );
        for dir in &out {
            assert_absolute_anchor(dir);
        }
    }

    /// Windows 双击常见形态：**没有 HOME / XDG**，exe 在本地目录，有 %APPDATA%。
    ///
    /// 旧实现会回退成相对的 live2d-ai.toml（cwd 只读时 = 409 persist_failed）；
    /// 现在必须落到绝对目录。
    #[test]
    fn windows_double_click_without_home_lands_in_appdata() {
        let exe = Path::new(r"D:\皮套 测试\live2d-ai-desktop.exe");
        let pairs = [("APPDATA", r"C:\Users\skystar\AppData\Roaming")];
        let out = candidates(Some(exe), &pairs);
        assert_eq!(
            out,
            vec![
                PathBuf::from("D:/皮套 测试"),
                PathBuf::from("C:/Users/skystar/AppData/Roaming/live2d-ai"),
            ],
            "exe 目录优先（便携包）；没有 HOME 时也要有 APPDATA 这个绝对锚点"
        );
        let chosen = choose_config_dir(Some(exe), env_of(&pairs), |_| true).expect("有绝对锚点");
        assert_absolute_anchor(&chosen);
        // 只读 exe 目录 → 跳过，落 APPDATA。
        let chosen = choose_config_dir(Some(exe), env_of(&pairs), |dir| {
            dir != Path::new("D:/皮套 测试")
        })
        .expect("APPDATA 可写");
        assert_eq!(
            chosen,
            Path::new("C:/Users/skystar/AppData/Roaming/live2d-ai")
        );
        assert_absolute_anchor(&chosen);
    }

    /// 只读 exe 目录不能当配置目录；选中目录必须**真的能写**（否则勾选写盘 409）。
    ///
    /// 用真实临时目录：ro 权限 0555（Linux 上不可写），rw 可写；APPDATA 注入成
    /// rw。选路必须跳 ro → 落 rw，随后模拟写 mods.json 必须成功。
    #[cfg(unix)]
    #[test]
    fn read_only_exe_dir_is_skipped_for_a_writable_user_dir() {
        use std::os::unix::fs::PermissionsExt;

        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root =
            std::env::temp_dir().join(format!("l2d-config-paths-{}-{stamp}", std::process::id()));
        let ro = root.join("read-only-install");
        let rw = root.join("user-data");
        std::fs::create_dir_all(&ro).expect("ro");
        std::fs::create_dir_all(&rw).expect("rw");
        std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o555)).expect("chmod ro");

        let result = {
            let exe = ro.join("live2d-ai-desktop.exe");
            let rw_text = rw.to_string_lossy().into_owned();
            let pairs = [("APPDATA", rw_text.as_str())];
            choose_config_dir(Some(&exe), env_of(&pairs), dir_is_writable)
        };
        // 先恢复权限再断言 / 清理，免得失败时留下删不掉的临时目录。
        let _ = std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o755));
        let chosen = result.expect("用户目录可写");
        assert_eq!(
            chosen,
            rw.join("live2d-ai"),
            "只读 exe 目录必须被跳过，落 APPDATA/live2d-ai"
        );
        assert_absolute_anchor(&chosen);
        // 选中目录真的能写：mods.json 落盘不会 409 persist_failed。
        std::fs::write(chosen.join("mods.json"), r#"{"mods":{}}"#).expect("选中的配置目录必须可写");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// 全部候选都不可写 → 仍返回绝对路径的第一候选（写盘失败交给上层报 409）。
    #[test]
    fn no_writable_candidate_falls_back_to_the_first_absolute_candidate() {
        let exe = Path::new(r"C:\pack\live2d-ai-desktop.exe");
        let chosen =
            choose_config_dir(Some(exe), env_of(&[]), |_| false).expect("exe 目录是绝对锚点");
        assert_eq!(chosen, Path::new("C:/pack"));
        assert_absolute_anchor(&chosen);
    }

    /// 一个绝对锚点都没有 → Err（中文说明），而不是相对 live2d-ai.toml。
    #[test]
    fn no_anchor_is_an_error_not_a_relative_fallback() {
        let err = choose_config_dir(None, env_of(&[]), |_| true).unwrap_err();
        assert!(err.contains("APPDATA"), "错误要指明缺哪个变量: {err}");
        assert!(
            err.contains("live2d-ai.toml"),
            "错误要说明拒绝相对的配置: {err}"
        );
        assert!(candidates(None, &[]).is_empty());
        // exe 是裸文件名 / 相对路径 / 空串：都不是锚点。
        for exe in ["live2d-ai-desktop.exe", "sub/live2d-ai-desktop.exe", ""] {
            let out = candidates(Some(Path::new(exe)), &[("HOME", "")]);
            assert!(out.is_empty(), "{exe}: {out:?}");
        }
    }

    /// 空串环境变量不是锚点（旧实现会把 XDG="" 拼成相对 live2d-ai/）。
    #[test]
    fn empty_environment_values_are_ignored() {
        let exe = Path::new("/opt/live2d-ai/live2d-ai-desktop.exe");
        let out = candidates(
            Some(exe),
            &[
                ("XDG_CONFIG_HOME", ""),
                ("HOME", ""),
                ("APPDATA", ""),
                ("LOCALAPPDATA", ""),
            ],
        );
        assert_eq!(out, vec![PathBuf::from("/opt/live2d-ai")]);
        assert_absolute_anchor(&out[0]);
    }

    /// WSL 网络路径上的 exe 不是锚点（红线），落 %LOCALAPPDATA%。
    #[test]
    fn wsl_network_exe_is_not_an_anchor() {
        let exe = Path::new(r"\\wsl$\Ubuntu\home\skystar\pack\live2d-ai-desktop.exe");
        let out = candidates(
            Some(exe),
            &[("LOCALAPPDATA", r"C:\Users\skystar\AppData\Local")],
        );
        assert_eq!(
            out,
            vec![PathBuf::from("C:/Users/skystar/AppData/Local/live2d-ai")]
        );
        assert_absolute_anchor(&out[0]);
    }

    /// 所有成功结果都是绝对锚点；无锚点一律 Err。
    #[test]
    fn resolutions_are_never_relative() {
        let win = Path::new(r"C:\pack\live2d-ai-desktop.exe");
        let wsl = Path::new(r"\\wsl.localhost\Ubuntu\home\x\live2d-ai-desktop.exe");
        let cases = [
            candidates(Some(win), &[("XDG_CONFIG_HOME", "/tmp/xdg")]),
            candidates(Some(win), &[("HOME", "/home/u")]),
            candidates(Some(win), &[]),
            candidates(Some(wsl), &[("APPDATA", r"C:\Users\u\AppData\Roaming")]),
        ];
        for out in &cases {
            assert!(!out.is_empty());
            for dir in out {
                assert_absolute_anchor(dir);
            }
        }
        assert!(choose_config_dir(None, env_of(&[]), |_| true).is_err());
    }

    /// 生产 wrapper 也必须是绝对路径（真实环境 + 真实磁盘探针）。
    #[test]
    fn production_config_dir_is_absolute() {
        let dir = production_config_dir().expect("总能找到绝对锚点");
        assert_absolute_anchor(&dir);
    }
}
