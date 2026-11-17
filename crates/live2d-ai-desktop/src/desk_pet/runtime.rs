//! 宠物窗的运行态与重复开关（2026-11-17「先进网页，再切桌宠」）。
//!
//! - `RUNNING` 防止连点开出第二扇窗，也保证「关窗 → 再开窗」串行化
//!   （旧窗 `Companion::drop` 会 `close_events`，未收尾就开新窗会互相清通道）；
//! - `PET_HWND` 供 `mode=web` / 右键菜单用 `PostMessageW(WM_CLOSE)` 复用现成的
//!   `WM_CLOSE → WM_DESTROY → WM_QUIT` 退出链，不新造停止协议；
//! - `HTTP_PORT` 由 `start_server` 绑定成功后注入，用来拼 `/app/` 地址。

#![allow(unsafe_code)]
// 单元测试期 `desk_pet::mode::apply` 走纯决策分支、不真开窗，开窗侧的函数
// 因此在本 target 下没有调用者；生产 target 里它们都由路由 / 菜单驱动。
#![cfg_attr(test, allow(dead_code))]

#[cfg(windows)]
use std::ffi::c_void;
#[cfg(windows)]
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};

/// 窗线程是否活着（防连点开出第二扇窗；`--desk-pet` 与 mode=pet 共用）。
static RUNNING: AtomicBool = AtomicBool::new(false);
/// 真实绑定端口：`start_server` 注入，供「回到网页」拼 `/app/` 地址。
static HTTP_PORT: AtomicU16 = AtomicU16::new(0);
/// 当前窗的 HWND（指针转 usize 存，避免 `*mut c_void` 的 Send/Sync 问题）。
#[cfg(windows)]
static PET_HWND: Mutex<Option<usize>> = Mutex::new(None);

pub fn set_http_port(port: u16) {
    HTTP_PORT.store(port, Ordering::SeqCst);
}

pub fn http_port() -> u16 {
    HTTP_PORT.load(Ordering::SeqCst)
}

pub fn is_running() -> bool {
    RUNNING.load(Ordering::SeqCst)
}

#[cfg(windows)]
pub fn set_hwnd(hwnd: *mut c_void) {
    if let Ok(mut slot) = PET_HWND.lock() {
        *slot = Some(hwnd as usize);
    }
}

#[cfg(windows)]
pub fn clear_hwnd() {
    if let Ok(mut slot) = PET_HWND.lock() {
        *slot = None;
    }
}

#[cfg(windows)]
fn pet_hwnd() -> Option<*mut c_void> {
    PET_HWND
        .lock()
        .ok()
        .and_then(|slot| *slot)
        .map(|v| v as *mut c_void)
}

/// 当前 `mods.json` 里的 `mods.desk-pet.config`（缺失 / 坏值 → 合同缺省）。
/// mode=pet 每次现读，不写盘、不 enable 工厂。
pub fn current_config() -> super::config::PetConfig {
    super::manifest::config_from_manifest(&super::manifest::read_manifest(
        &crate::web_api::cli_entry::mods_path_for_web(),
    ))
}

/// mode=pet：窗不在就开一扇（已在运行则幂等）。
pub fn open_pet() -> Result<(), String> {
    if is_running() {
        return Ok(());
    }
    // 「窗没开起来」要能同步回报：皮套包缺失是最常见的失败，先查这一项。
    // 与 window.rs 共用 `skin::model_path()`：当前皮肤不存在时不能被
    // 出厂 bai 的存在性蒙混过去。
    let model3 = super::skin::model_path();
    if !model3.is_file() {
        return Err(format!("皮套包不存在（{}）", model3.display()));
    }
    spawn_pet_thread(current_config())
}

/// mode=web / 右键菜单：关掉宠物窗（走现成的 WM_CLOSE → WM_DESTROY 退出链）。
/// 窗本来就不在时返回 false，调用方仍可照常打开浏览器。
#[cfg(windows)]
pub fn close_pet() -> bool {
    let Some(hwnd) = pet_hwnd() else {
        return false;
    };
    unsafe { super::win32::PostMessageW(hwnd, super::win32::WM_CLOSE, 0, 0) != 0 }
}

#[cfg(not(windows))]
pub fn close_pet() -> bool {
    false
}

/// 起 `desk-pet` 线程（Windows）；非 Windows 返回「仅 Windows」。
/// `start_if_armed`（--desk-pet）与 `open_pet`（mode=pet）共用这一条。
pub fn spawn_pet_thread(pet_config: super::config::PetConfig) -> Result<(), String> {
    #[cfg(windows)]
    {
        if RUNNING
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err("宠物窗正在运行（或正在退出）".to_string());
        }
        let spawned = std::thread::Builder::new()
            .name("desk-pet".to_string())
            .spawn(move || {
                let outcome =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        super::window::run_once(pet_config)
                    }));
                match outcome {
                    Ok(Ok(())) => {
                        tracing::info!(target: "desk_pet", "宠物窗已关闭；HTTP 服务继续");
                    }
                    Ok(Err(e)) => {
                        tracing::error!(target: "desk_pet", "宠物窗未打开/已退出：{e}（HTTP 服务继续）");
                    }
                    Err(_) => {
                        tracing::error!(target: "desk_pet", "宠物窗线程 panic（已捕获；HTTP 服务继续）");
                    }
                }
                // 必须最后做：旧窗 Companion::drop 已 close_events，
                // 之后 mode=pet 才能重新 open_events 而不被旧窗误清。
                clear_hwnd();
                RUNNING.store(false, Ordering::SeqCst);
            });
        match spawned {
            Ok(_) => Ok(()),
            Err(e) => {
                RUNNING.store(false, Ordering::SeqCst);
                Err(format!("宠物窗线程起不来：{e}"))
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = pet_config;
        Err("仅 Windows".to_string())
    }
}
