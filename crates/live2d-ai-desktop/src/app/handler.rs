//! `ApplicationHandler<AppEvent>` 实现（事件循环主骨架）。
//!
//! - [`ShellApp::resumed`]：转交 `bootstrap`。
//! - [`ShellApp::user_event`]：托盘 / 对话 UI / 错误 / 生命周期事件分发。
//! - [`ShellApp::window_event`]：关闭 / 鼠标左键 / Resize / DPI / Redraw /
//!   Occluded 等窗口事件。
//! - [`ShellApp::about_to_wait`]：shutdown watchdog + 超时兜底 + 初始定位
//!   回读验证 + 持续重绘。
//! - [`ShellApp::new_events`]：仅在 `StartCause::Init` 打印后端说明。
//! - [`ShellApp::exiting`]：托盘收尾 + 运行总结日志。

use tracing::{info, trace, warn};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, StartCause, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::WindowId;

use crate::app::types::ShellApp;
use crate::app_event::AppEvent;
use crate::backend::{StopReason, WINIT_WGPU_NOTES};

impl ApplicationHandler<AppEvent> for ShellApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() && self.state.is_none() {
            self.bootstrap(event_loop);
        }
    }

    /// 托盘/外部线程用户事件：唯一跨线程入口，落地全在本线程。
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::Tray(pe) => self.handle_tray_event(event_loop, pe),
            AppEvent::Conversation(ui) => self.apply_conversation_ui(ui),
            // root 审计投影：窗口侧不消费，只留调试通道（测试经 collector 断言）。
            AppEvent::RootAudit(fact) => {
                tracing::debug!(?fact, "root 审计事实");
            }
            // 链路错误（2026-09-11）：窗口侧不消费——它已经由 supervisor
            // `tracing::error!` 落盘、并经 WS `error` 帧上屏。这里只留一条
            // debug 通道，保证「谁在监听 AppEvent」的测试仍能观察到。
            AppEvent::Error(err) => {
                tracing::debug!(code = %err.code, epoch = err.epoch, "链路错误（窗口侧观察通道）");
            }
            AppEvent::ShutdownReady => {
                info!("shutdown handshake 完成（supervisor 清理完毕），退出事件循环");
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        // 先判归属，避免跨字段借用冲突。
        if !self.window.as_ref().is_some_and(|w| w.id() == window_id) {
            return;
        }
        // F10 显隐：仅在 KeyboardInput::Pressed 时触发；其余时刻照常走下面的分支。
        // 必须在窗口归属判别之后、egui 事件路由之前——保证 F10 即便在面板可见时
        // 也能反向隐藏（避免 egui 把它当普通 key 吞掉）。
        if let WindowEvent::KeyboardInput {
            event:
                winit::event::KeyEvent {
                    physical_key: PhysicalKey::Code(KeyCode::F10),
                    state: ElementState::Pressed,
                    ..
                },
            ..
        } = &event
        {
            if let Some(ui) = self.settings_ui.as_mut() {
                crate::app::settings_ui::toggle_visible(ui);
            }
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return;
        }
        // 面板可见 → 事件先送 egui：消费即吞（节点 C 锁定：桌宠穿透 / 拖动不丢）。
        // `should_route_to_ui` 与 settings_ui 内部一致（visible && consumed）。
        if let Some(ui) = self.settings_ui.as_mut()
            && let Some(window) = self.window.as_ref()
        {
            let resp = crate::app::settings_ui::handle_window_event(ui, window, &event);
            if crate::app::settings_ui::should_route_to_ui(ui.visible, resp.consumed) {
                return;
            }
        }
        match event {
            WindowEvent::CloseRequested => {
                self.stop_reason.get_or_insert(StopReason::WindowClosed);
                // D13：与 /quit、TrayExit 共用同一 shutdown handshake。
                self.request_shutdown(event_loop);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                // 交互态左键 = 拖动窗口（穿透模式下收不到本事件）。
                if let Some(window) = self.window.clone() {
                    self.begin_interactive_drag(&window);
                }
            }
            WindowEvent::Resized(size) => {
                if size.width == 0 || size.height == 0 {
                    // 折叠/最小化：保持现配置，绘制侧按尺寸守卫跳过。
                    trace!("resize 到零尺寸（最小化？）：跳过重配置");
                    return;
                }
                // C4 裁决 P0-3 latest-size 合并：
                // - 只写 pending 并请求 redraw，**不立即 configure**；
                // - 同一事件批次的多次 Resized 由 `merge_pending_resize`
                //   天然合并到最后非零值（后写覆盖前写）；
                // - 下一次 redraw/acquire 前由 `apply_pending_resize`
                //   统一落地 configure + 同步 viewport。
                // 不使用固定毫秒防抖（制造视觉延迟）。
                self.pending_surface_size =
                    crate::app::surface::merge_pending_resize(self.pending_surface_size, size);
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                trace!(
                    width = size.width,
                    height = size.height,
                    "resize recorded as pending; request redraw"
                );
            }
            WindowEvent::ScaleFactorChanged {
                mut inner_size_writer,
                ..
            } => {
                // DPI 变化时保持物理像素尺寸不变（避免窗口视觉跳变）；
                // 应答后 winit 会补发 Resized 事件驱动 surface 重配置。
                if let Some(window) = &self.window {
                    let size = window.inner_size();
                    if let Err(e) = inner_size_writer.request_inner_size(size) {
                        warn!("ScaleFactorChanged 应答 inner size 失败: {e}");
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if self.stop_reason.is_none() {
                    // C4 裁决 P0-3：在 redraw 开始前消费 pending resize，
                    // 落地 latest-size configure + 同步视口。
                    self.apply_pending_resize();
                    self.render_frame(event_loop);
                    if let Some(target) = self.frame_target_reached() {
                        info!(frames = target, "达到冒烟帧数目标，自动退出");
                        self.stop_reason
                            .get_or_insert(StopReason::FrameTargetReached(target));
                        event_loop.exit();
                    }
                }
                // model smoke 类型化失败：立即结束事件循环，由 run_shell
                // 映射为退出码 1（资产/模型代码）或 3（GPU 环境）。
                // environment_failure 由 `fail_environment` 路径直接
                // `event_loop.exit()`，无需在此重复处理。
                if self.model_failure.is_some() {
                    event_loop.exit();
                }
            }
            WindowEvent::Occluded(true) => {
                trace!("window occluded");
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // shutdown handshake 兜底（D13）。
        if self.tick_shutdown_watchdog(event_loop) {
            return;
        }
        // 超时兜底（CI 冒烟防挂死）。
        if self.stop_reason.is_none()
            && let Some(timeout) = self.options.effective_timeout()
            && self.started_at.elapsed() >= timeout
        {
            warn!(timeout = ?timeout, "smoke 超时兜底触发，自动退出");
            self.stop_reason
                .get_or_insert(StopReason::TimeoutElapsed(timeout));
            event_loop.exit();
            return;
        }
        // 初始定位回读验证：等前两帧 present 后再核对（X11 需要往返窗口）。
        if self.pending_position.is_some() && self.frames_presented >= 2 {
            self.verify_pending_position();
        }
        // 连续重绘：Poll 控制流 + 每轮主动请求下一帧。
        if self.stop_reason.is_none()
            && let Some(window) = &self.window
        {
            window.request_redraw();
        }
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if cause == StartCause::Init {
            info!(
                backend_notes = ?WINIT_WGPU_NOTES,
                session_hint = %self.session_hint,
                "窗口壳启动（winit-wgpu backend）"
            );
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        // 托盘服务收尾（幂等；迟到就绪的实例也在集合里，一并关闭）。
        self.tray.shutdown();
        info!(
            presented = self.frames_presented,
            skipped = self.frames_skipped,
            reason = ?self.stop_reason,
            backend = ?self.runtime.backend,
            click_through_active = self.click_through_active,
            always_on_top_active = self.always_on_top_active,
            "窗口壳事件循环结束"
        );
    }
}
