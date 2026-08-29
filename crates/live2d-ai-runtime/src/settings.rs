//! 应用级配置加载 [`AppSettings`]：把 `live2d-ai.toml`（+ 环境变量密钥）解析成可直接使用的 [`LlmConfig`] / [`TtsConfig`] / [`ConversationConfig`]。
//!
//! # 配置来源与边界
//!
//! - **文件**只放非敏感项：`base_url`、模型名、音色、persona 提示词；
//! - **密钥不进文件**：写环境变量**名**（`api_key_env`），真实 key 在运行环境里；
//!   解析经 `resolve`/`resolve_with` 读出并包成 [`ApiSecret`]；变量未设置或为空串一律视为「无鉴权」。
//! - 未知字段**拒绝解析**（`deny_unknown_fields`）：拼写错误在启动时暴露。
//!
//! # 文件示例
//!
//! ```toml
//! [llm]
//! base_url = "http://127.0.0.1:11434/v1"
//! model = "qwen2.5:7b"
//!
//! [tts]
//! base_url = "http://127.0.0.1:8000/v1"
//! voice = "alloy"
//!
//! [persona]
//! system_prompt = "你是桌宠，用简短口语回答。"
//! ```
//! 全部字段与默认值见 [`AppSettings::example_toml`]。视图/补丁/原子写回规划见子模块 [`view`] / [`patch`]（`patch_tests` 仅测试构建，避免 patch.rs 撞 500 行硬上限）。
pub mod patch;
#[cfg(test)]
mod patch_tests;
#[cfg(test)]
mod settings_tests;
pub mod view;
use std::path::Path;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::audio::AudioSpec;
use crate::config::{LlmConfig, TtsConfig};
use crate::conversation::ConversationConfig;
use crate::secret::ApiSecret;

/// 配置解析错误：文件读取、TOML 语法/字段、URL 与环境变量名校验。
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    /// 读文件失败（不存在 / 无权限）。
    #[error("读取配置文件失败 ({path}): {source}")]
    Io {
        /// 配置文件路径。
        path: String,
        /// 底层 IO 错误。
        #[source]
        source: std::io::Error,
    },

    /// TOML 解析失败（语法错误、缺必填字段、未知字段）。
    #[error("配置文件解析失败: {0}")]
    Parse(#[from] toml::de::Error),

    /// `base_url` 非法：无法解析为绝对 URL 或 scheme 不是 http(s)。
    #[error("{section} base_url 非法: {url:?}（需要绝对 http/https URL）")]
    InvalidBaseUrl {
        /// 所属配置段名（`llm` / `tts`），便于定位。
        section: &'static str,
        /// 原始字符串。
        url: String,
    },

    /// `api_key_env` 不是合法的环境变量名。
    #[error(
        "{section} api_key_env 非法: {name:?}（仅允许 ASCII 字母/数字/下划线，且不以数字开头）"
    )]
    InvalidEnvName {
        /// 所属配置段名。
        section: &'static str,
        /// 原始字符串。
        name: String,
    },

    /// `[tts]` 的 PCM 规格非法（采样率/声道数为 0）。
    #[error("tts 音频规格非法: {0}")]
    InvalidAudioSpec(#[from] crate::error::Error),
}

/// `live2d-ai.toml` 的反序列化形态（字段全部有默认值的最小文件即可工作）。
///
/// 构造 [`Self`] 只做「形状」解析；URL / 环境变量名校验与密钥读取发生在
/// [`AppSettings::resolve`]，让「语法错误」和「运行环境问题」分开报告。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
// 容器级 default：任一段整体缺省时回退到该段类型的 `Default`（deny_unknown_fields
// 与它独立——只拦「写了但拼错」的键，不拦「没写」的段）。
#[serde(default, deny_unknown_fields)]
pub struct AppSettings {
    /// LLM 段（`[llm]`）。
    pub llm: LlmSettings,
    /// TTS 段（`[tts]`）。
    pub tts: TtsSettings,
    /// persona 段（`[persona]`），可整体省略。
    pub persona: PersonaSettings,
    /// 开发者模式开关（默认 false）：打开后 `/api/v1/logs*` 端点放行
    /// （见 W3 任务约定）。可在 `live2d-ai.toml` 顶层显式写 `dev_mode = true`，
    /// 或由 CLI `--dev-mode` 覆盖，或在设置面板（egui）勾选后 PATCH 落盘。
    /// 来源优先级：CLI flag > settings 文件 > 默认 false。
    pub dev_mode: bool,
}

/// `[llm]` 段：OpenAI-compatible `/chat/completions` 上游。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LlmSettings {
    /// 服务基址（如 `http://127.0.0.1:11434/v1`）。
    #[serde(default)]
    pub base_url: String,
    /// 模型名，进入请求体 `model` 字段。
    #[serde(default)]
    pub model: String,
    /// 可选：存放 API key 的环境变量名。省略 = 不鉴权。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
}

/// `[tts]` 段：OpenAI-compatible `/audio/speech` 上游。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TtsSettings {
    /// 服务基址。
    #[serde(default)]
    pub base_url: String,
    /// 可选模型名；省略时请求体不带 `model` 字段。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// 音色（请求体 `voice` 字段）。
    #[serde(default = "default_voice")]
    pub voice: String,
    /// 可选：存放 API key 的环境变量名。省略 = 不鉴权。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    /// 返回 PCM 流采样率；默认 24 kHz（[`AudioSpec::DEFAULT_SAMPLE_RATE`]）。
    /// 注意：默认值不能靠 `u32::default()`（那是 0），必须显式指定函数。
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    /// 返回 PCM 流声道数；默认单声道。
    #[serde(default = "default_channels")]
    pub channels: u16,
}

fn default_voice() -> String {
    "alloy".to_string()
}

fn default_sample_rate() -> u32 {
    AudioSpec::DEFAULT_SAMPLE_RATE
}

fn default_channels() -> u16 {
    AudioSpec::DEFAULT_CHANNELS
}

impl Default for TtsSettings {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            model: None,
            voice: default_voice(),
            api_key_env: None,
            sample_rate: AudioSpec::DEFAULT_SAMPLE_RATE,
            channels: AudioSpec::DEFAULT_CHANNELS,
        }
    }
}

/// `[persona]` 段：对话人设与历史策略。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PersonaSettings {
    /// 固定注入每轮的 system 提示；空串 = 不发 system 消息。
    #[serde(default)]
    pub system_prompt: String,
    /// 保留的历史轮数上限；`0` = 每轮独立（默认，最小闭环先不做记忆）。
    #[serde(default)]
    pub max_history_pairs: usize,
}

/// 解析产物：三份可直接交给引擎/客户端使用的配置视图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSettings {
    /// LLM 客户端配置（key 已从环境读出并包装）。
    pub llm: LlmConfig,
    /// TTS 客户端配置。
    pub tts: TtsConfig,
    /// 对话引擎配置（system 提示 / 历史 / 队列等沿用 crate 默认）。
    pub conversation: ConversationConfig,
}

impl AppSettings {
    /// 解析 TOML 文本。
    pub fn from_toml_str(text: &str) -> Result<Self, SettingsError> {
        Ok(toml::from_str(text)?)
    }

    /// 从磁盘读取并解析 `live2d-ai.toml`。
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, SettingsError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|source| SettingsError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_toml_str(&text)
    }

    /// 用**当前进程环境**解析成 [`ResolvedSettings`]。
    ///
    /// 环境变量未设置或为空串都视为「无鉴权」（CI 里常导出空变量，不应报错）。
    pub fn resolve(&self) -> Result<ResolvedSettings, SettingsError> {
        self.resolve_with(|name| std::env::var(name).ok().filter(|v| !v.is_empty()))
    }

    /// 注入式解析：`lookup` 返回 `None` 表示该环境变量不存在。
    ///
    /// 生产路径 [`AppSettings::resolve`] 即 `lookup = |n| env::var(n).ok()`；
    /// 测试用它避免污染进程环境。
    pub fn resolve_with(
        &self,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> Result<ResolvedSettings, SettingsError> {
        let llm_key = match &self.llm.api_key_env {
            Some(name) => {
                validate_env_name("llm", name)?;
                lookup(name).map(ApiSecret::new)
            }
            None => None,
        };
        let tts_key = match &self.tts.api_key_env {
            Some(name) => {
                validate_env_name("tts", name)?;
                lookup(name).map(ApiSecret::new)
            }
            None => None,
        };

        validate_base_url("llm", &self.llm.base_url)?;
        validate_base_url("tts", &self.tts.base_url)?;

        // sample_rate/channels 为 0 时经 AudioSpec::new 显式报配置错误
        //（serde 默认值非零，只有显式写 0 才会走到这里）。
        let spec = AudioSpec::new(self.tts.sample_rate, self.tts.channels)?;

        Ok(ResolvedSettings {
            llm: LlmConfig {
                base_url: self.llm.base_url.clone(),
                model: self.llm.model.clone(),
                api_key: llm_key,
            },
            tts: TtsConfig {
                base_url: self.tts.base_url.clone(),
                model: self.tts.model.clone(),
                api_key: tts_key,
                voice: self.tts.voice.clone(),
                response_format: Some(crate::audio::SUPPORTED_FORMAT.to_string()),
                spec,
            },
            conversation: ConversationConfig {
                system_prompt: self.persona.system_prompt.clone(),
                max_history_pairs: self.persona.max_history_pairs,
                ..ConversationConfig::default()
            },
        })
    }

    /// 内置配置模板（`live2d-ai.toml.example` 的内容源），供 CLI 输出与文档引用。
    pub fn example_toml() -> &'static str {
        include_str!("../../../live2d-ai.toml.example")
    }

    /// 序列化为 TOML 文本（与 [`Self::from_toml_str`] 对称；供设置面板写回）。
    ///
    /// **密钥安全语义**：`api_key_env` 字段本身**只持有环境变量名**（与解析
    /// 对称），真实密钥永不出现在此序列化结果中；写回 `live2d-ai.toml` 后
    /// 下次启动仍由 `resolve_with(|n| env::var(n).ok())` 从环境读出。Serialize
    /// 走纯内存、无 IO / 无失败路径——返回 `String` 而非 `Result`。
    pub fn to_toml_string(&self) -> String {
        toml::to_string(self).expect("AppSettings 序列化 infallible（仅内存操作）")
    }
}
/// 校验环境变量名：非空、ASCII、`[A-Za-z_][A-Za-z0-9_]*`。
fn validate_env_name(section: &'static str, name: &str) -> Result<(), SettingsError> {
    let valid = !name.is_empty()
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && !name.as_bytes()[0].is_ascii_digit();
    if valid {
        Ok(())
    } else {
        Err(SettingsError::InvalidEnvName {
            section,
            name: name.to_string(),
        })
    }
}

/// 校验 `base_url` 是可解析的绝对 http(s) URL。
fn validate_base_url(section: &'static str, raw: &str) -> Result<(), SettingsError> {
    match Url::parse(raw) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => Ok(()),
        _ => Err(SettingsError::InvalidBaseUrl {
            section,
            url: raw.to_string(),
        }),
    }
}
