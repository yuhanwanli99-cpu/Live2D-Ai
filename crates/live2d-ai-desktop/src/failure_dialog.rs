//! 双击失败可见（0.4.7 rc.1 / W8）。
//!
//! Windows 包是**控制台子系统**（PE subsystem = console）：资源管理器双击时系统
//! 新建一个黑窗，进程一退窗就关。端口被占 / 目录只读 / 位数不符这类启动失败，
//! 用户只看到「黑窗一闪」。本模块在**失败退出且是双击起进程**时补一个
//! MessageBoxW（手写 user32 FFI，不加依赖；风格照抄 desk_pet/win32.rs）。
//! Linux / 测试路径保持现在的 stderr 行为（main.rs 里按 cfg(windows) 分叉，
//! 非 Windows 的 show_on_failure / record_failure 都是 no-op）。
//!
//! # 只弹「交互双击」
//!
//! 判据 = GetConsoleProcessList 只返回**本进程一个**：explorer 双击会新建控制台
//! （控制台里只有我们）；而从 cmd / PowerShell / CI / cargo test 启动时控制台里
//! 还有 shell / cargo，计数 > 1 → 不弹。另给一个显式静默开关
//! LIVE2D_AI_NO_DIALOG=1（测试 / 无人值守场景双保险）。
//!
//! # 纯文本
//!
//! 对话框正文由 failure_text 生成（纯函数，Linux 可单测）：退出码 + 失败详情 +
//! 常见原因（端口被占 / 目录只读 / 位数不符 / 缺运行库）+ 日志目录。
//!
//! 未验：Windows 运行时行为（真机弹窗）本机无法验证；Linux 侧只覆盖纯文本。

use std::path::Path;

/// 显式静默开关：设置即不弹（测试 / CI / 无人值守）。
const ENV_NO_DIALOG: &str = "LIVE2D_AI_NO_DIALOG";

/// panic 的进程退出码（Rust 默认 101）；对话框文案里如实写出来。
#[cfg(windows)]
const PANIC_EXIT_CODE: u8 = 101;

/// 最近一次启动失败详情（Windows 专用；show_on_failure 取走后清空）。
#[cfg(windows)]
static LAST_FAILURE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// 对话框正文（纯函数）：退出码 + 失败详情 + 常见原因 + 日志目录。
///
/// detail 为空 / None 时只给通用排查段落（不编造原因）；log_dir 未初始化时
/// 明确写「未能初始化」，而不是省略这一行。
pub fn failure_text(exit_code: u8, detail: Option<&str>, log_dir: Option<&Path>) -> String {
    let mut out = format!("live2d-ai 启动失败（退出码 {exit_code}）\n\n");
    if let Some(detail) = detail.map(str::trim).filter(|detail| !detail.is_empty()) {
        out.push_str(detail);
        out.push_str("\n\n");
    }
    out.push_str("常见原因：\n");
    out.push_str("  · 端口 18080 已被占用（另一个 live2d-ai 实例，或 WSL 镜像回环占着同端口）；\n");
    out.push_str("  · 程序目录只读：日志 / mods.json 写不进去；\n");
    out.push_str("  · 程序或依赖的 .exe 位数不符、缺少运行库（VC++ 运行库 / DLL）。\n\n");
    match log_dir {
        Some(dir) => out.push_str(&format!("日志目录：{}\n", dir.display())),
        None => out.push_str("日志：未能初始化（详情见控制台输出）\n"),
    }
    out.push_str("排查：在「终端 / PowerShell」里运行本程序可看到完整输出。");
    out
}

/// 记录最近一次启动失败详情（只由 run_web_mode 的失败出口调用）。
#[cfg(windows)]
pub fn record_failure(detail: impl Into<String>) {
    let mut slot = LAST_FAILURE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *slot = Some(detail.into());
}

/// 非 Windows：没有对话框，也就不需要记录（保持现在的 stderr 行为）。
#[cfg(not(windows))]
pub fn record_failure(_detail: impl Into<String>) {}

/// 取走最近一次失败详情（取走即清空，避免下一次失败复述旧原因）。
#[cfg(windows)]
fn take_failure() -> Option<String> {
    let mut slot = LAST_FAILURE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    slot.take()
}

/// 失败退出：**交互双击**才弹窗；返回原退出码（便于 main.rs 链式写）。
#[cfg(windows)]
pub fn show_on_failure(code: u8) -> u8 {
    if code != 0 && is_interactive_double_click() {
        let detail = take_failure();
        let log_dir = crate::logging::current_log_dir();
        win::message_box(
            &failure_text(code, detail.as_deref(), log_dir.as_deref()),
            "live2d-ai",
        );
    }
    code
}

/// 非 Windows：no-op（main.rs 的调用点平台无关，Linux 行为逐字不变）。
#[cfg(not(windows))]
pub fn show_on_failure(code: u8) -> u8 {
    code
}

/// Windows：安装 panic hook（双击崩溃也弹窗，不再「黑窗一闪」）；其它平台 no-op。
///
/// 只改**本进程**的 panic 行为；测试二进制不经过 main.rs，因此不受影响。
pub fn install_panic_hook() {
    #[cfg(windows)]
    {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            previous(info);
            if is_interactive_double_click() {
                let log_dir = crate::logging::current_log_dir();
                let detail = format!("程序崩溃（panic）：{info}");
                win::message_box(
                    &failure_text(PANIC_EXIT_CODE, Some(&detail), log_dir.as_deref()),
                    "live2d-ai",
                );
            }
        }));
    }
}

/// 是否由 explorer 双击起进程（控制台里只有本进程，且没有显式静默开关）。
///
/// 计数为 0（没有控制台）/ > 1（已有终端或 CI 父进程）一律不弹：宁可少弹，
/// 不能在 CI / 批处理里卡住一个模态窗。
#[cfg(windows)]
fn is_interactive_double_click() -> bool {
    if std::env::var_os(ENV_NO_DIALOG).is_some_and(|value| !value.is_empty()) {
        return false;
    }
    win::console_process_count() == 1
}

#[cfg(windows)]
#[allow(unsafe_code)] // 唯一 FFI 缝：user32（std 无 MessageBox / 控制台枚举封装）。
mod win {
    use std::ffi::c_void;

    /// MessageBox 常量（winuser.h）。
    const MB_OK: u32 = 0x0000_0000;
    const MB_ICONERROR: u32 = 0x0000_0010;
    const MB_SETFOREGROUND: u32 = 0x0001_0000;
    const MB_TOPMOST: u32 = 0x0004_0000;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(hwnd: *mut c_void, text: *const u16, caption: *const u16, utype: u32)
        -> i32;
        fn GetConsoleProcessList(list: *mut u32, count: u32) -> u32;
    }

    /// UTF-16 + NUL（宽字符 API 的入参形态；与 desk_pet/win32.rs 的 wide 同款）。
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// 阻塞式错误对话框（用户点确定后进程才退出，控制台不会先关）。
    pub(super) fn message_box(text: &str, caption: &str) {
        let text = wide(text);
        let caption = wide(caption);
        unsafe {
            let _ = MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                caption.as_ptr(),
                MB_OK | MB_ICONERROR | MB_SETFOREGROUND | MB_TOPMOST,
            );
        }
    }

    /// 调用方控制台里挂着的进程数（0 = 没有控制台 / 调用失败）。
    pub(super) fn console_process_count() -> u32 {
        let mut list = [0u32; 2];
        unsafe { GetConsoleProcessList(list.as_mut_ptr(), list.len() as u32) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 正文必须自带「退出码 + 详情 + 常见原因 + 日志目录」四段（用户只看这个框）。
    #[test]
    fn failure_text_carries_exit_code_detail_hints_and_log_dir() {
        let text = failure_text(
            3,
            Some("启动 HTTP server 失败: Address already in use（端口 18080）"),
            Some(Path::new("C:/pack/logs")),
        );
        assert!(text.contains("退出码 3"), "{text}");
        assert!(
            text.contains("Address already in use"),
            "详情要原样保留: {text}"
        );
        assert!(text.contains("端口 18080"), "要点名端口: {text}");
        assert!(text.contains("只读"), "要提示目录只读: {text}");
        assert!(text.contains("运行库"), "要提示缺运行库: {text}");
        assert!(text.contains("日志目录：C:/pack/logs"), "{text}");
        assert!(text.contains("PowerShell"), "要告诉用户怎么排查: {text}");
    }

    /// 没有详情 / 日志目录未初始化时也要成句，不出现 None / 空白段。
    #[test]
    fn failure_text_degrades_without_detail_or_log_dir() {
        let text = failure_text(1, None, None);
        assert!(text.contains("退出码 1"), "{text}");
        assert!(!text.contains("None"), "{text}");
        assert!(text.contains("未能初始化"), "日志失败要说清: {text}");
        let blank = failure_text(3, Some("   "), None);
        assert!(!blank.contains("   \n\n"), "空白详情不该留空段: {blank}");
    }

    /// 非 Windows 的失败出口是 no-op：退出码原样返回（Linux 行为逐字不变）。
    #[cfg(not(windows))]
    #[test]
    fn show_on_failure_is_a_noop_off_windows() {
        assert_eq!(show_on_failure(3), 3);
        assert_eq!(show_on_failure(0), 0);
        record_failure("不会弹窗（非 Windows）");
    }
}
