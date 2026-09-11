//! 纯 SSE 增量解码器 [`SseDecoder`]：无 I/O、无 async，支持**任意字节切割**。
//!
//! 按 WHATWG「server-sent events」行规则解析：
//! - 事件以**空行**分隔；行终止符 `\r\n` / `\n` / `\r` 三种皆可（可混用）；
//! - `:` 开头的行是注释，忽略；
//! - 字段形如 `field: value`，冒号后**至多一个空格**属于分隔符；
//! - 同一事件内多条 `data:` 行按顺序以 `\n` 连接成最终 data；
//! - 记录 `event:` 字段值；`id:` / `retry:` 与本层无关，忽略（不做重连语义）。
//!
//! 关键不变量：**绝不假设单个网络 chunk 是完整行**。所有字节先进内部缓冲，
//! 只有出现完整终止符才消费；因此 UTF-8 多字节字符、`\r\n` 终止符本身
//! 被网络拆到两个 chunk 里都天然安全。

use std::mem;

/// 一条完整的 SSE 事件。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SseEvent {
    /// `event:` 字段值；未出现时为 `None`（OpenAI 兼容流通常没有）。
    pub event: Option<String>,
    /// 该事件的 data；多条 `data:` 行已按序用 `\n` 连接。
    pub data: String,
}

/// UTF-8 BOM（流开头允许存在，须剥离）。
const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// 增量 SSE 解码器。
///
/// 用法：`push(chunk)` 喂任意切分的原始字节 → 反复
/// [`next_event`](Self::next_event) 取出已完整的事件 → 流结束时调一次
/// [`finish`](Self::finish) 兜底（处理无尾随空行的最后一条 data）。
#[derive(Debug, Default)]
pub struct SseDecoder {
    /// 尚未构成完整行的原始字节。
    buf: Vec<u8>,
    /// 当前累积中事件的 `event:` 字段。
    pending_event: Option<String>,
    /// 当前累积中事件的 data（多行以 `\n` 连接）。
    pending_data: String,
    /// 是否已完成开头的 BOM 处理。
    bom_checked: bool,
}

impl SseDecoder {
    /// 新建解码器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 追加一段任意切分的原始字节（不要求按行/按事件对齐）。
    pub fn push(&mut self, chunk: &[u8]) {
        self.buf.extend_from_slice(chunk);
    }

    /// 弹出一条已完整（遇到空行边界）的 SSE 事件；缓冲不足时返回 `None`。
    pub fn next_event(&mut self) -> Option<SseEvent> {
        while let Some(line) = self.pop_line() {
            if let Some(event) = self.handle_line(&line) {
                return Some(event);
            }
        }
        None
    }

    /// 流结束收尾：把残余缓冲当作最后一行处理，并在 data 非空时兜底派发。
    ///
    /// 宽容模式：兼容「最后一条 `data:` 后没有尾随空行就关闭连接」的服务端。
    /// 调用后解码器不应继续使用。
    pub fn finish(&mut self) -> Option<SseEvent> {
        if !self.buf.is_empty() {
            let mut rest = mem::take(&mut self.buf);
            // 流末尾悬挂的 CR 是行终止符，不属于行内容。
            if rest.last() == Some(&b'\r') {
                rest.pop();
            }
            if !rest.is_empty() {
                let line = String::from_utf8_lossy(&rest).into_owned();
                if let Some(event) = self.handle_line(&line) {
                    return Some(event);
                }
            }
        }
        self.take_pending()
    }

    /// 从缓冲取出一条**完整行**（不含终止符）；不足则等待更多字节。
    fn pop_line(&mut self) -> Option<String> {
        if !self.skip_bom() {
            return None;
        }

        let term = self.buf.iter().position(|&b| b == b'\n' || b == b'\r')?;
        // 行内容终点（CRLF 时回退掉 '\r'）。
        let mut content_end = term;
        // 本次消费的字节数。
        let mut consume = term + 1;
        if self.buf[term] == b'\r' {
            // 悬挂的 CR：可能是 CRLF 的前半，必须等到下一个字节才能确定消费量，
            // 否则会把 LF 误判成下一行的「空行」。真正的流末尾悬挂 CR 由 finish 兜底。
            if self.buf.len() <= term + 1 {
                return None;
            }
            if self.buf[term + 1] == b'\n' {
                consume += 1;
            }
        } else if content_end > 0 && self.buf[content_end - 1] == b'\r' {
            content_end -= 1;
        }

        // 只在完整行上转 UTF-8：多字节字符不可能被终止符劈开。
        let line = String::from_utf8_lossy(&self.buf[..content_end]).into_owned();
        self.buf.drain(..consume);
        Some(line)
    }

    /// 流开头的 BOM 剥离。返回 `false` 表示 BOM 本身可能被拆开、需等更多字节。
    fn skip_bom(&mut self) -> bool {
        if self.bom_checked {
            return true;
        }
        if self.buf.starts_with(&BOM) {
            self.buf.drain(..BOM.len());
            self.bom_checked = true;
            return true;
        }
        // BOM 的非空前缀：先等着看剩余字节。
        for partial in 1..BOM.len() {
            if self.buf.starts_with(&BOM[..partial]) {
                return false;
            }
        }
        self.bom_checked = true;
        true
    }

    /// 处理一行字段/注释/空行；空行触发派发，事件完整时返回它。
    fn handle_line(&mut self, line: &str) -> Option<SseEvent> {
        if line.is_empty() {
            // 空行 = 事件边界；没有累积数据则是多余空行，重置并跳过。
            return self.take_pending();
        }
        if line.starts_with(':') {
            return None; // 注释行（如 keep-alive ": ping"）。
        }

        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            // 无冒号的行：整行是字段名，值为空。
            None => (line, ""),
        };

        match field {
            "data" => {
                if !self.pending_data.is_empty() {
                    self.pending_data.push('\n');
                }
                self.pending_data.push_str(value);
            }
            "event" => self.pending_event = Some(value.to_string()),
            // "id" / "retry" 及未知字段：与本层职责无关，忽略。
            _ => {}
        }
        None
    }

    /// 派发并清空累积中的事件；data 为空时不派发（只复位 event 名）。
    fn take_pending(&mut self) -> Option<SseEvent> {
        if self.pending_data.is_empty() {
            self.pending_event = None;
            return None;
        }
        Some(SseEvent {
            event: self.pending_event.take(),
            data: mem::take(&mut self.pending_data),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 逐字节喂入，模拟最恶劣的网络切割。
    fn decode_byte_by_byte(raw: &str) -> Vec<SseEvent> {
        let mut dec = SseDecoder::new();
        let mut out = Vec::new();
        for b in raw.as_bytes() {
            dec.push(std::slice::from_ref(b));
            while let Some(ev) = dec.next_event() {
                out.push(ev);
            }
        }
        if let Some(ev) = dec.finish() {
            out.push(ev);
        }
        out
    }

    #[test]
    fn basic_lf_events_and_comments() {
        let raw = ": keep-alive\n\n\
                   data: hello\n\n\
                   : another comment\n\
                   data: world\n\n";
        let events = decode_byte_by_byte(raw);
        assert_eq!(
            events,
            vec![
                SseEvent {
                    event: None,
                    data: "hello".to_string()
                },
                SseEvent {
                    event: None,
                    data: "world".to_string()
                },
            ]
        );
    }

    #[test]
    fn crlf_and_cr_terminators_mix_freely() {
        let mut dec = SseDecoder::new();
        dec.push(b"data: one\r\n\r\ndata: two\r\r");
        assert_eq!(dec.next_event().unwrap().data, "one");
        // 结尾悬挂的 CR 必须等下一个字节才能判定（可能是 CRLF 的前半）。
        assert_eq!(dec.next_event(), None);
        dec.push(b"data: three\n");
        // 新字节证实旧 "\r" 是终止符：随之弹出的空行触发派发。
        assert_eq!(dec.next_event().unwrap().data, "two");
        assert_eq!(dec.next_event(), None);
        assert_eq!(dec.finish().unwrap().data, "three");
    }

    #[test]
    fn crlf_split_across_pushes_is_not_two_lines() {
        // "\r" 和 "\n" 分属两个 push：不得把 LF 误判成下一行（空行会提前派发事件）。
        let mut dec = SseDecoder::new();
        dec.push(b"data: split\r");
        assert_eq!(dec.next_event(), None);
        dec.push(b"\n\ndata: next\n\n");
        assert_eq!(dec.next_event().unwrap().data, "split");
        assert_eq!(dec.next_event().unwrap().data, "next");
        assert_eq!(dec.next_event(), None);
    }

    #[test]
    fn multiline_data_joins_with_newline() {
        let events = decode_byte_by_byte("data: line1\ndata: line2\ndata:\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "line1\nline2\n"); // 空值 data 行也占一段
    }

    #[test]
    fn field_value_strips_single_leading_space_only() {
        let events = decode_byte_by_byte("data:  two spaces\n\n");
        assert_eq!(events[0].data, " two spaces");
    }

    #[test]
    fn event_field_is_captured_and_reset_between_events() {
        let raw = "event: add\ndata: {\"a\":1}\n\nevent: remove\ndata: {}\n\n";
        let events = decode_byte_by_byte(raw);
        assert_eq!(events[0].event.as_deref(), Some("add"));
        assert_eq!(events[1].event.as_deref(), Some("remove"));

        // 下一个事件没写 event 字段 → 不应残留上一个值。
        let events = decode_byte_by_byte("event: x\ndata: 1\n\ndata: 2\n\n");
        assert_eq!(events[0].event.as_deref(), Some("x"));
        assert_eq!(events[1].event, None);
    }

    #[test]
    fn empty_events_are_skipped_without_reseting_pending_state() {
        // 连续多个空行只产生一个事件；event 名写在前面、隔了空行仍有效？——
        // 按 WHATWG 规则：空行若未携带 data，缓冲整体复位（event 名也被丢弃）。
        let events = decode_byte_by_byte("event: gone\n\n\ndata: kept\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, None); // "gone" 已被空行复位
        assert_eq!(events[0].data, "kept");
    }

    #[test]
    fn utf8_multibyte_split_across_chunks_survives() {
        // "你好" 的 UTF-8 字节从中间劈开。
        let full = "data: 你好\n\n".as_bytes().to_vec();
        let split_at = 8; // 劈在多字节字符中间
        let mut dec = SseDecoder::new();
        dec.push(&full[..split_at]);
        assert_eq!(dec.next_event(), None);
        dec.push(&full[split_at..]);
        assert_eq!(dec.next_event().unwrap().data, "你好");
    }

    #[test]
    fn bom_at_stream_start_is_stripped_even_when_split() {
        let mut dec = SseDecoder::new();
        dec.push(&[0xEF, 0xBB]);
        dec.push(&[0xBF]);
        dec.push(b"data: ok\n\n");
        assert_eq!(dec.next_event().unwrap().data, "ok");
    }

    #[test]
    fn finish_flushes_unterminated_final_data() {
        let mut dec = SseDecoder::new();
        dec.push(b"data: [DONE]\n"); // 没有 \n\n 就断流
        assert_eq!(dec.next_event(), None);
        assert_eq!(dec.finish().unwrap().data, "[DONE]");
    }

    #[test]
    fn finish_flushes_hanging_cr_as_line_end() {
        let mut dec = SseDecoder::new();
        dec.push(b"data: tail\r");
        assert_eq!(dec.next_event(), None);
        assert_eq!(dec.finish().unwrap().data, "tail");
    }

    #[test]
    fn arbitrary_chunk_cuts_produce_identical_results() {
        // 同一份数据按不同粒度切割，结果必须一致。
        let raw = ": c\n\ndata: a\n\ndata: b\r\n\rdata: c\n\ndata: [DONE]\n\n";
        let expected = decode_byte_by_byte(raw);
        assert_eq!(
            expected,
            vec![
                SseEvent {
                    event: None,
                    data: "a".to_string()
                },
                SseEvent {
                    event: None,
                    data: "b".to_string()
                },
                SseEvent {
                    event: None,
                    data: "c".to_string()
                },
                SseEvent {
                    event: None,
                    data: "[DONE]".to_string()
                },
            ]
        );

        for cut in [2usize, 5, 7, 13] {
            let mut dec = SseDecoder::new();
            let mut got = Vec::new();
            for chunk in raw.as_bytes().chunks(cut) {
                dec.push(chunk);
                while let Some(ev) = dec.next_event() {
                    got.push(ev);
                }
            }
            got.extend(dec.finish());
            assert_eq!(got, expected, "cut={cut}");
        }
    }
}
