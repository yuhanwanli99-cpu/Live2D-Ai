//! **二路按句清洗**（2026-10-08）：台词先交给快速模型清洗，再送 TTS。
//!
//! # 它解决什么
//!
//! 模型写出的台词里经常混着 emoji、颜文字、`（挥手）` 这类动作描写。原样交给
//! TTS 会被念出来（「左括号挥手右括号」），而原样剥掉又会让上屏丢掉这些内容。
//! 所以把「哪一部分拿去合成」交给**二路模型**判断：一句原文 → 一份
//! `{"display", "speech"}`。
//!
//! | 字段 | 用途 | 口径 |
//! | --- | --- | --- |
//! | `display` | **上屏**（气泡正文） | 必须是这一句原文的**连续切片**（`sentence.contains(display)`）；找不到就整句作废 |
//! | `speech` | **送 TTS** | 可念文本：emoji / 颜文字 / 动作描写可以删，剩下的台词**不得改写**；空串 = 这一句不发 TTS HTTP（仍上屏、仍占号） |
//!
//! # 失败一律回原文（不新写任何正则）
//!
//! 超时 / 非 2xx / 坏 JSON / 缺字段 / `display` 不是原文切片 → 这一句
//! `display = speech = 原文`。**不在这里剥 emoji 或颜文字**：那是二路模型的活，
//! 本模块不扩展 `clean_for_tts`、也不新增任何清洗正则。
//!
//! # 与一路的关系
//!
//! 模型与端点**与一路同一份**（`[llm]` 的 base_url / model / api_key）；请求体
//! 里**显式关思考**（`thinking.type = disabled`，见
//! [crate::performance::client::OpenAiPerformanceClient::build_body]）。
//! 这里不做任何延迟优化 / 探针 / 重试：关思考是开关，不是延迟课题。

use std::fmt;
use std::sync::Arc;

use serde_json::Value;

use crate::config::LlmConfig;
use crate::performance::PerformanceCue;
use crate::performance::client::{
    DisabledPerformance, OpenAiPerformanceClient, PerformanceClient, StructuredMode,
};
use crate::performance::prompt::strip_code_fence;
use crate::secret::ApiSecret;

/// host 注入的**按句规则回退**：这一句原文 + 句号 → 规则 cue。
///
/// 单一真源 = director 的 `rule_cues_for_sentence`（crate 依赖方向决定只能注入）。
/// **调用实参只能是这一句一路原文**，不得是用户输入。
///
/// 用 newtype 而不是裸 `Arc<dyn Fn>`：`ConversationEngine` 派生 `Debug`，而
/// `dyn Fn` 不实现 `Debug`——这里手写一份只打占位符的 `Debug`。
type SentenceRuleFn = Arc<dyn Fn(&str, u64) -> Vec<PerformanceCue> + Send + Sync>;

#[derive(Clone)]
pub struct SentenceRuleFallback(SentenceRuleFn);

impl SentenceRuleFallback {
    /// 包装一个纯函数闭包。
    pub fn new(f: impl Fn(&str, u64) -> Vec<PerformanceCue> + Send + Sync + 'static) -> Self {
        Self(Arc::new(f))
    }

    /// 用**这一句一路原文** + 句号求规则 cue。
    pub fn call(&self, text: &str, sentence_seq: u64) -> Vec<PerformanceCue> {
        (self.0)(text, sentence_seq)
    }
}

impl fmt::Debug for SentenceRuleFallback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SentenceRuleFallback")
    }
}

/// 单句清洗的超时（毫秒）。
///
/// 取值依据：二路只回一份两字段的短 JSON、且**关着思考**；正常在 1s 量级。
/// 给到 5s 是「网络抖动 + 慢机器」的余量，不是延迟预算——超时就按失败处理，
/// 这一句改送原文（宁可念出括号，也不卡住整轮）。
pub const CLEAN_TIMEOUT_MS: u64 = 5_000;

/// 二路 system 提示（**纯文本契约**：只回一份 JSON，不改写台词）。
pub const CLEAN_SYSTEM_PROMPT: &str = "你是台词清洗器。用户给你一句台词原文，\
你只输出一份 JSON，不要输出其它任何字。\
JSON 形状固定为 {\"display\": \"...\", \"speech\": \"...\"}。\
display：这一句原文里要显示给用户看的连续片段，必须能在原文里按原样、按顺序找到\
（通常就是整句原文，emoji、颜文字、括号里的动作描写都留着）。\
speech：拿去语音合成的文本——删掉 emoji、颜文字与括号里的动作描写，\
但**不要改写**剩下的台词。\
不要新增、删改或翻译台词内容，不要输出 Markdown 代码块。";

/// 动作门开着时的二路 system 提示：两字段契约照旧，另加**这一句**的可选 `cues`。
///
/// `cues` 只根据用户消息里的**这一句原文**选预设 id（不跨句、不看上下文）；
/// 能力集由调用方注入（= director 的主 allowlist）。提示词不写死 id 列表。
pub fn motion_system_prompt(allowed: &[String]) -> String {
    format!(
        "你是台词清洗器。用户给你一句台词原文，你只输出一份 JSON，不要输出其它任何字。\
JSON 形状固定为 {{\"display\": \"...\", \"speech\": \"...\", \"cues\": [\"<id>\", ...]}}。\
display：这一句原文里要显示给用户看的连续片段，必须能在原文里按原样、按顺序找到\
（通常就是整句原文，emoji、颜文字、括号里的动作描写都留着）。\
speech：拿去语音合成的文本——删掉 emoji、颜文字与括号里的动作描写，\
但**不要改写**剩下的台词。\
cues：只根据用户给你的这一句原文，从下面这组 id 里选 0~2 个动作预设，\
不要根据别的句子或上下文，选不出就给空数组 []：{}。\
不要在 display 或 speech 里写任何表情标签。\
不要新增、删改或翻译台词内容，不要输出 Markdown 代码块。",
        allowed.join(", ")
    )
}

/// 一句的**两份文本**（二路交回的成功结果）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanedSentence {
    /// 上屏文本（原文的连续切片）。
    pub display: String,
    /// 送 TTS 的文本（可为空 = 静音句）。
    pub speech: String,
    /// 动作门开着时 = 模型给的有效 preset_id（可空 = 这一句不动）；
    /// 关着 / 未跑 / 失败 = `None`（动作门下由调用方回退规则表）。
    pub cue_ids: Option<Vec<String>>,
}

impl CleanedSentence {
    /// 失败 / 未接二路：上屏与送 TTS 都用原文（`cue_ids = None` = 未解析）。
    pub fn raw(sentence: &str) -> Self {
        Self {
            display: sentence.to_string(),
            speech: sentence.to_string(),
            cue_ids: None,
        }
    }
}

/// 把**已校验**的 preset id 组装成 legacy cue（T12：host 填默认值）。
///
/// `field = None`（legacy 路径）、`hold` 不设；intensity=1、ttl_ms=2000。
pub fn cues_from_ids(ids: &[String], sentence_seq: u64) -> Vec<PerformanceCue> {
    ids.iter()
        .map(|id| PerformanceCue {
            sentence_seq,
            preset_id: id.clone(),
            intensity: 1,
            ttl_ms: 2_000,
            ..Default::default()
        })
        .collect()
}

/// 二路清洗运行时（引擎注入；缺省 [DisabledPerformance] = 不跑二路）。
pub struct SentenceCleaner {
    client: Box<dyn PerformanceClient>,
    timeout_ms: u64,
    /// **动作门**（T11，缺省关）：开着时这一次请求的 JSON 多要一份可选 `cues`。
    motion: bool,
    /// cue 的 preset id 能力集（动作门下用于校验模型给回的 id）。
    allowed: Vec<String>,
}

impl fmt::Debug for SentenceCleaner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SentenceCleaner")
            .field("client", &self.client.kind())
            .field("model", &self.client.model())
            .field("enabled", &self.client.enabled())
            .field("timeout_ms", &self.timeout_ms)
            .field("motion", &self.motion)
            .finish()
    }
}

impl SentenceCleaner {
    /// 注入式构造（测试 / 未来 host 替身）。
    pub fn new(client: Box<dyn PerformanceClient>, timeout_ms: u64) -> Self {
        Self {
            client,
            timeout_ms,
            motion: false,
            allowed: Vec::new(),
        }
    }

    /// 不跑二路：`enabled() == false`，引擎走既有的确定性清洗。
    pub fn disabled() -> Self {
        Self::new(Box::new(DisabledPerformance), CLEAN_TIMEOUT_MS)
    }

    /// 从**一路的 LLM 配置**构造（同一 base_url / model / key）。
    ///
    /// `llm.clean_tts == false` → [Self::disabled]：测试 / 便捷构造保持既有行为。
    /// 端点非法 / 模型名为空 → 同样 disabled（**不失败**：没有二路就回原文）。
    pub fn from_llm(llm: &LlmConfig) -> Self {
        if !llm.clean_tts {
            return Self::disabled();
        }
        Self::from_parts(
            &llm.base_url,
            &llm.model,
            llm.api_key.clone(),
            CLEAN_TIMEOUT_MS,
            false,
            Vec::new(),
        )
    }

    /// 从**显式端点**构造（动作门 `Own`：这一次按句清洗改用 staging 端点）。
    ///
    /// `motion` = 动作门是否开着（开着则请求体多要一份可选 `cues`）；
    /// `allowed` = cue 的 preset id 能力集（= director 主 allowlist）。
    pub fn from_parts(
        base_url: &str,
        model: &str,
        api_key: Option<ApiSecret>,
        timeout_ms: u64,
        motion: bool,
        allowed: Vec<String>,
    ) -> Self {
        match OpenAiPerformanceClient::with_api_key(
            base_url,
            model,
            api_key,
            timeout_ms,
            // 纯文本 JSON 契约：不带 response_format（上游不保证支持 json_schema）。
            StructuredMode::Prompt,
            &[],
        ) {
            Ok(client) => Self {
                client: Box::new(client),
                timeout_ms,
                motion,
                allowed,
            },
            Err(reason) => {
                tracing::warn!(
                    target: "cleaning",
                    code = "clean_client_unavailable",
                    "{reason}（二路清洗不启用：这一轮的上屏与送 TTS 都走原文）"
                );
                Self {
                    client: Box::new(DisabledPerformance),
                    timeout_ms,
                    motion,
                    allowed,
                }
            }
        }
    }

    /// 打开**动作开关**（解析可选 `cues`），能力集 = 主 allowlist。
    pub fn with_motion(mut self, allowed: Vec<String>) -> Self {
        self.motion = true;
        self.allowed = allowed;
        self
    }

    /// 二路是否可用（不可用时引擎走既有的确定性清洗）。
    pub fn enabled(&self) -> bool {
        self.client.enabled()
    }

    /// 动作门是否开着（解析 `cues`）。
    pub fn motion(&self) -> bool {
        self.motion
    }

    /// 二路会用的模型名（排障 / 状态面）。
    pub fn model(&self) -> &str {
        self.client.model()
    }

    /// 清洗一句。**任何失败都返回原文**（见模块头注）。
    pub async fn clean(&self, sentence: &str) -> CleanedSentence {
        if !self.enabled() {
            return CleanedSentence::raw(sentence);
        }
        let user = build_user_prompt(sentence);
        // 动作门关着 → 旧提示词 / 旧两字段解析（既有测试逐字不变）；
        // 开着 → 同一次请求多要一份可选 `cues`（T11）。
        let system = if self.motion {
            motion_system_prompt(&self.allowed)
        } else {
            CLEAN_SYSTEM_PROMPT.to_string()
        };
        let reply = match self.client.request(&system, &user, self.timeout_ms).await {
            Some(reply) => reply,
            None => {
                tracing::warn!(
                    target: "cleaning",
                    code = "clean_request_failed",
                    "二路清洗请求失败（这一句改送原文，原文照上屏）"
                );
                return CleanedSentence::raw(sentence);
            }
        };
        // 剥代码围栏是**宽容解析**，契约不变：仍必须过同一份校验（切片 + 字段）。
        let raw = strip_code_fence(&reply.raw);
        match parse_cleaned(&raw, sentence, self.motion, &self.allowed) {
            Some(cleaned) => cleaned,
            None => {
                tracing::warn!(
                    target: "cleaning",
                    code = "clean_plan_invalid",
                    "二路回的不是合法清洗结果（这一句改送原文，原文照上屏）"
                );
                CleanedSentence::raw(sentence)
            }
        }
    }
}

/// 二路 user 提示：只有这一句原文（不带上下文、不带历史）。
pub fn build_user_prompt(sentence: &str) -> String {
    format!("原文：{sentence}")
}

/// 解析并**校验**二路交回的 JSON。
///
/// 校验（任一条不过 → `None` = 整句作废、改送原文）：
/// 1. 是合法 JSON 对象；
/// 2. `display` 与 `speech` 都是字符串（`speech` 可为空串）；
/// 3. `display` 非空，且是原文的**连续切片**（`sentence.contains(display)`）。
pub fn parse_cleaned(
    raw: &str,
    sentence: &str,
    motion: bool,
    allowed: &[String],
) -> Option<CleanedSentence> {
    let value: Value = serde_json::from_str(raw.trim()).ok()?;
    let display = value.get("display")?.as_str()?;
    let speech = value.get("speech")?.as_str()?;
    if display.is_empty() || !sentence.contains(display) {
        return None;
    }
    // 动作门关着 → 旧两字段契约（忽略 `cues`，`cue_ids = None`）；
    // 开着 → 解析可选 `cues`（缺字段 / 空数组 = 这一句不动，是**成功**）。
    let cue_ids = motion.then(|| parse_cue_ids(value.get("cues"), allowed));
    Some(CleanedSentence {
        display: display.to_string(),
        speech: speech.to_string(),
        cue_ids,
    })
}

/// 解析 `cues`：元素只收 `preset_id`（字符串，或 `{"preset_id": "..."}`），
/// 且必须属于能力集，否则丢掉该条。空 / 缺字段 → 空表（这一句不动）。
fn parse_cue_ids(cues: Option<&Value>, allowed: &[String]) -> Vec<String> {
    let Some(items) = cues.and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            item.as_str()
                .or_else(|| item.get("preset_id").and_then(Value::as_str))
        })
        .filter(|id| allowed.iter().any(|a| a.as_str() == *id))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
#[path = "cleaning_tests.rs"]
mod tests;
