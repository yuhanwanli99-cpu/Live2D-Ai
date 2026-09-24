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
    DEFAULT_INJECTION_BUDGET_CHARS, DEFAULT_MAX_RECORDS, DEFAULT_STORE_FILE, DEFAULT_SUMMARY_RATIO,
    DEFAULT_TOP_K, DEFAULT_TRIM_RATIO, MAX_INJECTION_BUDGET_CHARS, MAX_MAX_RECORDS, MAX_TOP_K,
    MIN_INJECTION_BUDGET_CHARS, MIN_MAX_RECORDS, MIN_TOP_K, SUMMARY_KEEP_RECENT_TURNS,
};
use crate::summary::{
    MAX_TIMEOUT_MS as SUMMARY_MAX_TIMEOUT_MS, MIN_TIMEOUT_MS as SUMMARY_MIN_TIMEOUT_MS,
    SummaryConfig,
};

/// 摘要冷却缺省轮数（触发一次后这么多轮内不再触发）。
pub const DEFAULT_SUMMARY_COOLDOWN_TURNS: u64 = 8;

/// 摘要单次请求超时缺省（毫秒）。比导演二路宽：摘要是后台调用，主链不等它。
pub const DEFAULT_SUMMARY_TIMEOUT_MS: u64 = 8_000;

/// 「保留最近几轮原文」的下限。
pub const MIN_SUMMARY_KEEP_RECENT: usize = 0;
/// 「保留最近几轮原文」的上限。
pub const MAX_SUMMARY_KEEP_RECENT: usize = 50;

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
    /// **注入预算（字符数）**：注入块字符上限，超限按档裁剪（P1-5）。
    pub injection_budget_chars: usize,
    /// 注入块占用预算 > 该比例 → 按分数裁 top-k（缺省 0.60）。
    pub trim_ratio: f64,
    /// **桶内原文总字符** > 注入预算 × 该比例 → 触发真摘要（缺省 0.75）。
    pub summary_ratio: f64,
    /// 摘要冷却（轮）：触发一次真摘要后这么多轮内不再触发。
    pub summary_cooldown_turns: u64,
    /// 摘要保留的最近轮数（这几轮**保持原文**，不压进摘要）。
    pub summary_keep_recent_turns: usize,
    // 真摘要（P1-5；独立于主链 [llm]，见 settings_spec 的口径说明）：
    /// 总闸：false 时不摘要（旁车也不写）。缺省 false（显式开启）。
    pub summary_enabled: bool,
    /// OpenAI 兼容 base URL（空 = 不摘要）。
    pub summary_base_url: String,
    /// 模型名（空 = 不摘要）。
    pub summary_model: String,
    /// 存 API key 的环境变量名（空 = 不鉴权；值走 .env / 进程环境）。
    pub summary_api_key_env: String,
    /// 单次请求超时（毫秒）。
    pub summary_timeout_ms: u64,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            store_path: String::new(),
            top_k: DEFAULT_TOP_K,
            max_records: DEFAULT_MAX_RECORDS,
            enabled_injection: true,
            injection_budget_chars: DEFAULT_INJECTION_BUDGET_CHARS,
            trim_ratio: DEFAULT_TRIM_RATIO,
            summary_ratio: DEFAULT_SUMMARY_RATIO,
            summary_cooldown_turns: DEFAULT_SUMMARY_COOLDOWN_TURNS,
            summary_keep_recent_turns: SUMMARY_KEEP_RECENT_TURNS,
            summary_enabled: false,
            summary_base_url: String::new(),
            summary_model: String::new(),
            summary_api_key_env: String::new(),
            summary_timeout_ms: DEFAULT_SUMMARY_TIMEOUT_MS,
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
        let float = |key: &str, default: f64, lo: f64, hi: f64| {
            value
                .get(key)
                .and_then(serde_json::Value::as_f64)
                .filter(|f| f.is_finite())
                .map(|f| f.clamp(lo, hi))
                .unwrap_or(default)
        };
        let text = |key: &str| {
            value
                .get(key)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string()
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
            injection_budget_chars: number(
                "injection_budget_chars",
                DEFAULT_INJECTION_BUDGET_CHARS,
                MIN_INJECTION_BUDGET_CHARS,
                MAX_INJECTION_BUDGET_CHARS,
            ),
            trim_ratio: float("trim_ratio", DEFAULT_TRIM_RATIO, 0.1, 1.0),
            summary_ratio: float("summary_ratio", DEFAULT_SUMMARY_RATIO, 0.1, 1.0),
            summary_cooldown_turns: number(
                "summary_cooldown_turns",
                DEFAULT_SUMMARY_COOLDOWN_TURNS as usize,
                0,
                1000,
            ) as u64,
            summary_keep_recent_turns: number(
                "summary_keep_recent_turns",
                SUMMARY_KEEP_RECENT_TURNS,
                MIN_SUMMARY_KEEP_RECENT,
                MAX_SUMMARY_KEEP_RECENT,
            ),
            summary_enabled: value
                .get("summary_enabled")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            summary_base_url: text("summary_base_url"),
            summary_model: text("summary_model"),
            summary_api_key_env: text("summary_api_key_env"),
            summary_timeout_ms: number(
                "summary_timeout_ms",
                DEFAULT_SUMMARY_TIMEOUT_MS as usize,
                SUMMARY_MIN_TIMEOUT_MS as usize,
                SUMMARY_MAX_TIMEOUT_MS as usize,
            ) as u64,
        }
    }

    /// 摘要客户端的配置快照（assemble 与触发判定共用同一份投影）。
    pub fn summary_config(&self) -> SummaryConfig {
        SummaryConfig {
            enabled: self.summary_enabled,
            base_url: self.summary_base_url.clone(),
            model: self.summary_model.clone(),
            api_key_env: self.summary_api_key_env.clone(),
            timeout_ms: self.summary_timeout_ms,
            keep_recent: self.summary_keep_recent_turns,
        }
    }

    /// 解析实际 JSONL 路径（`strategy::resolve_store_path` 的配置包装）。
    pub fn resolve_store_path(&self, config_path: &str) -> Option<PathBuf> {
        strategy::resolve_store_path(config_path, &self.store_path)
    }
}

/// 本 Mod 的设置 schema（**静态**：未启用也拿得到，前端可先填再启用）。
///
/// 十个字段。两条边界要在 UI 上说清：
/// - `enabled_injection` 是「注入开关」——**不是**「Mod 启停开关」；
/// - `summary_*` 是**摘要专用**的 LLM 配置，**独立于主链 `[llm]`**
///   （Mod 拿到的设置快照是脱敏的，读不到主链 base_url / key 变量名，也不该读）。
///   想复用主链端点，就把同一组值抄进来；`.env` 里那把 key 可以共用同一个变量名。
pub fn memory_settings_spec() -> ModSettingsSpec {
    ModSettingsSpec {
        mod_id: DESCRIPTOR.id.to_string(),
        title: DESCRIPTOR.name.to_string(),
        version: 2,
        fields: vec![
            ModSettingField::String {
                key: "store_path".to_string(),
                label: format!("记忆库路径（留空 = 配置文件同目录的 {DEFAULT_STORE_FILE}）"),
                secret: false,
                default: None,
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
                label: "把检索结果注入本轮提示词（关掉则只记不注入）".to_string(),
                default: true,
            },
            ModSettingField::Bool {
                key: "summary_enabled".to_string(),
                label: format!(
                    "启用真摘要：桶内原文超过注入预算的 {:.0}% 时后台压缩成一段摘要（需要下面的端点与模型）",
                    DEFAULT_SUMMARY_RATIO * 100.0
                ),
                default: false,
            },
            ModSettingField::String {
                key: "summary_base_url".to_string(),
                label: "摘要 LLM base URL（OpenAI 兼容；留空 = 不摘要。可填与主链相同的端点）"
                    .to_string(),
                secret: false,
                default: None,
            },
            ModSettingField::String {
                key: "summary_model".to_string(),
                label: "摘要模型名（留空 = 不摘要）".to_string(),
                secret: false,
                default: None,
            },
            ModSettingField::String {
                key: "summary_api_key_env".to_string(),
                label: "摘要 API key 的环境变量名（留空 = 不鉴权；值写 .env，可复用主链那把）"
                    .to_string(),
                secret: false,
                default: None,
            },
            ModSettingField::Number {
                key: "summary_timeout_ms".to_string(),
                label: format!(
                    "摘要单次请求超时毫秒（{SUMMARY_MIN_TIMEOUT_MS}–{SUMMARY_MAX_TIMEOUT_MS}，缺省 {DEFAULT_SUMMARY_TIMEOUT_MS}）"
                ),
                min: SUMMARY_MIN_TIMEOUT_MS as f64,
                max: SUMMARY_MAX_TIMEOUT_MS as f64,
            },
            ModSettingField::Number {
                key: "summary_ratio".to_string(),
                label: format!("摘要触发阈值：桶内原文 / 注入预算（缺省 {DEFAULT_SUMMARY_RATIO}）"),
                min: 0.1,
                max: 1.0,
            },
            ModSettingField::Number {
                key: "summary_cooldown_turns".to_string(),
                label: format!(
                    "摘要冷却轮数（缺省 {DEFAULT_SUMMARY_COOLDOWN_TURNS}；冷却期内不重复触发）"
                ),
                min: 0.0,
                max: 1000.0,
            },
            ModSettingField::Number {
                key: "summary_keep_recent_turns".to_string(),
                label: format!("最近几轮原文不进摘要（缺省 {SUMMARY_KEEP_RECENT_TURNS}）"),
                min: MIN_SUMMARY_KEEP_RECENT as f64,
                max: MAX_SUMMARY_KEEP_RECENT as f64,
            },
        ],
    }
}
