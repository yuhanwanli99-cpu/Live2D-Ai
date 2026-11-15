//! **主链热槽记忆**（2026-11-16）——纯函数层：遗忘曲线、热卡、钉住块、四块拼接。
//!
//! # 它替换了什么
//!
//! 退休的是两条**产品路径**（不是导演，也不是 external-input / voice-input /
//! local-tts / local-engine / a2a）：
//!
//! | 退休前 | 之后 |
//! | --- | --- |
//! | `persona` Mod 把酒馆卡合成系统提示词 | 说话模型只看见「系统句 + 钉住的事 + 最近对话 + 热卡」 |
//! | `memory` Mod 用词重叠挑句子写进同一段系统提示词 | 事实库默认不进提示词；要进只能进热卡 |
//! | 会话槽「匿名 → persona → memory」叠成一段 | 说话用的系统文本不再读这两个 owner |
//!
//! `mods.json` 里若还留着这两项的 `enabled: true`，宿主**忽略**它们对系统提示词
//! 的写入（见 `crate::supervisor` 侧的组装函数）；旧 `memory.jsonl` 一概不读，
//! 也不删。
//!
//! # 这个模块的边界
//!
//! **无 IO、无 LLM、无时钟**：所有函数都是「给定输入 → 给定输出」的纯函数，
//! 时间以「距今多少小时」的形式**由调用方传入**。这样遗忘曲线、热卡预算、
//! 钉住块裁剪全部可以在单测里逐字钉住，不需要 mock 时钟、不需要临时目录。
//!
//! # 四块预算（说话请求只带这四块）
//!
//! | 块 | 预算 | 来源 |
//! | --- | --- | --- |
//! | 系统句 | 出厂约 80 字，用户可改 | `[persona].system_prompt` |
//! | 钉住的事 | 400 token | 耐久事实里 `R >= 0.7` 的少数几条 |
//! | 最近对话 | `max_history_pairs` 原文 + 一条 ≤300 字滚动摘要 | 会话历史 |
//! | 热卡 | 200 token | 本轮草稿检索的结果 |
//!
//! 热卡**整块替换**，禁止把检索结果往旧热卡后面追加——[HotCard] 只有整体构造
//! 入口、没有 `push`，形状本身就守住了这条。
//!
//! # token 口径（**钉死**，实现与测试用同一条）
//!
//! 本项目不引 tokenizer。预算按字符近似：**1 token = 1 个 CJK 字符 = 1 个
//! ASCII 词**（ASCII 字母数字连写的算一个词，空白与其余标点各算一个）。
//! [estimate_tokens] 是唯一实现，裁剪一律走它。

mod budget;
mod card;
mod fact;
mod pin;
mod render;
mod store;
#[cfg(test)]
mod tests;

pub use budget::{estimate_tokens, fit_lines, trim_to_tokens};
pub use card::{CardLine, CardTier, HotCard, render_hot_card};
pub use fact::{FactKind, HotFact, retention, strengthen};
pub use pin::{PINNED_BUDGET_TOKENS, PinnedItem, pinned_tokens, render_pinned};
pub use render::{
    HOT_CARD_BUDGET_TOKENS, HOT_CARD_HEADING, PINNED_HEADING, PromptBudget, SYSTEM_PROMPT_FACTORY,
    compose_system_text,
};
pub use store::{HotMemory, HotMemorySnapshot, NO_SESSION, Recollection, StoredFact};

/// 近期折摘要的字符上限（替换上一条摘要，不无限加长）。
pub const SUMMARY_MAX_CHARS: usize = 300;

/// 「清楚」档的下界（`R >= 0.7`）。
pub const TIER_CLEAR_MIN_R: f64 = 0.7;

/// 「模糊」档的下界（`0.2 <= R < 0.7`）；`R < 0.2` 不进热卡。
pub const TIER_VAGUE_MIN_R: f64 = 0.2;
