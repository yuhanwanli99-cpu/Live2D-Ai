//! 核心数据结构：`ShellApp` 字段与伴生类型（`ModelState` / `SurfaceState` /
//! `SHUTDOWN_WATCHDOG`），加 `run_shell` 入口装配。

use std::sync::Arc;
use std::time::Instant;

use raw_window_handle::RawWindowHandle;
use tracing::{info, warn};
use winit::dpi::PhysicalSize;
use winit::event_loop::{ControlFlow, EventLoop};

use crate::app_event::AppEvent;
use crate::backend::{BackendError, RunOptions, RunReport, StopReason};
use crate::model_smoke::{FrameClock, ModelSmokeError, ModelSmokeStats};
use crate::platform::LinuxSessionHint;
use crate::tray::TrayRuntime;
use crate::user_event::{PetLaunchPlan, pet_mode_tray_degradation_note, plan_launch};

/// 提取 panic payload 的可读信息（纯函数，供单测）。
pub(crate) fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
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
        EventLoop::<AppEvent>::with_user_event().build()
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

    let mut app = ShellApp::new(options, tray, plan, None);
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

/// Live2D 实时渲染状态（`--model-smoke`；纯透明壳运行时为 `None`）。
pub(crate) struct ModelState {
    /// 渲染核心：姿态栈 + 上游渲染器，`render_to_view_submit` 到任意视图
    /// （C1 双 API：实时路径只提交不等待，阻塞包装留给离屏/确定性测试）；
    /// 内部持有与窗口 surface **同一** device/queue 的共享 [`GpuContext`]
    /// （wgpu Device/Queue 是可克隆句柄，克隆不产生新 GPU）。
    pub(crate) core: l2d::renderer::ModelRendererCore,
    /// core ParameterFrame → Bai 标准参数映射适配器。
    pub(crate) adapter: crate::adapter::BaiParamAdapter,
    /// 六动作固定顺序播放驱动（含 core PerformancePlayer）。
    pub(crate) driver: crate::model_smoke::SmokeDriver,
    /// 表演帧复用缓冲（零分配）。
    pub(crate) frame_buf: live2d_ai_core::ParameterFrame,
    /// 兼容报告摘要行（notes/issues/结论，供最终报告输出）。
    pub(crate) compat_lines: Vec<String>,
    /// 兼容判定：v0 可播放。
    pub(crate) compat_supported_v0: bool,
}

/// wgpu 对象集合 + 已生效的 surface 配置。
pub(crate) struct SurfaceState {
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,
    /// 异步 GPU 故障写闩：`Device::on_uncaptured_error` 回调写入
    /// `GpuFault`（latest-wins），由事件循环线程每帧开始时消费。
    ///
    /// 必须是 `Arc<Mutex<_>>`：回调可能在 wgpu 内部线程触发，与事件循环
    /// 线程不在同一线程，回调**不得**直接操作 `ShellApp`（C2 裁决第 49 行）。
    pub(crate) gpu_fault_latch:
        std::sync::Arc<std::sync::Mutex<Option<crate::app::frame::GpuFault>>>,
    /// 最近一次 `Queue::submit` 返回的 `SubmissionIndex`（实时非阻塞
    /// 提交序号）。事件循环每帧提交后即刷新；日志/错误上下文读取以关联
    /// 异步 fault 与具体帧（C2 第 63 行）。
    pub(crate) last_submission_index: Option<wgpu::SubmissionIndex>,
}

/// chat 模式的窗口侧桥：命令句柄 + 只读口型快照（D5）。
///
/// 主线程**绝不**持有 supervisor 的可变资源：PlaybackHandle/引擎/reducer
/// 都留在 supervisor 线程；本桥只有单向命令发送与 Arc 快照读取。
pub(crate) struct ChatBridge {
    /// 共享命令句柄：REPL stdin 线程与事件循环（ShellApp）都要发送 say/stop/
    /// quit，因此经 `Arc` 分发——方法全部是 `&self` 的通道发送，不引入任何
    /// 共享可变状态（join 所有权仍归 backend 装配方持有）。
    pub(crate) supervisor: std::sync::Arc<crate::supervisor::SupervisorHandle>,
    /// 只读口型快照。`None` = 无音频设备降级（dry-run）：口型电平恒 0。
    pub(crate) snapshot: Option<crate::audio::MouthSnapshot>,
    /// 配置文件路径（`live2d-ai.toml`）。原生 egui 设置面板保存按钮的写盘
    /// 目标；同时用于启动时把已加载 `AppSettings` 回显到表单 `Draft`。
    /// `None` = 面板无可写目标（理论上 chat 模式恒为 `Some`）。
    pub(crate) config_path: Option<String>,
}

/// shutdown handshake 兜底上限（D13 裁决）：超过即警告强退。
pub(crate) const SHUTDOWN_WATCHDOG: std::time::Duration = std::time::Duration::from_millis(500);

/// winit ApplicationHandler 实现（窗口壳本体）。
pub(crate) struct ShellApp {
    pub(crate) options: RunOptions,
    pub(crate) instance: wgpu::Instance,
    pub(crate) session_hint: LinuxSessionHint,
    pub(crate) started_at: Instant,
    pub(crate) window: Option<Arc<winit::window::Window>>,
    pub(crate) state: Option<SurfaceState>,
    pub(crate) runtime: crate::platform::RuntimeCapabilities,
    pub(crate) gpu_adapter: String,
    pub(crate) frames_presented: u64,
    pub(crate) frames_skipped: u64,
    pub(crate) stop_reason: Option<StopReason>,
    /// 环境级失败（区别于代码缺陷）：置位后 run_shell 以环境错误收场。
    /// `pub(crate)`：chat 装配（backend）收尾时读取，与 run_shell 同退出码契约。
    pub(crate) environment_failure: Option<String>,
    /// Live2D 实时渲染状态（model smoke）。
    pub(crate) model: Option<ModelState>,
    /// model smoke 的类型化失败：资产/模型代码 → 退出 1，GPU 环境 → 退出 3。
    /// `pub(crate)`：chat 装配（backend）收尾时读取。
    pub(crate) model_failure: Option<ModelSmokeError>,
    /// 固定步长累加器（真实 dt → 60 Hz 模拟步）。
    pub(crate) clock: FrameClock,
    /// 上一帧时间戳（dt 测量起点）。
    pub(crate) last_frame_at: Option<Instant>,
    /// 已推进的模拟时间（秒；口型合成相位用）。
    pub(crate) sim_time: f32,
    // ---- model smoke 运行统计 ----
    pub(crate) sim_steps_total: u64,
    pub(crate) frame_time_total: std::time::Duration,
    pub(crate) frame_time_max: Option<std::time::Duration>,
    pub(crate) mouth_writes: u64,
    pub(crate) mouth_peak: f32,
    // ---- 桌宠窗口能力（本批新增）----
    /// 托盘运行时句柄（无论初始化成败都持有，便于刷新菜单与退出清理）。
    pub(crate) tray: TrayRuntime,
    /// pet-mode 启动决策（初始置顶/穿透意图）。
    pub(crate) launch_plan: PetLaunchPlan,
    /// 当前点击穿透是否生效（true = 鼠标穿透中，收不到输入事件）。
    pub(crate) click_through_active: bool,
    /// 当前置顶是否生效。
    pub(crate) always_on_top_active: bool,
    /// 当前窗口是否可见（托盘显隐切换的簿记镜像）。
    pub(crate) window_visible: bool,
    /// 初始定位目标（物理像素），待回读验证后记录 global_position。
    pub(crate) pending_position: Option<(i32, i32)>,
    /// 最新未应用的窗口物理尺寸（C4 裁决 P0-3）。
    ///
    /// 同一事件批次的多次 `Resized` 天然合并到最后值（后写覆盖前写）；
    /// 下一次 redraw/acquire 前由 [`crate::app::surface::apply_pending_resize`]
    /// 消费（非零才 configure + 同步 viewport），消费后清空。
    /// 零尺寸（最小化/折叠）保持 None，不覆盖既有 config。
    pub(crate) pending_surface_size: Option<PhysicalSize<u32>>,
    /// drag_window 是否已成功过一次（成功即记录能力，避免重复日志）。
    pub(crate) drag_recorded: bool,
    /// 窗口缩放因子（定位容差/边距换算用）。
    pub(crate) scale_factor: f64,
    // ---- chat 模式（最终接线；冒烟模式为 None）----
    /// 命令与快照桥。None = 冒烟模式（原语义不变）。
    pub(crate) chat: Option<ChatBridge>,
    /// 业务代次镜像（D9 防御闸门）：只能被 supervisor 下发的 NewEpoch 刷新，
    /// 绝不自增；Conversation 事件携带的 epoch 不匹配即丢弃。
    pub(crate) render_epoch: u64,
    /// 口型活跃窗口：VoiceStarted..VoiceEnded 区间内才使用真实 RMS 电平；
    /// 其余时刻强制 0.0——不依赖回调自然衰减（D5 口型归零协议）。
    pub(crate) voice_active: bool,
    /// 当前在演动作（自然完成回报需携带身份，P0-6）。
    ///
    /// 2026-09-11 用户裁决后**恒为 `None`**：唯一写入点 `apply_render_command`
    /// 已随 `AppEvent::Render` / `RenderCommand` 删除；保留字段只为让
    /// `frame.rs` 的完成回报分支保持形状（休眠，不再触发）。
    pub(crate) performing: Option<(u64, live2d_ai_core::SemanticAction)>,
    /// 关机 handshake 发起时刻（超时兜底判定起点）。
    pub(crate) quitting_since: Option<Instant>,
    /// `wgpu::CurrentSurfaceTexture::Validation` 连续计数（C2 裁决
    /// 第 59 行：surface `Validation` 连续出现 → 首次记录；连续 2 帧即
    /// 代码错误 → 退出 1）。`u8` 足够（C2 阈值 = 2）；任何非 Validation
    /// 取帧结果清零。
    pub(crate) surface_validation_streak: u8,
    // ---- 设置面板（egui 0.35 overlay；W6 接线后由 `bootstrap` 注入真实状态）----
    /// 面板状态机 + GPU 渲染器（`None` = bootstrap 未注入，正常路径不会发生；
    /// handler/frame 读取前先 `as_mut()` 判空，避免对裸 `unwrap` 提权）。
    pub(crate) settings_ui: Option<crate::app::settings_ui::SettingsUiState>,
}

impl ShellApp {
    /// 构造窗口壳。`chat` 为 `Some` 时进入 chat 模式语义（冒烟传 `None`）。
    /// `pub(crate)`：`backend::run_chat_session` 需要在本 crate 内装配 chat。
    pub(crate) fn new(
        options: RunOptions,
        tray: TrayRuntime,
        plan: PetLaunchPlan,
        chat: Option<ChatBridge>,
    ) -> Self {
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
            runtime: crate::platform::RuntimeCapabilities::default(),
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
            pending_surface_size: None,
            drag_recorded: false,
            scale_factor: 1.0,
            chat,
            render_epoch: 0,
            voice_active: false,
            performing: None,
            quitting_since: None,
            surface_validation_streak: 0,
            settings_ui: None,
        }
    }

    /// 汇总运行报告（`pub(crate)`：chat 装配在事件循环结束后调用）。
    pub(crate) fn into_report(self) -> RunReport {
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
    pub(crate) fn build_model_stats(&self) -> Option<ModelSmokeStats> {
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

    /// 标记环境级失败并请求事件循环退出。
    pub(crate) fn fail_environment(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        msg: String,
    ) {
        tracing::error!("{msg}");
        self.environment_failure.get_or_insert(msg);
        event_loop.exit();
    }

    /// 静默 silence 警告：`RawWindowHandle` 在 `capability` 模块使用，
    /// 重新导出以让本类型模块不显示未使用导入。
    #[allow(dead_code)]
    pub(crate) fn _raw_window_handle_marker() -> Option<RawWindowHandle> {
        None
    }
}
