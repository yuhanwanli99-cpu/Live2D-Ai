//! `live2d-ai-runtime` — Live2D-Ai 的**最小统一 API 层**（RFC D6/D7 落地）。
//!
//! 只做一件事：把「OpenAI-compatible LLM（`/chat/completions`，`stream=true`
//! SSE）」与「OpenAI-compatible TTS（`/audio/speech`，chunked bytes）」统一成
//! 一套最小客户端 API。本地服务（Ollama/vLLM 等）与云端同构，仅 `base_url`
//! 不同；**不内置任何 provider、不做 GUI**。HTTP 层原样转发字节；
//! 音频纯处理收敛在 [`audio`] 模块——本批只保证
//! `response_format = "pcm"`（little-endian signed 16-bit interleaved raw PCM），
//! WAV / MP3 等明确 [`Error::UnsupportedFormat`]，不假装支持。
//!
//! 职责边界：
//! - 2026-09-11 用户裁决：**LLM 不暴露任何工具，只做对话**（「llm 不暴露任何工具
//!   只做对话」）。本 crate 因此不再定义任何 tool schema / 参数 wire 类型，
//!   请求体也不带 `tools`；动作能力 gate 与优先级仲裁仍由 `live2d-ai-core`
//!   持有，但**不再从对话链路驱动**（core 的 action 子系统保留为内部能力）；
//! - 密钥用 [`ApiSecret`] 包装：`Debug`/`Display` 恒定脱敏；密钥**只**进入
//!   `Authorization: Bearer …` 头，不进 URL、不进请求体、不进错误文本；
//! - TLS 固定 rustls（reqwest `default-features = false` + `rustls-tls`），
//!   不启用 default native-tls。
//!
//! # 用法示意
//!
//! ```no_run
//! use futures_util::StreamExt;
//! use live2d_ai_runtime::{
//!     ChatMessage, LlmConfig, LlmEvent, OpenAiClient, PcmS16LeDecoder, RmsMeter, SampleQueue,
//!     TtsConfig,
//! };
//!
//! # async fn demo() -> Result<(), live2d_ai_runtime::Error> {
//! let client = OpenAiClient::new(
//!     LlmConfig::new("http://127.0.0.1:11434/v1", "qwen2.5:7b"),
//!     TtsConfig::new("http://127.0.0.1:8000/v1", "alloy"),
//! )?;
//!
//! // LLM：SSE 增量流（TextDelta / Done）。
//! let mut stream = client
//!     .chat_stream(&[ChatMessage::user("打个招呼")])
//!     .await?;
//! while let Some(event) = stream.next().await {
//!     match event? {
//!         LlmEvent::TextDelta(t) => print!("{t}"),
//!         LlmEvent::Done => break,
//!     }
//! }
//!
//! // 对话流编排（纯逻辑）：把上面的增量流整理成「完整句 / 结束」，
//! // 见 [`dialogue`] 模块与 `DialogueAssembler`。
//!
//! // TTS：chunked bytes 原样转发；默认已请求 response_format=pcm（s16le）。
//! let mut speech = client.synthesize_speech("你好").await?;
//! let mut pcm = PcmS16LeDecoder::new(client.tts().spec);
//! let mut queue = SampleQueue::new(24_000)?; // 1 秒 @24 kHz 的有界缓冲
//! let mut meter = RmsMeter::new(client.tts().spec, 50.0, 0.03, 0.10)?;
//! while let Some(chunk) = speech.next().await {
//!     let samples = pcm.decode(&chunk?); // 任意切割都安全（半个样本留到下批）
//!     if queue.push_slice(&samples) < samples.len() {
//!         // 溢出策略：前缀被接纳，多余被拒绝——由调用方记录/降级，不阻塞。
//!     }
//!     // 播放回调从 queue 取走样本送声卡后，把**实际消费**的样本喂给 meter。
//! }
//! pcm.finish()?; // 流总长为奇数字节 → Error::TruncatedPcm
//! # Ok(())
//! # }
//! ```
//!
//! 依赖保持克制：异步设施（[`tokio`](tokio) 的 sync/rt/time/macros 与
//! `tokio-util` 的 `CancellationToken`）只服务于 [`conversation`] 模块的
//! 最小对话引擎；其余传输仍由 reqwest 内部驱动，[`audio`] 模块为纯 std
//! 实现（无锁/无线程）。

pub mod audio;
pub mod config;
pub mod conversation;
pub mod dialogue;
pub mod error;
pub mod llm;
pub mod secret;
pub mod settings;
pub mod sse;
pub mod tts;

use url::Url;

pub use audio::{AudioSpec, PcmS16LeDecoder, RmsMeter, SampleQueue, convert_spec};
pub use config::{LlmConfig, TtsConfig};
pub use conversation::{
    ConversationConfig, ConversationEngine, EngineEvent, ErrorKind, TurnReport, TurnStatus,
};
pub use dialogue::{DialogueAssembler, DialogueEvent, SentenceAssembler};
pub use error::{Error, Result};
pub use llm::{ChatMessage, LlmEvent, Role};
pub use secret::ApiSecret;
pub use settings::patch::{PatchOutcome, SettingsPatch, apply_patch, plan_atomic_write};
pub use settings::view::SettingsView;
pub use settings::{AppSettings, ResolvedSettings, SettingsError};
pub use sse::{SseDecoder, SseEvent};

/// 统一 OpenAI-compatible 客户端：持有 HTTP 连接池与两份端点配置。
///
/// 构造成本低（内部是 `Arc` 化的连接池），可按需 clone 复用连接。
#[derive(Debug, Clone)]
pub struct OpenAiClient {
    http: reqwest::Client,
    llm: LlmConfig,
    tts: TtsConfig,
}

impl OpenAiClient {
    /// 创建客户端。HTTP 层使用 rustls + 内置 webpki 根证书。
    ///
    /// `pool_max_idle_per_host(0)`：禁用空闲连接复用。集成测试的 mock TCP 服务器
    /// 以 `connection: close` + 半关闭结束每次响应，若 reqwest 复用了这类连接，
    /// 会在下一个 TTS/LLM 请求读到 `hyper::Error(IncompleteMessage)` 而误判为失败
    /// （历史上那条工具调用的集成测试曾因此间歇性红；该测试已于 2026-09-11
    /// 随工具移除一并删除，池化策略本身保持不变）。
    /// 桌面端 LLM 每轮本就单连接、TTS 按句请求，禁用池化带来的握手开销可忽略；
    /// streaming 响应本就不参与池化复用。
    pub fn new(llm: LlmConfig, tts: TtsConfig) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .pool_max_idle_per_host(0)
            .build()?;
        Ok(Self { http, llm, tts })
    }

    /// LLM 端配置（只读视图）。
    pub fn llm(&self) -> &LlmConfig {
        &self.llm
    }

    /// TTS 端配置（只读视图）。
    pub fn tts(&self) -> &TtsConfig {
        &self.tts
    }
}

/// 显式拼接 `base_url + path`。
///
/// 不用 `Url::join`：它对「base 无尾斜杠」做相对段替换
/// （`…/v1` join `/chat/completions` 会丢掉 `v1`），这里改为归一化尾部 `/`
/// 后字符串拼接再解析，语义可预期。
pub(crate) fn join_endpoint(base: &str, path: &str) -> Result<Url> {
    let trimmed = base.trim_end_matches('/');
    let parsed = Url::parse(&format!("{trimmed}{path}")).map_err(|_| Error::InvalidBaseUrl {
        url: base.to_string(),
    })?;
    match parsed.scheme() {
        "http" | "https" => Ok(parsed),
        _ => Err(Error::InvalidBaseUrl {
            url: base.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_join_handles_trailing_slash_and_path_prefix() {
        for (base, expected) in [
            ("http://a:1/v1", "http://a:1/v1/chat/completions"),
            ("http://a:1/v1/", "http://a:1/v1/chat/completions"),
            ("https://a/v1///", "https://a/v1/chat/completions"),
        ] {
            assert_eq!(
                join_endpoint(base, "/chat/completions").unwrap().as_str(),
                expected
            );
        }
        assert_eq!(
            join_endpoint("http://a", "/audio/speech").unwrap().as_str(),
            "http://a/audio/speech"
        );
    }

    #[test]
    fn endpoint_join_rejects_bad_schemes_and_relative_urls() {
        assert!(matches!(
            join_endpoint("ftp://a/v1", "/x"),
            Err(Error::InvalidBaseUrl { .. })
        ));
        assert!(matches!(
            join_endpoint("not a url", "/x"),
            Err(Error::InvalidBaseUrl { .. })
        ));
        assert!(matches!(
            join_endpoint("/only/path", "/x"),
            Err(Error::InvalidBaseUrl { .. })
        ));
    }
}
