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

use serde_json::Value;
use std::fmt;

use crate::config::LlmConfig;
use crate::performance::PerformanceCue;
use crate::performance::client::{
    DisabledPerformance, OpenAiPerformanceClient, PerformanceClient, StructuredMode,
};
use crate::performance::prompt::strip_code_fence;
use crate::secret::ApiSecret;

/// 送 TTS 稿里**连续数字段**的最小长度（T1，2026-10-13 执行单）。
///
/// 本地 TTS 实测会拒收超长数字（400「超出数据范围，最长支持 16 位」），
/// 而一个 31 位的数字会把**整轮**判失败。17 = 上游明说的 16 位再多一位——
/// 短于它的原样念，不去猜上游还有没有别的边界。
pub const LONG_DIGIT_RUN_MIN: usize = 17;

/// 送 TTS 稿里长数字段的替换文本（念出来是这一句）。
pub const LONG_DIGIT_PLACEHOLDER: &str = "一长串数字";

/// **只改送 TTS 的那一份**：把长度 ≥ [`LONG_DIGIT_RUN_MIN`] 的连续 ASCII 数字段
/// （0-9，中间没有别的字符）整段换成 [`LONG_DIGIT_PLACEHOLDER`]。
///
/// 口径（T1，2026-10-13 执行单）：
/// - 短于 17 位的数字段**一字不动**（小数目该照念）；
/// - **不换上屏那份**：屏幕上的那一句仍是一路原文的连续切片；
/// - 除数字外**不做任何清洗**——emoji / 颜文字 / 动作描写是二路模型的活，
///   本函数一个新正则都不加（历史上「顺手加清洗」正是漂移的来源）。
///
/// 调用点只有一处：`crate::conversation::worker` 发 TTS 请求之前。
pub fn speech_for_tts(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i - start >= LONG_DIGIT_RUN_MIN {
                out.push_str(LONG_DIGIT_PLACEHOLDER);
            } else {
                // start..i 全是 ASCII：切片必定落在 char 边界上。
                out.push_str(&text[start..i]);
            }
            continue;
        }
        // 非数字：按 char 边界整字复制（多字节字符不许被劈开）。
        match text[i..].chars().next() {
            Some(ch) => {
                out.push(ch);
                i += ch.len_utf8();
            }
            None => break,
        }
    }
    out
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
/// 能力集由调用方注入（= runtime `performance::rules` 的主 allowlist）。
/// 提示词不写死 id 列表。
///
/// `bias` = 表演倾向（2026-10-13 执行单 T3）：`None` = 既有「克制」文案
/// （2026-10-13 前的行为逐字不变）；`Some` = 只把 `cues：` 那一句换成 bias 的
/// `cue_clause`，`display` / `speech` 两句保持原样。
pub fn motion_system_prompt(allowed: &[String], bias: Option<&MotionBias>) -> String {
    let clause = match bias {
        Some(bias) => bias.clause().to_string(),
        None => format!(
            "只根据用户给你的这一句原文，从下面这组 id 里选 0~2 个动作预设，\
             不要根据别的句子或上下文，选不出就给空数组 []：{}",
            allowed.join(", ")
        ),
    };
    format!(
        "你是台词清洗器。用户给你一句台词原文，你只输出一份 JSON，不要输出其它任何字。\
JSON 形状固定为 {{\"display\": \"...\", \"speech\": \"...\", \"cues\": [\"<id>\", ...]}}。\
display：这一句原文里要显示给用户看的连续片段，必须能在原文里按原样、按顺序找到\
（通常就是整句原文，emoji、颜文字、括号里的动作描写都留着）。\
speech：拿去语音合成的文本——删掉 emoji、颜文字与括号里的动作描写，\
但**不要改写**剩下的台词。\
cues：{clause}。\
不要在 display 或 speech 里写任何表情标签。\
不要新增、删改或翻译台词内容，不要输出 Markdown 代码块。"
    )
}

/// 表演倾向在 runtime 侧的**最小投影**（2026-10-13 执行单 T3）。
///
/// 单一真源 = `live2d_ai_mod_director::motion_bias::MotionBias`（三档枚举 +
/// 锁死的 `cue_clause` 文案；rc4 沿用 mods.json 既有键 `motion_bias`，不新增
/// 键 / UI）。依赖方向（director → runtime）决定 runtime 不能反向依赖，所以
/// 这里只收两条派生值，由 host 装配时投影进来。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotionBias {
    /// 已含能力集 id 列表的完整 cue 子句（文案真源在 director 侧）。
    clause: String,
    /// 模型交回空数组时是否回退按句规则表（只有「主动」档 true）。
    fallback_on_empty: bool,
}

impl MotionBias {
    /// 由 host 从导演档位投影构造。
    pub fn new(clause: String, fallback_on_empty: bool) -> Self {
        Self {
            clause,
            fallback_on_empty,
        }
    }

    /// cue 子句（`motion_system_prompt` 的 `cues：` 那一句）。
    pub fn clause(&self) -> &str {
        &self.clause
    }

    /// 空数组是否回退规则表（只有主动档 true）。
    pub fn fallback_on_empty(&self) -> bool {
        self.fallback_on_empty
    }
}

/// 一句的**两份文本**（二路交回的成功结果）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanedSentence {
    /// 上屏文本（原文的连续切片）。
    pub display: String,
    /// 送 TTS 的文本（可为空 = 静音句）。
    pub speech: String,
    /// 二路请求了 cues 且**成功解析**时 = 模型给的有效 preset_id
    /// （可空 = 这一句不动）；未请求 / 未跑 / 失败 = `None`
    /// （引擎据此回退**这一句原文**的规则表）。
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
    /// 这次请求的 JSON 是否多要一份可选 `cues`（rc4 常驻路径恒 true；
    /// false 只用于「只清洗、不要动作」的显式装配 / 既有测试）。
    motion: bool,
    /// cue 的 preset id 能力集（用于校验模型给回的 id）。
    allowed: Vec<String>,
    /// 表演倾向（2026-10-13 执行单 T3；请求 cues 时生效）：`None` = 既有克制
    /// 文案，`Some` = 只换 `cues：` 那一句 + 决定空数组是否回退规则表。
    bias: Option<MotionBias>,
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
            bias: None,
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

    /// 从**显式端点**构造（host 常驻装配点；端点 = `[llm]` 同一份）。
    ///
    /// `motion` = 是否请求可选 `cues`（常驻路径传 true）；
    /// `allowed` = cue 的 preset id 能力集（= runtime rules 主 allowlist）。
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
                bias: None,
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
                    bias: None,
                }
            }
        }
    }

    /// 打开**动作开关**（解析可选 `cues`），能力集 = 主 allowlist。
    ///
    /// `bias` = 表演倾向（2026-10-13 执行单 T3；rc4 沿用 mods.json 既有键）：
    /// 克制 / 平常只换提示词子句，主动档还决定「模型交回空数组时改用规则表」
    /// （[`Self::empty_cues_fall_back`]）。
    pub fn with_motion(mut self, allowed: Vec<String>, bias: MotionBias) -> Self {
        self.motion = true;
        self.allowed = allowed;
        self.bias = Some(bias);
        self
    }

    /// 给**已按 `from_parts(.., motion=true, ..)` 构造**的清洗器补上表演倾向
    /// （host 常驻装配路径；`with_motion` 的同义补充，不重复设置 motion / allowed）。
    pub fn with_bias(mut self, bias: MotionBias) -> Self {
        self.bias = Some(bias);
        self
    }

    /// 二路是否可用（不可用时引擎走既有的确定性清洗）。
    pub fn enabled(&self) -> bool {
        self.client.enabled()
    }

    /// 这次请求是否要/解析 `cues`。
    pub fn motion(&self) -> bool {
        self.motion
    }

    /// 模型交回**空数组**时是否回退按句规则表（2026-10-13 执行单 T3：只有
    /// 「主动」档 true；克制 / 平常的空数组仍是成功）。转给 bias 的
    /// [`MotionBias::fallback_on_empty`]。
    pub fn empty_cues_fall_back(&self) -> bool {
        self.bias
            .as_ref()
            .map(|bias| bias.fallback_on_empty())
            .unwrap_or(false)
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
        // 不请求 cues → 旧两字段契约（`cue_ids = None` = 引擎回退规则表）；
        // 请求 cues → 同一次请求多要一份可选 `cues`，提示词按表演倾向
        // （2026-10-13 执行单 T3）换 `cues：` 那一句。
        let system = if self.motion {
            motion_system_prompt(&self.allowed, self.bias.as_ref())
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
    // 不请求 cues → 旧两字段契约（忽略 `cues`，`cue_ids = None`）；
    // 请求 cues → 解析可选 `cues`（缺字段 / 空数组 = 这一句不动，是**成功**）。
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
