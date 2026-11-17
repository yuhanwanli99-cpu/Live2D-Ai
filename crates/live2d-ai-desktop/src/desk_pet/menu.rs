//! 桌宠窗右键菜单（2026-11-17 纠正版）：**六组**，子菜单用 `MF_POPUP`。
//!
//! - 只在新版 `hit.rs` 判定的「角色 / 气泡像素」上弹；空像素不弹、照旧穿透。
//! - `TPM_RETURNCMD` 同步拿命令号，**不发 `WM_COMMAND`**，`wnd_proc` 与
//!   `window.rs` 一行都不用改。菜单是模态的，调用方必须先松开 `STATE` 锁。
//! - 这次右键既不进 `start_drag` 也不进 `talk::on_down/...`：不拖拽、不录音。
//! - 命令表、打勾规则在平台无关的 `menu_state.rs`；这里只建 Win32 菜单、
//!   做副作用（置顶 / 皮肤 / 关窗 / 会话框），并写该写的两处持久化。
//!
//! 刻意**不做**：退出进程、透明度加减、打开 `/app/` 但把宠物留在桌面、
//! 第二套设置页。`live.rs` 里的透明度仍在配置里，只是不再由菜单改。

#![allow(unsafe_code)]

use super::live;
use super::menu_state::{self, Command, Group};
use super::skin;
use super::win32::*;
use crate::desk_pet::mode::{Mode, apply};

/// 置顶与当前皮肤之外的一切都只住内存：这里只写 `mods.desk-pet.config`。
fn remember(config: super::config::PetConfig) {
    live::publish(config);
    super::manifest::save_config(&crate::web_api::cli_entry::mods_path_for_web(), &config);
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

/// 子菜单由父菜单接管，父 `DestroyMenu` 会递归销毁；空皮肤表退化成一个组头。
unsafe fn append_group(menu: Hmenu, group: &Group, labels: &mut Vec<Vec<u16>>) {
    let head = wide(&group.label);
    if group.children.is_empty() {
        let mut flags = MF_STRING;
        if group.checked {
            flags |= MF_CHECKED;
        }
        let id = group.command.unwrap_or(0);
        unsafe { AppendMenuW(menu, flags, id, head.as_ptr()) };
        labels.push(head);
        return;
    }
    let sub = unsafe { CreatePopupMenu() };
    if sub.is_null() {
        return;
    }
    for child in &group.children {
        let mut flags = MF_STRING;
        if child.checked {
            flags |= MF_CHECKED;
        }
        if !child.enabled {
            flags |= MF_GRAYED | MF_DISABLED;
        }
        let text = wide(&child.label);
        unsafe { AppendMenuW(sub, flags, child.id, text.as_ptr()) };
        labels.push(text);
    }
    unsafe { AppendMenuW(menu, MF_POPUP, sub as usize, head.as_ptr()) };
    labels.push(head);
}

/// 副作用分类（纯函数，单测钉住「哪些动作会写盘 / 关窗」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Effect {
    None,
    Topmost,
    BackToWeb,
    Skin(usize),
    Voice,
    Chat,
}

#[must_use]
fn effect_of(command: &Command) -> Effect {
    match command {
        Command::TopmostToggle => Effect::Topmost,
        Command::BackToWeb => Effect::BackToWeb,
        Command::Skin(i) => Effect::Skin(*i),
        Command::Voice(_) => Effect::Voice,
        Command::ChatToggle | Command::ChatTopmost(_) => Effect::Chat,
        Command::DialogToggle | Command::UserToggle | Command::AiToggle => Effect::None,
    }
}

fn dispatch(hwnd: Hwnd, command: Command, skins: &[(String, String)]) {
    menu_state::apply(&command);
    match effect_of(&command) {
        Effect::None => {}
        Effect::Voice => {
            tracing::info!(
                target: "desk_pet",
                mode = ?menu_state::get().voice,
                "语音输入档位（没配钥匙时只选档，不打上游）"
            );
        }
        Effect::Chat => {
            // 下一帧 companion.tick 里的 chat::poll() 会按状态开/关/置顶。
            tracing::info!(target: "desk_pet", "简易会话框状态已更新");
        }
        Effect::Topmost => {
            let next = !live::get().always_on_top;
            apply_topmost(hwnd, next);
            remember(live::set_always_on_top(next));
            tracing::info!(target: "desk_pet", on = next, "右键「置顶 / 取消置顶」");
        }
        Effect::BackToWeb => {
            let outcome = apply(Mode::Web);
            tracing::info!(
                target: "desk_pet",
                ok = outcome.ok,
                reason = outcome.reason.as_deref().unwrap_or(""),
                "右键「回到 web 版本」"
            );
        }
        Effect::Skin(index) => {
            if let Some((name, rel)) = skins.get(index) {
                skin::select(rel);
                tracing::info!(target: "desk_pet", skin = %name, "右键「切换皮肤」");
            } else {
                tracing::warn!(target: "desk_pet", index, "皮肤索引越界，忽略");
            }
        }
    }
}

/// `on_pet` = 右键点是否落在角色或气泡像素上（由 `hit.rs` 判定）。
pub fn show(hwnd: Hwnd, cx: i32, cy: i32, on_pet: bool) -> Lresult {
    if !on_pet {
        return 0;
    }
    let state = menu_state::get();
    let topmost = live::get().always_on_top;
    let skins = skin::installed();
    let names: Vec<String> = skins.iter().map(|(name, _)| name.clone()).collect();
    let groups = menu_state::build(&state, topmost, &names, skin::active_index(&skins));
    let mut at = Point { x: cx, y: cy };
    // SAFETY: 菜单句柄由本函数创建，父菜单最后按一次 DestroyMenu 递归销毁；
    // `labels` 里所有宽字符串在整个菜单构建期间存活；`at` 是栈上 POINT。
    unsafe {
        if ClientToScreen(hwnd, &mut at) == 0 {
            return 0;
        }
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return 0;
        }
        let mut labels: Vec<Vec<u16>> = Vec::new();
        for group in &groups {
            append_group(menu, group, &mut labels);
        }
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD,
            at.x,
            at.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);
        // MSDN：NOACTIVATE 窗在菜单关闭后补一条空消息。
        PostMessageW(hwnd, WM_NULL, 0, 0);
        if let Some(command) = Command::from_id(cmd as usize) {
            dispatch(hwnd, command, &skins);
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desk_pet::menu_state::VoiceMode;

    #[test]
    fn only_topmost_and_skin_have_persistent_side_effects() {
        assert_eq!(effect_of(&Command::TopmostToggle), Effect::Topmost);
        assert_eq!(effect_of(&Command::Skin(2)), Effect::Skin(2));
        assert_eq!(effect_of(&Command::BackToWeb), Effect::BackToWeb);
        assert_eq!(effect_of(&Command::Voice(VoiceMode::Off)), Effect::Voice);
        assert_eq!(effect_of(&Command::ChatToggle), Effect::Chat);
        assert_eq!(effect_of(&Command::DialogToggle), Effect::None);
        assert_eq!(effect_of(&Command::UserToggle), Effect::None);
        assert_eq!(effect_of(&Command::AiToggle), Effect::None);
    }

    #[test]
    fn six_groups_and_their_submenu_item_counts() {
        let skins = [("bai".to_string(), "bai/runtime/bai.model3.json".to_string())];
        let names: Vec<String> = skins.iter().map(|(n, _)| n.clone()).collect();
        let groups = menu_state::build(&menu_state::MenuState::default(), true, &names, 0);
        assert_eq!(groups.len(), 6);
        assert_eq!(groups[0].children.len(), 3, "对话框：总开关 + 两个子开关");
        assert_eq!(groups[3].children.len(), 3, "语音：常开 / 长按 / 关闭");
        assert_eq!(groups[4].children.len(), 1, "皮肤：一项一个模型");
        assert_eq!(groups[5].children.len(), 3, "会话框：开/关 + 置顶/非置顶");
        assert!(groups[1].children.is_empty() && groups[2].children.is_empty());
    }
}
