//! `memory` Mod 的**纯逻辑**：分词 / 重叠打分 / top-k / marker 幂等 / 存储路径解析。
//!
//! 本文件是 [`crate`] 的可单测内核：除 [`unix_now`] 读系统时钟外**不碰 IO**，
//! 也不依赖任何向量 / embedding / 网络设施。IO 在 [`crate::store`]，
//! Mod 生命周期与注入在 [`crate`]。
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryRecord {
    /// 原始正文（去首尾空白后的整段；注入时再压平/截断）。
    pub text: String,
    /// Unix 秒（写入时刻）。
    pub ts: i64,
    /// 本 Mod 观察到的第几次 `TurnPrompt`（**不是** supervisor 的 turn id；
    /// `TurnPrompt` 的 payload 只有正文，见 `topics.rs`）。
    pub turn: u64,
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
    let mut seen = BTreeSet::new();
    let lines: Vec<String> = memories
        .iter()
        .map(|m| sanitize_memory_line(m))
        .filter(|l| !l.is_empty())
        .filter(|l| seen.insert(l.clone()))
        .collect();
    if lines.is_empty() {
        return base;
    }
    let body = lines
        .iter()
        .map(|l| format!("- {l}"))
        .collect::<Vec<_>>()
        .join("\n");
    let block = format!("{MEMORY_MARKER_BEGIN}\n{body}\n{MEMORY_MARKER_END}");
    if base.is_empty() {
        block
    } else {
        format!("{base}\n\n{block}")
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

/// 当前 Unix 秒（时钟不可用/回退 → `0`，永不 panic）。
pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
