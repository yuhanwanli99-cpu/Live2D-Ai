//! benchmark 统计原语：`TimingStats` / `SkipBuckets` / `SkipKind` /
//! `ActionCounts`，以及 [`mode_caliber`] 等展示用纯函数。
//!
//! 全部为纯类型 + 纯函数，无 GPU / 无时钟依赖 → 单测零外部副作用。
//! 详细字段语义见 [`crate::benchmark`] 模块文档。

use serde::Serialize;

use super::BenchmarkMode;

// ============================================================== TimingStats

/// 计时统计：avg / p50 / p95 / max（毫秒）。
///
/// 实现口径：p50/p95 = 排序后 `[floor(N * q)]` 处样本（N ≥ 1 时为单点；
/// 0 个样本时全部为 0）；`avg` = 算术平均；`max` = 最大样本。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
pub struct TimingStats {
    pub avg: f64,
    pub p50: f64,
    pub p95: f64,
    pub max: f64,
}

impl TimingStats {
    /// 由毫秒样本集合计算（空样本 = 全 0）。
    pub fn from_samples_ms(samples_ms: &[f64]) -> Self {
        if samples_ms.is_empty() {
            return Self::default();
        }
        let mut sorted: Vec<f64> = samples_ms.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = sorted.len();
        let avg = sorted.iter().sum::<f64>() / n as f64;
        let max = *sorted.last().expect("non-empty by guard");
        let p50 = sorted[((n as f64 * 0.50).floor() as usize).min(n - 1)];
        let p95 = sorted[((n as f64 * 0.95).floor() as usize).min(n - 1)];
        Self { avg, p50, p95, max }
    }
}

// ============================================================== Skip 桶

/// Skip 原因分桶（与 `wgpu::CurrentSurfaceTexture` 5 个非 Success/Suboptimal
/// 变体 + Suboptimal 对应）。
///
/// `Success` 不计入 skipped；`Suboptimal` 单独成桶（present 了但需要重配）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct SkipBuckets {
    pub timeout: u64,
    pub occluded: u64,
    pub outdated: u64,
    pub lost: u64,
    pub suboptimal: u64,
    pub validation: u64,
}

impl SkipBuckets {
    /// 累加一个新事件。
    pub fn record(&mut self, kind: SkipKind) {
        match kind {
            SkipKind::Timeout => self.timeout += 1,
            SkipKind::Occluded => self.occluded += 1,
            SkipKind::Outdated => self.outdated += 1,
            SkipKind::Lost => self.lost += 1,
            SkipKind::Suboptimal => self.suboptimal += 1,
            SkipKind::Validation => self.validation += 1,
        }
    }

    /// 纯跳过帧数（**不含** Suboptimal——Suboptimal 实际 present 了，
    /// 仅触发 reconfigure，不算"未呈现"）。
    ///
    /// 终止条件 / "实际跳过多少帧" 语义用本字段；
    /// Suboptimal 单列在 [`Self::recovery_events_total`]。
    pub fn skipped_total(&self) -> u64 {
        self.timeout + self.occluded + self.outdated + self.lost + self.validation
    }

    /// 回收事件总数：Suboptimal（present+reconfigure）+ Outdated +
    /// Lost——这些都是需要 surface 层面"做点什么"的事件，单独列供
    /// 诊断观察 swapchain 健康度。
    ///
    /// 与 [`Self::skipped_total`] 不重叠：Suboptimal 同时在 `suboptimal`
    /// 桶与本字段，Outdated/Lost 同时在 `outdated` / `lost` 桶与本字段。
    pub fn recovery_events_total(&self) -> u64 {
        self.suboptimal + self.outdated + self.lost
    }

    /// 全部桶总和（含 Suboptimal 单独成桶）——**保留**旧语义，
    /// 给需要"任意跳过相关事件"总数的诊断字段用。
    ///
    /// 协议收口 / 终止条件**禁止**依赖本字段（Suboptimal 会与
    /// `presented` 双计，导致提前结束）；用 [`Self::skipped_total`]
    /// 或 `formal_attempts` 收口。
    pub fn total(&self) -> u64 {
        self.timeout + self.occluded + self.outdated + self.lost + self.suboptimal + self.validation
    }
}

/// Skip 事件类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipKind {
    Timeout,
    Occluded,
    Outdated,
    Lost,
    /// Suboptimal 实际 present 了（仍记 presented += 1；本桶只观测
    /// "reconfigure 触发次数"）。
    Suboptimal,
    Validation,
}

impl SkipKind {
    /// 名称（字符串日志/报告用）。
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::Occluded => "occluded",
            Self::Outdated => "outdated",
            Self::Lost => "lost",
            Self::Suboptimal => "suboptimal",
            Self::Validation => "validation",
        }
    }
}

// ============================================================== Action 计数

/// 各 `SurfaceAction` 触发次数（与 `app::surface::SurfaceAction` 对齐；
/// benchmark 不复用 frame.rs 是因为要避免与生产路径交叉）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct ActionCounts {
    pub present: u64,
    pub present_then_reconfigure: u64,
    pub skip_and_reconfigure: u64,
    pub skip_and_recreate: u64,
    pub skip: u64,
}

// ============================================================== mode_caliber

/// 模式在 stdout 报告里的口径描述（供 [`super::BenchmarkReport::summarize_lines`]
/// 复用）。
pub(super) fn mode_caliber(m: BenchmarkMode) -> &'static str {
    match m {
        BenchmarkMode::Blocking => "包含 device.poll(Wait)，即 CPU 编码+提交+精确等本帧",
        BenchmarkMode::Submit => "仅 CPU 编码+提交（device.poll(Poll) 在帧末推进上传）",
    }
}

// ============================================================== 单测

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_buckets_total_sums_all_kinds() {
        let mut b = SkipBuckets::default();
        b.record(SkipKind::Timeout);
        b.record(SkipKind::Timeout);
        b.record(SkipKind::Occluded);
        b.record(SkipKind::Outdated);
        b.record(SkipKind::Lost);
        b.record(SkipKind::Suboptimal);
        b.record(SkipKind::Validation);
        assert_eq!(b.timeout, 2);
        assert_eq!(b.occluded, 1);
        assert_eq!(b.outdated, 1);
        assert_eq!(b.lost, 1);
        assert_eq!(b.suboptimal, 1);
        assert_eq!(b.validation, 1);
        // `total()` 保留全桶语义：7 事件全计（含 Suboptimal 单独成桶）。
        assert_eq!(b.total(), 7);
        // `skipped_total()`：**不**含 Suboptimal（present 了的回收事件
        // 不算"未呈现"）= 6。
        assert_eq!(b.skipped_total(), 6);
        // `recovery_events_total()`：Suboptimal + Outdated + Lost = 3。
        assert_eq!(b.recovery_events_total(), 3);
        for kind in [
            SkipKind::Timeout,
            SkipKind::Occluded,
            SkipKind::Outdated,
            SkipKind::Lost,
            SkipKind::Suboptimal,
            SkipKind::Validation,
        ] {
            assert!(!kind.as_str().is_empty());
        }
    }

    #[test]
    fn timing_stats_handles_empty_and_single_sample() {
        let s = TimingStats::from_samples_ms(&[]);
        assert_eq!(s.avg, 0.0);
        assert_eq!(s.p50, 0.0);
        assert_eq!(s.p95, 0.0);
        assert_eq!(s.max, 0.0);
        let s = TimingStats::from_samples_ms(&[12.0]);
        assert_eq!(s.avg, 12.0);
        assert_eq!(s.p50, 12.0);
        assert_eq!(s.p95, 12.0);
        assert_eq!(s.max, 12.0);
    }

    #[test]
    fn timing_stats_percentiles_use_floor_index_and_match_avg_max() {
        // 100 个样本 [0, 100)：p50 ≈ 50.0、p95 ≈ 95.0、avg ≈ 49.5、max = 99.0。
        let samples: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let s = TimingStats::from_samples_ms(&samples);
        assert!((s.p50 - 50.0).abs() < 0.5, "p50={}", s.p50);
        assert!((s.p95 - 95.0).abs() < 0.5, "p95={}", s.p95);
        assert!((s.avg - 49.5).abs() < 0.5, "avg={}", s.avg);
        assert!((s.max - 99.0).abs() < 0.5, "max={}", s.max);
    }

    #[test]
    fn timing_stats_does_not_panic_on_nan_inputs() {
        // 含 NaN 不应让整段 panic：partial_cmp 在 sort 中按 Equal 处理，
        // 故 NaN 不会被排到末尾；avg 算术和会传播 NaN（已知行为），max 取决于
        // 排序后 NaN 的位置。契约：**不** panic，**不**索引越界。
        let samples = vec![1.0, f64::NAN, 3.0, 2.0];
        let _ = TimingStats::from_samples_ms(&samples);
    }

    #[test]
    fn skip_buckets_default_is_all_zero() {
        let b = SkipBuckets::default();
        assert_eq!(b.total(), 0);
        assert_eq!(b.skipped_total(), 0);
        assert_eq!(b.recovery_events_total(), 0);
    }

    #[test]
    fn skip_buckets_separates_suboptimal_from_skipped_and_recovery() {
        // 7 事件：2 timeout + 1 occluded + 1 outdated + 1 lost + 1 suboptimal + 1 validation。
        // skipped_total = 6（不含 Suboptimal：present 了的回收事件）。
        // recovery_events_total = 3（Suboptimal + Outdated + Lost）。
        let mut b = SkipBuckets::default();
        b.record(SkipKind::Timeout);
        b.record(SkipKind::Timeout);
        b.record(SkipKind::Occluded);
        b.record(SkipKind::Outdated);
        b.record(SkipKind::Lost);
        b.record(SkipKind::Suboptimal);
        b.record(SkipKind::Validation);
        assert_eq!(b.skipped_total(), 6, "skipped_total 不含 Suboptimal");
        assert_eq!(
            b.recovery_events_total(),
            3,
            "recovery_events_total = suboptimal + outdated + lost"
        );
        // 三字段彼此互斥：skipped_total 与 recovery_events_total 共享
        // outdated/lost（不与 suboptimal 共享），合计 = skipped_total +
        // recovery_events_total - shared_count = 6 + 3 - 2 = 7 = total()。
        assert_eq!(b.total(), b.skipped_total() + b.recovery_events_total() - 2);
    }

    #[test]
    fn action_counts_default_is_all_zero() {
        let a = ActionCounts::default();
        assert_eq!(a.present, 0);
        assert_eq!(a.present_then_reconfigure, 0);
        assert_eq!(a.skip_and_reconfigure, 0);
        assert_eq!(a.skip_and_recreate, 0);
        assert_eq!(a.skip, 0);
    }

    #[test]
    fn mode_caliber_distinguishes_blocking_vs_submit() {
        assert!(mode_caliber(BenchmarkMode::Blocking).contains("Wait"));
        assert!(mode_caliber(BenchmarkMode::Submit).contains("仅 CPU 编码+提交"));
        // 互斥：blocking 不写 "仅 CPU 编码+提交"、submit 不含 "Wait"。
        assert!(!mode_caliber(BenchmarkMode::Blocking).contains("仅 CPU 编码+提交"));
        assert!(!mode_caliber(BenchmarkMode::Submit).contains("Wait"));
    }
}
