//! 钉住的事：400 token，**只收「清楚」档**，不放资料。
//!
//! 与热卡的区别是**来源与寿命**：热卡来自本轮草稿（一变就整块换），钉住块来自
//! 耐久事实库（跨轮存在，只随 `R` 掉档、被否认或预算挤掉而离开）。
//!
//! # 复读抑制
//!
//! 上一轮已经说过的钉住条目，本轮**可以留下索引，但渲染时跳过**（条目还在库里）。
//! 实现上就是渲染函数收一个 `spoken` 集合：命中即跳过，不删数据。

use super::budget::{estimate_tokens, fit_lines};

/// 钉住块的预算（token）。
pub const PINNED_BUDGET_TOKENS: usize = 400;

/// 钉住块里的一条（对应一条 durable 事实）。
#[derive(Debug, Clone, PartialEq)]
pub struct PinnedItem {
    /// 稳定 id（= 事实 id；用来做「上一轮说过没」的索引）。
    pub id: u64,
    /// 正文（单行）。
    pub text: String,
    /// 保留率（用来只收「清楚」档；渲染不带分档标记）。
    pub retention: f64,
}

impl PinnedItem {
    /// 造一条；正文压成单行。
    pub fn new(id: u64, text: impl Into<String>, retention: f64) -> Self {
        Self {
            id,
            text: text.into().split_whitespace().collect::<Vec<_>>().join(" "),
            retention,
        }
    }

    /// 是否够「清楚」档（`R >= 0.7`）。**钉住块只收清楚**，模糊的不钉。
    pub fn is_clear(&self) -> bool {
        self.retention.is_finite() && self.retention >= super::TIER_CLEAR_MIN_R
    }
}

/// 渲染钉住块：`- …` 每行一条，按 400 token 预算整行收。
///
/// 参数：
/// - `items`：候选（顺序即优先级；调用方按「新近加固优先」排好）；
/// - `spoken`：上一轮已经说过的 id —— **跳过但保留在库里**（复读抑制）。
///
/// 返回渲染好的正文（**不含标题行**；标题由 [super::render] 决定加不加）。
/// 空 = 这一轮没有钉住的事。
pub fn render_pinned(items: &[PinnedItem], spoken: &[u64]) -> String {
    let lines: Vec<String> = items
        .iter()
        .filter(|item| item.is_clear())
        .filter(|item| !spoken.contains(&item.id))
        .filter(|item| !item.text.is_empty())
        .map(|item| format!("- {}", item.text))
        .collect();
    fit_lines(&lines, PINNED_BUDGET_TOKENS).join("\n")
}

/// 渲染后估算 token（与预算同一口径；状态面 / 测试用）。
pub fn pinned_tokens(items: &[PinnedItem], spoken: &[u64]) -> usize {
    estimate_tokens(&render_pinned(items, spoken))
}
