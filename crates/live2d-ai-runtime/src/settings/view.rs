//! 脱敏视图 [`SettingsView`]：把 [`AppSettings`](super::AppSettings) 转成可序列化
//! 的、**不含任何密钥明文**的扁平结构供 Web API / egui 设置面板共用。
//!
//! 字段命名与 D1 契约一致：LLM/TTS 的 `api_key_env` 字段在这里换成
//! `has_api_key: bool`（`api_key_env` 字段非空即 true；与「环境变量是否被
//! 实际读到」解耦——后者属于 [`ResolvedSettings`](super::ResolvedSettings) 语义）。

use serde::Serialize;

use super::{AppSettings, LlmSettings, PersonaSettings, TtsSettings};

/// `AppSettings` 的脱敏只读视图，序列化结果可直接交给 Web API。
///
/// 字段名与 `AppSettings` 对齐，**唯一例外**：
/// - `llm.api_key_env` / `tts.api_key_env` 字段被替换成 `has_api_key: bool`，
///   永远不暴露环境变量名（也永远不暴露任何明文密钥——`AppSettings` 本来
///   就只持环境变量**名**，不是密钥本身，但「变量名」也属于敏感元数据）。
/// - 顶层 `dev_mode: bool` 透传（不涉敏感元数据；供设置面板与 PATCH
///   反馈使用，与 `AppStatus.dev_mode` 对齐）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SettingsView {
    /// LLM 段。
    pub llm: LlmView,
    /// TTS 段。
    pub tts: TtsView,
    /// persona 段。
    pub persona: PersonaView,
    /// 开发者模式开关（W7 任务：可配置运行时开关；与 `AppStatus.dev_mode`
    /// 共享唯一来源 `AppSettings.dev_mode`）。
    pub dev_mode: bool,
}

/// 视图中的 LLM 段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LlmView {
    pub base_url: String,
    pub model: String,
    pub has_api_key: bool,
    /// **最终生效**的输出 token 上限（`0` = 不限制）。
    ///
    /// 回的是生效值而不是原始 `Option`：界面要显示「现在到底卡在多少」，
    /// 显示 `null` 对用户没有意义。
    pub max_tokens: u32,
}

/// 视图中的 TTS 段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TtsView {
    pub base_url: String,
    pub model: Option<String>,
    pub voice: String,
    pub has_api_key: bool,
    pub sample_rate: u32,
    pub channels: u16,
}

/// 视图中的 persona 段（rc.4 M5：主链只留 `system_prompt` + `max_history_pairs`）。
///
/// 酒馆卡字段不再出现在主设置里——它们属于标准 Mod `live2d-ai-mod-persona`，
/// 经 `GET /api/v1/mods` 的 settings_spec 暴露。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PersonaView {
    pub system_prompt: String,
    pub max_history_pairs: usize,
}

impl From<&LlmSettings> for LlmView {
    fn from(s: &LlmSettings) -> Self {
        Self {
            base_url: s.base_url.clone(),
            model: s.model.clone(),
            has_api_key: s.api_key_env.as_deref().is_some_and(|n| !n.is_empty()),
            max_tokens: s.effective_max_tokens(),
        }
    }
}

impl From<&TtsSettings> for TtsView {
    fn from(s: &TtsSettings) -> Self {
        Self {
            base_url: s.base_url.clone(),
            model: s.model.clone(),
            voice: s.voice.clone(),
            has_api_key: s.api_key_env.as_deref().is_some_and(|n| !n.is_empty()),
            sample_rate: s.sample_rate,
            channels: s.channels,
        }
    }
}

impl From<&PersonaSettings> for PersonaView {
    fn from(s: &PersonaSettings) -> Self {
        Self {
            system_prompt: s.system_prompt.clone(),
            max_history_pairs: s.max_history_pairs,
        }
    }
}

/// 把 [`AppSettings`] 转成脱敏的 [`SettingsView`]。
///
/// 纯函数：不读文件、不读环境、不做 IO。
pub fn settings_to_view(s: &AppSettings) -> SettingsView {
    SettingsView {
        llm: (&s.llm).into(),
        tts: (&s.tts).into(),
        persona: (&s.persona).into(),
        dev_mode: s.dev_mode,
    }
}
