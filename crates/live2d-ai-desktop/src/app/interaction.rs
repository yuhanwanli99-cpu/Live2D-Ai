//! 托盘 / 渲染命令 / 对话 UI 事件落地（事件循环线程内）。
//!
//! - [`ShellApp::handle_tray_event`]：托盘菜单事件 → 窗口能力意图 → `Window`
//!   调用，幂等。
//! - [`ShellApp::apply_render_command`]：渲染命令落地（含 epoch 镜像闸门）。
//! - [`ShellApp::apply_conversation_ui`]：对话侧状态镜像（epoch 刷新与
//!   口型活跃门控）。

use tracing::{debug, info, trace};
use winit::event_loop::ActiveEventLoop;

use crate::app::types::ShellApp;
use crate::app_event::{ConversationUiEvent, RenderCommand};
use crate::backend::StopReason;
use crate::user_event::{EventTarget, PetUserEvent, event_target};

impl ShellApp {
    /// 处理托盘用户事件（事件循环线程内；所有 Window 调用都在这里落地）。
    pub(crate) fn handle_tray_event(&mut self, event_loop: &ActiveEventLoop, event: PetUserEvent) {
        let Some(window) = self.window.clone() else {
            trace!(?event, "窗口尚未创建：忽略托盘事件");
            return;
        };
        let target = match event {
            PetUserEvent::TrayToggleClickThrough | PetUserEvent::TraySetClickThrough(_) => {
                event_target(event, self.click_through_active)
            }
            PetUserEvent::TrayToggleAlwaysOnTop => event_target(event, self.always_on_top_active),
            // Toggle 与 SetVisible（托盘图标左键激活发显式值）都归到这里。
            PetUserEvent::TrayToggleVisible | PetUserEvent::TraySetVisible(_) => {
                event_target(event, self.window_visible)
            }
            PetUserEvent::TrayExit => None,
        };
        match target {
            Some(EventTarget::ClickThrough(want)) => self.apply_click_through(&window, want),
            Some(EventTarget::AlwaysOnTop(want)) => self.apply_always_on_top(&window, want),
            Some(EventTarget::Visible(want)) => self.apply_visible(&window, want),
            Some(EventTarget::Exit) | None => {}
        }
        if event == PetUserEvent::TrayExit {
            info!("托盘菜单请求退出");
            self.stop_reason.get_or_insert(StopReason::TrayExit);
            self.request_shutdown(event_loop);
            return;
        }
        self.sync_tray();
    }

    /// 渲染命令落地（副作用执行端；epoch 镜像防御闸门，权威闸在 core）。
    pub(crate) fn apply_render_command(&mut self, cmd: RenderCommand) {
        let (cmd_epoch, kind) = match cmd {
            RenderCommand::Perform { epoch, action } => (epoch, Some(action)),
            RenderCommand::CeasePerforming { epoch } => (epoch, None),
        };
        if cmd_epoch != self.render_epoch {
            debug!(
                cmd_epoch,
                render_epoch = self.render_epoch,
                ?kind,
                "陈旧渲染命令被镜像闸门丢弃"
            );
            return;
        }
        match kind {
            Some(action) => {
                if let Some(model) = self.model.as_mut() {
                    model.driver.player_mut().play(action);
                }
                self.performing = Some((cmd_epoch, action));
                info!(?action, "表演开始（chat）");
            }
            None => {
                if let Some(model) = self.model.as_mut() {
                    model.driver.player_mut().interrupt();
                }
                self.performing = None;
            }
        }
    }

    /// 对话侧状态落地：镜像刷新与口型活跃门控。
    pub(crate) fn apply_conversation_ui(&mut self, ui: ConversationUiEvent) {
        match ui {
            ConversationUiEvent::NewEpoch { epoch } => {
                // 权威下发，无条件刷新（含 stop 后推进）。
                self.render_epoch = epoch;
            }
            ConversationUiEvent::VoiceStarted { epoch } => {
                if epoch == self.render_epoch {
                    self.voice_active = true;
                    info!("口型活跃窗口开启（首个 PCM 入环）");
                }
            }
            ConversationUiEvent::VoiceEnded { .. } => {
                // 不匹配也照收：静默是安全方向（宁可少动嘴）。
                self.voice_active = false;
            }
            // P1WS-1：TextDelta 走 WS 通道给 web 前端，winit 路径不显示文本——
            // 仅维护 render_epoch 镜像即可（已被 NewEpoch 权威下发过）。
            ConversationUiEvent::TextDelta { .. } => {}
        }
    }
}
