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
//!
//! # 热卡不按保留率填（2026-11-16 维护者裁决）
//!
//! 保留率只说明「还记得」，**不说明「和这句草稿有关」**。所以本模块**没有**
//! 「按 R 把事实库铺进热卡」的入口：一行要进热卡，必须由调用方**显式**给出
//! （本轮检索命中的事实行、或外挂资料行）。没有向量检索时事实行一行都不给，
//! 热卡里只剩 `[资料]` 行、或者干脆是空卡——耐久且 `R >= 0.7` 的事实走
//! 「钉住的事」，不借热卡出场。删掉那条入口而不是留着不用：留着就会有人
//! 再写一行 `from_facts(..)` 把不相关的旧事每轮带进来。
//!
//! # 单行长度上限
//!
//! 每行正文（不含前缀）不超过 [CARD_LINE_MAX_CHARS] = 120 字。因为热卡按
//! **整行收、整行丢**（见 [super::budget::fit_lines]），一行放不下就等于这一行
//! 永远进不了卡；120 字 + `- [清楚] ` 前缀按本模块 token 口径约 123 token，
//! 单独一行必然放得进 200 token 预算。所以兜底 episode 在库里仍存 200 字
//! （[crate::hotmem::Recollection::fallback_episode] 的口径），**渲染进卡前**
//! 截到 120 字：库里的长度是「记得多少」，卡里的长度是「说得下多少」。

use super::budget::{estimate_tokens, fit_lines};

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

/// 热卡单行正文（**不含**前缀）的字符上限。
///
/// 为什么是 120：热卡整行收、整行丢，一行放不下 = 这一行永远进不了卡。
/// 120 个 CJK 字符 + `- [清楚] ` 前缀按本模块 token 口径约 123 token，
/// 单独一行必然能进 200 token 预算（见 [super::render::HOT_CARD_BUDGET_TOKENS]）。
/// 与材料切块上限 [super::material::MATERIAL_CHUNK_CHARS] 取同一个数字，
/// 因为两者解的是同一个问题：**一条候选必须能独自进卡**。
pub const CARD_LINE_MAX_CHARS: usize = 120;

/// 热卡里的一行（**不含**前缀；渲染时才加）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardLine {
    /// 分档。
    pub tier: CardTier,
    /// 正文（单行，无换行）。
    pub text: String,
}

impl CardLine {
    /// 造一行；正文里的换行与前后空白被压成单个空格（热卡是「一行一条」），
    /// 并**截到 [CARD_LINE_MAX_CHARS] 字**。
    ///
    /// 截断发生在这里而不是在各调用点：这是「一行进卡」的唯一入口，于是
    /// 「库里能存 200 字、卡里只说 120 字」这条口径由类型守住——兜底 episode
    /// （[crate::hotmem::Recollection::fallback_episode]，库里 200 字）渲染前
    /// 自动缩到 120 字，不需要谁记得手动截一次。
    pub fn new(tier: CardTier, text: impl Into<String>) -> Self {
        let text = normalize_line(&text.into());
        let text = if text.chars().count() > CARD_LINE_MAX_CHARS {
            text.chars().take(CARD_LINE_MAX_CHARS).collect()
        } else {
            text
        };
        Self { tier, text }
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
