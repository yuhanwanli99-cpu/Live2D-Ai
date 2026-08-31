//! 纯状态机 RMS 表 [`RmsMeter`]：无 I/O、无线程、可增量驱动。
//!
//! 输入约定：**只喂「实际被声卡消费」的 f32 样本**（即播放回调真正取走、
//! 送进 DAC 的那一部分），而不是解码产出或队列缓冲中的样本——否则口型会
//! 抢在声音前面。样本可为 interleaved 多声道（RMS 对所有声道能量一视同仁）。
//!
//! 算法（每样本一步，纯函数式更新）：
//! 1. 滑动窗口：维护最近 `window_len` 个样本的平方和，RMS = sqrt(和/窗内数)；
//! 2. attack/release 包络：`level += (rms - level) * coeff`，rms 升时用
//!    attack 系数、降时用 release 系数；系数由时间常数换算
//!    `coeff = 1 - exp(-1 / (秒 × sample_rate))`，0 秒 = 立即贴合；
//! 3. clamp 与洁净性：输出恒被夹到 [0, 1]；非有限输入按 0 能量处理；
//!    静音且电平低于 `f32::EPSILON` 时精确吸附到 0——**任何输入序列下
//!    输出都是有限值，永不产出 NaN**。

use std::collections::VecDeque;

use crate::audio::AudioSpec;
use crate::error::{Error, Result};

/// 静音吸附阈值：真 RMS 为 0 且包络已衰减到该值以下时直接归零，
/// 避免 f32 亚正规数无限拖尾。
const SILENCE_SNAP_LEVEL: f32 = f32::EPSILON;

/// 滑动窗口 RMS + attack/release 平滑的纯状态机电平表。
#[derive(Debug)]
pub struct RmsMeter {
    /// 最近样本的环形窗口（未填满时按实际数量平均）。
    window: VecDeque<f32>,
    /// 窗口容量（样本数），≥ 1。
    window_len: usize,
    /// 窗口内样本平方和（f64 累加，长窗口下保持数值稳定）。
    sum_squares: f64,
    /// 平滑后的当前电平 ∈ [0, 1]。
    smooth: f32,
    /// 上升（attack）单样本系数 ∈ [0, 1]。
    attack_coeff: f32,
    /// 回落（release）单样本系数 ∈ [0, 1]。
    release_coeff: f32,
}

impl RmsMeter {
    /// 创建电平表。
    ///
    /// - `window_ms`：滑动窗口时长（毫秒），必须有限且 > 0；
    /// - `attack_secs` / `release_secs`：上升/回落时间常数（秒），
    ///   必须有限且 ≥ 0（0 = 无平滑、立即贴合）。
    ///
    /// 参数非法返回 [`Error::InvalidAudioConfig`]。
    pub fn new(
        spec: AudioSpec,
        window_ms: f32,
        attack_secs: f32,
        release_secs: f32,
    ) -> Result<Self> {
        fn invalid(message: String) -> Error {
            Error::InvalidAudioConfig { message }
        }

        if !window_ms.is_finite() || window_ms <= 0.0 {
            return Err(invalid(format!(
                "window_ms 必须为正的有限值，得到 {window_ms}"
            )));
        }
        for (name, secs) in [("attack_secs", attack_secs), ("release_secs", release_secs)] {
            if !secs.is_finite() || secs < 0.0 {
                return Err(invalid(format!("{name} 必须为非负有限值，得到 {secs}")));
            }
        }

        let window_samples = window_ms * 1_000.0_f32.recip() * spec.sample_rate() as f32;
        if !window_samples.is_finite() {
            return Err(invalid(format!(
                "window_ms={window_ms} 在 {} Hz 下溢出",
                spec.sample_rate()
            )));
        }
        // 未填满窗口前按实际样本数平均，因此容量只决定「记忆长度」；
        // 延迟增长（VecDeque 惰性扩容）避免超大窗口的一次性分配。
        let window_len = (window_samples.round() as usize).max(1);

        Ok(Self {
            window: VecDeque::with_capacity(window_len.min(4096)),
            window_len,
            sum_squares: 0.0,
            smooth: 0.0,
            attack_coeff: time_constant_coeff(attack_secs, spec.sample_rate()),
            release_coeff: time_constant_coeff(release_secs, spec.sample_rate()),
        })
    }

    /// 当前平滑电平 ∈ [0, 1]（不推进状态）。
    pub const fn level(&self) -> f32 {
        self.smooth
    }

    /// 推入一批刚被声卡消费的样本（任意切分；interleaved 多声道亦可），
    /// 返回本批结束后的平滑电平 ∈ [0, 1]。
    ///
    /// 非有限值（NaN / ±∞）按 0 能量处理：不进入平方和、不产生 NaN。
    /// 空 slice 合法（状态不变，仅返回当前电平）。
    pub fn push(&mut self, samples: &[f32]) -> f32 {
        for &raw in samples {
            let sample = if raw.is_finite() { raw } else { 0.0 };
            let square = f64::from(sample) * f64::from(sample);

            self.sum_squares += square;
            self.window.push_back(sample);
            if self.window.len() > self.window_len
                && let Some(old) = self.window.pop_front()
            {
                self.sum_squares -= f64::from(old) * f64::from(old);
            }
            debug_assert!(!self.window.is_empty());

            // 平方和可能因浮点消去出现极小负值，sqrt 前 clamp。
            let count = self.window.len() as f64;
            let rms = (self.sum_squares.max(0.0) / count).sqrt() as f32;
            let coeff = if rms > self.smooth {
                self.attack_coeff
            } else {
                self.release_coeff
            };
            let mut next = self.smooth + (rms - self.smooth) * coeff;
            if rms == 0.0 && next.abs() < SILENCE_SNAP_LEVEL {
                next = 0.0; // 静音吸附：精确回 0 并终止亚正规拖尾
            }
            self.smooth = next.clamp(0.0, 1.0);
        }
        self.smooth
    }
}

/// 时间常数（秒）→ 单样本平滑系数 ∈ (0, 1]；0 秒 → 1（立即贴合）。
fn time_constant_coeff(seconds: f32, sample_rate: u32) -> f32 {
    if seconds <= 0.0 {
        return 1.0;
    }
    let per_sample_tc = seconds * sample_rate as f32; // > 0（seconds 已校验有限）
    1.0 - (-per_sample_tc.recip()).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meter(window_ms: f32, attack: f32, release: f32) -> RmsMeter {
        RmsMeter::new(AudioSpec::default(), window_ms, attack, release).expect("valid meter")
    }

    #[test]
    fn invalid_parameters_are_rejected() {
        assert!(matches!(
            RmsMeter::new(AudioSpec::default(), 0.0, 0.0, 0.0),
            Err(Error::InvalidAudioConfig { .. })
        ));
        assert!(matches!(
            RmsMeter::new(AudioSpec::default(), f32::NAN, 0.0, 0.0),
            Err(Error::InvalidAudioConfig { .. })
        ));
        assert!(matches!(
            RmsMeter::new(AudioSpec::default(), 50.0, -0.1, 0.0),
            Err(Error::InvalidAudioConfig { .. })
        ));
        assert!(matches!(
            RmsMeter::new(AudioSpec::default(), 50.0, 0.0, f32::INFINITY),
            Err(Error::InvalidAudioConfig { .. })
        ));
    }

    #[test]
    fn silence_returns_exactly_zero_and_stays_zero() {
        let mut m = meter(10.0, 0.0, 0.0);
        assert_eq!(m.level(), 0.0);
        assert_eq!(m.push(&[0.0; 480]), 0.0, "静音必须精确回 0");
        assert_eq!(m.push(&[]), 0.0);
        assert_eq!(m.level(), 0.0);
    }

    #[test]
    fn constant_amplitude_rms_matches_expected_with_instant_smoothing() {
        // attack=release=0 → 立即贴合；恒幅 ±0.5 的 RMS = 0.5。
        let mut m = meter(20.0, 0.0, 0.0);
        let batch: Vec<f32> = [0.5, -0.5].repeat(240);
        let level = m.push(&batch);
        assert!((level - 0.5).abs() < 1e-6, "got {level}");
    }

    #[test]
    fn step_input_rises_with_attack_then_releases_toward_zero() {
        // 慢 attack：阶跃后立即读数应仍接近 0（爬升中）。
        let mut m = meter(5.0, 0.05, 0.05);
        let loud = [1.0_f32; 48]; // 2 ms @24kHz 的满幅样本
        let first = m.push(&loud[..8]);
        assert!(first > 0.0 && first < 0.35, "attack 应缓慢上升: {first}");

        // 持续满幅 → 收敛到 1 的极近邻（f32 逐步平滑在距 1 约半个 ulp 处
        // 停止推进，残差 ~3.6e-5），clamp 保证永不越过上界。
        let peak = m.push(&loud.repeat(1250));
        assert!((peak - 1.0).abs() < 1e-4, "满幅 RMS 应逼近 1: {peak}");
        assert!((0.0..=1.0).contains(&peak), "clamp 保证不越界: {peak}");

        // 停止输入改喂静音 → release 衰减，最终吸附回精确 0。
        for _ in 0..60 {
            let level = m.push(&[0.0_f32; 480]); // 每次 20 ms 静音
            assert!(level.is_finite() && (0.0..=1.0).contains(&level));
        }
        assert_eq!(m.level(), 0.0, "release 后应精确回 0");
    }

    #[test]
    fn non_finite_samples_are_sanitized_without_nan() {
        let mut m = meter(10.0, 0.0, 0.0);
        // NaN / ±∞ 一律记 0 能量：纯垃圾输入下电平保持精确 0。
        assert_eq!(m.push(&[f32::NAN, f32::INFINITY, f32::NEG_INFINITY]), 0.0);

        // 随后正常样本照常计能量（0.6/-0.6 恒幅 RMS = 0.6）；喂满一个窗口
        // 把先前的 0 能量槽冲出窗口。
        let level = m.push(&[0.6_f32, -0.6].repeat(200));
        assert!(level.is_finite());
        assert!((level - 0.6).abs() < 1e-6, "got {level}");

        // 之后继续静音也不会把 NaN 带回来。
        assert_eq!(m.push(&[0.0; 480]), 0.0);
        assert!(m.level().is_finite());
    }

    #[test]
    fn out_of_range_input_is_clamped_into_unit_interval() {
        let mut m = meter(10.0, 0.0, 0.0);
        let level = m.push(&[42.0_f32; 96]);
        assert_eq!(level, 1.0);
    }

    #[test]
    fn empty_push_is_a_state_noop() {
        let mut m = meter(10.0, 0.0, 0.0);
        let before = m.push(&[0.25; 48]);
        assert_eq!(m.push(&[]), before);
        assert_eq!(m.level(), before);
    }
}
