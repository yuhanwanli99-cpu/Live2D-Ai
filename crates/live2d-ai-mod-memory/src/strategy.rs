//! `memory` Mod 的**纯逻辑**：分词 / 重叠打分 / top-k / marker 幂等 / 存储路径解析。
//!
//! 本文件是 [`crate`] 的可单测内核：除 [`unix_now`] 读系统时钟外**不碰 IO**，
//! 也不依赖任何向量 / embedding / 网络设施。IO 在 [`crate::store`]，
//! Mod 生命周期与注入在 [`crate`]。
//!
//! # 文件长度（>500 行，理由）
//!
//! 「一条记录的**形状**」（[`MemoryRecord`] / [`MemoryRole`] / 稳定 id）与
//! 「记录怎么被选出来」（分词 / 打分 / top-k / 摘要按轮保留窗口）是同一件事的两面：
//! 拆开会让「id 为什么长这样、角色前缀从哪来」跨文件追索。Wave 3（2026-09-21）
//! 加入 role 与 [recent_turn_window_start] 后超过 500 行，按 ≤1000 口径豁免，理由在此。
//!
//! # 检索口径（v0，明文钉死）
//!
//! - **中文按字 bigram**：一段连续汉字 `c0 c1 … cn` 产出 `c0c1, c1c2, …`；
//!   单字成段时产出该字本身（否则一个字的查询什么都检索不到）；
//! - **ASCII 按词**：连续 `[A-Za-z0-9_-]` 是一词，小写化，长度 < 2 或命中英文
//!   停用词则丢弃；
//! - **去停用词**：ASCII 停用词整词丢弃；中文停用词只在**单字成段**时丢弃
//!   （bigram 一律保留——否则「天气」这类含常用字的实词会被误杀）；
//! - **打分**：查询词元集合与记录词元集合的 **Jaccard 系数** `|∩| / |∪|`，
//!   天然落在 `[0, 1]`，无 idf、无长度惩罚项（v0 刻意保持可解释）；
//! - **top-k 平局**：分数降序后依次比 `ts` **新→旧**、`turn` **大→小**、
//!   `index` **大→小**（后两者是同一批数据的稳定兜底）。**选定的策略是
//!   「时间新→旧」**，见 [`rank_top_k`] 与单测 `tie_break_prefers_newer`。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// 本地 JSONL 缺省文件名（落在配置文件同目录；见 [`resolve_store_path`]）。
pub const DEFAULT_STORE_FILE: &str = "memory.jsonl";

/// 注入块开始标记（固定常量；幂等剥离靠它）。
pub const MEMORY_MARKER_BEGIN: &str = "<!-- live2d-ai:memory-v0:begin -->";
/// 注入块结束标记。
pub const MEMORY_MARKER_END: &str = "<!-- live2d-ai:memory-v0:end -->";

/// 单条记忆注入进提示词时的最大字符数（超出截断 + 省略号）。
///
/// 上限的意义：注入块总长随 `top_k` 线性增长，而 system_prompt 是每轮都要
/// 发的输入。v0 不做 token 计费，只用字符数这条**可解释**的界。
pub const MAX_MEMORY_LINE_CHARS: usize = 200;

/// `top_k` 缺省与钳位（配置里越界只钳，不报错）。
pub const DEFAULT_TOP_K: usize = 3;
pub const MIN_TOP_K: usize = 1;
pub const MAX_TOP_K: usize = 10;

/// `max_records` 缺省与钳位（**检索窗口**，不是物理删除；见 `crate` 头注）。
pub const DEFAULT_MAX_RECORDS: usize = 200;
pub const MIN_MAX_RECORDS: usize = 1;
pub const MAX_MAX_RECORDS: usize = 10_000;

/// ASCII 停用词（整词丢弃；小写比较）。
const STOPWORDS_ASCII: &[&str] = &[
    "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "am", "do", "does", "did",
    "to", "of", "and", "or", "but", "if", "then", "in", "on", "at", "by", "for", "with", "about",
    "as", "it", "its", "this", "that", "these", "those", "i", "you", "he", "she", "we", "they",
    "me", "him", "her", "us", "them", "my", "your", "his", "our", "their", "not", "no", "yes",
    "so", "very", "just", "can", "could", "would", "should", "will", "shall", "have", "has", "had",
    "there", "here", "what", "which", "who", "how", "why", "when", "where", "please", "ok", "okay",
];

/// 中文停用词（**只在单字成段时**丢弃；bigram 保留，见模块头注）。
const STOPWORDS_CJK: &[char] = &[
    '的', '了', '是', '在', '我', '你', '他', '她', '它', '们', '和', '与', '及', '就', '都', '而',
    '也', '还', '有', '吗', '呢', '吧', '啊', '嗯', '这', '那', '不', '没', '很', '请', '帮', '把',
    '被', '着', '过', '之', '其', '为', '以', '于', '对', '上', '下', '个', '些', '么', '呀', '哦',
    '唉', '喂', '下', '好', '要', '会', '能', '去', '来', '说', '看', '给', '从', '到', '里',
];

/// 一条本地记忆（JSONL 的一行）。
///
/// # 记录 id（L1 波次新增）
///
/// 每条记录有稳定 id `<ts>-<turn>-<fnv1a(text) 8 位十六进制>`（[Self::make_id]）：
/// 新写入时按公式生成并**落盘**；旧行没有 `id` 字段，[crate::store::parse_record_line]
/// 按同一公式**派生**——同一条记录每次载入得到同一个 id（测试
/// `id_derivation_is_stable_across_loads` 钉死）。面板的 update / delete 靠它定位。
///
/// 为什么自带一份 id 而不是只靠行号：JSONL 是追加写 + 超限物理重写，
/// 行号会随淘汰整体左移；而「用户刚在面板上选中的那条」必须在刷新后仍然指得中。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryRecord {
    /// 稳定记录 id（见类型头注；落盘或按公式派生，二者同形）。
    pub id: String,
    /// 原始正文（去首尾空白后的整段；注入时再压平/截断）。
    pub text: String,
    /// Unix 秒（写入时刻）。
    pub ts: i64,
    /// 本 Mod 观察到的第几次 `TurnPrompt`（**不是** supervisor 的 turn id；
    /// `TurnPrompt` 的 payload 只有正文，见 `topics.rs`）。
    ///
    /// **一轮之内用户记录与助手记录共用同一个 turn**（Wave 3，2026-09-21）：
    /// 助手记录在轮末（`AssistantReplied`）追加，此时 `turn_seq` 还没
    /// 递增，因此摘要的「保留最近 K 轮」可以按 turn 分组一并保留 user+assistant。
    pub turn: u64,
    /// 谁说的（Wave 3，2026-09-21）：user = 用户输入，assistant = 助手已上屏正文。
    pub role: MemoryRole,
}

/// 记忆记录的角色（Wave 3，2026-09-21）。
///
/// 落盘为 JSONL 的 "role" 字段；**缺省 = User**——升级前的老行没有该字段，
/// 它们全是用户输入。这条兼容决定了「老记忆注入形态不变」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MemoryRole {
    /// 用户侧（`TurnPrompt` 写入）。
    #[default]
    User,
    /// 助手侧（`AssistantReplied` 轮末写入）。
    Assistant,
}

impl MemoryRole {
    /// 落盘 / 事件里的稳定字符串。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }

    /// 解析落盘 / 事件里的 role 字符串；未知 / 缺失 → User（宽容，不失败）。
    pub fn parse(raw: Option<&str>) -> Self {
        match raw.map(str::trim) {
            Some("assistant") | Some("Assistant") => Self::Assistant,
            _ => Self::User,
        }
    }

    /// 注入 / 摘要行里的角色前缀（**必须区分角色**：模型要知道哪句是自己说的）。
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::User => "[用户] ",
            Self::Assistant => "[助手] ",
        }
    }
}

impl MemoryRecord {
    /// 按 id 公式构造（用户口径；新写入路径统一走它，避免公式散落多处）。
    pub fn new(text: impl Into<String>, ts: i64, turn: u64) -> Self {
        Self::with_role(text, ts, turn, MemoryRole::User)
    }

    /// 助手侧记录（Wave 3）：正文是**已清洗的上屏口径**，turn 与同轮用户记录相同。
    pub fn assistant(text: impl Into<String>, ts: i64, turn: u64) -> Self {
        Self::with_role(text, ts, turn, MemoryRole::Assistant)
    }

    /// 带角色构造（所有路径的唯一入口）。
    pub fn with_role(text: impl Into<String>, ts: i64, turn: u64, role: MemoryRole) -> Self {
        let text = text.into();
        let id = Self::make_id_for(role, ts, turn, &text);
        Self {
            id,
            text,
            ts,
            turn,
            role,
        }
    }

    /// id 公式：`<ts>-<turn>-<fnv1a(text) 8 位十六进制>`（用户口径）。
    ///
    /// 刻意**不引依赖**（5 行 FNV-1a 足够，见 [fnv1a_hex8]）；`turn` 参与公式
    /// 是为了让「同一秒内同一句话的两次不同轮次」拿到不同 id。
    ///
    /// **老行派生公式不变**：升级前没有 id 的行走这条，得到与旧版相同的 id。
    pub fn make_id(ts: i64, turn: u64, text: &str) -> String {
        format!("{ts}-{turn}-{}", fnv1a_hex8(text))
    }

    /// 角色化 id：user 沿用 [Self::make_id]（老行派生不变），assistant 在尾段
    /// 加一个 a 前缀——否则「同一秒、同一轮、用户与助手说了同一句话」会撞 id，
    /// 面板的 delete 会把两条一起删掉。
    pub fn make_id_for(role: MemoryRole, ts: i64, turn: u64, text: &str) -> String {
        match role {
            MemoryRole::User => Self::make_id(ts, turn, text),
            MemoryRole::Assistant => format!("{ts}-{turn}-a{}", fnv1a_hex8(text)),
        }
    }

    /// 注入 / 摘要共用的单行形态：**角色前缀** + 压平 + 截断。
    ///
    /// 角色前缀是「注入格式区分角色」的落点：用户行 `[用户] …`、
    /// 助手行 `[助手] …`，与摘要行 `[摘要] …` 同一套括号风格。
    pub fn line(&self) -> String {
        sanitize_memory_line(&format!("{}{}", self.role.prefix(), self.text))
    }
}

/// FNV-1a 32 位哈希 → 8 位小写十六进制（记录 id 的尾段）。
///
/// 选它的理由：实现 5 行、无依赖、对短文本分布够用；这里**不是**安全哈希，
/// 只是「同一句话稳定映射到同一串」的定位键。
pub fn fnv1a_hex8(text: &str) -> String {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in text.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    format!("{hash:08x}")
}

/// 一次检索命中（`index` 指向传入切片，`score` ∈ `(0, 1]`）。
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub index: usize,
    pub score: f64,
}

/// 是否 CJK（含汉字扩展 / 假名 / 谚文——同一套 bigram 口径对它们都成立）。
fn is_cjk(c: char) -> bool {
    matches!(
        c as u32,
        0x3040..=0x30FF        // 假名
        | 0x3400..=0x4DBF      // 汉字扩展 A
        | 0x4E00..=0x9FFF      // 基本汉字
        | 0xAC00..=0xD7AF      // 谚文音节
        | 0xF900..=0xFAFF      // 兼容汉字
    )
}

fn is_word_byte_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

fn flush_ascii(buf: &mut String, out: &mut Vec<String>) {
    if buf.is_empty() {
        return;
    }
    let word = buf.to_ascii_lowercase();
    buf.clear();
    if word.chars().count() >= 2 && !STOPWORDS_ASCII.contains(&word.as_str()) {
        out.push(word);
    }
}

fn flush_cjk(buf: &mut Vec<char>, out: &mut Vec<String>) {
    match buf.len() {
        0 => {}
        1 => {
            let c = buf[0];
            if !STOPWORDS_CJK.contains(&c) {
                out.push(c.to_string());
            }
        }
        _ => {
            for pair in buf.windows(2) {
                out.push(pair.iter().collect::<String>());
            }
        }
    }
    buf.clear();
}

/// 分词：中文 bigram + ASCII 词，去停用词（口径见模块头注）。
///
/// **保留重复词元**（调用方按集合去重打分，见 [`overlap_score`]），
/// 空输入 / 纯标点 → 空 `Vec`，永不 panic。
pub fn tokenize(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut ascii = String::new();
    let mut han: Vec<char> = Vec::new();
    for c in text.chars() {
        if is_word_byte_char(c) {
            flush_cjk(&mut han, &mut out);
            ascii.push(c);
        } else if is_cjk(c) {
            flush_ascii(&mut ascii, &mut out);
            han.push(c);
        } else {
            flush_ascii(&mut ascii, &mut out);
            flush_cjk(&mut han, &mut out);
        }
    }
    flush_ascii(&mut ascii, &mut out);
    flush_cjk(&mut han, &mut out);
    out
}

/// 词元重叠打分：查询集与记录集的 **Jaccard** 系数，恒在 `[0, 1]`。
///
/// 任一侧为空 → `0.0`（空查询不检索任何东西，空记录不命中）。
pub fn overlap_score(query: &[String], record: &[String]) -> f64 {
    if query.is_empty() || record.is_empty() {
        return 0.0;
    }
    let q: BTreeSet<&str> = query.iter().map(String::as_str).collect();
    let r: BTreeSet<&str> = record.iter().map(String::as_str).collect();
    let inter = q.intersection(&r).count() as f64;
    let union = q.union(&r).count() as f64;
    if union == 0.0 { 0.0 } else { inter / union }
}

/// 按本轮正文检索 top-k，返回按分数降序的命中（**含稳定平局**）。
///
/// 平局裁决顺序（全部降序）：`score` → `ts` → `turn` → `index`。
/// `k == 0` → 空；`k > 命中数` → 全部命中；分数为 `0` 的记录不进结果
/// （「没有相关记忆」与「有一条毫不相关的记忆」必须区分开——前者是 no-op）。
pub fn rank_top_k(query: &str, records: &[MemoryRecord], k: usize) -> Vec<Hit> {
    if k == 0 {
        return Vec::new();
    }
    let query_tokens = tokenize(query);
    if query_tokens.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<Hit> = records
        .iter()
        .enumerate()
        .filter_map(|(index, rec)| {
            let score = overlap_score(&query_tokens, &tokenize(&rec.text));
            (score > 0.0).then_some(Hit { index, score })
        })
        .collect();
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| records[b.index].ts.cmp(&records[a.index].ts))
            .then_with(|| records[b.index].turn.cmp(&records[a.index].turn))
            .then_with(|| b.index.cmp(&a.index))
    });
    hits.truncate(k);
    hits
}

/// 「保留最近 `keep_turns` 轮」的窗口左界（Wave 3，2026-09-21）。
///
/// 「轮」= 记录的 `turn` 字段（同轮的 user + assistant 共用一个值）。从最新
/// 记录往前数，遇到第 `keep_turns + 1` 个**不同 turn** 时，窗口从它**之后**
/// 开始；返回值即「可被摘要的区间右界」。`keep_turns == 0` → `len`（全部可压）。
///
/// 为什么不能再用「最后 K 条」：助手记录入桶后，4 条记录 = 2 轮——那样
/// 「保留最近 4 轮原文」会缩水成 2 轮，最近说过的话被提前压成二手转述。
pub fn recent_turn_window_start(records: &[MemoryRecord], keep_turns: usize) -> usize {
    if keep_turns == 0 {
        return records.len();
    }
    let mut seen: Vec<u64> = Vec::new();
    for (index, record) in records.iter().enumerate().rev() {
        if seen.contains(&record.turn) {
            continue;
        }
        if seen.len() == keep_turns {
            return index + 1;
        }
        seen.push(record.turn);
    }
    0
}

/// 提示词里是否已有本 Mod 的注入块（禁用时据此决定要不要清残留）。
pub fn contains_memory_block(prompt: &str) -> bool {
    prompt.contains(MEMORY_MARKER_BEGIN)
}

/// 剥掉**所有**注入块并去掉尾部空白；没有块时只做尾部规整。
///
/// 容错：有开始标记但缺结束标记（半截块）→ 从开始标记剥到字符串末尾，
/// 而不是原样留下一个会污染后续拼接的残块。幂等：
/// `strip(strip(x)) == strip(x)`。
pub fn strip_memory_block(prompt: &str) -> String {
    let mut out = prompt.to_string();
    while let Some(start) = out.find(MEMORY_MARKER_BEGIN) {
        let end = match out[start..].find(MEMORY_MARKER_END) {
            Some(rel) => start + rel + MEMORY_MARKER_END.len(),
            None => out.len(),
        };
        out.replace_range(start..end, "");
    }
    out.trim_end().to_string()
}

/// 一条记忆压成**单行**并按 [`MAX_MEMORY_LINE_CHARS`] 截断。
///
/// 换行会破坏 `- …` 列表结构（也会让 marker 块在 JSON 里更难读），
/// 因此空白一律折叠成单空格；空行返回空串，由调用方过滤。
pub fn sanitize_memory_line(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.is_empty() {
        return String::new();
    }
    let mut out: String = flat.chars().take(MAX_MEMORY_LINE_CHARS).collect();
    if flat.chars().count() > MAX_MEMORY_LINE_CHARS {
        out.push('…');
    }
    out
}

/// 重拼注入后的 `system_prompt`：**先剥旧块，再拼新块**（幂等）。
///
/// 形态固定为 `<base>\n\n<marker_begin>\n- …\n<marker_end>`；`memories` 全为
/// 空/空白时返回**不带 marker** 的 base（不做「空块」——空块会让下次剥离
/// 看起来「有过注入」，也会让提示词长出一个空标题）。
///
/// 压平/截断后**相同文本只出现一次**（保持先后顺序）：同一句话被记了多次时
/// 在提示词里重复三遍没有信息增益，而且会让「重拼结果是否与当前一致」这条
/// 幂等判据永远为假——每轮都写盘 + reload。
pub fn compose_injection(base: &str, memories: &[String]) -> String {
    let base = strip_memory_block(base);
    let block = block_from_lines(&memory_lines(memories));
    if block.is_empty() {
        return base;
    }
    if base.is_empty() {
        block
    } else {
        format!("{base}\n\n{block}")
    }
}
/// 注入预算（字符数）缺省与钳位。
pub const DEFAULT_INJECTION_BUDGET_CHARS: usize = 1600;
pub const MIN_INJECTION_BUDGET_CHARS: usize = 200;
pub const MAX_INJECTION_BUDGET_CHARS: usize = 20_000;

/// 注入块占用预算 **>该比例 → 按分数裁 top-k**（缺省 0.60）。
pub const DEFAULT_TRIM_RATIO: f64 = 0.60;

/// 注入块占用预算 **>该比例 → 标记待摘要**（缺省 0.75）。
pub const DEFAULT_SUMMARY_RATIO: f64 = 0.75;

/// 摘要时**保留最近几轮原文**的缺省值（Wave 3 起按 `turn` 分组计数，
/// 见 [recent_turn_window_start]）。
pub const SUMMARY_KEEP_RECENT_TURNS: usize = 4;

/// 一次预算裁剪的结果（纯数据，便于单测与 state_json）。
#[derive(Debug, Clone, PartialEq)]
pub struct BudgetedInjection {
    /// 保留的记忆行（已 sanitize / 去重；顺序 = 输入顺序 = 分数降序）。
    pub lines: Vec<String>,
    /// 被预算裁掉的条数（不含 sanitize 丢空 / 去重）。
    pub dropped: usize,
    /// 注入块字符数（含 marker；0 = 无块）。
    pub used_chars: usize,
    /// 预算字符数（回显，便于 state_json）。
    pub budget_chars: usize,
    /// used_chars / budget_chars（budget=0 -> 1.0）。
    pub ratio: f64,
    /// 是否发生了 top-k 裁剪。
    pub topk_trimmed: bool,
    /// 是否越过摘要阈值（调用方据此标记待摘要 + 冷却）。
    pub needs_summary: bool,
}

impl BudgetedInjection {
    /// 拼成最终注入块（无内容 -> 空串）。
    pub fn block(&self) -> String {
        block_from_lines(&self.lines)
    }
}

/// sanitize + 去重 + 丢空后的记忆行（[`compose_injection`] 与预算裁剪共用）。
fn memory_lines(memories: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    memories
        .iter()
        .map(|m| sanitize_memory_line(m))
        .filter(|l| !l.is_empty())
        .filter(|l| seen.insert(l.clone()))
        .collect()
}

/// 由记忆行拼出注入块（**不含 base**；空 -> 空串）。
fn block_from_lines(lines: &[String]) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let body = lines
        .iter()
        .map(|l| format!("- {l}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{MEMORY_MARKER_BEGIN}\n{body}\n{MEMORY_MARKER_END}")
}

/// 按**注入预算**裁剪记忆块：平时全量；> `trim_ratio` 起从**最低分**（列表尾）逐条裁。
///
/// 口径（PLAN §0 点 3）：
/// - 条数不是预算，**字符数**才是；单条上限由 [`sanitize_memory_line`] 兜底；
/// - 裁到只剩 1 条仍超预算 -> 不再裁（至少留一条）；`ratio` 会 >1、`needs_summary` 为真；
/// - `needs_summary` 在 `ratio > summary_ratio` 时为真（调用方标记 + 冷却）。
///
/// 纯函数：不 IO、不依赖 runtime。
pub fn budget_injection(
    memories: &[String],
    budget_chars: usize,
    trim_ratio: f64,
    summary_ratio: f64,
) -> BudgetedInjection {
    let mut lines = memory_lines(memories);
    let before = lines.len();
    let limit = (budget_chars as f64) * trim_ratio.clamp(0.0, 1.0);
    while lines.len() > 1 && (block_from_lines(&lines).chars().count() as f64) > limit {
        lines.pop();
    }
    let block = block_from_lines(&lines);
    let used_chars = block.chars().count();
    let ratio = if budget_chars == 0 {
        1.0
    } else {
        used_chars as f64 / budget_chars as f64
    };
    let dropped = before - lines.len();
    BudgetedInjection {
        lines,
        dropped,
        used_chars,
        budget_chars,
        ratio,
        topk_trimmed: dropped > 0,
        needs_summary: ratio > summary_ratio,
    }
}

/// 解析 `store_path` / `config_path` → 实际 JSONL 路径。
///
/// 规则（写清并测死）：
/// 1. `store_path` 非空 → 用它；**相对路径**在知道配置文件时相对配置文件所在
///    目录解析（否则相对 cwd），避免「cwd 不同 → 记忆写去别处」；
/// 2. `store_path` 空 → 配置文件同目录的 [`DEFAULT_STORE_FILE`]；
/// 3. `store_path` 空且 `config_path` 也空（无 host 注入的测试环境）→ `None`，
///    调用方必须 **no-op + warn**，不许 panic、不许瞎猜 cwd。
pub fn resolve_store_path(config_path: &str, store_path: &str) -> Option<PathBuf> {
    let config_dir = {
        let cp = config_path.trim();
        if cp.is_empty() {
            None
        } else {
            Path::new(cp)
                .parent()
                .map(Path::to_path_buf)
                .filter(|d| !d.as_os_str().is_empty())
        }
    };
    let explicit = store_path.trim();
    if !explicit.is_empty() {
        let path = PathBuf::from(explicit);
        if path.is_absolute() {
            return Some(path);
        }
        return Some(match config_dir {
            Some(dir) => dir.join(path),
            None => path,
        });
    }
    config_dir.map(|dir| dir.join(DEFAULT_STORE_FILE))
}

/// 会话桶目录名（与老路径同一个父目录）。
pub const SESSIONS_DIR: &str = "sessions";

/// 会话桶文件路径：`<base 同目录>/sessions/<session>.memory.jsonl`。
///
/// # 为什么不是 `<config 目录>/sessions`
///
/// 用户可以把 `store_path` 指到别处；会话桶应当**跟着那个库走**，所以这里从
/// **已解析出的老路径** `base` 取父目录（默认 store_path 空时父目录就是配置
/// 目录，与产品口径逐字一致）。
///
/// # 为什么返回 `Option`
///
/// 会话 id 要进**文件名**，必须过 [live2d_ai_mod_system::sanitize_session_id]
/// 这道路径穿越闸（`a/b` / `..` / 控制字符一律拒）。非法 id → `None`：
/// 调用方必须报可读错误或按「无会话」处理，**绝不**拿用户输入直接拼路径。
pub fn resolve_session_store_path(base: &Path, session: &str) -> Option<PathBuf> {
    let id = live2d_ai_mod_system::sanitize_session_id(session)?;
    let dir = base.parent().unwrap_or_else(|| Path::new(""));
    Some(dir.join(SESSIONS_DIR).join(format!("{id}.memory.jsonl")))
}

/// 当前 Unix 秒（时钟不可用/回退 → `0`，永不 panic）。
pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
