//! 真摘要（P1-5 补齐）：**摘要的纯逻辑 + LLM 客户端契约**。
//!
//! # 一句话
//!
//! 把「记忆桶被撑大」变成「把撑大的那段历史压成一段短摘要」：触发是
//! **非阻塞**的（后台线程跑 HTTP），产出落旁车文件（[crate::summary_store]），
//! 下一轮起以「摘要」身份进本 conversation 的注入块。
//!
//! # 输入是什么（**不是**整个对话）
//!
//! memory Mod 能看到的只有**记忆桶**（Wave 3 起每轮**用户 + 助手**各一行）——
//! 它拿不到 LLM 的多轮 messages。所以摘要的输入是「该桶里**尚未被摘要覆盖**的
//! 原文」，且**排除最近 keep_turns 轮**（缺省 4；按记录的 turn 分组，同轮的
//! user + assistant 一并排除）：最近几轮原文还在上下文窗口里，压进摘要既浪费
//! token 又会让「刚刚说的话」变成二手转述。
//!
//! ```text
//! 桶里 n 条原文：  [0 .. covers_upto)             已被上一版摘要覆盖（不再重复喂）
//!                  [covers_upto .. kept_start)   <- 本次摘要的输入（user/assistant 交错）
//!                  [kept_start .. n)             最近 K 轮（同轮两条一起留）
//! ```
//!
//! # 触发（口径）
//!
//! 桶内原文**总字符数** > 注入预算 x summary_ratio（缺省 0.75）
//! ＋ 过冷却（summary_cooldown_turns）＋ 有 >= [MIN_PENDING_RECORDS] 条
//! 待压原文 ＋ 客户端可用 -> 起后台线程。
//!
//! 为什么按**桶的总字符**而不是「本轮注入块占用」：本轮注入块只有 top-k
//! 几条，永远很小；真正在膨胀的是**库**。按注入块判会让阈值永远不触发，
//! 那正是「标记了待摘要却从没有摘要」的现场。
//!
//! # 失败 = 无摘要
//!
//! 超时 / 非 2xx / 坏 JSON / 只有思考没有正文 -> 返回 None ->
//! **旁车文件一字不改**（只记 last_error），注入退回「只有 top-k 原文」
//! 的既有形态。绝不写半截摘要、绝不 panic、绝不阻塞主链。
//!
//! # 输出纪律
//!
//! 只取 choices[0].message.content；reasoning_content 一律忽略
//! （与导演二路同一条纪律：思考不进任何注入面）。

use std::time::Duration;

use serde_json::Value;

use crate::strategy;

/// 摘要 system 提示（钉死输出契约，降低「模型开始写解释」的概率）。
pub const SUMMARY_SYSTEM: &str = "你是对话记忆压缩器。把用户给出的对话历史要点压成一段简短的中文摘要：只保留事实、偏好、约定、称呼、未完成的事；历史里的 [用户] 与 [助手] 标明每句话是谁说的，归属不要搞反。不要复述寒暄，不要评论，不要输出标题或 Markdown，不要提到「摘要」这个词。直接输出摘要正文。";

/// 一次摘要请求的输出 token 上限（短摘要；显式给硬上限）。
pub const MAX_TOKENS: u32 = 512;

/// 摘要正文的最大字符数（超出截断 + 省略号）。
///
/// 摘要必须比它压缩的原文短得多，否则「压缩」没有意义。超长的模型输出
/// 截断（而不是丢弃）——丢掉整份会让触发白跑，留着又吃掉预算。
pub const MAX_SUMMARY_CHARS: usize = 600;

/// 送进 LLM 的待压原文最大字符数（超出只取**最近**的那部分）。
///
/// 摘要输入本身也要有界：桶可能很长，而这条调用是后台的、按次计费的。
/// 从**新到旧**保留（越近的原文对摘要越重要），截断点之前的更老原文
/// 这轮不覆盖、下轮再进来（covers_upto 只推进到真正喂进去的那几条）。
pub const MAX_TRANSCRIPT_CHARS: usize = 8_000;

/// 触发摘要的最少待压条数（低于它不值得起一次 LLM 调用）。
pub const MIN_PENDING_RECORDS: usize = 2;

/// 客户端种类："disabled" / "openai" / "injected"（测试替身）。
pub trait Summarizer: Send + Sync {
    /// 是否可用（disabled -> false；Mod 据此如实报告，不假装「已启用」）。
    fn enabled(&self) -> bool {
        true
    }
    /// 客户端种类（进 state_json.summary.client）。
    fn kind(&self) -> &'static str {
        "injected"
    }
    /// 是否解析出 API key（**布尔可以出门，值不可以**）。
    fn has_api_key(&self) -> bool {
        false
    }
    /// 端点（日志 / 排障；不含密钥）。disabled -> 空串。
    fn endpoint(&self) -> &str {
        ""
    }
    /// 同步生成摘要；None = 失败 / 超时 / 未配置（调用方按「失败 = 无摘要」处理）。
    fn summarize(&self, system: &str, user: &str, timeout_ms: u64) -> Option<String>;
}

/// 未接线的缺省实现：enabled() == false、summarize 恒 None。
pub struct DisabledSummarizer;

impl Summarizer for DisabledSummarizer {
    fn enabled(&self) -> bool {
        false
    }
    fn kind(&self) -> &'static str {
        "disabled"
    }
    fn summarize(&self, _system: &str, _user: &str, _timeout_ms: u64) -> Option<String> {
        None
    }
}

/// 摘要的配置快照（从 [crate::MemoryConfig] 投影；assemble 与触发判定共用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryConfig {
    /// 总闸：false -> 永不摘要（旁车也不写）。
    pub enabled: bool,
    /// OpenAI 兼容 base URL（**独立于主链 [llm]**；空 = 无法摘要）。
    pub base_url: String,
    /// 模型名（空 = 无法摘要）。
    pub model: String,
    /// 存 API key 的环境变量**名**（空 = 不鉴权；值走 secrets::lookup）。
    pub api_key_env: String,
    /// 单次请求超时（毫秒）。
    pub timeout_ms: u64,
    /// 保留最近多少轮原文（不压进摘要）。
    pub keep_recent: usize,
}

/// 装配结果：客户端 + 「开了总闸但没接上」的原因（可观察 degraded）。
pub struct SummarySetup {
    pub client: std::sync::Arc<dyn Summarizer>,
    /// Some(reason) = enabled=true 但端点 / 模型缺失或非法 -> **不摘要**；
    /// reason 不含密钥，只进日志与 state_json.summary.note。
    pub degraded_note: Option<String>,
}

impl SummarySetup {
    /// 未启用：与「默认」逐字一致（无 note）。
    pub fn disabled() -> Self {
        Self {
            client: std::sync::Arc::new(DisabledSummarizer),
            degraded_note: None,
        }
    }

    /// 开了总闸但没接上：仍用 Disabled，附原因。
    pub fn degraded(reason: impl Into<String>) -> Self {
        Self {
            client: std::sync::Arc::new(DisabledSummarizer),
            degraded_note: Some(reason.into()),
        }
    }
}

/// 按配置装配摘要客户端（**唯一**装配点，别在别处再写一份判断）。
///
/// 1. enabled=false -> Disabled（无 note）；
/// 2. 开了但缺 base_url / model 或 URL 非法 -> Disabled + degraded_note；
/// 3. 否则 -> [crate::summary_http::OpenAiSummarizer]（密钥走 secrets::lookup）。
pub fn assemble(config: &SummaryConfig) -> SummarySetup {
    if !config.enabled {
        return SummarySetup::disabled();
    }
    match crate::summary_http::OpenAiSummarizer::new(
        &config.base_url,
        &config.model,
        &config.api_key_env,
        config.timeout_ms,
    ) {
        Ok(client) => {
            let note = if client.has_api_key() {
                None
            } else if config.api_key_env.trim().is_empty() {
                Some(
                    "未配 summary_api_key_env：请求不带 Authorization（端点要求鉴权时会失败）"
                        .to_string(),
                )
            } else {
                Some(format!(
                    "summary_api_key_env={} 在 .env / 进程环境里查不到值：请求不带 Authorization",
                    config.api_key_env.trim()
                ))
            };
            SummarySetup {
                client: std::sync::Arc::new(client),
                degraded_note: note,
            }
        }
        Err(reason) => SummarySetup::degraded(reason),
    }
}

/// 摘要请求的一对消息（system + user）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryRequest {
    pub system: String,
    pub user: String,
    /// 本次请求**真正覆盖到的位点**（桶内条数，1 起计；写入旁车的就是它）。
    pub covers_upto: usize,
    /// 覆盖到的最后一条的 turn（可读展示）。
    pub covers_turn: u64,
}

/// 按位点切出「待压原文」并组装请求；没有值得压的 -> None。
///
/// records 必须是**按落盘顺序**（旧 -> 新）的整桶记录；
/// start = 未被摘要覆盖的起点（见 [uncovered_start]）；
/// keep_turns = **保留最近几轮**（按 `turn` 分组，同轮的 user + assistant 一并
/// 保留，见 [strategy::recent_turn_window_start]）。
///
/// 窗口 [start, end) 是候选（Wave 3 起**用户与助手交错**在一起）；
/// 若其总长 > [MAX_TRANSCRIPT_CHARS]，从**最旧**一侧丢弃，直到不超预算——
/// 于是 covers_upto 只推进到真正喂进去的那些条，没喂进去的下轮再来。
///
/// 每条用 [strategy::MemoryRecord::line]（带 `[用户]` / `[助手]` 角色前缀）
/// 拼进 user 消息：摘要是**双方对话**的要点，模型必须知道哪句是谁说的
/// （用户偏好与助手承诺的归属完全不同）。
pub fn build_request(
    records: &[strategy::MemoryRecord],
    start: usize,
    keep_turns: usize,
) -> Option<SummaryRequest> {
    let end = strategy::recent_turn_window_start(records, keep_turns);
    if start >= end {
        return None;
    }
    let candidates = &records[start..end];
    if candidates.len() < MIN_PENDING_RECORDS {
        return None;
    }
    // 从新到旧累计，找最早的能进预算的那条。
    let mut total = 0usize;
    let mut first = candidates.len();
    for (offset, record) in candidates.iter().enumerate().rev() {
        total += record.line().chars().count() + 1;
        if total > MAX_TRANSCRIPT_CHARS {
            break;
        }
        first = offset;
    }
    if candidates.len() - first < MIN_PENDING_RECORDS {
        return None;
    }
    let chosen = &candidates[first..];
    let mut user = String::from("请把下面这些历史要点压成一段简短摘要：\n");
    for record in chosen {
        user.push_str("- ");
        user.push_str(&record.line());
        user.push('\n');
    }
    Some(SummaryRequest {
        system: SUMMARY_SYSTEM.to_string(),
        user,
        covers_upto: end,
        covers_turn: chosen.last().map(|r| r.turn).unwrap_or(0),
    })
}

/// 桶内「尚未被摘要覆盖」的起点（可越界的位点已钳位）。
///
/// 物理淘汰会让桶头部整体左移：那批被淘汰的记录**已经不在文件里了**，
/// 但摘要仍然「记得」它们。所以有效位点是
/// covers_upto.saturating_sub(evicted)，再钳到 0..=len。
/// 不这么算，淘汰一次就会让摘要覆盖位点指向错误位置——要么重复压、
/// 要么跳过一批没进摘要的原文。
pub fn uncovered_start(covers_upto: usize, evicted: usize, len: usize) -> usize {
    covers_upto.saturating_sub(evicted).min(len)
}

/// 把模型输出归一化成可落盘 / 可注入的摘要正文。
///
/// - 压平空白（注入块是 `- 行` 结构，换行会破坏它）；
/// - 剥掉可能被模型加上的引用符 / 项目符号 / 「摘要：」前缀；
/// - 超 [MAX_SUMMARY_CHARS] 截断 + 省略号；
/// - 归一化后为空 -> None（**空摘要按失败处理**，绝不写入旁车）。
pub fn normalize_summary(raw: &str) -> Option<String> {
    let flat = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut text = flat.as_str();
    for prefix in ["摘要：", "摘要:", "总结：", "总结:", "- ", "* "] {
        if let Some(rest) = text.strip_prefix(prefix) {
            text = rest.trim_start();
        }
    }
    let text = text.trim().trim_matches('"').trim();
    if text.is_empty() {
        return None;
    }
    let mut out: String = text.chars().take(MAX_SUMMARY_CHARS).collect();
    if text.chars().count() > MAX_SUMMARY_CHARS {
        out.push('…');
    }
    Some(out)
}

/// 摘要行的前缀（注入块里用来把「摘要」与「原文命中」分开）。
pub const SUMMARY_LINE_PREFIX: &str = "[摘要] ";

/// 把摘要和本轮命中拼成记忆行（**摘要永远在最前**：它描述的是更早的历史，
/// 先读它再读原文命中才是正确的时间顺序）。
///
/// summary 为空 -> 只返回命中行（失败 = 无摘要）。
pub fn compose_lines(summary: Option<&str>, memories: &[String]) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    if let Some(text) = summary.map(str::trim).filter(|s| !s.is_empty()) {
        lines.push(format!("{SUMMARY_LINE_PREFIX}{text}"));
    }
    lines.extend(memories.iter().cloned());
    lines
}

/// 解析 OpenAI 兼容响应里的 choices[0].message.content（只读正文）。
///
/// reasoning_content 即使存在也被忽略；正文空白 -> None
/// （「只有思考」不得冒充摘要）。
pub fn parse_completion(raw: &str) -> Option<String> {
    let value: Value = serde_json::from_str(raw).ok()?;
    let content = value
        .get("choices")?
        .as_array()?
        .first()?
        .get("message")?
        .get("content")?
        .as_str()?;
    normalize_summary(content)
}

/// 超时下限。
pub const MIN_TIMEOUT_MS: u64 = 500;
/// 超时上限（后台调用，允许比导演二路宽一点）。
pub const MAX_TIMEOUT_MS: u64 = 20_000;

/// 超时钳位（与配置解析同一口径）。
pub fn clamp_timeout_ms(ms: u64) -> u64 {
    ms.clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS)
}

/// 便捷：构造一个固定超时的 reqwest blocking 客户端（HTTP 实现共用）。
pub fn build_http_client(timeout_ms: u64) -> reqwest::Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(clamp_timeout_ms(timeout_ms)))
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
}
