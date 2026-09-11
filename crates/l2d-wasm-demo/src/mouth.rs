//! 口型开合量换算：**纯函数、无 web 依赖**，可在原生 target 上单测。
//!
//! 单独成文件的理由（与本项目 `shell/flutter/lib/audio/schedule.dart` 同款约定）：
//! 「线性 RMS → 参数开合量」是口型听感的核心算术，必须能在**不启动浏览器、
//! 不依赖 WebAudio** 的前提下回归。放在 `web::surface` 里会被
//! `#[cfg(target_arch = "wasm32")]` 挡在原生测试之外（等于没有回归）。
//!
//! # 为什么需要这个映射
//!
//! 实测真实 TTS 语音的线性 RMS 极低（24 kHz、50 ms 窗）：
//! 中位数 ≈ 0.030、p90 ≈ 0.118、最大 ≈ 0.282
//! （证据：`docs/plans/HANDOFF-2026-09-10-four-questions.md` §5）。
//! 直接把它写进 `ParamMouthOpenY`（0..1 量程）→ 嘴只张开原量程的 3%~12%，
//! 离屏实测形变量仅满幅的 27%，肉眼几乎看不出嘴在动。
//!
//! 因此按**分贝**映射到一个合理的听觉动态范围，而不是「乘一个固定增益」——
//! dB 是相对满幅的量，映射结果与 TTS 输出电平、音色、服务端实现无关，
//! 换一套后端不必重新调参。

/// dB 映射下界：-36 dBFS 及以下视为静音（口型 0）。
pub const MOUTH_FLOOR_DB: f32 = -36.0;

/// dB 映射上界：-6 dBFS 及以上视为满幅（口型 1）。
pub const MOUTH_CEIL_DB: f32 = -6.0;

/// 口型灵敏度默认值（1.0 = 实测标定：真实语音 p90 约开到 0.58）。
pub const DEFAULT_MOUTH_SENSITIVITY: f32 = 1.0;

/// 口型灵敏度上限（防止把嘴撑到长期全开而穿模）。
pub const MAX_MOUTH_SENSITIVITY: f32 = 4.0;

/// 口型电平（线性 RMS ∈ `[0,1]`）→ `ParamMouthOpenY` 开合量 ∈ `[0,1]`。
///
/// 映射规则：
/// - [`MOUTH_FLOOR_DB`]（-36 dBFS）→ `0.0`；
/// - [`MOUTH_CEIL_DB`]（-6 dBFS）→ `1.0`；
/// - 中间按 dB 线性插值，结果乘 `sensitivity` 后 clamp 到 `[0,1]`。
///
/// 标定验算（`sensitivity = 1.0`）：
///
/// | 输入 RMS | dBFS | 输出 |
/// | --- | --- | --- |
/// | 0.030（p50） | -30.5 | 0.18 |
/// | 0.118（p90） | -18.6 | **0.58** |
/// | 0.282（max） | -11.0 | 0.83 |
///
/// 非有限值 / 非正电平 / 非正灵敏度一律返回 `0.0`（防御，不 panic）。
pub fn mouth_open_from_level(level: f32, sensitivity: f32) -> f32 {
    if !level.is_finite() || level <= 0.0 || !sensitivity.is_finite() || sensitivity <= 0.0 {
        return 0.0;
    }
    let db = 20.0 * level.min(1.0).log10();
    let norm = (db - MOUTH_FLOOR_DB) / (MOUTH_CEIL_DB - MOUTH_FLOOR_DB);
    (norm.clamp(0.0, 1.0) * sensitivity).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **回归（2026-09-10）**：真实语音的 RMS 必须映射到「肉眼看得见」的开口量。
    ///
    /// 数值取自 2026-09-10 对真实 TTS 输出（24 kHz、50 ms 窗）的实测。
    /// 旧实现直接写 RMS → p90 处只有 0.118，实测形变量仅满幅 27%，等于看不见。
    #[test]
    fn real_speech_rms_maps_to_visible_mouth_open() {
        let p50 = mouth_open_from_level(0.030, 1.0);
        let p90 = mouth_open_from_level(0.118, 1.0);
        let max = mouth_open_from_level(0.282, 1.0);
        assert!(
            (0.15..0.25).contains(&p50),
            "静音间隙(p50)应轻微开合，got {p50}"
        );
        assert!(
            (0.5..0.7).contains(&p90),
            "常规音节(p90)应开到一半以上，got {p90}"
        );
        assert!(max > 0.75, "重音(max)应接近全开，got {max}");
        // 关键对照：旧行为（直接写 RMS）在 p90 处只有 0.118。
        assert!(p90 > 0.118 * 3.0, "dB 映射必须显著放大开口量");
    }

    #[test]
    fn mapping_is_monotonic_and_bounded() {
        let mut prev = -1.0;
        for step in 0..=100 {
            let level = step as f32 / 100.0;
            let open = mouth_open_from_level(level, 1.0);
            assert!((0.0..=1.0).contains(&open), "越界: {level} -> {open}");
            assert!(open >= prev, "必须单调不减: {level} -> {open} < {prev}");
            prev = open;
        }
        assert_eq!(mouth_open_from_level(1.0, 1.0), 1.0);
    }

    #[test]
    fn quiet_input_is_closed() {
        // -36 dBFS 及以下 → 0（因此不再需要旧的 `volume > 0.01` 门限；
        // 那个门限还会让轻声段落整段闭嘴）。
        assert_eq!(mouth_open_from_level(0.0, 1.0), 0.0);
        assert_eq!(mouth_open_from_level(0.01, 1.0), 0.0);
        assert_eq!(mouth_open_from_level(0.0158, 1.0), 0.0); // ≈ -36 dBFS
        assert!(mouth_open_from_level(0.05, 1.0) > 0.0);
    }

    #[test]
    fn sensitivity_scales_and_clamps() {
        // 用不会被 clamp 的电平验证「线性放大」。
        let base = mouth_open_from_level(0.030, 1.0);
        assert!((mouth_open_from_level(0.030, 2.0) - base * 2.0).abs() < 1e-5);
        // 放大后越界必须 clamp 到 1.0，而不是溢出。
        let strong = mouth_open_from_level(0.118, 1.0);
        assert!(strong * 2.0 > 1.0, "用例需能触发 clamp");
        assert_eq!(mouth_open_from_level(0.118, 2.0), 1.0);
        assert_eq!(mouth_open_from_level(0.118, MAX_MOUTH_SENSITIVITY), 1.0);
        // 非正灵敏度 → 全闭（而不是 panic 或取反）。
        assert_eq!(mouth_open_from_level(0.118, 0.0), 0.0);
        assert_eq!(mouth_open_from_level(0.118, -1.0), 0.0);
    }

    #[test]
    fn defensive_against_non_finite_input() {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.5] {
            assert_eq!(mouth_open_from_level(bad, 1.0), 0.0, "level={bad}");
        }
        for bad in [f32::NAN, f32::INFINITY] {
            assert_eq!(mouth_open_from_level(0.5, bad), 0.0, "sensitivity={bad}");
        }
    }
}
