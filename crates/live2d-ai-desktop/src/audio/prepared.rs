//! 已转换、待入环的 PCM（P0-2）+ 一次入环的明确结果 + 冒烟报告值对象。
//!
//! - [`PreparedPcm`]：净化 + 设备域转换**恰一次**；可跨多次
//!   `try_enqueue_prepared` 续写，`WouldBlock` 只推进游标不丢样本。
//! - [`EnqueueOutcome`] / [`TryEnqueue`]：入环/续写结果（溢出可观测，不阻塞）。
//! - [`AudioSmokeReport`]：`--audio-smoke` 报告值对象（无设备时为 skip 语义）。

use std::time::Duration;

use live2d_ai_runtime::{AudioSpec, convert_spec};

use crate::audio::output::DeviceSummary;

/// 一次 [`PlaybackHandle::enqueue_pcm_f32`] 的明确结果（溢出可观测，不阻塞）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnqueueOutcome {
    /// 输入的源域样本数。
    pub source_samples: usize,
    /// 转换后的设备域样本数。
    pub device_samples: usize,
    /// 实际入环的设备域样本数（整帧对齐）。
    pub accepted: usize,
    /// 因环形缓冲满而被拒绝的设备域样本数 = `device_samples - accepted`。
    pub rejected: usize,
}

impl EnqueueOutcome {
    /// 是否有样本因溢出被拒绝。
    pub fn overflowed(&self) -> bool {
        self.rejected > 0
    }
}

/// 已转换、待入环的 PCM（P0-2）：净化+设备域转换**恰一次**；可跨多次
/// `try_enqueue_prepared` 续写，`WouldBlock` 只推进游标不丢样本。
#[derive(Debug)]
pub struct PreparedPcm {
    pub(crate) converted: Vec<f32>,
    pub(crate) written: usize,
}

/// 一次 [`PlaybackHandle::try_enqueue_prepared`] 的结果。
///
/// B2-P0-2：携带**本次实际写入样本数**——`WouldBlock{accepted>0}` 表示
/// 部分写入同样构成「播放已经开始」的事实。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TryEnqueue {
    /// 全部剩余样本已入环。
    Enqueued {
        /// 本次实际写入的设备域样本数。
        accepted: usize,
    },
    /// 未全部写完（可能已部分写入）；重试同一 prepared 实例。
    WouldBlock {
        /// 本次实际写入的设备域样本数。
        accepted: usize,
    },
}

impl TryEnqueue {
    /// 本次是否发生了任何样本入环。
    pub const fn any_accepted(&self) -> bool {
        matches!(
            self,
            Self::Enqueued { accepted } | Self::WouldBlock { accepted } if *accepted > 0
        )
    }
}

impl PreparedPcm {
    /// 从源规格构造：净化 + 设备域转换各恰一次。空输入合法。
    pub fn from_source(samples: &[f32], source: AudioSpec, device: AudioSpec) -> Self {
        let clean: Vec<f32> = samples
            .iter()
            .map(|&s| if s.is_finite() { s } else { 0.0 })
            .collect();
        let converted = if source == device {
            clean
        } else {
            convert_spec(&clean, source, device)
        };
        Self {
            converted,
            written: 0,
        }
    }

    /// 剩余待写样本数。
    #[allow(dead_code)]
    pub fn remaining(&self) -> usize {
        self.converted.len() - self.written
    }

    /// 是否已全部入环。
    #[allow(dead_code)]
    pub fn is_fully_written(&self) -> bool {
        self.written >= self.converted.len()
    }
}

/// `--audio-smoke` 冒烟报告（逐行摘要供 CLI 打印）。
#[derive(Debug, Clone)]
pub struct AudioSmokeReport {
    /// 设备摘要（None 表示环境无设备，本次为 skip 语义）。
    pub device: Option<DeviceSummary>,
    /// 源规格（TTS 默认域）。
    pub source_spec: AudioSpec,
    /// 生成的源域样本总数。
    pub generated_source_samples: usize,
    /// 实际接受的设备域样本总数。
    pub accepted_device_samples: u64,
    /// 因溢出被拒的设备域样本总数。
    pub rejected_device_samples: u64,
    /// 排空等待结果：true = 在超时前排空。
    pub drained: bool,
    /// 排空等待耗时。
    pub drain_waited: Duration,
    /// 播放期间观测到的 mouth level 峰值。
    pub peak_mouth: f32,
    /// 结束时的 mouth level。
    pub final_mouth: f32,
    /// 欠载事件数。
    pub underruns: u64,
    /// 被打断静音的样本数（冒烟流程不打断，恒 0）。
    pub interrupted_samples: u64,
}

impl AudioSmokeReport {
    /// 逐行摘要（CLI 打印用）。
    pub fn summarize_lines(&self) -> Vec<String> {
        match &self.device {
            None => vec!["audio-smoke     : 无音频设备（skip，如实记录）".to_string()],
            Some(dev) => {
                let outcome = if self.drained { "drained" } else { "TIMEOUT" };
                vec![
                    format!(
                        "device          : {} ({}, {} Hz × {} ch)",
                        dev.device_name, dev.sample_format, dev.sample_rate, dev.channels
                    ),
                    format!(
                        "specs           : source {} Hz × {} ch → device {} Hz × {} ch",
                        self.source_spec.sample_rate(),
                        self.source_spec.channels(),
                        dev.sample_rate,
                        dev.channels
                    ),
                    format!(
                        "enqueued        : generated={} src-samples, accepted={} dev-samples, rejected={}",
                        self.generated_source_samples,
                        self.accepted_device_samples,
                        self.rejected_device_samples
                    ),
                    format!(
                        "mouth           : peak={:.4} final={:.4}",
                        self.peak_mouth, self.final_mouth
                    ),
                    format!(
                        "health          : {} in {:.1?}, underruns={}, interrupted={}",
                        outcome, self.drain_waited, self.underruns, self.interrupted_samples
                    ),
                    format!(
                        "ring            : capacity={} samples",
                        dev.ring_capacity_samples
                    ),
                ]
            }
        }
    }
}
