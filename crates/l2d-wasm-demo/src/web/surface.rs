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

use l2d::renderer::{FIXED_DT_60HZ, GpuContext, ModelRendererCore};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{HtmlCanvasElement, Window};

use super::status;
use crate::web::net::js_str;

/// HUD 统计窗口长度（ms）。500ms 刷新一次 DOM，远低于每帧刷屏成本；
/// 同时能覆盖足够多的帧样本估出稳定 FPS。
const HUD_INTERVAL_MS: f64 = 500.0;

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
    /// stage-config.lipSync（口型开关，默认 true）
    pub lip_sync: bool,
    /// 最近 audio-volume（0..1；每 20ms 音频帧更新）
    pub volume: f32,
    /// 衰减后的口型显示值（rAF 每帧 volume_display = volume.max(volume_display*0.85)）
    pub volume_display: f32,
    /// 进行中的参数等效动画（action-state start 创建 / end 或 t>=1 结束）
    pub action: Option<ActiveAction>,
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
    /// 动画时长（默认 800ms；end 帧提前结束）
    pub duration_ms: f64,
}

impl Default for BridgeState {
    fn default() -> Self {
        Self {
            scale: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            dark: true,
            lip_sync: true,
            volume: 0.0,
            volume_display: 0.0,
            action: None,
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
pub(crate) fn tick(slot: &FrameSlot, state: &SharedState) {
    // P0-2a-3：固定 dt 步进姿态/物理 → 每帧应用 bridge 状态 → 取 texture → render/present。
    {
        let mut st = state.borrow_mut();
        if let Err(e) = st.core.update(FIXED_DT_60HZ) {
            status(&format!("停止渲染循环：update({FIXED_DT_60HZ}) 失败：{e}"));
            return;
        }
    }

    // bridge 状态 → 可见效果（口型/动作/缩放/背景），独立借用窗口。
    apply_bridge_effects(state);

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

/// P0-2a-3：每帧把 bridge 状态应用为可见效果（口型/动作/缩放/背景）。
/// 由 `tick` 在 `core.update` 后、渲染前调用。
pub(crate) fn apply_bridge_effects(state: &SharedState) {
    let now_ms = web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0);
    // 先 immutable borrow：衰减音量显示、判定动画是否结束、取出 clone 值。
    let (apply_mouth, apply_action, scale, dark) = {
        let mut st = state.borrow_mut();
        // 口型衰减：display = volume.max(display*0.85)
        st.bridge.volume_display = st.bridge.volume.max(st.bridge.volume_display * 0.85);
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
            st.bridge.dark,
        )
    };
    // 后 mutable borrow：写入 core 参数 + canvas CSS。
    let mut st = state.borrow_mut();

    // 口型（lip_sync on 且 volume>0.01）：ParamMouthOpenY；静音写 0 收口。
    let mouth = apply_mouth.filter(|v| *v > 0.01).unwrap_or(0.0);
    let _ = st.core.set_parameter("ParamMouthOpenY", mouth);

    // 动作参数等效动画（override 层最高优先级）。
    if let Some(a) = &apply_action {
        let t = ((now_ms - a.started_ms) / a.duration_ms).clamp(0.0, 1.0) as f32;
        let m = a.strength_mult;
        let wave = (t * std::f32::consts::PI * 2.0).sin();
        match a.name.as_str() {
            "nod" => {
                let _ = st.core.override_parameter("ParamAngleX", wave * 12.0 * m);
            }
            "shake_no" => {
                let _ = st.core.override_parameter("ParamAngleY", wave * 14.0 * m);
            }
            "tilt" => {
                let _ = st
                    .core
                    .override_parameter("ParamAngleZ", (t * std::f32::consts::PI).sin() * 10.0 * m);
            }
            "look_around" => {
                let _ = st.core.override_parameter(
                    "ParamAngleX",
                    (t * 3.0 * std::f32::consts::PI).sin() * 10.0 * m,
                );
            }
            "listen" => {
                let _ = st
                    .core
                    .override_parameter("ParamAngleZ", 8.0 * m * (1.0 - t));
            }
            "surprise" => {
                let _ = st.core.override_parameter(
                    "ParamMouthOpenY",
                    (t * std::f32::consts::PI).sin() * 0.8 * m,
                );
            }
            _ => {}
        }
    }

    // 缩放 + 平移（CSS 层，避 GPU 重分配）。
    //
    // 方案说明：`ModelRendererCore` 公开 `set_transform(Affine2)`，但 `Affine2`
    // 来自 `glam`，而 `l2d-wasm-demo` 未把 `glam` 列为直接依赖（且本次任务仅允
    // 许改动 `main.rs`/`surface.rs`，不能改 `Cargo.toml`），因此在 `surface.rs`
    // 里无法构造 `Affine2` 值来写入逻辑变换 → **放弃方案 A（l2d transform API）**。
    // 走 **方案 C**：在 canvas CSS 层做缩放与平移。
    //
    // 可视性问题：原实现单靠 `transform: scale()` 视觉无效，因为 canvas CSS 尺寸
    // 钉在 `100%`（index.html），缩放后 overflow 的内容被裁剪。修法：
    //  1) 赋予 canvas 显式的 CSS 宽/高 = 基准宽高 × scale，使「缩放后的盒子」
    //     真实变动（而非 100%）→ 放大/缩小立即可见；
    //  2) `position: absolute` 脱离文档流，`left/top` 用 `50% - 盒子一半 + offset`
    //     居中并平移（50% 相对于存放盒 #stage，非 canvas 自身）。
    // 背景色沿用原语义。
    let style = st.canvas.style();
    // 基准 CSS 宽高 = 绘制缓冲的物理宽高除以 DPR cap（与 canvas_pixel_size 的
    // 逆运算），二者乘 scale 即为放缩后的显示尺寸。
    let dpr = st.window.device_pixel_ratio().clamp(1.0, DPR_CAP) as f32;
    let base_w_css = (st.config.width as f32) / dpr;
    let base_h_css = (st.config.height as f32) / dpr;
    let css_w = base_w_css * scale;
    let css_h = base_h_css * scale;
    // scale 与 pan 统一写一组内联样式（覆盖 index.html 的 width:100%/height:100%）。
    let _ = style.set_property("position", "absolute");
    // 居中 + 平移：left/top 用 50% - 盒子一半 + offset，50% 相对于存放
    // 盒（#stage）而非 canvas 自身，避免与缩放后的自身尺寸套嵌。
    // offset 单位 = CSS 像素。
    let ox = st.bridge.offset_x;
    let oy = st.bridge.offset_y;
    let _ = style.set_property("width", &format!("{css_w:.3}px"));
    let _ = style.set_property("height", &format!("{css_h:.3}px"));
    let _ = style.set_property(
        "left",
        &format!(
            "calc(50% - {half_w:.3}px + {ox}px)",
            half_w = css_w * 0.5,
            ox = ox
        ),
    );
    let _ = style.set_property(
        "top",
        &format!(
            "calc(50% - {half_h:.3}px + {oy}px)",
            half_h = css_h * 0.5,
            oy = oy
        ),
    );
    let bg = if dark { "#101418" } else { "#e8ecf1" };
    let _ = style.set_property("background-color", bg);
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
        (
            true,
            format!(
                "GPU: {adapter_line} | canvas {css_w}x{css_h}@{dpr}dpr(物理{phys_w}x{phys_h}) | FPS {fps:.1} | cpu {cpu:.1}ms",
                adapter_line = adapter_line,
                css_w = css_w,
                css_h = css_h,
                dpr = dpr,
                phys_w = phys_w,
                phys_h = phys_h,
                fps = st.hud.last_fps,
                cpu = cpu_ms,
            ),
        )
    };
    if due {
        super::status(&line);
        // 重置统计窗，准备下一轮 500ms 累计。
        state.borrow_mut().hud.flush();
    }
}
