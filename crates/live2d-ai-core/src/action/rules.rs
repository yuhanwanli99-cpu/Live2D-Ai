//! 确定性文本规则 fallback（从调度器文件拆出）。
//!
//! 从一条用户文本推断最小语义动作；source 恒为 [`ActionSource::RuleFallback`]。
//! 规则按序判定，全部确定：
//! 1. **硬抑制**：文本含「别 / 不要 / 为什么 / 怎么」→ 返回 `None`（保守不触发，
//!    复杂句子交由 LLM 主路径决定，避免 `?/!`/肯定词误触发）；
//! 2. **显式否定**：中文否定短语（不是/不行/没有/不对）或英文否定词 → `ShakeNo`(Low)；
//! 3. **肯定词**（需未被否定保护压制）→ `Nod`(Low)；
//! 4. **标点兜底**：`！` → `Surprise`(High)；`？` → `Listen`(Medium)；`？！` 并存时 `！` 优先；
//! 5. 其余（空文本/噪声）→ `None`。
//!
//! 全部候选都经能力集合过滤；`tilt / look_around` 不走规则层（保留给 LLM tool
//! 与用户命令）。
//!
//! 英文分词注意：撇号（`'` / `’`）并入词内并剔除，使 "can't / don't / won't"
//! 归一化为 cant / dont / wont 参与否定保护判定。此前撇号被当作分隔符，
//! 这些词会被切成 don/t 两段，导致否定保护失效（"I don't agree" 会误点头）。

use super::{ActionId, ActionSource, ModelCapabilities, SemanticAction, Strength};

/// 中文肯定（点头）最小关键词。
const NOD_ZH: &[&str] = &["嗯", "好的", "是的", "没错", "同意", "可以"];
/// 中文否定（摇头）最小关键词：显式否定短语（不收录裸「不/没/别」，避免子串误触发）。
const SHAKE_ZH: &[&str] = &["不是", "不行", "没有", "不对"];
/// 英文肯定（点头）最小关键词（整词匹配，小写）。
const NOD_EN: &[&str] = &[
    "yes",
    "yeah",
    "yea",
    "yep",
    "ok",
    "okay",
    "sure",
    "right",
    "agree",
    "exactly",
    "absolutely",
];
/// 英文否定（摇头）最小关键词（整词匹配，小写）。
const SHAKE_EN: &[&str] = &[
    "no",
    "nope",
    "not",
    "never",
    "wrong",
    "disagree",
    "incorrect",
];
/// 英文否定保护词：出现任一即抑制「点头」等肯定动作。
/// `dont/cant/wont/cannot` 为撇号归一化后的形态（don't → dont）。
const NEGATION_EN: &[&str] = &[
    "no", "nope", "not", "never", "none", "nothing", "nowhere", "dont", "cant", "cannot", "wont",
];
/// 中文硬抑制标记：「别」可作「别人/特别」的子串、「不要」是祈使否定、「为什么/怎么」
/// 是复杂疑问——这些句子交由 LLM 主路径决定，规则层保守不动作。
const HARD_SUPPRESS_ZH: &[&str] = &["别", "不要", "为什么", "怎么"];

/// 把文本切成小写英文单词：ASCII 字母数字成词；撇号并入词内并剔除
/// （`Don't` → `dont`），中文等其余字符视为分隔符。
fn english_tokens(text: &str) -> Vec<String> {
    const APOSTROPHES: [char; 2] = ['\'', '’'];
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            current.push(ch.to_ascii_lowercase());
        } else if APOSTROPHES.contains(&ch) && !current.is_empty() {
            // 词中撇号直接剔除（归一化 don't → dont）；孤立撇号按分隔符处理。
            continue;
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// 确定性规则 fallback：从一条用户文本推断最小语义动作（见模块文档的规则顺序）。
pub fn rule_fallback(text: &str, capabilities: &ModelCapabilities) -> Option<SemanticAction> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }

    // 1. 硬抑制：危险/复杂标记 → 不触发任何规则动作。
    if HARD_SUPPRESS_ZH.iter().any(|marker| t.contains(marker)) {
        return None;
    }

    let tokens = english_tokens(t);
    let has_zh_negation =
        (t.contains('不') && !t.contains("不错")) || (t.contains('没') && !t.contains("没错"));
    let is_negated = has_zh_negation
        || tokens
            .iter()
            .any(|word| NEGATION_EN.contains(&word.as_str()));

    // 2. 显式否定 → 摇头（优先于一切肯定/标点兜底）。
    if capabilities.supports(ActionId::ShakeNo)
        && (SHAKE_ZH.iter().any(|marker| t.contains(marker))
            || tokens.iter().any(|word| SHAKE_EN.contains(&word.as_str())))
    {
        return Some(SemanticAction::new(
            ActionId::ShakeNo,
            Strength::Low,
            ActionSource::RuleFallback,
        ));
    }

    // 3. 肯定 → 点头（被否定保护压制的句子不触发）。
    let zh_nod = NOD_ZH.iter().any(|marker| t.contains(marker));
    let en_nod = tokens.iter().any(|word| NOD_EN.contains(&word.as_str()));
    if !is_negated && capabilities.supports(ActionId::Nod) && (zh_nod || en_nod) {
        return Some(SemanticAction::new(
            ActionId::Nod,
            Strength::Low,
            ActionSource::RuleFallback,
        ));
    }

    // 4a. 惊叹 → 惊讶（强度最高）。
    if capabilities.supports(ActionId::Surprise) && (t.contains('！') || t.contains('!')) {
        return Some(SemanticAction::new(
            ActionId::Surprise,
            Strength::High,
            ActionSource::RuleFallback,
        ));
    }

    // 4b. 提问 → 倾听（强度中）。
    if capabilities.supports(ActionId::Listen) && (t.contains('？') || t.contains('?')) {
        return Some(SemanticAction::new(
            ActionId::Listen,
            Strength::Medium,
            ActionSource::RuleFallback,
        ));
    }

    None
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use ActionSource::RuleFallback;

    /// 表驱动用的 const 构造助手。
    const fn act(action: ActionId, strength: Strength) -> Option<SemanticAction> {
        Some(SemanticAction::new(action, strength, RuleFallback))
    }
    const NONE: Option<SemanticAction> = None;

    /// 表驱动用例：text → 期望结果（全能力集）。覆盖每条规则的命中与压制路径。
    const CASES: &[(&str, Option<SemanticAction>)] = &[
        // —— 空 / 噪声 → None
        ("", NONE),
        ("   ", NONE),
        ("，。", NONE),
        ("123456", NONE),
        // —— 硬抑制：复杂/危险标记整体不触发（含标点兜底也不给）
        ("别这样", NONE),
        ("别碰我！", NONE),
        ("不要！", NONE),
        ("不要这样", NONE),
        ("为什么？", NONE),
        ("怎么这样！", NONE),
        ("你怎么看", NONE),
        // —— 中文显式否定 → ShakeNo(Low)
        ("不是", act(ActionId::ShakeNo, Strength::Low)),
        ("不行", act(ActionId::ShakeNo, Strength::Low)),
        ("没有", act(ActionId::ShakeNo, Strength::Low)),
        ("不对", act(ActionId::ShakeNo, Strength::Low)),
        // —— 中文否定保护：「不用了」压制点头；「不错/没错」例外放行
        ("嗯，不用了", NONE),
        ("好的，但是不用了", NONE),
        ("嗯，不错", act(ActionId::Nod, Strength::Low)),
        ("没错", act(ActionId::Nod, Strength::Low)),
        // —— 中文肯定 → Nod(Low)
        ("嗯", act(ActionId::Nod, Strength::Low)),
        ("好的", act(ActionId::Nod, Strength::Low)),
        ("同意", act(ActionId::Nod, Strength::Low)),
        ("嗯嗯嗯嗯嗯", act(ActionId::Nod, Strength::Low)),
        // —— 英文显式否定 → ShakeNo(Low)
        ("No", act(ActionId::ShakeNo, Strength::Low)),
        ("not", act(ActionId::ShakeNo, Strength::Low)),
        ("never", act(ActionId::ShakeNo, Strength::Low)),
        ("nope", act(ActionId::ShakeNo, Strength::Low)),
        ("not sure", act(ActionId::ShakeNo, Strength::Low)),
        (
            "Yeah, but not really",
            act(ActionId::ShakeNo, Strength::Low),
        ),
        // —— 撇号收缩：归一化后命中否定保护（修复前 don't 被切碎、保护失效）
        ("I don't agree", NONE), // 点头被压制，且无显式否定词 → 不动作
        ("I can't do that", NONE),
        ("They won't come?", act(ActionId::Listen, Strength::Medium)), // 压制点头后问号兜底
        ("Yes, I'm sure", act(ActionId::Nod, Strength::Low)),          // 词内撇号不影响整词匹配
        ("Cannot say", NONE), // cannot 只做否定保护，不触发摇头（保守不动作）
        // —— 关键词优先于标点："Yes!" 是点头而非惊讶
        ("Yes!", act(ActionId::Nod, Strength::Low)),
        ("sure", act(ActionId::Nod, Strength::Low)),
        // —— 标点兜底
        ("你吃饭了吗？", act(ActionId::Listen, Strength::Medium)),
        ("Really?", act(ActionId::Listen, Strength::Medium)),
        ("太棒了！", act(ActionId::Surprise, Strength::High)),
        ("Wow!", act(ActionId::Surprise, Strength::High)),
        ("真的吗？！", act(ActionId::Surprise, Strength::High)), // ？+！并存：！优先
    ];

    #[test]
    fn rule_table_is_deterministic() {
        let caps = ModelCapabilities::all();
        assert!(!CASES.is_empty());
        for (text, expected) in CASES {
            assert_eq!(&rule_fallback(text, &caps), expected, "text: {text:?}");
        }
    }

    #[test]
    fn contractions_normalize_without_apostrophes() {
        let join = |text: &str| -> String { english_tokens(text).join(" ") };
        assert_eq!(join("Don't stop"), "dont stop");
        assert_eq!(join("can’t"), "cant");
        assert_eq!(join("won't"), "wont");
        assert_eq!(join("It's O'Brien's"), "its obriens");
        // 孤立撇号是分隔符，不产生空 token。
        assert_eq!(join("'tis '"), "tis");
    }

    #[test]
    fn rule_fallback_respects_capabilities() {
        // 模型不支持惊讶：即使文本带「！」也不产出 surprise。
        let no_surprise = ModelCapabilities::EMPTY
            .with(ActionId::Nod)
            .with(ActionId::Listen);
        assert_eq!(rule_fallback("太棒了！", &no_surprise), None);
        // 不支持倾听：带「？」也无动作。
        let no_listen = ModelCapabilities::EMPTY
            .with(ActionId::Nod)
            .with(ActionId::Surprise);
        assert_eq!(rule_fallback("你吃饭了吗？", &no_listen), None);
        // 具备能力时才产出对应动作。
        assert_eq!(
            rule_fallback("嗯", &no_surprise),
            act(ActionId::Nod, Strength::Low)
        );
        // 完全 fail-closed 的模型：任何文本都不产出。
        assert_eq!(rule_fallback("yes!", &ModelCapabilities::EMPTY), None);
    }
}
