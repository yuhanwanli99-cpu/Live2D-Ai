//! Windows 子进程生命周期的两件 Win32 小工具（0.4.7 rc.2 / W5 + W6）。
//!
//! - **W5**：CREATE_SUSPENDED 起子进程 → 指派 Job → 再 ResumeThread。0.4.6 以前
//!   是「spawn 之后才 AssignProcessToJobObject」，子进程在这一小段窗口里拉起的
//!   孙进程不在 Job 里，父进程被硬杀时会逃逸。挂起启动把窗口关掉：attach 完成
//!   前子进程一行代码都没跑（创建标志在 folder_spawn.rs 的 CHILD_CREATION_FLAGS）。
//! - **W6**：温和终止——先给子进程的顶层窗口发 WM_CLOSE，再给它独占的控制台
//!   进程组发 CTRL_BREAK_EVENT；调用方（folder_spawn::stop_child）宽限后才
//!   TerminateJobObject。Linux 仍是 SIGTERM → 宽限 → SIGKILL，语义不变。
//!
//! 只声明 kernel32 / user32 的少量函数：本 crate 顶层依赖顶格（18/18），**不加
//! windows crate**。工作区 unsafe_code = deny，本文件级显式豁免（与 folder_job /
//! desk_pet/win32 同一条纪律）。
//!
//! **未验**：本文件只在 cfg(windows) 下编译，Linux 单测覆盖不到运行时行为；
//! 类型 / 签名按 mingw-w64 头文件（winbase.h / tlhelp32.h / winuser.h）核对，
//! 真机行为由 Lead 在 Windows 上验证。

use std::ffi::c_void;
use std::io;
use std::os::windows::io::AsRawHandle;
use std::process::Child;

// ── 类型别名（与 desk_pet/win32.rs 同口径） ──
type Handle = *mut c_void;
type Hwnd = *mut c_void;
type Bool = i32;
type Lparam = isize;

/// CreateToolhelp32Snapshot 的失败返回值（(HANDLE)-1）。
const INVALID_HANDLE_VALUE: usize = usize::MAX;
/// TH32CS_SNAPTHREAD：线程快照（用来找刚创建的子进程主线程）。
const TH32CS_SNAPTHREAD: u32 = 0x0000_0004;
/// THREAD_SUSPEND_RESUME：OpenThread 需要的最小权限。
const THREAD_SUSPEND_RESUME: u32 = 0x0002;
/// CTRL_BREAK_EVENT：控制台进程组的温和终止事件。
const CTRL_BREAK_EVENT: u32 = 1;
/// WM_CLOSE：顶层窗口的温和关闭消息。
const WM_CLOSE: u32 = 0x0010;

/// THREADENTRY32（tlhelp32.h，64 位下 28 字节）。
#[repr(C)]
struct ThreadEntry32 {
    size: u32,
    usage: u32,
    thread_id: u32,
    owner_pid: u32,
    base_pri: i32,
    delta_pri: i32,
    flags: u32,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> Handle;
    fn Thread32First(snapshot: Handle, entry: *mut ThreadEntry32) -> Bool;
    fn Thread32Next(snapshot: Handle, entry: *mut ThreadEntry32) -> Bool;
    fn OpenThread(desired_access: u32, inherit: Bool, thread_id: u32) -> Handle;
    fn ResumeThread(thread: Handle) -> u32;
    fn CloseHandle(object: Handle) -> Bool;
    fn GenerateConsoleCtrlEvent(ctrl_event: u32, process_group_id: u32) -> Bool;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn EnumWindows(
        callback: Option<unsafe extern "system" fn(Hwnd, Lparam) -> Bool>,
        lparam: Lparam,
    ) -> Bool;
    fn GetWindowThreadProcessId(hwnd: Hwnd, process_id: *mut u32) -> u32;
    fn IsWindowVisible(hwnd: Hwnd) -> Bool;
    fn PostMessageW(hwnd: Hwnd, message: u32, wparam: usize, lparam: isize) -> Bool;
}

/// 恢复被 CREATE_SUSPENDED 挂起的主线程（必须在 Job 指派**之后**调用）。
///
/// 用线程快照找 owner = 子进程 pid 的第一条线程：子进程刚创建、只有主线程一条，
/// 所以不会认错。失败由调用方兜底结束子进程（不能让它永远挂着）。
pub fn resume_primary_thread(child: &Child) -> io::Result<()> {
    let pid = child.id();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if snapshot.is_null() || snapshot as usize == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let mut entry = ThreadEntry32 {
            size: std::mem::size_of::<ThreadEntry32>() as u32,
            usage: 0,
            thread_id: 0,
            owner_pid: 0,
            base_pri: 0,
            delta_pri: 0,
            flags: 0,
        };
        let mut primary = None;
        let mut more = Thread32First(snapshot, &mut entry) != 0;
        while more {
            if entry.owner_pid == pid {
                primary = Some(entry.thread_id);
                break;
            }
            more = Thread32Next(snapshot, &mut entry) != 0;
        }
        let _ = CloseHandle(snapshot);

        let thread_id = primary.ok_or_else(|| {
            io::Error::other(format!(
                "找不到子进程 {pid} 的主线程（无法从 CREATE_SUSPENDED 恢复运行）"
            ))
        })?;
        let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, thread_id);
        if thread.is_null() {
            return Err(io::Error::last_os_error());
        }
        let suspended = ResumeThread(thread);
        let error = io::Error::last_os_error();
        let _ = CloseHandle(thread);
        if suspended == u32::MAX {
            return Err(error);
        }
        Ok(())
    }
}

/// 请求**温和终止**：给子进程的可见顶层窗口发 WM_CLOSE，再给它的控制台进程组
/// 发 CTRL_BREAK_EVENT。
///
/// 两件事都只是请求（GUI 无窗口 / 子进程自己忽略 / 没有控制台），失败也无妨：
/// 调用方宽限后仍会 TerminateJobObject。子进程必须以 CREATE_NEW_PROCESS_GROUP
/// 创建，CTRL_BREAK 才能只打到它自己的进程组（见 folder_spawn 的创建标志）。
pub fn request_graceful_stop(pid: u32) {
    unsafe {
        let _ = EnumWindows(Some(close_windows_of), pid as Lparam);
        let _ = GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, pid);
    }
}

/// EnumWindows 回调：只给**该 pid 的可见顶层窗口**发 WM_CLOSE。
unsafe extern "system" fn close_windows_of(hwnd: Hwnd, lparam: Lparam) -> Bool {
    let target = lparam as u32;
    unsafe {
        let mut owner = 0u32;
        let _ = GetWindowThreadProcessId(hwnd, &mut owner);
        if owner == target && IsWindowVisible(hwnd) != 0 {
            let _ = PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
    }
    1 // 继续枚举（TRUE）
}
