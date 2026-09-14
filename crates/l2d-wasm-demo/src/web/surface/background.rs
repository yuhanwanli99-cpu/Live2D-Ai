//! 舞台背景的 GPU 实现（仅 `target_arch = "wasm32"` 下编译）：
//! 把 **stageColor 纯色底 + 背景图**画进 framebuffer，而不是依赖 canvas 的 CSS。
//!
//! # 为什么必须画进 framebuffer（2026-09-14 rc.5，实测逼出来的）
//!
//! Web 端 WebGPU 的 canvas 只能**不透明**合成：wgpu 29 的 webgpu 后端
//! `get_capabilities` 只报 `CompositeAlphaMode::Opaque`，而 `wgpu-core` 会以
//! `UnsupportedAlphaMode` 拒绝 caps 之外的 alpha_mode。于是
//! `LoadOp::Clear(TRANSPARENT)` 的像素被合成为**不透明黑**，整块 canvas 变成
//! 黑幕，盖住 canvas 自己的 CSS 背景（纯色底与背景图都看不见）。
//!
//! 「让 canvas 透明」这条路由**结构性不可达**；WebGL2 优先在本机也没生效
//! （HUD 实测仍是 WebGPU）。所以：**底色与背景图一律由渲染面自己填进 framebuffer**。
//!
//! # 一帧的顺序
//!
//! 1. 背景预通道（本模块）：`LoadOp::Clear(<stageColor>)`，有图时再画一个全屏
//!    三角形、按 cover 采样背景纹理；
//! 2. 模型通道（`l2d`）：`LoadOp::Load` 只叠 Live2D，**不再 clear**。
//!
//! 于是「图盖在纯色底上、模型在最上层、清图回纯色」的叠放语义完整保留，
//! 且与 canvas 是否透明**完全无关**。
//!
//! 纯逻辑（base64 解码 / cover 比例 / 纯色回退）在原生可测的
//! [`crate::stage_bg`]；本文件只做 GPU 资源与异步解码。

use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use super::render::{FrameState, SharedState};
use crate::stage_bg;

/// 全屏三角形 + cover 采样。uniform 布局：两个 `vec2<f32>`（scale / offset），共 16 字节。
const SHADER: &str = r#"
struct Uniforms {
    uv_scale: vec2<f32>,
    uv_offset: vec2<f32>,
};

@group(0) @binding(0) var bg_tex: texture_2d<f32>;
@group(0) @binding(1) var bg_sampler: sampler;
@group(0) @binding(2) var<uniform> u: Uniforms;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {
    var xy = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let p = xy[idx];
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    // 裁剪空间 y 向上、纹理 v 向下 → 翻 y；再套 cover 的 scale/offset。
    let t = vec2<f32>((p.x + 1.0) * 0.5, (1.0 - p.y) * 0.5);
    out.uv = t * u.uv_scale + u.uv_offset;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(bg_tex, bg_sampler, in.uv);
}
"#;

/// 背景图的 GPU 资源（纹理 + 绑定组 + 像素尺寸）。
struct ImageEntry {
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

/// 舞台背景渲染器：底色由调用方以 clear 值给出，图由本结构持有。
pub(crate) struct BackgroundRenderer {
    pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    image: Option<ImageEntry>,
    /// 最近一次**请求**的 dataURL（父页 `stage-bg` 的原样值）。
    requested: Option<String>,
    /// 最近一次**成功上屏**的 dataURL（诊断用；`None` = 当前无图）。
    applied: Option<String>,
}

impl BackgroundRenderer {
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("stage background shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("stage background pipeline"),
            // 默认布局由 shader 推导；绑定下标必须与 WGSL 的 @binding 一致。
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // 预通道已 clear 过底色，图是**不透明覆盖**，不需要混合。
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let bind_group_layout = pipeline.get_bind_group_layout(0);
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("stage background uniforms"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("stage background sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        Self {
            pipeline,
            sampler,
            uniform,
            bind_group_layout,
            image: None,
            requested: None,
            applied: None,
        }
    }

    /// 每帧渲染前调用：按当前视口尺寸更新 cover 变换。
    ///
    /// 无图时**不写** uniform（那帧根本不画背景）。16 字节的 `write_buffer` 极廉价，
    /// 所以不做「值没变就不写」的记账——少一处状态就少一处不一致。
    pub(crate) fn prepare(&mut self, queue: &wgpu::Queue, view_w: u32, view_h: u32) {
        let Some(image) = &self.image else {
            return;
        };
        let uv = stage_bg::cover_uv(
            image.width as f32,
            image.height as f32,
            view_w as f32,
            view_h as f32,
        );
        queue.write_buffer(&self.uniform, 0, &floats_to_bytes(&uv));
    }

    /// 背景预通道内绘制（无图时是 no-op，只剩调用方的 clear 底色）。
    pub(crate) fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(image) = &self.image else {
            return;
        };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &image.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    /// 已上屏的背景图 dataURL（`None` = 纯色底）。诊断用。
    pub(crate) fn applied(&self) -> Option<&str> {
        self.applied.as_deref()
    }

    fn install(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("stage background texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // **非 sRGB**：父页给的 dataURL 就是按 sRGB 编码的字节，直接透传采样，
            // 与旧的 CSS background-image 观感一致（不做二次转换）。
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("stage background bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(
                        self.uniform.as_entire_buffer_binding(),
                    ),
                },
            ],
        });
        self.image = Some(ImageEntry {
            bind_group,
            width,
            height,
        });
    }
}

fn floats_to_bytes(values: &[f32; 4]) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    for (i, value) in values.iter().enumerate() {
        bytes[i * 4..i * 4 + 4].copy_from_slice(&value.to_ne_bytes());
    }
    bytes
}

/// 每帧（`borrow` 之前）调用：把父页的 `stage-bg` 请求同步到 GPU 资源。
///
/// 变了才动：新图 → 异步解码后安装；清图 → 立刻丢掉纹理（回纯色底）。
pub(crate) fn sync_request(state: &SharedState) {
    let (changed, requested) = {
        let st = state.borrow();
        let wanted = st.bridge.bg_data_url.clone();
        (wanted != st.background.requested, wanted)
    };
    if !changed {
        return;
    }
    {
        let mut st = state.borrow_mut();
        st.background.requested = requested.clone();
        if requested.is_none() {
            st.background.image = None;
            st.background.applied = None;
        }
    }
    if let Some(data_url) = requested {
        spawn_decode(state, data_url);
    }
}

fn spawn_decode(state: &SharedState, data_url: String) {
    let state = state.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let decoded = decode_to_rgba(&data_url).await;
        let mut st = state.borrow_mut();
        // 解码是异步的：期间可能又换了图（甚至清空了）。只安装**仍然是最新请求**
        // 的那一张，否则会把后选的图覆盖成先选的。
        if st.background.requested.as_deref() != Some(data_url.as_str()) {
            return;
        }
        let FrameState {
            background, gpu, ..
        } = &mut *st;
        match decoded {
            Some((target_w, target_h, rgba)) => {
                background.install(gpu.device(), gpu.queue(), target_w, target_h, &rgba);
                background.applied = Some(data_url);
            }
            None => {
                // 坏图 / 非图片：退回纯色底，而不是留一张上一张图。
                background.image = None;
                background.applied = None;
            }
        }
    });
}

/// dataURL → RGBA8 像素。用浏览器的解码器（PNG/JPEG/WebP 全包），
/// 避免在 Rust 侧引入图像解码依赖。
async fn decode_to_rgba(data_url: &str) -> Option<(u32, u32, Vec<u8>)> {
    let bytes = stage_bg::base64_decode_data_url(data_url)?;
    if bytes.is_empty() {
        return None;
    }
    let window = web_sys::window()?;
    let array = js_sys::Uint8Array::from(bytes.as_slice());
    let parts = js_sys::Array::new();
    parts.push(array.as_ref());
    let bag = web_sys::BlobPropertyBag::new();
    bag.set_type("image/*");
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(parts.as_ref(), &bag).ok()?;
    let promise = window.create_image_bitmap_with_blob(&blob).ok()?;
    let value: JsValue = JsFuture::from(promise).await.ok()?;
    let bitmap: web_sys::ImageBitmap = value.dyn_into().ok()?;
    let width = bitmap.width();
    let height = bitmap.height();
    if width == 0 || height == 0 {
        return None;
    }
    // 限长边：大图按比例缩到 MAX_BACKGROUND_SIDE 以内再上传。
    let (target_w, target_h) = stage_bg::fit_within(width, height, stage_bg::MAX_BACKGROUND_SIDE);
    let canvas = web_sys::OffscreenCanvas::new(target_w, target_h).ok()?;
    let context = canvas.get_context("2d").ok()??;
    let context: web_sys::OffscreenCanvasRenderingContext2d = context.dyn_into().ok()?;
    context
        .draw_image_with_image_bitmap_and_dw_and_dh(
            &bitmap,
            0.0,
            0.0,
            f64::from(target_w),
            f64::from(target_h),
        )
        .ok()?;
    let data = context
        .get_image_data(0.0, 0.0, f64::from(target_w), f64::from(target_h))
        .ok()?;
    let rgba: Vec<u8> = data.data().0;
    if rgba.len() != (target_w as usize) * (target_h as usize) * 4 {
        return None;
    }
    Some((target_w, target_h, rgba))
}
