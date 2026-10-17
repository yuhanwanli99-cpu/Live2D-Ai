//! 包络 / 强度 morph / 逐通道增益（自 `preset/mod.rs` 机械拆出）。

use super::data::{MAX_INTENSITY, MotionWave, PresetKind, PresetSpec};
use super::motion_curve;

/// 正弦包络：`progress ∈ [0,1]` → 幅值乘子，两端为 0、中途为 1。
///
/// 用 `sin(πp)` 而不是线性三角波：起落都是**平滑**的，首末帧幅值为 0，于是撤销参数
/// 时不会「跳一下」（跳变正是皮套看起来像抽帧的原因）。
///
/// **2026-10-14 起手势不再走它**（改走 `motion_curve`）；留着是因为它「两端**恰好** 0」
/// 仍是要测的口径（旧回归继续测这条函数），且是 `motion_curve` 之前的起落定义。
#[allow(dead_code)]
pub fn envelope(progress: f64) -> f32 {
    let p = progress.clamp(0.0, 1.0);
    // 两端**恰好** 0：sin(pi) 在 f64 里是 1.2e-16 而不是 0。
    if p <= 0.0 || p >= 1.0 {
        return 0.0;
    }
    ((std::f64::consts::PI * p).sin()).max(0.0) as f32
}

/// 手势**头 / 其它通道**的幅值乘子 = 起落曲线 × 波形（表情忽略波形，增益仍走曲线）。
///
/// 曲线与身体滞后都住在 `motion_curve`：本函数是「用本帧 `p`」的那一支；
/// 身体通道（`ParamBodyAngle*`）的增益请走 `pack_gain`，它按通道选进度。
pub fn motion_gain(wave: MotionWave, progress: f64) -> f32 {
    motion_curve::gain(wave, progress)
}

/// 本帧的**基准参数**（不含 intensity / 倍率，但含起落 / 波形增益）。
///
/// - 普通包 = `params`；
/// - morph 包 = low 极（= `params`，等价于 `morph_mix(spec, low)`）。
///
/// 增益**逐通道**算：手势的身体通道走滞后进度（见 `pack_gain`）。
pub fn frame_params(spec: &PresetSpec, progress: f64) -> Vec<(&'static str, f32)> {
    base_params(spec)
        .into_iter()
        .map(|(id, v)| (id, v * pack_gain(spec.kind, spec.wave, progress, id)))
        .collect()
}

/// 普通包 = `params × intensity`；morph 包 = 按 intensity 在两极间插值。
///
/// 返回**未乘通道倍率、未钳位**的基准值（见 `frame_params_scaled_by`）。
pub fn base_params_for_intensity(spec: &PresetSpec, intensity: f32) -> Vec<(&'static str, f32)> {
    match spec.morph {
        Some(_) => morph_mix(spec, intensity),
        None => spec
            .params
            .iter()
            .map(|(id, v)| (*id, v * intensity))
            .collect(),
    }
}

/// morph 包的基准值：在 low / high 两极之间按 `intensity` 插值。
///
/// - `intensity <= low`：从 0（中性）线性升到 low 极（`t = i/low`）；
/// - `low < intensity < high`：low 极 ↔ high 极线性插值；
/// - `intensity >= high`：high 极。
///
/// 非 morph 包原样返回 `params`（调用方不应走这里）。
pub fn morph_mix(spec: &PresetSpec, intensity: f32) -> Vec<(&'static str, f32)> {
    let Some(m) = spec.morph else {
        return spec.params.to_vec();
    };
    let i = if intensity.is_finite() {
        intensity.max(0.0)
    } else {
        m.low
    };
    let low = if m.low.is_finite() && m.low > 0.0 {
        m.low
    } else {
        1.0
    };
    let high = if m.high.is_finite() && m.high > low {
        m.high
    } else {
        MAX_INTENSITY.max(low)
    };
    let high_of = |id: &str| -> f32 {
        m.high_params
            .iter()
            .find(|(k, _)| *k == id)
            .map_or(0.0, |(_, v)| *v)
    };
    let mut out: Vec<(&'static str, f32)> = Vec::with_capacity(spec.params.len());
    for (id, low_v) in spec.params {
        let high_v = high_of(id);
        let v = if i <= low {
            low_v * (i / low)
        } else if i >= high {
            high_v
        } else {
            let t = (i - low) / (high - low);
            low_v + (high_v - low_v) * t
        };
        out.push((*id, v));
    }
    // 只出现在 high 极的通道（shipped 表不允许；宽容补上，从 0 插值）。
    for (id, high_v) in m.high_params {
        if out.iter().any(|(k, _)| k == id) {
            continue;
        }
        let v = if i <= low {
            0.0
        } else if i >= high {
            *high_v
        } else {
            let t = (i - low) / (high - low);
            *high_v * t
        };
        out.push((*id, v));
    }
    out
}

/// 基准参数：morph 包 = low 极（`params`），普通包 = `params`。
fn base_params(spec: &PresetSpec) -> Vec<(&'static str, f32)> {
    spec.params.to_vec()
}

/// 起落 / 波形乘子：**表情与手势同走 `motion_curve::curve`**（进度 0 / 1 恰好 0，
/// 保持段恰好 1）；手势**逐通道**算，身体（`ParamBodyAngle*`）走滞后进度。
pub(super) fn pack_gain(kind: PresetKind, wave: MotionWave, progress: f64, param_id: &str) -> f32 {
    match kind {
        PresetKind::Expression => motion_curve::curve(progress) as f32,
        PresetKind::Motion => {
            if param_id.starts_with(motion_curve::BODY_CHANNEL_PREFIX) {
                motion_gain(wave, motion_curve::body_progress(progress))
            } else {
                motion_gain(wave, progress)
            }
        }
    }
}

/// 同 `frame_params`，再乘 `intensity`（P0-1；缺省 1.0 时逐值等价）。
// 只带强度的旧 API：运行时改走 frame_params_scaled_by；留着给旧回归逐值对照。
#[allow(dead_code)]
pub fn frame_params_scaled(
    spec: &PresetSpec,
    progress: f64,
    intensity: f32,
) -> Vec<(&'static str, f32)> {
    base_params_for_intensity(spec, intensity)
        .into_iter()
        .map(|(id, v)| (id, v * pack_gain(spec.kind, spec.wave, progress, id)))
        .collect()
}

/// 归一化进度（`elapsed_ms` 相对 `duration_ms`，两端钳到 `[0,1]`）。
pub fn progress_at(elapsed_ms: f64, duration_ms: f64) -> f64 {
    if duration_ms <= 0.0 {
        return 1.0;
    }
    (elapsed_ms / duration_ms).clamp(0.0, 1.0)
}
