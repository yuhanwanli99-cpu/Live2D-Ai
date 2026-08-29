//! 表演播放器：单 active，把 [`crate::action::SemanticAction`] 按固定 dt 推进为
//! [`ParameterFrame`] 序列。无内部时钟依赖（由调用方喂 dt）、无堆分配。

use crate::action::SemanticAction;

use super::sample_curve;
use super::{ParameterFrame, SampleStatus, duration, strength_gain};

/// 活动动作及其已推进的时间（私有）。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ActivePerformance {
    action: SemanticAction,
    elapsed: f32,
}

/// 表演播放器：单 active，把 [`SemanticAction`] 按固定 dt 推进为
/// [`ParameterFrame`] 序列。无内部时钟依赖（由调用方喂 dt）、无堆分配。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PerformancePlayer {
    active: Option<ActivePerformance>,
}

impl PerformancePlayer {
    /// 新建空闲播放器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前活动动作（若有）。
    pub fn active(&self) -> Option<SemanticAction> {
        self.active.map(|a| a.action)
    }

    /// 开始/抢占：无条件重置时间轴（开始时间 = 现在），旧动作不做叠加。
    /// 对应调度层 [`crate::ActionEffect::Transition`]（幂等去重由上游 reducer 负责）。
    pub fn play(&mut self, action: SemanticAction) {
        self.active = Some(ActivePerformance {
            action,
            elapsed: 0.0,
        });
    }

    /// 打断：立即清空活动动作；后续帧为精确 neutral。对应 `ActionEffect::End`。
    pub fn interrupt(&mut self) {
        self.active = None;
    }

    /// 推进一帧并把结果写入 `out`（零分配），返回 [`SampleStatus`]。
    ///
    /// dt 安全规则：非有限或 ≤ 0 的 dt 一律按 0 处理（帧照常输出、时间不前进），
    /// NaN/负 dt 因此不会污染时间轴；超大的有限 dt 则把动作一步推到终点。
    pub fn update(&mut self, dt: f32, out: &mut ParameterFrame) -> SampleStatus {
        let Some(active) = self.active.as_mut() else {
            out.set_neutral();
            return SampleStatus::Idle;
        };
        if dt.is_finite() && dt > 0.0 {
            active.elapsed += dt;
        }
        let dur = duration(active.action.action);
        if active.elapsed < dur {
            let progress = (active.elapsed / dur).clamp(0.0, 1.0);
            let (id, strength) = (active.action.action, active.action.strength);
            sample_curve(id, strength_gain(strength), progress, out);
            SampleStatus::Playing { progress }
        } else {
            self.active = None;
            out.set_neutral();
            SampleStatus::Finished
        }
    }
}
