//! 手势（`kind=motion`）的起落曲线与身体滞后（2026-10-14）。
//!
//! 旧实现用单峰包络 `sin(pi*p)`，**头与身共用同一个 `p`**——看起来就是「一下戳
//! 出去再收回」，身体没有落后，动作发僵。本模块把它换成三段曲线
//! （**攻击 → 保持 → 放回**，smootherstep，两端值与斜率都是 0），并给
//! `ParamBodyAngle*` 一条**滞后进度** `body_p`：身体晚起步、`p = 1` 时同步归零
//! （避免槽到期撤销时把尾巴剪断）。
//!
//! 通道分派（哪些通道走 `body_p`）在 `mod.rs` 的 `pack_gain` 里，本模块只提供
//! 曲线 / 滞后进度 / 波形。
//!
//! 范围：手势与**表情**都走这条曲线（`pack_gain` 按通道分派）；`field_runtime`
//! （v1 三字段）与 `apply_idle_life`（呼吸 / 眨眼 / 微表情）都不经过这里。
//! 旧的 `envelope()`（`sin(pi*p)`）留在 `mod.rs`，旧的两端为 0 的回归继续测它。

use super::{MotionWave, SHAKE_CYCLES};

/// 攻击段结束（进度）：曲线从 0 涨到 1。
pub const ATTACK_END: f64 = 0.22;
/// 保持段结束（进度）：曲线一直是 1，之后开始放回。
pub const HOLD_END: f64 = 0.62;
/// 身体滞后（进度）：`body_p = (p - BODY_LAG) / (1 - BODY_LAG)`。
///
/// 分子分母刻意这样配对：`p = 1` 时 `body_p = 1`，于是身体也在同一帧归零。
pub const BODY_LAG: f64 = 0.12;

/// 身体通道前缀（与 `scales::ScaleClass::Body` 同一口径；这里不引 scales）。
pub const BODY_CHANNEL_PREFIX: &str = "ParamBodyAngle";

/// smootherstep：`t^3 (t (6t - 15) + 10)`。两端值与斜率都是 0。
///
/// `pub`：`release`（`Revoke` 放回）的权重就是 `smootherstep(1 - q)`。
pub fn smootherstep(t: f64) -> f64 {
    t * t * t * (t * (6.0 * t - 15.0) + 10.0)
}

/// 手势起落曲线：`0 → 1`（攻击至 `ATTACK_END`）→ 保持 1（至 `HOLD_END`）
/// → `0`（放回）。
///
/// `p = 0` 与 `p = 1` **恰好**返回 0，两端斜率 0；非有限进度也返回 0（不做
/// NaN 传播）。保持段的值**恰好**是 1（峰值判据用它）。
pub fn curve(progress: f64) -> f64 {
    if !progress.is_finite() {
        return 0.0;
    }
    let p = progress.clamp(0.0, 1.0);
    if p <= 0.0 || p >= 1.0 {
        return 0.0;
    }
    if p < ATTACK_END {
        return attack_weight(p);
    }
    if p <= HOLD_END {
        return 1.0;
    }
    smootherstep((1.0 - p) / (1.0 - HOLD_END))
}

/// 攻击段权重：`smootherstep(p / ATTACK_END)`（`0 → 1`，两端斜率 0）。
///
/// `curve` 在攻击段就是它；单独暴露是给换槽接力（`handoff`）用——新旧参数
/// 按这个权重插值，于是 `p = 0` 恰好等于旧值、`p = ATTACK_END` 恰好等于正常帧。
pub fn attack_weight(progress: f64) -> f64 {
    if !progress.is_finite() {
        return 0.0;
    }
    smootherstep((progress / ATTACK_END).clamp(0.0, 1.0))
}

/// 身体进度：比头晚 `BODY_LAG` 起步，`p = 1` 时与头同步归零。
pub fn body_progress(progress: f64) -> f64 {
    if !progress.is_finite() {
        return 0.0;
    }
    ((progress - BODY_LAG) / (1.0 - BODY_LAG)).clamp(0.0, 1.0)
}

/// 幅值乘子 = **起落曲线 × 波形**。
///
/// - `MotionWave::Single`：就是曲线本身（保持段 = 1；不再等于 `envelope`）；
/// - `MotionWave::Oscillate`：曲线 × `sin(2*pi*cycles*p)`——首末仍是 0，
///   `|gain| <= 1`；`cycles` 非有限 / <= 0 回落 `SHAKE_CYCLES`。
pub fn gain(wave: MotionWave, progress: f64) -> f32 {
    let c = curve(progress);
    if c == 0.0 {
        return 0.0;
    }
    match wave {
        MotionWave::Single => c as f32,
        MotionWave::Oscillate { cycles } => {
            let cycles = if cycles.is_finite() && cycles > 0.0 {
                cycles
            } else {
                SHAKE_CYCLES
            };
            let p = progress.clamp(0.0, 1.0);
            (c * (2.0 * std::f64::consts::PI * cycles * p).sin()) as f32
        }
    }
}
