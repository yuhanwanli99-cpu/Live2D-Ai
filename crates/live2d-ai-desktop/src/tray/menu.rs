//! 菜单模型（与 ksni 解耦的纯数据规格 + 文案常量）。
//!
//! 与 [`super`] 模块中的 ksni 胶水分离：本文件是纯逻辑，
//! 单测可锚定菜单语义不依赖任何 D-Bus / 主题包。

use crate::user_event::PetUserEvent;

/// 与 ksni 解耦的菜单条目规格（纯数据，单测锚定菜单语义）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuEntrySpec {
    pub label: String,
    /// `Some(v)` = 勾选项（显示 v）；`None` = 普通条目。
    pub checked: Option<bool>,
    pub enabled: bool,
    /// 点击后发送的用户事件（托盘线程唯一被允许的动作）。
    pub event: PetUserEvent,
}

/// 菜单文案常量（测试与实现共用，防止口径漂移）。
pub mod menu_labels {
    pub const CLICK_THROUGH: &str = "点击穿透";
    pub const ALWAYS_ON_TOP: &str = "窗口置顶";
    pub const VISIBLE: &str = "显示窗口";
    pub const EXIT: &str = "退出";
}

/// 三项布尔的不可变快照（纯数据，供菜单模型构建）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraySnapshot {
    pub click_through: bool,
    pub always_on_top: bool,
    pub visible: bool,
}

/// 由状态快照构建菜单模型（纯函数；顺序固定：穿透/置顶/显隐 | 退出）。
pub fn build_menu_model(snapshot: TraySnapshot) -> Vec<MenuEntrySpec> {
    vec![
        MenuEntrySpec {
            label: menu_labels::CLICK_THROUGH.to_string(),
            checked: Some(snapshot.click_through),
            enabled: true,
            event: PetUserEvent::TrayToggleClickThrough,
        },
        MenuEntrySpec {
            label: menu_labels::ALWAYS_ON_TOP.to_string(),
            checked: Some(snapshot.always_on_top),
            enabled: true,
            event: PetUserEvent::TrayToggleAlwaysOnTop,
        },
        MenuEntrySpec {
            label: menu_labels::VISIBLE.to_string(),
            checked: Some(snapshot.visible),
            enabled: true,
            event: PetUserEvent::TrayToggleVisible,
        },
        MenuEntrySpec {
            label: menu_labels::EXIT.to_string(),
            checked: None,
            enabled: true,
            event: PetUserEvent::TrayExit,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::{MenuEntrySpec, TraySnapshot, build_menu_model, menu_labels};
    use crate::user_event::PetUserEvent;

    /// 类型映射：spec 字段命名稳定（外部 `ksni 胶水` 通过这些字段构造菜单项）。
    #[test]
    fn menu_model_has_required_entries_in_order() {
        let model = build_menu_model(TraySnapshot {
            click_through: true,
            always_on_top: false,
            visible: true,
        });
        assert_eq!(model.len(), 4);
        // 1) 交互/点击穿透切换（勾选态随状态）。
        assert_eq!(model[0].label, menu_labels::CLICK_THROUGH);
        assert_eq!(model[0].checked, Some(true));
        assert_eq!(model[0].event, PetUserEvent::TrayToggleClickThrough);
        // 2) 置顶切换。
        assert_eq!(model[1].label, menu_labels::ALWAYS_ON_TOP);
        assert_eq!(model[1].checked, Some(false));
        assert_eq!(model[1].event, PetUserEvent::TrayToggleAlwaysOnTop);
        // 3) 显示/隐藏切换。
        assert_eq!(model[2].label, menu_labels::VISIBLE);
        assert_eq!(model[2].event, PetUserEvent::TrayToggleVisible);
        // 4) 退出（非勾选普通条目）。
        assert_eq!(model[3].label, menu_labels::EXIT);
        assert_eq!(model[3].checked, None);
        assert_eq!(model[3].event, PetUserEvent::TrayExit);
        // 全部条目都必须携带事件（回调只经 proxy 发事件的静态证据）。
        assert!(model.iter().all(|e| e.enabled));
        // 编译期证据：MenuEntrySpec 字段集未漂移（外部胶水按名读取）。
        let _: Option<bool> = MenuEntrySpec {
            label: String::new(),
            checked: None,
            enabled: false,
            event: PetUserEvent::TrayExit,
        }
        .checked;
    }
}
