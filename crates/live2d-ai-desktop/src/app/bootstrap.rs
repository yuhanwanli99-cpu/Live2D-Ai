//! GPU/窗口 bootstrap：窗口创建 + wgpu surface/adapter/device/queue +
//! surface 配置 + 桌宠能力探测 + （可选）模型加载。
//!
//! - [`ShellApp::bootstrap`]：resumed 钩子主体。失败路径全部归入环境错误
//!   （无显示服务/无可用 adapter/device 属环境问题，不是代码缺陷）。
//! - [`ShellApp::build_model_state`]：在共享 device/queue 上接入 GpuContext、
//!   解析模型包、构建渲染核心。

use std::sync::Arc;

use l2d::asset::ModelPackage;
use l2d::model::LoadedModel;
use l2d::renderer::{GpuContext, ModelRendererCore};
use tracing::{error, info, warn};
use winit::dpi::LogicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use crate::adapter::BaiParamAdapter;
use crate::app::capability::{choose_surface_format, observed_alpha_mode, wgpu_alpha_mode};
use crate::app::types::{ModelState, ShellApp, SurfaceState};
use crate::model_smoke::{
    ActionSequence, ModelSmokeError, SmokeDriver, classify_core_init_error, classify_frame_error,
    classify_load_error,
};
use crate::platform::CapabilityState;

impl ShellApp {
    /// resumed 钩子主体：窗口 + GPU 初始化。失败路径全部归入环境错误
    /// （无显示服务/无可用 adapter/device 属环境问题，不是代码缺陷）。
    pub(crate) fn bootstrap(&mut self, event_loop: &ActiveEventLoop) {
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

        // ---- 异步 GPU 错误收口（C2 裁决第 45-63 行）----
        // 写闩：latest-wins，事件循环线程每帧开始消费。回调闭包不得直接
        // 操作 ShellApp（`Fn(Error) + Send + Sync + 'static`），只更新闩。
        let gpu_fault_latch: std::sync::Arc<std::sync::Mutex<Option<crate::app::frame::GpuFault>>> =
            std::sync::Arc::new(std::sync::Mutex::new(None));
        let latch_for_handler = std::sync::Arc::clone(&gpu_fault_latch);
        device.on_uncaptured_error(std::sync::Arc::new(move |err: wgpu::Error| {
            let kind = match &err {
                wgpu::Error::Validation { .. } => crate::app::frame::GpuFaultKind::Validation,
                wgpu::Error::OutOfMemory { .. } => crate::app::frame::GpuFaultKind::OutOfMemory,
                wgpu::Error::Internal { .. } => crate::app::frame::GpuFaultKind::Internal,
            };
            // source 链：逐级 downcast 到字符串（深度受限，循环打断）。
            let source_chain = {
                let mut chain = Vec::new();
                let mut src: Option<&(dyn std::error::Error + 'static)> =
                    std::error::Error::source(&err);
                let mut depth = 0;
                while let Some(s) = src
                    && depth < 8
                {
                    chain.push(s.to_string());
                    src = std::error::Error::source(s);
                    depth += 1;
                }
                chain.join(" -> ")
            };
            let description = err.to_string();
            if let Ok(mut guard) = latch_for_handler.lock() {
                *guard = Some(crate::app::frame::GpuFault {
                    kind,
                    description,
                    source_chain,
                });
            }
        }));

        // 能力协商：format / alpha mode 全部来自 surface 对该 adapter 的实测 caps。
        let caps = surface.get_capabilities(&adapter);
        let Some(format) = choose_surface_format(&caps.formats) else {
            self.fail_environment(
                event_loop,
                "surface 未提供任何可用纹理格式（adapter 与 surface 不兼容）".to_string(),
            );
            return;
        };
        let observed_modes: Vec<_> = caps
            .alpha_modes
            .iter()
            .copied()
            .map(observed_alpha_mode)
            .collect();
        let chosen = crate::platform::choose_alpha_mode(&observed_modes);
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
            gpu_fault_latch,
            last_submission_index: None,
        });
        self.window = Some(window.clone());

        // ---- 设置面板（egui 0.35 overlay；W6 接线 + W7 表单/保存）----
        // 用与 surface 完全共享的 device/queue 构造 egui-wgpu Renderer，egui-winit
        // 的 `State::new` 需要 `&dyn HasDisplayHandle`（`winit::Window` 自带 impl）。
        // 面板默认不可见（visible=false）——F10 显隐由 handler 触发；不可见时
        // `render_overlay` 第一步 return，零开销不影响 C7 基准。
        //
        // W7 增强：注入 `config_path`（保存按钮写盘目标）和 `supervisor`
        // （写盘成功后触发热重载）。supervisor 来自 `ChatBridge`：
        // - chat 模式（`self.chat.is_some()`）：注入真实 `Arc<SupervisorHandle>`；
        // - smoke 模式（无 chat bridge）：`None`，保存按钮只写盘不热重载
        //   （与「无 chat 装配」的语义一致——没有 supervisor 可通知）。
        let (device_ref, format) = {
            let s = self
                .state
                .as_ref()
                .expect("SurfaceState 刚刚才塞入 self.state");
            (s.device.clone(), s.config.format)
        };
        let supervisor_for_ui = self
            .chat
            .as_ref()
            .map(|b| std::sync::Arc::clone(&b.supervisor));
        // M0-5 修复：从 ChatBridge 取真实 config_path（chat 模式恒为 Some），
        // 与 web_api `ServerContext::config_path` 同源——保存按钮有了写盘目标。
        let config_path_for_ui = self.chat.as_ref().and_then(|b| b.config_path.clone());
        let mut ui_state = crate::app::settings_ui::init(
            &device_ref,
            format,
            0,
            config_path_for_ui.clone(),
            supervisor_for_ui,
        );
        // M0-5 修复：启动时把磁盘上已加载的 settings 回显到表单 Draft，
        // 避免「面板空表单 + 保存覆盖原配置」的闭环缺口。config 解析失败
        // （首次无配置）时保持默认 Draft，保存按钮仍提示无 path 走引导。
        if let Some(path) = config_path_for_ui.as_deref()
            && let Ok(loaded) =
                live2d_ai_runtime::AppSettings::load_from_path(std::path::Path::new(path))
        {
            ui_state.draft = crate::app::settings_ui::settings_to_draft(&loaded);
        }
        // 真实 `State::new`：传 0.35 真实签名所需的 `&dyn HasDisplayHandle` =
        // `&Window`（winit 0.30 的 Window 自带 rwh_06 HasDisplayHandle impl）。
        ui_state.winit_state = Some(egui_winit::State::new(
            ui_state.ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        ));
        self.settings_ui = Some(ui_state);

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
    pub(crate) fn build_model_state(
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
}
