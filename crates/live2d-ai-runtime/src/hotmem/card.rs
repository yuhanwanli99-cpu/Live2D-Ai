//! 热卡：整块替换、三种行、`R` 分档、200 token 裁剪。
//!
//! # 替换语义（**本模块最重要的一条**）
//!
//! [HotCard] 只能整块构造（[HotCard::from_lines] / [HotCard::empty]）
//! **没有 push / extend / append**。想「加一条」只能拿旧行 + 新行重建一张——调用
//! 点因此长成 `card = HotCard::from_lines(new_lines)` 的样子，读代码的人一眼
//! 看得见「整块换掉」。计划 §2 的「禁止把检索结果往旧热卡后面追加」由类型形状
//! 守住，而不是靠注释。
//!
//! # 三种行（没有命中就是空，**不写空标题**）
//!
//! ```text
//! - [清楚] …
//! - [模糊] …
//! - [资料] …
//! ```
//!
//! `R >= 0.7` 标「清楚」，`0.2 <= R < 0.7` 标「模糊」，`R < 0.2` 不进热卡。
//! 外挂资料与保留率无关，只标「资料」。钉住的事只放「清楚」，**不放「资料」**。

use super::budget::{estimate_tokens, fit_lines};
use super::fact::HotFact;

/// 一行的分档。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CardTier {
    /// `R >= 0.7`：可以自然提起。
    Clear,
    /// `0.2 <= R < 0.7`：用不确定的说法。
    Vague,
    /// 外挂资料，与保留率无关。
    Material,
}

impl CardTier {
    /// 由保留率分档；`R < 0.2` → None（不进热卡）。
    pub fn from_retention(r: f64) -> Option<Self> {
        if !r.is_finite() {
            return None;
        }
        if r >= super::TIER_CLEAR_MIN_R {
            Some(Self::Clear)
        } else if r >= super::TIER_VAGUE_MIN_R {
            Some(Self::Vague)
        } else {
            None
        }
    }

    /// 热卡正文里的字面前缀（**只有这三种**）。
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Clear => "- [清楚] ",
            Self::Vague => "- [模糊] ",
            Self::Material => "- [资料] ",
        }
    }

    /// 稳定码（状态面 / 日志）。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::Vague => "vague",
            Self::Material => "material",
        }
    }
}

/// 热卡里的一行（**不含**前缀；渲染时才加）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardLine {
    /// 分档。
    pub tier: CardTier,
    /// 正文（单行，无换行）。
    pub text: String,
}

impl CardLine {
    /// 造一行；正文里的换行与前后空白被压成单个空格（热卡是「一行一条」）。
    pub fn new(tier: CardTier, text: impl Into<String>) -> Self {
        Self {
            tier,
            text: normalize_line(&text.into()),
        }
    }

    /// 渲染成 `- [清楚] …` 形状。
    pub fn render(&self) -> String {
        format!("{}{}", self.tier.prefix(), self.text)
    }
}

/// 把一段可能带换行 / 多余空白的文本压成单行。
fn normalize_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 一张热卡 = 一组已排好序、已裁剪的行。
///
/// **没有 `push`**：整块替换语义靠「只能整体构造」守住（见模块头注）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HotCard {
    lines: Vec<CardLine>,
}

impl HotCard {
    /// 空热卡（没有命中 = 空，渲染为空串，**不写空标题**）。
    pub const fn empty() -> Self {
        Self { lines: Vec::new() }
    }

    /// 由候选行**整块**造卡：丢弃已重复的、按 200 token 预算裁剪。
    ///
    /// - `recent` 是本轮已经出现在消息列表里的原文（最近几轮的用户句 / 助手句）：
    ///   某一行与其中任何一句**逐字相同**就丢掉（避免复读已经在上下文里的话）；
    /// - 裁剪按 [super::render::HOT_CARD_BUDGET_TOKENS]，整行收，最后一行超预算
    ///   就整行不要（热卡宁可少一条，也不给半句）；
    /// - 顺序：清楚 → 模糊 → 资料（调用方给的顺序在**同档内**保留）。
    pub fn from_lines(candidates: Vec<CardLine>, recent: &[String]) -> Self {
        let mut kept: Vec<CardLine> = Vec::new();
        for line in candidates {
            if line.text.is_empty() {
                continue;
            }
            if recent.iter().any(|r| r.trim() == line.text) {
                continue;
            }
            if kept.iter().any(|k| k.text == line.text) {
                continue;
            }
            kept.push(line);
        }
        let rendered: Vec<String> = kept.iter().map(CardLine::render).collect();
        let fitted = fit_lines(&rendered, super::render::HOT_CARD_BUDGET_TOKENS);
        // 按渲染串反查回结构化行——裁剪只可能「留下前缀里的若干整行」，
        // 所以 refit 的条数一定 <= kept。
        let mut lines = Vec::with_capacity(fitted.len());
        for line in &kept {
            if fitted.iter().any(|f| *f == line.render()) {
                lines.push(line.clone());
            }
        }
        Self { lines }
    }

    /// 由事实列表造卡（正文 = 事实文本，档位按当前保留率；`R < 0.2` 丢）。
    ///
    /// 这是「查」的**最小实现**：没有向量时的候选集就是钉住 + 未过期事实。
    pub fn from_facts(facts: &[HotFact], recent: &[String]) -> Self {
        let mut candidates: Vec<CardLine> = facts
            .iter()
            .filter_map(|f| f.tier().map(|t| CardLine::new(t, f.text.clone())))
            .collect();
        candidates.sort_by_key(|l| l.tier);
        Self::from_lines(candidates, recent)
    }

    /// 行（只读）。
    pub fn lines(&self) -> &[CardLine] {
        &self.lines
    }

    /// 是否空卡。
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// 估算 token 数（与预算同一口径）。
    pub fn tokens(&self) -> usize {
        estimate_tokens(&render_hot_card(Some(self)))
    }
}

/// 渲染热卡正文：每行一条，`\n` 分隔；空卡 → 空串（**不写空标题**）。
///
/// `None` 与空卡同义（「这一轮没有热卡」）。
pub fn render_hot_card(card: Option<&HotCard>) -> String {
    let Some(card) = card else {
        return String::new();
    };
    card.lines
        .iter()
        .map(CardLine::render)
        .collect::<Vec<_>>()
        .join("\n")
}
