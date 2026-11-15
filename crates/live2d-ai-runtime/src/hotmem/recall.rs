//! 「记」的两块纯逻辑：**抽取结果解析**与**近期折摘要**。
//!
//! 本文件不出网络、不读时钟。抽取的 HTTP 调用在宿主（desktop 的
//! `hot_slot::record_turn`），这里只负责把模型吐回来的文本变成事实，以及把
//! 超出 `max_history_pairs` 的旧轮次折成一条短摘要——两者都是纯函数，于是
//! 「模型返回一段带 markdown 围栏的 JSON」这种真实情况能在单测里逐字钉住。
//!
//! # 抽取口径（计划 §3）
//!
//! 短提示词只要求一个 JSON 数组 `[{text, kind}]`，`kind` 为 `episode` 或
//! `durable`。**抽失败就只把这一轮原文记成 episode，不再调用第二次**——所以
//! [parse_recalled] 对任何畸形输入都必须返回**确定的**结果（空表），调用方据此
//! 走兜底，而不是重试。
//!
//! # 摘要口径
//!
//! `≤300 字，替换上一条，不无限加长`。这里用**确定性折叠**而不是再调一次 LLM：
//! 多一次调用就多一次失败面，而「摘要失败就留下上一条」的兜底本身也让这一步变成
//! 可选项；确定性折叠永远成功、永远可复现，测试能逐字断言。

use super::SUMMARY_MAX_CHARS;
use super::fact::FactKind;
use super::store::Recollection;

/// 一次抽取最多接受多少条（防止模型一口气吐一百条把库撑大）。
pub const MAX_RECALL_ITEMS: usize = 8;

/// 单条事实正文的字符上限（超了截断）。
pub const MAX_RECALL_TEXT_CHARS: usize = 120;

/// 解析抽取结果：`[{text, kind}]` → 事实列表。
///
/// 容忍三件事（都是真实会遇到的）：
/// 1. 模型把 JSON 包在 `json 围栏里 → 取第一个 `[` 到最后一个 `]`；
/// 2. 数组里混着坏元素（缺 `text` / 类型不对）→ **逐条丢弃**，不整体失败；
/// 3. 空正文 / 全空白 → 丢。
///
/// 返回空表 = 这次抽取不可用，调用方走「记成 episode 原文」的兜底。
pub fn parse_recalled(raw: &str) -> Vec<Recollection> {
    let Some(start) = raw.find('[') else {
        return Vec::new();
    };
    let Some(end) = raw.rfind(']') else {
        return Vec::new();
    };
    if end <= start {
        return Vec::new();
    }
    let slice = &raw[start..=end];
    let Ok(items) = serde_json::from_str::<Vec<serde_json::Value>>(slice) else {
        return Vec::new();
    };
    let mut out: Vec<Recollection> = Vec::new();
    for item in items {
        let Some(text) = item.get("text").and_then(|v| v.as_str()) else {
            continue;
        };
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        let clipped: String = text.chars().take(MAX_RECALL_TEXT_CHARS).collect();
        if out.iter().any(|r: &Recollection| r.text == clipped) {
            continue;
        }
        let kind = FactKind::parse(item.get("kind").and_then(|v| v.as_str()));
        out.push(Recollection::new(clipped, kind));
        if out.len() >= MAX_RECALL_ITEMS {
            break;
        }
    }
    out
}

/// 把「被挤出 `max_history_pairs` 的旧轮次」折成一条 ≤300 字摘要。
///
/// 语义与计划 §2 的「一条 ≤300 字的滚动摘要」「替换上一条摘要，不无限加长」
/// 对齐：`previous` 是上一条摘要（可为空），`dropped` 是这次新掉出窗口的
/// 轮次（用户句 / 助手句）。结果 = 上一段 + 本段，从**最旧**开始丢，直到 ≤300。
///
/// 为什么从最旧开始丢：摘要要保「最近发生过什么」，而掉出窗口的这一批本身就比
/// 上一条摘要更新——保留新内容、压掉旧内容，语义上正好。
pub fn fold_summary(previous: &str, dropped: &[(String, String)]) -> String {
    let mut pieces: Vec<String> = Vec::new();
    let previous = previous.trim();
    if !previous.is_empty() {
        pieces.push(previous.to_string());
    }
    for (user, assistant) in dropped {
        let user = user.trim();
        let assistant = assistant.trim();
        if user.is_empty() && assistant.is_empty() {
            continue;
        }
        let piece = match (user.is_empty(), assistant.is_empty()) {
            (false, false) => format!("{user} → {assistant}"),
            (false, true) => user.to_string(),
            _ => assistant.to_string(),
        };
        pieces.push(piece);
    }
    if pieces.is_empty() {
        return String::new();
    }
    let joined = |parts: &[String]| parts.join(" / ");
    let mut start = 0usize;
    while start + 1 < pieces.len() && joined(&pieces[start..]).chars().count() > SUMMARY_MAX_CHARS {
        start += 1;
    }
    let text = joined(&pieces[start..]);
    text.chars().take(SUMMARY_MAX_CHARS).collect()
}

/// 抽取用的系统提示词（**短**，只要求一个 JSON 数组）。
///
/// 计划 §3：「记的抽取要短提示词，只要求 JSON 数组：`{text, kind}`」。
pub const RECALL_SYSTEM_PROMPT: &str = "从这一轮对话里抽出值得长期记住的事实。只输出 JSON 数组，不要解释，不要代码围栏。每项形如 {\"text\":\"…\",\"kind\":\"durable\"}。kind 只有两种：durable（关于对方的稳定事实：名字、喜好、住处、习惯、约定）与 episode（这次聊了什么）。没有值得记的就输出 []。";

/// 抽取用的用户消息：把这一对（用户句 / 助手句）原样给它。
pub fn recall_user_message(user: &str, assistant: &str) -> String {
    format!("用户说：{}\n助手回：{}", user.trim(), assistant.trim())
}
