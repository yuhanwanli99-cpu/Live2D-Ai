//! 桌宠窗右键菜单：只有一项「回到网页」（2026-11-17）。
//!
//! - 只在 `WM_RBUTTONUP` 且**点落在角色或气泡像素上**时弹；空像素不弹，
//!   照旧穿透（命中判定由 `hit.rs` 用同一张 `HitMap` 给出）。
//! - 用 `TPM_RETURNCMD` 同步拿命令 ID：不必改 `wnd_proc` 去接 `WM_COMMAND`
//!   的 `wparam`（`hit::handle` 签名里没有 wparam），`window.rs` 也一行不用加。
//! - 菜单是模态的，会重入窗口过程；调用方（`hit.rs`）务必在**释放
//!   `STATE` 锁之后**再进本模块，否则重入消息取锁会自锁。
//! - 这次右键不进 `talk::on_down/on_move/on_up`：右键消息从不落到
//!   `start_drag`，所以既不拖拽也不开始录音。

#![allow(unsafe_code)]

use super::win32::*;
use crate::desk_pet::mode::{Mode, apply};

/// 唯一的菜单项 ID。
const ID_BACK_TO_WEB: usize = 1;

/// `on_pet` = 右键点是否落在角色/气泡上（由 `hit.rs` 判定）。
pub fn show(hwnd: Hwnd, cx: i32, cy: i32, on_pet: bool) -> Lresult {
    if !on_pet {
        return 0;
    }
    let label = wide("回到网页");
    let mut at = Point { x: cx, y: cy };
    // SAFETY: 句柄全部由本函数创建并按反序销毁；`at` 是栈上 8 字节 POINT；
    // label 的 Vec<u16> 在整个 TrackPopupMenu 调用期间存活。
    unsafe {
        if ClientToScreen(hwnd, &mut at) == 0 {
            return 0;
        }
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return 0;
        }
        let appended = AppendMenuW(menu, MF_STRING, ID_BACK_TO_WEB, label.as_ptr());
        // 本窗带 WS_EX_NOACTIVATE，弹菜单前先置前台，否则菜单可能不随外部点击消失。
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_LEFTALIGN,
            at.x,
            at.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);
        // MSDN 建议：NOACTIVATE 窗在菜单关闭后补一条空消息。
        PostMessageW(hwnd, WM_NULL, 0, 0);
        if appended != 0 && cmd as usize == ID_BACK_TO_WEB {
            let outcome = apply(Mode::Web);
            tracing::info!(
                target: "desk_pet",
                ok = outcome.ok,
                reason = outcome.reason.as_deref().unwrap_or(""),
                "右键「回到网页」"
            );
        }
    }
    0
}
