//! 无头（headless）离屏渲染：加载 → 固定 dt 更新 → RGBA 出帧 / PNG 落盘。
//!
//! 分层结构（自底向上）：
//! - [`GpuContext`]（[`gpu`]）：共享 GPU 上下文（device/queue/adapter 观测），
//!   可 headless 自建（vulkan→gl→all 回退），也可用**已有 device/queue** 构造
//!   （嵌入现有 wgpu 应用）；
//! - [`ModelRendererCore`]（[`model_core`]）：渲染核心——上游 [`ModelRenderer`]
//!   封装 + 姿态分层栈（见 [`crate::pose_stack`]），与具体输出目标解耦，
//!   可把一帧渲染到**任意视图**——实时路径调只提交的
//!   [`ModelRendererCore::render_to_view_submit`]，离屏/确定性测试可继续
//!   调用阻塞包装 [`ModelRendererCore::render_to_view`]（精确等待本帧
//!   submission 完成，不再用无索引 `wait_indefinitely`）；
//! - [`OffscreenRenderer`]（[`offscreen`]）：便利层——持有离屏目标纹理，
//!   提供 new/resize/render_frame/PNG 落盘等一步到位 API。
//!
//! 接线来自已验证的 bakeoff 工程（`docs/verification/rust-bakeoff-ayagami.md`）：
//! - 目标纹理 `Rgba8Unorm`，mask 与出图 1:1；变换 = model3 `Layout` 框 × 整画布映射
//!   （上游 demo WholeModel 公式）× **矩形视口等比适配**（非正方形不拉伸）；
//! - 读回前经 `unpremultiply(clamp=true)` 转直通 alpha；
//! - 读回为**诚实命名阻塞语义**：先 `poll(wait)` 等队列排空并触发 GPU 回调，
//!   再阻塞等待回调送达——没有人为重试循环或伪造超时预算；
//!   通道意外关闭返回类型化的 [`RenderError::ReadbackClosed`]。
//!
//! 本模块只暴露自有类型；第三方渲染器类型不进入公开 API 签名。

mod gpu;
mod model_core;
mod offscreen;
pub mod tier;

pub use gpu::GpuContext;
pub use model_core::ModelRendererCore;
pub use offscreen::OffscreenRenderer;
pub use tier::RenderTier;

use std::{path::Path, sync::Arc};

use ayagami_render::ModelRenderer;
use ayagami_render::texture::{Texture, TextureManager};
use glam::f32::{Affine2, Vec2};

use crate::model::LoadedModel;

/// 上游模型渲染器实例类型（crate 内部别名）。
pub(crate) type InnerRenderer =
    ModelRenderer<ayagami::file::ParsedModel, Arc<ayagami::file::ParsedModel>>;

/// 固定步长：60 Hz（bakeoff 验证值，物理引擎按此步进最稳定）。
pub const FIXED_DT_60HZ: f32 = 1.0 / 60.0;

/// 渲染错误（全部类型化；底层原因以字符串携带，不外泄第三方错误类型）。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum RenderError {
    /// 无可用的 headless GPU 后端（已尝试 vulkan/gl/all）。
    NoGpuBackend(String),
    /// GPU 设备请求失败。
    Device(String),
    /// 内部渲染器错误（字符串化，不外泄第三方错误类型）。
    Renderer(String),
    /// physics3.json 解析失败。
    PhysicsJson(String),
    /// 非法尺寸：宽/高为 0 或超出限制。
    InvalidSize {
        /// 传入的宽度。
        width: u32,
        /// 传入的高度。
        height: u32,
    },
    /// 非法 dt（非有限或 <= 0），透传自姿态栈校验。
    InvalidDt(f32),
    /// 设备 poll 失败（队列排空等待出错）。
    Poll(String),
    /// 帧读回回调通道关闭——GPU 回调未送达（设备丢失等异常路径）。
    ReadbackClosed,
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoGpuBackend(notes) => {
                write!(f, "无可用 headless GPU 后端（vulkan→gl→all）：{notes}")
            }
            Self::Device(e) => write!(f, "GPU 设备创建失败: {e}"),
            Self::Renderer(e) => write!(f, "渲染器错误: {e}"),
            Self::PhysicsJson(e) => write!(f, "physics3.json 解析失败: {e}"),
            Self::InvalidSize { width, height } => {
                write!(f, "非法画布尺寸 {width}x{height}（宽高必须 > 0）")
            }
            Self::InvalidDt(dt) => write!(f, "非法 dt `{dt}`（必须为有限且 > 0）"),
            Self::Poll(e) => write!(f, "device poll 失败: {e}"),
            Self::ReadbackClosed => {
                write!(f, "帧读回失败：GPU 回调通道关闭（回调未送达）")
            }
        }
    }
}

impl std::error::Error for RenderError {}

impl From<crate::pose_stack::InvalidDt> for RenderError {
    fn from(value: crate::pose_stack::InvalidDt) -> Self {
        Self::InvalidDt(value.0)
    }
}

/// 校验画布尺寸（宽高必须 > 0 且不超过设备纹理上限由 wgpu 兜底）。
pub(crate) fn validate_size(width: u32, height: u32) -> Result<(), RenderError> {
    if width == 0 || height == 0 {
        return Err(RenderError::InvalidSize { width, height });
    }
    Ok(())
}

/// 矩形视口的等比适配变换（letterbox：内容完整可见、居中、不拉伸）。
///
/// 逻辑变换（Layout × 整画布）把内容映到 [-1,1]² NDC；非正方形视口直接使用
/// 会拉伸。本函数返回绕原点的缩放修正：长边方向压到与短边一致的比例。
///
/// - `1024x512` → x 方向 ×0.5（水平收窄到与垂直同物理比例）；
/// - `512x1024` → y 方向 ×0.5；
/// - 正方形 → 恒等。
pub fn aspect_fit_transform(width: u32, height: u32) -> Affine2 {
    if width == 0 || height == 0 {
        return Affine2::IDENTITY;
    }
    let (wf, hf) = (width as f32, height as f32);
    let sx = if wf >= hf { hf / wf } else { 1.0 };
    let sy = if hf > wf { wf / hf } else { 1.0 };
    Affine2::from_scale(Vec2::new(sx, sy))
}

/// 一帧直通 alpha 的 RGBA8 图像。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaFrame {
    pub width: u32,
    pub height: u32,
    /// 行优先 RGBA8 像素（长度 = width * height * 4）。
    pub pixels: Vec<u8>,
}

impl RgbaFrame {
    /// alpha > 0 的像素占比（0.0–1.0），用于冒烟自检。
    pub fn alpha_coverage(&self) -> f64 {
        if self.pixels.is_empty() || self.width == 0 || self.height == 0 {
            return 0.0;
        }
        let opaque = self
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|px| px[3] != 0)
            .count();
        opaque as f64 / (self.width as f64 * self.height as f64)
    }

    /// alpha > 0 像素的包围盒 `(min_x, min_y, max_x, max_y)`（含端点）；
    /// 全透明时返回 `None`。用于冒烟自检与回归比对。
    pub fn visible_bbox(&self) -> Option<(u32, u32, u32, u32)> {
        if self.pixels.is_empty() || self.width == 0 || self.height == 0 {
            return None;
        }
        let mut bbox: Option<(u32, u32, u32, u32)> = None;
        for (i, px) in self.pixels.as_chunks::<4>().0.iter().enumerate() {
            if px[3] == 0 {
                continue;
            }
            let (x, y) = ((i as u32) % self.width, (i as u32) / self.width);
            bbox = Some(match bbox {
                None => (x, y, x, y),
                Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
            });
        }
        bbox
    }

    /// 编码并写入 PNG（父目录不存在则创建）。
    pub fn save_png(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let path = path.as_ref();
        if let Some(dir) = path.parent()
            && !dir.as_os_str().is_empty()
        {
            std::fs::create_dir_all(dir)?;
        }
        let image =
            image::RgbaImage::from_raw(self.width, self.height, Vec::from(&self.pixels[..]))
                .ok_or_else(|| {
                    std::io::Error::other(format!(
                        "pixel buffer size mismatch: expected {} bytes",
                        self.width as usize * self.height as usize * 4
                    ))
                })?;
        image
            .save_with_format(path, image::ImageFormat::Png)
            .map_err(|e| std::io::Error::other(e.to_string()))
    }
}

/// 从离屏目标纹理阻塞读回直通 alpha 的 RGBA8 帧。
///
/// # 阻塞语义（诚实命名）
///
/// 1. `unpremultiply` 转直通 alpha 并提交 copy；
/// 2. `device.poll(wait_indefinitely)` **真实阻塞**至队列排空——wgpu 保证
///    buffer 映射回调在该次 poll 内触发；
/// 3. 阻塞 `recv()` 收取回调结果（正常情况下此刻立即返回）。
///
/// 没有人为的重试循环、没有伪造的超时预算；若回调通道意外关闭
/// （设备丢失等异常路径），返回类型化的 [`RenderError::ReadbackClosed`]。
pub(crate) fn readback_rgba(gpu: &GpuContext, target: &Texture) -> Result<RgbaFrame, RenderError> {
    let manager = TextureManager::new(gpu.device());
    let straight = manager.unpremultiply(gpu.device(), gpu.queue(), target, true);

    let (tx, rx) = std::sync::mpsc::channel::<image::DynamicImage>();
    straight.download_to_image(gpu.device(), gpu.queue(), move |img| {
        let _ = tx.send(img);
    });

    gpu.device()
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| RenderError::Poll(format!("readback wait failed: {e}")))?;

    let img = rx.recv().map_err(|_| RenderError::ReadbackClosed)?;
    let rgba = img.to_rgba8();
    Ok(RgbaFrame {
        width: rgba.width(),
        height: rgba.height(),
        pixels: rgba.into_raw(),
    })
}

/// 由绑定的 [`LoadedModel`] 计算 Layout × 整画布的**逻辑**变换
/// （不含视口 aspect 修正；aspect 在 render 时叠加）。
pub(crate) fn layout_transform(loaded: &LoadedModel) -> Affine2 {
    let canvas = loaded.handle().canvas();
    layout_transform_from(
        glam::f32::Vec2::new(canvas.width_units, canvas.height_units),
        glam::f32::Vec2::new(canvas.center_x_units, canvas.center_y_units),
        canvas.pixels_per_unit,
        loaded.manifest().layout,
    )
}

/// [`layout_transform`] 的纯函数内核（不依赖模型资产，便于回归测试）。
pub(crate) fn layout_transform_from(
    canvas_dims: Vec2,
    canvas_center: Vec2,
    pixels_per_unit: f32,
    layout: crate::asset::LayoutBox,
) -> Affine2 {
    // **等比映射（2026-09-10 修复）**：x/y 必须用同一个缩放因子。
    //
    // 旧实现是 `Vec2::new(2.0, 2.0) / canvas_dims` —— **逐轴**缩放，等于把画布
    // 强行拉成正方形：Bai 皮套的 moc3 画布是 4000×6000（竖长），于是 y 比 x 少缩
    // 1.5 倍，模型被横向拉伸 1.5×（用户观感「上下被压扁」）。实测 x/y 比例为 1.5，
    // 与画布 4000:6000 完全吻合。
    //
    // 现在以**画布宽度**为基准取等比因子 `s`，并把画布中心平移到 NDC 原点：
    // 模型规范化空间 v ∈ [-0.5, 0.5]² 等比映射到 NDC [-1, 1]²，不产生任何拉伸。
    // （`pixels_per_unit` 与画布宽度不一致的模型只会整体变大/变小，仍不变形。）
    let s = 2.0 / canvas_dims.x;
    let offset = canvas_center * s;
    let whole_canvas = Affine2::from_translation(-offset)
        * Affine2::from_scale(Vec2::splat(s))
        * Affine2::from_translation(canvas_center)
        * Affine2::from_scale(Vec2::splat(pixels_per_unit));
    Affine2::from_translation(glam::f32::vec2(layout.center_x, layout.center_y))
        * Affine2::from_scale(glam::f32::vec2(layout.width, layout.height))
        * whole_canvas
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aspect_fit_is_identity_for_square() {
        assert_eq!(aspect_fit_transform(256, 256), Affine2::IDENTITY);
    }

    /// **回归（2026-09-10）**：`layout_transform_from` 必须是**等比**映射。
    ///
    /// 历史缺陷：逐轴 `Vec2::new(2.0, 2.0) / canvas_dims` 把非正方形画布
    /// （Bai 为 4000×6000）强行拉成正方形 → 模型横向拉伸 1.5×，观感「上下被压扁」。
    /// 本测试不依赖模型资产（纯参数），CI 无资产时同样生效。
    #[test]
    fn layout_transform_is_uniform_for_non_square_canvas() {
        for (w, h) in [(4000.0_f32, 6000.0_f32), (6000.0, 4000.0), (1024.0, 1024.0)] {
            let t = layout_transform_from(
                glam::f32::Vec2::new(w, h),
                glam::f32::Vec2::new(w / 2.0, h / 2.0),
                4000.0,
                crate::asset::LayoutBox {
                    center_x: 0.0,
                    center_y: -0.07,
                    width: 0.56,
                    height: 0.56,
                },
            );
            let o = t.transform_point2(glam::f32::Vec2::ZERO);
            let dx = t.transform_point2(glam::f32::Vec2::new(1.0, 0.0)) - o;
            let dy = t.transform_point2(glam::f32::Vec2::new(0.0, 1.0)) - o;
            assert!(
                (dx.x - dy.y).abs() < 1e-6,
                "画布 {w}x{h} 必须等比：每单位 dx={} dy={}",
                dx.x,
                dy.y
            );
            // 模型空间原点（画布中心）落在 layout 中心上。
            assert!(
                (o.x - 0.0).abs() < 1e-6 && (o.y + 0.07).abs() < 1e-6,
                "origin={o:?}"
            );
            // 正方形模型区域映射后仍是 NDC 正方形（不变形）。
            let span = t.transform_point2(glam::f32::Vec2::new(0.5, 0.5))
                - t.transform_point2(glam::f32::Vec2::new(-0.5, -0.5));
            assert!(
                (span.x - span.y).abs() < 1e-6,
                "0.5 见方应映射为 NDC 正方形，got {span:?}"
            );
        }
    }

    #[test]
    fn aspect_fit_letterboxes_wide_and_tall() {
        // 宽视口：x 方向压半，y 不动 → 单位内容在两轴上的像素跨度一致。
        let wide = aspect_fit_transform(1024, 512);
        assert!((wide.matrix2.x_axis.x - 0.5).abs() < 1e-6);
        assert!((wide.matrix2.y_axis.y - 1.0).abs() < 1e-6);

        let tall = aspect_fit_transform(512, 1024);
        assert!((tall.matrix2.x_axis.x - 1.0).abs() < 1e-6);
        assert!((tall.matrix2.y_axis.y - 0.5).abs() < 1e-6);
    }

    #[test]
    fn aspect_fit_degenerate_size_is_identity() {
        assert_eq!(aspect_fit_transform(0, 100), Affine2::IDENTITY);
        assert_eq!(aspect_fit_transform(100, 0), Affine2::IDENTITY);
    }

    #[test]
    fn invalid_size_is_rejected() {
        assert!(matches!(
            validate_size(0, 10),
            Err(RenderError::InvalidSize {
                width: 0,
                height: 10
            })
        ));
        assert!(matches!(
            validate_size(10, 0),
            Err(RenderError::InvalidSize {
                width: 10,
                height: 0
            })
        ));
        assert!(validate_size(10, 10).is_ok());
        assert!(
            RenderError::InvalidSize {
                width: 0,
                height: 0
            }
            .to_string()
            .contains("非法画布尺寸")
        );
    }

    #[test]
    fn invalid_dt_maps_into_render_error() {
        let err: RenderError = crate::pose_stack::InvalidDt(-1.0).into();
        assert_eq!(err, RenderError::InvalidDt(-1.0));
        assert!(err.to_string().contains("-1"));
    }

    /// 编译期契约锁：双 API 签名必须保持——
    /// `render_to_view_submit` 返回 `wgpu::SubmissionIndex`（实时路径使用），
    /// `render_to_view` 是 `Result<(), RenderError>` 阻塞包装。
    /// 类型不匹配会直接编译失败，不需要 GPU。
    #[test]
    fn render_api_signatures_are_stable() {
        // 函数指针类型检查：故意只取签名不调用。
        let _submit: fn(
            &mut ModelRendererCore,
            &wgpu::TextureView,
            wgpu::TextureFormat,
        ) -> Result<wgpu::SubmissionIndex, RenderError> = ModelRendererCore::render_to_view_submit;
        let _blocking: fn(
            &mut ModelRendererCore,
            &wgpu::TextureView,
            wgpu::TextureFormat,
        ) -> Result<(), RenderError> = ModelRendererCore::render_to_view;
    }

    #[test]
    fn rgba_frame_helpers_handle_empty_and_content() {
        let empty = RgbaFrame {
            width: 2,
            height: 2,
            pixels: vec![0; 16],
        };
        assert_eq!(empty.alpha_coverage(), 0.0);
        assert_eq!(empty.visible_bbox(), None);

        let mut pixels = vec![0_u8; 12];
        // (x=2, y=0) 半透明可见；(x=0, y=0)、(x=1, y=0) 全透明。
        pixels[2 * 4 + 3] = 128;
        let frame = RgbaFrame {
            width: 3,
            height: 1,
            pixels,
        };
        assert!((frame.alpha_coverage() - 1.0 / 3.0).abs() < 1e-9);
        assert_eq!(frame.visible_bbox(), Some((2, 0, 2, 0)));
    }
}
