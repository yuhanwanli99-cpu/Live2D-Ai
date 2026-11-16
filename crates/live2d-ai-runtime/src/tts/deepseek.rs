//! DeepSeek TTS：`POST https://api.deepseek.com/v1/tools/tts`（第二档，2026-11-16）。
//!
//! # 为什么单独一档，而不是再挂一个 OpenAI 兼容客户端
//!
//! 本地各引擎（MeloTTS / CosyVoice3 …）都能提供 OpenAI 兼容的
//! `POST {base_url}/audio/speech`，那一档继续由 [`super::speech`] 承担、
//! 一行不改。DeepSeek 的**路径与字段都不同**（`/v1/tools/tts`，请求体是
//! `text` / `voice_id` / `format`），所以只能单独一档；但它仍然只是
//! 「一次 HTTP 请求 + 一串 PCM 字节」，**不引厂商 SDK**。
//!
//! # 契约（接口依据：DeepSeek 文档 2026-11-13）
//!
//! - 请求体 `{"text":…,"voice_id":…,"format":"pcm","filter_markdown":false}`；
//! - `filter_markdown` **固定 false**：送进来的句子已经过导演清洗，
//!   再让上游删字符就是第二个人改台词；
//! - 返回 24 000 Hz / 16 位有符号 / 单声道裸 PCM，**忽略** `[tts] sample_rate` /
//!   `channels`（上游规格是固定的，配置里写成 44100 不该把它扳成 44100）；
//! - 音色只允许 [`DEEPSEEK_VOICES`] 里的两个；别的值在设置保存时就拒
//!   （[`super::super::settings::patch`]），运行期再兜一层 [`validate_voice`]；
//! - 单句超过 [`MAX_BILLED_CHARS`] 个计费字符 → 本句空音频 + 一条 `tts` 日志，
//!   **不让整轮失败**；
//! - 不接 WebSocket、不调语速（上游没有这两个能力）。
//!
//! # 整句语义（V2）
//!
//! 上游的响应体**整段收进内存**，HTTP 正常结束之后才交给播放侧。中途断开 /
//! 非 2xx / 超过 [`MAX_RESPONSE_BYTES`] 一律 `Err`，播放侧一个样本都收不到
//! （口型不会跟着半句抖）。这不是「去合成 30 分钟」的缓冲——8 MiB 只是这一句
//! 的保护上限，正常一句远小于它。

use std::pin::Pin;

use futures_util::Stream;
use futures_util::stream;

use crate::config::TtsConfig;
use crate::error::{Error, Result};

/// DeepSeek TTS 端点（固定，不读 `[tts] base_url`）。
pub const TTS_URL: &str = "https://api.deepseek.com/v1/tools/tts";

/// 上游 PCM 规格：24 000 Hz（[crate::audio::AudioSpec::DEFAULT_SAMPLE_RATE]）/
/// 单声道（[crate::audio::AudioSpec::DEFAULT_CHANNELS]）。
///
/// 与 `[tts] sample_rate / channels` **无关**：那一对是 OpenAI 兼容档的口径。
pub const SAMPLE_RATE: u32 = 24_000;
/// 见 [`SAMPLE_RATE`]：上游恒为单声道。
pub const CHANNELS: u16 = 1;

/// 允许的音色（上游只有这两个；别的值在**保存时**就拒）。
pub const DEEPSEEK_VOICES: [&str; 2] = ["stella", "tide"];

/// 出厂音色（设置面板选中 DeepSeek 时的第一项）。
pub const DEFAULT_VOICE: &str = "stella";

/// 单句计费字符上限（上游 10000）：超过就不发请求，本句走空音频。
pub const MAX_BILLED_CHARS: usize = 10_000;

/// 单句响应体保护上限（8 MiB）：超过判整句失败，不交给播放侧。
pub const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

/// 音色是否合法（[`DEEPSEEK_VOICES`] 之一）。
///
/// 保存路径与运行期共用这一个判据——两处各写一份就会漂移。
#[must_use]
pub fn validate_voice(voice: &str) -> bool {
    DEEPSEEK_VOICES.contains(&voice)
}

/// 这一句的**计费字符数**（上游口径：按字符数计，不按字节）。
///
/// 用 `chars().count()` 而不是 `len()`：中文一个字是 3 字节但只算 1 个字符，
/// 按字节判会把 3334 个汉字就当成超限。
#[must_use]
pub fn billed_chars(text: &str) -> usize {
    text.chars().count()
}

/// `/v1/tools/tts` 请求体。字段名是**上游契约**，不要按本项目习惯改名。
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct TtsRequestBody<'a> {
    /// 送合成的句子（导演清洗之后那一份）。
    pub text: &'a str,
    /// 音色（`stella` / `tide`）。
    pub voice_id: &'a str,
    /// 返回格式：本项目只吃裸 PCM。
    pub format: &'static str,
    /// **固定 false**：句子已清洗，不再让上游删字符。
    pub filter_markdown: bool,
}

impl<'a> TtsRequestBody<'a> {
    /// 从配置组装请求体（独立成函数便于单测 wire 形状）。
    ///
    /// `format` / `filter_markdown` 不读配置：它们是这一档的**常量语义**。
    #[must_use]
    pub fn from_config(text: &'a str, tts: &'a TtsConfig) -> Self {
        Self {
            text,
            voice_id: &tts.voice,
            format: "pcm",
            filter_markdown: false,
        }
    }
}

/// 这一句**要不要发请求**（不发 = 走空音频，与「未配置 TTS」同一条收口）。
///
/// 三个理由都是「这一句不该出声」，而不是「链路坏了」：
/// ① 钥匙为空（`[tts] api_key_env` 没配 / 变量为空）——与「未配置 TTS」
///    完全一样，只出文字；
/// ② 文本超过 [`MAX_BILLED_CHARS`]——上游会拒，本句空音频 + 一条日志；
/// ③ 音色不在 [`DEEPSEEK_VOICES`] 里——保存时已经拒过，这里是兜底
///    （配置文件被手改时不该把整轮打成失败）。
#[must_use]
pub fn skip_reason(text: &str, tts: &TtsConfig) -> Option<&'static str> {
    if tts.api_key.is_none() {
        return Some("deepseek_no_key");
    }
    if billed_chars(text) > MAX_BILLED_CHARS {
        return Some("deepseek_text_too_long");
    }
    if !validate_voice(&tts.voice) {
        return Some("deepseek_voice_invalid");
    }
    None
}

/// 上游响应体 → 整句 PCM（**正常结束**才返回；任何失败都是 `Err`）。
///
/// 单独成函数：单测不启网络也能钉住「超限 = 整句丢」这条语义。
pub fn collect_response(status_ok: bool, status: u16, body: &str, bytes: &[u8]) -> Result<Vec<u8>> {
    if !status_ok {
        return Err(Error::status(
            reqwest::StatusCode::from_u16(status).unwrap_or(reqwest::StatusCode::BAD_GATEWAY),
            body,
        ));
    }
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(Error::InvalidAudioConfig {
            message: format!("DeepSeek TTS 响应超过 {MAX_RESPONSE_BYTES} 字节上限（整句丢弃）"),
        });
    }
    Ok(bytes.to_vec())
}

/// `deepseek` 档的完整一次合成（**整句**语义见模块头）。
///
/// `endpoint` 是唯一可注入的一项：生产路径恒为 [`TTS_URL`]，单测把它指到
/// 本机假上游（本 crate 从不要求连真的 `api.deepseek.com`）。
pub(crate) async fn synthesize(
    http: &reqwest::Client,
    tts: &TtsConfig,
    input: &str,
    endpoint: &str,
) -> Result<Pin<Box<dyn Stream<Item = Result<Vec<u8>>> + Send>>> {
    if let Some(reason) = skip_reason(input, tts) {
        tracing::warn!(
            target: "tts",
            code = reason,
            chars = billed_chars(input),
            "DeepSeek TTS：本句不出声（只出文字），轮次继续"
        );
        return Ok(Box::pin(stream::empty()));
    }

    let url = reqwest::Url::parse(endpoint).map_err(|_| Error::InvalidBaseUrl {
        url: endpoint.to_string(),
    })?;
    let body = TtsRequestBody::from_config(input, tts);
    let mut request = http.post(url).json(&body);
    if let Some(key) = &tts.api_key {
        request = request.bearer_auth(key.expose_secret());
    }

    let mut response = request.send().await?;
    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(Error::status(status, &text));
    }

    // 整句缓冲：**中途断开不交出任何字节**——这里 `Err` 返回，已经收到的块
    // 一个都不往下游推（半句 PCM 会让口型跟着抖半句）。
    let mut collected: Vec<u8> = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if collected.len() + chunk.len() > MAX_RESPONSE_BYTES {
                    return Err(Error::InvalidAudioConfig {
                        message: format!(
                            "DeepSeek TTS 响应超过 {MAX_RESPONSE_BYTES} 字节上限（整句丢弃）"
                        ),
                    });
                }
                collected.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Err(e) => return Err(Error::Http(e)),
        }
    }
    Ok(Box::pin(stream::once(async move { Ok(collected) })))
}

/// DeepSeek 档回给播放侧的 PCM 规格：**固定** 24 000 Hz / 单声道。
///
/// 忽略 `[tts] sample_rate / channels`——那一对只对 OpenAI 兼容档有意义。
#[must_use]
pub fn resolved_spec() -> crate::audio::AudioSpec {
    crate::audio::AudioSpec::new(SAMPLE_RATE, CHANNELS).expect("24 kHz / mono 是合法规格")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secret::ApiSecret;

    fn tts_with_key(voice: &str) -> TtsConfig {
        let mut tts = TtsConfig::new("http://127.0.0.1:1/v1", voice);
        tts.api_key = Some(ApiSecret::new("sk-test"));
        tts
    }

    /// 请求体的 wire 形状：**只有** text / voice_id / format / filter_markdown，
    /// 没有 OpenAI 档的 `input` / `voice` / `response_format`。
    #[test]
    fn deepseek_request_body_uses_voice_id_and_fixed_format() {
        let tts = tts_with_key("tide");
        let body = TtsRequestBody::from_config("你好。", &tts);
        let json = serde_json::to_value(&body).expect("wire json");
        assert_eq!(
            json,
            serde_json::json!({
                "text": "你好。",
                "voice_id": "tide",
                "format": "pcm",
                "filter_markdown": false,
            }),
            "字段名是上游契约，不得改名"
        );
        assert!(json.get("input").is_none(), "不得带 OpenAI 档的 input");
        assert!(json.get("voice").is_none(), "不得带 OpenAI 档的 voice");
        assert!(
            json.get("response_format").is_none(),
            "不得带 OpenAI 档的 response_format"
        );
    }

    /// filter_markdown 恒 false：句子已由导演清洗，配置里也没有开关能翻它。
    #[test]
    fn deepseek_never_asks_upstream_to_filter_markdown() {
        for voice in DEEPSEEK_VOICES {
            let tts = tts_with_key(voice);
            let body = TtsRequestBody::from_config("**强调** 一句。", &tts);
            let json = serde_json::to_value(&body).expect("wire json");
            assert_eq!(json["filter_markdown"], false);
            assert_eq!(
                json["text"], "**强调** 一句。",
                "文本必须原样上行（一个字符都不改）"
            );
        }
    }

    /// 上游规格是固定的 24 kHz / 单声道，与 [tts] 里的 44100 无关。
    #[test]
    fn deepseek_forces_24000_hz_mono_regardless_of_configured_spec() {
        let mut tts = tts_with_key(DEFAULT_VOICE);
        tts.spec = crate::audio::AudioSpec::new(44_100, 2).expect("valid spec");
        // 配置里写着 44100/2，DeepSeek 档回给播放侧的规格仍是 24000/1。
        let spec = resolved_spec();
        assert_eq!(spec.sample_rate(), 24_000);
        assert_eq!(spec.channels(), 1);
        assert_ne!(spec, tts.spec, "DeepSeek 档不得沿用 [tts] 的采样率/声道");
    }

    /// 音色白名单：只有 stella / tide。
    #[test]
    fn deepseek_voice_whitelist_is_stella_and_tide() {
        assert!(validate_voice("stella"));
        assert!(validate_voice("tide"));
        for bad in ["", "alloy", "nova", "Stella", " stella"] {
            assert!(!validate_voice(bad), "{bad:?} 不该通过");
        }
    }

    /// 计费字符按**字符**算：中文一个字 1 个，不按 UTF-8 字节（3 字节）。
    #[test]
    fn deepseek_billed_chars_counts_chars_not_bytes() {
        let text = "喵".repeat(9_000);
        assert_eq!(billed_chars(&text), 9_000);
        assert_eq!(text.len(), 27_000, "字节数是三倍——不能按它判");
        assert!(billed_chars(&text) <= MAX_BILLED_CHARS);
        let over = "喵".repeat(MAX_BILLED_CHARS + 1);
        assert!(billed_chars(&over) > MAX_BILLED_CHARS);
    }

    /// 钥匙为空 → 这一句不发请求（走空音频），与「未配置 TTS」同一个收口。
    #[test]
    fn deepseek_without_key_skips_the_request() {
        let tts = TtsConfig::new("http://127.0.0.1:1/v1", DEFAULT_VOICE);
        assert!(tts.api_key.is_none(), "前提：无钥匙");
        assert_eq!(skip_reason("你好。", &tts), Some("deepseek_no_key"));
        // 有钥匙 + 合法文本 + 合法音色 → 不跳过。
        assert_eq!(skip_reason("你好。", &tts_with_key(DEFAULT_VOICE)), None);
    }

    /// 超长句与非法音色都只影响**这一句**（不发请求），不是链路错误。
    #[test]
    fn deepseek_overlong_or_bad_voice_skips_only_this_sentence() {
        let over = "喵".repeat(MAX_BILLED_CHARS + 1);
        assert_eq!(
            skip_reason(&over, &tts_with_key(DEFAULT_VOICE)),
            Some("deepseek_text_too_long")
        );
        assert_eq!(
            skip_reason("你好。", &tts_with_key("alloy")),
            Some("deepseek_voice_invalid")
        );
    }

    /// 非 2xx → 整句失败（不是空音频）。
    #[test]
    fn deepseek_non_2xx_fails_the_whole_sentence() {
        let err = collect_response(false, 503, "{\"error\":\"busy\"}", b"pcm")
            .expect_err("非 2xx 必须失败");
        assert_eq!(err.upstream_status(), Some(503));
    }

    /// 超过 8 MiB → 整句失败，一个样本都不外吐。
    #[test]
    fn deepseek_response_over_cap_discards_the_sentence() {
        let body = vec![0u8; MAX_RESPONSE_BYTES + 1];
        let err = collect_response(true, 200, "", &body).expect_err("超限必须失败");
        assert!(err.to_string().contains("上限"), "错误必须说清超限：{err}");
        // 恰好等于上限仍收（边界是「超过」）。
        let ok = collect_response(true, 200, "", &vec![0u8; MAX_RESPONSE_BYTES])
            .expect("等于上限应通过");
        assert_eq!(ok.len(), MAX_RESPONSE_BYTES);
    }
}
