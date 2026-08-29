//! 窗口后端实测分类与窗口定位算法。
//!
//! 本子模块专注两件事：
//! 1. [`WindowBackendKind`] —— 由 RawWindowHandle 实测分类的真实后端（X11/Wayland），
//!    与环境线索 [`super::capabilities::LinuxSessionHint`] 无关；
//! 2. [`bottom_right_target`] —— 桌面宠"底部右侧"目标位置的纯函数算法。
//!
//! 能力口（`supports_*`）和点击穿透/拖动等本属于后端能力描述的方法也归此模块：
//! 它们是后端协议的反映，不是运行时簿记。

/// 真实窗口系统后端（由 **RawWindowHandle 实测分类**得出，非环境线索）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowBackendKind {
    /// Xlib/Xcb 句柄（Xorg 或 XWayland）。
    X11,
    /// Wayland 句柄（原生 Wayland 合成器）。
    Wayland,
}

impl std::fmt::Display for WindowBackendKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            WindowBackendKind::X11 => "x11",
            WindowBackendKind::Wayland => "wayland",
        })
    }
}

impl WindowBackendKind {
    /// 该后端是否支持客户端侧置顶（`set_window_level`）。
    ///
    /// X11：winit 经 `_NET_WM_STATE_ABOVE` 落地；Wayland：无协议保证，
    /// winit 实现为无操作——因此仅 X11 返回 true（需求 2 的口径）。
    pub const fn supports_always_on_top(self) -> bool {
        matches!(self, Self::X11)
    }

    /// 该后端是否支持客户端全局初始定位（底部右侧 `set_outer_position`）。
    ///
    /// 同 [`Self::supports_always_on_top`] 口径：仅 X11 返回 true；
    /// Wayland 由合成器管理布局，位置请求不生效。
    pub const fn supports_global_position(self) -> bool {
        matches!(self, Self::X11)
    }

    /// 该后端是否支持点击穿透（`set_cursor_hittest`）。
    ///
    /// X11 走 XShape input region、Wayland 走 wl_surface input region，
    /// winit 两个后端都已实现 → 全部 true（不引 x11rb 的依据）。
    pub const fn supports_click_through(self) -> bool {
        true
    }

    /// 该后端是否支持交互态拖动窗口（`drag_window`）。
    ///
    /// X11 走指针抓取、Wayland 走 xdg_toplevel.move，两者均已实现 → true。
    pub const fn supports_drag_window(self) -> bool {
        true
    }
}

/// 计算"显示器底部右侧"的窗口左上角目标位置（物理像素，纯函数）。
///
/// 输入全部为显式值便于单测：显示器原点/尺寸、窗口外框尺寸与边距；
/// 结果会钳制在显示器范围内（窗口比屏幕大时贴边不越界）。
pub fn bottom_right_target(
    monitor_origin: (i32, i32),
    monitor_size: (u32, u32),
    window_size: (u32, u32),
    margin_px: i32,
) -> (i32, i32) {
    let (mx, my) = monitor_origin;
    let mw = i32::try_from(monitor_size.0).unwrap_or(i32::MAX);
    let mh = i32::try_from(monitor_size.1).unwrap_or(i32::MAX);
    let ww = i32::try_from(window_size.0.max(1)).unwrap_or(i32::MAX);
    let wh = i32::try_from(window_size.1.max(1)).unwrap_or(i32::MAX);
    let margin = margin_px.max(0);
    let x = mx + mw - ww - margin;
    let y = my + mh - wh - margin;
    // 钳制到显示器范围（含边距内），避免负坐标漂移到其他显示器。
    (
        x.clamp(mx, (mx + mw - ww).max(mx)),
        y.clamp(my, (my + mh - wh).max(my)),
    )
}

#[cfg(test)]
mod tests {
    use super::{WindowBackendKind, bottom_right_target};

    #[test]
    fn window_backend_kind_support_matrix_matches_protocol_reality() {
        // 置顶/全局定位：仅 X11（winit 在 Wayland 为无操作/合成器管理布局）。
        assert!(WindowBackendKind::X11.supports_always_on_top());
        assert!(!WindowBackendKind::Wayland.supports_always_on_top());
        assert!(WindowBackendKind::X11.supports_global_position());
        assert!(!WindowBackendKind::Wayland.supports_global_position());
        // 点击穿透/拖动：两种后端都支持（XShape / input region、指针抓取 / xdg move）。
        assert!(WindowBackendKind::X11.supports_click_through());
        assert!(WindowBackendKind::Wayland.supports_click_through());
        assert!(WindowBackendKind::X11.supports_drag_window());
        assert!(WindowBackendKind::Wayland.supports_drag_window());
        // Display 文案稳定（日志/报告词汇）。
        assert_eq!(WindowBackendKind::X11.to_string(), "x11");
        assert_eq!(WindowBackendKind::Wayland.to_string(), "wayland");
    }

    #[test]
    fn bottom_right_target_places_window_inside_monitor_with_margin() {
        // 1920x1080 显示器 + 480x640 窗口 + 16px 边距。
        assert_eq!(
            bottom_right_target((0, 0), (1920, 1080), (480, 640), 16),
            (1920 - 480 - 16, 1080 - 640 - 16)
        );
        // 多显示器原点偏移被叠加。
        assert_eq!(
            bottom_right_target((1920, 0), (1920, 1080), (480, 640), 16),
            (1920 + 1920 - 480 - 16, 1080 - 640 - 16)
        );
        // 窗口比屏幕大：钳制在原点，不越界。
        assert_eq!(
            bottom_right_target((0, 0), (800, 600), (1200, 900), 16),
            (0, 0)
        );
        // 负边距按 0 处理。
        let (x, y) = bottom_right_target((0, 0), (1920, 1080), (480, 640), -5);
        assert_eq!((x, y), (1920 - 480, 1080 - 640));
    }
}
