//! WAV（RIFF/WAVE）容器解析：为「返回 WAV 而非裸 PCM」的 OpenAI 兼容 TTS
//! 服务补解码能力（2026-09-10，CosyVoice 3 接入准备）。
//!
//! 背景：CosyVoice 3 的 OpenAI 兼容层（如 CosyVoice3-API）在非流式模式下
//! 返回标准 WAV；而本 crate 的流式路径只认裸 s16le。这里提供**一次性解析**
//! （`data` 块整体取出），调用方负责决定是否缓冲整段响应。
//!
//! 只接受 **PCM 16-bit**：
//! - `audio_format == 1`（WAVE_FORMAT_PCM），或
//! - `audio_format == 0xFFFE`（WAVE_FORMAT_EXTENSIBLE）且子格式 GUID 前两字节为 1。
//!
//! 其它位深/编码明确报 [`Error::UnsupportedFormat`]，**不假装支持**。
//!
//! 未知块（`LIST` / `fact` / `bext` 等）按 RIFF 规则跳过（含奇数字节的填充位）。

use crate::audio::AudioSpec;
use crate::error::{Error, Result};

/// 归一化系数：1 / 32768（与 [`super::decoder`] 同口径，保证两路逐位一致）。
const S16_SCALE: f32 = 1.0 / 32_768.0;

/// WAV 解析结果：容器内声明的规格 + 归一化 PCM 样本（interleaved 原序）。
#[derive(Debug, Clone, PartialEq)]
pub struct WavPcm {
    /// 容器 `fmt ` 块声明的规格（**以容器为准**，可能与配置不一致）。
    pub spec: AudioSpec,
    /// 归一化到 [-1, 1] 的样本；多声道 interleaved 原序透传。
    pub samples: Vec<f32>,
}

/// 解析 WAV 字节为 [`WavPcm`]（仅 PCM 16-bit）。
pub fn parse_wav_s16(bytes: &[u8]) -> Result<WavPcm> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(Error::UnsupportedFormat {
            format: "wav(非 RIFF/WAVE 容器)".to_string(),
        });
    }

    // 扫描块：记录 fmt（有效格式/声道/采样率/位深）与 data 切片。
    let mut pos = 12usize;
    let mut fmt: Option<(u16, u16, u32, u16)> = None;
    let mut data: Option<&[u8]> = None;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes([
            bytes[pos + 4],
            bytes[pos + 5],
            bytes[pos + 6],
            bytes[pos + 7],
        ]) as usize;
        let body_start = pos + 8;
        // 声明长度可能超过实际字节（截断响应 / 流式 WAV 的 0xFFFFFFFF）：取交集。
        let body_end = body_start.saturating_add(size).min(bytes.len());
        let body = &bytes[body_start..body_end];

        if id == b"fmt " && body.len() >= 16 {
            let audio_format = u16::from_le_bytes([body[0], body[1]]);
            let channels = u16::from_le_bytes([body[2], body[3]]);
            let sample_rate = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
            let bits = u16::from_le_bytes([body[14], body[15]]);
            // WAVE_FORMAT_EXTENSIBLE：子格式 GUID 的前两个字节才是真实格式码。
            let effective = if audio_format == 0xFFFE && body.len() >= 26 {
                u16::from_le_bytes([body[24], body[25]])
            } else {
                audio_format
            };
            fmt = Some((effective, channels, sample_rate, bits));
        } else if id == b"data" {
            data = Some(body);
        }

        // RIFF：块体按偶数字节对齐（奇数长度补 1 字节填充）。
        pos = body_start + size + (size & 1);
    }

    let (audio_format, channels, sample_rate, bits) =
        fmt.ok_or_else(|| Error::UnsupportedFormat {
            format: "wav(缺少 fmt 块)".to_string(),
        })?;
    if audio_format != 1 {
        return Err(Error::UnsupportedFormat {
            format: format!("wav(audio_format={audio_format}，仅支持 PCM)"),
        });
    }
    if bits != 16 {
        return Err(Error::UnsupportedFormat {
            format: format!("wav({bits}-bit，仅支持 16-bit)"),
        });
    }
    let data = data.ok_or_else(|| Error::UnsupportedFormat {
        format: "wav(缺少 data 块)".to_string(),
    })?;

    let spec = AudioSpec::new(sample_rate, channels).map_err(|e| Error::InvalidAudioConfig {
        message: format!("WAV 头规格非法: {e}"),
    })?;

    // s16le → f32（与 PcmS16LeDecoder 同口径；WAV data 长度理论恒为偶数）。
    let mut samples = Vec::with_capacity(data.len() / 2);
    let mut lo: Option<u8> = None;
    for &byte in data {
        match lo.take() {
            Some(low) => samples.push(f32::from(i16::from_le_bytes([low, byte])) * S16_SCALE),
            None => lo = Some(byte),
        }
    }
    if lo.is_some() {
        return Err(Error::TruncatedPcm { trailing_bytes: 1 });
    }

    Ok(WavPcm { spec, samples })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个最小合法 WAV（PCM 16-bit + 可选附加块）。
    fn build_wav(
        channels: u16,
        sample_rate: u32,
        bits: u16,
        samples: &[i16],
        extra_chunk: bool,
    ) -> Vec<u8> {
        let data: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let mut body: Vec<u8> = Vec::new();
        if extra_chunk {
            // 一个奇数长度的未知块，验证「跳过 + 填充位」处理。
            body.extend_from_slice(b"LIST");
            body.extend_from_slice(&3u32.to_le_bytes());
            body.extend_from_slice(&[1, 2, 3, 0]); // 3 字节 + 1 字节填充
        }
        body.extend_from_slice(b"fmt ");
        body.extend_from_slice(&16u32.to_le_bytes());
        body.extend_from_slice(&1u16.to_le_bytes()); // PCM
        body.extend_from_slice(&channels.to_le_bytes());
        body.extend_from_slice(&sample_rate.to_le_bytes());
        let byte_rate = sample_rate * u32::from(channels) * u32::from(bits / 8);
        body.extend_from_slice(&byte_rate.to_le_bytes());
        body.extend_from_slice(&(channels * (bits / 8)).to_le_bytes()); // block align
        body.extend_from_slice(&bits.to_le_bytes());
        body.extend_from_slice(b"data");
        body.extend_from_slice(&(data.len() as u32).to_le_bytes());
        body.extend_from_slice(&data);

        let mut out: Vec<u8> = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(&body);
        out
    }

    #[test]
    fn parses_mono_24k_pcm16() {
        let wav = build_wav(1, 24_000, 16, &[0, 1000, -1000, i16::MIN, i16::MAX], false);
        let parsed = parse_wav_s16(&wav).expect("合法 WAV");
        assert_eq!(parsed.spec, AudioSpec::new(24_000, 1).unwrap());
        assert_eq!(parsed.samples.len(), 5);
        assert_eq!(parsed.samples[0], 0.0);
        assert_eq!(parsed.samples[3], -1.0);
    }

    #[test]
    fn skips_unknown_chunks_with_padding() {
        let wav = build_wav(1, 16_000, 16, &[1, 2, 3], true);
        let parsed = parse_wav_s16(&wav).expect("含未知块仍应解析");
        assert_eq!(parsed.spec, AudioSpec::new(16_000, 1).unwrap());
        assert_eq!(parsed.samples.len(), 3);
    }

    #[test]
    fn stereo_order_is_preserved() {
        let wav = build_wav(2, 44_100, 16, &[100, -100, 200, -200], false);
        let parsed = parse_wav_s16(&wav).expect("立体声");
        assert_eq!(parsed.spec, AudioSpec::new(44_100, 2).unwrap());
        assert!(parsed.samples[0] > 0.0 && parsed.samples[1] < 0.0);
    }

    #[test]
    fn rejects_non_riff() {
        let err = parse_wav_s16(b"not a wav at all").expect_err("必须拒绝");
        assert!(matches!(err, Error::UnsupportedFormat { .. }), "{err}");
    }

    #[test]
    fn rejects_non_pcm16() {
        let wav = build_wav(1, 24_000, 8, &[0, 1, 2], false);
        let err = parse_wav_s16(&wav).expect_err("8-bit 必须拒绝");
        let msg = err.to_string();
        assert!(msg.contains("8-bit"), "{msg}");
    }

    #[test]
    fn accepts_extensible_pcm_subformat() {
        // 手工构造 0xFFFE + 子格式 1（PCM）的 fmt 块（40 字节）。
        let samples: [i16; 2] = [123, -123];
        let data: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let mut fmt: Vec<u8> = Vec::new();
        fmt.extend_from_slice(&0xFFFEu16.to_le_bytes());
        fmt.extend_from_slice(&1u16.to_le_bytes()); // channels
        fmt.extend_from_slice(&24_000u32.to_le_bytes());
        fmt.extend_from_slice(&48_000u32.to_le_bytes());
        fmt.extend_from_slice(&2u16.to_le_bytes());
        fmt.extend_from_slice(&16u16.to_le_bytes());
        fmt.extend_from_slice(&22u16.to_le_bytes()); // cbSize
        fmt.extend_from_slice(&16u16.to_le_bytes()); // valid bits
        fmt.extend_from_slice(&3u32.to_le_bytes()); // channel mask
        fmt.extend_from_slice(&1u16.to_le_bytes()); // subformat 低 2 字节 = PCM
        fmt.extend_from_slice(&[0u8; 14]); // GUID 余下字节
        let mut body: Vec<u8> = Vec::new();
        body.extend_from_slice(b"fmt ");
        body.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
        body.extend_from_slice(&fmt);
        body.extend_from_slice(b"data");
        body.extend_from_slice(&(data.len() as u32).to_le_bytes());
        body.extend_from_slice(&data);
        let mut wav: Vec<u8> = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(body.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(&body);

        let parsed = parse_wav_s16(&wav).expect("extensible PCM 应被接受");
        assert_eq!(parsed.samples.len(), 2);
    }
}
