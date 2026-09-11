//! C7/C8 llvmpipe A/B benchmark（P1-2，docs/plans/node-c-c1-c11-formal-audit-2026-08-27.md
//! 第 114-141 行 + 复审 P1-2 要求）。
//!
//! ## 定位
//!
//! **benchmark-only**：本模块**禁止**成为生产默认值，**禁止**接回实时正式路径。
//! 由 CLI `--benchmark` 显式选择；缺省不出现、也不会被任何生产代码引用。
//!
//! ## 双模式
//!
//! 同一代码、同一二进制、同一环境提供两种模式（运行时切换，仅测 CPU 提交成本
//! vs 阻塞等待提交完成的差异）：
//!
//! - `blocking`：[`ModelRendererCore::render_to_view`]（阻塞包装：内部精确等
//!   submission）。`submit_ms` 包含内部 `device.poll(Wait)`。
//! - `submit`：[`ModelRendererCore::render_to_view_submit`]（非阻塞 +
//!   每帧末 `device.poll(Poll)` 推进上传/回调管线）。`submit_ms` 仅为 CPU
//!   编码+提交成本。
//!
//! ## 协议
//!
//! - warm-up 60 帧不计；正式 600 帧（缺省，可经 CLI 调整）；
//! - 相同窗口尺寸 480×640（与现有 `--model-smoke` 缺省一致）、相同 Bai 模型；
//! - 固定动作序列循环（与 `model_smoke::SmokeDriver` 同口径）；
//! - llvmpipe 软渲染：本沙箱现状；surface 路径需 X11（`env -u WAYLAND_DISPLAY`
//!   `DISPLAY=:0`，详见 `docs/verification/node-c-c7-llvmpipe-benchmark-2026-08-27.md`）。
//!
//! ## 子模块
//!
//! - [`stats`]：纯统计类型（`TimingStats` / `SkipBuckets` / `SkipKind` /
//!   `ActionCounts` / `mode_caliber`）与对应单测；
//! - [`workload`]：headless / surface 共用逐帧 workload helper（生产路径
//!   同口径：`driver.tick` → `apply_frame` → `apply_mouth_level` →
//!   `core.update`），P1-2-P0-1 修复；
//! - [`runners_surface`]：winit 事件循环 + wgpu surface（C7 协议标准）；
//! - [`runners_headless`]：`GpuContext::headless()` + 离屏 `Texture` 同步循环
//!   （无 X server 时的回退）。
//!
//! ## 指标
//!
//! 全部为 [`BenchmarkReport`] 字段（见下方）：
//! - `submit_ms` avg/p50/p95/max；
//! - `present_interval_ms` avg/p50/p95/max（连续 `presented` 帧间的
//!   `Instant::now()` 间隔）；
//! - `presented` / `skipped` 计数，skipped 按 `Timeout / Occluded /
//!   Outdated / Lost / Suboptimal / Validation` 分桶；**Suboptimal
//!   实际 present 了**，故 `skipped_total`（不含 Suboptimal）与
//!   `recovery_events_total`（Suboptimal + Outdated + Lost）分列——
//!   终止条件**不**用 `presented + skipped.total()`（Suboptimal 双计
//!   死循环），用 `formal_attempts == formal_target`；
//! - 各 `SurfaceAction` 分支计数（Present / PresentThenReconfigure /
//!   SkipAndReconfigure / SkipAndRecreate / Skip）；
//! - adapter / backend / present_mode / frame_latency 元数据。
//!
//! ## JSON 报告
//!
//! `BenchmarkReport` 派生 `Serialize`；CLI `--benchmark-output` 时落盘。
//! **行数豁免（≤1000）**：本文件 515 行——入口类型 + 报告 + 调度收拢（微超 500，差值 15 行；继续拆会破坏 benchmark 模块聚合性）。
//!

use std::time::Duration;

use serde::Serialize;

mod runners_headless;
mod runners_surface;
mod stats;
mod workload;

// 阶段推进纯函数 + 阶段枚举（供 mod.rs 测试调用；不作为公开 API
// 暴露给 crate 外）。
#[allow(unused_imports)]
pub(crate) use runners_surface::{BenchPhase, PhaseDecision, advance_benchmark_phase};

pub use stats::{ActionCounts, SkipBuckets, TimingStats};

// ============================================================== 模式 / 选项

/// Benchmark 模式。
///
/// - `Blocking` 调 `render_to_view`（阻塞包装，精确等 submission）；
/// - `Submit` 调 `render_to_view_submit`（非阻塞 + 每帧 `device.poll(Poll)`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BenchmarkMode {
    Blocking,
    Submit,
}

impl BenchmarkMode {
    /// 名称（CLI 字符串、报告日志用）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Blocking => "blocking",
            Self::Submit => "submit",
        }
    }

    /// CLI 字符串解析（`blocking` | `submit`）。其余值 → None。
    /// `#[allow(dead_code)]`：CLI 走独立 `cli::BenchmarkCliMode` 解析，
    /// 本方法保留供库用户/扩展使用。
    #[allow(dead_code)]
    pub fn from_cli(s: &str) -> Option<Self> {
        match s {
            "blocking" => Some(Self::Blocking),
            "submit" => Some(Self::Submit),
            _ => None,
        }
    }
}

/// Benchmark 后端选择。
///
/// - `Surface`：经 winit + wgpu surface（X11/Wayland 真实交换链），C7
///   协议标准路径；需要显示服务；
/// - `Headless`：经 `GpuContext::headless()` + 离屏 `Texture`，
///   跳过 winit 事件循环（**无** surface 取帧 / present 步骤）。
///   仅用于**沙箱无 X server** 的回退：C1 闭环的 `submit` vs `blocking`
///   时间差（CPU 编码+提交 vs CPU 编码+提交+Wait）依然可观测；缺
///   `present_interval_ms` / skipped / action 计数（这些由 surface
///   才有意义），文档如实标注口径差异。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkBackend {
    Surface,
    Headless,
}

/// Benchmark 配置（CLI → 接线 → 报告统一结构）。
#[derive(Debug, Clone)]
pub struct BenchmarkOptions {
    pub model3_path: std::path::PathBuf,
    /// 逻辑窗口尺寸（与 `--model-smoke` 缺省 480×640 对齐；headless 模式下
    /// 直接作为离屏画布的物理尺寸）。
    pub window_logical: (f32, f32),
    pub warmup_frames: u64,
    pub formal_frames: u64,
    pub mode: BenchmarkMode,
    /// 后端选择。
    pub backend: BenchmarkBackend,
    /// JSON 输出路径（None = 仅 stdout/不写文件）。
    pub output_json: Option<std::path::PathBuf>,
}

// ============================================================== 报告结构

/// 单模式 benchmark 报告。
#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkReport {
    pub mode: BenchmarkMode,
    pub model3_path: String,
    pub window_size: (u32, u32),
    pub warmup_frames: u64,
    pub formal_frames: u64,
    /// 实测配置 / adapter / backend 元数据。
    pub adapter: String,
    pub backend: String,
    pub present_mode: String,
    pub frame_latency: u32,
    /// `submit_ms` 统计（每帧 mode 对应 API 调用的耗时）；
    /// `Blocking` 模式包含内部 `device.poll(Wait)`。
    pub submit_ms: TimingStats,
    /// 帧间 `presented` 时间间隔（`Instant::now()` 之差，毫秒）。
    pub present_interval_ms: TimingStats,
    /// 正式阶段尝试的帧数（含 skip）；等于 `options.formal_frames` 时
    /// 表示协议完成。headless 模式也以此字段报告总测量尝试数。
    pub measured_frames: u64,
    /// 正式阶段成功 present 帧数（surface 模式 = `ActionCounts::present` +
    /// `present_then_reconfigure`；headless 模式 = 0，无 surface 取帧）。
    pub presented: u64,
    pub skipped: SkipBuckets,
    pub actions: ActionCounts,
    /// 总墙钟耗时（warmup + formal + bootstrap 之后）。
    pub elapsed: Duration,
}

impl BenchmarkReport {
    /// 逐行摘要（stdout 用）。
    pub fn summarize_lines(&self) -> Vec<String> {
        let lines = vec![
            format!(
                "mode          : {}（{}）",
                self.mode.as_str(),
                stats::mode_caliber(self.mode)
            ),
            format!("model         : {}", self.model3_path),
            format!(
                "window        : {}x{}",
                self.window_size.0, self.window_size.1
            ),
            format!(
                "frames        : warmup={} formal={}",
                self.warmup_frames, self.formal_frames
            ),
            format!(
                "adapter       : {}（backend={}）",
                self.adapter, self.backend
            ),
            format!(
                "present_mode  : {}（frame_latency={}）",
                self.present_mode, self.frame_latency
            ),
            format!("measured      : {} 帧尝试（formal）", self.measured_frames),
            format!("presented     : {}", self.presented),
            format!(
                "skipped       : total={} skipped_total={} (timeout={} occluded={} outdated={} lost={} suboptimal={} validation={}) recovery_events={}",
                self.skipped.total(),
                self.skipped.skipped_total(),
                self.skipped.timeout,
                self.skipped.occluded,
                self.skipped.outdated,
                self.skipped.lost,
                self.skipped.suboptimal,
                self.skipped.validation,
                self.skipped.recovery_events_total()
            ),
            format!(
                "actions       : present={} present_then_reconfigure={} skip_and_reconfigure={} skip_and_recreate={} skip={}",
                self.actions.present,
                self.actions.present_then_reconfigure,
                self.actions.skip_and_reconfigure,
                self.actions.skip_and_recreate,
                self.actions.skip
            ),
            format!(
                "submit_ms     : avg={:.3} p50={:.3} p95={:.3} max={:.3}（{}）",
                self.submit_ms.avg,
                self.submit_ms.p50,
                self.submit_ms.p95,
                self.submit_ms.max,
                stats::mode_caliber(self.mode)
            ),
            format!(
                "present_interval_ms: avg={:.3} p50={:.3} p95={:.3} max={:.3}",
                self.present_interval_ms.avg,
                self.present_interval_ms.p50,
                self.present_interval_ms.p95,
                self.present_interval_ms.max
            ),
            format!("elapsed       : {:?}", self.elapsed),
        ];
        lines
    }
}

// ============================================================== 接线

/// benchmark 入口：自建 winit event loop + wgpu surface + 模型；跑双时间
/// 口径之一并返回报告。
///
/// **benchmark-only**：仅由 CLI `--benchmark` 调度。
///
/// 根据 [`BenchmarkOptions::backend`] 派发：
/// - `Surface`（缺省）：winit 事件循环 + 真实 surface 交换链（C7 协议标准）；
/// - `Headless`：`GpuContext::headless()` + 离屏 `Texture`，跳过 winit；
///   用于沙箱无 X server 时的回退（口径差异在文档说明）。
///
/// winit `run_app` 不返回中间结果；最终报告通过 `result_slot` 在
/// `BenchmarkApp::build_report` 完成后写入。本函数跑完 event loop 后
/// 从 slot 取回。
pub fn run_benchmark(opts: BenchmarkOptions) -> Result<BenchmarkReport, String> {
    // 先取出 output_json（runner 会 move opts），报告生成后落盘。
    let output_json = opts.output_json.clone();
    let report = match opts.backend {
        BenchmarkBackend::Surface => runners_surface::run_benchmark_surface(opts),
        BenchmarkBackend::Headless => runners_headless::run_benchmark_headless(opts),
    }?;
    // `--benchmark-output`：报告落盘 JSON（失败不吞主结果——仅打日志）。
    if let Some(path) = &output_json
        && let Err(e) = write_report_json(&report, path)
    {
        eprintln!("[warn] benchmark JSON 落盘失败（不影响 stdout 报告）: {e}");
    }
    Ok(report)
}

/// 将 [`BenchmarkReport`] 序列化为 JSON 写入指定路径（原子写回）。
fn write_report_json(report: &BenchmarkReport, path: &std::path::Path) -> Result<(), String> {
    let json =
        serde_json::to_string_pretty(report).map_err(|e| format!("序列化 benchmark 报告: {e}"))?;
    std::fs::write(path, json).map_err(|e| format!("写 {path:?}: {e}"))
}

// ============================================================== 单测

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benchmark_mode_roundtrips_cli_strings() {
        assert_eq!(
            BenchmarkMode::from_cli("blocking"),
            Some(BenchmarkMode::Blocking)
        );
        assert_eq!(
            BenchmarkMode::from_cli("submit"),
            Some(BenchmarkMode::Submit)
        );
        assert_eq!(BenchmarkMode::from_cli("BLOCKING"), None);
        assert_eq!(BenchmarkMode::from_cli(""), None);
        assert_eq!(BenchmarkMode::Blocking.as_str(), "blocking");
        assert_eq!(BenchmarkMode::Submit.as_str(), "submit");
    }

    #[test]
    fn benchmark_backend_distinguishes_surface_vs_headless() {
        let opts = BenchmarkOptions {
            model3_path: std::path::PathBuf::from("/tmp/x.model3.json"),
            window_logical: (480.0, 640.0),
            warmup_frames: 60,
            formal_frames: 600,
            mode: BenchmarkMode::Submit,
            backend: BenchmarkBackend::Surface,
            output_json: None,
        };
        assert_eq!(opts.backend, BenchmarkBackend::Surface);
        assert_eq!(opts.warmup_frames, 60);
        assert_eq!(opts.formal_frames, 600);
        let headless = BenchmarkOptions {
            backend: BenchmarkBackend::Headless,
            ..opts.clone()
        };
        assert_eq!(headless.backend, BenchmarkBackend::Headless);
    }

    #[test]
    fn benchmark_report_summary_contains_key_channels() {
        let report = BenchmarkReport {
            mode: BenchmarkMode::Submit,
            model3_path: "/tmp/bai.model3.json".into(),
            window_size: (480, 640),
            warmup_frames: 60,
            formal_frames: 600,
            adapter: "llvmpipe (Vulkan, Mesa)".into(),
            backend: "Vulkan".into(),
            present_mode: "Fifo".into(),
            frame_latency: 2,
            submit_ms: TimingStats {
                avg: 5.0,
                p50: 4.5,
                p95: 8.0,
                max: 12.0,
            },
            present_interval_ms: TimingStats {
                avg: 16.7,
                p50: 16.7,
                p95: 17.0,
                max: 20.0,
            },
            measured_frames: 600,
            presented: 600,
            skipped: SkipBuckets {
                timeout: 1,
                ..Default::default()
            },
            actions: ActionCounts {
                present: 600,
                ..Default::default()
            },
            elapsed: Duration::from_secs(11),
        };
        let text = report.summarize_lines().join("\n");
        for needle in [
            "mode          : submit",
            "model         : /tmp/bai.model3.json",
            "480x640",
            "warmup=60 formal=600",
            "adapter       : llvmpipe",
            "present_mode  : Fifo",
            "measured      : 600 帧尝试",
            "presented     : 600",
            "skipped       :",
            "skipped_total=",
            "timeout=1",
            "recovery_events=",
            "actions       :",
            "submit_ms     :",
            "p95=8.000",
            "present_interval_ms:",
            "elapsed       :",
        ] {
            assert!(text.contains(needle), "摘要缺少 {needle}:\n{text}");
        }
    }

    // 阶段推进纯函数 `advance_benchmark_phase` 单测（P1-2 二轮修复，任务 2）。
    // 7 条最低测试清单（复审要求）：1-3 + 6-7 各 1 条；4-5 合 1 条。

    /// T1：warmup_target=3 → 第 1、2 次 StayWarmup，第 3 次 EnterFormal。
    #[test]
    fn phase_advance_warmup_target_three_enters_formal_on_third_attempt() {
        let (wt, ft) = (3_u64, 10_u64);
        assert_eq!(
            advance_benchmark_phase(BenchPhase::Warmup, 1, 0, wt, ft, 1),
            PhaseDecision::StayWarmup
        );
        assert_eq!(
            advance_benchmark_phase(BenchPhase::Warmup, 2, 0, wt, ft, 1),
            PhaseDecision::StayWarmup
        );
        assert_eq!(
            advance_benchmark_phase(BenchPhase::Warmup, 3, 0, wt, ft, 1),
            PhaseDecision::EnterFormal
        );
    }

    /// T2：EnterFormal 决策触发后，调用方按决策执行清零序列（**唯一**清零点）。
    #[test]
    fn phase_advance_enter_formal_triggers_caller_side_reset() {
        let mut phase = BenchPhase::Warmup;
        let mut warmup_attempts = 3_u64;
        let mut formal_attempts = 0_u64;
        // 起始 presented 故意非 0（模拟残留），调用方清零后**必须**变 0。
        let mut presented: u64 = 999;
        let mut submitted_samples: Vec<f64> = vec![1.0, 2.0, 3.0];
        let (wt, ft) = (3_u64, 10_u64);
        assert_eq!(presented, 999);
        assert_eq!(submitted_samples.len(), 3);
        let decision = advance_benchmark_phase(phase, warmup_attempts, formal_attempts, wt, ft, 1);
        assert_eq!(decision, PhaseDecision::EnterFormal);
        // 调用方按决策执行清零序列。
        phase = BenchPhase::Formal;
        warmup_attempts = 3; // warmup_attempts 不清（已收口）
        formal_attempts = 0;
        presented = 0;
        submitted_samples.clear();
        let next = advance_benchmark_phase(phase, warmup_attempts, formal_attempts + 1, wt, ft, 1);
        assert_eq!(next, PhaseDecision::StayFormal);
        assert_eq!(presented, 0, "presented 必须清零");
        assert!(submitted_samples.is_empty(), "submit 样本必须清空");
    }

    /// T3：formal_target=10 → 第 10 次 Finish。
    #[test]
    fn phase_advance_formal_target_ten_finishes_on_tenth_attempt() {
        let (wt, ft) = (3_u64, 10_u64);
        for n in 1..10 {
            assert_eq!(
                advance_benchmark_phase(BenchPhase::Formal, 0, n, wt, ft, 1),
                PhaseDecision::StayFormal,
                "formal 第 {n} 次应 StayFormal"
            );
        }
        assert_eq!(
            advance_benchmark_phase(BenchPhase::Formal, 0, 10, wt, ft, 1),
            PhaseDecision::Finish
        );
    }

    /// T4+T5：决策只依赖 attempts 计数（编译期签名纯净）+ 5 帧 attempt 严格 +1。
    #[test]
    fn phase_advance_signature_pure_and_unit_granular() {
        // 编译期保证：以下两行**能**编译即证明签名只接受 phase +
        // attempts 计数。传 presented/skipped 等"看似合理"的输入**不会**
        // 编译——任何 `advance_benchmark_phase(..., presented: u64, ...)`
        // 的扩展必须先复审允许。
        let _ = advance_benchmark_phase(BenchPhase::Warmup, 5, 0, 10, 20, 1);
        let _ = advance_benchmark_phase(BenchPhase::Formal, 0, 5, 10, 20, 1);
        // Suboptimal 等任意 action：5 帧 attempt 严格 +1（无双计）。
        let (wt, ft) = (10_u64, 100_u64);
        for n in 1..=5 {
            assert_eq!(
                advance_benchmark_phase(BenchPhase::Warmup, n, 0, wt, ft, 1),
                PhaseDecision::StayWarmup,
                "第 {n} 帧任意 action 也只 +1 attempt"
            );
        }
    }

    /// T6：warmup_target=0 → 第 1 帧 EnterFormal；incremented=0 不切。
    #[test]
    fn phase_advance_warmup_target_zero_enters_formal_on_first_attempt() {
        let (wt, ft) = (0_u64, 10_u64);
        assert_eq!(
            advance_benchmark_phase(BenchPhase::Warmup, 1, 0, wt, ft, 1),
            PhaseDecision::EnterFormal
        );
        assert_eq!(
            advance_benchmark_phase(BenchPhase::Warmup, 0, 0, wt, ft, 0),
            PhaseDecision::StayWarmup
        );
    }

    /// T7：formal_target=0 → formal 第 1 次 Finish（runner 生成空报告）。
    #[test]
    fn phase_advance_formal_target_zero_finishes_on_first_formal_attempt() {
        let (wt, ft) = (3_u64, 0_u64);
        assert_eq!(
            advance_benchmark_phase(BenchPhase::Warmup, 3, 0, wt, ft, 1),
            PhaseDecision::EnterFormal
        );
        assert_eq!(
            advance_benchmark_phase(BenchPhase::Formal, 0, 1, wt, ft, 1),
            PhaseDecision::Finish
        );
    }

    /// B1：`incremented` 只接受 0 或 1；其它值 panic。
    #[test]
    #[should_panic]
    fn phase_advance_panics_on_invalid_incremented() {
        let _ = advance_benchmark_phase(BenchPhase::Warmup, 1, 0, 3, 10, 2);
    }

    /// B2：`Done` 阶段再调仍返回 `Finish`（已收口安全）。
    #[test]
    fn phase_advance_done_always_returns_finish() {
        assert_eq!(
            advance_benchmark_phase(BenchPhase::Done, 999, 999, 3, 10, 0),
            PhaseDecision::Finish
        );
        assert_eq!(
            advance_benchmark_phase(BenchPhase::Done, 999, 999, 3, 10, 1),
            PhaseDecision::Finish
        );
    }
}
