//! 增量 raw PCM 解码器 [`PcmS16LeDecoder`]：无 I/O、无 async，支持**任意字节切割**。
//!
//! 关键不变量（与 [`crate::sse::SseDecoder`] 同一哲学）：**绝不假设单个网络
//! chunk 按样本对齐**。奇数字节 chunk 的最后一个字节是半个 s16 样本（低字节），
//! 保留到下一个 chunk 与其高字节拼接；因此任意切割粒度下解码结果逐位一致。
//!
//! 输出归一化：`f32 = i16 / 32768.0`。除数取 32 768 保证闭区间 [-1, 1]：
//! `i16::MIN → -1.0` 精确成立，`i16::MAX → 0.9999695…` 不越界
//! （若除以 32 767 则负半轴会超出 -1，违反输出域约定）。
//! 多声道按 interleaved **原序透传**：帧边界每 `spec.channels()` 个样本一处，
//! 本层不做去交错。

use crate::audio::AudioSpec;
use crate::error::{Error, Result};

/// 归一化系数：1 / 32768（2 的幂，乘法精确）。
const S16_SCALE: f32 = 1.0 / 32_768.0;

/// 增量 little-endian signed 16-bit raw PCM 解码器。
///
/// 用法：反复 `decode(chunk)` 喂任意切分的字节、取走当批 f32 样本；
/// 流结束后调一次 [`finish`](Self::finish) 校验总字节数按样本对齐。
#[derive(Debug, Clone)]
pub struct PcmS16LeDecoder {
    /// 流规格（只影响文档语义/帧长换算；s16 样本恒为 2 字节）。
    spec: AudioSpec,
    /// 上一个 chunk 遗留的悬挂低字节（半个样本），等下一 chunk 的高字节。
    carry: Option<u8>,
}

impl PcmS16LeDecoder {
    /// 为给定规格创建解码器。
    pub fn new(spec: AudioSpec) -> Self {
        Self { spec, carry: None }
    }

    /// 流规格（只读视图）。
    pub const fn spec(&self) -> AudioSpec {
        self.spec
    }

    /// 解码一个任意切割的字节 chunk，返回其中**完整样本**对应的 f32 序列
    /// （interleaved 原序）。
    ///
    /// 若上一轮遗留了悬挂低字节，则先与本 chunk 的首个字节拼成一个样本；
    /// 本 chunk 自身末尾若又剩单个字节，同样保留到下一轮。
    /// 空 chunk 合法（返回空 Vec，状态不变）。
    pub fn decode(&mut self, chunk: &[u8]) -> Vec<f32> {
        let pending = usize::from(self.carry.is_some());
        let total = chunk.len() + pending;
        let mut out = Vec::with_capacity(total / 2);

        // lo = 待配对的低字节。遍历中每凑齐 (lo, hi) 即产出一个样本，
        // 收尾时 lo 若仍有值即为新的悬挂字节——奇偶处理天然统一。
        let mut lo = self.carry.take();
        for &byte in chunk {
            match lo.take() {
                Some(low) => out.push(s16_to_f32(i16::from_le_bytes([low, byte]))),
                None => lo = Some(byte),
            }
        }
        self.carry = lo;
        out
    }

    /// 流结束校验。总字节数按样本对齐（无悬挂字节）→ `Ok(())`；
    /// 否则返回 [`Error::TruncatedPcm`]，明确指出残留的悬挂字节——
    /// **绝不静默丢弃或臆造补零样本**。
    ///
    /// 消费 `self`：finish 之后解码器不可再用（状态机终态）。
    pub fn finish(self) -> Result<()> {
        if self.carry.is_some() {
            return Err(Error::TruncatedPcm { trailing_bytes: 1 });
        }
        Ok(())
    }
}

/// i16 → 归一化 f32 ∈ [-1, 1]（见模块文档的除数论证）。
fn s16_to_f32(raw: i16) -> f32 {
    f32::from(raw) * S16_SCALE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mono() -> AudioSpec {
        AudioSpec::new(24_000, 1).expect("valid spec")
    }

    fn le_bytes(samples: &[i16]) -> Vec<u8> {
        samples.iter().flat_map(|s| s.to_le_bytes()).collect()
    }

    fn decode_all(spec: AudioSpec, bytes: &[u8], cut: usize) -> Vec<f32> {
        let mut dec = PcmS16LeDecoder::new(spec);
        let mut out = Vec::new();
        for chunk in bytes.chunks(cut.max(1)) {
            out.extend(dec.decode(chunk));
        }
        dec.finish().expect("aligned stream");
        out
    }

    #[test]
    fn extremes_map_into_closed_unit_interval() {
        let bytes = le_bytes(&[i16::MIN, -1, 0, 1, i16::MAX]);
        assert_eq!(
            decode_all(mono(), &bytes, bytes.len()),
            vec![
                -1.0, // i16::MIN 精确映射到下界
                -1.0 / 32_768.0,
                0.0,
                1.0 / 32_768.0,
                32_767.0 / 32_768.0, // 正上界略小于 +1.0，但不越界
            ]
        );
    }

    #[test]
    fn arbitrary_odd_cuts_produce_identical_output() {
        // 7 个样本 = 14 字节，覆盖各种奇偶组合的切割粒度。
        let samples = [123, -456, 789, -32768, 32767, -1, 4096];
        let bytes = le_bytes(&samples);
        let expected: Vec<f32> = samples.iter().map(|&s| f32::from(s) * S16_SCALE).collect();

        for cut in [1usize, 2, 3, 4, 5, 13, 14, 100] {
            assert_eq!(decode_all(mono(), &bytes, cut), expected, "cut={cut}");
        }
    }

    #[test]
    fn hanging_low_byte_is_carried_to_next_chunk() {
        let mut dec = PcmS16LeDecoder::new(mono());
        // 单字节 chunk：只有半个样本，不产出任何值。
        assert!(dec.decode(&[0xCD]).is_empty());
        // 下一个 chunk 的首字节补成高字节：LE 拼回 [低 0xCD, 高 0xAB] = -21555。
        let raw = i16::from_le_bytes([0xCD, 0xAB]);
        assert_eq!(dec.decode(&[0xAB]), vec![f32::from(raw) * S16_SCALE]);
        dec.finish().expect("no hanging byte left");
    }

    #[test]
    fn finish_rejects_hanging_trailing_byte() {
        let mut dec = PcmS16LeDecoder::new(mono());
        assert_eq!(dec.decode(&[0x34, 0x12]).len(), 1);
        assert!(dec.decode(&[0x56]).is_empty()); // 又剩半格

        let err = dec.finish().expect_err("odd total byte count");
        match err {
            Error::TruncatedPcm { trailing_bytes } => assert_eq!(trailing_bytes, 1),
            other => panic!("unexpected error: {other:?}"),
        }
        assert!(err.to_string().contains("悬挂字节"));
    }

    #[test]
    fn empty_chunks_and_empty_stream_are_noops() {
        let mut dec = PcmS16LeDecoder::new(mono());
        assert!(dec.decode(&[]).is_empty());
        assert!(dec.decode(&[]).is_empty());
        dec.finish().expect("empty stream is aligned");

        let mut dec = PcmS16LeDecoder::new(mono());
        assert!(dec.decode(&[0x00]).is_empty());
        assert!(dec.decode(&[]).is_empty());
        assert_eq!(dec.decode(&[0x00]).len(), 1);
        dec.finish().expect("aligned");
    }

    #[test]
    fn stereo_interleaved_order_is_preserved() {
        // 双声道：帧 = (L, R)。L/R 交替幅度便于辨认顺序。
        let frames = [(100_i16, -100_i16), (200, -200), (300, -300)];
        let bytes = le_bytes(
            &frames
                .iter()
                .copied()
                .flat_map(|(l, r)| [l, r])
                .collect::<Vec<_>>(),
        );

        let stereo = AudioSpec::new(48_000, 2).expect("valid");
        let got = decode_all(stereo, &bytes, 3); // 故意用奇数切割打乱帧边界
        let expected: Vec<f32> = frames
            .iter()
            .flat_map(|&(l, r)| [f32::from(l) * S16_SCALE, f32::from(r) * S16_SCALE])
            .collect();
        assert_eq!(got, expected);
        assert_eq!(stereo.channels(), 2);
    }
}
