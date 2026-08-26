//! 音频流规格 [`AudioSpec`]：raw PCM 字节流的解释参数（采样率 + 声道数）。
//!
//! 字段私有、只能经 `AudioSpec::new` 构造并在构造时做**非零校验**，
//! 使「0 采样率 / 0 声道」成为不可表示状态。

use crate::error::{Error, Result};

/// raw PCM 流的规格：采样率与声道数。
///
/// 默认 24 000 Hz / 1 声道（OpenAI-compatible `/audio/speech` `pcm` 输出的
/// 常见默认），可用 [`AudioSpec::DEFAULT_SAMPLE_RATE`] /
/// [`AudioSpec::DEFAULT_CHANNELS`] 引用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioSpec {
    sample_rate: u32,
    channels: u16,
}

impl AudioSpec {
    /// 默认采样率：24 kHz。
    pub const DEFAULT_SAMPLE_RATE: u32 = 24_000;
    /// 默认声道数：单声道。
    pub const DEFAULT_CHANNELS: u16 = 1;

    /// 创建规格。`sample_rate` 与 `channels` 必须非零，否则返回
    /// [`Error::InvalidAudioConfig`]。
    pub fn new(sample_rate: u32, channels: u16) -> Result<Self> {
        if sample_rate == 0 {
            return Err(Error::InvalidAudioConfig {
                message: "sample_rate 必须非零".to_string(),
            });
        }
        if channels == 0 {
            return Err(Error::InvalidAudioConfig {
                message: "channels 必须非零".to_string(),
            });
        }
        Ok(Self {
            sample_rate,
            channels,
        })
    }

    /// 采样率（Hz）。
    pub const fn sample_rate(self) -> u32 {
        self.sample_rate
    }

    /// 声道数。多声道样本按 interleaved 排列（L R L R …）。
    pub const fn channels(self) -> u16 {
        self.channels
    }
}

impl Default for AudioSpec {
    fn default() -> Self {
        // 常量本身非零，unwrap 不可能触发。
        Self::new(Self::DEFAULT_SAMPLE_RATE, Self::DEFAULT_CHANNELS)
            .expect("default AudioSpec constants are non-zero")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_24khz_mono() {
        let spec = AudioSpec::default();
        assert_eq!(spec.sample_rate(), 24_000);
        assert_eq!(spec.channels(), 1);
    }

    #[test]
    fn explicit_values_round_trip() {
        let spec = AudioSpec::new(44_100, 2).expect("valid");
        assert_eq!(spec.sample_rate(), 44_100);
        assert_eq!(spec.channels(), 2);
    }

    #[test]
    fn zero_sample_rate_or_channels_is_rejected() {
        assert!(matches!(
            AudioSpec::new(0, 1),
            Err(Error::InvalidAudioConfig { .. })
        ));
        assert!(matches!(
            AudioSpec::new(24_000, 0),
            Err(Error::InvalidAudioConfig { .. })
        ));
        // 错误信息可观测。
        let err = AudioSpec::new(0, 0).unwrap_err();
        assert!(err.to_string().contains("sample_rate"));
    }
}
