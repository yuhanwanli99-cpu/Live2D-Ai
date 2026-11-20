//! 简易会话框的跨平台历史：与气泡省略策略完全分离，最多保留 12 条。

use std::collections::VecDeque;

pub const MAX_LINES: usize = 12;
/// 仅防止异常输入无限占内存；普通长句（包括数千字）逐字保留。
pub const MAX_ENTRY_CHARS: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Line {
    who: &'static str,
    text: String,
}

#[derive(Debug, Default)]
pub struct History {
    lines: VecDeque<Line>,
}

impl History {
    pub const fn new() -> Self {
        Self {
            lines: VecDeque::new(),
        }
    }

    pub fn push(&mut self, who: &'static str, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        let mut chars = text.chars();
        let mut owned = String::new();
        if chars.by_ref().take(MAX_ENTRY_CHARS + 1).count() > MAX_ENTRY_CHARS {
            owned.extend(text.chars().take(MAX_ENTRY_CHARS));
            owned.push('…');
        }
        self.lines.push_back(Line {
            who,
            text: if owned.is_empty() {
                text.to_string()
            } else {
                owned
            },
        });
        while self.lines.len() > MAX_LINES {
            self.lines.pop_front();
        }
    }

    /// Win32 多行 EDIT 使用 CRLF；存储中的每条正文不被气泡截断。
    #[must_use]
    pub fn rendered_text(&self) -> String {
        if self.lines.is_empty() {
            return "（还没有对话）".to_string();
        }
        self.lines
            .iter()
            .map(|line| format!("{}：{}", line.who, line.text))
            .collect::<Vec<_>>()
            .join("\r\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_keeps_complete_long_sentences_not_bubble_truncation() {
        let mut history = History::default();
        let long = format!("开头{}结尾", "完整长句".repeat(1000));
        history.push("AI", &long);
        assert_eq!(history.rendered_text(), format!("AI：{long}"));
        assert!(long.chars().count() > super::super::bubble::MAX_CHARS);
    }

    #[test]
    fn history_keeps_only_the_last_twelve_entries() {
        let mut history = History::default();
        history.push("我", "  \n ");
        assert_eq!(history.rendered_text(), "（还没有对话）");
        for index in 0..17 {
            history.push("我", &format!("第 {index} 句"));
        }
        let rendered = history.rendered_text();
        assert_eq!(rendered.lines().count(), MAX_LINES);
        assert!(!rendered.contains("第 0 句"));
        assert!(!rendered.contains("第 4 句"));
        assert!(rendered.contains("第 5 句"));
        assert!(rendered.contains("第 16 句"));
    }

    #[test]
    fn only_the_large_safety_limit_can_clip_an_entry() {
        let mut history = History::default();
        history.push("AI", &"安".repeat(MAX_ENTRY_CHARS + 10));
        let rendered = history.rendered_text();
        assert_eq!(rendered.chars().count(), 3 + MAX_ENTRY_CHARS + 1);
        assert!(rendered.ends_with('…'));
    }
}
