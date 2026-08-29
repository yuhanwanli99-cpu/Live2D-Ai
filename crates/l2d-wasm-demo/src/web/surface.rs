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
            power_preference: wgpu::PowerPreference::LowPower,
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

/// CSS 像素 × devicePixelRatio = 期望的物理画布尺寸（至少 1×1）。
pub(crate) fn canvas_pixel_size(canvas: &HtmlCanvasElement, window: &Window) -> (u32, u32) {
    let dpr = window.device_pixel_ratio().max(1.0);
    let css_w = canvas.client_width().max(0) as f64;
    let css_h = canvas.client_height().max(0) as f64;
    let width = ((css_w * dpr).round() as u32).max(1);
    let height = ((css_h * dpr).round() as u32).max(1);
    (width, height)
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
    let mut st = state.borrow_mut();

    if let Err(e) = st.core.update(FIXED_DT_60HZ) {
        status(&format!("停止渲染循环：update({FIXED_DT_60HZ}) 失败：{e}"));
        return;
    }

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
        crate::web::schedule_frame(slot);
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
    match st.core.render_to_view_submit(&view, st.config.format) {
        Ok(submission) => {
            let _ = submission; // 可选：记录 last_submission 供错误状态栏
            frame.present();
            true
        }
        Err(e) => {
            status(&format!("停止渲染循环：render_to_view_submit 失败：{e}"));
            false
        }
    }
}
