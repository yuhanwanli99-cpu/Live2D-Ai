//! 渲染面的 GPU / surface 协商（仅 `target_arch = "wasm32"` 下编译）。
//!
//! - [`init_gpu`]：WebGPU 优先、WebGL2 回退；显式设置
//!   `desired_maximum_frame_latency = 2`（C3）。
//! - [`backend_label`] / [`adapter_info`]：实际后端与 adapter 信息（HUD 观测用）。
//! - [`canvas_pixel_size`]：CSS 尺寸 × DPR 钳制 → 物理画布尺寸（含档位上限）。

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

    let (width, height) = canvas_pixel_size(canvas, window, RenderTier::DEFAULT);
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
/// **档位钳制**（ADR §3.3）：`tier.side()` 作为物理画布目标侧边的**上限**，
/// 实际尺寸 = `min(clamp 后的值, tier.side())`，避免超大离屏 target 超 sampler
/// 上限/显存预算而失败。默认档位 [`RenderTier::DEFAULT`] 不影响既有 1.5 cap。
pub(crate) const DPR_CAP: f64 = 1.5;
pub(crate) fn canvas_pixel_size(
    canvas: &HtmlCanvasElement,
    window: &Window,
    tier: RenderTier,
) -> (u32, u32) {
    // 钳 dpr 到 [1, DPR_CAP]，保证最小 1 倍（HiDPI 缩放也得有下限）。
    let dpr = window.device_pixel_ratio().clamp(1.0, DPR_CAP);
    let css_w = canvas.client_width().max(0) as f64;
    let css_h = canvas.client_height().max(0) as f64;
    let width = ((css_w * dpr).round() as u32).max(1);
    let height = ((css_h * dpr).round() as u32).max(1);
    let side = tier.side();
    // **保比钳制**：逐维 `min(side)` 会改变宽高比（宽画布被单独削宽 → 显示时被横向拉伸），
    // 改为「最长边超限时整体等比缩小」，保证缓冲宽高比恒等于 CSS 显示盒宽高比。
    let longest = width.max(height);
    if longest > side {
        let k = side as f64 / longest as f64;
        return (
            ((width as f64 * k).round() as u32).max(1),
            ((height as f64 * k).round() as u32).max(1),
        );
    }
    (width, height)
}
