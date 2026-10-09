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

use serde_json::Value;

use crate::config::LlmConfig;
use crate::performance::client::{
    DisabledPerformance, OpenAiPerformanceClient, PerformanceClient, StructuredMode,
};
use crate::performance::prompt::strip_code_fence;

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

/// 一句的**两份文本**（二路交回的成功结果）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanedSentence {
    /// 上屏文本（原文的连续切片）。
    pub display: String,
    /// 送 TTS 的文本（可为空 = 静音句）。
    pub speech: String,
}

impl CleanedSentence {
    /// 失败 / 未接二路：上屏与送 TTS 都用原文。
    pub fn raw(sentence: &str) -> Self {
        Self {
            display: sentence.to_string(),
            speech: sentence.to_string(),
        }
    }
}

/// 二路清洗运行时（引擎注入；缺省 [DisabledPerformance] = 不跑二路）。
pub struct SentenceCleaner {
    client: Box<dyn PerformanceClient>,
    timeout_ms: u64,
}

impl fmt::Debug for SentenceCleaner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SentenceCleaner")
            .field("client", &self.client.kind())
            .field("model", &self.client.model())
            .field("enabled", &self.client.enabled())
            .field("timeout_ms", &self.timeout_ms)
            .finish()
    }
}

impl SentenceCleaner {
    /// 注入式构造（测试 / 未来 host 替身）。
    pub fn new(client: Box<dyn PerformanceClient>, timeout_ms: u64) -> Self {
        Self { client, timeout_ms }
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
        match OpenAiPerformanceClient::with_api_key(
            &llm.base_url,
            &llm.model,
            llm.api_key.clone(),
            CLEAN_TIMEOUT_MS,
            // 纯文本 JSON 契约：不带 response_format（上游不保证支持 json_schema）。
            StructuredMode::Prompt,
            &[],
        ) {
            Ok(client) => Self::new(Box::new(client), CLEAN_TIMEOUT_MS),
            Err(reason) => {
                tracing::warn!(
                    target: "cleaning",
                    code = "clean_client_unavailable",
                    "{reason}（二路清洗不启用：这一轮的上屏与送 TTS 都走原文）"
                );
                Self::disabled()
            }
        }
    }

    /// 二路是否可用（不可用时引擎走既有的确定性清洗）。
    pub fn enabled(&self) -> bool {
        self.client.enabled()
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
        let reply = match self
            .client
            .request(CLEAN_SYSTEM_PROMPT, &user, self.timeout_ms)
            .await
        {
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
        match parse_cleaned(&raw, sentence) {
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
pub fn parse_cleaned(raw: &str, sentence: &str) -> Option<CleanedSentence> {
    let value: Value = serde_json::from_str(raw.trim()).ok()?;
    let display = value.get("display")?.as_str()?;
    let speech = value.get("speech")?.as_str()?;
    if display.is_empty() || !sentence.contains(display) {
        return None;
    }
    Some(CleanedSentence {
        display: display.to_string(),
        speech: speech.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::performance::client::{PerformanceFuture, PerformanceReply};

    /// 固定回一份 raw 的测试替身（`reply=None` = 请求失败）。
    #[derive(Debug)]
    struct StubCleaner {
        reply: Option<String>,
    }

    impl PerformanceClient for StubCleaner {
        fn enabled(&self) -> bool {
            true
        }
        fn kind(&self) -> &'static str {
            "injected"
        }
        fn has_api_key(&self) -> bool {
            false
        }
        fn request<'a>(
            &'a self,
            _system: &'a str,
            _user: &'a str,
            _timeout_ms: u64,
        ) -> PerformanceFuture<'a, Option<PerformanceReply>> {
            let reply = self.reply.clone().map(|raw| PerformanceReply {
                raw,
                structured: false,
            });
            Box::pin(async move { reply })
        }
    }

    /// **计划口径的那份假响应**：TTS 文本 `你好`、上屏文本 `你好😄`。
    #[tokio::test]
    async fn accepted_plan_splits_display_and_speech() {
        let cleaner = SentenceCleaner::new(
            Box::new(StubCleaner {
                reply: Some(r#"{"display":"你好😄","speech":"你好"}"#.to_string()),
            }),
            CLEAN_TIMEOUT_MS,
        );
        let cleaned = cleaner.clean("你好😄").await;
        assert_eq!(cleaned.speech, "你好", "送 TTS 的必须只有可念文本");
        assert_eq!(cleaned.display, "你好😄", "上屏必须保留 emoji");
    }

    /// 空 `speech` = 这一句不发 TTS（引擎侧走静音句），上屏照旧。
    #[tokio::test]
    async fn empty_speech_is_kept_and_display_survives() {
        let cleaner = SentenceCleaner::new(
            Box::new(StubCleaner {
                reply: Some(r#"{"display":"（挥手）","speech":""}"#.to_string()),
            }),
            CLEAN_TIMEOUT_MS,
        );
        let cleaned = cleaner.clean("（挥手）").await;
        assert_eq!(cleaned.speech, "");
        assert_eq!(cleaned.display, "（挥手）");
    }

    /// `display` 不是原文切片 → 整句作废，改送原文（上屏也是原文）。
    #[tokio::test]
    async fn display_must_be_a_contiguous_slice_of_the_sentence() {
        let cleaner = SentenceCleaner::new(
            Box::new(StubCleaner {
                reply: Some(r#"{"display":"你好啊","speech":"你好啊"}"#.to_string()),
            }),
            CLEAN_TIMEOUT_MS,
        );
        let cleaned = cleaner.clean("你好😄").await;
        assert_eq!(
            cleaned,
            CleanedSentence::raw("你好😄"),
            "不是切片就要整句作废"
        );
    }

    /// 请求失败 / 坏 JSON / 缺字段 → 一律原文（**不新写正则去猜**）。
    #[tokio::test]
    async fn failures_fall_back_to_the_raw_sentence() {
        for reply in [
            None,
            Some("这不是 JSON".to_string()),
            Some(r#"{"display":"你好"}"#.to_string()),
            Some(r#"{"speech":"你好"}"#.to_string()),
            Some(r#"{"display":"","speech":"你好"}"#.to_string()),
        ] {
            let cleaner = SentenceCleaner::new(
                Box::new(StubCleaner {
                    reply: reply.clone(),
                }),
                CLEAN_TIMEOUT_MS,
            );
            let cleaned = cleaner.clean("你好😄（挥手）").await;
            assert_eq!(
                cleaned,
                CleanedSentence::raw("你好😄（挥手）"),
                "reply={reply:?}"
            );
        }
    }

    /// 没接二路（disabled）→ 引擎走既有路径：`enabled() == false`。
    #[test]
    fn disabled_cleaner_is_off_and_from_llm_respects_the_flag() {
        assert!(!SentenceCleaner::disabled().enabled());
        let mut llm = LlmConfig::new("https://api.deepseek.com/v1", "deepseek-flash");
        assert!(
            !SentenceCleaner::from_llm(&llm).enabled(),
            "clean_tts=false"
        );
        llm.clean_tts = true;
        let on = SentenceCleaner::from_llm(&llm);
        assert!(on.enabled());
        assert_eq!(on.model(), "deepseek-flash");
        // 端点非法也不失败：转 disabled（回原文）。
        let mut bad = LlmConfig::new("", "deepseek-flash");
        bad.clean_tts = true;
        assert!(!SentenceCleaner::from_llm(&bad).enabled());
    }
}
