//! 四块的拼接：**说话请求的 system 文本只由这里产生**。
//!
//! ```text
//! 系统句
//! 【钉住的事】
//! - …
//! 【本轮相关】
//! - [清楚] …
//! - [资料] …
//! ```
//!
//! 三条硬规则：
//! 1. **热卡空则不出现该段**（标题也不写）——不然模型会对着一个空标题编；
//! 2. 钉住块同理，空则不写标题；
//! 3. 拼好的东西**只进这一轮请求**：不写 `live2d-ai.toml`、不写会话槽、
//!    不打 `<!-- live2d-ai:memory-v0` 那种 marker。
//!
//! 弹幕解说仍接在**系统文本之后**（[crate::conversation::engine] 的既有行为），
//! 本模块不认识它。

use super::budget::estimate_tokens;
use super::card::{HotCard, render_hot_card};

/// 热卡的预算（token）。整块替换，超预算整行丢弃。
pub const HOT_CARD_BUDGET_TOKENS: usize = 200;

/// 钉住块的标题行。
pub const PINNED_HEADING: &str = "【钉住的事】";

/// 热卡的标题行。
pub const HOT_CARD_HEADING: &str = "【本轮相关】";

/// 滚动摘要的标题行（空摘要不写）。
pub const SUMMARY_HEADING: &str = "【之前聊过】";

/// 说话请求的 system 文本 = 系统句 + 钉住块 + 热卡（空块连同标题一起略过）。
///
/// 参数：
/// - `system_sentence`：本轮的系统句（**出厂不设人设**：用户没写就是空串，空块整段略过）；
/// - `pinned`：钉住块正文（[render_pinned] 的结果；空 = 没有）；
/// - `hot_card`：本轮热卡（`None` / 空卡 = 没有）。
pub fn compose_system_text(
    system_sentence: &str,
    pinned: &str,
    hot_card: Option<&HotCard>,
) -> String {
    compose_system_text_with_summary(system_sentence, pinned, hot_card, "")
}

/// 带**滚动摘要**的四块拼接（2026-11-16 热槽记忆 M4）。
///
/// 摘要块排在「最近对话」与「热卡」之间：它描述的正是**已经掉出历史窗口**的那几轮，
/// 读起来应当紧挨着历史之前、热卡之后。空摘要 = 不写这一块（连同标题）。
pub fn compose_system_text_with_summary(
    system_sentence: &str,
    pinned: &str,
    hot_card: Option<&HotCard>,
    summary: &str,
) -> String {
    let mut blocks: Vec<String> = Vec::with_capacity(4);
    let sentence = system_sentence.trim();
    if !sentence.is_empty() {
        blocks.push(sentence.to_string());
    }
    let pinned = pinned.trim();
    if !pinned.is_empty() {
        blocks.push(format!("{PINNED_HEADING}\n{pinned}"));
    }
    let card = render_hot_card(hot_card);
    if !card.is_empty() {
        blocks.push(format!("{HOT_CARD_HEADING}\n{card}"));
    }
    let summary = summary.trim();
    if !summary.is_empty() {
        blocks.push(format!("{SUMMARY_HEADING}\n{summary}"));
    }
    blocks.join("\n\n")
}

/// 四块的预算账（状态面 / 测试；**不参与渲染**）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PromptBudget {
    /// 系统句 token。
    pub system_sentence: usize,
    /// 钉住块 token（不含标题）。
    pub pinned: usize,
    /// 热卡 token（不含标题）。
    pub hot_card: usize,
}

impl PromptBudget {
    /// 按当前输入算账。
    pub fn measure(system_sentence: &str, pinned: &str, hot_card: Option<&HotCard>) -> Self {
        Self {
            system_sentence: estimate_tokens(system_sentence.trim()),
            pinned: estimate_tokens(pinned.trim()),
            hot_card: estimate_tokens(&render_hot_card(hot_card)),
        }
    }
}
