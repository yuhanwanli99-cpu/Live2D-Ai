//! locale 归一化策略（Wave 3 轨 A，2026-09-14）。
//!
//! `locale` 在本项目里的**唯一**作用，是决定 `clean_transcript` 之后的文本
//! 归一化档位。它：
//!
//! - **不**选 ASR 引擎（ASR 在 sidecar，见 `docs/examples/voice-sidecar/`）；
//! - **不**随请求发给 sidecar（推模式，Rust 不主动发起任何请求）；
//! - **不**改 `clean_transcript` 的通用规则（零宽 / 控制符 / 空白折叠）。
//!
//! 「locale 只影响 text 归一化、不影响 ASR 引擎选型」这句话由本文件 +
//! `docs/voice-input.md` §6.1 钉死；回归见 `crate::tests` 的 `locale_*`。

/// 缺省识别语言（Mod config 的 `locale` 为空/缺失时使用）。
pub const DEFAULT_LOCALE: &str = "zh-CN";

/// locale → 归一化档（按**语言族**，不是逐 locale 枚举）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocaleProfile {
    /// CJK（`zh*` / `ja*` / `ko*`）：删掉**两个 CJK 字符之间**的空格。
    /// 那是 ASR 分词伪影（CJK 不用词间空格），不是词边界。
    Cjk,
    /// 拉丁（其余，含 `en-US`）：保留空格；只在 CJK 与 ASCII 词字符
    /// **直接相邻**处补一个空格（中英混读的词边界）。
    Latin,
}

/// BCP-47 语言标签 → 归一化档。
///
/// 只看**语言子标签**（`zh-CN` / `zh-Hans-CN` / `ZH` 都算 `zh`），
/// 大小写与 `-` / `_` 分隔无关；空 / 未知语言 → [`LocaleProfile::Latin`]。
pub fn locale_profile(locale: &str) -> LocaleProfile {
    let lang = locale
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match lang.as_str() {
        "zh" | "ja" | "ko" => LocaleProfile::Cjk,
        _ => LocaleProfile::Latin,
    }
}

/// CJK 字符（汉字 / 平假名 / 片假名 / 谚文音节，含扩展区）。
pub fn is_cjk(ch: char) -> bool {
    matches!(ch as u32,
        0x3040..=0x30FF    // 平假名 / 片假名
        | 0x3400..=0x4DBF  // 汉字扩展 A
        | 0x4E00..=0x9FFF  // 汉字基本区
        | 0xAC00..=0xD7AF  // 谚文音节
        | 0xF900..=0xFAFF  // 兼容汉字
    )
}

/// ASCII 词字符（字母 / 数字）。
pub fn is_ascii_word(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
}

/// locale 归一化：**在 [`crate::clean_transcript`] 之后**调用（幂等）。
///
/// - `zh-CN` 等 CJK 档：`"你好 世界"` → `"你好世界"`（删 CJK 词间空格）；
/// - `en-US` 等拉丁档：`"打开空调wifi"` → `"打开空调 wifi"`（补中英边界），
///   已有空格原样保留。
pub fn normalize_for_locale(text: &str, locale: &str) -> String {
    match locale_profile(locale) {
        LocaleProfile::Cjk => drop_inter_cjk_spaces(text),
        LocaleProfile::Latin => space_cjk_ascii_boundaries(text),
    }
}

/// 删除**两侧都是 CJK** 的那个空格（输入已由清洗折成单个空格）。
fn drop_inter_cjk_spaces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == ' '
            && out.chars().last().is_some_and(is_cjk)
            && chars.peek().copied().is_some_and(is_cjk)
        {
            continue;
        }
        out.push(ch);
    }
    out
}

/// 在 CJK 与 ASCII 词字符直接相邻处补一个空格（其余字符原样）。
fn space_cjk_ascii_boundaries(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev: Option<char> = None;
    for ch in text.chars() {
        if let Some(p) = prev {
            let boundary = (is_cjk(p) && is_ascii_word(ch)) || (is_ascii_word(p) && is_cjk(ch));
            if boundary {
                out.push(' ');
            }
        }
        out.push(ch);
        prev = Some(ch);
    }
    out
}
