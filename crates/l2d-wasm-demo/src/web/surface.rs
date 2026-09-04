//! WASM 端 surface / 交换链 / 单帧调度模块（C4 裁决落地处）。
//!
//! 与 `main.rs` 同样只在 `target_arch = "wasm32"` 下编译（由父模块的
//! `#[cfg(target_arch = "wasm32")]` 门控）。承载：
//! - [`init_gpu`]：WebGPU 优先、WebGL2 回退；显式设置 `desired_maximum_frame_latency = 2`（C3）。
//! - [`FrameState`] / [`SharedState`] / [`FrameSlot`]：rAF 循环共享态与递归闭包句柄。
//! - [`install_resize_handler`]：resize → 重配 surface → 同步视口。
//! - [`tick`]：单帧状态机，C4 裁决的 surface 生命周期契约：
//!   - Suboptimal：先 render/present 当前 texture，再 configure（frame 存活时
//!     configure 会 panic，本分支历史上违反过该契约）。
//!   - Outdated：无 texture，直接应用当前 config 后跳过本帧。
//!   - Lost：configure + 同步视口。
//!   - Timeout/Occluded：正常瞬态，跳过本帧。
//!   - Validation：首次提示后继续；连续两次命中 → 写 fatal 状态栏并停循环
//!     （P0-2-2：避免 GPU 校验错误状态下无限重试）。
//! - [`render_and_present`]：frame → view → **render_to_view_submit**（实时
//!   路径，不得调用阻塞的 `render_to_view`——C1/C6/C8 硬门禁）→ present。

use std::{cell::RefCell, rc::Rc, sync::Arc};

use glam::f32::{Affine2, Vec2};
use l2d::renderer::{FIXED_DT_60HZ, GpuContext, ModelRendererCore};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{HtmlCanvasElement, Window};

use super::status;
use crate::web::net::js_str;

/// HUD 统计窗口长度（ms）。500ms 刷新一次 DOM，远低于每帧刷屏成本；
/// 同时能覆盖足够多的帧样本估出稳定 FPS。
const HUD_INTERVAL_MS: f64 = 500.0;

/// 口型音量显示衰减时间常数（ms）。
/// `volume_display *= exp(-dt_ms / TAU_MS)` —— 衰减到 37% 需时 TAU_MS。
/// 160ms 使普通语速下的音节尖端自然衰落，避免高帧率（240fps）下口型
/// 死板硬扣音量峰值。该时间常数与帧率无关：60fps 与 240fps 分别给出
/// `exp(-16.67/160)≈0.902` 与 `exp(-4.17/160)≈0.974`，总衰减速率一致。
const TAU_MS: f64 = 160.0;

/// 创建 Canvas surface 并协商设备/格式/尺寸（WebGPU 优先，WebGL2 回退）。
///
/// 回退语义由 wgpu 承担：instance 同时声明 `BROWSER_WEBGPU | GL` 后端，
/// `webgl` 特性使 GL 落到 WebGL2；adapter 协商成功即用其真实后端，
/// 状态栏展示实际选择，不做任何伪装。
pub(crate) async fn init_gpu(
    window: &Window,
    canvas: &HtmlCanvasElement,
) -> Result<
    (
        Arc<GpuContext>,
        wgpu::Adapter,
        wgpu::Surface<'static>,
        wgpu::SurfaceConfiguration,
    ),
    String,
> {
    // wgpu 29：BROWSER_WEBGPU 与 GL 同时声明——浏览器有 navigator.gpu 时走
    // WebGPU 原生路径；缺失/不可用时落到 wgpu-core 适配器，配合 `webgl`
    // 特性即 WebGL2 回退（见 wgpu InstanceDescriptor 文档）。
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL;
    let instance = wgpu::Instance::new(descriptor);
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
        .map_err(|e| format!("创建 surface 失败：{e}"))?;
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            // Live2D 主展示路径：优先独显 / 高性能 adapter。
            // 双显卡 Windows 上，LowPower 让浏览器选核显，导致帧率下降；
            // HighPerformance 走 NVIDIA/AMD 独显，才能承担实时 moc3 渲染 +
            // IK 骨骼计算。（PowerPreference 只是 Hint，浏览器仍可据电源与
            // 用户设定覆写——这里只表达“愿选性能优的”。）
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .await
        .map_err(|e| format!("无可适配配器（WebGPU 与 WebGL 均不可用或浏览器未启用）：{e}"))?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("l2d-wasm-demo"),
            ..Default::default()
        })
        .await
        .map_err(|e| format!("GPU 设备请求失败：{e}"))?;

    // ModelRendererCore 的共享上下文签名要求 Arc<GpuContext>（native 下
    // GpuContext 为 Send+Sync）；wasm 单线程且 wgpu web 后端默认非 Send，
    // 该 Arc 不会跨线程使用——本行豁免 clippy 提示。
    #[allow(clippy::arc_with_non_send_sync)]
    let gpu = Arc::new(GpuContext::from_parts_with_label(
        device,
        queue,
        format!("browser {}", backend_label(&adapter)),
    ));

    // 格式优先 Rgba8/Bgra8Unorm（渲染核心按 sRGB 输出直通色），否则用首个支持项；
    // alpha 优先 PreMultiplied（上游渲染输出即预乘 alpha），否则首个支持项。
    let caps = surface.get_capabilities(&adapter);
    let format = caps
        .formats
        .iter()
        .copied()
        .find(|f| {
            matches!(
                f,
                wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
            )
        })
        .or_else(|| caps.formats.first().copied())
        .ok_or_else(|| "surface 无可用像素格式".to_owned())?;
    let alpha_mode = caps
        .alpha_modes
        .iter()
        .copied()
        .find(|a| *a == wgpu::CompositeAlphaMode::PreMultiplied)
        .or_else(|| caps.alpha_modes.first().copied())
        .ok_or_else(|| "surface 无可用 alpha 合成模式".to_owned())?;

    let (width, height) = canvas_pixel_size(canvas, window);
    let mut config = surface
        .get_default_config(&adapter, width, height)
        .ok_or_else(|| "无法协商 surface 配置".to_owned())?;
    config.format = format;
    config.alpha_mode = alpha_mode;
    // C3 裁决：WASM 端显式设置 desired_maximum_frame_latency = 2，
    // 不依赖 wgpu 版本缺省值（与 desktop 一致，构成可对照承诺）。
    config.desired_maximum_frame_latency = 2;
    surface.configure(gpu.device(), &config);
    Ok((gpu, adapter, surface, config))
}

/// 实际选中的后端标签（观测用：区分 WebGPU 原生路径与 WebGL2 回退路径）。
pub(crate) fn backend_label(adapter: &wgpu::Adapter) -> String {
    let info = adapter.get_info();
    format!("{:?} ({})", info.backend, info.name)
}

/// 供 HUD 展示用的 adapter 原始信息：
/// - `backend`：wgpu 后端枚举（Vulkan/Dx12/Metal/Gl/BrowserWebGpu …）；
/// - `name`：adapter 设备名（如 `NVIDIA GeForce RTX 5060`）；
/// - `is_webgpu`：是否走浏览器原生 WebGPU 路径（BrowserWebGpu => true，
///   Gl => WebGL2 回退 => false）。
pub(crate) fn adapter_info(adapter: &wgpu::Adapter) -> (wgpu::Backend, String, bool) {
    let info = adapter.get_info();
    (
        info.backend,
        info.name.clone(),
        info.backend == wgpu::Backend::BrowserWebGpu,
    )
}

/// CSS 像素 × devicePixelRatio = 期望的物理画布尺寸（至少 1×1）。
///
/// **DPR 钳制**（C4 裁决 v1）：物理像素 = CSS 尺寸 × `min(dpr, 1.5)`。
/// 全面屏 / 高 DPI 屏的 dpr 常达 2–3，裸采会把像素数翻到 4–9 倍，压垮
/// WebGPU 着色器与 16ms 预算——Live2D 的骨骼 IK + 多层混合已是瓶颈。
/// 钢鞥 1.5 可在锐度与性能间取平衡：
///   - Performance 0.75：像素数最少，最省 GPU，适应旧设备；
///   - Balanced 1.0  ：dpr 钳至 1.0，1:1 渲染，中等清晰度；
///   - Quality 1.5  ：当前默认 cap，钝化 2 以上高 DPI 冗余像素。
///
/// v1 不暴露 UI 档位，固定 1.5 cap；如后需切换等级，改 `DPR_CAP` 即可。
const DPR_CAP: f64 = 1.5;
pub(crate) fn canvas_pixel_size(canvas: &HtmlCanvasElement, window: &Window) -> (u32, u32) {
    // 钳 dpr 到 [1, DPR_CAP]，保证最小 1 倍（HiDPI 缩放也得有下限）。
    let dpr = window.device_pixel_ratio().clamp(1.0, DPR_CAP);
    let css_w = canvas.client_width().max(0) as f64;
    let css_h = canvas.client_height().max(0) as f64;
    let width = ((css_w * dpr).round() as u32).max(1);
    let height = ((css_h * dpr).round() as u32).max(1);
    (width, height)
}

/// iframe bridge 接收状态（P0-2a）：父页 postMessage → 可见效果。
/// 首帧后由 main.rs 的 message listener 写入；rAF 循环每帧读取产生可见效果。
#[derive(Clone)]
pub(crate) struct BridgeState {
    /// stage-config.scale（滚轮/滑块缩放，0.5..2.0，默认 1.0）。
    /// 应用为 ModelRendererCore 的逻辑变换缩放（以画布中心为锚）。
    pub scale: f32,
    /// 左键拖动累计平移（NDC 单位，Default 0.0）。
    /// 由 pointermove 在拖盘过程中写入，写入前已换算为 NDC。
    pub offset_x: f32,
    pub offset_y: f32,
    /// stage-config.dark（背景暗色，默认 true）
    pub dark: bool,
    /// 已写到 DOM 的 background-color 值（用于只在变化时写 DOM，规避每帧
    /// set_property 触发 style recalc 的抖动——240Hz 下 DOM 写是主卡点）。
    pub applied_dark: bool,
    /// stage-config.lipSync（口型开关，默认 true）
    pub lip_sync: bool,
    /// 最近 audio-volume（0..1；每 20ms 音频帧更新）
    pub volume: f32,
    /// 衰减后的口型显示值（rAF 每帧 volume_display = volume.max(volume_display*0.85)）
    pub volume_display: f32,
    /// 进行中的参数等效动画（action-state start 创建 / end 或 t>=1 结束）
    pub action: Option<ActiveAction>,
    /// 诊断（P3.1）：收到的消息数 / 应用数 / 最近类型——HUD 显示定位"模型不见"。
    pub msg_recv: u32,
    pub msg_applied: u32,
    pub last_msg: String,
}

/// 进行中的动作动画（无内置 motion 段时的参数等效实现）。
#[derive(Clone)]
pub(crate) struct ActiveAction {
    /// 动作名（nod/shake_no/tilt/look_around/listen/surprise）
    pub name: String,
    /// 强度倍率（strength 1/2/3 → 0.6/1.0/1.5）
    pub strength_mult: f32,
    /// 开始时刻（performance.now() 毫秒）
    pub started_ms: f64,
    /// 动画时长（默认 1500ms；end 帧提前结束）。
    ///
    /// 800ms 实测因波形 `(t*2π).sin()` 整周期过快、肉眼难辨且 end 帧易提前清除而
    /// 看不见动作；延长到 1500ms 使单次摆动达到约 3/4 周期，人眼可清晰捕捉。
    pub duration_ms: f64,
}

impl Default for BridgeState {
    fn default() -> Self {
        Self {
            scale: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            dark: true,
            applied_dark: false,
            lip_sync: true,
            volume: 0.0,
            volume_display: 0.0,
            action: None,
            msg_recv: 0,
            msg_applied: 0,
            last_msg: String::new(),
        }
    }
}

/// rAF 循环共享态：渲染核心 + surface + 配置 + 尺寸来源。
pub(crate) struct FrameState {
    pub(crate) core: ModelRendererCore,
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) gpu: Arc<GpuContext>,
    pub(crate) canvas: HtmlCanvasElement,
    pub(crate) window: Window,
    /// 校验类取帧错误只提示一次（避免每帧刷屏）。
    pub(crate) warned_validation: bool,
    /// 连续 `CurrentSurfaceTexture::Validation` 命中计数（P0-2-2）：
    /// `>= 2` 时判定为 fatal、写状态栏并停 rAF 调度，避免 GPU 校验错误
    /// 状态下无限重试。任意非 Validation 取帧结果会清零。
    pub(crate) validation_streak: u8,
    /// 运行时 HUD 统计（adapter 固定在首次写入时快照，其余每 500ms 刷新）。
    pub(crate) hud: HudState,
    pub bridge: BridgeState,
    /// RM6 待机生命体征采样器（呼吸/眨眼/微表情）。
    pub(crate) idle: IdleState,
    /// load_model 后立即捕获的 layout 初始 transform（Affine2）。
    /// 每帧 set_transform 都在此基础上右乘用户缩放/平移，保持 layout 构图
    /// 不变，仅叠加交互变换。
    /// 累积未模拟的真实时间（秒）：用于帧率无关的固定 dt 步进（R4）。
    /// rAF 时间戳到达时累加 real_dt，直到累积 >= FIXED_DT_60HZ 再消费一次
    /// core.update，避免 240Hz 时物理/姿态模拟跑到 4× 速度。
    pub(crate) last_layout_transform: Affine2,
    pub(crate) accumulator: f64,
}

/// 运行时性能 HUD 统计态。
///
/// 生命周期：rAF 循环内逐帧累积，错峰 500ms 刷屏一次 DOM。
/// - `window_start`：当前统计窗起点（performance.now()）；
/// - `frame_count`：本窗内已渲染帧数；
/// - `cpu_time_sum`：本窗内 CPU 渲染耗时总和（ms），滑动求平均帧耗时；
/// - `last_cpu_ms`：最近一次渲染耗时（ms），用于实时反馈；
/// - `adapter_line`：adapter 信息快照（一次性写入）；
/// - `pending`/`due`：500ms 刷新节拍标记。
pub(crate) struct HudState {
    window_start: f64,
    frame_count: u32,
    cpu_time_sum: f64,
    last_cpu_ms: f64,
    adapter_line: Option<String>,
    last_fps: f64,
    /// 上一帧 rAF 时间戳（performance.now()，毫秒）。用于计算真实帧间 dt。
    /// None 表示首帧，dt 使用 FIXED_DT_60HZ 兜底。
    last_frame_ms: Option<f64>,
    /// R4：最近一次 tick 消耗的模拟步数（core.update 调用次数）。
    /// 证明帧率无关：60fps 恒为 1，240fps 偶尔 2-4。
    last_sim_steps: u32,
}

impl Default for HudState {
    fn default() -> Self {
        let now = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0);
        Self {
            window_start: now,
            frame_count: 0,
            cpu_time_sum: 0.0,
            last_cpu_ms: 0.0,
            adapter_line: None,
            last_fps: 0.0,
            last_frame_ms: None,
            last_sim_steps: 0,
        }
    }
}

impl HudState {
    /// 一次性写入 adapter 信息（在 init_gpu 后、rAF 启动前调用）。
    pub(crate) fn snapshot_adapter(
        &mut self,
        backend: wgpu::Backend,
        adapter_name: &str,
        is_webgpu: bool,
    ) {
        // WebGPU 原生路 → "WebGPU"；GL 回退 → "WebGL2"。
        let api = if is_webgpu { "WebGPU" } else { "WebGL2" };
        self.adapter_line = Some(format!("{backend}/{api} ({adapter_name})"));
    }

    /// 记录一帧 CPU 渲染耗时（由 render_and_present 调用）。
    pub(crate) fn record_frame(&mut self, cpu_ms: f64) {
        self.frame_count = self.frame_count.saturating_add(1);
        self.cpu_time_sum += cpu_ms;
        self.last_cpu_ms = cpu_ms;
    }

    /// 是否到了 500ms 刷新 DOM 的时机。
    pub(crate) fn due(&self) -> bool {
        let now = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0);
        now - self.window_start >= HUD_INTERVAL_MS
    }

    /// 刷新统计窗：结算当前 FPS/CPU 均值，重置计数器。
    pub(crate) fn flush(&mut self) {
        let elapsed = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(self.window_start + HUD_INTERVAL_MS)
            - self.window_start;
        if elapsed > 0.0 && self.frame_count > 0 {
            self.last_fps = (self.frame_count as f64 / elapsed) * 1000.0;
        }
        self.window_start = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(self.window_start + HUD_INTERVAL_MS);
        self.frame_count = 0;
        self.cpu_time_sum = 0.0;
    }
}

pub(crate) type SharedState = Rc<RefCell<FrameState>>;
pub(crate) type FrameClosure = Closure<dyn FnMut(f64)>;
pub(crate) type FrameSlot = Rc<RefCell<Option<FrameClosure>>>;

/// resize 处理：重算物理尺寸 → 重配 surface → 同步渲染核心视口。
/// （窗口缩放/DPR 变化都会触发；尺寸不变时跳过。）
///
/// **C4 裁决 P0-3 WASM 同步**（与 desktop 同语义）：
/// - 同一尺寸重复触发由 `(width, height) == (st.config.width, st.config.height)`
///   守卫直接 return，等价于 desktop 的 latest-size 合并（last-wins）；
/// - **不**使用固定毫秒防抖：rAF 循环天然每帧重读 `canvas.client_*`，
///   `tick` 在每次 `get_current_texture` 前都按当前 config 出帧；
///   浏览器连续 resize 事件间无未 present 的 swapchain texture 存活
///   （`install_resize_handler` 与 `tick` 不并发），所以可立即 configure
///   而不违反"先 present 再 configure"契约；
/// - 因此**不**像 desktop 那样维护 `pending_surface_size`：WASM 路径
///   resize → configure → set_viewport 在 rAF 帧间是原子的，合并语义
///   已被 rAF 节流 + 同尺寸守卫覆盖。
pub(crate) fn install_resize_handler(state: &SharedState) {
    let handler: Closure<dyn FnMut()> = Closure::new({
        let state = Rc::clone(state);
        move || {
            let mut st = state.borrow_mut();
            let (width, height) = canvas_pixel_size(&st.canvas, &st.window);
            if (width, height) == (st.config.width, st.config.height) {
                return;
            }
            st.config.width = width;
            st.config.height = height;
            st.surface.configure(st.gpu.device(), &st.config);
            if let Err(e) = st.core.set_viewport(width, height) {
                status(&format!("resize 失败：{e}"));
            }
        }
    });
    let window = state.borrow().window.clone();
    if let Err(e) =
        window.add_event_listener_with_callback("resize", handler.as_ref().unchecked_ref())
    {
        status(&format!("注册 resize 监听失败：{}", js_str(&e)));
        return;
    }
    // 页面生命周期内常驻（demo 不做卸载路径），防止闭包被回收。
    std::mem::forget(handler);
}

/// 单帧：固定 dt 步进姿态/物理 → 取交换链纹理 → **render_to_view_submit** → present。
/// 致命错误写状态栏并停止调度；C4 裁决的 surface 生命周期契约见模块头注释。
///
/// **与 desktop `render_frame` 的契约对照**（C4 P0-3 同步，决策语义一致）：
/// - `Suboptimal(frame)` → 先 `render_and_present`，**再** `configure`
///   （texture 仍存活，先 configure 会 panic）；desktop 走
///   `FrameOutcome::PresentedThenReconfigure` + `reconfigure_current`，
///   同一合约；
/// - `Outdated` → 无 texture，直接 `configure` 跳过本帧；
/// - `Lost` → `configure` + `set_viewport`；desktop 走 `recreate_surface`
///   （重建 surface + configure + viewport），demo 简化为同 surface 上的
///   configure + viewport（wgpu web 端在 Lost 后只需重新协商 surface 配置）；
/// - `Timeout` / `Occluded` → 静默跳过本帧；
///   `Validation` → 首次提示后跳过本帧、连续第二次命中视为 fatal 并停循环
///   （见 P0-2-2 裁决）。
pub(crate) fn tick(slot: &FrameSlot, state: &SharedState, now_ms: f64) {
    // R4：帧率无关的时间积累。rAF 时间戳 → real_dt（秒，0.1s 防巨步）→
    // 累积到 accumulator，直到 >= FIXED_DT_60HZ 才消费一次 core.update。
    // 这样 60fps 与 240fps 每秒模拟步数一致（60 步），避免 240fps 跑到 4× 速度。
    let dt_millis = {
        let mut st = state.borrow_mut();
        let real_dt = match st.hud.last_frame_ms {
            Some(prev) if now_ms > prev => ((now_ms - prev) / 1000.0).clamp(0.0, 0.1),
            _ => FIXED_DT_60HZ as f64,
        };
        st.hud.last_frame_ms = Some(now_ms);
        st.accumulator += real_dt;
        let mut steps = 0u32;
        while st.accumulator >= FIXED_DT_60HZ as f64 && steps < 5 {
            if let Err(e) = st.core.update(FIXED_DT_60HZ) {
                drop(st);
                status(&format!("停止渲染循环：update({FIXED_DT_60HZ}) 失败：{e}"));
                return;
            }
            st.accumulator -= FIXED_DT_60HZ as f64;
            steps += 1;
        }
        // 记录本轮模拟步数供 HUD 显示（R4）。
        st.hud.last_sim_steps = steps;
        // real_dt 毫秒用于口型衰减（dt 归一化）。
        real_dt * 1000.0
    };

    // bridge 状态 → 可见效果（口型/动作/缩放/背景），独立借用窗口。
    apply_bridge_effects(state, dt_millis);

    let mut st = state.borrow_mut();
    let keep_going = match st.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(frame) => {
            st.validation_streak = 0;
            render_and_present(&mut st, frame)
        }
        // C4 裁决：frame 仍存活时不得先 configure；先渲染并 present，
        // 让 frame 在 drop 前完成提交与呈现，再 reconfigure。
        wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
            st.validation_streak = 0;
            let presented = render_and_present(&mut st, frame);
            st.surface.configure(st.gpu.device(), &st.config);
            presented
        }
        // C4 裁决：surface 配置过期，无 texture 可渲，直接应用当前
        // config 后跳过本帧（下一帧按新 config 重新出帧）。
        wgpu::CurrentSurfaceTexture::Outdated => {
            st.validation_streak = 0;
            st.surface.configure(st.gpu.device(), &st.config);
            true
        }
        // C4 裁决：surface Lost 需重配并同步视口（configure 后
        // 渲染核心的 viewport 需对齐 surface 当前尺寸）。
        wgpu::CurrentSurfaceTexture::Lost => {
            st.validation_streak = 0;
            st.surface.configure(st.gpu.device(), &st.config);
            // 先取尺寸副本，避免与 set_viewport 的 mut 借用冲突。
            let (vw, vh) = (st.config.width, st.config.height);
            if let Err(e) = st.core.set_viewport(vw, vh) {
                status(&format!("Lost 后视口同步失败：{e}"));
            }
            true
        }
        // 超时/窗口被遮挡：正常瞬态，静默跳过。
        wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
            st.validation_streak = 0;
            true
        }
        // P0-2-2 裁决：首次 Validation 记录后跳过本帧并继续循环；连续
        // 第二次命中视为 fatal——写状态栏并返回 `false` 停 rAF 调度，
        // 避免 GPU 校验错误状态下无限重试。任意非 Validation 取帧结果
        // 在上列分支把 `validation_streak` 清零，所以仅"连续"两次才 fatal。
        wgpu::CurrentSurfaceTexture::Validation => {
            st.validation_streak = st.validation_streak.saturating_add(1);
            if st.validation_streak == 1 {
                st.warned_validation = true;
                status("surface 取帧校验错误（已跳过该帧，继续循环）");
                true
            } else {
                status(&format!(
                    "停止渲染循环：连续 {} 次 surface 取帧校验错误，判定为 fatal（GPU 校验错误状态下不再无限重试）。",
                    st.validation_streak
                ));
                false
            }
        }
    };

    drop(st);
    if keep_going {
        // 错峰 500ms 刷新一次 HUD（DOM 写操作），再重新调度下一帧。
        // write_hud_if_due/clone 已完成的 state borrow，新 borrow 互不冲突。
        write_hud_if_due(state);
        crate::web::schedule_frame(slot);
    }
}

/// RM6 待机生命体征层（idle-breath / idle-blink / idle-micro-expr）。
///
/// 照抄 py 版 `idle-breath.ts` / `idle-blink.ts` / `idle-micro-expr.ts`（MIT），
/// 写入 input 层 `set_parameter` —— 优先级低于动作的 final_override 层，
/// 因此做 nod 时头部角度自然盖过 idle 微表情；但 breath/blink 参数
/// （ParamBreath / ParamEyeLOpen / ParamEyeROpen）动作不写入它们，
/// 二者天然不打架。
///
/// 状态机与 py 版同构：
/// - **呼吸**：周期 [3.1, 5]s 随机锁定，正弦 `0.5 + 0.15*sin(2π·t/period)`。
/// - **眨眼**：间隔 [3000, 7500]ms 随机触发；闭眼 75ms 线性 1→0；睁眼
///   [150,300]ms 随机线性 0→1。状态机：Open → Closing → Opening → Open。
/// - **微表情**：间隔 [3,6]s 随机触发；随机选一个参数 ±0.05，400ms fade。
///
/// 与 py 版差异（v1 简化）：
/// - py 版会在动作播放期间**挂起** idle（surprise 用 override 层写 EyeLOpen/ROpen
///   时，idle blink 被挂起避免冲突）；本版**不挂起**——动作 override 层盖住同名
///   参数即可，呼吸/眨眼参数（ParamBreath/EyeL/R）动作不动它们，天然不打架。
///   详见 `apply_idle_life`。
///
/// dt 来源：`apply_bridge_effects(state, dt_millis)` 已有 dt（R4 加的），
/// idle 采样用它，帧率无关。
///
/// RNG：wasm 环境无 `rand` crate（体积与 Send 约束），手写 xorshift64。
/// 种子用启动时 `performance.now() as u64`——每次加载不同，待机天然随机。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlinkPhase {
    /// 睁眼待机：不写参数（保持 1.0），等待随机间隔。
    Open,
    /// 闭眼中：线性 1→0，固定 75ms。
    Closing,
    /// 睁眼中：线性 0→1，随机 [150,300]ms。
    Opening,
}

/// idle 生命体征采样器状态（储存在 FrameState，跨帧持续）。
///
/// xorshift64 状态 `rng_state` 永不为 0（见 `rng_next` 种子保证）。
pub(crate) struct IdleState {
    /// 呼吸周期 ms（随机锁定 [3100, 5000]）。
    breath_period_ms: f64,
    /// 呼吸相位起点（performance.now() 毫秒）。
    breath_t0_ms: f64,
    /// 下次眨眼触发时刻（now + rand[3000,7500]）。
    blink_next_ms: f64,
    /// 眨眼当前相位。
    blink_phase: BlinkPhase,
    /// 眨眼当前相位已推进 ms。
    blink_phase_ms: f64,
    /// 单次闭眼固定时长 ms（py 版 75ms 定步长）。
    blink_close_ms: f64,
    /// 本次睁眼总时长 ms（随机 [150,300]）。
    blink_open_ms: f64,
    /// 下次微表情触发时刻（now + rand[3000,6000]）。
    micro_next_ms: f64,
    /// 进行中的微表情：(参数 ID, 目标 delta, 开始时刻 ms)。
    /// fade 进 1/3 → 停 1/3 → fade 出 1/3，共 400ms。
    micro_active: Option<(String, f32, f64)>,
    /// xorshift64 状态（永不为 0）。
    rng_state: u64,
}

impl IdleState {
    /// 用当前时间作为种子，随机初始化呼吸周期 / 眨眼间隔 / 微表情间隔。
    pub(crate) fn new(now_ms: f64) -> Self {
        // xorshift64 种子：performance.now() 微秒部分 + 恒定盐，保证非零。
        let mut s = (now_ms as u64).wrapping_mul(0x9E3779B97F4A6295) | 1;
        let breath_period_ms = rng_range_f64(&mut s, 3100.0, 5000.0);
        let blink_next_ms = now_ms + rng_range_f64(&mut s, 3000.0, 7500.0);
        let micro_next_ms = now_ms + rng_range_f64(&mut s, 3000.0, 6000.0);
        let blink_open_ms = rng_range_f64(&mut s, 150.0, 300.0);
        Self {
            breath_period_ms,
            breath_t0_ms: now_ms,
            blink_next_ms,
            blink_phase: BlinkPhase::Open,
            blink_phase_ms: 0.0,
            blink_close_ms: 75.0,
            blink_open_ms,
            micro_next_ms,
            micro_active: None,
            rng_state: s,
        }
    }
}

/// xorshift64 伪随机（wasm 无 rand crate，手写）。
/// 三次异或移位，状态永不为 0（`new` 用 `| 1` 保证）。
#[inline]
fn rng_next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

/// 返回 `[min, max)` 区间内的随机 f64。
#[inline]
fn rng_range_f64(state: &mut u64, min: f64, max: f64) -> f64 {
    let span = max - min;
    // 取 state 高 53 位（u64 → [0,1)）以保证精度。
    let r = (rng_next(state) >> 11) as f64 / ((1u64 << 53) as f64);
    min + r * span
}

/// 眨眼闭眼/睁眼线性插值因子（0..=1）。
#[inline]
fn blink_factor(phase_ms: f64, duration_ms: f64) -> f64 {
    (phase_ms / duration_ms).clamp(0.0, 1.0)
}

/// 所有动作动画可能驱动的参数 ID。
const ACTION_PARAM_IDS: &[&str] = &[
    "ParamAngleX",
    "ParamAngleY",
    "ParamAngleZ",
    "ParamEyeBallX",
    "ParamEyeLOpen",
    "ParamEyeROpen",
    "ParamBrowLY",
    "ParamBrowRY",
    "ParamMouthOpenY",
];

/// 语义关键帧（照抄 py `choreography.ts`）。
/// - `at_ms`：相对动作开始的毫秒时刻；
/// - `fade_ms`：到达本帧值的缓动时长（ease-in 窗 `[at-fade, at]`）；
/// - `delta`：各参数相对 profile 默认的语义偏移 —— -1..1，0=默认位；
///   末帧空切片表示「释放回默认」（向 0 过渡）。
#[derive(Clone, Copy)]
pub(crate) struct Keyframe {
    pub at_ms: f32,
    pub fade_ms: f32,
    pub delta: &'static [(&'static str, f32)],
}

/// 关键帧编舞库：`(动作名, 关键帧序列)`，数值**逐条照抄** py 版
/// `choreography.ts` (MIT)。动作节奏（两拍 / 多段缓动）与旧实现的
/// 单周期正弦完全不同 —— 这是「体感很快」手感的直接修复。
const CHOREOGRAPHY: &[(&str, &[Keyframe])] = &[
    (
        "nod",
        &[
            Keyframe {
                at_ms: 0.0,
                fade_ms: 120.0,
                delta: &[("ParamAngleY", -0.55)],
            },
            Keyframe {
                at_ms: 240.0,
                fade_ms: 120.0,
                delta: &[("ParamAngleY", 0.45)],
            },
            Keyframe {
                at_ms: 500.0,
                fade_ms: 120.0,
                delta: &[("ParamAngleY", -0.50)],
            },
            Keyframe {
                at_ms: 760.0,
                fade_ms: 120.0,
                delta: &[("ParamAngleY", 0.40)],
            },
            Keyframe {
                at_ms: 1000.0,
                fade_ms: 320.0,
                delta: &[],
            },
        ],
    ),
    (
        "shake_no",
        &[
            Keyframe {
                at_ms: 0.0,
                fade_ms: 90.0,
                delta: &[("ParamAngleX", -0.55)],
            },
            Keyframe {
                at_ms: 180.0,
                fade_ms: 90.0,
                delta: &[("ParamAngleX", 0.55)],
            },
            Keyframe {
                at_ms: 360.0,
                fade_ms: 90.0,
                delta: &[("ParamAngleX", -0.50)],
            },
            Keyframe {
                at_ms: 540.0,
                fade_ms: 90.0,
                delta: &[("ParamAngleX", 0.50)],
            },
            Keyframe {
                at_ms: 700.0,
                fade_ms: 300.0,
                delta: &[],
            },
        ],
    ),
    (
        "tilt",
        &[
            Keyframe {
                at_ms: 0.0,
                fade_ms: 220.0,
                delta: &[("ParamAngleZ", -0.50)],
            },
            Keyframe {
                at_ms: 650.0,
                fade_ms: 220.0,
                delta: &[("ParamAngleZ", -0.40)],
            },
            Keyframe {
                at_ms: 1100.0,
                fade_ms: 400.0,
                delta: &[],
            },
        ],
    ),
    (
        "look_around",
        &[
            Keyframe {
                at_ms: 0.0,
                fade_ms: 140.0,
                delta: &[("ParamEyeBallX", -0.55)],
            },
            Keyframe {
                at_ms: 200.0,
                fade_ms: 260.0,
                delta: &[("ParamEyeBallX", -0.35), ("ParamAngleX", -0.50)],
            },
            Keyframe {
                at_ms: 750.0,
                fade_ms: 180.0,
                delta: &[("ParamEyeBallX", 0.60)],
            },
            Keyframe {
                at_ms: 950.0,
                fade_ms: 260.0,
                delta: &[("ParamEyeBallX", 0.40), ("ParamAngleX", 0.50)],
            },
            Keyframe {
                at_ms: 1500.0,
                fade_ms: 420.0,
                delta: &[],
            },
        ],
    ),
    (
        "listen",
        &[
            Keyframe {
                at_ms: 0.0,
                fade_ms: 300.0,
                delta: &[
                    ("ParamAngleZ", -0.16),
                    ("ParamAngleY", 0.12),
                    ("ParamEyeBallX", -0.22),
                    ("ParamBrowLY", 0.12),
                    ("ParamBrowRY", 0.12),
                ],
            },
            Keyframe {
                at_ms: 900.0,
                fade_ms: 300.0,
                delta: &[
                    ("ParamAngleZ", -0.14),
                    ("ParamAngleY", 0.10),
                    ("ParamEyeBallX", -0.18),
                    ("ParamBrowLY", 0.10),
                    ("ParamBrowRY", 0.10),
                ],
            },
            Keyframe {
                at_ms: 1700.0,
                fade_ms: 480.0,
                delta: &[],
            },
        ],
    ),
    (
        "surprise",
        &[
            Keyframe {
                at_ms: 0.0,
                fade_ms: 120.0,
                delta: &[
                    ("ParamEyeLOpen", 0.85),
                    ("ParamEyeROpen", 0.85),
                    ("ParamBrowLY", 0.80),
                    ("ParamBrowRY", 0.80),
                    ("ParamMouthOpenY", 0.90),
                    ("ParamAngleY", 0.20),
                ],
            },
            Keyframe {
                at_ms: 500.0,
                fade_ms: 120.0,
                delta: &[
                    ("ParamEyeLOpen", 0.60),
                    ("ParamEyeROpen", 0.60),
                    ("ParamBrowLY", 0.60),
                    ("ParamBrowRY", 0.60),
                    ("ParamMouthOpenY", 0.70),
                    ("ParamAngleY", 0.15),
                ],
            },
            Keyframe {
                at_ms: 900.0,
                fade_ms: 400.0,
                delta: &[],
            },
        ],
    ),
];

/// `const` 可用的字节相等比较（stable `PartialEq` for `&[u8]` 非 const）。
const fn str_eq(a: &str, b: &str) -> bool {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    if ab.len() != bb.len() {
        return false;
    }
    let mut i = 0;
    while i < ab.len() {
        if ab[i] != bb[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// 动作总时长（含末帧尾 fade），用于 `ActiveAction.duration_ms` 建立时查表。
/// 数值 = 末关键帧 `at + fade`（照抄 py 尾 fade）。
pub(crate) const fn choreography_total_ms(name: &str) -> f32 {
    let mut i = 0;
    while i < CHOREOGRAPHY.len() {
        if str_eq(CHOREOGRAPHY[i].0, name) {
            let frames = CHOREOGRAPHY[i].1;
            let last = frames[frames.len() - 1];
            return last.at_ms + last.fade_ms;
        }
        i += 1;
    }
    // 未知动作：退而使用旧的兜底时长。
    1500.0
}

/// 在一个关键帧区间内，将 `prev` 与 `cur` 的 delta 逐参 ease-in 插值。
/// `s` 为 smoothstep 因子（0..=1）；结果乘 `mult` 后作为 offset 写入
/// final_override 层（override_parameter 用法不变）。
fn blend(
    prev: &[(&'static str, f32)],
    cur: &[(&'static str, f32)],
    s: f32,
    mult: f32,
) -> Vec<(&'static str, f32)> {
    let one_minus = 1.0 - s;
    // prev 与 cur 可能有不同参集，union 去重（id 在任一出现）。
    let mut out: Vec<(&'static str, f32)> = Vec::new();
    for (id, p) in prev {
        let c = cur
            .iter()
            .find(|(cid, _)| *cid == *id)
            .map(|(_, v)| *v)
            .unwrap_or(0.0);
        let value = (p * one_minus + c * s) * mult;
        out.push((*id, value));
    }
    for (id, c) in cur {
        if prev.iter().any(|(pid, _)| *pid == *id) {
            continue;
        }
        let p = 0.0_f32;
        let value = (p * one_minus + c * s) * mult;
        out.push((*id, value));
    }
    out
}

/// 纯函数：给定动作名 + 已过毫秒 `elapsed_ms` + 强度倍率 `strength_mult`，
/// 返回该时刻所有应写入 final_override 的 `(id, value)` 列表。
///
/// 抽出为纯函数以便 `#[cfg(test)]` 在 native target 下无 web_sys 依赖即可验证。
/// 与 `apply_bridge_effects` 中实时计算保持一致（单一真实来源）。
///
/// 区间语义（照抄 py `choreography.ts`）：
/// - 确定 `elapsed_ms` 落在 `[prev.at, cur.at)` 区间；
/// - 缓动窗 `[cur.at - cur.fade, cur.at]` 内 smoothstep 过渡到 cur 帧值；
/// - 末帧 delta 为空 = 释放回 0（过渡到全部归零）；超过末帧 at 后保持 0。
pub(crate) fn action_frames(
    name: &str,
    elapsed_ms: f32,
    strength_mult: f32,
) -> Vec<(&'static str, f32)> {
    // 找动作。
    let frames = match CHOREOGRAPHY.iter().find(|(n, _)| str_eq(n, name)) {
        Some((_, f)) => *f,
        None => return Vec::new(),
    };
    if frames.is_empty() {
        return Vec::new();
    }
    // 未到第一个关键帧时刻（含起始）：钳到首帧自身。
    if elapsed_ms <= frames[0].at_ms {
        return blend(frames[0].delta, frames[0].delta, 0.0, strength_mult);
    }
    let n = frames.len();
    // 推进 cur 至第一个 at > elapsed 的关键帧（prev = cur-1）。
    let mut cur = 1usize;
    while cur < n && elapsed_ms >= frames[cur].at_ms {
        cur += 1;
    }
    // elapsed 达到末帧 at 及其后 → release 完成，全部归零（向 0 过渡完毕）。
    // 末帧 fade_ms 用作 release 缓动窗（见上），at+fade 即总时长；
    // 此后持续 0 直到 duration_ms 清除 action。
    if cur >= n {
        let mut out: Vec<(&'static str, f32)> = Vec::new();
        let mut seen: Vec<&'static str> = Vec::new();
        // 统计除末帧外所有关键帧中出现的参（末帧 delta 空 = release），
        // 逐一写 0 归零，避免 stale override 卡死。
        for frame in &frames[..n - 1] {
            for (id, _) in frame.delta {
                if !seen.contains(id) {
                    seen.push(id);
                    out.push((*id, 0.0));
                }
            }
        }
        return out;
    }
    let prev = &frames[cur - 1];
    let cur_frame = &frames[cur];
    // ease-in 窗：从 (cur.at - fade) 到 cur.at。
    let fade_start = cur_frame.at_ms - cur_frame.fade_ms;
    let t_factor = if cur_frame.fade_ms > 0.0 {
        ((elapsed_ms - fade_start) / cur_frame.fade_ms).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let s = t_factor * t_factor * (3.0 - 2.0 * t_factor);
    blend(prev.delta, cur_frame.delta, s, strength_mult)
}

/// P0-2a-3：每帧把 bridge 状态应用为可见效果（口型/动作/缩放/背景）。
/// 由 `tick` 在 `core.update` 后、渲染前调用。
///
/// `dt_millis`：真实帧间隔（毫秒），用于 dt 归一化的口型衰减，避免高帧率
///（如 240fps）下口型/动作"动得飞快"。
pub(crate) fn apply_bridge_effects(state: &SharedState, dt_millis: f64) {
    let now_ms = web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0);
    // 先 immutable borrow：dt 归一化口型衰减、判定动画是否结束、取出 clone 值。
    let (apply_mouth, apply_action, scale, offset_x, offset_y, _dark) = {
        let mut st = state.borrow_mut();
        // 口型衰减（dt 归一化）：volume_display *= exp(-dt_ms / TAU_MS)
        // 再取 max(volume, ...) 保持峰值不掉落。
        // 该公式与帧率无关：60fps 与 240fps 衰减速率一致
        //   60fps:  exp(-16.67/160) ≈ 0.902
        //   240fps: exp(-4.17/160) ≈ 0.974
        // 二者单帧衰减不同，但*速率*相同——这是正确的物理行为。
        let decay = (-dt_millis / TAU_MS).exp();
        st.bridge.volume_display = st
            .bridge
            .volume
            .max(st.bridge.volume_display * decay as f32);
        // 动画结束判定：t>=1 清除 Action（end 帧也会写入 None，这里兜底）。
        if let Some(a) = &st.bridge.action
            && now_ms - a.started_ms >= a.duration_ms
        {
            st.bridge.action = None;
        }
        (
            st.bridge.lip_sync.then(|| st.bridge.volume_display),
            st.bridge.action.clone(),
            st.bridge.scale,
            st.bridge.offset_x,
            st.bridge.offset_y,
            st.bridge.dark,
        )
    };

    let mut st = state.borrow_mut();

    // 口型（lip_sync on 且 volume>0.01）：ParamMouthOpenY；静音写 0 收口。
    let mouth = apply_mouth.filter(|v| *v > 0.01).unwrap_or(0.0);
    let _ = st.core.set_parameter("ParamMouthOpenY", mouth);

    // 动作参数等效动画（override 层最高优先级）。
    //
    // 层级选择依据：PoseStack finalize 合成顺序为
    //   base ← idle ← input ← physics ← final_override
    // final_override 层在物理输出之后再应用，故物理引擎 **无法** 清除 / 覆盖
    // 动作参数 —— `override_parameter` 写在最高优先级层，确保动作始终可见。
    //
    // 旧实现仅在 `if let Some(a) = &apply_action` 分支写入 override 值，但在
    // action 结束（action 变为 None）时从不清除，导致 stale override 值永久
    // 卡死在 final_override 层 —— 表现为「动作结束后模型卡在最后一帧姿态」。
    // 修复：action 不活跃时主动 `clear_override_parameter`，让模型归还
    // idle/physics/lip_sync 的自然状态。
    if let Some(a) = &apply_action {
        let params = action_frames(&a.name, (now_ms - a.started_ms) as f32, a.strength_mult);
        for (id, value) in &params {
            let _ = st.core.override_parameter(id, *value);
        }
    } else {
        // action 未激活：清除所有可能由 action 写入的 override 参数，
        // 防止上一轮 action 的 stale 值卡死在 final_override 层。
        // ParamMouthOpenY 在 input 层由 lip_sync 写入，此处仅清 override，
        // 不影响收入同步。
        for id in ACTION_PARAM_IDS {
            let _ = st.core.clear_override_parameter(id);
        }
    }

    // 缩放 + 平移（模型级逻辑变换，背景不受影响）。
    //
    // 方案 A（l2d `set_transform(Affine2)`）：
    // `ModelRendererCore::set_transform` 直接设 `self.transform`（模型逻辑变换），
    // 渲染时 `aspect_fit_transform(viewport) * self.transform` 叠加 —— 这已是
    // layout_transform 初始值的基础上进行额外变换，背景 clear color 与 canvas
    // 尺寸完全不受影响，实现"仅模型缩放"。
    //
    // 关键：`set_transform` 是赋值（替换整个 transform 字段），不是累乘。
    // 因此 tick 每帧都必须在 `last_layout_transform`（load_model 后一次性捕获
    // 的 layout 初始值）基础上右乘用户缩放/平移，才能保持 layout 构图
    // 不变，仅叠加用户交互。
    //
    // offset_x/offset_y 存储的 CSS 像素单位，直接作为 NDC 平移分量
    // （bridge comment 声明"已换算为 NDC"），scale 作用于模型自身以
    //   画布中心为锚。
    let base = st.last_layout_transform;
    let user_transform = Affine2::from_translation(Vec2::new(offset_x, offset_y))
        * Affine2::from_scale(Vec2::splat(scale));
    st.core.set_transform(base * user_transform);

    // 背景色：仅 canvas CSS background-color，与模型 viewport 清空色
    // (wgpu::Color::TRANSPARENT) 独立，缩放不影响背景。
    //
    // 仅在值变化时写 DOM。`apply_bridge_effects` 每帧调用（rAF 循环），
    // 而 `dark` 只在 stage-config 消息时变化——每次都 set_property 会触发
    // style recalc（240Hz = 秒 240 次 DOM 写），是用户实测 "FPS 高但卡"
    // 的根因。`applied_dark` 记录"已写到 DOM 的值"，相等时跳过写。
    //
    // `applied_dark: false` 初始值：index.html 的 canvas CSS 背景色可能
    // 与 #101418 不一致，首帧不写会闪烁——保守地强制首帧写一次同步。
    if st.bridge.dark != st.bridge.applied_dark {
        let bg = if st.bridge.dark { "#101418" } else { "#e8ecf1" };
        let _ = st.canvas.style().set_property("background-color", bg);
        st.bridge.applied_dark = st.bridge.dark;
    }

    // RM6 待机生命体征层（呼吸/眨眼/微表情）——写入 input 层，优先级低于
    // 动作 final_override 层（动作做 nod 时头部角度盖过 idle 微表情）。
    //
    // 叠加语义（v1 简化，见 IdleState 注释）：
    // - 动作 override 层盖住同名参数 → 做 surprise 时 EyeLOpen/ROpen 被覆盖，
    //   动作结束后 override 清除 → idle blink 自动恢复（input 层持续写入）。
    // - breath/blink 参数（ParamBreath / EyeL/R）动作不写入 → 天然不打架。
    // - py 版会在动作播放时**挂起** idle，本版不挂起 —— 纯参数层面天然互斥。
    //
    // dt 来源：`apply_bridge_effects(state, dt_millis)` 已有 dt（R4 加的），
    // idle 采样用它，帧率无关。
    //
    // 呼吸永不打断（底层持续）；眨眼/微表情状态机用 dt_ms 推进。
    apply_idle_life(&mut st, now_ms, dt_millis);
}

/// RM6 待机生命体征采样：呼吸 / 眨眼 / 微表情，每帧调用一次。
///
/// 写入 input 层 `set_parameter`（优先级低于动作 final_override 层）。
/// 公式与 py 版 `idle-breath.ts` / `idle-blink.ts` / `idle-micro-expr.ts`（MIT）同构。
///
/// - **呼吸**：`0.5 + 0.15*sin(2π*(now-t0)/period)` → ParamBreath，永不打断。
/// - **眨眼**：状态机推进（dt 归一化）；Closing 线性 1→0（75ms）；
///   Opening 线性 0→1（随机 150-300ms）。写入 ParamEyeLOpen/ROpen。
/// - **微表情**：到触发时刻随机选参 ±0.05，400ms fade（进 1/3 → 停 1/3 → 出 1/3）。
///   `base` 为对应参数中性值（嘴形 0.5，眉/眼球 0.0）。
///
/// 与 py 版差异：py 版微表情触发时对**所有** MICRO_PARAMS 同时写入一个 batch；
/// 本版**随机选一个**参数做一次性小偏移（更自然、减轻写入），fade 仍 400ms。
/// 详见 IdleState 文档注释。
pub(crate) fn apply_idle_life(st: &mut FrameState, now_ms: f64, dt_ms: f64) {
    // ---- 呼吸：正弦波，永不打断 ----
    // value = 0.5 + 0.15 * sin(2π * (now - t0) / period) ∈ [0.35, 0.65]
    let breath = {
        let phase = (now_ms - st.idle.breath_t0_ms) / st.idle.breath_period_ms;
        0.5 + 0.15 * (2.0 * std::f64::consts::PI * phase).sin()
    };
    let _ = st.core.set_parameter("ParamBreath", breath as f32);

    // ---- 眨眼：状态机 ----
    // 推进当前相位计时；到时切换。Closing 线性 1→0；Opening 线性 0→1。
    st.idle.blink_phase_ms += dt_ms;
    match st.idle.blink_phase {
        BlinkPhase::Open => {
            // 睁眼待机：不写参数（保持 1.0），等待随机间隔。
            if now_ms >= st.idle.blink_next_ms {
                st.idle.blink_phase = BlinkPhase::Closing;
                st.idle.blink_phase_ms = 0.0;
                // 首帧写入 1.0（开始下降）。
                let _ = st.core.set_parameter("ParamEyeLOpen", 1.0);
                let _ = st.core.set_parameter("ParamEyeROpen", 1.0);
            }
        }
        BlinkPhase::Closing => {
            // 线性 1→0，固定 75ms。
            let f = (1.0 - blink_factor(st.idle.blink_phase_ms, st.idle.blink_close_ms)) as f32;
            let _ = st.core.set_parameter("ParamEyeLOpen", f);
            let _ = st.core.set_parameter("ParamEyeROpen", f);
            if st.idle.blink_phase_ms >= st.idle.blink_close_ms {
                // 闭眼到位 → 进入睁眼，随机 [150,300]ms。
                st.idle.blink_phase = BlinkPhase::Opening;
                st.idle.blink_phase_ms = 0.0;
                st.idle.blink_open_ms = rng_range_f64(&mut st.idle.rng_state, 150.0, 300.0);
            }
        }
        BlinkPhase::Opening => {
            // 线性 0→1，随机时长。
            let f = blink_factor(st.idle.blink_phase_ms, st.idle.blink_open_ms) as f32;
            let _ = st.core.set_parameter("ParamEyeLOpen", f);
            let _ = st.core.set_parameter("ParamEyeROpen", f);
            if st.idle.blink_phase_ms >= st.idle.blink_open_ms {
                // 睁眼完成 → 回到 Open，单次写回 1.0。
                let _ = st.core.set_parameter("ParamEyeLOpen", 1.0);
                let _ = st.core.set_parameter("ParamEyeROpen", 1.0);
                st.idle.blink_phase = BlinkPhase::Open;
                st.idle.blink_phase_ms = 0.0;
                // 下一次间隔：[3000, 7500]ms。
                st.idle.blink_next_ms =
                    now_ms + rng_range_f64(&mut st.idle.rng_state, 3000.0, 7500.0);
            }
        }
    }

    // ---- 微表情：间隔触发，随机参数 ±0.05，400ms fade ----
    // fade 三段：推进 1/3（0→delta）→ 停 1/3（持 delta）→ 缩退 1/3（delta→0）。
    // end_t = start_ms + 400ms；过时清除（恢复 base）。
    const MICRO_FADE_MS: f64 = 400.0;
    let micro_done = if let Some((param, delta, start_ms)) = &st.idle.micro_active {
        let elapsed = now_ms - *start_ms;
        if elapsed >= MICRO_FADE_MS {
            // 结束：清参数回 base（input 层 set base 值），清活跃。
            let base = micro_param_base(param);
            let _ = st.core.set_parameter(param, base as f32);
            st.idle.micro_active = None;
            // 排下一次：[3000, 6000]ms。
            st.idle.micro_next_ms = now_ms + rng_range_f64(&mut st.idle.rng_state, 3000.0, 6000.0);
            true
        } else if elapsed >= MICRO_FADE_MS * 2.0 / 3.0 {
            // 阶段 3：缩退 delta→0，线性。
            let s = (1.0 - (elapsed - MICRO_FADE_MS * 2.0 / 3.0) / (MICRO_FADE_MS / 3.0))
                .clamp(0.0, 1.0);
            let v = (micro_param_base(param) + *delta as f64 * s) as f32;
            let _ = st.core.set_parameter(param, v);
            true
        } else if elapsed >= MICRO_FADE_MS / 3.0 {
            // 阶段 2：停（持 delta 峰值）。
            let v = (micro_param_base(param) + *delta as f64) as f32;
            let _ = st.core.set_parameter(param, v);
            false
        } else {
            // 阶段 1：推进 0→delta。
            let s = (elapsed / (MICRO_FADE_MS / 3.0)).clamp(0.0, 1.0);
            let v = (micro_param_base(param) + *delta as f64 * s) as f32;
            let _ = st.core.set_parameter(param, v);
            false
        }
    } else {
        false
    };
    // 只有活跃微表情已结束（或从未启动），才检查是否触发新的。
    if !micro_done && now_ms >= st.idle.micro_next_ms {
        // 随机选一个参数 + 随机 ±0.05 delta。
        let params = [
            "ParamBrowLY",
            "ParamBrowRY",
            "ParamMouthForm",
            "ParamEyeBallX",
            "ParamEyeBallY",
        ];
        let idx = (rng_next(&mut st.idle.rng_state) % params.len() as u64) as usize;
        let param = params[idx].to_string();
        let delta = (rng_range_f64(&mut st.idle.rng_state, 0.0, 1.0) - 0.5) * 0.1; // ±0.05
        let start_ms = now_ms;
        // 立即写初始值（base + 0），fade-in 逐渐推进。
        let _ = st
            .core
            .set_parameter(&param, micro_param_base(&param) as f32);
        st.idle.micro_active = Some((param, delta as f32, start_ms));
    }
}

/// 微表情参数的中性基线值（对应 M3 参数表 neutral）。
/// 嘴形 0.5，眉毛/眼球 0.0。
fn micro_param_base(param: &str) -> f64 {
    match param {
        "ParamMouthForm" => 0.5,
        _ => 0.0,
    }
}

/// 渲染一帧并 present。返回 `false` 表示致命错误、应停止循环。
///
/// 实时路径（wasm32 rAF）只调用 `render_to_view_submit`，**不得**调用阻塞的
/// `render_to_view`（C1/C6/C8 硬门禁「实时调用图不得出现 PollType::Wait」）。
/// 返回的 `SubmissionIndex` 在交换链路径下无须等待——浏览器自身的 `present()`
/// 负责把提交与呈现串行化。
pub(crate) fn render_and_present(st: &mut FrameState, frame: wgpu::SurfaceTexture) -> bool {
    let view = frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    // CPU frame time：只统计提交命令缓冲所用时，排除了取 texture / present
    // 本身（后者由浏览器在合成线程完成）。这里衡量的是 JS↔GPU 命令桥开销，
    // 也就是骨骼 IK + draw 提交的本地开销——最能反馈渲染核心热点的指标。
    let t0 = performance_now();
    match st.core.render_to_view_submit(&view, st.config.format) {
        Ok(submission) => {
            let _ = submission; // 可选：记录 last_submission 供错误状态栏
            frame.present();
            let cpu_ms = performance_now() - t0;
            st.hud.record_frame(cpu_ms);
            true
        }
        Err(e) => {
            // 统计失败不计入 FPS：本帧未真正渲染出像素。
            status(&format!("停止渲染循环：render_to_view_submit 失败：{e}"));
            false
        }
    }
}

/// `performance.now()` 封装（wasm 单线程，不存在时回 0）。
fn performance_now() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0)
}

/// 每 500ms 刷新一次运行时 HUD（adapter / canvas / dpr / FPS / cpu ms）。
/// 由 `tick` 在错峰内调用，避免每帧刷 DOM。
pub(crate) fn write_hud_if_due(state: &SharedState) {
    // 先 immutable borrow 读数据；错峰判定与 DOM 写完成后 drop，再 mut borrow 重置窗。
    let (due, line) = {
        let now_hud = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0);
        let st = state.borrow();
        if !st.hud.due() {
            return;
        }
        // canvas CSS 尺寸 × DPR cap = 物理尺寸。
        let css_w = st.canvas.client_width().max(0);
        let css_h = st.canvas.client_height().max(0);
        let dpr = st.window.device_pixel_ratio().clamp(1.0, DPR_CAP);
        let (phys_w, phys_h) = (st.config.width, st.config.height);
        let adapter_line = st
            .hud
            .adapter_line
            .clone()
            .unwrap_or_else(|| "<adapter unknown>".to_owned());
        // 本窗 CPU 均值 ms / 帧。
        let cpu_ms = if st.hud.frame_count > 0 {
            st.hud.cpu_time_sum / st.hud.frame_count as f64
        } else {
            0.0
        };
        let stage_diag = format!(
            "stage: scale={:.2} off=({:+.2},{:+.2}) msg={}/{} [{}]",
            st.bridge.scale,
            st.bridge.offset_x,
            st.bridge.offset_y,
            st.bridge.msg_applied,
            st.bridge.msg_recv,
            st.bridge.last_msg
        );
        // RM6 待机生命体征快照：呼吸正弦值 + 眨眼相位回路。
        // breath 复算自正弦公式；blink 显示当前相位 + 相位内推进。
        let breath = {
            let phase = (now_hud - st.idle.breath_t0_ms) / st.idle.breath_period_ms;
            0.5 + 0.15 * (2.0 * std::f64::consts::PI * phase).sin()
        };
        let blink_phase_str = match st.idle.blink_phase {
            BlinkPhase::Open => "O",
            BlinkPhase::Closing => "C",
            BlinkPhase::Opening => "P",
        };
        let idle_diag = format!(
            "idle: b{breath:.2} bl{blink_phase_str}{:.0}",
            st.idle.blink_phase_ms
        );
        (
            true,
            format!(
                "GPU: {adapter_line} | canvas {css_w}x{css_h}@{dpr}dpr(物理{phys_w}x{phys_h}) | FPS {fps:.1} | cpu {cpu:.1}ms | sim {sim}/frame | {stage_diag} | {idle_diag}",
                adapter_line = adapter_line,
                css_w = css_w,
                css_h = css_h,
                dpr = dpr,
                phys_w = phys_w,
                phys_h = phys_h,
                fps = st.hud.last_fps,
                cpu = cpu_ms,
                sim = st.hud.last_sim_steps,
                idle_diag = idle_diag,
            ),
        )
    };
    if due {
        super::status(&line);
        // 重置统计窗，准备下一轮 500ms 累计。
        state.borrow_mut().hud.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// nod 在 elapsed=0 时 ParamAngleY 应等于 -0.55 × 倍率（首帧持值）。
    #[test]
    fn nod_start_value() {
        let params = action_frames("nod", 0.0, 1.0);
        assert_eq!(params.len(), 1, "nod 应驱动恰好一个参数");
        assert_eq!(params[0].0, "ParamAngleY");
        assert!(
            (params[0].1 - (-0.55)).abs() < 1e-5,
            "t=0 时 nod 应为 -0.55"
        );
        let weak = action_frames("nod", 0.0, 0.6);
        assert!(
            (weak[0].1 - (-0.55 * 0.6)).abs() < 1e-5,
            "强度 0.6 应缩放值"
        );
    }

    /// nod 在关键帧交界处（elapsed=240ms）应达到 +0.45 × 倍率。
    /// 区间 [0,240): prev=帧0(-0.55) cur=帧1(+0.45, fade120)；
    /// 于 240ms 进入下一区间 [240,500)：prev=帧1(+0.45) cur=帧2(-0.50)
    /// fade_start=500-120=380 → 240<380 → s=0 → 取 prev +0.45。
    #[test]
    fn nod_at_keyframe_boundary() {
        let params = action_frames("nod", 240.0, 1.0);
        assert!(
            (params[0].1 - 0.45).abs() < 1e-5,
            "elapsed=240 时 nod 应为 +0.45"
        );
    }

    /// nod 在总时长末尾（1320ms）应全部归零。
    #[test]
    fn nod_total_zero_at_end() {
        let params = action_frames("nod", 1320.0, 1.0);
        for (_, v) in &params {
            assert!(v.abs() < 1e-5, "nod 总末应归零，实际 {v}");
        }
    }

    /// nod 在末帧 at 时（1000ms）应已趋近归零（尾 fade）。
    #[test]
    fn nod_tail_fades_to_zero() {
        let params = action_frames("nod", 1000.0, 1.0);
        let v = params
            .iter()
            .find(|(id, _)| *id == "ParamAngleY")
            .map(|(_, v)| *v);
        assert!(v.is_some(), "nod 在末帧区间仍在驱动 ParamAngleY");
        assert!(v.unwrap().abs() < 1e-4, "nod 末帧 at 时应趋近 0");
    }

    /// 末帧空 delta 区间的 release 语义：各动作在总时长时归零。
    #[test]
    fn action_zero_at_total_duration() {
        for (name, total) in [
            ("nod", 1320.0),
            ("shake_no", 1000.0),
            ("tilt", 1500.0),
            ("look_around", 1920.0),
            ("listen", 2180.0),
            ("surprise", 1300.0),
        ] {
            let params = action_frames(name, total, 1.0);
            for (_, v) in &params {
                assert!(
                    v.abs() < 1e-5,
                    "{name} 总时长 {}ms 时值应为 0，实际 {v}",
                    total
                );
            }
        }
    }

    /// 未知动作名返回空参数列表。
    #[test]
    fn unknown_action_returns_empty() {
        assert!(action_frames("unknown", 500.0, 1.0).is_empty());
    }

    /// 强度倍率 m 正比缩放所有参数值。
    #[test]
    fn strength_scales_values() {
        let weak = action_frames("nod", 0.0, 0.6);
        let strong = action_frames("nod", 0.0, 1.35);
        assert!((weak[0].1 - (-0.55 * 0.6)).abs() < 1e-5);
        assert!((strong[0].1 - (-0.55 * 1.35)).abs() < 1e-5);
    }

    /// choreoraphy_total_ms 产出正确的末帧时长（at + fade）。
    #[test]
    fn duration_table_correct() {
        assert!((choreography_total_ms("nod") - 1320.0).abs() < 1e-3);
        assert!((choreography_total_ms("shake_no") - 1000.0).abs() < 1e-3);
        assert!((choreography_total_ms("tilt") - 1500.0).abs() < 1e-3);
        assert!((choreography_total_ms("look_around") - 1920.0).abs() < 1e-3);
        assert!((choreography_total_ms("listen") - 2180.0).abs() < 1e-3);
        assert!((choreography_total_ms("surprise") - 1300.0).abs() < 1e-3);
        assert!((choreography_total_ms("bogus") - 1500.0).abs() < 1e-3);
    }

    /// ACTION_PARAM_IDS 覆盖所有动作可能写入的参数。
    #[test]
    fn action_param_ids_covers_all_actions() {
        use std::collections::HashSet;
        let set: HashSet<&str> = ACTION_PARAM_IDS.iter().copied().collect();
        for name in [
            "nod",
            "shake_no",
            "tilt",
            "look_around",
            "listen",
            "surprise",
        ] {
            let params = action_frames(name, 500.0, 1.0);
            for (id, _) in &params {
                assert!(
                    set.contains(id),
                    "{name} 写入的参数 {id} 不在 ACTION_PARAM_IDS 中"
                );
            }
        }
    }
}
