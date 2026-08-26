//! 真实 Linux 原生窗口/GPU：winit 0.30 `ApplicationHandler`（UserEvent）+ wgpu 29 surface。
//!
//! 生命周期（[`ShellApp`]）：
//! - [`ApplicationHandler::resumed`]：创建**透明、无边框、可调整大小**窗口 →
//!   建 wgpu surface/adapter/device/queue → 按 surface 实际能力协商
//!   format / alpha mode / present mode 并完成首次 configure；
//!   协商结果如实记入 [`RuntimeCapabilities`]（不凭会话线索推断）；
//! - **桌宠窗口能力**（本批）：RawWindowHandle 实测分类后端
//!   （Xlib/Xcb→X11，Wayland→Wayland）；底部右侧初始定位 +
//!   回读验证；`set_window_level` 置顶（仅 X11 生效）；
//!   左键按下交互态 `drag_window` 拖动；`set_cursor_hittest` 动态切换点击穿透；
//!   托盘菜单事件经 [`PetUserEvent`] 在事件循环线程落地——托盘线程绝不触窗；
//! - **Live2D 实时渲染**（`RunOptions::model_smoke` 为 `Some` 时）：用**同一**
//!   device/queue 构造 [`GpuContext::from_parts_with_label`]（绝不创建第二个
//!   GPU）→ [`ModelPackage::load`] / [`LoadedModel::resolve`]（兼容报告逐条记录，
//!   否决即类型化失败）→ [`ModelRendererCore::new`/`load_model`]；每帧按真实
//!   dt（截断到合理范围，60 Hz 固定步累加器）推进模拟，resize 同步
//!   `set_viewport`，渲染到 **surface view** 后 present。注意当前
//!   `render_to_view` 内部 submit + 阻塞 poll——功能优先、耗时如实统计；
//! - 纯透明壳模式（无模型配置）：连续重绘透明清屏（原 `--window-smoke` 行为）；
//! - 每帧 `device.poll(PollType::Poll)` 泵送设备队列（纯透明壳路径）；
//! - resize / ScaleFactorChanged / 取帧结果（Success/Suboptimal/Timeout/Occluded/
//!   Outdated/Lost/Validation）逐类处理，Lost 时重建 surface；
//! - 冒烟停止条件：帧数目标 / 超时兜底 / 窗口关闭 / 托盘退出。
//!
//! 本模块是环境边界：允许读取进程环境变量做观测日志；
//! 纯判定逻辑一律下沉到 [`crate::platform`] / [`crate::model_smoke`] /
//! [`crate::user_event`]（保持零依赖可单测）。

use std::sync::Arc;
use std::time::Instant;

use l2d::asset::ModelPackage;
use l2d::model::LoadedModel;
use l2d::renderer::{GpuContext, ModelRendererCore};
use raw_window_handle::{HasDisplayHandle as _, HasWindowHandle as _, RawWindowHandle};
use tracing::{error, info, trace, warn};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId, WindowLevel};

use crate::adapter::BaiParamAdapter;
use crate::backend::{BackendError, RunOptions, RunReport, StopReason, WINIT_WGPU_NOTES};
use crate::model_smoke::{
    ActionSequence, FrameClock, ModelSmokeError, ModelSmokeStats, SIM_DT, SmokeDriver, TickOutcome,
    classify_core_init_error, classify_frame_error, classify_load_error, synthesized_mouth_level,
};
use crate::platform::{
    CapabilityState, LinuxSessionHint, ObservedAlphaMode, RuntimeCapabilities, WindowBackendKind,
    bottom_right_target, choose_alpha_mode,
};
use crate::tray::TrayRuntime;
use crate::user_event::{
    EventTarget, PetLaunchPlan, PetUserEvent, event_target, pet_mode_tray_degradation_note,
    plan_launch,
};

/// 提取 panic payload 的可读信息（纯函数，供单测）。
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&'static str>()
                .copied()
                .map(str::to_string)
        })
        .unwrap_or_else(|| "<non-string panic payload>".to_string())
}

/// winit+wgpu 窗口壳入口（由 backend 层调用）。
///
/// 环境不满足（无显示服务/缺 X11/Wayland 系统库/GPU 缺失）返回
/// [`BackendError::Environment`]，由 CLI 映射为独立退出码（3），与代码错误区分。
///
/// 本批起事件循环为 **UserEvent** 型：托盘线程只经
/// [`EventLoopProxy`] 发 [`PetUserEvent`]；托盘初始化失败/超时可见降级，
/// 绝不阻塞窗口启动。pet-mode 的自动点击穿透以「托盘确认就绪」为前置条件。
pub fn run_shell(options: RunOptions) -> Result<RunReport, BackendError> {
    options
        .validate()
        .map_err(|msg| BackendError::Failed(format!("运行参数非法: {msg}")))?;

    // winit 平台初始化会 dlopen 系统库（libxkbcommon-x11 等），缺失时以 panic 形式
    // 暴露——这里捕获并归类为环境错误，避免把“环境缺库”伪装成代码崩溃（exit 101）。
    // 仅包裹创建阶段；事件循环回调内的 panic 属真实缺陷，不在此吞掉。
    let event_loop = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        EventLoop::<PetUserEvent>::with_user_event().build()
    })) {
        Ok(Ok(event_loop)) => event_loop,
        Ok(Err(e)) => {
            return Err(BackendError::Environment(format!(
                "无法创建 winit 事件循环（无显示服务？DISPLAY/WAYLAND_DISPLAY 未设置或不可达）: {e}"
            )));
        }
        Err(payload) => {
            return Err(BackendError::Environment(format!(
                "winit 平台初始化失败（缺系统库？如 libxkbcommon-x11 / 显示服务不可达）: {}",
                panic_message(payload.as_ref())
            )));
        }
    };
    event_loop.set_control_flow(ControlFlow::Poll);

    // 代理先于 run_app 创建：托盘线程持有它发用户事件（Clone + Send）。
    let proxy = event_loop.create_proxy();

    // 托盘：看护线程 + 确认窗口，失败/超时可见降级且不阻塞启动；
    // pet-mode 自动穿透只在托盘**确认就绪**时允许（防不可恢复状态）。
    let tray = crate::tray::spawn_pet_tray(proxy.clone(), crate::tray::TRAY_CONFIRM_WAIT);
    let tray_ready = tray.is_ready();
    if let Some(note) = &tray.degradation_note {
        // 可见降级：日志（tracing）+ stdout 双通道，CI 与桌面会话都能看到。
        warn!("tray 降级: {note}");
        println!("[tray] {note}");
    }
    let plan = plan_launch(options.pet_mode, tray_ready);
    if let Some(note) = pet_mode_tray_degradation_note(options.pet_mode, tray_ready) {
        warn!("{note}");
        println!("[pet-mode] {note}");
    }
    info!(?plan, tray_ready, "pet 启动决策（pet-mode → 初始窗口意图）");
    // 托盘菜单勾选态与启动决策对齐（后续实际落地后由事件循环线程继续同步）。
    if let Some(state) = tray.state() {
        state.set_click_through(plan.click_through);
        state.set_always_on_top(plan.always_on_top);
        state.set_visible(true);
    }

    let mut app = ShellApp::new(options, tray, plan);
    let run_result = event_loop.run_app(&mut app);

    if let Some(msg) = app.environment_failure.take() {
        return Err(BackendError::Environment(msg));
    }
    // model smoke 的类型化失败 → 退出码契约（资产/模型代码 = Failed(1)，
    // GPU 环境 = Environment(3)）。
    if let Some(e) = app.model_failure.take() {
        return match e {
            ModelSmokeError::GpuEnvironment(msg) => Err(BackendError::Environment(format!(
                "Live2D 渲染 GPU 环境不满足: {msg}"
            ))),
            other => Err(BackendError::Failed(other.to_string())),
        };
    }
    run_result.map_err(|e| BackendError::Failed(format!("事件循环异常退出: {e}")))?;

    Ok(app.into_report())
}

/// 单帧取帧结果的归一化分类（便于统一计数与恢复动作）。
enum FrameOutcome {
    /// 已成功 present。
    Presented,
    /// 已 present 但需要重新 configure（suboptimal）。
    PresentedThenReconfigure,
    /// 跳过本帧并重新 configure（outdated）。
    SkipAndReconfigure,
    /// 跳过本帧并重建 surface（lost）。
    SkipAndRecreate,
    /// 单纯跳过本帧（timeout / occluded / validation）。
    Skipped(&'static str),
}

/// Live2D 实时渲染状态（`--model-smoke`；纯透明壳运行时为 `None`）。
struct ModelState {
    /// 渲染核心：姿态栈 + 上游渲染器，render_to_view 到任意视图；
    /// 内部持有与窗口 surface **同一** device/queue 的共享 [`GpuContext`]
    /// （wgpu Device/Queue 是可克隆句柄，克隆不产生新 GPU）。
    core: ModelRendererCore,
    /// core ParameterFrame → Bai 标准参数映射适配器。
    adapter: BaiParamAdapter,
    /// 六动作固定顺序播放驱动（含 core PerformancePlayer）。
    driver: SmokeDriver,
    /// 表演帧复用缓冲（零分配）。
    frame_buf: live2d_ai_core::ParameterFrame,
    /// 兼容报告摘要行（notes/issues/结论，供最终报告输出）。
    compat_lines: Vec<String>,
    /// 兼容判定：v0 可播放。
    compat_supported_v0: bool,
}

/// wgpu 对象集合 + 已生效的 surface 配置。
struct SurfaceState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
}

/// winit ApplicationHandler 实现（窗口壳本体）。
struct ShellApp {
    options: RunOptions,
    instance: wgpu::Instance,
    session_hint: LinuxSessionHint,
    started_at: Instant,
    window: Option<Arc<Window>>,
    state: Option<SurfaceState>,
    runtime: RuntimeCapabilities,
    gpu_adapter: String,
    frames_presented: u64,
    frames_skipped: u64,
    stop_reason: Option<StopReason>,
    /// 环境级失败（区别于代码缺陷）：置位后 run_shell 以环境错误收场。
    environment_failure: Option<String>,
    /// Live2D 实时渲染状态（model smoke）。
    model: Option<ModelState>,
    /// model smoke 的类型化失败：资产/模型代码 → 退出 1，GPU 环境 → 退出 3。
    model_failure: Option<ModelSmokeError>,
    /// 固定步长累加器（真实 dt → 60 Hz 模拟步）。
    clock: FrameClock,
    /// 上一帧时间戳（dt 测量起点）。
    last_frame_at: Option<Instant>,
    /// 已推进的模拟时间（秒；口型合成相位用）。
    sim_time: f32,
    // ---- model smoke 运行统计 ----
    sim_steps_total: u64,
    frame_time_total: std::time::Duration,
    frame_time_max: Option<std::time::Duration>,
    mouth_writes: u64,
    mouth_peak: f32,
    // ---- 桌宠窗口能力（本批新增）----
    /// 托盘运行时句柄（无论初始化成败都持有，便于刷新菜单与退出清理）。
    tray: TrayRuntime,
    /// pet-mode 启动决策（初始置顶/穿透意图）。
    launch_plan: PetLaunchPlan,
    /// 当前点击穿透是否生效（true = 鼠标穿透中，收不到输入事件）。
    click_through_active: bool,
    /// 当前置顶是否生效。
    always_on_top_active: bool,
    /// 当前窗口是否可见（托盘显隐切换的簿记镜像）。
    window_visible: bool,
    /// 初始定位目标（物理像素），待回读验证后记录 global_position。
    pending_position: Option<(i32, i32)>,
    /// drag_window 是否已成功过一次（成功即记录能力，避免重复日志）。
    drag_recorded: bool,
    /// 窗口缩放因子（定位容差/边距换算用）。
    scale_factor: f64,
}

impl ShellApp {
    fn new(options: RunOptions, tray: TrayRuntime, plan: PetLaunchPlan) -> Self {
        // 与 crates/l2d headless 相同的 InstanceDescriptor 形状（wgpu 29）：
        // 全后端候选；display 句柄交给 raw-window-handle 流程（display: None）。
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
            display: None,
        });
        // 会话线索仅用于启动日志对照展示；不参与任何能力判定。
        let xdg = std::env::var("XDG_SESSION_TYPE").ok();
        let wl = std::env::var("WAYLAND_DISPLAY").ok();
        let x11 = std::env::var("DISPLAY").ok();
        let session_hint =
            LinuxSessionHint::detect_from(xdg.as_deref(), wl.as_deref(), x11.as_deref());
        Self {
            options,
            instance,
            session_hint,
            started_at: Instant::now(),
            window: None,
            state: None,
            runtime: RuntimeCapabilities::default(),
            gpu_adapter: String::new(),
            frames_presented: 0,
            frames_skipped: 0,
            stop_reason: None,
            environment_failure: None,
            model: None,
            model_failure: None,
            clock: FrameClock::new(),
            last_frame_at: None,
            sim_time: 0.0,
            sim_steps_total: 0,
            frame_time_total: std::time::Duration::ZERO,
            frame_time_max: None,
            mouth_writes: 0,
            mouth_peak: 0.0,
            tray,
            launch_plan: plan,
            click_through_active: false,
            always_on_top_active: false,
            window_visible: true,
            pending_position: None,
            drag_recorded: false,
            scale_factor: 1.0,
        }
    }

    fn into_report(self) -> RunReport {
        let wall_time = self.started_at.elapsed();
        let model = self.build_model_stats();
        RunReport {
            stop_reason: self.stop_reason.unwrap_or(StopReason::WindowClosed),
            runtime: self.runtime,
            gpu_adapter: self.gpu_adapter,
            frames_presented: self.frames_presented,
            frames_skipped: self.frames_skipped,
            wall_time,
            model,
        }
    }

    /// 汇总 model smoke 统计（未启用模型渲染时返回 None）。
    fn build_model_stats(&self) -> Option<ModelSmokeStats> {
        let model = self.model.as_ref()?;
        let frames = self.frames_presented;
        let avg = if frames == 0 {
            None
        } else {
            Some(self.frame_time_total / u32::try_from(frames).unwrap_or(u32::MAX))
        };
        let model3_path = self
            .options
            .model_smoke
            .as_ref()
            .map(|m| m.model3_path.display().to_string())
            .unwrap_or_default();
        Some(ModelSmokeStats {
            model3_path,
            compat_lines: model.compat_lines.clone(),
            compat_supported_v0: model.compat_supported_v0,
            frames_rendered: frames,
            sim_steps: self.sim_steps_total,
            avg_frame_time: avg,
            max_frame_time: self.frame_time_max,
            actions_completed: model.driver.completed(),
            mouth_writes: self.mouth_writes,
            mouth_peak: self.mouth_peak,
            missing_params: model
                .adapter
                .missing_params()
                .iter()
                .map(ToString::to_string)
                .collect(),
            viewport: model.core.viewport(),
        })
    }

    fn fail_environment(&mut self, event_loop: &ActiveEventLoop, msg: String) {
        error!("{msg}");
        self.environment_failure.get_or_insert(msg);
        event_loop.exit();
    }

    // ------------------------------------------------------------ 桌宠窗口能力

    /// RawWindowHandle 实测分类：Xlib/Xcb → X11，Wayland → Wayland（纯映射，
    /// 见 [`classify_raw_window_handle`]）。结果记入 runtime.backend。
    fn detect_backend(&mut self, window: &Window) {
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
    fn request_initial_position(&mut self, event_loop: &ActiveEventLoop, window: &Window) {
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
    fn verify_pending_position(&mut self) {
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
    fn apply_always_on_top(&mut self, window: &Window, want: bool) {
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

    /// 应用点击穿透意图并如实记录能力（两种后端均可；失败保持原状态）。
    fn apply_click_through(&mut self, window: &Window, want: bool) {
        match window.set_cursor_hittest(want) {
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
    fn apply_visible(&mut self, window: &Window, want: bool) {
        window.set_visible(want);
        self.window_visible = want;
        info!(visible = want, "set_visible 已应用");
    }

    /// 窗口态变更后同步托盘共享簿记并请求 host 刷新菜单。
    fn sync_tray(&self) {
        if let Some(state) = self.tray.state() {
            state.set_click_through(self.click_through_active);
            state.set_always_on_top(self.always_on_top_active);
            state.set_visible(self.window_visible);
        }
        self.tray.refresh();
    }

    /// 处理托盘用户事件（事件循环线程内；所有 Window 调用都在这里落地）。
    fn handle_user_event(&mut self, event_loop: &ActiveEventLoop, event: PetUserEvent) {
        let Some(window) = self.window.clone() else {
            trace!(?event, "窗口尚未创建：忽略托盘事件");
            return;
        };
        let target = match event {
            PetUserEvent::TrayToggleClickThrough => event_target(event, self.click_through_active),
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
            event_loop.exit();
            return;
        }
        self.sync_tray();
    }

    /// 左键按下且处于交互态时发起拖动（穿透中收不到输入，天然互斥）。
    fn begin_interactive_drag(&mut self, window: &Window) {
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

    /// resumed 钩子主体：窗口 + GPU 初始化。失败路径全部归入环境错误
    /// （无显示服务/无可用 adapter/device 属环境问题，不是代码缺陷）。
    fn bootstrap(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = Window::default_attributes()
            .with_title("Live2D-Ai 桌宠 — 窗口壳冒烟")
            .with_inner_size(LogicalSize::new(
                self.options.initial_logical_size.0,
                self.options.initial_logical_size.1,
            ))
            .with_min_inner_size(LogicalSize::new(120.0, 160.0))
            .with_resizable(true)
            .with_decorations(false)
            .with_transparent(true)
            .with_visible(true);
        let window = match event_loop.create_window(attrs) {
            Ok(window) => Arc::new(window),
            Err(e) => {
                self.fail_environment(event_loop, format!("创建原生窗口失败（环境问题）: {e}"));
                return;
            }
        };
        self.runtime.window_created = true;
        // 请求 ≠ 生效：真实 alpha 支持以下面 surface 实测协商为准。
        self.runtime.transparent_window_requested = true;

        let surface = match self.instance.create_surface(window.clone()) {
            Ok(surface) => surface,
            Err(e) => {
                self.fail_environment(
                    event_loop,
                    format!("从窗口创建 wgpu surface 失败（环境问题）: {e}"),
                );
                return;
            }
        };

        // adapter：绑定该 surface；选不到即环境无兼容 GPU。
        let adapter =
            match pollster::block_on(self.instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })) {
                Ok(adapter) => adapter,
                Err(e) => {
                    self.fail_environment(
                        event_loop,
                        format!("无可兼容 GPU adapter（环境问题）: {e}"),
                    );
                    return;
                }
            };
        let adapter_info = adapter.get_info();
        self.gpu_adapter = format!(
            "{} ({:?}, {})",
            adapter_info.name, adapter_info.backend, adapter_info.driver_info
        );

        let (device, queue) =
            match pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("live2d-ai-desktop-shell"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })) {
                Ok(pair) => pair,
                Err(e) => {
                    self.fail_environment(
                        event_loop,
                        format!("GPU device 请求失败（环境问题）: {e}"),
                    );
                    return;
                }
            };

        // 能力协商：format / alpha mode 全部来自 surface 对该 adapter 的实测 caps。
        let caps = surface.get_capabilities(&adapter);
        let Some(format) = choose_surface_format(&caps.formats) else {
            self.fail_environment(
                event_loop,
                "surface 未提供任何可用纹理格式（adapter 与 surface 不兼容）".to_string(),
            );
            return;
        };
        let observed_modes: Vec<ObservedAlphaMode> = caps
            .alpha_modes
            .iter()
            .copied()
            .map(observed_alpha_mode)
            .collect();
        let chosen = choose_alpha_mode(&observed_modes);
        let alpha_mode = chosen.map_or(wgpu::CompositeAlphaMode::Auto, wgpu_alpha_mode);

        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        // 记录真实能力：只依据刚才的实测结果，绝不从会话线索推导。
        self.runtime.surface_initialized = true;
        self.runtime.alpha_mode = chosen;
        self.runtime.transparent_alpha = match chosen {
            Some(mode) if mode.respects_alpha() => CapabilityState::Available,
            // Inherit：大概率透明但未经确认 → Unknown（诚实口径）。
            Some(_) => CapabilityState::Unknown,
            // 无候选（理论上不可达）：显式 Unavailable。
            None => CapabilityState::Unavailable,
        };

        info!(
            session_hint = %self.session_hint,
            gpu = %self.gpu_adapter,
            format = ?config.format,
            alpha_mode = ?chosen,
            "surface 初始化完成：真实能力已记录",
        );
        for (name, value) in self.runtime.as_table() {
            info!(capability = name, value = %value, "runtime capability");
        }

        // ---- 桌宠窗口能力（全部基于上面实测的后端分类，绝不凭环境线索）----
        self.scale_factor = window.scale_factor();
        self.detect_backend(&window);
        self.request_initial_position(event_loop, &window);
        // 点击穿透 API 探测：hittest(true) 幂等无害，两种后端都应成功；
        // 成功即记录 Available（能力证据），是否真的进入穿透看 launch_plan。
        // 后端谓词先行否决（当前恒真；未来接入不支持的后端时自动降级）。
        let click_through_supported = self
            .runtime
            .backend
            .is_none_or(|kind| kind.supports_click_through());
        if click_through_supported {
            self.apply_click_through(&window, false);
            if self.launch_plan.click_through && !self.click_through_active {
                // 探测后按启动决策进入穿透模式。
                self.apply_click_through(&window, true);
            }
        } else {
            self.runtime.click_through = CapabilityState::Unavailable;
            warn!("当前后端不支持点击穿透（协议限制），按 unavailable 记录");
        }
        self.apply_always_on_top(&window, self.launch_plan.always_on_top);
        self.sync_tray();

        let initial_viewport = (config.width.max(1), config.height.max(1));
        self.state = Some(SurfaceState {
            surface,
            device,
            queue,
            config,
        });
        self.window = Some(window);

        // Live2D 实时渲染（model smoke）：在同一 device 上加载皮套与渲染核心。
        // 失败为类型化错误（资产/模型代码 → 退出 1；GPU 环境 → 退出 3），
        // 记录后立即退出事件循环，由 run_shell 映射成对应 BackendError。
        if self.options.model_smoke.is_some() {
            let (device, queue) = {
                let state = self.state.as_ref().expect("state just set");
                (state.device.clone(), state.queue.clone())
            };
            match self.build_model_state(device, queue, initial_viewport) {
                Ok(model) => {
                    info!(
                        adapter = %self.gpu_adapter,
                        viewport = ?model.core.viewport(),
                        "Live2D 渲染核心就绪（与 surface 共享同一 device）"
                    );
                    self.model = Some(model);
                }
                Err(e) => {
                    error!(category = e.category(), "{e}");
                    self.model_failure.get_or_insert(e);
                    event_loop.exit();
                }
            }
        }
    }

    /// 构建 Live2D 实时渲染状态（bootstrap 专用；失败即类型化冒烟错误）。
    ///
    /// 步骤（全部复用**同一个** wgpu device/queue，绝不创建第二个 GPU）：
    /// 1. [`GpuContext::from_parts_with_label`] 接入已有 device/queue；
    /// 2. [`ModelPackage::load`] 读皮套包（IO/清单问题 → Asset）；
    /// 3. [`LoadedModel::resolve`] 解析 moc3 + 头探测×运行时版本交叉核对
    ///    （解析问题 → ModelCode），兼容报告逐条记录，否决 v0 即 Asset；
    /// 4. [`ModelRendererCore::new`] 建渲染管线（失败 → GpuEnvironment）、
    ///    `load_model` 上传模型（physics JSON → Asset；其余 → ModelCode）。
    fn build_model_state(
        &mut self,
        device: wgpu::Device,
        queue: wgpu::Queue,
        initial_viewport: (u32, u32),
    ) -> Result<ModelState, ModelSmokeError> {
        let Some(config) = self.options.model_smoke.clone() else {
            return Err(ModelSmokeError::ModelCode(
                "build_model_state 在未启用模型渲染时被调用".to_owned(),
            ));
        };

        // 1. 同一 device/queue 接入共享 GPU 上下文。
        let gpu = Arc::new(GpuContext::from_parts_with_label(
            device,
            queue,
            format!("shared-with-surface ({})", self.gpu_adapter),
        ));

        // 2-3. 皮套包加载 + 统一解析 + 兼容报告。
        let package = ModelPackage::load(&config.model3_path)?;
        let loaded = LoadedModel::resolve(package)?;

        let report = loaded.report();
        let mut compat_lines: Vec<String> = report.notes().to_vec();
        for issue in report.issues() {
            compat_lines.push(issue.to_string());
        }
        compat_lines.push(format!("v0 可播放判定: {}", report.is_supported_v0()));
        for line in &compat_lines {
            info!(target: "compat", "{line}");
        }
        let compat_supported_v0 = report.is_supported_v0();
        if !compat_supported_v0 {
            let issues = report
                .issues()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ");
            return Err(ModelSmokeError::Asset(format!(
                "兼容报告否决 v0 播放（{}）：{issues}",
                config.model3_path.display()
            )));
        }
        info!(
            stats = ?loaded.handle().stats(),
            "模型解析完成（moc 版本 {:?}）",
            loaded.handle().moc_version()
        );

        // 4. 渲染核心 + 模型上传。
        let mut core =
            ModelRendererCore::new(Arc::clone(&gpu)).map_err(classify_core_init_error)?;
        core.load_model(&loaded).map_err(classify_load_error)?;
        core.set_viewport(initial_viewport.0.max(1), initial_viewport.1.max(1))
            .map_err(classify_frame_error)?;

        // 六动作固定顺序播放从第一个动作开始。
        let driver = SmokeDriver::new();
        info!(
            action = driver.active().map(|a| a.action.name()).unwrap_or(""),
            sequence = ActionSequence::names().join(","),
            "动作序列开始（每动作 Medium；结束即切换下一动作）"
        );

        Ok(ModelState {
            core,
            adapter: BaiParamAdapter::new(),
            driver,
            frame_buf: live2d_ai_core::ParameterFrame::neutral(),
            compat_lines,
            compat_supported_v0,
        })
    }

    /// 把模型视口同步到窗口当前物理尺寸（resize / 重配置共用）。
    fn sync_model_viewport(&mut self) {
        let Some(window) = &self.window else {
            return;
        };
        let PhysicalSize { width, height } = window.inner_size();
        if let Some(model) = &mut self.model
            && let Err(e) = model.core.set_viewport(width.max(1), height.max(1))
        {
            warn!("同步模型视口失败: {e}");
        }
    }

    /// 用窗口当前物理尺寸重新 configure（Outdated/Suboptimal/Lost 恢复共用）。
    ///
    /// 注意：必须在旧 `SurfaceTexture` drop/present **之后**调用——
    /// wgpu 在仍有存活 swapchain 纹理时 configure 会 panic。
    fn reconfigure_current(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };
        let Some(window) = &self.window else {
            return;
        };
        let PhysicalSize { width, height } = window.inner_size();
        state.config.width = width.max(1);
        state.config.height = height.max(1);
        state.surface.configure(&state.device, &state.config);
        // 模型视口与 surface 尺寸保持同步（aspect 修正随 set_viewport 变化）。
        self.sync_model_viewport();
    }

    /// surface Lost：丢弃旧 surface 并按当前窗口重建后重配置。
    fn recreate_surface(&mut self) {
        let Some(window) = &self.window else {
            return;
        };
        match self.instance.create_surface(window.clone()) {
            Ok(surface) => {
                if let Some(state) = &mut self.state {
                    state.surface = surface;
                }
                self.reconfigure_current();
                info!("surface 因 Lost 重建完成");
            }
            Err(e) => error!("surface 重建失败（保留状态等待下帧重试）: {e}"),
        }
    }

    /// 编码一帧全透明清屏并 present（纯透明壳路径；不含取帧错误处理）。
    fn draw_and_present_clear(&mut self, frame: wgpu::SurfaceTexture) {
        let Some(state) = &self.state else {
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
        state.queue.submit(Some(encoder.finish()));
        frame.present();

        // 泵送设备（非阻塞），驱动上传/回调管线。
        if let Err(e) = state.device.poll(wgpu::PollType::Poll) {
            warn!("device poll 异常: {e:?}");
        }
    }

    /// Live2D 实时渲染一帧：真实 dt → 固定步模拟 → 参数写入 → 渲染到
    /// **surface view** → present。失败为类型化冒烟错误。
    ///
    /// 耗时口径：`render_to_view` 内部 submit 后会阻塞 poll 等队列排空
    /// （离屏读回同款语义），因此本函数测得的帧耗时是**包含 GPU 等待的诚实值**，
    /// 不代表纯 CPU 提交成本；功能先通，优化留待后续批次。
    fn draw_and_present_model(
        &mut self,
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

        {
            let model = self.model.as_mut().ok_or_else(|| {
                ModelSmokeError::ModelCode("draw_and_present_model 无模型状态".to_owned())
            })?;
            for _ in 0..ticks {
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
                // 表演帧 → Bai 标准参数（input 层；缺参静默降级、每 ID 只记一次）。
                model.adapter.apply_frame(&mut model.core, &model.frame_buf);
                // 姿态栈推进（idle 重算 + 物理步进 + 合成最终姿态驱动渲染器）。
                model.core.update(SIM_DT).map_err(classify_frame_error)?;
                self.sim_time += SIM_DT;
                self.sim_steps_total += 1;
            }

            // ---- 2. 合成口型电平 → final_override 最高优先级通道 ----
            // 低频合成电平验证 ParamMouthOpenY 通道端到端可达；
            // 不被任何表演曲线覆盖（曲线层不写嘴开合，且 override 层最高优先）。
            let level = synthesized_mouth_level(self.sim_time);
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
            model
                .core
                .render_to_view(&view, format)
                .map_err(classify_frame_error)?;
        }
        frame.present();

        // ---- 4. 帧耗时统计（含 render_to_view 内部 submit+poll 的等待）----
        let elapsed = started.elapsed();
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
    fn render_frame(&mut self) {
        let Some(state) = &self.state else {
            return;
        };
        if state.config.width == 0 || state.config.height == 0 {
            trace!("skip frame: 零尺寸（最小化/折叠）");
            self.frames_skipped += 1;
            return;
        }
        let outcome = match state.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => {
                if let Err(e) = if self.model.is_some() {
                    self.draw_and_present_model(texture)
                } else {
                    self.draw_and_present_clear(texture);
                    Ok(())
                } {
                    error!(category = e.category(), "{e}");
                    self.model_failure.get_or_insert(e);
                    return;
                }
                FrameOutcome::Presented
            }
            wgpu::CurrentSurfaceTexture::Suboptimal(texture) => {
                // 先照常呈现本帧（texture 归还后再 configure，避免 panic）。
                if let Err(e) = if self.model.is_some() {
                    self.draw_and_present_model(texture)
                } else {
                    self.draw_and_present_clear(texture);
                    Ok(())
                } {
                    error!(category = e.category(), "{e}");
                    self.model_failure.get_or_insert(e);
                    return;
                }
                FrameOutcome::PresentedThenReconfigure
            }
            wgpu::CurrentSurfaceTexture::Outdated => FrameOutcome::SkipAndReconfigure,
            wgpu::CurrentSurfaceTexture::Lost => FrameOutcome::SkipAndRecreate,
            wgpu::CurrentSurfaceTexture::Timeout => FrameOutcome::Skipped("timeout"),
            wgpu::CurrentSurfaceTexture::Occluded => FrameOutcome::Skipped("occluded"),
            wgpu::CurrentSurfaceTexture::Validation => FrameOutcome::Skipped("validation"),
        };
        match outcome {
            FrameOutcome::Presented => {
                self.frames_presented += 1;
                trace!(
                    frame = self.frames_presented,
                    "presented transparent clear frame"
                );
            }
            FrameOutcome::PresentedThenReconfigure => {
                self.frames_presented += 1;
                trace!("frame suboptimal → 重新 configure");
                self.reconfigure_current();
            }
            FrameOutcome::SkipAndReconfigure => {
                self.frames_skipped += 1;
                trace!("frame outdated → 重新 configure");
                self.reconfigure_current();
            }
            FrameOutcome::SkipAndRecreate => {
                self.frames_skipped += 1;
                warn!("surface lost → 重建");
                self.recreate_surface();
            }
            FrameOutcome::Skipped(reason) => {
                self.frames_skipped += 1;
                trace!(reason, "skip frame");
            }
        }
    }

    /// 是否已满足冒烟退出条件（帧数目标）。
    fn frame_target_reached(&self) -> Option<u64> {
        let target = self.options.frame_target?;
        (self.frames_presented >= target).then_some(target)
    }
}

impl ApplicationHandler<PetUserEvent> for ShellApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() && self.state.is_none() {
            self.bootstrap(event_loop);
        }
    }

    /// 托盘/外部线程用户事件：唯一跨线程入口，落地全在本线程。
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: PetUserEvent) {
        self.handle_user_event(event_loop, event);
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
        match event {
            WindowEvent::CloseRequested => {
                self.stop_reason.get_or_insert(StopReason::WindowClosed);
                event_loop.exit();
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
                if let Some(state) = &mut self.state {
                    state.config.width = size.width;
                    state.config.height = size.height;
                    state.surface.configure(&state.device, &state.config);
                    trace!(
                        width = size.width,
                        height = size.height,
                        "surface resized+reconfigured"
                    );
                }
                // 模型视口同步 resize（渲染核心的 mask/aspect 修正随之更新）。
                self.sync_model_viewport();
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
                    self.render_frame();
                    if let Some(target) = self.frame_target_reached() {
                        info!(frames = target, "达到冒烟帧数目标，自动退出");
                        self.stop_reason
                            .get_or_insert(StopReason::FrameTargetReached(target));
                        event_loop.exit();
                    }
                }
                // model smoke 类型化失败：立即结束事件循环，由 run_shell
                // 映射为退出码 1（资产/模型代码）或 3（GPU 环境）。
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

/// RawWindowHandle → 真实后端分类（需求 2：Xlib/Xcb 标 X11，Wayland 标 Wayland）。
///
/// 纯映射、可单测（句柄类型可无显示环境构造）；未知句柄返回 None
/// （调用方保持 unknown，不做任何能力假设）。
fn classify_raw_window_handle(handle: RawWindowHandle) -> Option<WindowBackendKind> {
    match handle {
        RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_) => Some(WindowBackendKind::X11),
        RawWindowHandle::Wayland(_) => Some(WindowBackendKind::Wayland),
        _ => None,
    }
}

/// RawDisplayHandle → 同口径分类（窗口句柄缺失时的回退证据）。
fn classify_raw_display_handle(
    handle: raw_window_handle::RawDisplayHandle,
) -> Option<WindowBackendKind> {
    use raw_window_handle::RawDisplayHandle as H;
    match handle {
        H::Xlib(_) | H::Xcb(_) => Some(WindowBackendKind::X11),
        H::Wayland(_) => Some(WindowBackendKind::Wayland),
        _ => None,
    }
}

/// 从 surface 实测格式列表中选择 shell 主格式：
/// 优先带 alpha 的非 sRGB 格式（透明桌宠基础；色彩管理后续批次再细化）。
fn choose_surface_format(formats: &[wgpu::TextureFormat]) -> Option<wgpu::TextureFormat> {
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
const fn observed_alpha_mode(mode: wgpu::CompositeAlphaMode) -> ObservedAlphaMode {
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
const fn wgpu_alpha_mode(mode: ObservedAlphaMode) -> wgpu::CompositeAlphaMode {
    match mode {
        ObservedAlphaMode::PreMultiplied => wgpu::CompositeAlphaMode::PreMultiplied,
        ObservedAlphaMode::PostMultiplied => wgpu::CompositeAlphaMode::PostMultiplied,
        ObservedAlphaMode::Inherit => wgpu::CompositeAlphaMode::Inherit,
        ObservedAlphaMode::Opaque => wgpu::CompositeAlphaMode::Opaque,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        choose_surface_format, classify_raw_display_handle, classify_raw_window_handle,
        observed_alpha_mode, panic_message, wgpu_alpha_mode,
    };
    use crate::platform::ObservedAlphaMode;
    use crate::platform::WindowBackendKind;
    use raw_window_handle::RawDisplayHandle;
    use raw_window_handle::RawWindowHandle;

    #[test]
    fn raw_window_handle_classification_marks_x11_and_wayland() {
        // Xlib / Xcb → X11（需求 2 口径）。
        let xlib = RawWindowHandle::Xlib(raw_window_handle::XlibWindowHandle::new(12345));
        assert_eq!(
            classify_raw_window_handle(xlib),
            Some(WindowBackendKind::X11)
        );
        let xcb = RawWindowHandle::Xcb(raw_window_handle::XcbWindowHandle::new(
            std::num::NonZeroU32::new(6789).unwrap(),
        ));
        assert_eq!(
            classify_raw_window_handle(xcb),
            Some(WindowBackendKind::X11)
        );
        // Wayland → Wayland。
        let wayland = RawWindowHandle::Wayland(raw_window_handle::WaylandWindowHandle::new(
            std::ptr::NonNull::<std::ffi::c_void>::dangling(),
        ));
        assert_eq!(
            classify_raw_window_handle(wayland),
            Some(WindowBackendKind::Wayland)
        );
        // 其他句柄（如 Web）→ None：不识别就不假设。
        let web = RawWindowHandle::Web(raw_window_handle::WebWindowHandle::new(1));
        assert_eq!(classify_raw_window_handle(web), None);
    }

    #[test]
    fn raw_display_handle_classification_matches_window_side() {
        let xlib = RawDisplayHandle::Xlib(raw_window_handle::XlibDisplayHandle::new(None, 0));
        assert_eq!(
            classify_raw_display_handle(xlib),
            Some(WindowBackendKind::X11)
        );
        let xcb = RawDisplayHandle::Xcb(raw_window_handle::XcbDisplayHandle::new(None, 0));
        assert_eq!(
            classify_raw_display_handle(xcb),
            Some(WindowBackendKind::X11)
        );
        let wayland = RawDisplayHandle::Wayland(raw_window_handle::WaylandDisplayHandle::new(
            std::ptr::NonNull::<std::ffi::c_void>::dangling(),
        ));
        assert_eq!(
            classify_raw_display_handle(wayland),
            Some(WindowBackendKind::Wayland)
        );
        let web = RawDisplayHandle::Web(raw_window_handle::WebDisplayHandle::new());
        assert_eq!(classify_raw_display_handle(web), None);
    }

    #[test]
    fn panic_payload_message_extraction() {
        let boxed: Box<dyn std::any::Any + Send> =
            Box::new(String::from("libxkbcommon-x11 missing"));
        assert_eq!(panic_message(boxed.as_ref()), "libxkbcommon-x11 missing");
        let boxed: Box<dyn std::any::Any + Send> = Box::new("static str panic");
        assert_eq!(panic_message(boxed.as_ref()), "static str panic");
        let boxed: Box<dyn std::any::Any + Send> = Box::new(42_u32);
        assert_eq!(panic_message(boxed.as_ref()), "<non-string panic payload>");
    }

    #[test]
    fn surface_format_prefers_non_srgb_with_alpha() {
        assert_eq!(
            choose_surface_format(&[
                wgpu::TextureFormat::Bgra8UnormSrgb,
                wgpu::TextureFormat::Bgra8Unorm,
            ]),
            Some(wgpu::TextureFormat::Bgra8Unorm)
        );
        // 无偏好命中时退回首项而非 None。
        assert_eq!(
            choose_surface_format(&[wgpu::TextureFormat::Rgba16Float]),
            Some(wgpu::TextureFormat::Rgba16Float)
        );
        assert_eq!(choose_surface_format(&[]), None);
    }

    #[test]
    fn alpha_mode_conversion_round_trip_is_lossless_for_explicit_modes() {
        for mode in [
            wgpu::CompositeAlphaMode::PreMultiplied,
            wgpu::CompositeAlphaMode::PostMultiplied,
            wgpu::CompositeAlphaMode::Inherit,
            wgpu::CompositeAlphaMode::Opaque,
        ] {
            assert_eq!(wgpu_alpha_mode(observed_alpha_mode(mode)), mode);
        }
        // Auto 观测侧保守折叠为 Opaque（不会作为显式选择被回写）。
        assert_eq!(
            observed_alpha_mode(wgpu::CompositeAlphaMode::Auto),
            ObservedAlphaMode::Opaque
        );
    }
}
