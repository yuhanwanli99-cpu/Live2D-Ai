//! 客户端配置：[`LlmConfig`]（`/chat/completions`）与 [`TtsConfig`]（`/audio/speech`）。
//!
//! 两者都是纯数据结构：本地（Ollama/vLLM 等）与云端服务同构，仅 `base_url` 不同。
//! 可选 API key 用 [`ApiSecret`] 包装，Debug 打印自动脱敏。

use crate::audio::AudioSpec;
use crate::secret::ApiSecret;

/// LLM 配置（OpenAI-compatible `/chat/completions`，固定 `stream=true`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmConfig {
    /// 服务基址，如 `http://127.0.0.1:11434/v1` 或 `https://api.openai.com/v1`。
    /// 尾部 `/` 可省略；请求路径由本 crate 拼接为 `<base_url>/chat/completions`。
    pub base_url: String,
    /// 模型名（如 `qwen2.5:7b` / `gpt-4o-mini`），原样进入请求体 `model` 字段。
    pub model: String,
    /// 可选 key。`Some` 时只写入 `Authorization: Bearer <key>` 头；本地无鉴权服务留 `None`。
    pub api_key: Option<ApiSecret>,
    /// 输出 token 上限（**0 = 不限制**，请求体里省略该字段）。
    ///
    /// 已由 [`crate::settings::LlmSettings::effective_max_tokens`] 解析过默认值，
    /// 所以这里拿到的是**最终生效值**。
    pub max_tokens: u32,
}

impl LlmConfig {
    /// 无鉴权的本地服务配置。
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            model: model.into(),
            api_key: None,
            // 测试/便捷构造默认不限制；生产路径由 settings 解析注入。
            max_tokens: 0,
        }
    }

    /// 附带 key 的配置（key 仅用于 Authorization 头）。
    pub fn with_api_key(mut self, key: impl Into<ApiSecret>) -> Self {
        self.api_key = Some(key.into());
        self
    }
}

/// TTS 配置（OpenAI-compatible `/audio/speech`）。
///
/// 传输层原样转发 chunked bytes；`response_format = "pcm"`（本 crate 的默认值）
/// 的字节流契约见 [`crate::audio`]：little-endian signed 16-bit interleaved
/// raw PCM，解释参数由 [`spec`](TtsConfig::spec) 携带。WAV / MP3 等其他格式
/// 本批不保证可解码（见 [`crate::audio::ensure_supported_format`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtsConfig {
    /// 服务基址；请求路径拼接为 `<base_url>/audio/speech`。
    pub base_url: String,
    /// 可选模型名（如 `tts-1`）；`None` 时请求体省略 `model` 字段，由服务端默认决定。
    pub model: Option<String>,
    /// 可选 key。语义同 [`LlmConfig::api_key`]。
    pub api_key: Option<ApiSecret>,
    /// 音色（如 `alloy`），进入请求体 `voice` 字段。
    pub voice: String,
    /// 音频格式。默认 `Some("pcm")`——本批唯一保证可解码的格式；
    /// 显式设为 `None` 表示交由服务端默认（此时本 crate 不承诺解码能力）。
    pub response_format: Option<String>,
    /// 返回的 raw PCM 流规格：默认 24 000 Hz / 单声道。
    /// 仅在 `response_format == Some("pcm")` 时有意义。
    pub spec: AudioSpec,
}

impl TtsConfig {
    /// 无鉴权配置：请求 `pcm` 格式，按 24 kHz 单声道解释。
    pub fn new(base_url: impl Into<String>, voice: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            model: None,
            api_key: None,
            voice: voice.into(),
            response_format: Some(crate::audio::SUPPORTED_FORMAT.to_string()),
            spec: AudioSpec::default(),
        }
    }

    /// 是否已配置 TTS 端点。
    ///
    /// 2026-09-10：TTS 选型未定期间 `base_url` 允许为空 —— 表示「本轮不接 TTS」，
    /// 引擎走空句路径（只出文本、不合成音频），不报错。
    pub fn is_configured(&self) -> bool {
        !self.base_url.trim().is_empty()
    }

    /// 覆盖 PCM 流规格（采样率/声道数）。规格合法性在 [`AudioSpec::new`]
    /// 构造时已校验（非零），此处无需再验。
    pub fn with_spec(mut self, spec: AudioSpec) -> Self {
        self.spec = spec;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builders_fill_expected_fields() {
        let llm = LlmConfig::new("http://127.0.0.1:11434/v1", "qwen2.5:7b").with_api_key("sk-test");
        assert_eq!(llm.base_url, "http://127.0.0.1:11434/v1");
        assert_eq!(llm.model, "qwen2.5:7b");
        assert_eq!(
            llm.api_key.as_ref().map(ApiSecret::expose_secret),
            Some("sk-test")
        );

        let tts = TtsConfig::new("http://127.0.0.1:8000/v1", "alloy");
        assert_eq!(tts.voice, "alloy");
        // 默认契约：请求 pcm，按 24 kHz 单声道解释；model/key 不发。
        assert_eq!(tts.response_format.as_deref(), Some("pcm"));
        assert_eq!(tts.spec, AudioSpec::default());
        assert!(tts.model.is_none() && tts.api_key.is_none());

        // with_spec 覆盖规格（非零校验在 AudioSpec::new 完成）。
        let wide = tts
            .clone()
            .with_spec(AudioSpec::new(48_000, 2).expect("valid"));
        assert_eq!(wide.spec.sample_rate(), 48_000);
        assert_eq!(wide.spec.channels(), 2);
        assert_ne!(wide, tts);
    }

    #[test]
    fn debug_redacts_keys_in_both_configs() {
        const SECRET: &str = "sk-very-secret-value";
        let llm = LlmConfig::new("http://a", "m").with_api_key(SECRET);
        let tts = TtsConfig {
            api_key: Some(SECRET.into()),
            ..TtsConfig::new("http://b", "v")
        };
        let dbg = format!("{llm:?} {tts:?}");
        assert!(!dbg.contains(SECRET));
        assert!(dbg.contains("ApiSecret(REDACTED)"));
    }
}
