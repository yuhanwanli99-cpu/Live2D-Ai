//! 离屏便利层：共享 GPU 上下文 + 渲染核心 + 目标纹理的一步到位封装。
//!
//! [`OffscreenRenderer`] 只是把常用路径（建画布 → 加载 → 固定 dt 更新 →
//! 出帧/PNG 落盘）串起来；需要嵌入现有 wgpu 应用、渲染到窗口视图或
//! 多画布共享设备时，直接用 [`GpuContext`] + [`ModelRendererCore`]。
//!
//! 尺寸纪律：创建与 [`resize`](Self::resize) 都拒绝 0 尺寸
//! （[`RenderError::InvalidSize`]）；resize 重建目标纹理并同步渲染核心视口。

use std::sync::Arc;

use ayagami_render::texture::Texture;
use glam::u32::UVec2;

use super::{GpuContext, ModelRendererCore, RenderError, RgbaFrame, readback_rgba, validate_size};
use crate::model::LoadedModel;

/// headless 离屏渲染器（便利层）。
pub struct OffscreenRenderer {
    gpu: Arc<GpuContext>,
    core: ModelRendererCore,
    target: Texture,
    width: u32,
    height: u32,
}

impl OffscreenRenderer {
    /// 创建指定尺寸的 headless 离屏画布（自建共享 GPU 上下文）。
    ///
    /// 宽高必须 > 0。尝试 Vulkan → GL → 全部后端；容器/无显卡环境通常
    /// 落到软件光栅（功能验证有效，性能不代表真机）。
    pub fn new(width: u32, height: u32) -> Result<Self, RenderError> {
        // ModelRendererCore 的共享上下文签名要求 Arc；native 下 GpuContext 为
        // Send+Sync。wasm32 下 wgpu web 后端类型非 Send（本路径为 headless 专用，
        // 不跨线程），clippy 在该目标下会提示——按行豁免。
        #[allow(clippy::arc_with_non_send_sync)]
        let gpu = Arc::new(GpuContext::headless()?);
        Self::with_gpu_context(gpu, width, height)
    }

    /// 用**已有**共享 GPU 上下文创建离屏画布（宽高必须 > 0）。
    pub fn with_gpu_context(
        gpu: Arc<GpuContext>,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        validate_size(width, height)?;
        let target = create_target(&gpu, width, height);
        let mut core = ModelRendererCore::new(Arc::clone(&gpu))?;
        core.set_viewport(width, height)?;
        Ok(Self {
            gpu,
            core,
            target,
            width,
            height,
        })
    }

    /// 共享 GPU 上下文。
    pub fn gpu_context(&self) -> &Arc<GpuContext> {
        &self.gpu
    }

    /// 实际选中的 GPU adapter 描述（观测用）。
    pub fn gpu_adapter(&self) -> &str {
        self.gpu.adapter_label()
    }

    /// 画布尺寸 `(width, height)`。
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// 渲染核心（高级用法：render_to_view / 直接操作姿态栈等）。
    pub fn core(&mut self) -> &mut ModelRendererCore {
        &mut self.core
    }

    /// 调整画布尺寸（宽高必须 > 0）：重建目标纹理并同步核心视口。
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), RenderError> {
        validate_size(width, height)?;
        if width == self.width && height == self.height {
            return Ok(());
        }
        self.target = create_target(&self.gpu, width, height);
        self.core.set_viewport(width, height)?;
        self.width = width;
        self.height = height;
        Ok(())
    }

    /// 加载绑定的 [`LoadedModel`]（纹理/物理取自同一包——错配在类型层面不可能）。
    ///
    /// 物理描述取自包内 physics3.json（无该文件的模型自动跳过物理，
    /// 仅保留 idle 驱动）。
    pub fn load_model(&mut self, loaded: &LoadedModel) -> Result<(), RenderError> {
        self.core.load_model(loaded)
    }

    /// 推进一个固定 dt 步：idle 正弦驱动 → 物理步进 → 合成最终姿态。
    ///
    /// `dt` 建议 [`FIXED_DT_60HZ`](super::FIXED_DT_60HZ)；非法 dt
    /// （非有限 / <= 0）返回 [`RenderError::InvalidDt`] 且状态不变。
    pub fn update(&mut self, dt: f32) -> Result<(), RenderError> {
        self.core.update(dt)
    }

    /// 渲染一帧并阻塞读回为直通 alpha 的 RGBA8 图像。
    ///
    /// 读回语义见模块文档（诚实命名阻塞：无伪造超时预算）。
    pub fn render_frame(&mut self) -> Result<RgbaFrame, RenderError> {
        self.core
            .render_to_view(&self.target.view, self.target.texture.format())?;
        readback_rgba(&self.gpu, &self.target)
    }
}

/// 创建离屏目标纹理（`Rgba8Unorm`、单 mip、mask 与出图 1:1 的用法约定）。
fn create_target(gpu: &GpuContext, width: u32, height: u32) -> Texture {
    Texture::new(
        gpu.device(),
        UVec2::new(width, height),
        wgpu::TextureFormat::Rgba8Unorm,
        Some(1),
        Some("l2d offscreen target"),
    )
}

impl std::fmt::Debug for OffscreenRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OffscreenRenderer")
            .field("adapter", &self.gpu.adapter_label())
            .field("size", &(self.width, self.height))
            .finish_non_exhaustive()
    }
}
