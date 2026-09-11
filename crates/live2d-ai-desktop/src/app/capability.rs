//! 桌宠窗口能力 + wgpu/surface 协商纯函数。
//!
//! - 能力方法（[`ShellApp`] 上）：后端探测 / 初始定位 / 回读验证 / 置顶 /
//!   点击穿透 / 显隐 / 托盘同步 / 拖动。
//! - 纯函数（`fn`）：`RawWindowHandle`/`RawDisplayHandle` → `WindowBackendKind`
//!   分类；wgpu 纹理格式选择；alpha 模式观测/回转。

use raw_window_handle::{
    HasDisplayHandle as _, HasWindowHandle as _, RawDisplayHandle, RawWindowHandle,
};
use tracing::{info, trace, warn};
use winit::dpi::PhysicalPosition;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowLevel};

use crate::app::types::ShellApp;
use crate::platform::{CapabilityState, ObservedAlphaMode, WindowBackendKind, bottom_right_target};

impl ShellApp {
    /// RawWindowHandle 实测分类：Xlib/Xcb → X11，Wayland → Wayland（纯映射，
    /// 见 [`classify_raw_window_handle`]）。结果记入 runtime.backend。
    pub(crate) fn detect_backend(&mut self, window: &Window) {
        let window_kind = window
            .window_handle()
            .ok()
            .map(|h| h.as_raw())
            .and_then(classify_raw_window_handle);
        let display_kind = window
            .display_handle()
            .ok()
            .map(|h| h.as_raw())
            .and_then(classify_raw_display_handle);
        // 窗口句柄优先；缺失时退回 display 句柄（两者应一致）。
        let kind = window_kind.or(display_kind);
        self.runtime.backend = kind;
        match (window_kind, display_kind) {
            (Some(w), Some(d)) if w != d => {
                warn!(window=%w, display=%d, "窗口/显示句柄后端不一致（罕见），以窗口句柄为准")
            }
            _ => {}
        }
        if let Some(kind) = kind {
            info!(backend = %kind, "实测窗口后端（RawWindowHandle 分类，非环境线索）");
        } else {
            warn!("无法从 RawWindowHandle 识别后端（保持 unknown，不做能力假设）");
        }
    }

    /// 底部右侧初始定位：所有后端都**请求**；是否生效由回读验证决定
    /// （X11 预期 available；Wayland 合成器管理布局预期 unavailable）。
    pub(crate) fn request_initial_position(
        &mut self,
        event_loop: &ActiveEventLoop,
        window: &Window,
    ) {
        let monitor = event_loop
            .primary_monitor()
            .or_else(|| event_loop.available_monitors().next());
        let Some(monitor) = monitor else {
            warn!("无可用显示器信息：跳过初始定位");
            return;
        };
        if self
            .runtime
            .backend
            .is_some_and(|kind| !kind.supports_global_position())
        {
            info!(
                backend = ?self.runtime.backend,
                "当前后端由合成器管理窗口布局：位置请求预期不生效（以回读验证为准）"
            );
        }
        let m_pos = monitor.position();
        let m_size = monitor.size();
        let win_size = window.outer_size();
        let margin_px = (16.0 * window.scale_factor()).round() as i32;
        let target = bottom_right_target(
            (m_pos.x, m_pos.y),
            (m_size.width, m_size.height),
            (win_size.width, win_size.height),
            margin_px,
        );
        window.set_outer_position(PhysicalPosition::new(target.0, target.1));
        self.pending_position = Some(target);
        info!(
            ?target,
            margin_px,
            monitor = monitor.name().unwrap_or_default(),
            "初始定位请求（屏幕底部右侧）"
        );
    }

    /// 回读验证初始定位是否生效（帧数 ≥2 后调用一次）。
    ///
    /// 只依据 `outer_position()` 的真实回读结果记录 global_position——
    /// 不凭「调用了 set_outer_position」就声称成功。
    pub(crate) fn verify_pending_position(&mut self) {
        let Some(target) = self.pending_position.take() else {
            return;
        };
        let Some(window) = &self.window else {
            return;
        };
        let tolerance = (8.0 * self.scale_factor).round().max(4.0) as i32;
        match window.outer_position() {
            Ok(actual) => {
                let dx = (actual.x - target.0).abs();
                let dy = (actual.y - target.1).abs();
                if dx <= tolerance && dy <= tolerance {
                    self.runtime.global_position = CapabilityState::Available;
                    info!(?actual, ?target, "全局定位生效（回读验证通过）");
                } else {
                    self.runtime.global_position = CapabilityState::Unavailable;
                    warn!(
                        ?actual,
                        ?target,
                        "全局定位未生效（合成器未采纳位置请求，按 unavailable 记录）"
                    );
                }
            }
            Err(e) => {
                self.runtime.global_position = CapabilityState::Unavailable;
                warn!("outer_position 回读失败（按 unavailable 记录）: {e}");
            }
        }
    }

    /// 应用置顶意图并如实记录能力（仅 X11 生效；Wayland 协议不支持）。
    pub(crate) fn apply_always_on_top(&mut self, window: &Window, want: bool) {
        match self.runtime.backend {
            Some(kind) if kind.supports_always_on_top() => {
                window.set_window_level(if want {
                    WindowLevel::AlwaysOnTop
                } else {
                    WindowLevel::Normal
                });
                self.runtime.always_on_top = CapabilityState::Available;
                self.always_on_top_active = want;
                info!(
                    enabled = want,
                    level = if want { "always-on-top" } else { "normal" },
                    "set_window_level 已应用"
                );
            }
            Some(kind) => {
                self.runtime.always_on_top = CapabilityState::Unavailable;
                self.always_on_top_active = false;
                if want {
                    warn!(
                        "{kind} 后端不支持置顶（合成器协议限制，winit 为无操作），已降级为普通层级"
                    );
                }
            }
            None => {
                trace!("后端未识别：置顶能力保持 unknown");
            }
        }
    }

    /// 业务布尔 → winit hittest 布尔（C9-P0 纯函数；单测锁定方向契约）。
    ///
    /// 业务语义：`want=true` 表示「启用点击穿透」（事件穿过窗口）；
    /// winit 契约：`set_cursor_hittest(true)` 捕获事件、`false` 放行。
    /// 二者方向相反，恒取反。
    pub(crate) const fn click_through_to_hittest(want: bool) -> bool {
        !want
    }

    /// 应用点击穿透意图并如实记录能力（两种后端均可；失败保持原状态）。
    pub(crate) fn apply_click_through(&mut self, window: &Window, want: bool) {
        // C9-P0（节点 C 正式裁决）：winit 契约是 `set_cursor_hittest(true)`
        // 捕获鼠标事件、`false` 才让事件穿过窗口。业务语义 `want=true` 表示
        // 「启用点击穿透」，方向相反，必须取反后传给 winit。
        match window.set_cursor_hittest(Self::click_through_to_hittest(want)) {
            Ok(()) => {
                self.runtime.click_through = CapabilityState::Available;
                self.click_through_active = want;
                info!(
                    enabled = want,
                    mode = if want { "click-through" } else { "interactive" },
                    "set_cursor_hittest 已应用"
                );
            }
            Err(e) => {
                warn!(
                    want,
                    current = self.click_through_active,
                    "set_cursor_hittest 失败（保持原状态）: {e}"
                );
            }
        }
    }

    /// 应用显隐意图（托盘「显示窗口」恢复入口的落地端）。
    pub(crate) fn apply_visible(&mut self, window: &Window, want: bool) {
        window.set_visible(want);
        self.window_visible = want;
        info!(visible = want, "set_visible 已应用");
    }

    /// 窗口态变更后同步托盘共享簿记并请求 host 刷新菜单。
    pub(crate) fn sync_tray(&self) {
        if let Some(state) = self.tray.state() {
            state.set_click_through(self.click_through_active);
            state.set_always_on_top(self.always_on_top_active);
            state.set_visible(self.window_visible);
        }
        self.tray.refresh();
    }

    /// 左键按下且处于交互态时发起拖动（穿透中收不到输入，天然互斥）。
    pub(crate) fn begin_interactive_drag(&mut self, window: &Window) {
        if self.click_through_active {
            return;
        }
        if self
            .runtime
            .backend
            .is_some_and(|kind| !kind.supports_drag_window())
        {
            trace!(backend = ?self.runtime.backend, "当前后端不支持 drag_window：跳过");
            return;
        }
        match window.drag_window() {
            Ok(()) => {
                if !self.drag_recorded {
                    self.drag_recorded = true;
                    self.runtime.drag_move = CapabilityState::Available;
                    info!("drag_window 成功（交互态左键拖动可用）");
                }
            }
            Err(e) => trace!("drag_window 未生效（可重试，不影响能力判定）: {e}"),
        }
    }
}

/// RawWindowHandle → 真实后端分类（需求 2：Xlib/Xcb 标 X11，Wayland 标 Wayland）。
///
/// 纯映射、可单测（句柄类型可无显示环境构造）；未知句柄返回 None
/// （调用方保持 unknown，不做任何能力假设）。
pub(crate) fn classify_raw_window_handle(handle: RawWindowHandle) -> Option<WindowBackendKind> {
    match handle {
        RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_) => Some(WindowBackendKind::X11),
        RawWindowHandle::Wayland(_) => Some(WindowBackendKind::Wayland),
        _ => None,
    }
}

/// RawDisplayHandle → 同口径分类（窗口句柄缺失时的回退证据）。
pub(crate) fn classify_raw_display_handle(handle: RawDisplayHandle) -> Option<WindowBackendKind> {
    use raw_window_handle::RawDisplayHandle as H;
    match handle {
        H::Xlib(_) | H::Xcb(_) => Some(WindowBackendKind::X11),
        H::Wayland(_) => Some(WindowBackendKind::Wayland),
        _ => None,
    }
}

/// 从 surface 实测格式列表中选择 shell 主格式：
/// 优先带 alpha 的非 sRGB 格式（透明桌宠基础；色彩管理后续批次再细化）。
pub(crate) fn choose_surface_format(
    formats: &[wgpu::TextureFormat],
) -> Option<wgpu::TextureFormat> {
    const PREFERRED: [wgpu::TextureFormat; 4] = [
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Bgra8UnormSrgb,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ];
    PREFERRED
        .into_iter()
        .find(|f| formats.contains(f))
        .or_else(|| formats.first().copied())
}

/// wgpu CompositeAlphaMode → 自有观测枚举（隔离第三方类型于 platform.rs 之外）。
pub(crate) const fn observed_alpha_mode(mode: wgpu::CompositeAlphaMode) -> ObservedAlphaMode {
    match mode {
        wgpu::CompositeAlphaMode::PreMultiplied => ObservedAlphaMode::PreMultiplied,
        wgpu::CompositeAlphaMode::PostMultiplied => ObservedAlphaMode::PostMultiplied,
        wgpu::CompositeAlphaMode::Inherit => ObservedAlphaMode::Inherit,
        // Auto/Opaque 都按“未确认尊重 alpha”口径折叠：
        // Auto 只作缺省回写值，不会作为显式选择被匹配。
        wgpu::CompositeAlphaMode::Opaque | wgpu::CompositeAlphaMode::Auto => {
            ObservedAlphaMode::Opaque
        }
    }
}

/// 自有观测枚举 → wgpu CompositeAlphaMode（回转，用于写回配置）。
pub(crate) const fn wgpu_alpha_mode(mode: ObservedAlphaMode) -> wgpu::CompositeAlphaMode {
    match mode {
        ObservedAlphaMode::PreMultiplied => wgpu::CompositeAlphaMode::PreMultiplied,
        ObservedAlphaMode::PostMultiplied => wgpu::CompositeAlphaMode::PostMultiplied,
        ObservedAlphaMode::Inherit => wgpu::CompositeAlphaMode::Inherit,
        ObservedAlphaMode::Opaque => wgpu::CompositeAlphaMode::Opaque,
    }
}

#[cfg(test)]
mod tests {
    use super::ShellApp;

    // C9-P0（节点 C 正式裁决）：业务布尔 → winit hittest 布尔方向相反。
    // 三条断言锁住契约：
    //   1. 业务 want=true（启用点击穿透）→ hittest=false（让事件穿过窗口）。
    //   2. 业务 want=false（交互）       → hittest=true（捕获事件）。
    //   3. 连续两次取反回到原值（幂等，便于上游无状态回放）。

    #[test]
    fn click_through_to_hittest_inverts_click_through_intent() {
        // 启用点击穿透 → winit 必须放行（hittest=false）。
        assert!(!ShellApp::click_through_to_hittest(true));
    }

    #[test]
    fn click_through_to_hittest_keeps_interactive_intent() {
        // 交互态 → winit 必须捕获（hittest=true）。
        assert!(ShellApp::click_through_to_hittest(false));
    }

    #[test]
    fn click_through_to_hittest_is_idempotent_under_double_negation() {
        for want in [true, false] {
            let once = ShellApp::click_through_to_hittest(want);
            let twice = ShellApp::click_through_to_hittest(once);
            assert_eq!(twice, want, "两次取反应回到原值（want={want}）");
        }
    }
}
