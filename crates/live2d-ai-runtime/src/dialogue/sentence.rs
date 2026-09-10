//! 句子组装器：把 `TextDelta` 增量拼成完整句的状态机。
//!
//! **契约（2026-09-10 用户裁决）：一句一单元——延迟可接受，断句不可接受。**
//! 一个句子必须完整地交给 TTS 合成、再连续播放。因此本模块的正常路径**只按真实
//! 句读边界**切分，绝不按字符位置把一句话劈开。
//!
//! 性质：
//! - 输出句子保留原文与标点（所有输出拼接 + 残余 == 输入，逐字不丢）；
//! - 省略号/连续标点折叠为一个句界，永不产出空句；
//! - ASCII `.` 的保守规则：两侧均为数字（小数，如 `3.14`）或紧跟小写字母
//!   （缩写续词，如 `e.g.this`）不切分——宁可不切也不误切，完整 NLP 分段
//!   属于上层职责；
//! - 缓冲字符数有上界（安全阀）：仅在「超过 `max_chars` 且一个真实句读都没有」
//!   时触发，触发时**优先在弱标点（逗号/顿号等）处断开**，实在没有才按位置硬切。
//!   切点只由缓冲内容决定，因此任意 delta 切割得到完全一致的结果；
//! - `flush` 收尾：把悬挂的终止符 run 与无标点残余作为最后一句输出。
//!
//! 关键不变量「任意 delta 切割得到完全一致的输出」由 [`SentenceAssembler::reduce`]
//! 的纯内容决定切分配合 [`SentenceAssembler::sentence_end_in_buf`] 的「需要前瞻时
//! 返回 `None`」共同保证。

/// 句终止符集合：中英文句读 + 换行（协议固定，不含分号/冒号等弱标点）。
fn is_terminator(c: char) -> bool {
    matches!(c, '.' | '?' | '!' | '。' | '！' | '？' | '…' | '\n')
}

/// 弱标点（自然停顿点）：顿号/逗号/分号/冒号 + 半角对应。
///
/// **只在安全阀触发时**用作切点（见 [`SentenceAssembler::reduce`]）：在这些位置
/// 断开听感上是「停顿」，不会像按字符位置硬切那样把一句话从中间劈开。
fn is_soft_break(c: char) -> bool {
    matches!(c, '，' | ',' | '、' | '；' | ';' | '：' | ':')
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
    /// 默认最大缓冲字符数：**安全阀**，不是切句目标。
    ///
    /// **2026-09-10 契约调整（用户裁决：一句一单元，延迟可接受、断句不可接受）**：
    /// 旧值 `48` 会把超过 48 字的长句**按字符位置硬切**，TTS 侧表现为一句被劈成
    /// 多个请求 → 每个请求各有一段合成延迟 → 播放时逐段空档，听感即「断断续续」。
    ///
    /// 现在默认 **200**：正常口语句长（中文约 10–40 字）永远碰不到这个上限，
    /// 因此**一句话恰好对应一次 TTS 请求**。只有在模型吐出无标点长文时才会触发，
    /// 且触发时优先在弱标点（逗号等）处断开（见 [`Self::reduce`]）。
    pub const DEFAULT_MAX_CHARS: usize = 200;

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
            if let Some(end) = self.sentence_end_in_buf(eof)
                && self.buf[..end].chars().count() <= self.max_chars
            {
                // 句子恒非空（至少含一个终止符），is_empty 分支仅为防御。
                let sentence: String = self.buf.drain(..end).collect();
                if !sentence.is_empty() {
                    out.push(sentence);
                }
                continue;
            }
            if self.buf.chars().count() > self.max_chars {
                // **安全阀**（2026-09-10 契约调整：一句一单元，不断句）。
                //
                // 正常路径**绝不**走到这里：只要出现真实句读（`。！？…`）就在上面
                // 按句提取。本分支只在「流式输入已超过 `max_chars` 却仍无任何句读」
                // 时触发——例如模型吐出一大段没有标点的文字。
                //
                // 触发时**优先在弱标点处断开**（逗号/顿号等，听感是自然停顿），
                // 确实一个弱标点都没有才退回按字符位置硬切。这样即使安全阀生效，
                // 也不会把一句话从中间劈成两半。
                let limit = self.byte_of_char(self.max_chars);
                let cut = self.last_soft_break_at_or_before(limit).unwrap_or(limit);
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

    /// `limit` 字节以内**最后一个**弱标点的**结束**字节下标（含该标点）。
    ///
    /// 用于安全阀切点：返回 `Some(end)` 表示可以在 `end` 处断开（标点归前一段）。
    /// 窗口恒为 `[0, limit]`——只依赖已到达的缓冲内容，因此「任意 delta 切割结果
    /// 一致」的不变量继续成立（与 [`Self::sentence_end_in_buf`] 同口径）。
    fn last_soft_break_at_or_before(&self, limit: usize) -> Option<usize> {
        self.buf
            .char_indices()
            .filter(|(i, c)| is_soft_break(*c) && *i + c.len_utf8() <= limit)
            .map(|(i, c)| i + c.len_utf8())
            .next_back()
            .filter(|end| *end > 0)
    }
}
