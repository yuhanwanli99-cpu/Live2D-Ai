//! 唤醒闸 + 手动闸（L1 产品级，2026-09-15）——**纯函数**，可直接单测。
//!
//! 两把闸决定「一段转写能不能进主链」，判定顺序**钉死**（见 [`evaluate`]）：
//!
//! 1. `manual_enabled == false` → [`GateOutcome::ManualOff`]（手动闸）；
//! 2. `wake_phrase` 空白 → [`GateOutcome::GateClosed`]（**总闸 = 唤醒短语**：空 = 关）；
//! 3. 清洗后的文本不包含唤醒短语（大小写不敏感、忽略空白差异）→
//!    [`GateOutcome::WakeRequired`]；
//! 4. 否则 [`GateOutcome::Allow`]，`text` = **剥掉唤醒短语**后的正文。
//!
//! # 产品定义（本轮钉死）
//!
//! **总闸就是唤醒短语**：没有配 `wake_phrase` = 一切转写被拒
//! （`403 voice_gate_closed`）。这是刻意的「默认关闭」——语音输入必须先被
//! 用户显式打开（填一个唤醒词），而不是装好就能灌。
//!
//! 空输入（清洗后为空串）同样按 [`GateOutcome::WakeRequired`] 处理：**没听见
//! 唤醒词**。因此 handler 的 `empty_transcript` 只在「整句就是唤醒词本身」时出现。
//!
//! # 大小写 / 空白
//!
//! 匹配时把两侧的空白都忽略、字符大小写不敏感（`"你好 小爱"` 能命中
//! `wake_phrase = "小爱好"` 之外的写法，例如 `"你好小爱"`）；
//! 剥离发生在**清洗后**的文本上，所以「开头短语 + 紧接的标点 / 空白」会被一起去掉。

use serde_json::Value;

/// 闸门判定结果（四态）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateOutcome {
    /// 放行；`text` 是**剥掉唤醒短语**后的正文（可能为空串，由调用方按
    /// 空文本拒绝）。
    Allow { text: String },
    /// 手动闸关闭（`manual_enabled = false`）。
    ManualOff,
    /// 总闸关闭（`wake_phrase` 缺省 / 空白）。
    GateClosed,
    /// 文本里没有唤醒短语（含空输入）。
    WakeRequired,
}

/// 从 Mod config 读 `wake_phrase`：非字符串 / 缺失 → 空串（= 总闸关）。
///
/// 返回值已 `trim`；调用方**不得**回显它的明文（面板只显示「是否已设置」）。
pub fn wake_phrase_from_config(config: &Value) -> String {
    config
        .get("wake_phrase")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string()
}

/// 总闸是否打开（= 是否配了非空 `wake_phrase`）。
pub fn wake_gate_open(config: &Value) -> bool {
    !wake_phrase_from_config(config).is_empty()
}

/// 手动闸是否打开（`manual_enabled`，缺省 **true**）。
///
/// 非布尔值按缺省处理（宽容：配置写错不该把链路无声打死，自检负责点名）。
pub fn manual_enabled_from_config(config: &Value) -> bool {
    config
        .get("manual_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

/// 四态判定：manual → 总闸 → 唤醒词 → 放行（顺序见模块头注）。
///
/// `cleaned` 必须是 [`crate::clean_transcript`] 处理后的文本
/// （handler 与 Mod 注入共用同一份清洗）。
pub fn evaluate(config: &Value, cleaned: &str) -> GateOutcome {
    if !manual_enabled_from_config(config) {
        return GateOutcome::ManualOff;
    }
    let phrase = wake_phrase_from_config(config);
    if phrase.is_empty() {
        return GateOutcome::GateClosed;
    }
    match strip_wake_phrase(cleaned, &phrase) {
        Some(text) => GateOutcome::Allow { text },
        None => GateOutcome::WakeRequired,
    }
}

/// 文本里是否包含唤醒短语（忽略空白差异、大小写不敏感）。
pub fn contains_wake_phrase(text: &str, phrase: &str) -> bool {
    find_wake_span(text, phrase).is_some()
}

/// 剥掉唤醒短语，返回正文；找不到 → `None`。
///
/// - 短语在**开头**：同时去掉紧随其后的标点 / 空白（`"小爱，关灯"` → `"关灯"`）；
/// - 短语在**中间**：只删短语，并把因此产生的多余空白折叠；
/// - 剥完为空 → 返回 `Some("")`（由调用方按空文本拒绝）。
pub fn strip_wake_phrase(text: &str, phrase: &str) -> Option<String> {
    let (start, end) = find_wake_span(text, phrase)?;
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    if start > 0 {
        out.extend(chars[..start].iter());
    }
    out.extend(chars[end..].iter());
    let body = if start == 0 {
        out.trim_start_matches(is_separator).to_string()
    } else {
        out
    };
    // 折叠短语被删掉后留下的空白（"请 小爱 关灯" → "请 关灯"）。
    Some(crate::clean_transcript(&body).unwrap_or_default())
}

/// 找唤醒短语在 `text` 里的**字符区间** `[start, end)`。
///
/// 匹配时：短语自身的空白先被丢掉；文本里的空白在匹配过程中被跳过
/// （因此 `"小 爱"` 命中 `"小爱"`）；字符比较大小写不敏感。
fn find_wake_span(text: &str, phrase: &str) -> Option<(usize, usize)> {
    let hay: Vec<char> = text.chars().collect();
    let needle: Vec<char> = phrase.chars().filter(|c| !c.is_whitespace()).collect();
    if needle.is_empty() {
        return None;
    }
    for start in 0..hay.len() {
        if hay[start].is_whitespace() {
            continue;
        }
        let mut pi = 0usize;
        let mut i = start;
        while i < hay.len() && pi < needle.len() {
            let c = hay[i];
            if c.is_whitespace() {
                i += 1;
                continue;
            }
            if lower_eq(c, needle[pi]) {
                pi += 1;
                i += 1;
            } else {
                break;
            }
        }
        if pi == needle.len() {
            return Some((start, i));
        }
    }
    None
}

/// 单字符大小写不敏感相等（对 CJK 退化为普通相等）。
fn lower_eq(a: char, b: char) -> bool {
    if a == b {
        return true;
    }
    a.to_lowercase().eq(b.to_lowercase())
}

/// 「短语后面可以一起丢掉的」分隔符：空白 + ASCII 标点 + 常见中英文标点。
fn is_separator(ch: char) -> bool {
    ch.is_whitespace()
        || ch.is_ascii_punctuation()
        || matches!(
            ch,
            '，' | '。'
                | '！'
                | '？'
                | '、'
                | '；'
                | '：'
                | '“'
                | '”'
                | '‘'
                | '’'
                | '（'
                | '）'
                | '《'
                | '》'
                | '【'
                | '】'
                | '…'
                | '—'
                | '～'
                | '·'
        )
}

#[cfg(test)]
#[path = "gate_tests.rs"]
mod gate_tests;
