//! benchmark surface 路径（C7 协议标准路径）。
//!
//! **文件体量豁免说明（≤1000 上限）**：本文件承载 surface 路径的全部代码：
//! [`Slot`] 类型别名 + [`run_benchmark_surface`] / [`run_benchmark_with_slot`]
//! 公开入口 + [`BenchmarkApp`] 结构体 + `BenchState` / `BenchPhase` /
//! `PhaseDecision` 状态 + `advance_benchmark_phase` 阶段推进纯函数 +
//! `bootstrap` / `render_one_frame` / `recreate_surface` / `build_report`
//! 四个方法 + `ApplicationHandler` impl。P1-2 二轮修复后约 880 行，
//! 超出 500 上限但远低于 1000 硬豁免阈值，故在此显式声明豁免。
//!
//! P1-2 复审修复（headless 路径同步）：
//! - **P0-1 workload helper**：每帧调
//!   [`super::workload::step_frame_workload`] 推进表演曲线 + 写入
//!   input 层 + 写口型 override + `core.update`，与生产
//!   [`crate::app::frame::draw_and_present_model`] 顺序一致。
//! - **P0-2 状态机复用**：用 `app::surface::AcquireOutcome::from_current` +
//!   `decide_after_acquire` 取得 `SurfaceAction`，按 action 执行真实副作用
//!   （`Present` / `PresentThenReconfigure` 渲染+present+configure /
//!   `SkipAndReconfigure` 真实 configure / `SkipAndRecreate` 真实重建 +
//!   configure / `Skip(reason)` 仅计数）。
//! - **P0-3 阶段隔离**：warm-up 与 formal **完全隔离**——`formal_attempts`
//!   / `formal_skipped` / `formal_actions` / `formal_presented` 在进入
//!   Formal 时从 0 起算；终止条件 `formal_attempts == options.frames`。
//! - **P1-4 measured_frames**：报告新增 `measured_frames: u64` =
//!   正式阶段尝试帧数（无论 skip/present），如实标注。
//!
//! P1-2 二轮修复（任务 1：阶段推进死循环 + 真实阶段计数）：
//! - **B0 阶段计数**：BenchState 新增 `warmup_attempts` / `formal_attempts`，
//!   每次 redraw 尝试（无论 Success/Suboptimal/Outdated/Lost/Timeout/
//!   Occluded/Validation）**仅一次** `+1`；Suboptimal 仍只 +1（present
//!   + reconfigure 同一次 attempt）。
//! - **B1 阶段决策纯函数**：[`advance_benchmark_phase`] 接受当前
//!   `BenchPhase` + warmup/formal attempts/target，返回 [`PhaseDecision`]：
//!   `StayWarmup` / `EnterFormal` / `StayFormal` / `Finish`。
//!   `StayWarmup` / `StayFormal` 区分**调用方在 attempt 递增前** vs **后**
//!   ——`attempts_before_advance` 显式传 0 表示「这一帧还没算」，`1` 表示
//!   「已经算过这次 attempt」。
//! - **B2 终止条件**：用 `state.formal_attempts == formal_target` 收口
//!   （**不**再 `presented + skipped.total()`，因为 Suboptimal 双计会
//!   提前结束 + 死循环）。
//! - **B3 报告**：`measured_frames = state.formal_attempts`（真实
//!   attempt 数，不再是配置值）。
//!
//! 运行时序：
//! - [`run_benchmark_surface`]：调 [`run_benchmark_with_slot`] 启动
//!   winit 事件循环；
//! - [`run_benchmark_with_slot`]：建 [`BenchmarkApp`] → `event_loop.run_app`
//!   → 在 `event_loop.exit` 后从 `app.report` 或 `app.bootstrap_error` 取
//!   最终结果写入 `result_slot`；
//! - [`BenchmarkApp`]：`resumed` 调 `bootstrap` 建窗口/设备/模型；
//!   `window_event::RedrawRequested` 调 `render_one_frame` 跑一帧；
//!   达到帧数上限时调 `build_report` 组装报告 + `event_loop.exit`。

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use l2d::asset::ModelPackage;
use l2d::model::LoadedModel;
use l2d::renderer::{GpuContext, ModelRendererCore};
use live2d_ai_core::ParameterFrame;
use tracing::{info, warn};
use winit::dpi::LogicalSize;
use winit::event_loop::EventLoop;
use winit::window::Window;

use crate::adapter::BaiParamAdapter;
use crate::app::{AcquireOutcome, SurfaceAction, decide_after_acquire};
use crate::model_smoke::{ActionSequence, SmokeDriver};

use super::stats::{ActionCounts, SkipBuckets, SkipKind, TimingStats};
use super::workload::step_frame_workload;
use super::{BenchmarkMode, BenchmarkOptions, BenchmarkReport};

/// benchmark 退出报告 slot 类型别名。
pub type Slot = Rc<RefCell<Option<Result<BenchmarkReport, String>>>>;

/// surface 模式（C7 协议标准）：winit 事件循环 + wgpu surface。
pub fn run_benchmark_surface(opts: BenchmarkOptions) -> Result<BenchmarkReport, String> {
    let slot: Slot = Rc::new(RefCell::new(None));
    run_benchmark_with_slot(opts, slot.clone())?;
    slot.borrow_mut()
        .take()
        .unwrap_or_else(|| Err("event loop 跑完但 slot 为空（未知状态）".to_owned()))
}

/// 跑 benchmark 并把最终报告写入 `result_slot`（用于 event loop 退出后
/// 在 [`run_benchmark_surface`] 顶层取回）。
///
/// 与 `run_benchmark_surface` 的差别：本函数允许调用方注入 slot，从而支持
/// 把本模块的结果接到外部存储（CLI 报告 → stdout；本函数 → Rc slot）；
/// 语义与 `run_benchmark_surface` 完全一致。
pub fn run_benchmark_with_slot(opts: BenchmarkOptions, result_slot: Slot) -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|e| format!("创建 event loop 失败: {e}"))?;
    let mut app = BenchmarkApp::new(opts);
    let bootstrap_err = event_loop
        .run_app(&mut app)
        .err()
        .map(|e| format!("event loop 退出异常: {e}"));
    // 优先：bootstrap 错误；否则从 app 内部报告取；否则 fallback。
    let final_result = if let Some(e) = app.bootstrap_error.clone() {
        Err(e)
    } else if let Some(r) = app.report.clone() {
        Ok(r)
    } else if let Some(be) = bootstrap_err {
        Err(be)
    } else {
        Err("event loop 跑完但无报告（未知状态）".to_owned())
    };
    *result_slot.borrow_mut() = Some(final_result);
    Ok(())
}

// ============================================================== BenchmarkApp

/// Benchmark 内部状态：winit 事件循环 + wgpu surface + 模型。
struct BenchmarkApp {
    opts: BenchmarkOptions,
    /// bootstrap 之后填充。
    window: Option<Arc<Window>>,
    instance: Option<wgpu::Instance>,
    surface: Option<wgpu::Surface<'static>>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    core: Option<ModelRendererCore>,
    driver: Option<SmokeDriver>,
    adapter: Option<BaiParamAdapter>,
    frame_buf: ParameterFrame,
    adapter_label: String,
    backend: String,
    /// 模拟累计时间（决定 synthesized_mouth_level 相位）。
    sim_time: f32,
    /// 报告状态。
    state: BenchState,
    /// bootstrap 阶段错误（如有）。
    bootstrap_error: Option<String>,
    /// event_loop exit 之后保留的最终报告。
    report: Option<BenchmarkReport>,
}

/// 帧循环统计 + 当前帧序号等。
///
/// P1-2-P0-3 修复：warm-up 与 formal **完全隔离**——下划线 `_total` /
/// `presented_total` 仅作历史兼容保留（外部仍用 `state.presented` 读
/// formal present 计数），内部所有 formal 阶段统计（attempts /
/// submitted / present / skip / actions）从 0 起算。
///
/// P1-2 二轮修复（任务 1）：新增 `warmup_attempts` / `formal_attempts`
/// ——**真实** redraw 尝试计数，**不**依赖 presented/skipped 推导。
/// 每次 redraw 尝试（无论 Success/Suboptimal/Outdated/Lost/Timeout/
/// Occluded/Validation）**仅一次** +1；Suboptimal 同样 +1（present +
/// reconfigure 算同一帧 attempt）。
/// 阶段推进靠 [`advance_benchmark_phase`] 纯函数判定，runner 按决策
/// 执行副作用（清零 / 收口）。
struct BenchState {
    warmup_target: u64,
    formal_target: u64,
    mode: BenchmarkMode,
    /// warm-up 阶段真实 redraw 尝试数（每次尝试 +1，无论
    /// Success/Suboptimal/Timeout/Occluded/...）。
    warmup_attempts: u64,
    /// formal 阶段真实 redraw 尝试数（每次尝试 +1）；终止条件
    /// `formal_attempts == formal_target`（**不**依赖 presented +
    /// skipped.total()，避免 Suboptimal 双计导致死循环）。
    formal_attempts: u64,
    /// formal 阶段成功 present 帧数（= `formal_presented`；`build_report`
    /// 读此字段）。
    presented: u64,
    /// formal 阶段 skip 分桶（仅在 formal 阶段累加）。
    skipped: SkipBuckets,
    /// formal 阶段各 surface action 触发次数（仅在 formal 阶段累加）。
    actions: ActionCounts,
    /// `submit_ms` 样本（毫秒；仅正式阶段记录）。
    submit_ms_samples: Vec<f64>,
    /// `present_interval_ms` 样本（毫秒；仅正式阶段记录）。
    present_interval_samples: Vec<f64>,
    /// 上一次 `presented` 的时刻。
    last_present: Option<Instant>,
    /// 阶段：warm-up 阶段 / formal 阶段 / 完成。
    phase: BenchPhase,
    /// formal 阶段开始时刻（用于计算 elapsed）。
    formal_started: Option<Instant>,
    /// 总墙钟起点（bootstrap 完成后）。
    run_started: Option<Instant>,
    /// 完成的最后时刻。
    finished_at: Option<Instant>,
    /// 实际尺寸（物理像素；报告用）。
    physical_size: (u32, u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchPhase {
    Warmup,
    Formal,
    Done,
}

/// 阶段推进纯函数返回值（[`advance_benchmark_phase`] 决策）。
///
/// 设计：每次 redraw 结束后，runner 调一次 `advance_benchmark_phase`，
/// 把「这一帧 attempt 是否已计入」也作为输入——这样纯函数**不**关心
/// attempts 是递增前还是递增后的值，调用方显式传 0 或 1。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseDecision {
    /// 仍处 warmup 阶段；继续 redraw。
    StayWarmup,
    /// **本帧** attempt 已达 warmup_target；调用方应：
    /// 1. 把 `phase` 切到 `Formal`；
    /// 2. 把 `formal_attempts` 设为 0（**整层统计重置**——`presented` /
    ///    `skipped` / `actions` / `submit_ms_samples` /
    ///    `present_interval_samples` / `last_present` 全部清空；
    ///    `formal_started = Some(Instant::now())`）。
    ///
    /// 本决策**只在 attempts 递增后**触发（incremented = 1）。
    EnterFormal,
    /// 仍处 formal 阶段；继续 redraw。
    StayFormal,
    /// formal 阶段 attempt 已达 `formal_target`；调用方应
    /// `phase = Done` + `build_report` + `event_loop.exit()`。
    /// 本决策**只在 attempts 递增后**触发（incremented = 1）。
    Finish,
}

/// 阶段推进纯函数（任务 1 + 任务 2 测试目标）。
///
/// ## 语义
///
/// 每次 redraw 结束后，runner 调用本函数，传入：
/// - `phase`：当前 `BenchPhase`；
/// - `warmup_attempts` / `formal_attempts`：当前 attempt 计数（含本帧）；
/// - `warmup_target` / `formal_target`：CLI 传入的目标值；
/// - `incremented`：本帧 attempt 是否已 +1（0 = 调用方要在 `StayWarmup`
///   / `StayFormal` 后再 +1；1 = 本帧 attempt 已计入，函数自行判定
///   是否收口 / 切阶段）。
///
/// ## 决策矩阵
///
/// | 当前 phase | attempts vs target | incremented | 决策 |
/// | --- | --- | --- | --- |
/// | `Warmup` | `warmup < target` | 0 或 1 | `StayWarmup` |
/// | `Warmup` | `warmup >= target` | 1 | `EnterFormal`（**唯一**触发点）|
/// | `Warmup` | `warmup >= target` | 0 | `StayWarmup`（attempt 未计入，**不**切）|
/// | `Formal` | `formal < target` | 0 或 1 | `StayFormal` |
/// | `Formal` | `formal >= target` | 1 | `Finish` |
/// | `Formal` | `formal >= target` | 0 | `StayFormal` |
/// | `Done` | 任意 | 任意 | `Finish`（已收口，再调一次安全）|
///
/// ## 边界
///
/// - `warmup_target == 0` + `incremented == 1`：第 1 次 attempt 即
///   `EnterFormal`；
/// - `formal_target == 0` + `incremented == 1`：第 1 次 formal attempt
///   即 `Finish`（runner 生成空报告）；
/// - `incremented` 只接受 0 或 1；其它值 panic（调用方保证）。
pub fn advance_benchmark_phase(
    phase: BenchPhase,
    warmup_attempts: u64,
    formal_attempts: u64,
    warmup_target: u64,
    formal_target: u64,
    incremented: u64,
) -> PhaseDecision {
    assert!(incremented <= 1, "incremented 只接受 0 或 1");
    match phase {
        BenchPhase::Done => PhaseDecision::Finish,
        BenchPhase::Warmup => {
            if warmup_attempts >= warmup_target && incremented == 1 {
                PhaseDecision::EnterFormal
            } else {
                PhaseDecision::StayWarmup
            }
        }
        BenchPhase::Formal => {
            if formal_attempts >= formal_target && incremented == 1 {
                PhaseDecision::Finish
            } else {
                PhaseDecision::StayFormal
            }
        }
    }
}

impl BenchmarkApp {
    fn new(opts: BenchmarkOptions) -> Self {
        Self {
            opts,
            window: None,
            instance: None,
            surface: None,
            device: None,
            queue: None,
            surface_config: None,
            core: None,
            driver: None,
            adapter: None,
            frame_buf: ParameterFrame::neutral(),
            adapter_label: String::new(),
            backend: String::new(),
            sim_time: 0.0,
            state: BenchState {
                warmup_target: 0,
                formal_target: 0,
                mode: BenchmarkMode::Submit,
                warmup_attempts: 0,
                formal_attempts: 0,
                presented: 0,
                skipped: SkipBuckets::default(),
                actions: ActionCounts::default(),
                submit_ms_samples: Vec::new(),
                present_interval_samples: Vec::new(),
                last_present: None,
                phase: BenchPhase::Warmup,
                formal_started: None,
                run_started: None,
                finished_at: None,
                physical_size: (0, 0),
            },
            bootstrap_error: None,
            report: None,
        }
    }

    /// bootstrap：建窗口、wgpu surface/device/queue、surface configure、加载模型。
    fn bootstrap(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let opts = &self.opts;
        let attrs = Window::default_attributes()
            .with_title("Live2D-Ai benchmark（C7/C8 llvmpipe A/B）")
            .with_inner_size(LogicalSize::new(
                opts.window_logical.0,
                opts.window_logical.1,
            ))
            .with_min_inner_size(LogicalSize::new(120.0, 160.0))
            .with_resizable(false)
            .with_decorations(false)
            .with_transparent(true)
            .with_visible(false) // benchmark 窗口不显示，避免扰动（仅 X11 提交即可）。
            .with_position(winit::dpi::PhysicalPosition::new(0, 0));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.bootstrap_error = Some(format!("创建原生窗口失败（环境问题）: {e}"));
                return;
            }
        };

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
            display: None,
        });
        let surface = match instance.create_surface(window.clone()) {
            Ok(s) => s,
            Err(e) => {
                self.bootstrap_error = Some(format!("从窗口创建 wgpu surface 失败: {e}"));
                return;
            }
        };
        let adapter =
            match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })) {
                Ok(a) => a,
                Err(e) => {
                    self.bootstrap_error = Some(format!("无可兼容 GPU adapter: {e}"));
                    return;
                }
            };
        let info = adapter.get_info();
        self.adapter_label = format!("{} ({:?}, {})", info.name, info.backend, info.driver_info);
        self.backend = format!("{:?}", info.backend);

        let (device, queue) =
            match pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("live2d-ai-desktop-benchmark"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })) {
                Ok(pair) => pair,
                Err(e) => {
                    self.bootstrap_error = Some(format!("GPU device 请求失败: {e}"));
                    return;
                }
            };

        // 异步 GPU 错误：benchmark 不升级为 fatal（仅 warn 便于观测）。
        device.on_uncaptured_error(Arc::new(|err: wgpu::Error| {
            warn!("benchmark on_uncaptured_error: {err}");
        }));

        // surface 协商：format / alpha mode / present_mode。
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .or_else(|| caps.formats.first().copied())
            .ok_or_else(|| "surface 无可用纹理格式".to_string());
        let format = match format {
            Ok(f) => f,
            Err(e) => {
                self.bootstrap_error = Some(e);
                return;
            }
        };
        let alpha_mode = caps
            .alpha_modes
            .iter()
            .copied()
            .find(|m| *m == wgpu::CompositeAlphaMode::PreMultiplied)
            .or_else(|| caps.alpha_modes.first().copied())
            .unwrap_or(wgpu::CompositeAlphaMode::Auto);
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
        self.state.physical_size = (config.width, config.height);

        // 加载模型。
        let gpu = Arc::new(GpuContext::from_parts_with_label(
            device.clone(),
            queue.clone(),
            format!("benchmark ({})", self.adapter_label),
        ));
        let package = match ModelPackage::load(&opts.model3_path) {
            Ok(p) => p,
            Err(e) => {
                self.bootstrap_error = Some(format!("加载皮套包失败: {e}"));
                return;
            }
        };
        let loaded = match LoadedModel::resolve(package) {
            Ok(l) => l,
            Err(e) => {
                self.bootstrap_error = Some(format!("解析皮套失败: {e}"));
                return;
            }
        };
        let mut core = match ModelRendererCore::new(Arc::clone(&gpu)) {
            Ok(c) => c,
            Err(e) => {
                self.bootstrap_error = Some(format!("创建渲染核心失败: {e}"));
                return;
            }
        };
        if let Err(e) = core.load_model(&loaded) {
            self.bootstrap_error = Some(format!("load_model 失败: {e}"));
            return;
        }
        if let Err(e) = core.set_viewport(config.width.max(1), config.height.max(1)) {
            self.bootstrap_error = Some(format!("set_viewport 失败: {e}"));
            return;
        }
        info!(adapter = %self.adapter_label, viewport = ?core.viewport(), "benchmark 渲染核心就绪");

        // 适配器：与生产路径一致地持有（P1-2-P0-1 修复——不再丢弃）。
        // 每帧 workload helper 会调 apply_frame / apply_mouth_level 真正写入。
        let adapter = BaiParamAdapter::new();

        // 烟驱：与 `model_smoke::SmokeDriver` 同口径（六动作循环 Medium）。
        let driver = SmokeDriver::new();
        info!(
            sequence = ActionSequence::names().join(","),
            "benchmark 动作序列开始"
        );

        self.window = Some(window);
        self.instance = Some(instance);
        self.surface = Some(surface);
        self.device = Some(device);
        self.queue = Some(queue);
        self.surface_config = Some(config);
        self.core = Some(core);
        self.driver = Some(driver);
        self.adapter = Some(adapter);
        self.state.warmup_target = opts.warmup_frames;
        self.state.formal_target = opts.formal_frames;
        self.state.mode = opts.mode;
    }

    /// 按当前 config 重新 configure（与 [`crate::app::surface::ShellApp::reconfigure_current`]
    /// 同口径）。
    ///
    /// benchmark 自有 surface 所有权（独立 wgpu::Surface + config + device），
    /// 这里直接调 `surface.configure(&device, &config)`；不调用生产路径的
    /// `ShellApp::reconfigure_current` 是因为其绑定在 `ShellApp` 上。
    /// 语义等价：按当前 config 重新配置 surface。
    fn reconfigure_current(&mut self) {
        if let (Some(s), Some(d), Some(c)) = (
            self.surface.as_ref(),
            self.device.as_ref(),
            self.surface_config.as_ref(),
        ) {
            s.configure(d, c);
        }
    }

    /// surface Lost 重建（与 [`crate::app::surface::ShellApp::recreate_surface`]
    /// 同口径）。
    ///
    /// benchmark 持有自己的 `wgpu::Instance`；重建路径：丢弃旧 surface
    /// → 用 instance 重建 → configure → 模型视口与 surface 同步。
    fn recreate_surface(&mut self) -> Result<(), String> {
        let window = self
            .window
            .as_ref()
            .ok_or_else(|| "recreate_surface：无 window".to_string())?;
        let instance = self
            .instance
            .as_ref()
            .ok_or_else(|| "recreate_surface：无 instance".to_string())?;
        let new_surface = instance
            .create_surface(window.clone())
            .map_err(|e| format!("recreate_surface 失败: {e}"))?;
        self.surface = Some(new_surface);
        self.reconfigure_current();
        // 视口同步：与 surface 尺寸保持一致（C4 复审精度改进——与 configure
        // 同一份尺寸快照）。
        if let (Some(core), Some(cfg)) = (self.core.as_mut(), self.surface_config.as_ref())
            && let Err(e) = core.set_viewport(cfg.width.max(1), cfg.height.max(1))
        {
            warn!("recreate_surface 同步模型视口失败: {e}");
        }
        info!("benchmark surface 因 Lost 重建完成");
        Ok(())
    }

    /// 跑一帧。
    ///
    /// 流程（P1-2-P0-2 修复）：
    /// 1. **推进 workload**（生产路径同口径）——driver / adapter / sim_time
    ///    无论 warm-up/formal 都持续推进；
    /// 2. **取帧** → `AcquireOutcome::from_current` → `decide_after_acquire` →
    ///    `SurfaceAction`；
    /// 3. 按 action 执行真实副作用（present / configure / recreate / skip）；
    /// 4. 仅 formal 阶段累计 presented / submit_ms / present_interval。
    ///
    /// 终止条件由调用方在 `window_event` 里按 `formal_attempts` 判定（本方法
    /// 不直接 exit）。
    fn render_one_frame(&mut self) -> Result<(), String> {
        let surface = self.surface.as_ref().ok_or("surface 未就绪")?;
        let device = self.device.as_ref().ok_or("device 未就绪")?;
        let queue = self.queue.as_ref().ok_or("queue 未就绪")?;
        let core = self.core.as_mut().ok_or("core 未就绪")?;
        let driver = self.driver.as_mut().ok_or("driver 未就绪")?;
        let adapter = self.adapter.as_mut().ok_or("adapter 未就绪")?;
        let config = self
            .surface_config
            .as_ref()
            .ok_or("surface_config 未就绪")?;

        // 1. 推进 workload（每帧：tick → apply_frame → apply_mouth_level → core.update → sim_time += SIM_DT）。
        if let Err(e) = step_frame_workload(
            driver,
            adapter,
            core,
            &mut self.frame_buf,
            &mut self.sim_time,
        ) {
            return Err(format!("workload step 失败: {e}"));
        }

        // 2. 取帧 → 复用生产 `decide_after_acquire` 状态机。
        let acquired = surface.get_current_texture();
        let acquire = AcquireOutcome::from_current(&acquired);
        let action = decide_after_acquire(acquire);

        // 3. 按 action 执行真实副作用。
        match action {
            SurfaceAction::Present | SurfaceAction::PresentThenReconfigure => {
                // `Success` / `Suboptimal` 都携带 texture：先 render+present，
                // Suboptimal 再 reconfigure（C4 顺序：先 present 再 configure）。
                let texture = match acquired {
                    wgpu::CurrentSurfaceTexture::Success(t)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                    // 状态机不会把 Success/Suboptimal 映射到 Skip 路径，
                    // 走到这里说明 acquired 与 action 不一致——保守按 skip 处理。
                    _ => {
                        return self.record_skip(&acquired, "acquire-mismatch");
                    }
                };

                // 测量：模式对应 API 调用的耗时。
                let submit_started = Instant::now();
                let mode = self.state.mode;
                let outcome: Result<(), String> = match mode {
                    BenchmarkMode::Submit => core
                        .render_to_view_submit(
                            &texture
                                .texture
                                .create_view(&wgpu::TextureViewDescriptor::default()),
                            config.format,
                        )
                        .map(|_| ())
                        .map_err(|e| format!("render_to_view_submit: {e}")),
                    BenchmarkMode::Blocking => core
                        .render_to_view(
                            &texture
                                .texture
                                .create_view(&wgpu::TextureViewDescriptor::default()),
                            config.format,
                        )
                        .map_err(|e| format!("render_to_view: {e}")),
                };
                let submit_ms = submit_started.elapsed().as_secs_f64() * 1000.0;

                // 帧末 device.poll(Poll)：submit 模式需要推进上传/回调；blocking 模式
                // 已经在内部 Wait 过了，仍做一次 Poll 推进上传队列（C1 闭环同口径）。
                if let Err(e) = device.poll(wgpu::PollType::Poll) {
                    return Err(format!("device.poll 失败: {e:?}"));
                }
                let _ = queue; // 已通过 device.poll 推进

                // render 失败：仍 present（让 swapchain 推进），但不计入
                // presented / submit_ms 样本——口径如实标注。
                if let Err(e) = outcome {
                    texture.present();
                    return Err(e);
                }
                texture.present();

                // 记录样本 + 计数（仅 formal 阶段）。
                if self.state.phase == BenchPhase::Formal {
                    self.record_formal_present(submit_ms);
                }

                // Suboptimal：本帧 present 完成后再 reconfigure（C4 顺序）。
                if matches!(action, SurfaceAction::PresentThenReconfigure) {
                    self.reconfigure_current();
                    if self.state.phase == BenchPhase::Formal {
                        // 仍记 present 一次 + Suboptimal 桶 +1（present 了
                        // 但需要重配；与原 Split 路径口径一致）。
                        self.state.actions.present_then_reconfigure += 1;
                        self.state.skipped.record(SkipKind::Suboptimal);
                    }
                } else if self.state.phase == BenchPhase::Formal {
                    self.state.actions.present += 1;
                }
            }
            SurfaceAction::SkipAndReconfigure => {
                // Outdated：无 texture，直接 configure 跳过本帧。
                if self.state.phase == BenchPhase::Formal {
                    self.record_formal_skip(&acquired);
                }
                self.reconfigure_current();
                if self.state.phase == BenchPhase::Formal {
                    self.state.actions.skip_and_reconfigure += 1;
                }
            }
            SurfaceAction::SkipAndRecreate => {
                // Lost：重建 + configure + viewport 同步后跳过本帧。
                self.recreate_surface()?;
                if self.state.phase == BenchPhase::Formal {
                    self.record_formal_skip(&acquired);
                    self.state.actions.skip_and_recreate += 1;
                }
            }
            SurfaceAction::Skip(_reason) => {
                // Timeout / Occluded / Validation：仅跳过，无副作用。
                if self.state.phase == BenchPhase::Formal {
                    self.record_formal_skip(&acquired);
                    self.state.actions.skip += 1;
                }
            }
        }

        Ok(())
    }

    /// 记录一次 formal 阶段成功 present。
    fn record_formal_present(&mut self, submit_ms: f64) {
        let now = Instant::now();
        if let Some(prev) = self.state.last_present {
            let dt = now.duration_since(prev).as_secs_f64() * 1000.0;
            self.state.present_interval_samples.push(dt);
        }
        self.state.last_present = Some(now);
        self.state.presented += 1;
        self.state.submit_ms_samples.push(submit_ms);
    }

    /// 按 `AcquireOutcome` 记录一次 formal 阶段 skip。
    fn record_formal_skip(&mut self, acquired: &wgpu::CurrentSurfaceTexture) -> Option<String> {
        use super::stats::SkipKind;
        let kind = match acquired {
            wgpu::CurrentSurfaceTexture::Success(_)
            | wgpu::CurrentSurfaceTexture::Suboptimal(_) => {
                // 不应被状态机分类到 skip 路径；保守按 timeout 计数 + 返回 mismatch。
                self.state.skipped.record(SkipKind::Timeout);
                return Some("acquire-mismatch".to_owned());
            }
            wgpu::CurrentSurfaceTexture::Outdated => SkipKind::Outdated,
            wgpu::CurrentSurfaceTexture::Lost => SkipKind::Lost,
            wgpu::CurrentSurfaceTexture::Timeout => SkipKind::Timeout,
            wgpu::CurrentSurfaceTexture::Occluded => SkipKind::Occluded,
            wgpu::CurrentSurfaceTexture::Validation => SkipKind::Validation,
        };
        self.state.skipped.record(kind);
        None
    }

    /// 记录一次 skip（acquire-mismatch 路径专用）。
    fn record_skip(
        &mut self,
        _acquired: &wgpu::CurrentSurfaceTexture,
        reason: &str,
    ) -> Result<(), String> {
        if self.state.phase == BenchPhase::Formal {
            self.state.actions.skip += 1;
        }
        // 收口：acquire-mismatch 不视为 fatal（保守：让下一帧再尝试）。
        let _ = reason;
        Ok(())
    }

    /// 组装报告（在 `event_loop.exit` 之前调用一次）。
    fn build_report(&mut self) {
        let formal_started = self.state.formal_started.unwrap_or_else(Instant::now);
        let finished_at = self.state.finished_at.unwrap_or_else(Instant::now);
        let elapsed = finished_at.duration_since(formal_started);
        let submit_stats = TimingStats::from_samples_ms(&self.state.submit_ms_samples);
        let interval_stats = TimingStats::from_samples_ms(&self.state.present_interval_samples);
        let present_mode = format!(
            "{:?}",
            self.surface_config
                .as_ref()
                .map(|c| c.present_mode)
                .unwrap_or(wgpu::PresentMode::Fifo)
        );
        let frame_latency = self
            .surface_config
            .as_ref()
            .map(|c| c.desired_maximum_frame_latency)
            .unwrap_or(0);
        // P1-2-P1-4 修复：measured_frames = formal_attempts（**真实**
        // redraw 尝试数，不再是配置值 `opts.formal_frames`）。`presented`
        // 是 formal 阶段成功 present 帧数（不 saturating_sub）。
        let measured_frames = self.state.formal_attempts;
        let report = BenchmarkReport {
            mode: self.state.mode,
            model3_path: self.opts.model3_path.display().to_string(),
            window_size: self.state.physical_size,
            warmup_frames: self.opts.warmup_frames,
            formal_frames: self.opts.formal_frames,
            adapter: self.adapter_label.clone(),
            backend: self.backend.clone(),
            present_mode,
            frame_latency,
            submit_ms: submit_stats,
            present_interval_ms: interval_stats,
            measured_frames,
            presented: self.state.presented,
            skipped: self.state.skipped,
            actions: self.state.actions,
            elapsed,
        };
        self.report = Some(report);
    }
}

impl winit::application::ApplicationHandler for BenchmarkApp {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.window.is_none() && self.bootstrap_error.is_none() {
            self.bootstrap(event_loop);
        }
        if self.bootstrap_error.is_some() {
            event_loop.exit();
            return;
        }
        // bootstrap 成功：启动帧循环（warm-up 阶段不计时）。
        self.state.run_started = Some(Instant::now());
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        if self.bootstrap_error.is_some() {
            event_loop.exit();
            return;
        }
        match event {
            winit::event::WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            winit::event::WindowEvent::RedrawRequested => {
                // 阶段推进（P1-2 二轮修复，任务 1）：
                // 1. 单一递增点：当前 phase 的 attempts +1（每次 redraw 一次），
                //    **不**区分 action 类型（Suboptimal 同 +1，Skip 也 +1，
                //    唯一的「不补跑」由调用方在 `Err` 路径处理——本帧
                //    render 失败时本 attempts 仍计入：已发生 redraw 尝试）。
                // 2. `render_one_frame` 仅做 workload + surface action
                //    副作用（**不**改 attempts）。
                // 3. 调 `advance_benchmark_phase`（incremented = 1）得
                //    决策，按决策执行：
                //    - `EnterFormal`：清空全部 formal 统计（从 0 起算）
                //    - `Finish`：phase = Done + build_report + exit
                //    - `StayWarmup` / `StayFormal`：继续 redraw。
                match self.state.phase {
                    BenchPhase::Warmup => self.state.warmup_attempts += 1,
                    BenchPhase::Formal => self.state.formal_attempts += 1,
                    BenchPhase::Done => {
                        // 已收口；保险退出（理论上窗口不应再发 redraw）。
                        event_loop.exit();
                        return;
                    }
                }
                match self.render_one_frame() {
                    Ok(()) => {
                        let decision = advance_benchmark_phase(
                            self.state.phase,
                            self.state.warmup_attempts,
                            self.state.formal_attempts,
                            self.state.warmup_target,
                            self.state.formal_target,
                            1,
                        );
                        match decision {
                            PhaseDecision::StayWarmup | PhaseDecision::StayFormal => {
                                if let Some(window) = self.window.as_ref() {
                                    window.request_redraw();
                                }
                            }
                            PhaseDecision::EnterFormal => {
                                // 清空全部 formal 统计（**唯一**清零点）。
                                self.state.phase = BenchPhase::Formal;
                                self.state.formal_attempts = 0;
                                self.state.presented = 0;
                                self.state.skipped = SkipBuckets::default();
                                self.state.actions = ActionCounts::default();
                                self.state.submit_ms_samples.clear();
                                self.state.present_interval_samples.clear();
                                self.state.last_present = None;
                                self.state.formal_started = Some(Instant::now());
                                if let Some(window) = self.window.as_ref() {
                                    window.request_redraw();
                                }
                            }
                            PhaseDecision::Finish => {
                                self.state.phase = BenchPhase::Done;
                                self.state.finished_at = Some(Instant::now());
                                self.build_report();
                                event_loop.exit();
                            }
                        }
                    }
                    Err(e) => {
                        self.bootstrap_error = Some(e);
                        event_loop.exit();
                    }
                }
            }
            _ => {}
        }
    }
}
