//! token 口径与预算裁剪（**唯一实现**）。
//!
//! # 为什么按字符近似
//!
//! 本项目不引 tokenizer（那是又一个 crate、又一份许可、又一处版本漂移）。
//! 预算只需要**单调、稳定、可在测试里逐字复现**，不需要与厂商分词的误差最小。
//!
//! 口径（钉死）：**1 token = 1 个非 ASCII 字符 = 1 个 ASCII 词**。
//! ASCII 词 = `[A-Za-z0-9_'-]` 的连写；空白与其余 ASCII 标点**不计**。
//! 于是「你好」= 2 token、`hello world` = 2 token、`你好吗，hello` = 4 token。
//!
//! 全项目只有这里一个 [estimate_tokens]；热卡与钉住块的裁剪都走它，测试也按
//! 同一条口径断言（**实现换口径 = 测试跟着换**，不允许两套）。

/// 是否属于 ASCII 词（连写的字母数字 / 下划线 / 撇号 / 连字符算一个词）。
const fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '\'' || c == '-'
}

/// 估算 token 数（口径见模块头注）。
///
/// 只依赖字符分类，**不做任何 IO / 分配**。
pub fn estimate_tokens(text: &str) -> usize {
    let mut tokens = 0usize;
    let mut in_word = false;
    for c in text.chars() {
        if is_word_char(c) {
            if !in_word {
                tokens += 1;
                in_word = true;
            }
        } else {
            in_word = false;
            if !c.is_ascii() {
                tokens += 1;
            }
        }
    }
    tokens
}

/// 截取 `<= budget_tokens` 的最长**字符边界安全**前缀。
///
/// 已经在预算内 → 原样返回（**不复制**）；否则在超预算的那个字符之前切开。
pub fn trim_to_tokens(text: &str, budget_tokens: usize) -> &str {
    if estimate_tokens(text) <= budget_tokens {
        return text;
    }
    let mut tokens = 0usize;
    let mut in_word = false;
    for (idx, c) in text.char_indices() {
        if is_word_char(c) {
            let add = usize::from(!in_word);
            if tokens + add > budget_tokens {
                // 切在词首：顺手去掉前缀尾部的空白，别留一个「hello 」。
                return text[..idx].trim_end();
            }
            tokens += add;
            in_word = true;
        } else {
            in_word = false;
            if !c.is_ascii() {
                if tokens + 1 > budget_tokens {
                    return text[..idx].trim_end();
                }
                tokens += 1;
            }
        }
    }
    text
}

/// 按行铺满预算：**整行**收，直到下一行放不下为止。
///
/// 边界：
/// - 第一行自己就超预算时不返回空表（那会让「有命中却像没命中」），而是把
///   这一行截到预算内——**只有第一行**享受这个待遇，后面一律整行收；
/// - `budget_tokens == 0` → 空表（0 预算就是不要）。
pub fn fit_lines(lines: &[String], budget_tokens: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if budget_tokens == 0 {
        return out;
    }
    let mut used = 0usize;
    for (i, line) in lines.iter().enumerate() {
        let cost = estimate_tokens(line);
        if used + cost <= budget_tokens {
            used += cost;
            out.push(line.clone());
            continue;
        }
        if i == 0 {
            let cut = trim_to_tokens(line, budget_tokens);
            if !cut.is_empty() {
                out.push(cut.to_string());
            }
        }
        break;
    }
    out
}
