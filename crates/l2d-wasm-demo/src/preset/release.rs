//! `Revoke`（`preset_id == "none"`）的**放回**：不从当帧清参数，从**屏幕上这一帧
//! 的值**收到 0，时长 = 该槽 `ttl_ms × (1 - HOLD_END)`，权重是现成的
//! `smootherstep(1 - q)`（2026-10-15 执行单 §1/§3）。
//!
//! 到点才清槽：`write` 返回 `true` 时把这次放回写过的通道整批 `clear`，调用方把槽
//! 置空。放回途中再 `Revoke` → 从**当前屏幕值**重开一段（不跳变）；放回途中换预设 →
//! `handoff::on_screen` 用 `values_at` 这一帧的值做快照。
//!
//! 本模块只被 `mod.rs` 调用（开一段 / 每帧写），自身不碰槽位状态。

use super::PresetSink;
use super::motion_curve::{self, HOLD_END};

/// 一个槽的放回状态。
#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    /// `Revoke` 那一帧屏幕上的值（逐通道）。
    from: Vec<(&'static str, f32)>,
    /// 开始时刻（与 `apply_frame` 的 `now_ms` 同源）。
    started_ms: f64,
    /// 放回时长 = 该槽 `ttl_ms × (1 - HOLD_END)`。
    duration_ms: f64,
}

impl Release {
    /// 开一段放回；`ttl_ms` = 该槽自己的时长。
    pub fn new(from: Vec<(&'static str, f32)>, ttl_ms: f64, now_ms: f64) -> Self {
        let duration_ms = if ttl_ms.is_finite() {
            (ttl_ms * (1.0 - HOLD_END)).max(0.0)
        } else {
            0.0
        };
        Self {
            from,
            started_ms: now_ms,
            duration_ms,
        }
    }

    /// 放回进度 `q ∈ [0,1]`（时长为 0 → 直接 1）。
    pub fn progress_at(&self, now_ms: f64) -> f64 {
        if self.duration_ms <= 0.0 {
            return 1.0;
        }
        ((now_ms - self.started_ms) / self.duration_ms).clamp(0.0, 1.0)
    }

    /// 这一帧屏幕上的值：`from × smootherstep(1 - q)`（`q = 0` 恰好等于撤销前的值）。
    pub fn values_at(&self, now_ms: f64) -> Vec<(&'static str, f32)> {
        let w = motion_curve::smootherstep(1.0 - self.progress_at(now_ms)) as f32;
        self.from.iter().map(|(id, v)| (*id, v * w)).collect()
    }

    /// 写这一帧。返回 `true` = 放回结束（override 已整批清，调用方把槽置空）。
    pub fn write(&self, now_ms: f64, sink: &mut impl PresetSink) -> bool {
        if self.progress_at(now_ms) >= 1.0 {
            for (id, _) in &self.from {
                sink.clear_override_param(id);
            }
            return true;
        }
        for (id, value) in self.values_at(now_ms) {
            sink.override_param(id, value);
        }
        false
    }
}
