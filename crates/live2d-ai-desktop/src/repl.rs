//! 终端 REPL 输入行的**纯解析**（零 IO、零线程，可单测）。
//!
//! 职责边界刻意收窄：本模块只做「一行文本 → [`ReplCommand`]」的判定，
//! 不读取 stdin、不开线程、不触碰窗口/声卡——线程与通道接线在后续批次，
//! 届时 REPL 线程逐行读取后调用 [`parse_line`]，把结果发往主事件循环。
//!
//! 行分类规则：
//! - 空行 / 纯空白 / 去除首尾空白后以 `#` 开头的注释行 → 忽略
//!   （[`parse_line`] 返回 `None`，调用方直接跳过该行）；
//! - 以 `/` 开头的行按命令解析：命令词大小写不敏感地匹配
//!   `/stop` / `/release` / `/quit`（别名 `/exit` 等价于 `/quit`）；
//!   命令词与其后内容之间的多空白被容忍——本批命令均无参数，
//!   命令词之后的剩余内容一律忽略；
//!   未识别的命令词归为 [`ReplCommand::Unknown`]（保留原始词的大小写，
//!   仅去掉前导 `/`；单独一个 `/` 即无名未知命令），由调用方负责提示；
//! - 其余非空行 → 普通聊天文本 [`ReplCommand::Say`]（去除首尾空白后的整行，
//!   中间空白原样保留；行中间出现的 `/xxx` 不构成命令）。

// 最终应用接线批次启用；当前仅暴露纯解析供单测与后续 REPL 线程使用。
#![allow(dead_code)]

/// 一条 REPL 输入行的解析结果。
///
/// 语义约定：
/// - [`ReplCommand::Say`]：普通聊天输入（去除首尾空白后的整行文本）；
/// - [`ReplCommand::Stop`]：取消当前对话轮次并清空声卡待播队列；
/// - [`ReplCommand::Release`]：退出点击穿透恢复交互模式（若当前非穿透则无操作）；
///   名称取「释放鼠标事件回桌面」；
/// - [`ReplCommand::Quit`]：退出整个应用；
/// - [`ReplCommand::Unknown`]：以 `/` 开头但不认识的命令，携带原始词，
///   调用方负责向用户提示。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplCommand {
    /// 普通聊天输入（去除首尾空白后的整行文本）。
    Say(String),
    /// 取消当前对话轮次并清空声卡待播队列。
    Stop,
    /// 退出点击穿透恢复交互模式（若当前非穿透则无操作）。
    Release,
    /// 退出整个应用。
    Quit,
    /// 以 `/` 开头但不认识的命令（携带去掉前导 `/` 后的原始词）。
    Unknown(String),
}

/// 解析一行终端输入；返回 `None` 表示该行应被整体忽略。
///
/// 判定顺序：先排除空行 / 纯空白 / `#` 注释行，再区分命令行（`/` 开头）
/// 与聊天文本；命令词取第一个空白分隔的词并按 ASCII 大小写折叠比较。
pub fn parse_line(line: &str) -> Option<ReplCommand> {
    // 统一先 trim：空行/纯空白在此归零，后续判定都在 trim 后的视图上进行。
    let trimmed = line.trim();

    // 注释行：与 shell 惯例一致，允许行首缩进后再写 `#`。
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }

    // 不以 `/` 开头即聊天文本：整行（trim 后）交给对话层。
    if !trimmed.starts_with('/') {
        return Some(ReplCommand::Say(trimmed.to_owned()));
    }

    // 命令行：首个空白分隔的词即命令词；本批命令均无参数，
    // 命令词之后的剩余内容（含多空白分隔）一律忽略。
    let word = trimmed.split_whitespace().next().unwrap_or(trimmed);
    match word.to_ascii_lowercase().as_str() {
        "/stop" => Some(ReplCommand::Stop),
        "/release" => Some(ReplCommand::Release),
        // `/exit` 是 `/quit` 的别名（终端用户肌肉记忆）。
        "/quit" | "/exit" => Some(ReplCommand::Quit),
        // 未知命令：原始词保留大小写，仅剥掉前导 `/`，提示留给调用方。
        _ => Some(ReplCommand::Unknown(word[1..].to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::{ReplCommand, parse_line};

    /// 表驱动：空行 / 纯空白 / `#` 注释行（含缩进后注释）→ `None`（忽略）。
    #[test]
    fn blank_and_comment_lines_yield_none() {
        let ignored = [
            "",
            "   ",
            "\t \r\n",
            "# 这是注释",
            "#no-space-comment",
            "   # 缩进后的注释同样忽略",
        ];
        for line in ignored {
            assert_eq!(parse_line(line), None, "该行应被忽略：{line:?}");
        }
    }

    /// 表驱动：四种命令各自小写 / 大写 / 混合大小写（`/exit` 别名 → Quit）。
    #[test]
    fn known_commands_match_case_insensitively() {
        let cases = [
            ("/stop", ReplCommand::Stop),
            ("/STOP", ReplCommand::Stop),
            ("/Stop", ReplCommand::Stop),
            ("/release", ReplCommand::Release),
            ("/RELEASE", ReplCommand::Release),
            ("/Release", ReplCommand::Release),
            ("/quit", ReplCommand::Quit),
            ("/QUIT", ReplCommand::Quit),
            ("/Quit", ReplCommand::Quit),
            ("/exit", ReplCommand::Quit),
            ("/EXIT", ReplCommand::Quit),
            ("/Exit", ReplCommand::Quit),
        ];
        for (line, expected) in cases {
            assert_eq!(parse_line(line), Some(expected), "命令行：{line:?}");
        }
    }

    /// 表驱动：命令行首尾空白、以及命令词与行尾之间的多空白均被容忍。
    #[test]
    fn command_words_tolerate_extra_whitespace() {
        let cases = [
            ("  /quit  ", ReplCommand::Quit),
            ("\t/stop\t", ReplCommand::Stop),
            ("/release   ", ReplCommand::Release),
            ("/exit \t \r\n", ReplCommand::Quit),
        ];
        for (line, expected) in cases {
            assert_eq!(parse_line(line), Some(expected), "空白容忍：{line:?}");
        }
    }

    /// 表驱动：未知的 `/` 命令 → `Unknown(原始词)`；原始词保留大小写、
    /// 只去掉前导 `/`；单独一个 `/` 即无名未知命令。
    #[test]
    fn unrecognized_slash_word_yields_unknown() {
        let cases = [
            ("/foo", ReplCommand::Unknown(String::from("foo"))),
            ("/FOO", ReplCommand::Unknown(String::from("FOO"))),
            ("/FooBar arg", ReplCommand::Unknown(String::from("FooBar"))),
            ("/退出", ReplCommand::Unknown(String::from("退出"))),
            ("/", ReplCommand::Unknown(String::new())),
        ];
        for (line, expected) in cases {
            assert_eq!(parse_line(line), Some(expected), "未知命令：{line:?}");
        }
    }

    /// 聊天文本去除首尾空白，且中间连续空格原样保留（不做任何折叠）。
    #[test]
    fn chat_text_is_trimmed_with_inner_spaces_kept() {
        assert_eq!(
            parse_line("   hello   world  "),
            Some(ReplCommand::Say(String::from("hello   world")))
        );
    }

    /// 中文聊天文本（含标点）原样作为 `Say` 文本，不做转写。
    #[test]
    fn chinese_chat_text_passes_through_verbatim() {
        let text = "今天天气怎么样？陪我聊聊天。";
        assert_eq!(parse_line(text), Some(ReplCommand::Say(String::from(text))));
    }

    /// 行中间出现的 `/quit` 不是命令：不以 `/` 开头的行一律是聊天文本。
    #[test]
    fn slash_inside_text_stays_chat() {
        assert_eq!(
            parse_line("请帮我查 /quit 这个词"),
            Some(ReplCommand::Say(String::from("请帮我查 /quit 这个词")))
        );
    }
}
