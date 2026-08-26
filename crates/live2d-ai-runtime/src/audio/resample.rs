//! 最小确定性重采样与声道映射（纯函数，无状态、无 I/O、无分配入参）。
//!
//! 用途：TTS 侧 [`AudioSpec`]（如 24 kHz / mono）与实际声卡 config
//! （如 48 kHz / stereo）不一致时，在**生产者线程**上把 PCM 转换到设备域，
//! 再写入无锁环形缓冲。转换只发生在这里——音频回调内绝不做重采样/映射。
//!
//! # 确定性
//!
//! - 重采样相位用**逐输出样本的整数定点计算**
//!   （`pos = i * from_rate * 2^32 / to_rate`，u128 中间量），不累积浮点误差、
//!   不依赖迭代顺序：同输入永远得到逐位相同的输出；
//! - 插值本身是 f32 一次线性插值（IEEE 基本运算，平台间逐位一致）；
//! - 声道映射只有「复制 / 平均 / 直通」三种确定性规则，见 [`map_channels`]。
//!
//! # 第一版边界（显式声明）
//!
//! 线性插值重采样（非多相/ sinc），抗混叠不做——对语音口型驱动的 v1 足够；
//! 任意声道数走「先混到 mono，再展开」的保守路径。升格质量属后续批次。

use crate::audio::AudioSpec;

/// 定点小数位数：相位以 Q32 定点表示。
const Q32: u128 = 1 << 32;

/// 声道数映射：`from` 声道 interleaved → `to` 声道 interleaved。
///
/// 规则（全部确定性）：
/// - `from == to`：原样拷贝；
/// - `to == 1`（任意 → mono）：按帧平均（部分尾帧按实际声道数平均）；
/// - `from == 1`（mono → 任意）：每样本复制 `to` 份；
/// - 其余（如 2 → 3）：先按帧平均混到 mono，再复制展开——保守且可逆性无关的
///   最小策略，第一版不追求矩阵混音。
pub fn map_channels(samples: &[f32], from: u16, to: u16) -> Vec<f32> {
    debug_assert!(from > 0 && to > 0, "声道数必须非零（AudioSpec 已保证）");
    if from == to || samples.is_empty() {
        return samples.to_vec();
    }
    if to == 1 {
        // 任意 → mono：按帧平均；尾部不足一帧时按实际数量平均。
        return samples.chunks(from as usize).map(avg_chunk).collect();
    }
    if from == 1 {
        // mono → N：逐样本复制。
        let to = to as usize;
        let mut out = Vec::with_capacity(samples.len() * to);
        for &s in samples {
            out.extend(std::iter::repeat_n(s, to));
        }
        return out;
    }
    // M → N（均 ≠ 1）：先混 mono 再展开。
    let mono: Vec<f32> = samples.chunks(from as usize).map(avg_chunk).collect();
    map_channels(&mono, 1, to)
}

/// 线性插值重采样：`from_rate` Hz → `to_rate` Hz，**按帧操作**。
///
/// `samples` 是 `channels` 声道 interleaved 流；重采样以「帧」（一组 `channels`
/// 个样本）为单位：输出第 `i` 帧在源帧相位 `pos = i * from / to`（Q32 定点、
/// 逐帧独立计算，无累积漂移）处对相邻两帧**逐分量**线性插值。这样多声道的
/// 帧对齐天然保持（L/R 同相位），等价于对各声道分别用相同相位重采样。
///
/// 输出长度 = `ceil(frames * to / from) * channels`；末尾越界夹持到最后一帧
/// （hold）。尾部不足一帧的残样被丢弃（< 1 帧，文档化策略）。采样率相等时
/// 原样拷贝直通；需要重采样但没有完整帧时返回空。
pub fn resample_linear(samples: &[f32], channels: u16, from_rate: u32, to_rate: u32) -> Vec<f32> {
    debug_assert!(
        from_rate > 0 && to_rate > 0,
        "采样率必须非零（AudioSpec 已保证）"
    );
    debug_assert!(channels > 0, "声道数必须非零（AudioSpec 已保证）");
    let ch = usize::from(channels);
    if from_rate == to_rate {
        // 无需重采样：纯拷贝直通（不对齐做任何处理）。
        return samples.to_vec();
    }
    let frames = samples.len() / ch; // 完整帧数；部分尾帧丢弃
    if frames == 0 {
        return Vec::new();
    }
    let out_frames = (frames as u64 * u64::from(to_rate)).div_ceil(u64::from(from_rate));
    let last_frame = frames - 1;
    let mut out = Vec::with_capacity(out_frames as usize * ch);
    for i in 0..out_frames {
        // pos = i * from * 2^32 / to：u128 中间量保证不溢出、无累积漂移。
        let pos = ((u128::from(i) * u128::from(from_rate)) << 32) / u128::from(to_rate);
        // frac ∈ [0, 1)：Q32 小数部分换算为 f32（确定性舍入）。
        let frac = ((pos & (Q32 - 1)) as f32) / (Q32 as f32);
        let base0 = (idx_of(pos).min(last_frame)) * ch;
        let base1 = (idx_of(pos) + 1).min(last_frame) * ch;
        for c in 0..ch {
            let s0 = samples[base0 + c];
            let s1 = samples[base1 + c];
            out.push(s0 + (s1 - s0) * frac);
        }
    }
    out
}

/// Q32 定点相位的整数部分（帧下标）。
fn idx_of(pos: u128) -> usize {
    (pos >> 32) as usize
}

/// 一步到位：把 `source` 规格的 interleaved PCM 转换为 `device` 规格
/// （先声道映射、后按帧重采样；两者一致时原样返回）。
///
/// 这是 desktop 音频 facade 入环前的唯一转换入口——回调线程因此零 DSP。
pub fn convert_spec(samples: &[f32], source: AudioSpec, device: AudioSpec) -> Vec<f32> {
    let mapped = map_channels(samples, source.channels(), device.channels());
    resample_linear(
        &mapped,
        device.channels(),
        source.sample_rate(),
        device.sample_rate(),
    )
}

/// 按 chunk 实际长度求平均（空 chunk 返回 0.0，不会除零）。
fn avg_chunk(chunk: &[f32]) -> f32 {
    let sum: f32 = chunk.iter().sum();
    sum / chunk.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(rate: u32, ch: u16) -> AudioSpec {
        AudioSpec::new(rate, ch).expect("valid spec")
    }

    #[test]
    fn same_spec_is_bit_identical_passthrough() {
        let src = spec(24_000, 1);
        let samples = vec![0.25_f32, -0.5, 0.75, f32::MIN_POSITIVE];
        assert_eq!(map_channels(&samples, 1, 1), samples);
        assert_eq!(resample_linear(&samples, 1, 24_000, 24_000), samples);
        assert_eq!(convert_spec(&samples, src, src), samples);
        // 空 input 在任何规格组合下都是空输出。
        assert!(convert_spec(&[], src, spec(48_000, 2)).is_empty());
    }

    #[test]
    fn mono_to_stereo_duplicates_and_stereo_to_mono_averages() {
        assert_eq!(map_channels(&[0.5, -0.5], 1, 2), vec![0.5, 0.5, -0.5, -0.5]);
        assert_eq!(
            map_channels(&[1.0, 0.0, -1.0, 0.5], 2, 1),
            vec![0.5, -0.25],
            "stereo→mono 按帧平均"
        );
        // 部分尾帧按实际声道数平均（3 样本 stereo 流 = 1 整帧 + 单样本尾巴）。
        assert_eq!(map_channels(&[1.0, 0.0, -0.5], 2, 1), vec![0.5, -0.5]);
        // M,N 均 ≠1 的保守路径：先混 mono 再展开。
        assert_eq!(map_channels(&[1.0, 0.0], 2, 3), vec![0.5, 0.5, 0.5]);
    }

    #[test]
    fn dc_signal_survives_any_rate_conversion_exactly() {
        // 恒定直流经任意比率重采样必须逐位不变（插值的平凡情形）。
        for (from, to) in [(24_000_u32, 48_000), (48_000, 24_000), (44_100, 48_000)] {
            let out = resample_linear(&vec![0.125_f32; 1000], 1, from, to);
            assert_eq!(out, vec![0.125_f32; out.len()], "{from}→{to}");
        }
        // stereo 直流同样逐位不变且保持 L==R 成对。
        let out = resample_linear(&vec![0.125_f32; 1000], 2, 44_100, 48_000);
        assert_eq!(out, vec![0.125_f32; out.len()]);
    }

    #[test]
    fn upsampling_interpolates_exactly_on_half_steps() {
        // mono [0, 1] 从 24k 升到 48k：相位落在 0、0.5、1、1.5(夹持)。
        let out = resample_linear(&[0.0_f32, 1.0], 1, 24_000, 48_000);
        assert_eq!(out, vec![0.0, 0.5, 1.0, 1.0]);
    }

    #[test]
    fn stereo_frames_are_resampled_with_aligned_components() {
        // 两帧 stereo：(L,R) = (0,1) → (2,3)，24k→48k（每帧出两帧）。
        // 帧相位 i/2 ∈ {0,.5,1,1.5}，L 与 R 同相位独立插值 → 帧对齐不破坏。
        let frames = [0.0_f32, 1.0, 2.0, 3.0];
        let out = resample_linear(&frames, 2, 24_000, 48_000);
        assert_eq!(
            out,
            vec![
                0.0, 1.0, // 帧0：相位 0
                1.0, 2.0, // 帧0.5：分量各自取中点
                2.0, 3.0, // 帧1
                2.0, 3.0, // 帧1.5：夹持到末帧
            ]
        );
    }

    #[test]
    fn downsampling_takes_every_other_frame_for_exact_ratio() {
        // 48k → 24k：帧相位恰好落在偶数帧下标，权重为 0，应精确取原帧。
        let input: Vec<f32> = (0..16_i16).map(f32::from).collect();
        let out = resample_linear(&input, 1, 48_000, 24_000);
        assert_eq!(out.len(), 8);
        assert_eq!(out, input.iter().step_by(2).copied().collect::<Vec<_>>());
    }

    fn div_ceil(a: u64, b: u64) -> u64 {
        a.div_ceil(b)
    }

    #[test]
    fn output_length_formula_holds_for_non_integer_ratios() {
        // 44100 → 48000：out_len = ceil(len * 480/441)（mono）。
        for len in [1_usize, 2, 99, 1000, 4_801] {
            let expected = div_ceil(len as u64 * 48_000, 44_100) as usize;
            let out = resample_linear(&vec![0.0_f32; len], 1, 44_100, 48_000);
            assert_eq!(out.len(), expected, "len={len}");
        }
        // 反向同样成立；stereo 输出恒为偶数个样本。
        let out = resample_linear(&vec![0.0_f32; 999], 2, 48_000, 44_100);
        assert_eq!(
            out.len(),
            (div_ceil((999 / 2) as u64 * 44_100, 48_000) * 2) as usize
        );
        assert_eq!(out.len() % 2, 0);
    }

    #[test]
    fn single_sample_input_holds_value_across_all_outputs() {
        let out = resample_linear(&[0.75_f32], 1, 8_000, 19_200);
        assert_eq!(out, vec![0.75_f32; out.len()]);
        assert!(!out.is_empty());
        // 尾部残样（不足一帧）被丢弃：2 声道只喂 1 个样本 → 空。
        assert!(resample_linear(&[0.5_f32], 2, 8_000, 16_000).is_empty());
    }

    #[test]
    fn resampling_is_deterministic_bitwise_across_runs() {
        // 正弦扫出的伪随机波形跑两遍，要求逐位相同。
        let input: Vec<f32> = (0..4096).map(|i| (i as f32 * 0.037).sin() * 0.9).collect();
        let a = resample_linear(&input, 1, 44_100, 47_999);
        let b = resample_linear(&input, 1, 44_100, 47_999);
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.to_bits(), y.to_bits(), "重采样必须逐位确定");
        }
    }

    #[test]
    fn mono_24k_to_device_48k_stereo_end_to_end() {
        // 第一版主路径：mono 24k TTS → 48k stereo 设备。
        let source = spec(24_000, 1);
        let device = spec(48_000, 2);
        let input = vec![0.0_f32, 1.0];
        let out = convert_spec(&input, source, device);
        // mapped = [0,0,1,1]（两帧），帧相位 i/2 ∈ {0,.5,1,1.5}：
        assert_eq!(
            out,
            vec![0.0, 0.0, 0.5, 0.5, 1.0, 1.0, 1.0, 1.0],
            "L==R 且等于手算线性插值"
        );
        // 帧完整性：相邻样本两两相等（L==R）。
        for pair in out.chunks(2) {
            assert_eq!(pair[0], pair[1]);
        }
    }

    #[test]
    fn mono_24k_to_odd_device_rate_keeps_length_contract() {
        let source = spec(24_000, 1);
        let device = spec(44_100, 1);
        let input = vec![0.5_f32; 240]; // 10 ms
        let out = convert_spec(&input, source, device);
        assert_eq!(out.len(), (240 * 44_100_u64).div_ceil(24_000) as usize); // = 441
    }

    #[test]
    fn stereo_source_to_mono_device_downmixes_after_no_rate_change() {
        let source = spec(48_000, 2);
        let device = spec(48_000, 1);
        let out = convert_spec(&[1.0, 0.0, -1.0, 1.0], source, device);
        assert_eq!(out, vec![0.5, 0.0]);
    }
}
