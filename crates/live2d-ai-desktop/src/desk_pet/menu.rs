//! 桌宠窗右键菜单。
//!
//! 角色或气泡上右键弹出。空像素不弹，仍然穿透。
//! 菜单是模态的；调用方必须先放开 `STATE` 锁再进来。
//! 用 `TPM_RETURNCMD` 取命令号，按编号分发，不改 `wnd_proc`。

#![allow(unsafe_code)]

use super::config::PetConfig;
use super::live;
use super::win32::*;
use crate::desk_pet::mode::{Mode, apply};

const ID_BACK_TO_WEB: usize = 1;
const ID_OPEN_CHAT: usize = 2;
const ID_TOPMOST: usize = 3;
const ID_FADE: usize = 4;
const ID_SOLID: usize = 5;

struct Entry {
    id: usize,
    label: &'static str,
    /// 分隔线。`label` 忽略。
    separator: bool,
    checked: bool,
}

fn entries(config: &PetConfig) -> Vec<Entry> {
    vec![
        Entry {
            id: ID_BACK_TO_WEB,
            label: "回到网页",
            separator: false,
            checked: false,
        },
        Entry {
            id: ID_OPEN_CHAT,
            label: "打开对话（宠物留在桌面）",
            separator: false,
            checked: false,
        },
        Entry {
            id: 0,
            label: "",
            separator: true,
            checked: false,
        },
        Entry {
            id: ID_TOPMOST,
            label: "置顶",
            separator: false,
            checked: config.always_on_top,
        },
        Entry {
            id: ID_FADE,
            label: "淡一点",
            separator: false,
            checked: false,
        },
        Entry {
            id: ID_SOLID,
            label: "浓一点",
            separator: false,
            checked: false,
        },
    ]
}

fn remember(config: PetConfig) {
    live::publish(config);
    super::manifest::save_config(
        &crate::web_api::cli_entry::mods_path_for_web(),
        &config,
    );
}

fn apply_topmost(hwnd: Hwnd, on: bool) {
    let after = if on { HWND_TOPMOST } else { HWND_NOTOPMOST };
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            after,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

fn dispatch(hwnd: Hwnd, cmd: usize) {
    match cmd {
        ID_BACK_TO_WEB => {
            let outcome = apply(Mode::Web);
            tracing::info!(
                target: "desk_pet",
                ok = outcome.ok,
                reason = outcome.reason.as_deref().unwrap_or(""),
                "右键「回到网页」"
            );
        }
        ID_OPEN_CHAT => {
            crate::browser::open_app(crate::desk_pet::http_port());
            tracing::info!(target: "desk_pet", "右键「打开对话」宠物窗保持");
        }
        ID_TOPMOST => {
            let next = !live::get().always_on_top;
            apply_topmost(hwnd, next);
            let updated = live::set_always_on_top(next);
            remember(updated);
            tracing::info!(target: "desk_pet", on = next, "右键「置顶」");
        }
        ID_FADE => {
            let updated = live::nudge_opacity(-0.1);
            remember(updated);
            tracing::info!(target: "desk_pet", opacity = updated.opacity, "右键「淡一点」");
        }
        ID_SOLID => {
            let updated = live::nudge_opacity(0.1);
            remember(updated);
            tracing::info!(target: "desk_pet", opacity = updated.opacity, "右键「浓一点」");
        }
        _ => {}
    }
}

/// `on_pet` = 右键是否落在角色或气泡上。
pub fn show(hwnd: Hwnd, cx: i32, cy: i32, on_pet: bool) -> Lresult {
    if !on_pet {
        return 0;
    }
    let config = live::get();
    let rows = entries(&config);
    let labels: Vec<Vec<u16>> = rows.iter().map(|row| wide(row.label)).collect();
    let mut at = Point { x: cx, y: cy };
    unsafe {
        if ClientToScreen(hwnd, &mut at) == 0 {
            return 0;
        }
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return 0;
        }
        for (row, label) in rows.iter().zip(labels.iter()) {
            let flags = if row.separator {
                MF_SEPARATOR
            } else if row.checked {
                MF_STRING | MF_CHECKED
            } else {
                MF_STRING
            };
            let _ = AppendMenuW(menu, flags, row.id, label.as_ptr());
        }
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
        PostMessageW(hwnd, WM_NULL, 0, 0);
        dispatch(hwnd, cmd as usize);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_lists_the_product_actions_and_marks_topmost() {
        let mut config = PetConfig::default();
        config.always_on_top = true;
        let rows = entries(&config);
        let labels: Vec<_> = rows.iter().map(|row| row.label).collect();
        assert!(labels.contains(&"回到网页"));
        assert!(labels.contains(&"打开对话（宠物留在桌面）"));
        assert!(labels.contains(&"置顶"));
        assert!(labels.contains(&"淡一点"));
        assert!(labels.contains(&"浓一点"));
        assert!(rows.iter().any(|row| row.id == ID_TOPMOST && row.checked));
        config.always_on_top = false;
        assert!(entries(&config).iter().any(|row| row.id == ID_TOPMOST && !row.checked));
    }
}
