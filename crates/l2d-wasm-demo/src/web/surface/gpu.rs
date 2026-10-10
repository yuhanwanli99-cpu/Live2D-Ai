//! 渲染面的 GPU / surface 协商（仅 `target_arch = "wasm32"` 下编译）。
//!
//! - [`init_gpu`]：WebGPU 优先、WebGL2 回退；显式设置
//!   `desired_maximum_frame_latency = 2`（C3）。
//!
//! **2026-09-14 rc.5 记录**：曾把这里改成「WebGL2 优先」以换取透明 canvas，
//! 但 HUD 实测用户机仍走 WebGPU（`request_gl` 未生效），且透明 canvas 在
//! WebGPU 下结构性不可达。舞台底色与背景图现在**画进 framebuffer**
//! （见 `background.rs`），因此不再需要动后端顺序——恢复 WebGPU 优先。
//! - [`backend_label`] / [`adapter_info`]：实际后端与 adapter 信息（HUD 观测用）。
//! - [`canvas_pixel_size`]：把 CSS 显示盒换算成物理画布尺寸——**最长边 =
//!   档位边长**（按 CSS 宽高比保比，不撑成正方形），档位为 0 时退回
//!   「CSS × min(dpr, DPR_CAP)」。
//! - [`negotiate_surface`]：申请并配置 surface，按 16384→8192→4096→CSS×DPR
//!   逐级回落（申请 `get_default_config` 返回 `None` 就往下试）；全部失败返回
//!   `None`，调用方保留原 config，舞台照常。

use std::sync::Arc;

use l2d::renderer::{GpuContext, RenderTier};
use web_sys::{HtmlCanvasElement, Window};

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
        u32,
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
    // adapter 汇报的纹理最长边上限（通常 8192；独显/新版可到 16384）。
    // 档位 16384 只有在把它抬进 device limits 后才有机会申请到。
    let adapter_max_dim = adapter.limits().max_texture_dimension_2d;
    let limits = wgpu::Limits {
        max_texture_dimension_2d: adapter_max_dim.min(16384),
        ..wgpu::Limits::default()
    };
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("l2d-wasm-demo"),
            required_limits: limits,
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

    // **画布尺寸协商（2026-10-10）**：档位 = 画布**最长边**。默认档位（4096）
    // 按 adapter 上限回落；申请不到再逐级降档 → CSS×DPR 兜底，**绝不让舞台打不开**。
    let config = negotiate_surface(
        &surface,
        &adapter,
        gpu.device(),
        canvas,
        window,
        RenderTier::DEFAULT,
        adapter_max_dim,
        format,
        alpha_mode,
        None,
    )
    .map(|(config, _)| config)
    .ok_or_else(|| "无法协商 surface 配置".to_owned())?;
    Ok((gpu, adapter, surface, config, adapter_max_dim))
}

/// 按给定档位**协商并配置** surface，逐级回落。
///
/// 候选顺序：用户档位边长 → 更低档（8192 → 4096）→ `0`（CSS × DPR，DPR 封顶 1.5）。
/// 每个候选先用 [`wgpu::Surface::get_default_config`] 申请（`None` = 该尺寸超出
/// device/surface 能力，继续往下试），申请成功才 `configure`。
///
/// 返回 `Some((config, changed))`：
/// - `changed == false`：候选尺寸与 `current` 相同 ⇒ **没有 configure**（no-op，
///   避免 500ms 兜底定时器每次重配 swapchain）；
/// - `changed == true`：已 configure，调用方需同步 `set_viewport`。
///
/// 全部候选都申请不到时返回 `None`——调用方保留原 config，**舞台不因一次坏尺寸
/// 而黑屏**。
#[allow(clippy::too_many_arguments)]
pub(crate) fn negotiate_surface(
    surface: &wgpu::Surface<'static>,
    adapter: &wgpu::Adapter,
    device: &wgpu::Device,
    canvas: &HtmlCanvasElement,
    window: &Window,
    tier: RenderTier,
    max_dim: u32,
    format: wgpu::TextureFormat,
    alpha_mode: wgpu::CompositeAlphaMode,
    current: Option<(u32, u32)>,
) -> Option<(wgpu::SurfaceConfiguration, bool)> {
    let device_max = device.limits().max_texture_dimension_2d;
    let mut candidates = candidate_sides(tier, max_dim);
    // 最后一道：CSS × DPR（`side == 0`）。
    candidates.push(0);
    for side in candidates {
        let (width, height) = canvas_pixel_size(canvas, window, side);
        if width > device_max || height > device_max {
            continue;
        }
        let Some(mut config) = surface.get_default_config(adapter, width, height) else {
            continue;
        };
        config.format = format;
        config.alpha_mode = alpha_mode;
        // C3 裁决：WASM 端显式设置 desired_maximum_frame_latency = 2，
        // 不依赖 wgpu 版本缺省值（与 desktop 一致，构成可对照承诺）。
        config.desired_maximum_frame_latency = 2;
        if current == Some((width, height)) {
            return Some((config, false));
        }
        surface.configure(device, &config);
        return Some((config, true));
    }
    None
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

/// CSS 像素 × devicePixelRatio 的兜底物理画布尺寸（至少 1×1）——档位救不回
/// 来时用它。
///
/// **DPR 钳制**（C4 裁决 v1）：物理像素 = CSS 尺寸 × `min(dpr, 1.5)`。
/// 全面屏 / 高 DPI 屏的 dpr 常达 2–3，裸采会把像素数翻到 4–9 倍，压垮
/// WebGPU 着色器与 16ms 预算——Live2D 的骨骼 IK + 多层混合已是瓶颈。
pub(crate) const DPR_CAP: f64 = 1.5;

/// 逐个尝试的档位边长（从高到低，去重；全部超过 adapter 上限时为空）。
///
/// 与「只算一个回落值」不同：申请要**逐个试**——`get_default_config` 可能对某个
/// 尺寸返回 `None`，此时必须继续往下（见 [`negotiate_surface`]）。
fn candidate_sides(tier: RenderTier, max_dim: u32) -> Vec<u32> {
    let mut sides: Vec<u32> = Vec::new();
    if let Some(highest) = tier.highest_within(max_dim) {
        for t in [highest, RenderTier::Tier8192, RenderTier::Tier4096] {
            let s = t.side();
            if s <= max_dim && !sides.contains(&s) {
                sides.push(s);
            }
        }
    }
    sides
}

/// CSS 显示盒 → 物理画布尺寸。
///
/// `side > 0`：**最长边 = side**，另一边按 CSS 宽高比等比缩放（保证缓冲宽高比
/// 恒等于显示盒宽高比——**不撑成正方形**，也不把宽画布单独削宽）。
/// `side == 0`：退回「CSS × min(dpr, DPR_CAP)」（至少 1×1）。
pub(crate) fn canvas_pixel_size(
    canvas: &HtmlCanvasElement,
    window: &Window,
    side: u32,
) -> (u32, u32) {
    let css_w = canvas.client_width().max(0) as f64;
    let css_h = canvas.client_height().max(0) as f64;
    if side == 0 {
        let dpr = window.device_pixel_ratio().clamp(1.0, DPR_CAP);
        return (
            ((css_w * dpr).round() as u32).max(1),
            ((css_h * dpr).round() as u32).max(1),
        );
    }
    // 没有可用的 CSS 尺寸（尚未布局 / 隐藏）时退化成正方形，保证 ≥1。
    if css_w <= 0.0 || css_h <= 0.0 {
        return (side, side);
    }
    let longest = css_w.max(css_h);
    let k = f64::from(side) / longest;
    (
        ((css_w * k).round() as u32).max(1),
        ((css_h * k).round() as u32).max(1),
    )
}
