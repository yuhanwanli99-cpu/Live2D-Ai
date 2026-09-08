//! 共享 GPU 上下文：device / queue / adapter 观测信息的自有封装。
//!
//! 与渲染核心（[`super::ModelRendererCore`]）解耦：
//! - 同一 [`GpuContext`] 可被多个渲染核心/离屏画布共享（`Arc` 计数）；
//! - 既可 headless 自建设备（[`GpuContext::headless`]，vulkan→gl→all 回退，
//!   容器内通常落到软件光栅），也可用**已有 device/queue** 接入
//!   （[`GpuContext::from_parts`]，嵌入现有 wgpu 应用/窗口应用）。

use super::{RenderError, RenderTier};

/// 共享 GPU 上下文（廉价 [`Clone`] 语义经 `Arc` 在调用方持有）。
#[derive(Clone)]
pub struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_label: String,
}

impl GpuContext {
    /// headless 初始化：尝试 Vulkan → GL → 全部后端。
    ///
    /// 容器/无显卡环境通常落到软件光栅（功能验证有效，性能不代表真机）。
    /// 使用默认档位（[`RenderTier::DEFAULT`]）。
    pub fn headless() -> Result<Self, RenderError> {
        Self::headless_with_tier(RenderTier::DEFAULT)
    }

    /// 同 [`Self::headless`]，但按指定档位构造 `max_texture_dimension_2d`。
    ///
    /// 档位=4096/8192 时 wgpu `Limits::default()` 已满足；档位=16384 需把上限
    /// 显式设为 16384 才能建立对应尺寸的离屏 target。
    pub fn headless_with_tier(tier: RenderTier) -> Result<Self, RenderError> {
        let (adapter_label, device, queue) = init_headless_device(tier)?;
        Ok(Self {
            device,
            queue,
            adapter_label,
        })
    }

    /// 用已有的 device / queue 构造（嵌入现有 wgpu 应用；adapter 标注为 external）。
    pub fn from_parts(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        Self::from_parts_with_label(device, queue, "external")
    }

    /// 同 [`Self::from_parts`]，但可自定义 adapter 观测标注。
    pub fn from_parts_with_label(
        device: wgpu::Device,
        queue: wgpu::Queue,
        adapter_label: impl Into<String>,
    ) -> Self {
        Self {
            device,
            queue,
            adapter_label: adapter_label.into(),
        }
    }

    /// wgpu 设备。
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// wgpu 命令队列。
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// 实际使用的 GPU adapter 描述（观测用；外部接入时为调用方给的标注）。
    pub fn adapter_label(&self) -> &str {
        &self.adapter_label
    }
}

impl std::fmt::Debug for GpuContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuContext")
            .field("adapter", &self.adapter_label)
            .finish_non_exhaustive()
    }
}

/// headless 设备初始化（bakeoff 验证的回退顺序）。
fn init_headless_device(
    tier: RenderTier,
) -> Result<(String, wgpu::Device, wgpu::Queue), RenderError> {
    let attempts: &[(&str, wgpu::Backends)] = &[
        ("vulkan", wgpu::Backends::VULKAN),
        ("gl", wgpu::Backends::GL),
        ("all", wgpu::Backends::all()),
    ];
    let mut notes = Vec::new();

    for (label, backends) in attempts {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: *backends,
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions::default(),
            display: None,
        });
        let Ok(adapter) =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            }))
        else {
            notes.push(format!("{label}: no compatible adapter"));
            continue;
        };

        let info = adapter.get_info();
        let adapter_label = format!("{} ({:?}, {})", info.name, info.backend, info.driver_info);
        // 按档位构造 limits：只有 16384 档需要把 max_texture_dimension_2d 显式升高。
        let limits = wgpu::Limits {
            max_texture_dimension_2d: tier.required_max_texture_dimension(),
            ..wgpu::Limits::default()
        };
        match pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("l2d-headless"),
            required_features: wgpu::Features::empty(),
            required_limits: limits,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
        })) {
            Ok((device, queue)) => return Ok((adapter_label, device, queue)),
            Err(e) => notes.push(format!(
                "{label}: adapter {} failed request_device: {e}",
                info.name
            )),
        }
    }
    Err(RenderError::NoGpuBackend(notes.join("; ")))
}
