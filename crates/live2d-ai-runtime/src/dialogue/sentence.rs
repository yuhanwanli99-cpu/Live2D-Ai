//! 句子组装器：把 `TextDelta` 增量拼成完整句的状态机。
//!
//! 性质：
//! - 输出句子保留原文与标点（所有输出拼接 + 残余 == 输入，逐字不丢）；
//! - 省略号/连续标点折叠为一个句界，永不产出空句；
//! - ASCII `.` 的保守规则：两侧均为数字（小数，如 `3.14`）或紧跟小写字母
//!   （缩写续词，如 `e.g.this`）不切分——宁可不切也不误切，完整 NLP 分段
//!   属于上层职责；
//! - 缓冲字符数有上界：无句界的超长输入按「距上次输出的纯位置」每满
//!   `max_chars` 强制切一次——切点只由内容位置决定，因此任意 delta 切割
//!   得到完全一致的结果；
//! - `flush` 收尾：把悬挂的终止符 run 与无标点残余作为最后一句输出。
//!
//! 关键不变量「任意 delta 切割得到完全一致的输出」由 [`SentenceAssembler::reduce`]
//! 的纯位置切分配合 [`SentenceAssembler::sentence_end_in_buf`] 的「需要前瞻时
//! 返回 `None`」共同保证。

/// 句终止符集合：中英文句读 + 换行（协议固定，不含分号/冒号等弱标点）。
fn is_terminator(c: char) -> bool {
    matches!(c, '.' | '?' | '!' | '。' | '！' | '？' | '…' | '\n')
}

/// 把 `TextDelta` 增量拼成**完整句**的状态机。
///
/// 保证：
/// - 输出句子保留原文与标点（所有输出拼接 + 残余 == 输入，逐字不丢）；
/// - 省略号/连续标点折叠为一个句界，永不产出空句；
/// - ASCII `.` 的保守规则：两侧均为数字（小数，如 `3.14`）或紧跟小写字母
///   （缩写续词，如 `e.g.this`）不切分——宁可不切也不误切，完整 NLP 分段
///   属于上层职责；
/// - 缓冲字符数有上界：无句界的超长输入按「距上次输出的纯位置」每满
///   `max_chars` 强制切一次——切点只由内容位置决定，因此任意 delta 切割
///   得到完全一致的结果；超过 `max_chars` 的长句同样被位置切分（上限同时
///   约束单句长度与悬挂缓冲），`flush` 的收尾残余除外；
/// - `flush` 收尾：把悬挂的终止符 run 与无标点残余作为最后一句输出。
///
/// `flush` 之后状态清零，可继续复用于下一轮流。
#[derive(Debug)]
pub struct SentenceAssembler {
    buf: String,
    max_chars: usize,
}

impl SentenceAssembler {
    /// 默认最大缓冲字符数（约两三个短句，够 TTS 分段又不至于久等）。
    pub const DEFAULT_MAX_CHARS: usize = 48;

    /// 创建切句器。`max_chars` 为缓冲上限，会被钳制到 ≥1。
    pub fn new(max_chars: usize) -> Self {
        Self {
            buf: String::new(),
            max_chars: max_chars.max(1),
        }
    }

    /// 当前缓冲的字符数（观测用；`push` 之后保证 ≤ 最大缓冲字符数）。
    pub fn buffered_chars(&self) -> usize {
        self.buf.chars().count()
    }

    /// 消费一段正文增量，返回因此凑齐的完整句（可能为空）。
    pub fn push(&mut self, delta: &str) -> Vec<String> {
        self.buf.push_str(delta);
        let mut out = Vec::new();
        self.reduce(false, &mut out);
        out
    }

    /// 流结束收尾：结算悬挂的终止符与无标点残余。之后可复用。
    ///
    /// [`crate::dialogue::DialogueAssembler`] 在流切换到工具调用事件时也会调用本方法
    /// （别名 [`Self::seal`]）：正文不再增长，悬挂的句界可以安全封口，
    /// 从而保证 `SentenceReady` 出现在网络顺序的正确位置。
    pub fn flush(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        self.reduce(true, &mut out);
        out
    }

    /// [`Self::flush`] 的语义别名：在「上游切换到非文本事件」的时机封口悬挂句界。
    pub fn seal(&mut self) -> Vec<String> {
        self.flush()
    }

    /// 归结循环：反复提取已确定的短句；提取不动且超限时按纯位置强制切分；
    /// `eof` 时把残余整体收尾。
    ///
    /// 只提取**不超过 `max_chars`** 的完整句：更长的句子交给位置切分。这保证
    /// 「标点已在缓冲里」与「标点还没到」两种时机收敛到同一输出——提取与切分
    /// 的每一步都只由当前缓冲内容决定，与 delta 到达方式无关。
    fn reduce(&mut self, eof: bool, out: &mut Vec<String>) {
        loop {
            if let Some(end) = self.sentence_end_in_buf(eof) {
                if self.buf[..end].chars().count() <= self.max_chars {
                    // 句子恒非空（至少含一个终止符），is_empty 分支仅为防御。
                    let sentence: String = self.buf.drain(..end).collect();
                    if !sentence.is_empty() {
                        out.push(sentence);
                    }
                    continue;
                }
            }
            if self.buf.chars().count() > self.max_chars {
                let cut = self.byte_of_char(self.max_chars);
                out.push(self.buf.drain(..cut).collect());
                continue;
            }
            break;
        }
        if eof && !self.buf.is_empty() {
            out.push(std::mem::take(&mut self.buf));
        }
    }

    /// buf 中第一个**完全确定**的句子边界（字节下标，含终止符 run）。
    ///
    /// 返回 `None` 表示需要更多输入：终止符 run 或待判定的 `.` 悬挂在缓冲末尾时，
    /// 后续字符可能改变结论（连续标点、小数点后半），必须等待——这正是
    /// 「任意 delta 切割结果一致」的关键。
    fn sentence_end_in_buf(&self, eof: bool) -> Option<usize> {
        let mut chars = self.buf.char_indices().peekable();
        while let Some((i, c)) = chars.next() {
            if !is_terminator(c) {
                continue;
            }
            if c == '.' {
                match self.classify_period(i, eof) {
                    // 缺前瞻字符：整个扫描无法得出确定边界。
                    None => return None,
                    // 小数 / 缩写续词：不是句界，继续找。
                    Some(false) => continue,
                    Some(true) => {}
                }
            }
            // 吞掉连续终止符（`……`、`?!`、`。\n` 等只算一个句界）。
            let mut end = i + c.len_utf8();
            while let Some(&(j, nc)) = chars.peek() {
                if !is_terminator(nc) {
                    break;
                }
                if nc == '.' {
                    match self.classify_period(j, eof) {
                        None => return None,
                        Some(false) => break,
                        Some(true) => {}
                    }
                }
                chars.next();
                end = j + nc.len_utf8();
            }
            // run 必须被「下一个字符」或 EOF 封口才算确定。
            if end < self.buf.len() || eof {
                return Some(end);
            }
            return None;
        }
        None
    }

    /// 判定 buf 中字节下标 `i` 处的 `.` 是否为句终止符。
    ///
    /// - `Some(true/false)`：已判定；
    /// - `None`：`.` 恰在缓冲末尾且未到 EOF，需要前瞻一个字符才能判定。
    ///
    /// 保守规则：两侧均数字（小数）→ 否；紧跟 Unicode 小写字母（缩写续词）→ 否；
    /// EOF 下「非数字.」视为句末，「数字.」（如被截断的 `3.`）保守不切。
    fn classify_period(&self, i: usize, eof: bool) -> Option<bool> {
        let bytes = self.buf.as_bytes();
        let prev_is_digit = i > 0 && bytes[i - 1].is_ascii_digit();
        let follower = self.buf[i + 1..].chars().next();
        Some(match follower {
            None if !eof => return None,
            // EOF：`句子.` 切；`3.` 保守视为未写完的小数，留作残余。
            None => !prev_is_digit,
            Some(f) if prev_is_digit && f.is_ascii_digit() => false,
            Some(f) if f.is_lowercase() => false,
            Some(_) => true,
        })
    }

    /// 第 `n_chars` 个字符的字节下标（`buf` 按构造必是字符边界）。
    fn byte_of_char(&self, n_chars: usize) -> usize {
        self.buf
            .char_indices()
            .nth(n_chars)
            .map_or(self.buf.len(), |(i, _)| i)
    }
}
