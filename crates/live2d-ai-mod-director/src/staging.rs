//! 异步第二路 LLM 的客户端接口（P1-3）与**真实 OpenAI 兼容客户端**的装配面
//! （P1-4，2026-09-19）。
//!
//! # 现状（诚实标注）
//!
//! - **接口**（本文件）：`StagingClient` 同步契约 + 缺省 `DisabledStaging`；
//! - **真实客户端**（[`crate::staging_http`]）：OpenAI 兼容
//!   `POST {base_url}/chat/completions`（**非流式**，独立 base_url/model/timeout/
//!   api_key_env），由 `DirectorFactory::create` 在 host 装配时按配置构造并注入；
//! - **缺配置 / `staging_enabled=false`** → 仍装配 `DisabledStaging`，行为与
//!   「默认关」逐字一致（`staging_http::assemble`）。
//!
//! 输入纪律：**绝不含 reasoning_content**（思考不进句子装配器 / TTS）。
//! 输出只能读、不能改送 TTS 的文本——导演是备注，不是誊写员。
//!
//! # v1 三字段（V12 事实：director 只产 cues）
//!
//! [STAGING_SYSTEM] 同时给出两条合法形态（V11：只增不改）：
//! - legacy：`{sentence_seq, preset_id, intensity, ttl_ms}`；
//! - v1：`{sentence_seq, field, x/y/z, id, intensity, at, hold}`
//!   （协议 §2.2；`docs/architecture/performance-protocol-v1.md`）。
//!
//! 两条形态**都只产 cues**——这里没有任何 `segments` / `speak`：
//! 切分原文是表演层（runtime）的职责。

use crate::DirectorConfig;

/// 异步第二路 LLM 客户端（host 注入）。
pub trait StagingClient: Send + Sync {
    /// 该客户端是否可用（Disabled -> false；Mod 据此如实报告）。
    fn enabled(&self) -> bool {
        true
    }
    /// 客户端种类（`state_json.staging.client` 可观察）：
    /// `"disabled"` / `"openai"` / `"injected"`（测试 / 未来 host 替身）。
    fn kind(&self) -> &'static str {
        "injected"
    }
    /// 是否已解析出一把 API key（**布尔可以出门，值不可以**）。
    fn has_api_key(&self) -> bool {
        false
    }
    /// 同步补全；None = 失败 / 超时 / 未配置（调用方静默回退规则）。
    fn complete(&self, system: &str, user: &str, timeout_ms: u64) -> Option<String>;
}

/// 未接线的缺省实现：enabled=false、complete 恒 None。
pub struct DisabledStaging;

impl StagingClient for DisabledStaging {
    fn enabled(&self) -> bool {
        false
    }
    fn kind(&self) -> &'static str {
        "disabled"
    }
    fn complete(&self, _system: &str, _user: &str, _timeout_ms: u64) -> Option<String> {
        None
    }
}

/// 二路端点**接管判定**（2026-10-07 T8）：host 的 `build_performance_runtime` 与本 Mod
/// 的 `should_fire_async` 共用这一份规则——同一份配置不允许出现两种解释。
///
/// | `staging_base_url` | `staging_model` | 结果 |
/// | --- | --- | --- |
/// | 空 | 空 | [Takeover::ReuseConversation]：主链复用对话模型 |
/// | 有 | 有 | [Takeover::Own]：主链改用这里填的端点 |
/// | 只填一项 | 只填一项 | [Takeover::Incomplete]：**不接管**，保持直送 |
///
/// `staging_enabled` **不是**第二道总闸：它只决定本 Mod 自己的异步 HTTP，
/// 不参与接管判定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Takeover {
    /// 两项都空：复用对话模型（主 LLM 的 base_url / 模型名 / 密钥变量名）。
    ReuseConversation,
    /// 两项都有：用二路自己的端点（`staging_api_key_env` 空则仍用主 LLM 的密钥变量名）。
    Own,
    /// 只填了一项：不接管，保持直送。
    Incomplete,
}

impl Takeover {
    /// 主链是否已接管（`Incomplete` = 没接上）。
    pub fn active(self) -> bool {
        !matches!(self, Self::Incomplete)
    }

    /// 稳定字符串（`state_json.staging.takeover.source`）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReuseConversation => "conversation",
            Self::Own => "own",
            Self::Incomplete => "none",
        }
    }
}

/// 判定（**唯一实现**；host 与 Mod 两侧都调它，不各写一份 `is_empty` 比较）。
pub fn takeover_of(base_url: &str, model: &str) -> Takeover {
    match (base_url.trim().is_empty(), model.trim().is_empty()) {
        (true, true) => Takeover::ReuseConversation,
        (false, false) => Takeover::Own,
        _ => Takeover::Incomplete,
    }
}

/// 装配结果：客户端 + 「开了二路但没接上」的原因（可观察 degraded）。
pub struct StagingSetup {
    /// 真正注入运行时的客户端。
    pub client: Box<dyn StagingClient>,
    /// `Some(reason)` = `staging_enabled=true` 但端点/模型缺失或非法 →
    /// **仅规则**；reason 不含密钥，只写进日志与 `state_json.staging.note`。
    pub degraded_note: Option<String>,
}

impl StagingSetup {
    /// 关闸 / 未启用：与「默认关」逐字一致（无 note）。
    pub fn disabled() -> Self {
        Self {
            client: Box::new(DisabledStaging),
            degraded_note: None,
        }
    }

    /// 开了二路但没接上：仍用 Disabled，附原因。
    pub fn degraded(reason: impl Into<String>) -> Self {
        Self {
            client: Box::new(DisabledStaging),
            degraded_note: Some(reason.into()),
        }
    }
}

/// 异步 LLM 的 system 提示（钉死输出契约，降低解析失败率）。
pub const STAGING_SYSTEM: &str = "你是 Live2D 皮套的动作导演。只输出 JSON，不要解释，也不要输出任何台词文本——导演只产动作 cue，不产文本。格式：{\"epoch\":<整数>,\"covers_upto_seq\":<整数>,\"cues\":[{\"sentence_seq\":<整数>,\"preset_id\":\"<id>\",\"intensity\":1,\"ttl_ms\":2000},{\"sentence_seq\":<整数>,\"field\":\"body|head|expression\",\"x\":0.0,\"y\":0.3,\"id\":\"<表情id>\",\"intensity\":1,\"at\":\"now|seg:<n>|after_prev\",\"hold\":true}]}。plan.epoch 必须原样回填用户消息里给出的「本轮 epoch」——回错/不回的值会让整份 plan 被丢弃。preset_id 与 expression 的 id 只允许使用给定能力集里的值；不要输出 priority。field 形态里 hold 必填（true=保持到下次指令）；body 不接受 z。";

/// 组装异步 LLM 的用户输入：**本轮 epoch** + 用户正文 + 助手正文（已清洗的
/// TTS 文本）+ 能力集。
///
/// 为什么必须带 epoch：`arbiter` 以 epoch 为硬闸（不匹配整份丢弃），而模型没有
/// 别的途径知道当前轮次——不带它，真实端点的 plan 会**永远**被丢。这正是
/// 「接线完成了却什么也不发生」那类缺陷。
///
/// 不含思考；不修改任何文本。
pub fn build_user_prompt(
    epoch: u64,
    user_text: &str,
    assistant_text: &str,
    allow: &[&str],
) -> String {
    format!(
        "本轮 epoch：{epoch}\n用户：{user_text}\n助手（送 TTS 的正文，逐字勿改）：{assistant_text}\n能力集：{}",
        allow.join(", ")
    )
}

/// 按导演配置装配二路客户端（host 装配点 = `DirectorFactory::create`）。
///
/// 口径（**唯一**入口，不要在别处再写一份判断）：
/// 1. `staging_enabled=false` → `DisabledStaging`（与「默认关」逐字一致）；
/// 2. 开了但 `staging_base_url` / `staging_model` 为空或 URL 非法 →
///    `DisabledStaging` + `degraded_note`（仅规则，可观察）；
/// 3. 否则 → `OpenAiStagingClient`（密钥经 `live2d_ai_runtime::secrets::lookup`）。
pub fn assemble(config: &DirectorConfig) -> StagingSetup {
    if !config.staging_enabled {
        return StagingSetup::disabled();
    }
    match crate::staging_http::OpenAiStagingClient::new(
        &config.staging_base_url,
        &config.staging_model,
        &config.staging_api_key_env,
        config.staging_timeout_ms,
    ) {
        Ok(client) => {
            let note = if client.has_api_key() {
                None
            } else if config.staging_api_key_env.trim().is_empty() {
                Some(
                    "未配 staging_api_key_env：请求不带 Authorization（端点要求鉴权时会失败）"
                        .to_string(),
                )
            } else {
                Some(format!(
                    "staging_api_key_env={} 在 .env / 进程环境里查不到值：请求不带 Authorization",
                    config.staging_api_key_env.trim()
                ))
            };
            StagingSetup {
                client: Box::new(client),
                degraded_note: note,
            }
        }
        Err(reason) => StagingSetup::degraded(format!("{reason}（二路关闭，仅规则层）")),
    }
}
