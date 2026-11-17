//! `[tts]` 段（2026-11-16 从 `settings.rs` 拆出）。
//!
//! 拆出的理由只有一条：`settings.rs` 已贴着 code-stats 的
//! `crates/*/src > 1000 行` 棘轮（上限 0）——加入出声分档
//! （`provider`，2026-11-16）之后它越过了这条线。搬家**一行语义未改**：
//! 类型、缺省值、平台分叉常量与 `apply_local_tts_defaults` 都只是换了住处。
//!
//! 语义归属不变：这是**核心链路**的配置（`tts-is-core.md`），不是 Mod 的。

use serde::{Deserialize, Serialize};

use crate::audio::AudioSpec;
use crate::config::TtsProvider;

/// 语音来源二选一（`[tts].mode`，2026-10-09）。
///
/// **缺省 `Local`**：出声走「本机 TTS 端点」——非 Windows 是仓库自带的
/// MeloTTS 垫片（`127.0.0.1:8091` / 音色 `ZH`）；Windows 包不带 MeloTTS，
/// 出厂端点是随包的 CosyVoice3 sidecar（`127.0.0.1:8011` / 音色 `dafeiyu`，见
/// [`DEFAULT_TTS_BASE_URL`] 的平台分叉）。音色 / 格式 / 采样率 / 声道由设置保存
/// 写死（普通层不给改端口）。
/// `Cloud`：地址 / 音色 / 模型名 / 密钥由用户自己填，本项目**不预填厂商**、
/// **不带 Key**。
///
/// 切换走同一次「保存并应用」；失败就是这一轮没生效（错误码照旧），
/// **不自动改回另一个**。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TtsMode {
    /// 本地（缺省）。
    #[default]
    Local,
    /// 云端。
    Cloud,
}

impl TtsMode {
    /// 稳定值（写进 `[tts].mode`，也是 HTTP patch 的取值）。
    pub const LOCAL: &'static str = "local";
    /// 见 [Self::LOCAL]。
    pub const CLOUD: &'static str = "cloud";

    /// 稳定字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => Self::LOCAL,
            Self::Cloud => Self::CLOUD,
        }
    }
}

/// `[tts]` 段：OpenAI-compatible `/audio/speech` 上游。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TtsSettings {
    /// 语音来源（本地 / 云端）。缺省 `local`——出厂是本机端点（平台分叉：非 Windows
    /// 是仓库自带 MeloTTS 垫片，Windows 是随包 CosyVoice3 sidecar，见 [`DEFAULT_TTS_BASE_URL`]）。
    #[serde(default)]
    pub mode: TtsMode,
    /// 出声分档（2026-11-16）：`openai`（缺省，OpenAI 兼容 `/audio/speech`）
    /// 或 `deepseek`（固定 `/v1/tools/tts`）。
    ///
    /// **缺省 `openai`**，所以旧 toml 不写这个键时仍是今天那条 `/audio/speech`，
    /// 逐块转发、规格看 `sample_rate` / `channels`。
    ///
    /// 与 `[llm] mode` 同一条纪律：缺省值**不落盘**（`skip_serializing_if`），
    /// 于是旧 toml 逐字 round-trip——不会因为读了一次就凭空长出一行
    /// `provider = "openai"`。
    #[serde(default, skip_serializing_if = "TtsProvider::is_openai")]
    pub provider: TtsProvider,
    /// 服务基址。**出厂缺省按平台分叉**（见 [`DEFAULT_TTS_BASE_URL`]）：
    /// 非 Windows = 仓库自带 MeloTTS 垫片（8091）；Windows = 随包 CosyVoice3
    /// sidecar（8011，Windows 包不带 MeloTTS 运行环境）。本地 TTS 不是 Mod，
    /// 出声端点唯一权威仍是这一段。
    #[serde(default = "default_tts_base_url")]
    pub base_url: String,
    /// 可选模型名；省略时请求体不带 `model` 字段。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// 音色（请求体 `voice` 字段）。**出厂缺省按平台分叉**：非 Windows =
    /// `ZH`（`melo/weights/config.json` 里 `spk2id` 的键）；Windows = `dafeiyu`
    /// （CosyVoice3 sidecar 的 voice）。
    #[serde(default = "default_voice")]
    pub voice: String,
    /// 返回格式：`"pcm"`（出厂默认，裸 s16le 流式）或 `"wav"`（RIFF 容器，一次性解析）。
    ///
    /// 缺省随出厂端点（MeloTTS 垫片回 s16le 裸流）取 `"pcm"`。换了返回 WAV 的
    /// 服务端才需要写 `"wav"`——填错要么被 400 拒绝，要么按错容器解出杂音。
    #[serde(default = "default_response_format")]
    pub response_format: String,
    /// 可选：存放 API key 的环境变量名。省略 = 不鉴权。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    /// 返回 PCM 流采样率；**出厂缺省 44.1 kHz**（[`DEFAULT_TTS_SAMPLE_RATE`] =
    /// `melo/weights/config.json` 的 `sampling_rate`）。
    /// 注意：默认值不能靠 `u32::default()`（那是 0），必须显式指定函数。
    /// 它与 [`AudioSpec::DEFAULT_SAMPLE_RATE`]（播放侧 24 kHz）**是两个口径**：
    /// 改这里不动播放默认，改播放默认也不动出厂 TTS 配置。
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    /// 返回 PCM 流声道数；默认单声道。
    #[serde(default = "default_channels")]
    pub channels: u16,
}

/// 出厂 TTS 服务地址：**平台条件**（2026-11-10，0.6.0）。
///
/// - **Windows**：`http://127.0.0.1:8011/v1` = 随包的 CosyVoice3 Windows 原生
///   sidecar（`tools/cosyvoice3/`）。理由：Windows 包**不带** MeloTTS 运行环境，
///   8091 在 Windows 上是死路径；出厂模板（`live2d-ai.toml.example`）与这里必须
///   一致，否则用户在设置里保存「本地」TTS 会把地址打回 8091，开箱即用立刻坏。
/// - **非 Windows**：`http://127.0.0.1:8091/v1` = 仓库自带的 MeloTTS 垫片
///   （`local-tts-melo` 随应用拉起 `melo/start.sh`），语义保持 2026-10-09 口径不变。
///
/// Mod **不写**这一段（`tts-is-core.md`）：它只是与平台缺省一致；用户改了地址或
/// 换了端点，链路照样跟着 `[tts]` 走。
#[cfg(windows)]
pub const DEFAULT_TTS_BASE_URL: &str = "http://127.0.0.1:8011/v1";
#[cfg(not(windows))]
pub const DEFAULT_TTS_BASE_URL: &str = "http://127.0.0.1:8091/v1";

/// 出厂音色：**平台条件**（2026-11-10，0.6.0）。
///
/// - **Windows**：`dafeiyu`（CosyVoice3 sidecar 的 voice，模板同值）；
/// - **非 Windows**：`ZH`（`melo/weights/config.json` 里 `spk2id` 的键）。
#[cfg(windows)]
pub const DEFAULT_TTS_VOICE: &str = "dafeiyu";
#[cfg(not(windows))]
pub const DEFAULT_TTS_VOICE: &str = "ZH";

/// 出厂 TTS 采样率 = `melo/weights/config.json` 的 `sampling_rate`。
///
/// **与播放侧 `AudioSpec::DEFAULT_SAMPLE_RATE`（24 kHz）是两个口径**：那个是
/// 「没有配置时音频调度按多少赫兹理解」，这个是「出厂 `[tts]` 那一行写什么」。
pub const DEFAULT_TTS_SAMPLE_RATE: u32 = 44_100;

fn default_tts_base_url() -> String {
    DEFAULT_TTS_BASE_URL.to_string()
}

fn default_voice() -> String {
    DEFAULT_TTS_VOICE.to_string()
}

fn default_response_format() -> String {
    crate::audio::SUPPORTED_FORMAT.to_string()
}

fn default_sample_rate() -> u32 {
    DEFAULT_TTS_SAMPLE_RATE
}

fn default_channels() -> u16 {
    AudioSpec::DEFAULT_CHANNELS
}

/// 本地模式的**冻结字段**：切到「本地」时由设置保存写进 `[tts]`。
///
/// **地址 / 音色按平台分叉**（2026-11-10，0.6.0；理由同 [`DEFAULT_TTS_BASE_URL`]）：
/// Windows = `127.0.0.1:8011` / `dafeiyu`（CosyVoice3 sidecar；Windows 包不带
/// MeloTTS，8091 是死路径）；非 Windows = `127.0.0.1:8091` / `ZH`（MeloTTS 垫片，
/// 语义不动）。其余冻结：裸 PCM / 44100 / 单声道、**无密钥**、model 清空。
/// 这是核心链路（不是 Mod）——本地 Mod 只负责把进程拉起来（`tts-is-core.md`）。
///
/// **不**碰 [`AudioSpec::DEFAULT_SAMPLE_RATE`]（那是没配音频时的调度常量，
/// 与「出厂 `[tts]` 那一行写什么」是两个口径）。
pub fn apply_local_tts_defaults(tts: &mut TtsSettings) {
    tts.mode = TtsMode::Local;
    // 本地 = 本机 OpenAI 兼容端点（MeloTTS / CosyVoice3），走 `openai` 这一档。
    tts.provider = TtsProvider::OpenAi;
    tts.base_url = DEFAULT_TTS_BASE_URL.to_string();
    tts.model = None;
    tts.voice = DEFAULT_TTS_VOICE.to_string();
    tts.response_format = default_response_format();
    tts.api_key_env = None;
    tts.sample_rate = DEFAULT_TTS_SAMPLE_RATE;
    tts.channels = AudioSpec::DEFAULT_CHANNELS;
}

impl Default for TtsSettings {
    fn default() -> Self {
        Self {
            mode: TtsMode::default(),
            provider: TtsProvider::default(),
            base_url: default_tts_base_url(),
            model: None,
            voice: default_voice(),
            response_format: default_response_format(),
            api_key_env: None,
            sample_rate: default_sample_rate(),
            channels: default_channels(),
        }
    }
}
