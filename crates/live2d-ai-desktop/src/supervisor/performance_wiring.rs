//! 表演层**装配面**（T8，2026-10-07）：导演 Mod 的 `staging_*` → 主链
//! [`PerformanceRuntime`]。
//!
//! # 开关（2026-10-08 改口径：整段表演层是**显式 opt-in**）
//!
//! 唯一开关 = `[performance].enabled`（`live2d-ai.toml`，**缺省 false**）。
//!
//! **为什么改**：产品口径现在是「**二路按句清洗**」——主链边流边切句，一句闭合就
//! 交给二路拿 `{display, speech}` 并立刻送 TTS（`crate::cleaning`）。整段表演层
//! 要等本轮原文收齐才交回一份 JSON，那条路与「不要等整段回复都合成完再开始」
//! 冲突，因此它**只在用户显式打开 `[performance].enabled` 时**才接管文本路径；
//! 它的设置面已从产品界面拿掉（见 `shell/flutter/lib/settings/sections/llm_section.dart`）。
//!
//! 导演 Mod 启用**不再**是开关：启用后按句 cue 由规则层经
//! `ModEventTopic::SentenceReady` 常开兜底（见 supervisor 的
//! `forward_sentence_ready_to_mods`）。`staging_*` 仍是**可选**的端点来源
//! （见下表）。
//!
//! | `staging_base_url` | `staging_model` | 装配 |
//! | --- | --- | --- |
//! | 空 | 空 | 复用对话模型（同一 base_url / 模型名 / 密钥变量名） |
//! | 有 | 有 | 用二路端点；`staging_api_key_env` 空则仍用主 LLM 的密钥变量名 |
//! | 只填一项 | 只填一项 | `None`（保持「边流边切句边送 TTS」） |
//!
//! 判定真源 = `live2d_ai_mod_director::staging::takeover_of`：Mod 的
//! `should_fire_async` 用**同一份规则**，接管时它恒 false——同一轮绝不会有两份 cue。
//!
//! # 何时读
//!
//! 只在启动装配与热重载（`ControlCommand::Reload`）时读一次：改导演启停 / 改
//! `staging_*` 都要重载才生效。**只读不写**（`mods.json` / `live2d-ai.toml` 一行不改）。
//!
//! # 行数
//!
//! 从 `supervisor.rs` 拆出（那份已 971 行、贴近 `code_stats` 的 >1000 棘轮）：
//! 本文件仍是 ≤500 行的小文件。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use live2d_ai_mod_director::staging::{Takeover, takeover_of};
use live2d_ai_runtime::performance::{PerformanceRuntime, RuleFallback, assemble};
use live2d_ai_runtime::settings::{LlmSettings, PerformanceSettings};

/// 导演 Mod 的接管输入（`mods.json` 的 `mods.director` 段）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DirectorTakeover {
    /// `mods.director.enabled`（缺省 false = 不接管）。
    pub enabled: bool,
    /// 二路端点 base_url（空 = 复用对话模型）。
    pub staging_base_url: String,
    /// 二路模型名（空 = 复用对话模型）。
    pub staging_model: String,
    /// 二路密钥的**变量名**（空 = 与主 LLM 同名；**永不读值、永不落日志**）。
    pub staging_api_key_env: String,
}

impl DirectorTakeover {
    /// 从 manifest JSON 读：缺段 / 类型不对一律按「未启用 / 空串」，绝不失败。
    pub fn from_manifest(manifest: &serde_json::Value) -> Self {
        let Some(entry) = manifest
            .get("mods")
            .and_then(|mods| mods.get("director"))
            .and_then(|value| value.as_object())
        else {
            return Self::default();
        };
        let config = entry.get("config");
        let text = |key: &str| {
            config
                .and_then(|value| value.get(key))
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim()
                .to_string()
        };
        Self {
            enabled: entry
                .get("enabled")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            staging_base_url: text("staging_base_url"),
            staging_model: text("staging_model"),
            staging_api_key_env: text("staging_api_key_env"),
        }
    }

    /// 本次加载的接管判定（与 Mod 侧同一份规则）。
    pub fn takeover(&self) -> Takeover {
        takeover_of(&self.staging_base_url, &self.staging_model)
    }
}

/// **动作门**（T10，2026-10-13）：导演启用且 `staging_*` 两项都空 / 都有（接管生效）。
///
/// 只填一项（`Incomplete`）→ 关：保持今天的行为（清洗走主 LLM、不带 cues，
/// Mod 规则层仍按用户句出第一句 cue）。这是本单唯一的**新开关**。
pub fn motion_on(director: &DirectorTakeover) -> bool {
    director.enabled && director.takeover().active()
}

/// 动作门开着时，这一次按句清洗用哪个端点（纯函数；网络由装配方接）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanEndpoint {
    /// 复用对话模型（`ReuseConversation`，或动作门没开）。
    Conversation,
    /// 用导演自己的端点（`Own`）。
    Staging,
}

/// 端点判定：只有「动作门开 **且** 两项都有」才改用 staging；其余都用对话模型。
pub fn clean_endpoint(director: &DirectorTakeover) -> CleanEndpoint {
    if motion_on(director) && director.takeover() == Takeover::Own {
        CleanEndpoint::Staging
    } else {
        CleanEndpoint::Conversation
    }
}

/// `SentenceReady` 要不要转给 director Mod（T16，2026-10-13）。
///
/// 表演层开着、**或**动作门开着时不转——否则同一轮会有两份 cue 互相整份覆盖。
/// 纯函数（唯一判据），`turn.rs` 用它算那个布尔。
pub fn should_forward_sentence_ready(performance_enabled: bool, motion_enabled: bool) -> bool {
    !(performance_enabled || motion_enabled)
}

/// `mods.json` 路径 = `live2d-ai.toml` 同目录（与 `cli_entry::mods_path_for_web` 同口径）。
pub fn mods_manifest_path(config_path: &str) -> PathBuf {
    Path::new(config_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("mods.json")
}

/// 读 `live2d-ai.toml` 同目录的 `mods.json`。
///
/// 文件缺失 / 坏 JSON / 没有 director 段 → [`DirectorTakeover::default`]（= 未启用）——
/// 与 `cli_entry::mods_manifest_for_web` 在缺文件时回落的内建缺省（其中没有 director）
/// 同一条口径。
pub fn load_director_takeover(config_path: &str) -> DirectorTakeover {
    let path = mods_manifest_path(config_path);
    match std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
    {
        Some(manifest) => DirectorTakeover::from_manifest(&manifest),
        None => DirectorTakeover::default(),
    }
}

/// 按上表装配表演层运行时（**唯一装配点**；启动与热重载共用）。
///
/// - `[performance].enabled == false`（**缺省**）→ `None`：主链走**边流边切句 +
///   二路按句清洗**（产品路径）；
/// - 显式打开后：导演启用则按 `staging_*` 两项判定端点来源，导演未启用则用
///   `[performance].base_url/model` 自己的端点；只填一项 `staging_*` → `None`；
/// - 端点缺 / URL 非法时仍装配 degraded 运行时（`enabled()==false`）→
///   每轮确定性回退，原文一个字不丢；
/// - **能力集 = director 的 `accepted_preset_ids()`**（= 主 allowlist）、**规则回退 =
///   director 的 `rule_cues_for_text`**（同一张映射表的单一真源，不在这里再抄一份）。
pub fn build_performance_runtime(
    llm: &LlmSettings,
    performance: &PerformanceSettings,
    director: &DirectorTakeover,
) -> Option<Arc<PerformanceRuntime>> {
    // 2026-10-08：整段表演层是显式 opt-in（见文件头注）。缺省关 = 二路按句清洗。
    if !performance.enabled {
        return None;
    }
    let (base_url, model, api_key_env) = if director.enabled {
        match director.takeover() {
            // 两项都空：复用对话模型（同一个 base_url、模型名、密钥变量名）。
            Takeover::ReuseConversation => (
                llm.base_url.clone(),
                llm.model.clone(),
                llm.api_key_env.clone(),
            ),
            // 两项都有：用导演自己的端点；密钥变量名留空则仍用主 LLM 的。
            Takeover::Own => (
                director.staging_base_url.clone(),
                director.staging_model.clone(),
                if director.staging_api_key_env.is_empty() {
                    llm.api_key_env.clone()
                } else {
                    Some(director.staging_api_key_env.clone())
                },
            ),
            // 只填一项：不接管，保持直送。
            Takeover::Incomplete => return None,
        }
    } else {
        // 导演没启用：用 `[performance]` 自己的端点（老口径）。
        if performance.base_url.trim().is_empty() || performance.model.trim().is_empty() {
            return None;
        }
        (
            performance.base_url.clone(),
            performance.model.clone(),
            performance.api_key_env.clone(),
        )
    };
    let effective = PerformanceSettings {
        enabled: true,
        base_url,
        model,
        api_key_env,
        ..performance.clone()
    };
    let allow: Vec<String> = live2d_ai_mod_director::presets::accepted_preset_ids();
    let rule: RuleFallback =
        Arc::new(|text: &str| live2d_ai_mod_director::presets::rule_cues_for_text(text));
    let setup = assemble(&effective, allow, Some(rule));
    if let Some(note) = setup.note.as_deref() {
        tracing::warn!(note, "表演层接管了但没接上：每轮回退到确定性规则层");
    }
    if let Some(rt) = setup.runtime.as_ref() {
        tracing::info!(
            client = rt.client_kind(),
            mode = rt.mode().as_str(),
            model = rt.model(),
            source = director.takeover().as_str(),
            allow_len = rt.allow().len(),
            "表演层已装配（导演接管：原文分段送 TTS + 头/表情 cue）"
        );
    }
    setup.runtime
}

#[cfg(test)]
#[path = "performance_wiring_tests.rs"]
mod tests;
