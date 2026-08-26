//! 纯音频处理层（本批契约：**只保证 raw PCM**）。
//!
//! `response_format = "pcm"` 在第一版里**明确定义**为：
//! **little-endian signed 16-bit interleaved raw PCM**（无文件头、无帧封装、
//! 字节流即样本流）。采样率与声道数由 [`AudioSpec`] 携带，默认 24 kHz / 单声道。
//!
//! 本模块全部是**纯算法/状态机**：无 I/O、无 async、无线程、无锁，可在任意
//! 执行器或裸线程上驱动。组成：
//!
//! - [`PcmS16LeDecoder`]：增量解码。接受**任意字节切割**的 chunk（含奇数字节），
//!   半个样本（1 字节）保留到下一个 chunk；输出归一化 f32 ∈ [-1, 1]，
//!   多声道按 interleaved 原序透传；`finish` 对悬挂字节明确报错。
//! - [`RmsMeter`]：从「实际被声卡消费的 f32 样本」滑动窗口计算 RMS，
//!   带 attack/release 平滑与 [0, 1] clamp；静音精确回 0，永不产出 NaN。
//! - [`SampleQueue`]：有界 f32 样本 FIFO。溢出策略显式——**接纳前缀并返回
//!   接纳数量**，绝不阻塞、绝不 panic（音频回调线程安全语义见下）。
//! - [`resample`]：最小确定性重采样与声道映射纯函数（[`convert_spec`] /
//!   [`map_channels`] / [`resample_linear`]）——TTS [`AudioSpec`] 与实际声卡
//!   config 不一致时，在**生产者线程**上完成设备域转换，音频回调零 DSP。
//!
//! # 队列实现说明（ringbuf 迁移路径）
//!
//! 当前用纯 std `VecDeque` 实现（单消费者 `&mut` API），零新增依赖。
//! 接入 cpal 回调需要跨线程无锁 SPSC 时，再把内部替换为 `ringbuf` 的
//! Producer/Consumer——对外 facade（[`SampleQueue`] 的方法签名与溢出策略）
//! 保持不变，依赖类型不泄漏到公共 API。desktop 侧第一批已按此路径接入
//! cpal + ringbuf（见 `crates/live2d-ai-desktop/src/audio.rs`）。
//!
//! # 格式门禁
//!
//! WAV / MP3 / FLAC 等容器或压缩格式一律返回
//! [`Error::UnsupportedFormat`](crate::Error::UnsupportedFormat)——本批不假装
//! 支持它们的流式解码。

mod decoder;
mod queue;
mod resample;
mod rms;
mod spec;

pub use decoder::PcmS16LeDecoder;
pub use queue::SampleQueue;
pub use resample::{convert_spec, map_channels, resample_linear};
pub use rms::RmsMeter;
pub use spec::AudioSpec;

/// 本批唯一保证支持的 TTS `response_format` 取值。
pub const SUPPORTED_FORMAT: &str = "pcm";

/// 格式门禁：校验 TTS `response_format` 是否为本批保证可解码的 raw PCM。
///
/// - `Some("pcm")`（大小写不敏感、允许首尾空白）→ `Ok(())`；
/// - `None`（未指定格式，交由服务端默认决定）与一切其他取值
///   （`"wav"` / `"mp3"` / …）→ [`Error::UnsupportedFormat`](crate::Error::UnsupportedFormat)。
///
/// 上层在把网络字节喂给 [`PcmS16LeDecoder`] 之前应先过这道门；
/// 默认配置下 [`TtsConfig`](crate::TtsConfig) 已固定请求 `"pcm"`。
pub fn ensure_supported_format(response_format: Option<&str>) -> crate::Result<()> {
    match response_format.map(str::trim) {
        Some(fmt) if fmt.eq_ignore_ascii_case(SUPPORTED_FORMAT) => Ok(()),
        other => Err(crate::Error::UnsupportedFormat {
            format: other.unwrap_or("<未指定，服务端默认>").to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;

    #[test]
    fn format_gate_accepts_pcm_case_insensitively() {
        assert!(ensure_supported_format(Some("pcm")).is_ok());
        assert!(ensure_supported_format(Some("PCM")).is_ok());
        assert!(ensure_supported_format(Some(" pcm ")).is_ok());
    }

    #[test]
    fn format_gate_rejects_other_formats_and_unspecified() {
        for bad in [None, Some("wav"), Some("mp3"), Some("opus"), Some("pcmx")] {
            let err = ensure_supported_format(bad).expect_err("must reject");
            assert!(
                matches!(err, Error::UnsupportedFormat { .. }),
                "got {err:?}"
            );
            // 可观测：错误文本携带具体格式。
            let text = err.to_string();
            assert!(text.contains("pcm"), "display: {text}");
        }
        let err = ensure_supported_format(None).unwrap_err();
        assert!(err.to_string().contains("未指定"));
    }
}
