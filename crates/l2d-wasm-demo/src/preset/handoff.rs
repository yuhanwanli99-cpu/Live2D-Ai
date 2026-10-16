//! 同槽换动作的**接力**：从换下去那一帧的参数接到新动作（2026-10-15 执行单 §1）。
//!
//! 旧行为是硬切：同一槽换一条**不同 id** 的预设时先 `clear_spec_overrides` 把旧包
//! 写过的通道整批清掉，新预设从 `progress = 0` 重新攻击——画面上就是「先归零、
//! 再起手」，俗称抽一下。这里把「换下去那一帧的最终值」记在新活动预设上，并在
//! **攻击段**（`progress < ATTACK_END`）内逐通道插值，权重是现成的
//! `motion_curve::attack_weight`（= `smootherstep(p / ATTACK_END)`，两端斜率 0）：
//!
//! - 两边都有的通道：旧值 → 新预设在这一进度的正常帧值；
//! - 只有旧预设有的通道：旧值 → 0，进度到 `ATTACK_END` 那一帧 `clear`（此后不再写）；
//! - 只有新预设有的通道：0 → 它在这一进度的正常帧值。
//!
//! 进度 `>= ATTACK_END` 之后丢掉快照，帧值与不接旧值**逐值相同**。空槽第一次
//! 施加、同一 id 再施加、到点撤销都不走这里（行为不变）；`Revoke` 走 `release` 的
//! 放回，但它的**快照**仍走这里。
//!
//! **快照 = 屏幕上这一帧的值**（2026-10-15 执行单 §1/§3）：正在放回 → 放回写到一半
//! 的值；正在接力 → 这一帧的混合结果；否则才是 `frame_params_scaled_by`。表情与手势
//! **共用这一段窗口**（表情现在也带攻击段，接力仍是「旧表情 → 新表情」的淡入；到期
//! 回待机仍是到点立即撤销，不做放回曲线）；两个槽仍然互不混合——快照只记本槽上一次
//! 写下的参数。

use super::motion_curve::{self, ATTACK_END};
use super::{
    ActivePreset, PresetScales, PresetSink, PresetSpec, frame_params_scaled_by, progress_at,
};

/// 换槽时记下的「换下去那一帧」的最终值（含 intensity / 通道倍率 / 钳位）。
#[derive(Debug, Clone, PartialEq)]
pub struct Handoff {
    old: Vec<(&'static str, f32)>,
}

/// 一个槽**屏幕上这一帧**的值：放回途中 → 放回写到一半的值；接力途中 → 这一帧的
/// 混合结果；否则 → 正常帧值（`frame_params_scaled_by`，含「施加后还没画过一帧」）。
pub fn on_screen(
    active: &ActivePreset,
    now_ms: f64,
    scales: PresetScales,
) -> Vec<(&'static str, f32)> {
    if let Some(release) = &active.release {
        return release.values_at(now_ms);
    }
    let elapsed = (now_ms - active.started_ms).max(0.0);
    let progress = progress_at(elapsed, active.ttl_ms);
    match active.handoff.as_ref() {
        Some(handoff) => {
            handoff
                .frame_at(active.spec, progress, active.intensity, scales)
                .1
        }
        None => frame_params_scaled_by(active.spec, progress, active.intensity, scales),
    }
}

impl Handoff {
    /// 记下 `old` **屏幕上这一帧**的值。**不清**任何 override。
    pub fn capture(old: &ActivePreset, now_ms: f64, scales: PresetScales) -> Self {
        Self {
            old: on_screen(old, now_ms, scales),
        }
    }

    /// 本帧的逐通道结果（**不写** sink）；`.0` = 接力是否结束。
    ///
    /// 攻击段内逐通道插值（见模块头注）；`progress >= ATTACK_END` 时就是正常帧。
    pub fn frame_at(
        &self,
        spec: &PresetSpec,
        progress: f64,
        intensity: f32,
        scales: PresetScales,
    ) -> (bool, Vec<(&'static str, f32)>) {
        let frame = frame_params_scaled_by(spec, progress, intensity, scales);
        if progress >= ATTACK_END {
            return (true, frame);
        }
        let w = motion_curve::attack_weight(progress) as f32;
        let mut values: Vec<(&'static str, f32)> = Vec::with_capacity(self.old.len() + frame.len());
        for (id, old_value) in &self.old {
            let value = match frame.iter().find(|(new_id, _)| new_id == id) {
                Some((_, new_value)) => old_value + (new_value - old_value) * w,
                None => old_value * (1.0 - w),
            };
            values.push((id, value));
        }
        for (id, new_value) in &frame {
            if !self.old.iter().any(|(old_id, _)| old_id == id) {
                values.push((id, new_value * w));
            }
        }
        (false, values)
    }

    /// 写这一帧。返回 `true` = 接力结束（调用方应丢掉快照）。
    pub fn write(
        &self,
        spec: &PresetSpec,
        progress: f64,
        intensity: f32,
        scales: PresetScales,
        sink: &mut impl PresetSink,
    ) -> bool {
        let (finished, values) = self.frame_at(spec, progress, intensity, scales);
        if finished {
            for (id, _) in &self.old {
                if !values.iter().any(|(new_id, _)| new_id == id) {
                    sink.clear_override_param(id);
                }
            }
        }
        for (id, value) in values {
            sink.override_param(id, value);
        }
        finished
    }
}
