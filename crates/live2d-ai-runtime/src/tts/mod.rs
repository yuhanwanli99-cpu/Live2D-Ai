//! TTS：**两档**，不是每家一个客户端。
//!
//! 职责边界（RFC D7）：
//! - **不内置任何 TTS 引擎**、不调各家私有 SDK；只发统一 HTTP 请求；
//! - 网络方法**原样转发 chunked bytes**——传输层不解码、不缓冲整段；
//!   raw PCM 的增量解码 / RMS / 有界队列见 [`crate::audio`]（本批只保证
//!   `response_format = "pcm"` = s16le，WAV/MP3 明确 UnsupportedFormat）；
//! - 非 2xx 一律 [`Error::Status`]（含状态码与截断响应体），可观测。
//!
//! # 两档（2026-11-16）
//!
//! | `[tts] provider` | 端点 | 送播放的时机 |
//! | --- | --- | --- |
//! | `openai`（缺省） | `<base_url>/audio/speech` | **逐块**：第一块到了就往下游送 |
//! | `deepseek` | 固定 `https://api.deepseek.com/v1/tools/tts` | **整句**：HTTP 正常结束才交出 |
//!
//! `openai` 这一档就是 2026-09 以来的行为（本地 MeloTTS / CosyVoice3 以及任何
//! OpenAI 兼容端点都走它）；`deepseek` 那一档见 [`deepseek`] 模块。
//! 缺省是 `openai`，所以**旧 toml 不写这个键时行为一字不变**。

pub mod deepseek;

use std::pin::Pin;

use futures_util::Stream;
use futures_util::stream::try_unfold;
use serde::Serialize;

use crate::config::{TtsConfig, TtsProvider};
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
    /// 合成语音：按 `[tts] provider` 分派到两档之一。
    ///
    /// 流的每一项是 `Ok(Vec<u8>)`（一段 PCM 字节）/ `Err(Error)`；错误终止流。
    /// 本方法不做任何音频解码；两档的**缓冲语义不同**（见模块头表）：
    /// `openai` 逐块转发，`deepseek` 整句收完再交。
    pub async fn synthesize_speech(
        &self,
        input: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Vec<u8>>> + Send>>> {
        match self.tts.provider {
            TtsProvider::OpenAi => self.synthesize_speech_openai(input).await,
            TtsProvider::DeepSeek => self.synthesize_speech_deepseek(input).await,
        }
    }

    /// `openai` 档：POST `<tts.base_url>/audio/speech`，返回上游的 chunked 字节流。
    ///
    /// **逐块**：每拿到一个上游 chunk 就交给下游（播放与口型都按到达序推进）。
    /// 这一档的缓冲语义不因 DeepSeek 档的整句需求而改变。
    pub async fn synthesize_speech_openai(
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

    /// `deepseek` 档：整句收完再交给播放侧（见 [`deepseek`] 模块头）。
    ///
    /// 三个「这一句不出声、只出文字」的情形（钥匙为空 / 超长句 / 非法音色）
    /// 在 [`deepseek::synthesize`] 里收口成**空音频**，与「未配置 TTS」同一条路；
    /// 其余失败一律 `Err`，播放侧一个样本都收不到。
    pub async fn synthesize_speech_deepseek(
        &self,
        input: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Vec<u8>>> + Send>>> {
        deepseek::synthesize(&self.http, &self.tts, input, deepseek::TTS_URL).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TtsProvider;

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

    /// 缺省 provider = openai：旧 toml 不写这个键，走的就是今天那条 `/audio/speech`。
    #[test]
    fn default_provider_is_openai_so_old_toml_is_unchanged() {
        let tts = TtsConfig::new("http://127.0.0.1:8000/v1", "nova");
        assert_eq!(tts.provider, TtsProvider::OpenAi);
        assert_eq!(tts.provider.as_str(), "openai");
        // 旧 toml 里根本没有 provider 键 → 解出来仍是 openai。
        let parsed: crate::settings::TtsSettings =
            serde_json::from_str(r#"{"base_url":"http://127.0.0.1:8091/v1","voice":"ZH"}"#)
                .expect("parse");
        assert_eq!(parsed.provider, TtsProvider::OpenAi);
    }

    /// openai 档的三条既有语义在分档之后**一条都不变**：
    /// 端点还是 `/audio/speech`，字段还是 `input`，空 base_url 仍是「未配置」。
    #[test]
    fn openai_tier_keeps_slash_audio_speech_and_input_field() {
        let tts = TtsConfig::new("http://127.0.0.1:8000/v1", "alloy");
        assert_eq!(tts.provider, TtsProvider::OpenAi);
        assert_eq!(
            crate::join_endpoint(&tts.base_url, "/audio/speech")
                .expect("url")
                .as_str(),
            "http://127.0.0.1:8000/v1/audio/speech"
        );
        let json = serde_json::to_value(SpeechRequestBody::from_config("hi", &tts)).unwrap();
        assert_eq!(json["input"], "hi");
        assert!(json.get("voice_id").is_none(), "openai 档不带 voice_id");
        assert!(tts.is_configured());
        // 空 base_url = 未配置（不出声）——分档没有改这条。
        let blank = TtsConfig::new("", "alloy");
        assert!(!blank.is_configured());
    }

    /// deepseek 档不读 base_url：即使为空，只要钥匙在，这一档就是「已配置」。
    #[test]
    fn deepseek_tier_ignores_base_url_and_needs_a_key() {
        let mut tts = TtsConfig::new("", deepseek::DEFAULT_VOICE);
        tts.provider = TtsProvider::DeepSeek;
        assert_eq!(tts.provider, TtsProvider::DeepSeek);
        assert!(!tts.is_configured(), "没有钥匙 = 这一句不出声");
        tts.api_key = Some(crate::secret::ApiSecret::new("sk-test"));
        assert!(
            tts.is_configured(),
            "deepseek 档不读 base_url，钥匙在即已配置"
        );
    }
}
