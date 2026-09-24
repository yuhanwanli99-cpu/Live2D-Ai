//! 唤醒闸 + 手动闸（L1 产品级，2026-09-15）——**纯函数**，可直接单测。
//!
//! 两把闸决定「一段转写能不能进主链」，判定顺序**钉死**（见 [`evaluate`]）：
//!
//! 1. `manual_enabled == false` → [`GateOutcome::ManualOff`]（手动闸）；
//! 2. `wake_phrase` **显式**空白 → [`GateOutcome::GateClosed`]（**总闸 = 唤醒短语**：
//!    空 = 关；键缺失走缺省词，见下方「产品定义」）；
//! 3. 清洗后的文本**不以**唤醒短语**开头**（大小写不敏感、忽略空白差异）→
//!    [`GateOutcome::WakeRequired`]；
//! 4. 否则 [`GateOutcome::Allow`]，`text` = **剥掉唤醒短语**后的正文。
//!
//! # 按住说话（PTT，「跳过唤醒匹配」，P0-4）
//!
//! `ptt = true`（`POST /api/v1/voice/transcript` 的 body 字段）是**用户显式按键**，
//! 表达「我现在就要说」——因此 [`evaluate_mode`] 对它**跳过第 3 步**（不要求文本
//! 含唤醒短语）。**手动闸与总闸不变**（总闸关 = 语音输入整体关闭，PTT 也进不来）；
//! 若用户按住时仍说了唤醒词，顺手剥掉（不留进正文）。清洗 / 归一化 / 长度 /
//! token 全部不变。
//!
//! # 句首锚定（P0-4 加固）
//!
//! 唤醒短语只在**句首**命中（跳过前导空白）。旧行为是任意位置子串命中，
//! 「我昨天说小可爱好看」会被误触发——那既误发一句无关的话，也把「小可爱」
//! 从句子中间挖掉。现在 [`contains_wake_phrase`] / [`strip_wake_phrase`] 都是
//! 「以它开头」。
//!
//! # 产品定义（2026-09-15 更新）
//!
//! **总闸就是唤醒短语**：
//! - 配置里**缺** `wake_phrase` 键 → 走产品缺省 [`DEFAULT_WAKE_PHRASE`]
//!   （「小可爱」）→ 总闸**默认开**（装好就能用唤醒词，符合「产品默认应开着
//!   唤醒词」的用户裁决）；
//! - 键**存在但为空**（用户显式清空）→ 总闸关，一切转写被拒
//!   （`403 voice_gate_closed`）——「空 = 关」这条语义保留；
//! - 已有非空自定义**不覆盖**。
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
    /// 总闸关闭（`wake_phrase` **显式**为空 / 纯空白；键缺失不算）。
    GateClosed,
    /// 文本里没有唤醒短语（含空输入）。
    WakeRequired,
}

/// **产品缺省唤醒词**（2026-09-15 用户裁决）：新装 / 配置里缺该键时用它，
/// 于是「装好就能用唤醒词」，而不是默认把语音总闸关着。
///
/// 两条语义要分清：
/// - **键缺失**（从未设置）→ 缺省「小可爱」→ 总闸**开**；
/// - **键存在但为空 / 纯空白**（显式清空）→ 空串 → 总闸**关**。
///
/// 已有用户自定义（键存在且非空）**不覆盖**。
pub const DEFAULT_WAKE_PHRASE: &str = "小可爱";

/// 从 Mod config 读 `wake_phrase`（语义见 DEFAULT_WAKE_PHRASE 的说明）。
///
/// 返回值已 `trim`；调用方**不得**回显它的明文（面板只显示「是否已设置」）。
/// 非字符串（写错类型）按「键缺失」处理 → 缺省词，绝不失败。
pub fn wake_phrase_from_config(config: &Value) -> String {
    match config.get("wake_phrase").and_then(Value::as_str) {
        Some(raw) => raw.trim().to_string(),
        None => DEFAULT_WAKE_PHRASE.to_string(),
    }
}

/// 总闸是否打开（= 生效的唤醒词非空；键缺失时缺省词非空 → 开着）。
pub fn wake_gate_open(config: &Value) -> bool {
    !wake_phrase_from_config(config).is_empty()
}

/// config 里是否**显式**配了非空 wake_phrase。
///
/// 与 wake_gate_open 的区别只在**缺键**这一种情形：缺键 → 总闸开（走缺省
/// 「小可爱」），但用户并没有设过唤醒词。面板 / 自检用这个键说清「你用的是
/// 缺省词还是自己设的词」，而不是把「开着」谎报成「你设过」。
pub fn wake_phrase_explicitly_set(config: &Value) -> bool {
    config
        .get("wake_phrase")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.trim().is_empty())
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

/// 四态判定（`ptt = false`，即常驻唤醒路径）：manual → 总闸 → 唤醒词 → 放行
/// （顺序见模块头注）。等价于 [`evaluate_mode`]。
///
/// `cleaned` 必须是 [`crate::clean_transcript`] 处理后的文本
/// （handler 与 Mod 注入共用同一份清洗）。
pub fn evaluate(config: &Value, cleaned: &str) -> GateOutcome {
    evaluate_mode(config, cleaned, false)
}

/// 四态判定 + **PTT 模式**（P0-4）：`ptt = true` 时**跳过唤醒匹配**。
///
/// 手动闸与总闸（`wake_phrase` 显式空）**不变**——PTT 是「用户显式按键」，
/// 不是「绕过语音输入总开关」。命中唤醒词时仍然剥掉（正文里不留）。
/// 真源只有一个：handler / Mod 注入都走这里。
pub fn evaluate_mode(config: &Value, cleaned: &str, ptt: bool) -> GateOutcome {
    if !manual_enabled_from_config(config) {
        return GateOutcome::ManualOff;
    }
    let phrase = wake_phrase_from_config(config);
    if phrase.is_empty() {
        return GateOutcome::GateClosed;
    }
    if ptt {
        // 按住说话：不要求含唤醒词；若仍说了，顺手剥掉。
        let text = strip_wake_phrase(cleaned, &phrase).unwrap_or_else(|| cleaned.to_string());
        return GateOutcome::Allow { text };
    }
    match strip_wake_phrase(cleaned, &phrase) {
        Some(text) => GateOutcome::Allow { text },
        None => GateOutcome::WakeRequired,
    }
}

/// `ptt = true` 的便捷入口（等价于 [`evaluate_mode`] 的 PTT 分支）。
pub fn evaluate_ptt(config: &Value, cleaned: &str) -> GateOutcome {
    evaluate_mode(config, cleaned, true)
}

/// 文本是否**以**唤醒短语开头（忽略前导空白、空白差异、大小写）。
pub fn contains_wake_phrase(text: &str, phrase: &str) -> bool {
    find_wake_span(text, phrase).is_some()
}

/// 剥掉**句首**的唤醒短语，返回正文；没有以它开头 → `None`。
///
/// - 短语必须出现在**句首**（跳过前导空白；P0-4 加固）；
/// - 同时去掉紧随其后的标点 / 空白（`"小爱，关灯"` → `"关灯"`）；
/// - 剥完为空 → 返回 `Some("")`（由调用方按空文本拒绝）。
pub fn strip_wake_phrase(text: &str, phrase: &str) -> Option<String> {
    let (start, end) = find_wake_span(text, phrase)?;
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    // 句首匹配下 `start` 之前只可能有前导空白（`clean_transcript` 通常已去掉）。
    out.extend(chars[..start].iter());
    out.extend(chars[end..].iter());
    // 短语与其后紧跟的标点 / 空白一起去掉。
    let body = out.trim_start_matches(is_separator).to_string();
    Some(crate::clean_transcript(&body).unwrap_or_default())
}

/// 找唤醒短语是否在 `text` **句首**，返回字符区间 `[start, end)`。
///
/// 句首锚定（P0-4）：跳过**前导空白**，从第一个非空白字符开始匹配，只试一次；
/// 短语出现在中间 → `None`（旧行为是任意位置子串命中，会误触发）。
/// 短语自身的空白先被丢掉；文本里的空白在匹配过程中被跳过
/// （因此 `"小 爱"` 命中 `"小爱"`）；字符比较大小写不敏感。
fn find_wake_span(text: &str, phrase: &str) -> Option<(usize, usize)> {
    let hay: Vec<char> = text.chars().collect();
    let needle: Vec<char> = phrase.chars().filter(|c| !c.is_whitespace()).collect();
    if needle.is_empty() {
        return None;
    }
    // 句首 = 第一个非空白字符；文本为空 / 全空白 → 不命中。
    let start = hay.iter().position(|c| !c.is_whitespace())?;
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
        Some((start, i))
    } else {
        None
    }
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
