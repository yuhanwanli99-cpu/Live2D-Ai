//! 每帧取帧与渲染。
//!
//! - [`ShellApp::draw_and_present_clear`]：编码一帧全透明清屏并 present
//!   （纯透明壳路径；不含取帧错误处理）。
//! - [`ShellApp::draw_and_present_model`]：Live2D 实时渲染一帧（真实 dt →
//!   固定步模拟 → 参数写入 → 渲染到 surface view → present）。失败为类型化
//!   冒烟错误。
//! - [`ShellApp::render_frame`]：取帧 → 分类处理 → 计数（Presented /
//!   Suboptimal / Outdated / Lost / Timeout / Occluded / Validation 逐类处理）。
//! - [`ShellApp::frame_target_reached`]：冒烟退出条件（帧数目标）。
//!
//! 实时路径已切换为 C1 双 API 闭环：
//! - `draw_and_present_model` 调 `render_to_view_submit` 获取 `SubmissionIndex`
//!   并记录到 `SurfaceState.last_submission_index`，**不再阻塞等待**；每帧末尾
//!   仍做一次 `device.poll(Poll)` 推进上传/回调管线。
//! - 异步 GPU 错误经 `Device::on_uncaptured_error` 写入 `SurfaceState`
//!   的 `gpu_fault_latch`（`Arc<Mutex<Option<GpuFault>>>`），由 `render_frame`
//!   每帧开始时消费并映射到现有退出码分类（C2 裁决第 53-59 行）：
//!   `Validation` → `ModelCode`（退出 1）；`OutOfMemory`/`Internal` → 环境错误
//!   （退出 3）。映射由纯函数 [`route_gpu_fault`] 完成（tests.rs 断言）。
//! - `Device::poll` 失败由 [`classify_poll_failure`] 映射为
//!   `ModelSmokeError::GpuEnvironment`（C2 裁决 → 退出 3）。
//!
//! W6（egui 0.35 overlay 接线）：`draw_and_present_clear`/`draw_and_present_model`
//! 末尾 present 前调 [`crate::app::settings_ui::render_overlay`]——清屏路径与
//! 主渲染共用同一次 submit；模型路径因 `l2d::render_to_view_submit` 内部已 submit，
//! 走第二次非阻塞 submit（同帧、均无 `PollType::Wait`）。文件略超 510 行
//! 头注豁免一行（W6 接线净增 31 行）。

use std::time::Instant;

use live2d_ai_core::SampleStatus;
use tracing::{debug, error, info, trace, warn};
use winit::event_loop::ActiveEventLoop;

use crate::app::surface::{
    AcquireOutcome, StreakDecision, SurfaceAction, advance_validation_streak, decide_after_acquire,
};
use crate::app::types::ShellApp;
use crate::model_smoke::{
    ModelSmokeError, SIM_DT, TickOutcome, classify_frame_error, synthesized_mouth_level,
};

/// 异步 GPU 故障的归类目标（`on_uncaptured_error` 写闩的消费结果）。
///
/// 不直接动 `model_failure`/`environment_failure`，由调用方决定落地时机——
/// 这样可以在事件循环线程**消费即退出**（C2 第 62 行），同时让 model 与
/// transparent 两条路径共用同一套映射。
///
/// `pub(crate)`：纯函数 [`route_gpu_fault`] 返回此枚举供调用方与测试使用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FaultRouting {
    /// 代码/资源使用错误（Validation）→ `model_failure` → 退出码 1。
    ModelCode(String),
    /// 设备/驱动失效（OOM/Internal）→ `environment_failure` → 退出码 3。
    Environment(String),
}

/// 异步 GPU 故障分类（来自 `wgpu::Device::on_uncaptured_error`）。
///
/// 分类映射由 C2 裁决第 53-59 行约束：
/// - `Validation` → 代码/资源使用错误 → 退出码 1
/// - `OutOfMemory` → GPU 运行环境不足 → 退出码 3
/// - `Internal` → 驱动/后端内部失效 → 退出码 3
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GpuFaultKind {
    /// `wgpu::Error::Validation`（代码/资源使用错误）。
    Validation,
    /// `wgpu::Error::OutOfMemory`（GPU 运行环境不足）。
    OutOfMemory,
    /// `wgpu::Error::Internal`（驱动/后端内部失效）。
    Internal,
}

impl GpuFaultKind {
    /// 分类名（日志/报告用）。
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Validation => "validation",
            Self::OutOfMemory => "out-of-memory",
            Self::Internal => "internal",
        }
    }
}

/// 一次异步 GPU 故障的快照（写闩的载荷）。
///
/// 写闩策略：仅保留**最新**一条；回调可能异步多次触发，最近一次代表当前
/// 设备状态，事件循环线程一帧一消费。`description` 来自 wgpu 错误描述；
/// `source_chain` 是 `Error::source()` 链的字符串化（深度有限，循环打断）。
#[derive(Debug, Clone)]
pub(crate) struct GpuFault {
    pub(crate) kind: GpuFaultKind,
    pub(crate) description: String,
    pub(crate) source_chain: String,
}

/// 把 `GpuFault` 映射到 [`FaultRouting`] 的纯函数（C2 裁决第 53-59 行）：
/// `Validation → ModelCode`（退出 1）；`OutOfMemory`/`Internal → Environment`
/// （退出 3）。`detail` 由调用方提供（adapter / submission 等），纯透传。
pub(crate) fn route_gpu_fault(kind: GpuFaultKind, detail: String) -> FaultRouting {
    match kind {
        GpuFaultKind::Validation => FaultRouting::ModelCode(format!(
            "GPU uncaptured validation error（代码错误，退出码 1）: {detail}"
        )),
        GpuFaultKind::OutOfMemory => FaultRouting::Environment(format!(
            "GPU uncaptured out-of-memory（环境错误，退出码 3）: {detail}"
        )),
        GpuFaultKind::Internal => FaultRouting::Environment(format!(
            "GPU uncaptured internal error（环境错误，退出码 3）: {detail}"
        )),
    }
}

/// 把 `Device::poll` 失败映射为 `ModelSmokeError::GpuEnvironment`（C2 第 58
/// 行 → 退出 3）。`path` = 调用方标识（`"transparent-shell"` / `"model"`），
/// 用于日志区分。`submission` / `adapter` 仅作可读上下文。
pub(crate) fn classify_poll_failure(
    path: &'static str,
    wgpu_err: String,
    submission: Option<wgpu::SubmissionIndex>,
    adapter: &str,
) -> ModelSmokeError {
    let msg = format!(
        "device poll 失败（{path} GPU 环境错误，退出码 3）: {wgpu_err:?}; last_submission={submission:?}, adapter={adapter}",
    );
    ModelSmokeError::GpuEnvironment(msg)
}

impl ShellApp {
    /// 编码一帧全透明清屏并 present（纯透明壳路径；不含取帧错误处理）。
    ///
    /// 实时非阻塞（C1+C2 闭环）：encoder 提交即返回，不等 GPU；`device.poll`
    /// **失败升级为 fatal**（之前只是 warn），由调用方在事件循环线程上把
    /// 设备/队列失效归类为环境错误并退出。
    pub(crate) fn draw_and_present_clear(
        &mut self,
        event_loop: &ActiveEventLoop,
        frame: wgpu::SurfaceTexture,
    ) {
        let Some(state) = &mut self.state else {
            return;
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = state
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("live2d-ai-desktop-shell-frame"),
            });
        {
            // 清屏为全透明：桌面宠底色（模型渲染由后续批次接入）。
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("live2d-ai-desktop-shell-clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        // W6：egui overlay 与本帧 submit 复用同一 encoder（visible=false → 内部 0 开销）。
        if let (Some(ui), Some(window)) = (self.settings_ui.as_mut(), self.window.as_ref()) {
            let screen_px = [state.config.width as f32, state.config.height as f32];
            crate::app::settings_ui::render_overlay(
                ui,
                &state.device,
                &state.queue,
                &mut encoder,
                &view,
                screen_px,
                window,
            );
        }
        // 记录最新 submission 序号——异步 uncaptured-error handler 写入 fault
        // 上下文时一并读取，输出"最近一次提交"便于定位问题帧（C2 第 63 行）。
        state.last_submission_index = Some(state.queue.submit(Some(encoder.finish())));
        frame.present();

        // 泵送设备（非阻塞）—— Poll 失败即设备/队列失效，按 C2 第 58 行归为
        // 环境错误（退出码 3），不再仅 warn。分类走 [`classify_poll_failure`]
        // 纯函数。
        if let Err(e) = state.device.poll(wgpu::PollType::Poll) {
            let err = classify_poll_failure(
                "transparent-shell",
                format!("{e:?}"),
                state.last_submission_index.clone(),
                &self.gpu_adapter,
            );
            let msg = err.to_string();
            self.fail_environment(event_loop, msg);
        }
    }

    /// Live2D 实时渲染一帧：真实 dt → 固定步模拟 → 参数写入 → 渲染到
    /// **surface view** → present。失败为类型化冒烟错误。
    ///
    /// 耗时口径（C1 闭环后）：调 `render_to_view_submit` 后**只记录 SubmissionIndex
    /// 不等待**，本函数测得的帧耗时是**纯 CPU 提交成本**（与 C7 benchmark
    /// 的 `submit_ms` 对齐，不再含 GPU 等待）。
    pub(crate) fn draw_and_present_model(
        &mut self,
        event_loop: &ActiveEventLoop,
        frame: wgpu::SurfaceTexture,
    ) -> Result<(), ModelSmokeError> {
        let started = Instant::now();

        // ---- 1. 真实 dt（截断到合理范围）→ 60Hz 固定步模拟 ----
        let now = Instant::now();
        let wall_dt = self
            .last_frame_at
            .map_or(0.0, |t| now.duration_since(t).as_secs_f32());
        self.last_frame_at = Some(now);
        let ticks = self.clock.advance(wall_dt);

        let mut natural_finish: Option<(u64, live2d_ai_core::SemanticAction)> = None;
        {
            let model = self.model.as_mut().ok_or_else(|| {
                ModelSmokeError::ModelCode("draw_and_present_model 无模型状态".to_owned())
            })?;
            let chat_mode = self.chat.is_some();
            for _ in 0..ticks {
                if !chat_mode {
                    // 表演曲线推进一固定步；动作结束即切换下一动作并记录日志。
                    match model.driver.tick(&mut model.frame_buf) {
                        TickOutcome::Advanced {
                            finished,
                            started: next_action,
                            completed,
                        } => {
                            info!(finished, started = next_action, completed, "动作切换");
                        }
                        TickOutcome::Playing => {}
                    }
                } else {
                    // chat 模式：直接推进 player；自然完成即记下身份（P0-6）。
                    //
                    // 2026-09-11 用户裁决：动作系统整体移除——原先的表演源
                    // （`RenderCommand` → `apply_render_command`）已删除，因此
                    // `self.performing` **不再有任何写入点**，本分支的
                    // `natural_finish` 恒为 `None`（动作完成回报通道保留但休眠）。
                    // 播放器本身照常推进：它同时服务于 model-smoke 的脚本动作。
                    match model
                        .driver
                        .player_mut()
                        .update(SIM_DT, &mut model.frame_buf)
                    {
                        SampleStatus::Finished => {
                            natural_finish = self.performing.take();
                        }
                        SampleStatus::Playing { .. } | SampleStatus::Idle => {}
                    }
                }
                // 表演帧 → Bai 标准参数（input 层；缺参静默降级、每 ID 只记一次）。
                model.adapter.apply_frame(&mut model.core, &model.frame_buf);
                // 姿态栈推进（idle 重算 + 物理步进 + 合成最终姿态驱动渲染器）。
                model.core.update(SIM_DT).map_err(classify_frame_error)?;
                self.sim_time += SIM_DT;
                self.sim_steps_total += 1;
            }

            // 自然完成事实回报（P0-6 身份；epoch 镜像不符则丢弃）。
            if let Some((ep, action)) = natural_finish
                && ep == self.render_epoch
                && let Some(bridge) = self.chat.as_ref()
            {
                debug!(?action, "动作自然完成回报");
                bridge.supervisor.report_action_finished(ep, action);
            }

            // ---- 2. 口型电平 → final_override 最高优先级通道 ----
            // - 冒烟模式沿用低频合成电平做端到端验证；
            // - chat 模式使用 MouthSnapshot 的真实 RMS；
            // - **口型归零协议（D5）**：仅在本轮 VoiceStarted..VoiceEnded 窗口内
            //   使用真实电平，其余时刻强制 0.0——不依赖回调自然衰减。
            let level = match (self.chat.as_ref(), self.voice_active) {
                // dry-run（无音频设备）时快照为 None → 电平取 0：与 VoiceEnded
                // 归零协议同一安全方向（宁可少动嘴）。
                (Some(bridge), true) => bridge.snapshot.as_ref().map(|s| s.level()).unwrap_or(0.0),
                (Some(_), false) => 0.0,
                (None, _) => synthesized_mouth_level(self.sim_time),
            };
            if model.adapter.apply_mouth_level(&mut model.core, level) {
                self.mouth_writes += 1;
                self.mouth_peak = self.mouth_peak.max(level);
            }

            // ---- 3. 渲染到 surface view 并 present ----
            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let format = {
                let state = self.state.as_ref().ok_or_else(|| {
                    ModelSmokeError::ModelCode("draw_and_present_model 无 surface".to_owned())
                })?;
                state.config.format
            };
            // 只提交不等待（C1 裁决第 42 行 + C2 第 47 行）；记录本帧
            // SubmissionIndex 供日志/错误上下文使用，**绝不**调阻塞 wrapper。
            let submission = model
                .core
                .render_to_view_submit(&view, format)
                .map_err(classify_frame_error)?;
            // model 的 &mut 借用在 render_to_view_submit 返回后已释放，可写 state。
            let state = self.state.as_mut().ok_or_else(|| {
                ModelSmokeError::ModelCode("draw_and_present_model 无 surface".to_owned())
            })?;
            state.last_submission_index = Some(submission);
            // W6：l2d 已 submit 了 Live2D pass；这里同帧再起一个 encoder 编码 egui
            // 然后 submit，两次 submit 均无 `PollType::Wait`（与提交语义兼容）。
            if let (Some(ui), Some(window)) = (self.settings_ui.as_mut(), self.window.as_ref()) {
                let screen_px = [state.config.width as f32, state.config.height as f32];
                let mut ui_enc =
                    state
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("egui_overlay_model_frame"),
                        });
                crate::app::settings_ui::render_overlay(
                    ui,
                    &state.device,
                    &state.queue,
                    &mut ui_enc,
                    &view,
                    screen_px,
                    window,
                );
                state.queue.submit(Some(ui_enc.finish()));
            }
        }
        frame.present();

        // ---- 4. 帧耗时统计（仅 CPU 提交成本，不含 GPU 等待）----
        let elapsed = started.elapsed();

        // 泵送设备（非阻塞）—— Poll 失败即设备/队列失效，按 C2 第 58 行归为
        // 模型路径的 GPU 环境错误（退出码 3），不再仅 warn（C2 裁决）。
        // 分类走 [`classify_poll_failure`] 纯函数。
        if let Some(state) = &self.state
            && let Err(e) = state.device.poll(wgpu::PollType::Poll)
        {
            let err = classify_poll_failure(
                "model",
                format!("{e:?}"),
                state.last_submission_index.clone(),
                &self.gpu_adapter,
            );
            error!("{err}");
            // ModelSmokeError::GpuEnvironment → run_shell 映射为 BackendError::Environment → 退出码 3
            self.model_failure.get_or_insert(err.clone());
            // 立即请求事件循环退出，避免下一帧再试图提交/触发同一类错误。
            event_loop.exit();
            return Err(err);
        }
        self.frame_time_total += elapsed;
        self.frame_time_max = Some(match self.frame_time_max {
            Some(prev) if prev > elapsed => prev,
            _ => elapsed,
        });
        trace!(
            frame = self.frames_presented + 1,
            ms = elapsed.as_millis() as u64,
            ticks,
            "model frame rendered"
        );
        Ok(())
    }

    /// 渲染一帧：取帧 → 分类处理 → 计数。
    ///
    /// 闭环（实时非阻塞 + 异步故障收口 + 共享状态机，C1+C2+C4 裁决）：
    /// 1. 每帧开始先**消费 `on_uncaptured_error` 写闩**，把最近一次未取走的
    ///    GPU fault 映射为 `model_failure`（Validation → 退出 1）或
    ///    `environment_failure`（OOM/Internal → 退出 3）——故障收口后立即
    ///    跳出取帧，**不再继续提交**（避免每帧刷错，C2 第 62 行）。映射走
    ///    纯函数 [`route_gpu_fault`]。
    /// 2. 取帧后用 [`decide_after_acquire`]（surface.rs 纯状态机，与测试
    ///    共享）把 `wgpu::CurrentSurfaceTexture` 7 个变体映射到 5 个
    ///    `SurfaceAction`；按 action 执行副作用：Present / PresentThenReconfigure
    ///    / SkipAndReconfigure / SkipAndRecreate / Skip(reason)。Validation
    ///    走 streak 计数：首次记录并跳过；连续 ≥2 即代码错误，落地
    ///    `model_failure` + `event_loop.exit()`（C2 第 59 行）。
    /// 3. 取帧前的 `environment_failure` 由 `fail_environment` 路径直接
    ///    `event_loop.exit()`，本函数不重复处理。
    pub(crate) fn render_frame(&mut self, event_loop: &ActiveEventLoop) {
        // 1. 异步故障闩：先消费再画本帧（C2 裁决第 49-62 行）。映射走
        //    纯函数 [`route_gpu_fault`]（tests.rs 断言三档）。
        if let Some(fault) = self.take_gpu_fault() {
            let detail = format!(
                "kind={}, desc={}, source={}; last_submission={:?}, adapter={}",
                fault.kind.as_str(),
                fault.description,
                fault.source_chain,
                self.state
                    .as_ref()
                    .and_then(|s| s.last_submission_index.as_ref()),
                self.gpu_adapter,
            );
            let route = route_gpu_fault(fault.kind, detail);
            // 落地并立即退出本帧（不再继续提交），然后请求事件循环退出。
            match route {
                FaultRouting::ModelCode(msg) => {
                    error!("{msg}");
                    self.model_failure
                        .get_or_insert(ModelSmokeError::ModelCode(msg));
                    // 本帧不再继续提交；RedrawRequested 处理钩会因
                    // model_failure.is_some() 调用 event_loop.exit()。
                    return;
                }
                FaultRouting::Environment(msg) => {
                    error!("{msg}");
                    // 直接走 fail_environment：置位 + event_loop.exit()，
                    // 避免每帧重复记录（C2 第 62 行"消费即停止"）。
                    self.fail_environment(event_loop, msg);
                    return;
                }
            }
        }

        let Some(state) = &self.state else {
            return;
        };
        if state.config.width == 0 || state.config.height == 0 {
            trace!("skip frame: 零尺寸（最小化/折叠）");
            self.frames_skipped += 1;
            return;
        }
        // 取帧 → 走共享状态机（C4 裁决 P0-3：让生产路径真调用
        // `decide_after_acquire` 而不是只测试里用），按 action 执行
        // 副作用。`Success` / `Suboptimal` 携带的 texture 在 `match` 里
        // 提交并 present，再回到 `action` 分支执行后续步骤。
        let acquired = state.surface.get_current_texture();
        let acquire = AcquireOutcome::from_current(&acquired);
        let action = decide_after_acquire(acquire);

        match acquired {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => {
                if let Err(e) = if self.model.is_some() {
                    self.draw_and_present_model(event_loop, texture)
                } else {
                    self.draw_and_present_clear(event_loop, texture);
                    Ok(())
                } {
                    error!(category = e.category(), "{e}");
                    self.model_failure.get_or_insert(e);
                    return;
                }
            }
            wgpu::CurrentSurfaceTexture::Outdated
            | wgpu::CurrentSurfaceTexture::Lost
            | wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => {
                // 无 texture：直接进入 action 副作用阶段。
            }
        }
        match action {
            SurfaceAction::Present => {
                // 任意非 Validation 取帧都清零 streak（C2 第 59 行）：
                // 只在真正"连取两次 Validation"才升级为代码错误。
                self.surface_validation_streak = 0;
                self.frames_presented += 1;
                trace!(
                    frame = self.frames_presented,
                    "presented transparent clear frame"
                );
            }
            SurfaceAction::PresentThenReconfigure => {
                self.surface_validation_streak = 0;
                self.frames_presented += 1;
                trace!("frame suboptimal → 重新 configure");
                self.reconfigure_current();
            }
            SurfaceAction::SkipAndReconfigure => {
                self.surface_validation_streak = 0;
                self.frames_skipped += 1;
                trace!("frame outdated → 重新 configure");
                self.reconfigure_current();
            }
            SurfaceAction::SkipAndRecreate => {
                self.surface_validation_streak = 0;
                self.frames_skipped += 1;
                warn!("surface lost → 重建");
                self.recreate_surface();
            }
            SurfaceAction::Skip(reason) => {
                // C2 第 59 行：Validation 连续 2 帧即代码错误（退出 1）。
                // 计数/分支走纯函数 [`advance_validation_streak`]。
                match advance_validation_streak(
                    &mut self.surface_validation_streak,
                    reason == "validation",
                ) {
                    StreakDecision::SkipFirst => {
                        warn!(
                            streak = self.surface_validation_streak,
                            "surface Validation（首次记录，跳过本帧）"
                        );
                        self.frames_skipped += 1;
                    }
                    StreakDecision::FatalCode => {
                        let msg = format!(
                            "surface Validation 连续 {} 帧（C2 第 59 行代码错误，退出码 1）",
                            self.surface_validation_streak
                        );
                        error!("{msg}");
                        self.model_failure
                            .get_or_insert(ModelSmokeError::ModelCode(msg));
                        event_loop.exit();
                    }
                    StreakDecision::NotValidation => {
                        self.frames_skipped += 1;
                        trace!(reason, "skip frame");
                    }
                }
            }
        }
    }

    /// 是否已满足冒烟退出条件（帧数目标）。
    pub(crate) fn frame_target_reached(&self) -> Option<u64> {
        let target = self.options.frame_target?;
        (self.frames_presented >= target).then_some(target)
    }

    /// 从 `SurfaceState::gpu_fault_latch` 取出最近一次未消费的 GPU 故障。
    ///
    /// 取出语义 = **取走并清空**（`Mutex::take` 风格）：回调可能异步多次
    /// 触发，但帧循环只看到最新一次；连续故障由 wgpu 自身的错误聚合负责，
    /// 我们只关心"现在该不该 fatal"。
    fn take_gpu_fault(&mut self) -> Option<GpuFault> {
        let state = self.state.as_mut()?;
        let mut guard = state.gpu_fault_latch.lock().ok()?;
        guard.take()
    }
}
