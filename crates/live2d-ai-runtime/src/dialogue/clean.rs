//! **送 TTS / 上屏前的确定性清洗**（纯函数，2026-09-21）。
//!
//! # 它解决什么
//!
//! 模型经常把「动作描写」写进正文：你好呀（挥手）。、*轻轻歪头* 是这样吗？、
//! **重点**在第三点。这些内容一旦原样交给 TTS 就会被**念出来**
//! （「左括号挥手右括号」），上屏也难看。本模块把它剥掉。
//!
//! # 确定性（硬要求）
//!
//! - **纯函数**：无 IO、无时钟、无随机、无全局状态；同一输入恒等输出；
//! - **不写 TTS**：它只**清洗**，不改写说辞、不缩写、不润色、不翻译；
//! - **不重排句边界**：本函数**不新增也不删除** 。！？….!? 换行 这类句读——
//!   调用点因此必须是「**切句之后**」（见 engine.rs：句子已经由
//!   crate::dialogue::SentenceAssembler 定好，清洗只作用在句内）。
//!   正文兜底（整段）是唯一的段级调用，它只影响显示，不影响 TTS 切句；
//! - **宽容**：孤立 / 不配对的括号、星号原样保留（宁可不剥也不吞掉后面半句
//!   话）；永不 panic。
//!
//! # 剥什么（写死，可测）
//!
//! | 形态 | 处理 | 例 |
//! | --- | --- | --- |
//! | 成对全角括号 | 连同内容删除 | 你好（挥手）。 → 你好。 |
//! | 成对半角括号 | 连同内容删除（支持同类嵌套） | 好(笑)呀 → 好呀 |
//! | 双星号成对 | 只拆标记、**保留正文**（Markdown 粗体） | **重点** → 重点 |
//! | 成对单星号 | 连同内容删除（舞台指示） | *歪头* 是吗 → 是吗 |
//! | 反引号 | 只拆标记、保留正文 | 看 反引号 code 反引号 → 看 code |
//! | 行首 # 标题 / > 引用 | 只拆标记 | # 标题 → 标题 |
//! | Markdown 链接 [文字](url) | 只留文字 | [官网](http://x) → 官网 |
//! | 连续空格 / 制表 / 全角空格 | 折叠成一个半角空格并去首尾 | 好  呀 → 好 呀 |
//! | 全角句读后的空格 | 直接丢弃（中文排版不空格；ASCII 句读后的空格保留） | 你好。 再见。 → 你好。再见。 |
//!
//! **不剥**：行首 - / + 列表符（与负号、正文歧义）、下划线强调
//! （与 snake_case 歧义）、【】（产品文案里可能是正文）。
//!
//! # 与「一句一单元」的关系
//!
//! 清洗后为空串 = 这一句**没有可说的内容**（例如整句就是（笑））。
//! 它照常占一个句子序号、照常走既有的**静音句**路径（worker 见 trim 为空即
//! 不发 TTS HTTP），因此**空串永远到不了 TTS 上游**——上游对空 input 回 400，
//! 而 TTS 错误是 fatal，会把整轮判失败。

/// 清洗一段**已经切好句**（或整段兜底）的正文。
///
/// 见模块头注的「剥什么」表；幂等（clean(clean(x)) == clean(x)）。
pub fn clean_for_tts(raw: &str) -> String {
    let text = strip_markdown_links(raw);
    let text = strip_code_markers(&text);
    let text = strip_line_markers(&text);
    let text = unwrap_marker_pairs(&text, "**");
    let text = remove_star_stage_directions(&text);
    let text = remove_paired_spans(&text, '（', '）');
    let text = remove_paired_spans(&text, '(', ')');
    collapse_spaces(&text)
}

/// 删除 open..close 成对区间（含两端），支持**同类嵌套**。
///
/// 不配对的孤立开括号原样保留（不吞后文）——「宁可不剥也不改语义」。
fn remove_paired_spans(input: &str, open: char, close: char) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == open {
            let mut depth = 0usize;
            let mut j = i;
            let mut matched = None;
            while j < chars.len() {
                if chars[j] == open {
                    depth += 1;
                } else if chars[j] == close {
                    depth -= 1;
                    if depth == 0 {
                        matched = Some(j);
                        break;
                    }
                }
                j += 1;
            }
            match matched {
                Some(end) => i = end + 1,
                None => {
                    out.push(chars[i]);
                    i += 1;
                }
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// 成对单星号：连同内容删除（舞台指示）。
///
/// 必须在 unwrap_marker_pairs 处理完双星号**之后**调用，否则 **粗体** 会被
/// 当成两段空舞台指示拆坏。落单的星号原样保留。
fn remove_star_stage_directions(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '*' {
            let mut j = i + 1;
            let mut matched = None;
            while j < chars.len() {
                if chars[j] == '*' {
                    matched = Some(j);
                    break;
                }
                j += 1;
            }
            if let Some(end) = matched {
                i = end + 1;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// 成对标记 marker … marker：**只拆标记、保留中间正文**（粗体 / 删除线）。
///
/// 落单的标记原样保留；多个成对区间逐个处理（不跨区间配对）。
fn unwrap_marker_pairs(input: &str, marker: &str) -> String {
    if marker.is_empty() {
        return input.to_string();
    }
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find(marker) {
        let after_open = &rest[start + marker.len()..];
        match after_open.find(marker) {
            Some(rel) => {
                out.push_str(&rest[..start]);
                out.push_str(&after_open[..rel]);
                rest = &after_open[rel + marker.len()..];
            }
            None => {
                out.push_str(rest);
                return out;
            }
        }
    }
    out.push_str(rest);
    out
}

/// 去掉反引号（inline / fenced code 的标记），**保留其中的文字**。
fn strip_code_markers(input: &str) -> String {
    input.chars().filter(|c| *c != '\u{60}').collect()
}

/// 行首 Markdown 标记：# 标题、> 引用（只拆标记，保留正文）。
fn strip_line_markers(input: &str) -> String {
    let mut out: Vec<String> = Vec::with_capacity(input.lines().count().max(1));
    for line in input.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix('>') {
            out.push(rest.trim_start().to_string());
        } else if trimmed.starts_with('#') {
            out.push(trimmed.trim_start_matches('#').trim_start().to_string());
        } else {
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

/// Markdown 链接 [文字](url) → 文字。
///
/// 只有「方括号紧跟圆括号」才动手——孤立的 [笑] 一律保留（本波只剥
/// 全/半角括号与成对星号两类动作描写，方括号不在契约里）。
fn strip_markdown_links(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' {
            let mut close_bracket = None;
            let mut j = i + 1;
            while j < chars.len() {
                if chars[j] == ']' {
                    close_bracket = Some(j);
                    break;
                }
                j += 1;
            }
            if let Some(cb) = close_bracket
                && chars.get(cb + 1) == Some(&'(')
            {
                let mut depth = 0usize;
                let mut k = cb + 1;
                let mut close_paren = None;
                while k < chars.len() {
                    if chars[k] == '(' {
                        depth += 1;
                    } else if chars[k] == ')' {
                        depth -= 1;
                        if depth == 0 {
                            close_paren = Some(k);
                            break;
                        }
                    }
                    k += 1;
                }
                if let Some(cp) = close_paren {
                    out.extend(chars[i + 1..cb].iter());
                    i = cp + 1;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// 折叠空白：连续半角空格 / 制表 / 全角空格 → 一个半角空格；换行保留；去首尾。
///
/// **全角句读后的空格直接丢弃**（中文排版不在句读后留空格）：剥掉一个动作描写
/// 之后常会留下 句读+空格 这种空隙——保留它会让「整段清洗」与「逐句清洗再
/// 拼接」得到不同文本（你好（挥手）。*歪头* 再见。 整段洗会变成 你好。 再见。
/// 而逐句是 你好。再见。）。记忆侧拿到的是**整段清洗**产物，必须与上屏口径一致。
///
/// 只对**全角**句读（。！？…）生效：ASCII . ? ! 后面的空格是英文的正常分句，
/// 丢掉会把 Hello. World. 粘成 Hello.World.。
fn collapse_spaces(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut pending_space = false;
    for c in input.chars() {
        if c == ' ' || c == '\t' || c == '\u{3000}' {
            pending_space = true;
            continue;
        }
        if pending_space {
            let after_cjk_terminator = out
                .chars()
                .last()
                .is_some_and(|p| matches!(p, '。' | '！' | '？' | '…'));
            if c != '\n' && !out.is_empty() && !out.ends_with('\n') && !after_cjk_terminator {
                out.push(' ');
            }
            pending_space = false;
        }
        out.push(c);
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 表驱动：每条都是「输入 → 期望」；加条例只改这张表。
    #[test]
    fn table_driven_cleaning() {
        let cases: &[(&str, &str)] = &[
            // ---- 全角 / 半角括号动作描写 ----
            ("你好呀（挥手）。", "你好呀。"),
            ("好(笑)呀", "好呀"),
            ("(叹气) 我知道了。", "我知道了。"),
            ("他（小声说（补一句））走了。", "他走了。"),
            (
                "没有配对的开括号（后面还有话",
                "没有配对的开括号（后面还有话",
            ),
            (
                "没有配对的闭括号）后面还有话",
                "没有配对的闭括号）后面还有话",
            ),
            // ---- 成对星号舞台指示 ----
            ("*轻轻歪头* 是这样吗？", "是这样吗？"),
            ("嗨 *笑* 你好", "嗨 你好"),
            ("3*4=12", "3*4=12"),
            // ---- Markdown：粗体拆标记保正文 ----
            ("**重点**在第三点。", "重点在第三点。"),
            ("**a** 和 **b**", "a 和 b"),
            // ---- 反引号 / 标题 / 引用 / 链接 ----
            ("看 \u{60}code\u{60} 这段。", "看 code 这段。"),
            ("# 标题\n正文", "标题\n正文"),
            ("> 引用一句", "引用一句"),
            ("见 [官网](http://example.com)。", "见 官网。"),
            // ---- 空白折叠 ----
            ("好  呀", "好 呀"),
            // 剥掉句间动作描写后的空隙：整段清洗 == 逐句清洗再拼接。
            ("你好呀（挥手）。*歪头* 再见。", "你好呀。再见。"),
            // ASCII 句读后的空格是正文，不得丢。
            ("Hello. World.", "Hello. World."),
            ("  两端空白  ", "两端空白"),
            // ---- 句读一个不增不减（不重排句边界）----
            ("你好（笑）！今天真好？", "你好！今天真好？"),
            // ---- 空 / 全剥光 ----
            ("（笑）", ""),
            ("*歪头*", ""),
            ("", ""),
            ("   ", ""),
        ];
        for (input, expected) in cases {
            assert_eq!(clean_for_tts(input), *expected, "clean_for_tts({input:?})");
        }
    }

    /// 句读集合**逐字不变**：清洗只删「标记 / 括号内容」，绝不改标点种类与顺序。
    #[test]
    fn sentence_terminators_are_never_added_or_removed() {
        let input = "一。二！三？四…五.六!七?";
        assert_eq!(clean_for_tts(input), input, "无标记时逐字不变");
        assert_eq!(clean_for_tts("一（x）。二*笑*！三"), "一。二！三");
    }

    /// 幂等：再洗一遍结果不变（调用点可能在兜底路径二次调用）。
    #[test]
    fn cleaning_is_idempotent() {
        let input = "你好（笑）呀 *歪头* **重点** \u{60}code\u{60} [x](y)";
        let once = clean_for_tts(input);
        assert_eq!(clean_for_tts(&once), once);
        for s in ["（笑）", "没有标记的普通句子。"] {
            let once = clean_for_tts(s);
            assert_eq!(clean_for_tts(&once), once, "二次清洗改变了 {s:?}");
        }
    }

    /// 落单标记原样保留（宁可不剥也不吞后文）。
    #[test]
    fn dangling_markers_survive() {
        assert_eq!(clean_for_tts("半句（没有收尾"), "半句（没有收尾");
        assert_eq!(clean_for_tts("半句没有收尾)"), "半句没有收尾)");
        assert_eq!(clean_for_tts("孤立的 * 星号"), "孤立的 * 星号");
        assert_eq!(clean_for_tts("孤立的 [笑] 方括号"), "孤立的 [笑] 方括号");
    }
}
