//! 表演层**装配面**（T8，2026-10-07；2026-10-17 rc4 去导演 Mod）。
//!
//! # 两个独立装配点
//!
//! 1. **核心第二路按句清洗**（产品路径，**常驻**）：[`SentenceCleaner`] 恒从
//!    `[llm]` 同一份端点/模型/密钥构造（[`build_sentence_cleaner`]）；动作
//!    能力集与按句规则表来自 [`live2d_ai_runtime::performance::rules`]——**单一
//!    cue 写者**（导演 Mod 已于 rc4 退役，不再并行投递）。表演倾向只读
//!    `mods.json` 的 `mods.director.config.motion_bias`（既有键，缺省平常；
//!    **不读** `enabled` / 导演自己的 staging 端点配置，不新增键）。
//! 2. **整段表演层**（显式 opt-in）：`[performance].enabled`（缺省 false）→
//!    [`PerformanceRuntime`]，每轮一份 JSON 负责切段 / speak。**动作仍恒开**
//!    （2026-10-17 返工裁决）：阶段 1.5 对**每一段原文**用同一张核心本地规则表
//!    产 cue（非空才发 ActionCue，纯函数、不新增 LLM 调用）；表演层自己的
//!    `resolution.cues` **不投递**，避免两份 cue 互相整份覆盖。
//!
//! # 何时读
//!
//! 只在启动装配与热重载（`ControlCommand::Reload`）时读一次：改
//! `motion_bias` / `[performance]` 都要重载才生效。**只读不写**
//! （`mods.json` / `live2d-ai.toml` 一行不改）。
//!
//! # 行数
//!
//! 从 `supervisor.rs` 拆出（那份已 971 行、贴近 `code_stats` 的 >1000 棘轮）：
//! 本文件仍是 ≤500 行的小文件。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use live2d_ai_mod_director::motion_bias::MotionBias;
use live2d_ai_runtime::performance::{PerformanceRuntime, RuleFallback, assemble, rules};
use live2d_ai_runtime::settings::PerformanceSettings;

/// `mods.json` 路径 = `live2d-ai.toml` 同目录（与 `cli_entry::mods_path_for_web` 同口径）。
pub fn mods_manifest_path(config_path: &str) -> PathBuf {
    Path::new(config_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("mods.json")
}

/// 读 `mods.json` 里**仅**的表演倾向（`mods.director.config.motion_bias`）。
///
/// 只读这一个**既有**键：缺文件 / 坏 JSON / 缺段 / 未知值 → 平常
/// （[`MotionBias::default`]）。**不读** `enabled` / 导演自己的 staging 端点
/// 配置——导演 Mod 已退役，它的启用状态不再是任何开关；端点也已删除。
pub fn load_motion_bias(config_path: &str) -> MotionBias {
    let path = mods_manifest_path(config_path);
    let raw = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|manifest| {
            manifest
                .get("mods")
                .and_then(|mods| mods.get("director"))
                .and_then(|entry| entry.get("config"))
                .and_then(|config| config.get("motion_bias"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        });
    let bias = MotionBias::parse(raw.as_deref());
    tracing::debug!(
        bias = bias.as_str(),
        path = %path.display(),
        "表演倾向已读（只读 mods.director.config.motion_bias）"
    );
    bias
}

/// 按 `[performance]` 装配整段表演层运行时（**唯一装配点**；启动与热重载共用）。
///
/// - `[performance].enabled == false`（**缺省**）→ `None`：主链走**常驻的
///   二路按句清洗**（产品路径）；
/// - 显式打开后只读 `[performance]` 自己的 `base_url/model/api_key_env`；
///   端点缺 / URL 非法时仍装配 degraded 运行时（`enabled()==false`）→ 每轮
///   确定性回退，原文一个字不丢；
/// - **能力集 = runtime 规则表的 `accepted_preset_ids()`**、**规则回退 =
///   `rule_cues_for_text`**（单一真源在 runtime，不在这里再抄一份）。
pub fn build_performance_runtime(
    performance: &PerformanceSettings,
) -> Option<Arc<PerformanceRuntime>> {
    // 2026-10-08：整段表演层是显式 opt-in（见文件头注）。缺省关 = 二路按句清洗。
    if !performance.enabled {
        return None;
    }
    // 只读 [performance]：端点两项必须齐（缺 → None，保持主链行为，不装配）。
    if performance.base_url.trim().is_empty() || performance.model.trim().is_empty() {
        return None;
    }
    let allow: Vec<String> = rules::accepted_preset_ids();
    let rule: RuleFallback = Arc::new(rules::rule_cues_for_text);
    let setup = assemble(performance, allow, Some(rule));
    if let Some(note) = setup.note.as_deref() {
        tracing::warn!(note, "表演层接管了但没接上：每轮回退到确定性规则层");
    }
    if let Some(rt) = setup.runtime.as_ref() {
        tracing::info!(
            client = rt.client_kind(),
            mode = rt.mode().as_str(),
            model = rt.model(),
            allow_len = rt.allow().len(),
            "整段表演层已装配（[performance].enabled=true；本轮 0 条按句 action_cue）"
        );
    }
    setup.runtime
}

#[cfg(test)]
#[path = "performance_wiring_tests.rs"]
mod tests;
