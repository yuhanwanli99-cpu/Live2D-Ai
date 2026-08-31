//! 跨线程用户事件与桌宠启动决策（纯逻辑、可单测）。
//!
//! 事件循环从本批起改为 [`winit::event_loop::EventLoop::with_user_event`]：
//! 托盘（[`crate::tray`]）等外部线程**只**通过
//! [`winit::event_loop::EventLoopProxy::send_event`](winit::event_loop::EventLoopProxy)
//! 发送本枚举；所有 `Window::*` 调用一律留在事件循环线程内
//! （`ApplicationHandler::user_event` / `window_event`），杜绝跨线程触窗。
//!
//! 本模块同时承载「pet-mode 启动决策」纯函数：托盘初始化失败时禁止自动进入
//! 点击穿透（否则窗口收不到输入且无恢复入口 → 不可恢复状态）。

/// 托盘/其他线程发往事件循环的用户事件。
///
/// 语义约定：
/// - `Toggle*` 由菜单勾选项发送，目标态 = 当前态取反（当前态由共享原子簿记）；
/// - `TraySetVisible` 为显式目标态版本（托盘图标左键激活用它发「取反后的显式值」）；
/// - 所有事件的落地实现都在事件循环线程（见 `app.rs` 的 user_event 处理）。
///
/// `Tray` 前缀是刻意语义（标识事件来源 = 托盘线程，与 winit user_event 通道一致），
/// 去前缀会丢失来源语义，故豁免 clippy::enum_variant_names。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum PetUserEvent {
    /// 切换点击穿透（交互模式 ⇄ 穿透模式）。
    TrayToggleClickThrough,
    /// 显式设置点击穿透（REPL `/release` = 关闭穿透恢复交互；不经 toggle）。
    TraySetClickThrough(bool),
    /// 切换置顶。
    TrayToggleAlwaysOnTop,
    /// 切换窗口显示/隐藏（托盘恢复入口：隐藏后由此唤回）。
    TrayToggleVisible,
    /// 显式设置显示/隐藏。
    TraySetVisible(bool),
    /// 从托盘请求退出整个应用。
    TrayExit,
}

/// 单一布尔状态的切换（纯函数，供 Toggle 语义单测锚定）。
pub const fn flip(current: bool) -> bool {
    !current
}

/// pet-mode 的初始窗口意图。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PetLaunchPlan {
    /// 初始是否置顶。
    pub always_on_top: bool,
    /// 初始是否点击穿透。
    pub click_through: bool,
}

/// 计算 `--pet-mode` 的初始窗口意图（纯函数）。
///
/// 安全规则（RFC §4 批次 6 / 需求 D4）：
/// - `tray_confirmed == false`（初始化失败或超时未确认）时，
///   **强制 `click_through = false`**——穿透 + 无托盘恢复入口 = 不可恢复；
///   置顶不受此规则约束（交互仍在，Alt+F4 / 冒烟超时均可退出）；
/// - 非 pet-mode 一律不自动穿透、不自动置顶（默认模型冒烟保持交互可关闭）。
pub fn plan_launch(pet_mode: bool, tray_confirmed: bool) -> PetLaunchPlan {
    match (pet_mode, tray_confirmed) {
        // 托盘就绪：置顶 + 穿透按承诺开启。
        (true, true) => PetLaunchPlan {
            always_on_top: true,
            click_through: true,
        },
        // 托盘失败：仍可置顶，但绝不自动穿透（可见降级 + 可恢复优先）。
        (true, false) => PetLaunchPlan {
            always_on_top: true,
            click_through: false,
        },
        // 非 pet-mode：保持既有交互语义（不置顶、不穿透）。
        (false, _) => PetLaunchPlan {
            always_on_top: false,
            click_through: false,
        },
    }
}

/// 托盘初始化结果对 pet-mode 的约束说明（供日志口径统一，纯函数）。
///
/// 返回 `Some(说明)` 表示需要向用户输出可见降级警告。
pub fn pet_mode_tray_degradation_note(
    pet_mode: bool,
    tray_confirmed: bool,
) -> Option<&'static str> {
    if pet_mode && !tray_confirmed {
        Some(
            "pet-mode：托盘未确认可用，本次运行已禁用自动点击穿透 \
             （避免「穿透 + 无恢复入口」的不可恢复状态）；\
             窗口保持交互，可在托盘恢复后手动开启穿透",
        )
    } else {
        None
    }
}

/// 把用户事件归一为「目标布尔态」三元组视图（纯函数，测试/日志用）。
///
/// 返回 `(作用域, 目标值, 是否为取反语义)`：
/// - 取反语义（`Toggle*`）由调用方结合当前态求值；
/// - 显式语义（`Set*`）直接携带目标值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventTarget {
    ClickThrough(bool),
    AlwaysOnTop(bool),
    Visible(bool),
    Exit,
}

/// 将事件映射为目标态；`current_*` 仅在 Toggle 语义下参与求值。
pub fn event_target(event: PetUserEvent, current: bool) -> Option<EventTarget> {
    let next = flip(current);
    match event {
        PetUserEvent::TrayToggleClickThrough => Some(EventTarget::ClickThrough(next)),
        PetUserEvent::TraySetClickThrough(v) => Some(EventTarget::ClickThrough(v)),
        PetUserEvent::TrayToggleAlwaysOnTop => Some(EventTarget::AlwaysOnTop(next)),
        PetUserEvent::TrayToggleVisible => Some(EventTarget::Visible(next)),
        PetUserEvent::TraySetVisible(v) => Some(EventTarget::Visible(v)),
        PetUserEvent::TrayExit => Some(EventTarget::Exit),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EventTarget, PetLaunchPlan, PetUserEvent, event_target, flip,
        pet_mode_tray_degradation_note, plan_launch,
    };

    #[test]
    fn flip_is_boolean_negation() {
        assert!(!flip(true));
        assert!(flip(false));
    }

    #[test]
    fn launch_plan_full_when_tray_confirmed() {
        assert_eq!(
            plan_launch(true, true),
            PetLaunchPlan {
                always_on_top: true,
                click_through: true
            }
        );
    }

    #[test]
    fn launch_plan_forbids_click_through_without_tray() {
        // 核心安全规则：托盘失败 ⇒ 绝不自动穿透（置顶允许，交互仍可退出）。
        let plan = plan_launch(true, false);
        assert!(plan.always_on_top);
        assert!(!plan.click_through);
        // 降级提示必须可见。
        assert!(pet_mode_tray_degradation_note(true, false).is_some());
    }

    #[test]
    fn launch_plan_non_pet_mode_stays_interactive() {
        for tray_confirmed in [true, false] {
            let plan = plan_launch(false, tray_confirmed);
            assert!(!plan.always_on_top);
            assert!(!plan.click_through);
            // 非 pet-mode 不产生 pet 相关降级提示。
            assert!(pet_mode_tray_degradation_note(false, tray_confirmed).is_none());
        }
    }

    #[test]
    fn event_target_resolves_toggle_against_current_state() {
        assert_eq!(
            event_target(PetUserEvent::TrayToggleClickThrough, false),
            Some(EventTarget::ClickThrough(true))
        );
        assert_eq!(
            event_target(PetUserEvent::TrayToggleClickThrough, true),
            Some(EventTarget::ClickThrough(false))
        );
        assert_eq!(
            event_target(PetUserEvent::TraySetVisible(false), true),
            Some(EventTarget::Visible(false))
        );
        assert_eq!(
            event_target(PetUserEvent::TrayToggleVisible, true),
            Some(EventTarget::Visible(false))
        );
        assert_eq!(
            event_target(PetUserEvent::TrayExit, false),
            Some(EventTarget::Exit)
        );
    }
}
