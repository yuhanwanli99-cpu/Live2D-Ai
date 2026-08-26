//! 统一错误类型 [`Error`]：HTTP 错误、状态码、JSON / SSE 解析失败都可观测。
//!
//! 可观测性边界：
//! - [`Error::Status`] 携带状态码与**截断后**的响应体片段（≤512 字符），便于定位
//!   上游 4xx/5xx 的具体报错（如 `invalid_api_key`）；
//! - [`Error::Http`] 来自 reqwest，其 Display 只包含 URL 与传输层描述——请求头
//!   （含 `Authorization` 密钥）永远不会进入错误文本；
//! - 密钥只进 `Authorization` 头，因此任何错误变体都不会携带密钥明文。

use reqwest::StatusCode;

/// crate 统一 Result 别名。
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// 响应体片段的最大保留字符数。
const MAX_SNIPPET_CHARS: usize = 512;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// `base_url` 缺失 scheme、非 http(s)，或无法解析为绝对 URL。
    #[error("非法 base_url: {url:?}")]
    InvalidBaseUrl {
        /// 原始（未脱敏必要——URL 不含密钥）的 base_url 字符串。
        url: String,
    },

    /// 连接/传输层错误。Display 只含 URL 与 IO 描述，不含请求头（密钥安全）。
    #[error("HTTP 传输错误: {0}")]
    Http(#[from] reqwest::Error),

    /// 上游返回非 2xx 状态码；附状态码与响应体片段（超长截断）便于观测。
    #[error("上游非成功状态 {status}: {body}")]
    Status {
        /// HTTP 状态码（如 `401 Unauthorized`）。
        status: StatusCode,
        /// 截断后的响应体片段。
        body: String,
    },

    /// SSE 流违反协议/语义：如 `data:` 不是合法 JSON 且也不是 `[DONE]`。
    #[error("SSE 解析错误: {message}")]
    Sse {
        /// 具体的解析失败描述。
        message: String,
    },

    /// JSON 序列化/反序列化失败。
    #[error("JSON 错误: {0}")]
    Json(#[from] serde_json::Error),

    /// 请求/输入的音频格式本批不支持：只保证 raw PCM
    /// （`"pcm"` = little-endian signed 16-bit raw PCM，见 [`crate::audio`]）。
    /// WAV / MP3 / FLAC 等一律明确报错，**不假装支持流式解码**。
    #[error(
        "不支持的音频格式 {format:?}: 本批次仅保证 \"pcm\"（little-endian signed 16-bit raw PCM）"
    )]
    UnsupportedFormat {
        /// 被拒绝的格式标识（`None` 时为占位描述，如「未指定」）。
        format: String,
    },

    /// PCM 字节流总长不是样本对齐的：流结束时残留不足一个 s16 样本（1 字节）的悬挂字节。
    #[error(
        "PCM 流长度未按样本对齐: 末尾残留 {trailing_bytes} 个悬挂字节，无法构成完整的 signed 16-bit 样本"
    )]
    TruncatedPcm {
        /// 残留的悬挂字节数（s16le 下恒为 1）。
        trailing_bytes: usize,
    },

    /// 音频相关配置非法（如采样率/声道数为 0、窗口或时间常数为负/非有限值）。
    #[error("音频配置非法: {message}")]
    InvalidAudioConfig {
        /// 具体原因描述。
        message: String,
    },
}

impl Error {
    /// 构造带截断响应体片段的状态错误（按字符截断，避免劈开 UTF-8）。
    pub(crate) fn status(status: StatusCode, body: &str) -> Self {
        let mut snippet: String = body.chars().take(MAX_SNIPPET_CHARS).collect();
        if body.chars().count() > MAX_SNIPPET_CHARS {
            snippet.push('…');
        }
        Self::Status {
            status,
            body: snippet,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_body_is_truncated_by_chars() {
        let long = "x".repeat(2000);
        let err = Error::status(StatusCode::INTERNAL_SERVER_ERROR, &long);
        match &err {
            Error::Status { status, body } => {
                assert_eq!(*status, StatusCode::INTERNAL_SERVER_ERROR);
                assert_eq!(body.chars().count(), MAX_SNIPPET_CHARS + 1); // 含省略号
                assert!(body.ends_with('…'));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
        // Display 可观测：状态码与片段都在。
        let text = err.to_string();
        assert!(text.contains("500"));
        assert!(text.contains("xxx"));
    }

    #[test]
    fn short_body_is_kept_verbatim() {
        let err = Error::status(StatusCode::UNAUTHORIZED, "invalid api key");
        match err {
            Error::Status { status, body } => {
                assert_eq!(status, StatusCode::UNAUTHORIZED);
                assert_eq!(body, "invalid api key");
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }
}
