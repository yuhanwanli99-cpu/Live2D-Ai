//! `memory` Mod 的配置面：namespaced 配置解析 + 静态 `settings_spec`。
//!
//! Wave 3 C 轨从 `lib.rs` 拆出（原文件已 487 行，再加计数/淘汰会越过
//! 「源码 ≤500 行」的口径）。本文件是**纯配置**：不碰 IO、不持运行时状态。
//!
//! `max_records` 在 Wave 3 的语义**变了**：它不只是检索窗口，而是**条数上限
//! （count cap）**——写入超过上限时物理淘汰最旧的记录（见
//! `docs/architecture/memory-mod-v0.md` §6.1 的取舍）。

use std::path::PathBuf;

use live2d_ai_mod_system::{ModSettingField, ModSettingsSpec};

use crate::DESCRIPTOR;
use crate::strategy;
use crate::strategy::{
    DEFAULT_MAX_RECORDS, DEFAULT_STORE_FILE, DEFAULT_TOP_K, MAX_MAX_RECORDS, MAX_TOP_K,
    MIN_MAX_RECORDS, MIN_TOP_K,
};

/// 本 Mod 的 namespaced 配置（缺省全部安全；**没有** `enabled` 键）。
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryConfig {
    /// JSONL 路径；空 = 配置文件同目录的 `memory.jsonl`。
    pub store_path: String,
    /// 每轮注入几条（钳在 `MIN_TOP_K`..=`MAX_TOP_K`）。
    pub top_k: usize,
    /// **条数上限（count cap）**，同时是检索窗口：文件里最多保留这么多条，
    /// 写入超过时**物理淘汰**最旧的（Wave 3 起）。钳在
    /// `MIN_MAX_RECORDS`..=`MAX_MAX_RECORDS`。
    pub max_records: usize,
    /// 是否注入（false = 只记不注入；缺省 true）。
    pub enabled_injection: bool,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            store_path: String::new(),
            top_k: DEFAULT_TOP_K,
            max_records: DEFAULT_MAX_RECORDS,
            enabled_injection: true,
        }
    }
}

impl MemoryConfig {
    /// 从 namespaced JSON 读配置：**越界只钳、类型不对只用缺省**，绝不失败。
    pub fn from_value(value: &serde_json::Value) -> Self {
        let store_path = value
            .get("store_path")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let number = |key: &str, default: usize, lo: usize, hi: usize| {
            value
                .get(key)
                .and_then(serde_json::Value::as_f64)
                .filter(|f| f.is_finite())
                .map(|f| (f.round() as i64).clamp(lo as i64, hi as i64) as usize)
                .unwrap_or(default)
        };
        Self {
            store_path,
            top_k: number("top_k", DEFAULT_TOP_K, MIN_TOP_K, MAX_TOP_K),
            max_records: number(
                "max_records",
                DEFAULT_MAX_RECORDS,
                MIN_MAX_RECORDS,
                MAX_MAX_RECORDS,
            ),
            enabled_injection: value
                .get("enabled_injection")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true),
        }
    }

    /// 解析实际 JSONL 路径（`strategy::resolve_store_path` 的配置包装）。
    pub fn resolve_store_path(&self, config_path: &str) -> Option<PathBuf> {
        strategy::resolve_store_path(config_path, &self.store_path)
    }
}

/// 本 Mod 的设置 schema（**静态**：未启用也拿得到，前端可先填再启用）。
///
/// 四个字段，`enabled_injection` 是「注入开关」——**不是**「Mod 启停开关」。
pub fn memory_settings_spec() -> ModSettingsSpec {
    ModSettingsSpec {
        mod_id: DESCRIPTOR.id.to_string(),
        title: DESCRIPTOR.name.to_string(),
        version: 1,
        fields: vec![
            ModSettingField::String {
                key: "store_path".to_string(),
                label: format!("记忆库路径（留空 = 配置文件同目录的 {DEFAULT_STORE_FILE}）"),
                secret: false,
            },
            ModSettingField::Number {
                key: "top_k".to_string(),
                label: format!("每轮注入条数（{MIN_TOP_K}–{MAX_TOP_K}，缺省 {DEFAULT_TOP_K}）"),
                min: MIN_TOP_K as f64,
                max: MAX_TOP_K as f64,
            },
            ModSettingField::Number {
                key: "max_records".to_string(),
                label: format!(
                    "条数上限（{MIN_MAX_RECORDS}–{MAX_MAX_RECORDS}，缺省 {DEFAULT_MAX_RECORDS}；超出物理淘汰最旧）"
                ),
                min: MIN_MAX_RECORDS as f64,
                max: MAX_MAX_RECORDS as f64,
            },
            ModSettingField::Bool {
                key: "enabled_injection".to_string(),
                label: "把检索结果注入下一轮提示词（关掉则只记不注入）".to_string(),
                default: true,
            },
        ],
    }
}
