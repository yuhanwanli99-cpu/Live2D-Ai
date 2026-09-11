//! 渲染核心：上游 `ModelRenderer` 封装 + 姿态分层栈，与输出目标解耦。
//!
//! [`ModelRendererCore`] 不持有任何目标纹理：一帧可经
//! [`render_to_view`](Self::render_to_view) 渲染到**任意** `wgpu::TextureView`
//! （离屏画布、窗口交换链视图皆可）。姿态更新走
//! [`crate::pose_stack::PoseStack`] 分层（base/idle/input/physics/final_override），
//! 外部输入用 [`set_parameter`](Self::set_parameter) /
//! [`clear_parameter`](Self::clear_parameter)，最高优先级覆盖用
//! [`override_parameter`](Self::override_parameter)。

use std::sync::Arc;

use ayagami_render::{RenderColorspace, RenderOptions};
use glam::f32::Affine2;
use glam::u32::UVec2;

use super::{
    GpuContext, InnerRenderer, RenderError, aspect_fit_transform, layout_transform, validate_size,
};
use crate::model::LoadedModel;
use crate::pose_stack::PoseStack;

/// 渲染核心（与输出目标解耦）。
pub struct ModelRendererCore {
    gpu: Arc<GpuContext>,
    renderer: InnerRenderer,
    stack: PoseStack,
    /// 逻辑变换（Layout × 整画布；不含视口 aspect 修正）。
    transform: Affine2,
    /// 视口尺寸（mask 与之 1:1；aspect 修正按此计算）。
    viewport: (u32, u32),
    colorspace: RenderColorspace,
}

impl ModelRendererCore {
    /// 在给定共享 GPU 上下文上创建渲染核心（未加载模型）。
    pub fn new(gpu: Arc<GpuContext>) -> Result<Self, RenderError> {
        let renderer = InnerRenderer::new(gpu.device().clone(), gpu.queue().clone())
            .map_err(|e| RenderError::Renderer(e.to_string()))?;
        Ok(Self {
            gpu,
            renderer,
            stack: PoseStack::from_pose_map(Arc::new(ayagami::pose::PoseMap::new())),
            transform: Affine2::IDENTITY,
            viewport: (1, 1),
            colorspace: RenderColorspace::SRgb,
        })
    }

    /// 共享 GPU 上下文。
    pub fn gpu(&self) -> &Arc<GpuContext> {
        &self.gpu
    }

    /// 姿态分层栈（高级用法：直接操作层/观测合成值）。
    pub fn stack(&mut self) -> &mut PoseStack {
        &mut self.stack
    }

    /// 向 **input 层**写入参数（模型缺失该参数时返回 `false`）。
    pub fn set_parameter(&mut self, id: &str, value: f32) -> bool {
        self.stack.set_parameter(id, value)
    }

    /// 撤销 **input 层**参数（下一帧起不再生效）。
    pub fn clear_parameter(&mut self, id: &str) -> bool {
        self.stack.clear_parameter(id)
    }

    /// 向 **final_override 层**写入最高优先级覆盖。
    pub fn override_parameter(&mut self, id: &str, value: f32) -> bool {
        self.stack.override_parameter(id, value)
    }

    /// 撤销 **final_override 层**参数。
    pub fn clear_override_parameter(&mut self, id: &str) -> bool {
        self.stack.clear_override_parameter(id)
    }

    /// 加载绑定的 [`LoadedModel`]（纹理/物理取自同一包——错配在类型层面不可能）。
    ///
    /// 变换 = Layout 构图框 × 整画布映射（逻辑变换，aspect 修正渲染时叠加）；
    /// 初始姿态为分层栈的合成值（含物理 settle 结果）。
    pub fn load_model(&mut self, loaded: &LoadedModel) -> Result<(), RenderError> {
        let texture_bytes = loaded.package().texture_bytes();
        let texrefs: Vec<&[u8]> = texture_bytes.iter().map(Vec::as_slice).collect();
        self.renderer
            .load_model(loaded.handle().shared(), &texrefs)
            .map_err(|e| RenderError::Renderer(e.to_string()))?;

        self.stack = PoseStack::from_loaded_model(loaded)
            .map_err(|e| RenderError::PhysicsJson(e.to_string()))?;
        self.transform = layout_transform(loaded);

        // 确定性初始姿态：先同步一次合成结果（含物理 settle 输出）。
        let initial = self.stack.finalize();
        self.renderer.driver().apply_pose(&initial);
        Ok(())
    }

    /// 推进一个固定 dt 步：重算 idle → 物理步进 → 合成最终姿态并驱动渲染器。
    ///
    /// `dt` 必须有限且 > 0（推荐 [`super::FIXED_DT_60HZ`]），否则返回
    /// [`RenderError::InvalidDt`] 且状态不变。
    pub fn update(&mut self, dt: f32) -> Result<(), RenderError> {
        self.stack.update(dt)?;
        let final_pose = self.stack.finalize();
        self.renderer.driver().set_pose(&final_pose);
        Ok(())
    }

    /// 视口尺寸 `(width, height)`。
    pub fn viewport(&self) -> (u32, u32) {
        self.viewport
    }

    /// 更新视口尺寸（mask 与 aspect 修正随之变化）。宽高必须 > 0。
    pub fn set_viewport(&mut self, width: u32, height: u32) -> Result<(), RenderError> {
        validate_size(width, height)?;
        self.viewport = (width, height);
        Ok(())
    }

    /// 逻辑变换（Layout × 整画布，不含 aspect 修正）。
    pub fn transform(&self) -> Affine2 {
        self.transform
    }

    /// 覆盖逻辑变换（默认由 [`Self::load_model`] 从 model3 `Layout` 推得）。
    pub fn set_transform(&mut self, transform: Affine2) {
        self.transform = transform;
    }

    /// 编码当前状态的一帧并提交到队列，**不等待 GPU 完成**——返回本帧的
    /// [`wgpu::SubmissionIndex`]，调用方可借此精确等待自己的提交。
    ///
    /// 视图尺寸应与视口一致（mask 与出图 1:1）；实际使用的变换 =
    /// **矩形视口等比适配**（[`aspect_fit_transform`]）× 逻辑变换，
    /// 非正方形视口不拉伸。
    ///
    /// 实时路径（桌面交换链 / WASM `rAF`）应只调用本方法，避免 CPU 阻塞
    /// 等待 GPU；离屏读回等需要"帧已完成"语义的场景使用阻塞包装
    /// [`render_to_view`](Self::render_to_view)。
    pub fn render_to_view_submit(
        &mut self,
        view: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) -> Result<wgpu::SubmissionIndex, RenderError> {
        let options = RenderOptions {
            transform: aspect_fit_transform(self.viewport.0, self.viewport.1) * self.transform,
            mask_dimensions: UVec2::new(self.viewport.0, self.viewport.1),
            colorspace: self.colorspace,
        };
        let mut encoder =
            self.gpu
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("l2d core frame encoder"),
                });
        self.renderer.prepare(&mut encoder, &options);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("l2d core pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            self.renderer.render(&mut pass, format);
        }
        Ok(self.gpu.queue().submit(Some(encoder.finish())))
    }

    /// 阻塞包装：编码 + 提交 + **仅等待本帧的 submission 完成**。
    ///
    /// 内部调用 [`render_to_view_submit`](Self::render_to_view_submit)，
    /// 然后 `device.poll(Wait { submission_index: Some(index), timeout: None })`——
    /// 只精确等待自己刚提交的那一帧，**不再用无索引 `wait_indefinitely()`
    /// 等待调用时"最新全部工作"**，避免与同 device 上其它路径抢锁。
    ///
    /// 离屏读回与确定性测试继续使用本版本；实时路径必须改调 submit 版本。
    pub fn render_to_view(
        &mut self,
        view: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) -> Result<(), RenderError> {
        let index = self.render_to_view_submit(view, format)?;
        self.gpu
            .device()
            .poll(wgpu::PollType::Wait {
                submission_index: Some(index),
                timeout: None,
            })
            .map_err(|e| RenderError::Poll(format!("frame wait failed: {e}")))?;
        Ok(())
    }
}

impl std::fmt::Debug for ModelRendererCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelRendererCore")
            .field("gpu", &self.gpu)
            .field("viewport", &self.viewport)
            .finish_non_exhaustive()
    }
}
