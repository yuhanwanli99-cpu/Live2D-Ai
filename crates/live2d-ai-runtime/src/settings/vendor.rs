//! 厂商快照（rc.6）：`[llm.vendors.<id>]` / `[tts.vendors.<id>]` 的类型与 id 规则。
//!
//! 从 `settings.rs` 拆出（2026-10-17），让后者留在 1000 行豁免上限内。语义不变：
//! 快照**不参与** resolve，只有被选中并复制进当前段才生效；密钥只存变量名。

use serde::{Deserialize, Serialize};

use super::{DEFAULT_TTS_SAMPLE_RATE, DEFAULT_TTS_VOICE, LlmSettings, TtsSettings};
use crate::audio::AudioSpec;

/// 内置虚拟厂商 id（rc.6）：MeloTTS 本地垫片的冻结快照。
///
/// 它**不进文件**：选中即 apply_local_tts_defaults() + mode=local；GET 的 vendors
/// 列表里始终有它，方便用户从任意自定义厂商切回出厂本地。
pub const MELO_VENDOR_ID: &str = "melo";

/// 厂商 id 的最大长度（rc.6；与目录 / 标签排版的既有约束一致）。
pub const MAX_VENDOR_ID_LEN: usize = 32;

/// 厂商 id 规则（rc.6）：[a-z0-9][a-z0-9_-]{0,31}。
///
/// 小写、不以分隔符开头、只允许字母数字下划线与连字符；不接受空串。
#[must_use]
pub fn is_valid_vendor_id(id: &str) -> bool {
    if id.is_empty() || id.len() > MAX_VENDOR_ID_LEN {
        return false;
    }
    let mut bytes = id.bytes();
    let first = bytes.next().expect("非空已判");
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return false;
    }
    id.bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// [llm.vendors.<id>] 的一份快照（rc.6）。字段与 [LlmSettings] 的当前值同形，
/// **不含** vendor / vendors（快照不嵌套）。
///
/// 密钥纪律同主段：只存变量**名**，值仍只住 .env；GET 只回 has_api_key。
/// 本表**不参与解析**，所以这里不做 URL / env 名校验——只有「选中」那一刻
/// 才按当前段的同一套规则校验。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct LlmVendor {
    /// 服务基址。
    #[serde(default)]
    pub base_url: String,
    /// 模型名。
    #[serde(default)]
    pub model: String,
    /// 存放密钥的环境变量名。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    /// 输出 token 上限；省略 = 缺省。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// 思考展示总闸；省略 = 缺省。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_reasoning: Option<bool>,
}

impl LlmVendor {
    /// 从当前 [llm] 段抓一份快照（忽略标签与快照表）。
    #[must_use]
    pub fn from_settings(s: &LlmSettings) -> Self {
        Self {
            base_url: s.base_url.clone(),
            model: s.model.clone(),
            api_key_env: s.api_key_env.clone(),
            max_tokens: s.max_tokens,
            show_reasoning: s.show_reasoning,
        }
    }

    /// 把快照复制进当前段（**不动** vendor / vendors）。
    pub fn apply_to(&self, s: &mut LlmSettings) {
        s.base_url.clone_from(&self.base_url);
        s.model.clone_from(&self.model);
        s.api_key_env.clone_from(&self.api_key_env);
        s.max_tokens = self.max_tokens;
        s.show_reasoning = self.show_reasoning;
    }
}

/// [tts.vendors.<id>] 的一份快照（rc.6）。字段与 [TtsSettings] 的当前值同形，
/// **不含** mode / vendor / vendors：
/// - 选 melo → mode=local；
/// - 选其它任何厂商（含 localhost 第二引擎）→ mode=cloud（本版口径）。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct TtsVendor {
    /// 服务基址。
    #[serde(default)]
    pub base_url: String,
    /// 可选模型名。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// 音色。
    #[serde(default = "default_voice")]
    pub voice: String,
    /// 返回格式。
    #[serde(default = "default_response_format")]
    pub response_format: String,
    /// 存放密钥的环境变量名。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    /// 采样率。
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    /// 声道数。
    #[serde(default = "default_channels")]
    pub channels: u16,
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

impl Default for TtsVendor {
    fn default() -> Self {
        let t = TtsSettings::default();
        Self {
            base_url: t.base_url,
            model: t.model,
            voice: t.voice,
            response_format: t.response_format,
            api_key_env: t.api_key_env,
            sample_rate: t.sample_rate,
            channels: t.channels,
        }
    }
}

impl TtsVendor {
    /// 从当前 [tts] 段抓一份快照（忽略 mode / 标签与快照表）。
    #[must_use]
    pub fn from_settings(s: &TtsSettings) -> Self {
        Self {
            base_url: s.base_url.clone(),
            model: s.model.clone(),
            voice: s.voice.clone(),
            response_format: s.response_format.clone(),
            api_key_env: s.api_key_env.clone(),
            sample_rate: s.sample_rate,
            channels: s.channels,
        }
    }

    /// 把快照复制进当前段（**不动** mode / vendor / vendors）。
    pub fn apply_to(&self, s: &mut TtsSettings) {
        s.base_url.clone_from(&self.base_url);
        s.model.clone_from(&self.model);
        s.voice.clone_from(&self.voice);
        s.response_format.clone_from(&self.response_format);
        s.api_key_env.clone_from(&self.api_key_env);
        s.sample_rate = self.sample_rate;
        s.channels = self.channels;
    }
}
