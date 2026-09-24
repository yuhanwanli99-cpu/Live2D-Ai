//! **表演层**（导演 / 大脑）——每轮一份**合法化 JSON**，决定「这一轮说什么（speak）
//! 与做什么（cues）」。
//!
//! # 它在链路里的位置（与主模型的分工）
//!
//! ```text
//! 主模型（酒馆式角色扮演；无工具、无表演类预设）
//!    └─ 本轮助手原文（流式，只累积）
//!          └─► 表演层：POST {performance.base_url}/chat/completions（非流式）
//!                 └─ 一份 JSON：{"speak": string|null, "cues": [...]}
//!                       ├─ speak → 切句 → 送 TTS + 上屏（唯一真源）
//!                       └─ cues  → WS action_cue（锚该句音频 first_chunk）
//! ```
//!
//! **主模型不负责表演**：它只写剧情正文；动作 / 说辞整理归表演层。表演层默认关
//! （`[performance] enabled=false`）——关掉时主链保持既有的「边流边切句边送 TTS」
//! 行为，规则导演（director Mod）照旧。
//!
//! # 失败回退（**只有失败才回退**）
//!
//! 关 / 超时 / 非 2xx / 坏 JSON / 校验失败 → `speak=clean_for_tts(原文)` +
//! 规则 cue（由 host 注入的 [RuleFallback]，单一真源是 director 的规则层）。
//! 回退原因码见 [FallbackReason::code]，计数与最近原因见 [PerformanceStats]。
//!
//! # 为什么在 runtime 而不是 Mod
//!
//! 因为 `speak` 必须成为**主链 TTS / 上屏的真源**——Mod API 没有改写 TTS 文本的
//! 通道（那条纪律写死在 core-chain-baseline）。选择与并存关系见
//! docs/architecture/performance-layer-v0.md：**本模块是表演层主路由**；
//! director Mod 的 `staging_*`（二路 LLM）是同能力的**遗留并行实现**，
//! 缺省停用，保留为规则回退来源之一。

pub mod client;
pub mod plan;
pub mod prompt;

#[cfg(test)]
mod tests;

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

use serde_json::{Value, json};

pub use client::{
    DisabledPerformance, MAX_TIMEOUT_MS, MIN_TIMEOUT_MS, OpenAiPerformanceClient,
    PerformanceClient, PerformanceFuture, PerformanceReply, StructuredMode, parse_completion,
};
pub use plan::{
    MAX_CUES, MAX_INTENSITY, MAX_SPEAK_CHARS, MAX_TTL_MS, MIN_INTENSITY, MIN_TTL_MS,
    PRIORITY_PERFORMANCE, PerformanceCue, PerformancePlan, PlanError, action_cue_payload,
    json_schema_strict, parse_plan,
};
pub use prompt::{build_user_prompt, strip_code_fence};

/// host 注入的**规则回退**：正文 → 规则 cue（单一真源 = director 的纯函数）。
pub type RuleFallback = Arc<dyn Fn(&str) -> Vec<PerformanceCue> + Send + Sync>;

/// 本轮回退原因（`None` = 表演层成功）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FallbackReason {
    /// 没有回退：用了表演层的 JSON。
    #[default]
    None,
    /// 表演层关闸（缺省）或客户端不可用 → 主链保持原行为。
    Disabled,
    /// 超时 / 连接失败 / 非 2xx / 响应里没有正文。
    RequestFailed,
    /// 拿到了正文但过不了**同一个校验器**（坏 JSON / 缺字段 / 未知 preset）。
    InvalidPlan,
    /// 主模型本轮没有正文（没有可表演的内容）。
    EmptyAssistant,
}

impl FallbackReason {
    /// 稳定回退原因码（进日志与状态面；**不含正文**）。
    pub fn code(&self) -> &'static str {
        match self {
            Self::None => "performance_ok",
            Self::Disabled => "performance_disabled",
            Self::RequestFailed => "performance_request_failed",
            Self::InvalidPlan => "performance_plan_invalid",
            Self::EmptyAssistant => "performance_empty_assistant",
        }
    }

    /// 是否真的发生了回退。
    pub fn is_fallback(&self) -> bool {
        !matches!(self, Self::None)
    }

    fn as_u8(&self) -> u8 {
        match self {
            Self::None => 0,
            Self::Disabled => 1,
            Self::RequestFailed => 2,
            Self::InvalidPlan => 3,
            Self::EmptyAssistant => 4,
        }
    }

    fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Disabled,
            2 => Self::RequestFailed,
            3 => Self::InvalidPlan,
            4 => Self::EmptyAssistant,
            _ => Self::None,
        }
    }
}

/// 表演层累计计数（**只自增，不落盘**；Web 状态面读它）。
#[derive(Debug, Default)]
pub struct PerformanceStats {
    plans: AtomicU64,
    fallbacks: AtomicU64,
    noops: AtomicU64,
    cue_turns: AtomicU64,
    speak_turns: AtomicU64,
    last_reason: AtomicU8,
    /// 0 = 无 / 1 = json_schema / 2 = prompt。
    last_structured: AtomicU8,
}

impl PerformanceStats {
    /// 成功 plan 数。
    pub fn plans(&self) -> u64 {
        self.plans.load(Ordering::Relaxed)
    }
    /// 回退数（不含「本来就没开」的 [FallbackReason::Disabled]？——含，口径写死）。
    pub fn fallbacks(&self) -> u64 {
        self.fallbacks.load(Ordering::Relaxed)
    }
    /// 最近一次回退原因码。
    pub fn last_fallback(&self) -> &'static str {
        FallbackReason::from_u8(self.last_reason.load(Ordering::Relaxed)).code()
    }
    /// noop 轮数（speak 空且 cues 空）。
    pub fn noops(&self) -> u64 {
        self.noops.load(Ordering::Relaxed)
    }
    /// 说出台词的轮数。
    pub fn speak_turns(&self) -> u64 {
        self.speak_turns.load(Ordering::Relaxed)
    }
    /// 带 cue 的轮数。
    pub fn cue_turns(&self) -> u64 {
        self.cue_turns.load(Ordering::Relaxed)
    }
    /// 最近一次成功走的 structured 路（`None` = 还没成功过）。
    pub fn last_structured(&self) -> Option<&'static str> {
        match self.last_structured.load(Ordering::Relaxed) {
            1 => Some("json_schema"),
            2 => Some("prompt"),
            _ => None,
        }
    }
    /// 状态面 JSON（脱敏：没有正文、没有密钥）。
    pub fn to_json(&self) -> Value {
        let structured = match self.last_structured.load(Ordering::Relaxed) {
            1 => Some("json_schema"),
            2 => Some("prompt"),
            _ => None,
        };
        json!({
            "plans": self.plans(),
            "fallbacks": self.fallbacks(),
            "noops": self.noops.load(Ordering::Relaxed),
            "speak_turns": self.speak_turns.load(Ordering::Relaxed),
            "cue_turns": self.cue_turns.load(Ordering::Relaxed),
            "last_fallback": self.last_fallback(),
            "last_structured": structured,
        })
    }
}

/// 一次 `resolve` 的结果（引擎据此决定送不送 TTS、发不发 cue）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    /// 本轮要说的话；`None` = 不说（noop 的一半）。
    pub speak: Option<String>,
    /// 按句 cue；空 = 不动。
    pub cues: Vec<PerformanceCue>,
    /// 回退原因（`None` = 表演层成功）。
    pub reason: FallbackReason,
    /// 成功时是否走的 structured 路（回退时 `None`）。
    pub structured: Option<bool>,
}

impl Resolution {
    /// 本轮什么都不做。
    pub fn is_noop(&self) -> bool {
        self.speak.is_none() && self.cues.is_empty()
    }
}

/// 表演层运行时（host 构造并注入引擎；`Arc` 共享给状态面）。
pub struct PerformanceRuntime {
    client: Box<dyn PerformanceClient>,
    allow: Vec<String>,
    timeout_ms: u64,
    mode: StructuredMode,
    rule: Option<RuleFallback>,
    stats: Arc<PerformanceStats>,
}

impl fmt::Debug for PerformanceRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PerformanceRuntime")
            .field("client", &self.client.kind())
            .field("enabled", &self.client.enabled())
            .field("allow_len", &self.allow.len())
            .field("timeout_ms", &self.timeout_ms)
            .field("mode", &self.mode)
            .field("has_rule_fallback", &self.rule.is_some())
            .field("stats", &self.stats.to_json())
            .finish()
    }
}

impl PerformanceRuntime {
    /// 构造（`client` 缺省 [DisabledPerformance]）。
    pub fn new(
        client: Box<dyn PerformanceClient>,
        allow: Vec<String>,
        timeout_ms: u64,
        mode: StructuredMode,
        rule: Option<RuleFallback>,
    ) -> Self {
        Self {
            client,
            allow,
            timeout_ms,
            mode,
            rule,
            stats: Arc::new(PerformanceStats::default()),
        }
    }

    /// 计数句柄（与 Web 状态面共享同一个 `Arc`）。
    pub fn stats(&self) -> Arc<PerformanceStats> {
        Arc::clone(&self.stats)
    }

    /// 客户端是否可用（关掉时引擎走既有的流式路径）。
    pub fn enabled(&self) -> bool {
        self.client.enabled()
    }

    /// 客户端种类（状态面）。
    pub fn client_kind(&self) -> &'static str {
        self.client.kind()
    }

    /// structured 策略（状态面）。
    pub fn mode(&self) -> StructuredMode {
        self.mode
    }

    /// 能力集（面板 / 状态面可看）。
    pub fn allow(&self) -> &[String] {
        &self.allow
    }

    /// 单轮解析：成功给 speak+cues，任何失败给确定性回退。
    pub async fn resolve(&self, user_text: &str, assistant_text: &str) -> Resolution {
        let assistant = assistant_text.trim();
        if !self.client.enabled() {
            return self.fallback(assistant, FallbackReason::Disabled);
        }
        if assistant.is_empty() {
            return self.fallback(assistant, FallbackReason::EmptyAssistant);
        }
        let system = match self.mode {
            StructuredMode::Prompt => prompt::SYSTEM_JSON_ONLY,
            _ => prompt::SYSTEM_STRUCTURED,
        };
        let user = prompt::build_user_prompt(user_text, assistant, &self.allow);
        let reply = match self.client.request(system, &user, self.timeout_ms).await {
            Some(reply) => reply,
            None => return self.fallback(assistant, FallbackReason::RequestFailed),
        };
        // 剥围栏是**宽容解析**，契约不变：仍然必须过同一个校验器。
        let raw = prompt::strip_code_fence(&reply.raw);
        match plan::parse_plan(&raw, &self.allow) {
            Ok(plan) => {
                self.stats.plans.fetch_add(1, Ordering::Relaxed);
                self.stats.last_reason.store(0, Ordering::Relaxed);
                self.stats
                    .last_structured
                    .store(if reply.structured { 1 } else { 2 }, Ordering::Relaxed);
                if plan.is_noop() {
                    self.stats.noops.fetch_add(1, Ordering::Relaxed);
                }
                if plan.speak.is_some() {
                    self.stats.speak_turns.fetch_add(1, Ordering::Relaxed);
                }
                if !plan.cues.is_empty() {
                    self.stats.cue_turns.fetch_add(1, Ordering::Relaxed);
                }
                // 成功一份 JSON 摘要（**不打全文**、绝无密钥）。
                tracing::info!(
                    target: "performance",
                    structured = reply.structured,
                    "performance plan ok：{}",
                    plan.summary()
                );
                // speak 同走确定性清洗（只拆标记、不改句界）——任何路径都不把
                // Markdown / 舞台指示念出来。
                let speak = clean_speak(plan.speak.as_deref());
                Resolution {
                    speak,
                    cues: plan.cues,
                    reason: FallbackReason::None,
                    structured: Some(reply.structured),
                }
            }
            Err(e) => {
                self.stats.fallbacks.fetch_add(1, Ordering::Relaxed);
                self.stats.last_reason.store(3, Ordering::Relaxed);
                tracing::warn!(
                    target: "performance",
                    code = e.code(),
                    "{}（静默回退规则层）",
                    e.message()
                );
                self.fallback(assistant, FallbackReason::InvalidPlan)
            }
        }
    }

    /// 回退：`speak=clean_for_tts(原文)` + 规则 cue。
    fn fallback(&self, assistant: &str, reason: FallbackReason) -> Resolution {
        // 口径写死：**每一次没用上表演层 JSON 的轮次都计入 fallbacks**（含关闸）。
        // 想区分「关闸」与「开了但失败」看 [PerformanceStats::last_fallback] 的原因码。
        self.stats.fallbacks.fetch_add(1, Ordering::Relaxed);
        self.stats
            .last_reason
            .store(reason.as_u8(), Ordering::Relaxed);
        if reason.is_fallback() {
            tracing::warn!(
                target: "performance",
                code = reason.code(),
                "表演层回退：speak=确定性清洗(原文)，cues=规则"
            );
        }
        let speak = clean_speak(Some(assistant));
        let cues = self.rule.as_ref().map(|f| f(assistant)).unwrap_or_default();
        Resolution {
            speak,
            cues,
            reason,
            structured: None,
        }
    }
}

/// `clean_for_tts` + 空串归一成 `None`。
fn clean_speak(speak: Option<&str>) -> Option<String> {
    let raw = speak?;
    let cleaned = crate::dialogue::clean_for_tts(raw);
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// host 装配结果：运行时 + 「开了但没接上」的原因（可观察 degraded）。
pub struct PerformanceSetup {
    /// `None` = 表演层关闸（引擎走既有流式路径）。
    pub runtime: Option<Arc<PerformanceRuntime>>,
    /// `Some(reason)` = 开了但缺端点 / 缺模型 / URL 非法（仍装配，每轮回退）。
    /// 原因**不含密钥**，只写日志与状态面。
    pub note: Option<String>,
}

impl PerformanceSetup {
    /// 关闸：与「从来没有表演层」逐字一致。
    pub fn disabled() -> Self {
        Self {
            runtime: None,
            note: None,
        }
    }
}

/// 按配置装配（host 唯一入口）。
///
/// 1. `enabled=false` → `None`（既有流式路径）；
/// 2. 开了但 `base_url` / `model` 为空或 URL 非法 → 装配一个
///    [DisabledPerformance] 运行时 + `note`（每轮走确定性回退，degraded 可见）；
/// 3. 配齐 → [OpenAiPerformanceClient]（密钥经 [crate::secrets::lookup]）。
pub fn assemble(
    settings: &crate::settings::PerformanceSettings,
    allow: Vec<String>,
    rule: Option<RuleFallback>,
) -> PerformanceSetup {
    if !settings.enabled {
        return PerformanceSetup::disabled();
    }
    let timeout_ms = settings.effective_timeout_ms();
    let mode = StructuredMode::parse(&settings.structured);
    let api_key_env = settings.api_key_env.as_deref().unwrap_or("");
    match OpenAiPerformanceClient::new(
        &settings.base_url,
        &settings.model,
        api_key_env,
        timeout_ms,
        mode,
        &allow,
    ) {
        Ok(client) => {
            let note = if client.has_api_key() || api_key_env.trim().is_empty() {
                None
            } else {
                Some(format!(
                    "performance.api_key_env={} 在 .env / 进程环境里查不到值：请求不带 Authorization",
                    api_key_env.trim()
                ))
            };
            let runtime = PerformanceRuntime::new(Box::new(client), allow, timeout_ms, mode, rule);
            PerformanceSetup {
                runtime: Some(Arc::new(runtime)),
                note,
            }
        }
        Err(reason) => {
            let runtime = PerformanceRuntime::new(
                Box::new(DisabledPerformance),
                allow,
                timeout_ms,
                mode,
                rule,
            );
            PerformanceSetup {
                runtime: Some(Arc::new(runtime)),
                note: Some(format!("{reason}（每轮回退到确定性规则层）")),
            }
        }
    }
}
