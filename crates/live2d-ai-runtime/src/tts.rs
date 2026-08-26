//! TTS：OpenAI-compatible `/audio/speech`。
//!
//! 职责边界（RFC D7）：
//! - **不内置任何 TTS 引擎**、不调各家私有 SDK；只发统一 HTTP 请求；
//! - 网络方法**原样转发 chunked bytes**——传输层不解码、不缓冲整段；
//!   raw PCM 的增量解码 / RMS / 有界队列见 [`crate::audio`]（本批只保证
//!   `response_format = "pcm"` = s16le，WAV/MP3 明确 UnsupportedFormat）；
//! - 非 2xx 一律 [`Error::Status`]（含状态码与截断响应体），可观测。

use std::pin::Pin;

use futures_util::Stream;
use futures_util::stream::try_unfold;
use serde::Serialize;

use crate::config::TtsConfig;
use crate::error::{Error, Result};

/// `/audio/speech` 请求体。可选字段缺省时从 wire 上整体省略
/// （`response_format` 默认配置下恒为 `"pcm"`，见 [`TtsConfig::new`]）。
#[derive(Serialize)]
struct SpeechRequestBody<'a> {
    input: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<&'a str>,
    voice: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<&'a str>,
}

impl<'a> SpeechRequestBody<'a> {
    /// 从 TTS 配置组装请求体（独立成函数便于单测 wire 形状）。
    fn from_config(input: &'a str, tts: &'a TtsConfig) -> Self {
        Self {
            input,
            model: tts.model.as_deref(),
            voice: &tts.voice,
            response_format: tts.response_format.as_deref(),
        }
    }
}

impl crate::OpenAiClient {
    /// 合成语音：POST `<tts.base_url>/audio/speech`，返回上游的 chunked 字节流。
    ///
    /// 流的每一项是 `Ok(Vec<u8>)`（一个上游 chunk 的拷贝）/ `Err(Error)`；
    /// 错误终止流。本方法不做任何音频解码，也不缓冲整段音频。
    pub async fn synthesize_speech(
        &self,
        input: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Vec<u8>>> + Send>>> {
        let url = crate::join_endpoint(&self.tts.base_url, "/audio/speech")?;
        let body = SpeechRequestBody::from_config(input, &self.tts);

        let mut request = self.http.post(url).json(&body);
        if let Some(key) = &self.tts.api_key {
            request = request.bearer_auth(key.expose_secret());
        }

        let response = request.send().await?;
        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(Error::status(status, &text));
        }

        // 逐块转发：reqwest 0.12 不再 re-export Bytes，这里以 Vec<u8> 承载每个
        // 上游 chunk（每块一次小拷贝，换取依赖克制）。
        Ok(Box::pin(try_unfold(response, |mut response| async move {
            match response.chunk().await {
                Ok(Some(chunk)) => Ok(Some((chunk.to_vec(), response))),
                Ok(None) => Ok(None),
                Err(e) => Err(Error::Http(e)),
            }
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_body_omits_optional_fields() {
        let minimal = SpeechRequestBody {
            input: "你好",
            model: None,
            voice: "alloy",
            response_format: None,
        };
        let json = serde_json::to_value(&minimal).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "input": "你好", "voice": "alloy" })
        );

        let full = SpeechRequestBody {
            input: "hi",
            model: Some("tts-1"),
            voice: "alloy",
            response_format: Some("pcm"),
        };
        let json = serde_json::to_value(&full).unwrap();
        assert_eq!(json["model"], "tts-1");
        assert_eq!(json["response_format"], "pcm");
    }

    #[test]
    fn default_config_requests_pcm_on_the_wire() {
        // 默认配置必须把 response_format=pcm 发上线（本批唯一保证可解码格式）。
        let tts = TtsConfig::new("http://127.0.0.1:8000/v1", "nova");
        assert!(crate::audio::ensure_supported_format(tts.response_format.as_deref()).is_ok());

        let body = SpeechRequestBody::from_config("hi", &tts);
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "input": "hi", "voice": "nova", "response_format": "pcm" })
        );
    }

    #[test]
    fn explicit_none_still_omits_response_format() {
        let mut tts = TtsConfig::new("http://127.0.0.1:8000/v1", "nova");
        tts.response_format = None; // 显式交给服务端默认
        let body = SpeechRequestBody::from_config("hi", &tts);
        let json = serde_json::to_value(&body).unwrap();
        assert!(json.get("response_format").is_none());
    }
}
