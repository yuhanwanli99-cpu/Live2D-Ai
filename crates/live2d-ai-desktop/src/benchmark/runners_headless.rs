//! benchmark headless 路径（C7 沙箱无 X server 时的回退）。
//!
//! - `GpuContext::headless()` 自建 GPU 上下文；
//! - 离屏 `wgpu::Texture` 当作 `render_to_view` 的 target；
//! - 同步循环跑 N 帧（无 winit 事件循环），每帧按 [`super::BenchmarkMode`] 调
//!   对应 API。
//!
//! **口径差异（必读）**：
//! - 没有 wgpu surface → **无** `present_interval_ms` / skipped / action 桶
//!   （这些是 surface 才有意义的指标）；
//! - `submit_ms` 仍可直接对比：`submit` = CPU 编码+提交；
//!   `blocking` = CPU 编码+提交+精确 `Wait`；
//! - 报告 `backend` / `present_mode` 字段分别记 `headless` / `n/a`。
//!
//! workload：每帧经 [`super::workload::step_frame_workload`] 推进表演
//! 曲线 + 写入 input 层 + 写口型 override + 推 `core.update`，与生产
//! [`crate::app::frame::draw_and_present_model`] 顺序一致（P1-2-P0-1 修复）。
//!
//! 统计：warm-up 与 formal **完全隔离**（P1-2-P0-3 修复）——
//! `formal_attempts` / `formal_presented` / `formal_submit_ms` 在进入
//! Formal 时从 0 起算；终止条件 `formal_attempts == options.formal_frames`。
//!
//! presented 诚实标注：headless 没有 surface 取帧，**不**伪称
//! `presented > 0`（P1-2-P1-4 修复）——`measured_frames` 等于正式尝试
//! 帧数，`presented = 0`。

use std::sync::Arc;
use std::time::Instant;

use l2d::asset::ModelPackage;
use l2d::model::LoadedModel;
use l2d::renderer::{GpuContext, ModelRendererCore};
use tracing::info;

use crate::adapter::BaiParamAdapter;

use super::stats::TimingStats;
use super::workload::step_frame_workload;
use super::{BenchmarkMode, BenchmarkOptions, BenchmarkReport};

/// headless 模式 benchmark 入口。
pub fn run_benchmark_headless(opts: BenchmarkOptions) -> Result<BenchmarkReport, String> {
    // 1. headless GPU 上下文。
    let gpu = Arc::new(GpuContext::headless().map_err(|e| format!("headless GPU 初始化: {e}"))?);
    let adapter_label = gpu.adapter_label().to_string();
    let device = gpu.device().clone();
    // `queue` 经 `gpu` 持有（`device.poll` 推进其上传队列），不必再克隆：
    // 整段循环里仅靠 `device.poll(Poll)` 推进上传/回调管线（C1 闭环同口径）。
    let _ = gpu.queue();

    // 2. 离屏目标纹理。
    let width = opts.window_logical.0.max(1.0) as u32;
    let height = opts.window_logical.1.max(1.0) as u32;
    let target = create_offscreen_target(&device, width, height, "l2d-bench-headless-target");

    // 3. 加载模型。
    let package =
        ModelPackage::load(&opts.model3_path).map_err(|e| format!("加载皮套包失败: {e}"))?;
    let loaded = LoadedModel::resolve(package).map_err(|e| format!("解析皮套失败: {e}"))?;
    let mut core =
        ModelRendererCore::new(Arc::clone(&gpu)).map_err(|e| format!("创建渲染核心失败: {e}"))?;
    core.load_model(&loaded)
        .map_err(|e| format!("load_model: {e}"))?;
    core.set_viewport(width, height)
        .map_err(|e| format!("set_viewport: {e}"))?;
    info!(adapter = %adapter_label, "benchmark headless 渲染核心就绪");

    // 4. workload 状态：driver / adapter / frame_buf / sim_time 在整个
    //    warm-up + formal 期间持续推进，**不**因阶段切换重置
    //    （P1-2-P0-1：与生产路径 1:1）。
    let mut driver = crate::model_smoke::SmokeDriver::new();
    let mut adapter = BaiParamAdapter::new();
    let mut frame_buf = live2d_ai_core::ParameterFrame::neutral();
    let mut sim_time = 0.0_f32;

    // 5. 阶段状态：warm-up 与 formal 完全隔离（P1-2-P0-3 修复）。
    // 正式阶段：以下两个量从 0 起算。
    let mut formal_attempts: u64 = 0;
    let mut formal_submit_samples: Vec<f64> = Vec::new();

    let run_started = Instant::now();
    let total = opts.warmup_frames.saturating_add(opts.formal_frames);
    for frame_idx in 0..total {
        // 5.1 阶段判定：warm-up 帧不计 attempts/formal 样本。
        let is_warmup = frame_idx < opts.warmup_frames;
        if is_warmup {
            // warm-up 帧：仅推进 workload + 渲染，不计 attempts。
        } else if formal_attempts < opts.formal_frames {
            formal_attempts += 1;
        } else {
            // formal 已收口：理论上前一循环应已退出；保险跳出。
            break;
        }

        // 5.2 推进 workload（生产路径同口径）。
        if let Err(e) = step_frame_workload(
            &mut driver,
            &mut adapter,
            &mut core,
            &mut frame_buf,
            &mut sim_time,
        ) {
            return Err(format!("workload step 失败: {e}"));
        }

        // 5.3 渲染：模式对应 API 调用的耗时。
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let started = Instant::now();
        let outcome = match opts.mode {
            BenchmarkMode::Submit => core
                .render_to_view_submit(&view, target.format())
                .map(|_| ())
                .map_err(|e| format!("render_to_view_submit: {e}")),
            BenchmarkMode::Blocking => core
                .render_to_view(&view, target.format())
                .map_err(|e| format!("render_to_view: {e}")),
        };
        let submit_ms = started.elapsed().as_secs_f64() * 1000.0;
        if let Err(e) = device.poll(wgpu::PollType::Poll) {
            return Err(format!("device.poll: {e:?}"));
        }
        outcome?;

        // 5.4 仅 formal 阶段累计 submit 样本。
        if !is_warmup {
            formal_submit_samples.push(submit_ms);
        }
    }
    let elapsed = run_started.elapsed();
    let submit_stats = TimingStats::from_samples_ms(&formal_submit_samples);
    // headless 协议收口：formal_attempts == options.formal_frames（终止条件）。
    let measured_frames = formal_attempts;
    Ok(BenchmarkReport {
        mode: opts.mode,
        model3_path: opts.model3_path.display().to_string(),
        window_size: (width, height),
        warmup_frames: opts.warmup_frames,
        formal_frames: opts.formal_frames,
        adapter: adapter_label,
        backend: "headless (GpuContext::headless)".into(),
        present_mode: "n/a".into(),
        frame_latency: 0,
        submit_ms: submit_stats,
        // headless 没有 surface → present_interval / skipped / actions 全部为 0。
        present_interval_ms: TimingStats::default(),
        measured_frames,
        // headless 诚实：presented = 0（无 surface 取帧，P1-2-P1-4 修复）。
        presented: 0,
        skipped: super::stats::SkipBuckets::default(),
        actions: super::stats::ActionCounts::default(),
        elapsed,
    })
}

/// 创建离屏目标纹理（`Rgba8Unorm`、单 mip、与 `l2d::offscreen::create_target`
/// 同口径但不依赖 ayagami 私有 Texture 类型——直接用 wgpu 公共 API）。
fn create_offscreen_target(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    label: &str,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}
