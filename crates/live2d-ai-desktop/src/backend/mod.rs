//! 桌宠平台后端统一接口（winit-wgpu 单实现；Linux 桌宠窗口能力本批落地）。
//!
//! 设计约束：
//! - 本批唯一落地实现是 [`WinitWgpuBackend`]（winit 0.30 UserEvent 事件循环 +
//!   wgpu 29 surface + ksni 托盘）；
//! - **能力口径**：[`DesktopBackend::request_feature`] 只回答“代码路径是否存在”
//!   （静态声明）；实际能力一律以 [`RunReport::runtime`] 的真实初始化/API 调用
//!   结果为准——置顶/全局定位仅 RawWindowHandle 实测为 X11 时可用，点击穿透/
//!   拖动两种后端均可（winit 原生），托盘以 ksni spawn 确认为准；
//! - 后端返回的 [`RunReport::runtime`] 只能记录真实结果，
//!   不允许从会话线索推导；
//! - 终端 chat 模式（[`run_chat_session`]，见 `chat` 子模块）是独立 CLI 入口。

pub mod chat;
#[cfg(test)]
mod tests;

pub use chat::{ChatOptions, WINIT_WGPU_NOTES, create_backend, run_chat_session};

use std::path::PathBuf;
use std::time::Duration;

use crate::model_smoke::ModelSmokeStats;
use crate::platform::RuntimeCapabilities;

/// 后端标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// winit 0.30 事件循环 + wgpu 29 渲染 surface 的原生窗口壳。
    WinitWgpu,
}

impl std::fmt::Display for BackendKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            BackendKind::WinitWgpu => "winit-wgpu",
        })
    }
}

/// 当前可用的后端集合（后续阶段在此追加 X11 专项后端等）。
pub fn available_backends() -> &'static [BackendKind] {
    &[BackendKind::WinitWgpu]
}

/// 桌宠平台能力请求（静态声明口径；实际能力见 `RunReport.runtime`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureRequest {
    /// 窗口置顶。
    AlwaysOnTop,
    /// 客户端全局定位窗口。
    GlobalPosition,
    /// 鼠标点击穿透。
    ClickThrough,
    /// 交互态拖动窗口。
    DragMove,
    /// 系统托盘图标。
    Tray,
}

impl std::fmt::Display for FeatureRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            FeatureRequest::AlwaysOnTop => "always_on_top",
            FeatureRequest::GlobalPosition => "global_position",
            FeatureRequest::ClickThrough => "click_through",
            FeatureRequest::DragMove => "drag_move",
            FeatureRequest::Tray => "tray",
        })
    }
}

/// 能力请求被拒绝：该能力尚未在当前后端真正实现。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedFeature {
    pub feature: FeatureRequest,
    pub reason: &'static str,
}

/// 后端错误。区分“环境不满足”与“代码缺陷”，供 CLI 映射不同退出码。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    /// 运行环境不满足（无显示服务、无可用 GPU adapter/device 等），非代码错误。
    Environment(String),
    /// 后端内部错误（代码缺陷或意外平台行为）。
    Failed(String),
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Environment(msg) => write!(f, "运行环境不满足（非代码错误）: {msg}"),
            Self::Failed(msg) => write!(f, "后端内部错误: {msg}"),
        }
    }
}

impl std::error::Error for BackendError {}

/// 模型实时渲染冒烟的配置（`RunOptions::model_smoke` 为 `Some` 时启用）。
///
/// `None` = 纯透明窗口壳（`--window-smoke`，不加载任何 Live2D 资产）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSmokeOptions {
    /// model3.json 清单路径（皮套包加载入口）。
    pub model3_path: PathBuf,
}

/// 窗口壳运行参数。
#[derive(Debug, Clone)]
pub struct RunOptions {
    /// 初始逻辑尺寸 `(width, height)`。
    pub initial_logical_size: (f64, f64),
    /// 连续渲染到该帧数后自动退出（CI 冒烟用；None = 直到窗口关闭）。
    pub frame_target: Option<u64>,
    /// 超时自动退出兜底；None 表示不限时（仅手动关闭退出）。
    pub timeout: Option<Duration>,
    /// Live2D 实时渲染冒烟配置；None = 纯透明清屏壳。
    pub model_smoke: Option<ModelSmokeOptions>,
    /// **桌宠模式**：默认置顶 + 点击穿透。
    ///
    /// 安全规则：托盘未确认就绪时**禁止**自动点击穿透（见
    /// `crate::user_event::plan_launch`）——穿透 + 无托盘恢复入口 = 不可恢复。
    pub pet_mode: bool,
}

impl RunOptions {
    /// 校验运行参数（纯函数，供单测与 run 前检查）。
    pub fn validate(&self) -> Result<(), String> {
        let (w, h) = self.initial_logical_size;
        if !(w.is_finite() && h.is_finite()) || w <= 0.0 || h <= 0.0 {
            return Err(format!(
                "initial_logical_size 必须为正有限值，得到 ({w}, {h})"
            ));
        }
        if let Some(n) = self.frame_target
            && n == 0
        {
            return Err("frame_target 必须 >= 1".to_string());
        }
        if let Some(model) = &self.model_smoke
            && model.model3_path.as_os_str().is_empty()
        {
            return Err("model_smoke.model3_path 不能为空".to_string());
        }
        Ok(())
    }

    /// 冒烟帧数模式下若未显式给超时，提供默认兜底超时（防止 CI 挂死）。
    pub fn effective_timeout(&self) -> Option<Duration> {
        self.timeout.or({
            // 有帧目标 ⇒ 视为无人值守冒烟：默认 60s 兜底。
            self.frame_target.map(|_| Duration::from_secs(60))
        })
    }
}

/// 窗口壳停止原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// 用户/系统关闭了窗口。
    WindowClosed,
    /// 达成冒烟帧数目标。
    FrameTargetReached(u64),
    /// 超时兜底触发。
    TimeoutElapsed(Duration),
    /// 托盘菜单请求退出（桌宠常驻模式的正常退出路径）。
    TrayExit,
}

impl std::fmt::Display for StopReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WindowClosed => f.write_str("window-closed"),
            Self::FrameTargetReached(n) => write!(f, "frame-target-reached({n})"),
            Self::TimeoutElapsed(d) => write!(f, "timeout-elapsed({d:.1?})"),
            Self::TrayExit => f.write_str("tray-exit"),
        }
    }
}

/// 一次窗口壳运行的报告（含真实 RuntimeCapabilities）。
#[derive(Debug, Clone)]
pub struct RunReport {
    pub stop_reason: StopReason,
    pub runtime: RuntimeCapabilities,
    /// 实际选中的 GPU adapter 描述（观测用；未初始化成功时为空串）。
    pub gpu_adapter: String,
    /// 成功 present 的帧数。
    pub frames_presented: u64,
    /// 因 Timeout/Occluded/Outdated 等跳过的帧数。
    pub frames_skipped: u64,
    pub wall_time: Duration,
    /// 模型实时渲染冒烟统计（纯透明壳运行时为 None）。
    pub model: Option<ModelSmokeStats>,
}

impl RunReport {
    /// 逐行摘要（CLI 打印用）。
    pub fn summarize_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("stop          : {}", self.stop_reason),
            format!(
                "gpu           : {}",
                if self.gpu_adapter.is_empty() {
                    "(n/a)"
                } else {
                    &self.gpu_adapter
                }
            ),
            format!(
                "frames        : presented={} skipped={}",
                self.frames_presented, self.frames_skipped
            ),
            format!("wall_time     : {:.2?}", self.wall_time),
            format!("runtime caps  : {}", self.runtime.summarize()),
        ];
        if let Some(model) = &self.model {
            lines.push("model smoke   :".to_owned());
            for line in model.summarize_lines() {
                lines.push(format!("  {line}"));
            }
        }
        lines
    }
}
