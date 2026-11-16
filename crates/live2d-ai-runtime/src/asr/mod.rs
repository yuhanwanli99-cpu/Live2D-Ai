//! 语音识别（ASR）：核心的一次 HTTP 转发（2026-11-16）。
//!
//! # 为什么在核心里，而不是 Mod
//!
//! 2026-10-19「一 Mod 一本地仓库」把 `voice-input` Mod 的实现移出主仓之后，
//! 产品听写走的是**浏览器 Web Speech**——音频出本机送浏览器厂商云端，而且
//! 「识别 → 送进输入框」这两步与核心没有关系。这一档把它改成核心的一次
//! 转发：浏览器把 WAV 交给 `POST /api/v1/voice/asr`，核心转给 DeepSeek。
//!
//! 三件事**刻意不做**（它们随 Mod 一起退休）：
//! - 不恢复唤醒词 / sidecar / `run_sidecar` / `backend = mock|sidecar`；
//! - 不把 `clean_transcript` 搬进主仓——本端点只做首尾空白；
//! - 识别成功**不自动开一轮对话**：文本回到输入框，由用户决定发不发。
//!
//! 上游没有流式、也没有时间戳，所以这里也**不假装**有：一次请求、一份整段
//! 音频、一份文本。

pub mod deepseek;

/// 上游接受的容器：16 位 PCM WAV 或 Ogg Opus。
///
/// 判据只看 Content-Type 的**主类型**（`audio/wav` / `audio/x-wav` /
/// `audio/wave` / `audio/ogg`），不看扩展名——前端是从 `MediaRecorder` /
/// 自建 WAV 编码器直接 POST 上来的，没有文件名。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioContainer {
    /// `audio/wav`（16 位 PCM）。
    Wav,
    /// `audio/ogg`（Ogg Opus）。
    Ogg,
}

impl AudioContainer {
    /// Content-Type 主类型 → 容器；不认识 → `None`（路由层回 415）。
    ///
    /// 参数（`; codecs=...` 之类）忽略。
    #[must_use]
    pub fn from_content_type(raw: &str) -> Option<Self> {
        let main = raw
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        match main.as_str() {
            "audio/wav" | "audio/x-wav" | "audio/wave" | "audio/vnd.wave" => Some(Self::Wav),
            "audio/ogg" | "audio/opus" | "application/ogg" => Some(Self::Ogg),
            _ => None,
        }
    }

    /// 转发给上游时用的 Content-Type（multipart 里那一份的 `Content-Type`）。
    #[must_use]
    pub const fn mime(self) -> &'static str {
        match self {
            Self::Wav => "audio/wav",
            Self::Ogg => "audio/ogg",
        }
    }

    /// 上游要求的文件后缀（multipart 的 `filename`）。
    #[must_use]
    pub const fn filename(self) -> &'static str {
        match self {
            Self::Wav => "audio.wav",
            Self::Ogg => "audio.ogg",
        }
    }
}

/// 上游单次请求的音频上限：8 MiB（接口契约，超过上游会拒）。
pub const MAX_UPLOAD_BYTES: usize = 8 * 1024 * 1024;

/// 上游单次请求的音频时长上限：120 秒（接口契约）。
pub const MAX_DURATION_MS: u64 = 120_000;

/// 从 WAV 头读时长（毫秒）。读不出来（不是 WAV / 头不完整）→ `None`。
///
/// **只读头**，不解析 `data` 块内容：路由层要在打上游之前判 120 秒，
/// 拿到的是一整段已经读完的字节，但这里只关心 `fmt ` / `data` 两个块声明的长度。
/// 时长 = `data_size / (sample_rate × channels × 2)`（16 位单/多声道 PCM）。
#[must_use]
pub fn wav_duration_ms(bytes: &[u8]) -> Option<u64> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut pos = 12usize;
    let mut rate_channels: Option<(u64, u64)> = None;
    let mut data_size: Option<u64> = None;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes([
            bytes[pos + 4],
            bytes[pos + 5],
            bytes[pos + 6],
            bytes[pos + 7],
        ]) as u64;
        let body_start = pos + 8;
        let body_end = body_start.saturating_add(size as usize).min(bytes.len());
        let body = &bytes[body_start..body_end];
        match id {
            b"fmt " if body.len() >= 8 => {
                let channels = u16::from_le_bytes([body[2], body[3]]) as u64;
                let rate = u32::from_le_bytes([body[4], body[5], body[6], body[7]]) as u64;
                if channels > 0 && rate > 0 {
                    rate_channels = Some((rate, channels));
                }
            }
            b"data" => {
                data_size = Some(size);
            }
            _ => {}
        }
        if rate_channels.is_some() && data_size.is_some() {
            break;
        }
        pos = body_start + size as usize + ((size as usize) & 1);
    }
    let (rate, channels) = rate_channels?;
    let data = data_size?;
    let bytes_per_second = rate.saturating_mul(channels).saturating_mul(2);
    if bytes_per_second == 0 {
        return None;
    }
    Some(data.saturating_mul(1000) / bytes_per_second)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav_header(rate: u32, channels: u16, data_len: u32) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36u32 + data_len).to_le_bytes());
        v.extend_from_slice(b"WAVE");
        v.extend_from_slice(b"fmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes()); // PCM
        v.extend_from_slice(&channels.to_le_bytes());
        v.extend_from_slice(&rate.to_le_bytes());
        v.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
        v.extend_from_slice(&(channels * 2).to_le_bytes());
        v.extend_from_slice(&16u16.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&data_len.to_le_bytes());
        v
    }

    #[test]
    fn content_type_maps_the_two_supported_containers() {
        for (raw, want) in [
            ("audio/wav", Some(AudioContainer::Wav)),
            ("audio/x-wav", Some(AudioContainer::Wav)),
            ("audio/wav; codecs=1", Some(AudioContainer::Wav)),
            ("AUDIO/WAV", Some(AudioContainer::Wav)),
            ("audio/ogg", Some(AudioContainer::Ogg)),
            ("audio/ogg; codecs=opus", Some(AudioContainer::Ogg)),
            ("audio/webm", None),
            ("application/json", None),
            ("", None),
        ] {
            assert_eq!(AudioContainer::from_content_type(raw), want, "{raw:?}");
        }
    }

    #[test]
    fn wav_duration_is_read_from_the_header() {
        // 1 秒：16000 Hz 单声道 16 位 = 32000 字节。
        let wav = wav_header(16_000, 1, 32_000);
        assert_eq!(wav_duration_ms(&wav), Some(1_000));
        // 120 秒整：48000 Hz 立体声 16 位。
        let data = 48_000u32 * 2 * 2 * 120;
        let wav = wav_header(48_000, 2, data);
        assert_eq!(wav_duration_ms(&wav), Some(120_000));
        // 超过 120 秒。
        let wav = wav_header(48_000, 2, data + 48_000 * 2 * 2);
        assert!(wav_duration_ms(&wav).unwrap() > MAX_DURATION_MS);
    }

    #[test]
    fn wav_duration_is_none_for_non_wav_bytes() {
        assert_eq!(wav_duration_ms(b""), None);
        assert_eq!(wav_duration_ms(b"OggS\0\0\0\0"), None);
        assert_eq!(wav_duration_ms(b"RIFF\0\0\0\0WAVE"), None, "没有 fmt/data");
    }
}
