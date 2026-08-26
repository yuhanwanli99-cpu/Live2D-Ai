//! 对话流编排**纯逻辑**：把 [`LlmEvent`] 增量流整理成「完整句 / 完整工具调用 /
//! 结束」三类事件，供下游（TTS、字幕、动作分发）直接消费。
//!
//! 职责边界：
//! - **不发网络、不做 TTS、无锁无线程**——全部是内存状态机，输入什么切割粒度都行；
//! - 动作能力 gate 与优先级仲裁仍归 `live2d-ai-core`：本模块只校验工具名必须是
//!   `live2d_perform_action` 并把参数解析成 [`Live2dPerformActionArgs`]，
//!   **不判断**模型能不能做该动作、强度是否越界；
//! - 流结束时仍有残缺调用 → 类型化 [`IncompleteToolCalls`]（列出每个坏调用的
//!   index/id 与原因），绝不静默丢弃。
//!
//! # 组成
//!
//! - [`SentenceAssembler`]：正文增量 → 完整句（含原标点）。中英文 `.?!。！？…\n`
//!   均为句界；省略号与连续标点折叠为**一个**边界、不产生空句；`.` 采用保守规则
//!   （`3.14` 这类小数、`e.g.this` 这类后随小写的缩写不切）；超过最大缓冲字符数时
//!   按纯位置强制切分兜底；`flush` 收尾。任意 delta 切割 ⇒ 相同输出（见测试）。
//! - [`ToolCallAssembler`]：按 `index` 分组组装同轮多个工具调用的 name/arguments
//!   分片；`finish`（对应 `[DONE]`/flush）全量结算，成功或类型化 Incomplete。
//! - [`DialogueAssembler`]：组合前两者，输出 [`DialogueEvent`]，并保证文本与工具
//!   的相对顺序 = 网络到达顺序。
//!
//! # 用法
//!
//! ```
//! use live2d_ai_runtime::dialogue::{DialogueAssembler, DialogueEvent};
//! use live2d_ai_runtime::{LlmEvent, PerformAction};
//!
//! let mut d = DialogueAssembler::new(64);
//! let mut out = Vec::new();
//! out.extend(d.push(&LlmEvent::TextDelta("你好呀！今".into())));
//! out.extend(d.push(&LlmEvent::TextDelta("天天气不错。".into())));
//! out.extend(d.push(&LlmEvent::ToolCallDelta {
//!     index: 0,
//!     id: Some("call_1".into()),
//!     name: Some("live2d_perform_action".into()),
//!     arguments: "{\"action\":\"nod\",\"strength\":2}".into(),
//! }));
//! out.extend(d.flush());
//!
//! assert_eq!(
//!     out,
//!     vec![
//!         DialogueEvent::SentenceReady { text: "你好呀！".into() },
//!         DialogueEvent::SentenceReady { text: "今天天气不错。".into() },
//!         DialogueEvent::ToolReady { call: live2d_ai_runtime::dialogue::ParsedToolCall {
//!             index: 0,
//!             id: Some("call_1".into()),
//!             arguments: live2d_ai_runtime::Live2dPerformActionArgs {
//!                 action: PerformAction::Nod,
//!                 strength: 2,
//!                 reason: None,
//!             },
//!         }},
//!         DialogueEvent::Done { incomplete_tools: None },
//!     ]
//! );
//! ```

use std::fmt;

use crate::llm::LlmEvent;
use crate::tool::{Live2dPerformActionArgs, PERFORM_ACTION_TOOL_NAME};

// ---------------------------------------------------------------- 句子组装

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
    /// [`DialogueAssembler`] 在流切换到工具调用事件时也会调用本方法
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

// ---------------------------------------------------------------- 工具调用组装

/// 一个已完成组装并通过校验的工具调用：名称必为 `live2d_perform_action`，
/// 参数已解析为 wire 类型。**不做**能力/优先级判定（归 core）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedToolCall {
    /// 该调用在本次响应内的流式下标。
    pub index: usize,
    /// 服务端调用 id（wire 未携带时为 `None`）。
    pub id: Option<String>,
    /// 解析后的动作参数。
    pub arguments: Live2dPerformActionArgs,
}

/// 单个调用未能完成结算的原因（类型化，便于上游分类处理/上报）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncompleteReason {
    /// 直到流结束都没收到任何 name 片段。
    MissingName,
    /// 名称合规但 arguments 一个片段都没到。
    MissingArguments,
    /// arguments 拼接完仍不是合法 JSON（典型：片段丢失/被截断）。
    InvalidArgumentsJson(String),
    /// 最终 name 不是 `live2d_perform_action`。
    UnsupportedTool(String),
}

impl fmt::Display for IncompleteReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingName => write!(f, "直到流结束都没有收到 name 片段"),
            Self::MissingArguments => write!(f, "arguments 为空"),
            Self::InvalidArgumentsJson(m) => write!(f, "arguments 不是合法 JSON: {m}"),
            Self::UnsupportedTool(name) => write!(
                f,
                "工具名 {name:?} 不被支持（仅接受 {PERFORM_ACTION_TOOL_NAME}）"
            ),
        }
    }
}

/// 一个未完成结算的调用（定位信息 + 原因）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncompleteToolCall {
    /// 流式下标。
    pub index: usize,
    /// 服务端调用 id（若收到过）。
    pub id: Option<String>,
    /// 未完成原因。
    pub reason: IncompleteReason,
}

impl fmt::Display for IncompleteToolCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "#[index={} id={:?}] {}",
            self.index, self.id, self.reason
        )
    }
}

/// `[DONE]`/flush 时的类型化「未完成」错误：按首现顺序列出所有未能结算的调用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncompleteToolCalls {
    /// 全部问题调用（保持首现顺序）。
    pub calls: Vec<IncompleteToolCall>,
}

impl fmt::Display for IncompleteToolCalls {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} 个工具调用未完成: ", self.calls.len())?;
        let joined = self
            .calls
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        f.write_str(&joined)
    }
}

impl std::error::Error for IncompleteToolCalls {}

/// 待组装中的一个调用（name/arguments 分片持续追加）。
#[derive(Debug, Default, Clone)]
struct PendingCall {
    index: usize,
    id: Option<String>,
    name: String,
    arguments: String,
}

/// 按 `index` 组装同轮多个工具调用的状态机。
///
/// - `index` 是分组键（wire 缺省 0）；首次出现的顺序即网络顺序；
/// - `id` 取该 index 收到的第一个非空值（OpenAI 只在首个分片携带）；
/// - `name` / `arguments` 分片跨事件顺序拼接（arguments 标准行为就是分块到达）；
/// - [`Self::seal_parseable`] 在「能证明某调用不再有新分片」的时机（新 index
///   出现、正文恢复、DONE）把已完整的调用取走；
/// - [`Self::finish`] 对应 `[DONE]`/flush：全量结算——要么全部成功，
///   要么返回类型化 [`IncompleteToolCalls`]（全有或全无，绝不静默丢调用）。
///
/// 只接受最终 name == `live2d_perform_action` 并解析
/// [`Live2dPerformActionArgs`]；其余名称记为 [`IncompleteReason::UnsupportedTool`]。
#[derive(Debug, Clone, Default)]
pub struct ToolCallAssembler {
    pending: Vec<PendingCall>,
}

impl ToolCallAssembler {
    /// 创建空的组装器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 队列里还有待结算的调用吗？
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// 累积一个增量分片（纯累积，无输出）。
    pub fn push(&mut self, index: usize, id: Option<&str>, name: Option<&str>, arguments: &str) {
        let call = self.entry(index);
        if call.id.is_none() {
            call.id = id.filter(|s| !s.is_empty()).map(str::to_owned);
        }
        if let Some(name) = name {
            call.name.push_str(name);
        }
        call.arguments.push_str(arguments);
    }

    /// 取走所有「参数已是完整合法 JSON 且通过名称校验」的调用（保持首现顺序）；
    /// 其余留在队列里，等 [`Self::finish`] 报类型化错误。
    pub fn seal_parseable(&mut self) -> Vec<ParsedToolCall> {
        let (ready, kept) =
            self.pending
                .drain(..)
                .fold((Vec::new(), Vec::new()), |(mut ready, mut kept), call| {
                    match Self::settle(&call) {
                        Ok(parsed) => ready.push(parsed),
                        Err(_) => kept.push(call),
                    }
                    (ready, kept)
                });
        self.pending = kept;
        ready
    }

    /// `[DONE]`/flush 收尾：全量结算剩余调用。成功 → 按首现顺序的全部调用；
    /// 有任何一个残缺 → [`Err(IncompleteToolCalls)`]（列出全部问题，含已成功的
    /// 也不会返回——先调 [`Self::seal_parseable`] 可在流中逐步取走完成的调用）。
    pub fn finish(mut self) -> Result<Vec<ParsedToolCall>, IncompleteToolCalls> {
        let mut ready = Vec::new();
        let mut broken = Vec::new();
        for call in self.pending.drain(..) {
            match Self::settle(&call) {
                Ok(parsed) => ready.push(parsed),
                Err(reason) => broken.push(IncompleteToolCall {
                    index: call.index,
                    id: call.id,
                    reason,
                }),
            }
        }
        if broken.is_empty() {
            Ok(ready)
        } else {
            Err(IncompleteToolCalls { calls: broken })
        }
    }

    /// 按 index 取（或建）待组装条目。
    fn entry(&mut self, index: usize) -> &mut PendingCall {
        if let Some(pos) = self.pending.iter().position(|c| c.index == index) {
            return &mut self.pending[pos];
        }
        self.pending.push(PendingCall {
            index,
            ..PendingCall::default()
        });
        self.pending.last_mut().expect("just pushed")
    }

    /// 结算单个调用：名称校验 + 参数解析。
    fn settle(call: &PendingCall) -> Result<ParsedToolCall, IncompleteReason> {
        if call.name.is_empty() {
            return Err(IncompleteReason::MissingName);
        }
        if call.name != PERFORM_ACTION_TOOL_NAME {
            return Err(IncompleteReason::UnsupportedTool(call.name.clone()));
        }
        if call.arguments.is_empty() {
            return Err(IncompleteReason::MissingArguments);
        }
        serde_json::from_str(&call.arguments)
            .map(|arguments| ParsedToolCall {
                index: call.index,
                id: call.id.clone(),
                arguments,
            })
            .map_err(|e| IncompleteReason::InvalidArgumentsJson(e.to_string()))
    }
}

// ---------------------------------------------------------------- 对话编排

/// 编排输出的对话事件（纯逻辑，不含任何 IO）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogueEvent {
    /// 一句完整文本（保留原标点），可直接交给 TTS/字幕。
    SentenceReady {
        /// 句子全文（含标点）。
        text: String,
    },
    /// 一个完整工具调用（名称校验通过、参数已解析）。
    ToolReady {
        /// 已解析的调用。
        call: ParsedToolCall,
    },
    /// 流结束（收到 `Done` 或调用 `flush` 兜底），每次流恰好一次；
    /// `incomplete_tools` 非 `None` 表示有调用残缺（类型化详情）。
    Done {
        /// 残缺调用的类型化错误；全部正常时为 `None`。
        incomplete_tools: Option<IncompleteToolCalls>,
    },
}

/// 对话流编排器：[`SentenceAssembler`] + [`ToolCallAssembler`] 的组合状态机。
///
/// 输入 [`LlmEvent`] 流，输出 [`DialogueEvent`] 流，并保证**文本与工具的相对
/// 顺序 = 网络到达顺序**：
/// - 正文恢复（`TextDelta`）或新工具 index 出现，说明之前挂起的调用不会再有
///   新分片 → 先封存它们，再处理当前事件，`ToolReady` 因此插在正确位置；
/// - `Done` / [`Self::flush`] 收尾：先吐完残余句子与已完成调用，再发恰一次
///   `Done`（携带类型化的残缺调用报告，若有）。
///
/// `Done`/`flush` 之后进入终态，后续 `push` 一律忽略（返回空）。
#[derive(Debug)]
pub struct DialogueAssembler {
    sentences: SentenceAssembler,
    tools: ToolCallAssembler,
    last_tool_index: Option<usize>,
    finished: bool,
}

impl DialogueAssembler {
    /// 创建编排器。`sentence_max_chars` 透传给 [`SentenceAssembler`]。
    pub fn new(sentence_max_chars: usize) -> Self {
        Self {
            sentences: SentenceAssembler::new(sentence_max_chars),
            tools: ToolCallAssembler::new(),
            last_tool_index: None,
            finished: false,
        }
    }

    /// 消费一个 LLM 事件，返回因此产生的对话事件（可能为空）。
    pub fn push(&mut self, event: &LlmEvent) -> Vec<DialogueEvent> {
        if self.finished {
            return Vec::new();
        }
        match event {
            LlmEvent::TextDelta(delta) => {
                // 正文恢复：之前挂起的调用不可能再有分片了，先封存保序。
                let mut out = self.ready_events();
                for sentence in self.sentences.push(delta) {
                    out.push(DialogueEvent::SentenceReady { text: sentence });
                }
                out
            }
            LlmEvent::ToolCallDelta {
                index,
                id,
                name,
                arguments,
            } => {
                let mut out = Vec::new();
                // 正文不再增长：先把悬挂的句界封口，保住文本 vs 工具的网络顺序。
                for sentence in self.sentences.seal() {
                    out.push(DialogueEvent::SentenceReady { text: sentence });
                }
                if self.last_tool_index.is_some_and(|prev| prev != *index) {
                    // 新调用开始：封存之前可结算的调用（网络顺序）。
                    out.extend(self.ready_events());
                }
                self.tools
                    .push(*index, id.as_deref(), name.as_deref(), arguments);
                self.last_tool_index = Some(*index);
                out
            }
            LlmEvent::Done => self.end(),
        }
    }

    /// 对端未发 `[DONE]` 就关流的兜底收尾（幂等；终态后返回空）。
    pub fn flush(&mut self) -> Vec<DialogueEvent> {
        if self.finished {
            Vec::new()
        } else {
            self.end()
        }
    }

    /// 收尾序列：残余句子 → 已完成调用 → 恰一次 Done（带类型化残缺报告）。
    fn end(&mut self) -> Vec<DialogueEvent> {
        self.finished = true;
        let mut out = self.ready_events();
        for sentence in self.sentences.flush() {
            out.push(DialogueEvent::SentenceReady { text: sentence });
        }
        out.extend(self.ready_events());
        let incomplete_tools = std::mem::take(&mut self.tools).finish().err();
        out.push(DialogueEvent::Done { incomplete_tools });
        out
    }

    /// 取走当前可结算的调用并包装成事件。
    fn ready_events(&mut self) -> Vec<DialogueEvent> {
        self.tools
            .seal_parseable()
            .into_iter()
            .map(|call| DialogueEvent::ToolReady { call })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::PerformAction;

    // ------------------------------------------------------------ 工具

    fn sentences_with(max: usize, input: &str) -> Vec<String> {
        let mut s = SentenceAssembler::new(max);
        let mut out = s.push(input);
        out.extend(s.flush());
        out
    }

    /// 确定性伪随机数（xorshift64），避免引入 rand 依赖。
    struct XorShift(u64);

    impl XorShift {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
    }

    /// 断言：整段喂入的结果 == 任意随机切割喂入的结果，且逐字无损、无空句。
    fn assert_splitting_invariant(input: &str, max: usize) {
        let baseline = sentences_with(max, input);
        assert!(
            baseline.iter().all(|s| !s.is_empty()),
            "不允许空句: {baseline:?}"
        );
        assert_eq!(
            baseline.iter().map(String::as_str).collect::<String>(),
            input,
            "输出必须逐字保留原文"
        );
        for seed in 1..=32u64 {
            let mut rng = XorShift(seed | 1);
            let mut s = SentenceAssembler::new(max);
            let mut got = Vec::new();
            let mut rest = input;
            while !rest.is_empty() {
                // 每步随机吃 1..=7 个字符（按字符边界）。
                let take = (rng.next() % 7 + 1) as usize;
                let take = take.min(rest.chars().count());
                let split: usize = rest.char_indices().nth(take).map_or(rest.len(), |(i, _)| i);
                let (head, tail) = rest.split_at(split);
                got.extend(s.push(head));
                rest = tail;
            }
            got.extend(s.flush());
            assert_eq!(got, baseline, "seed={seed} 切割结果不一致");
        }
    }

    #[test]
    fn chinese_sentences_keep_punctuation() {
        assert_eq!(
            sentences_with(64, "你好呀！今天天气不错。我们出去走走吧？好啊。"),
            ["你好呀！", "今天天气不错。", "我们出去走走吧？", "好啊。"]
        );
    }

    #[test]
    fn english_sentences_split_on_punctuation() {
        // 句间空格原样保留在下一句开头（不吞任何字符）。
        assert_eq!(
            sentences_with(64, "Hello world! How are you? I'm fine."),
            ["Hello world!", " How are you?", " I'm fine."]
        );
    }

    #[test]
    fn newline_is_a_boundary() {
        assert_eq!(
            sentences_with(64, "第一行\n第二行\n第三行无换行"),
            ["第一行\n", "第二行\n", "第三行无换行"]
        );
    }

    #[test]
    fn ellipsis_and_consecutive_punctuation_never_make_empty_sentences() {
        // …… / 。。。 / ?! 都折叠成一个句界，且不产生空句。
        assert_eq!(sentences_with(64, "等等……好。"), ["等等……", "好。"]);
        assert_eq!(
            sentences_with(64, "想想。。。行吧！"),
            ["想想。。。", "行吧！"]
        );
        assert_eq!(sentences_with(64, "什么?!真的。"), ["什么?!", "真的。"]);
        // 整个输入只有标点：不产出空串；残余在 flush 原样输出（逐字保留）。
        assert_eq!(sentences_with(64, "。\n\n"), ["。\n\n"]);
        let mut s = SentenceAssembler::new(8);
        assert!(s.push("……").is_empty());
        assert_eq!(s.buffered_chars(), 2);
    }

    #[test]
    fn decimals_are_conservatively_not_split() {
        // 中英文小数都不切。
        assert_eq!(
            sentences_with(64, "圆周率约3.14，很好。"),
            ["圆周率约3.14，很好。"]
        );
        assert_eq!(
            sentences_with(64, "pi is 3.14159 ok? yes."),
            ["pi is 3.14159 ok?", " yes."]
        );
        // 「3.」结尾（可能是没写完的小数）也保守不切，flush 时原样保留。
        assert_eq!(sentences_with(64, "值约为3."), ["值约为3."]);
    }

    #[test]
    fn lowercase_following_period_is_conservatively_not_split() {
        // 缩写续词（后随小写字母）不切；正常句界（大写/中文/空白结尾）照切。
        assert_eq!(
            sentences_with(64, "send to mr.smith now."),
            ["send to mr.smith now."]
        );
        assert_eq!(
            sentences_with(64, "use e.g.this thing. Done."),
            ["use e.g.this thing.", " Done."]
        );
    }

    #[test]
    fn arbitrary_delta_splitting_is_consistent() {
        let corpus = [
            "你好呀！今天天气不错。我们出去走走吧？好啊。",
            "等等……让我想想。。。好吧！什么?!来吧。",
            "Hello world! How are you? I'm fine. Me too.",
            "圆周率约3.14，pi=3.14159。see e.g.notes and mr.smith soon. Done.",
            "第一行\n第二行\n第三行无换行",
            "无标点的很长中文输入会在缓冲上限处被强制切开但结果必须稳定",
        ];
        for text in corpus {
            for max in [1usize, 3, 8, 48] {
                assert_splitting_invariant(text, max);
            }
        }
    }

    #[test]
    fn max_buffer_bounds_pending_and_cuts_deterministically() {
        let text = "abcdefghijklmnopqrstuvwxyz"; // 26 字符，无句界
        let mut s = SentenceAssembler::new(10);
        let mut out = s.push(text);
        out.extend(s.flush());
        assert_eq!(out, ["abcdefghij", "klmnopqrst", "uvwxyz"]);
        // 缓冲有界：push 之后永远 ≤ 上限。
        let mut s = SentenceAssembler::new(5);
        let mut rng = XorShift(7);
        let mut rest = text;
        while !rest.is_empty() {
            let take = ((rng.next() % 4 + 1) as usize).min(rest.chars().count());
            let split = rest.char_indices().nth(take).map_or(rest.len(), |(i, _)| i);
            s.push(&rest[..split]);
            assert!(s.buffered_chars() <= 5);
            rest = &rest[split..];
        }
        // 多字节字符不被劈开：强制切点落在字符边界上。
        let wide = "一二三四五六七八九十";
        assert_eq!(
            sentences_with(3, wide),
            ["一二三", "四五六", "七八九", "十"]
        );
    }

    #[test]
    fn flush_resolves_hanging_terminators_and_tail() {
        let mut s = SentenceAssembler::new(16);
        assert!(s.push("还没说完").is_empty());
        assert_eq!(s.flush(), ["还没说完"]);
        // flush 幂等且之后可复用。
        assert!(s.flush().is_empty());
        assert_eq!(s.push("第二句。tail"), ["第二句。"]);
        assert_eq!(s.flush(), ["tail"]);

        // 行尾悬挂的「。」由 flush 封口。
        let mut s = SentenceAssembler::new(16);
        assert!(s.push("你好。").is_empty()); // run 悬挂，等下一个字符
        assert_eq!(s.push("再见"), ["你好。"]); // 「再见」证明 run 结束
        assert_eq!(s.flush(), ["再见"]);
    }

    // ------------------------------------------------------------ 工具调用

    const NAME: &str = "live2d_perform_action";

    fn nod_args(strength: u8) -> Live2dPerformActionArgs {
        Live2dPerformActionArgs {
            action: PerformAction::Nod,
            strength,
            reason: None,
        }
    }

    #[test]
    fn multiple_calls_assembled_by_index_across_fragments() {
        let mut t = ToolCallAssembler::new();
        // 两个调用交错到达；name 也劈成两个分片。
        t.push(0, Some("call_a"), Some("live2d_perform"), "");
        t.push(0, None, Some("_action"), "{\"action\":\"nod\",");
        t.push(1, Some("call_b"), Some(NAME), "{\"action\":\"tilt\"");
        t.push(1, None, None, ",\"strength\":3}");

        // call_0 的参数还缺一截：封存只取走已完整的 call_1（首现顺序语义不变）。
        let ready = t.seal_parseable();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].index, 1);
        assert_eq!(ready[0].id.as_deref(), Some("call_b"));
        assert_eq!(ready[0].arguments.action, PerformAction::Tilt);
        assert_eq!(ready[0].arguments.strength, 3);

        // 补齐 call_0 后 DONE 收尾。
        t.push(0, None, None, "\"strength\":2}");
        assert!(!t.is_empty());
        let rest = t.finish().expect("call_0 应结算成功");
        assert_eq!(rest.len(), 1);
        assert_eq!(rest[0].index, 0);
        assert_eq!(rest[0].id.as_deref(), Some("call_a"));
        assert_eq!(rest[0].arguments, nod_args(2));
    }

    #[test]
    fn partial_arguments_are_not_sealed_until_complete() {
        let mut t = ToolCallAssembler::new();
        t.push(0, Some("c"), Some(NAME), "{\"action\":\"no");
        assert!(t.seal_parseable().is_empty());
        t.push(0, None, None, "d\",\"strength\":1}");
        let ready = t.seal_parseable();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].arguments, nod_args(1));
        assert!(t.is_empty());
    }

    #[test]
    fn missing_fragment_yields_typed_incomplete() {
        // 丢掉中间一段 arguments → JSON 不完整。
        let t = {
            let mut t = ToolCallAssembler::new();
            t.push(0, Some("c0"), Some(NAME), "{\"action\":\"nod\",");
            // 缺 ",\"strength\":2}"
            t
        };
        let err = t.finish().expect_err("应报类型化 Incomplete");
        assert_eq!(err.calls.len(), 1);
        assert_eq!(err.calls[0].index, 0);
        assert_eq!(err.calls[0].id.as_deref(), Some("c0"));
        assert!(matches!(
            err.calls[0].reason,
            IncompleteReason::InvalidArgumentsJson(_)
        ));
        assert!(err.to_string().contains("1 个工具调用未完成"));
    }

    #[test]
    fn missing_name_and_empty_arguments_are_typed() {
        let mut t = ToolCallAssembler::new();
        t.push(0, None, None, "{}");
        t.push(1, Some("c1"), Some(NAME), "");
        let err = t.finish().expect_err("两个都残缺");
        assert_eq!(err.calls.len(), 2);
        assert_eq!(err.calls[0].reason, IncompleteReason::MissingName);
        assert_eq!(err.calls[1].reason, IncompleteReason::MissingArguments);
    }

    #[test]
    fn unsupported_tool_name_is_rejected() {
        let mut t = ToolCallAssembler::new();
        t.push(0, Some("x"), Some("search_web"), "{\"query\":\"hi\"}");
        let err = t.finish().expect_err("只接受 live2d_perform_action");
        assert_eq!(
            err.calls[0].reason,
            IncompleteReason::UnsupportedTool("search_web".to_string())
        );
        assert!(err.to_string().contains(NAME));
    }

    #[test]
    fn finish_returns_all_remaining_calls_in_first_seen_order() {
        let mut t = ToolCallAssembler::new();
        t.push(
            2,
            Some("c2"),
            Some(NAME),
            "{\"action\":\"listen\",\"strength\":1}",
        );
        t.push(
            0,
            Some("c0"),
            Some(NAME),
            "{\"action\":\"surprise\",\"strength\":3}",
        );
        let done = t.finish().expect("全部完整");
        assert_eq!(done.iter().map(|c| c.index).collect::<Vec<_>>(), [2, 0]);
    }

    #[test]
    fn wire_default_index_zero_groups_id_only_fragments() {
        // wire 层对缺 index 的分片补 0：id-only 分片也能把 id 关联到同一调用。
        let mut t = ToolCallAssembler::new();
        t.push(0, Some("only-id-chunk"), None, "");
        t.push(0, None, Some(NAME), "{\"action\":\"nod\",\"strength\":1}");
        let done = t.finish().expect("ok");
        assert_eq!(done[0].id.as_deref(), Some("only-id-chunk"));
    }

    // ------------------------------------------------------------ 对话编排

    fn tool_delta(index: usize, id: Option<&str>, arguments: &str) -> LlmEvent {
        LlmEvent::ToolCallDelta {
            index,
            id: id.map(str::to_owned),
            name: Some(NAME.to_string()),
            arguments: arguments.to_string(),
        }
    }

    fn sentence_texts(events: &[DialogueEvent]) -> Vec<String> {
        events
            .iter()
            .filter_map(|e| match e {
                DialogueEvent::SentenceReady { text } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn dialogue_keeps_network_order_between_text_and_tools() {
        let mut d = DialogueAssembler::new(64);
        let mut out = Vec::new();
        out.extend(d.push(&LlmEvent::TextDelta("好的。".into())));
        out.extend(d.push(&tool_delta(
            0,
            Some("t0"),
            "{\"action\":\"nod\",\"strength\":1}",
        )));
        // 新 index 出现 → 封存 call0（插在正确位置）。
        out.extend(d.push(&tool_delta(
            1,
            Some("t1"),
            "{\"action\":\"tilt\",\"strength\":2}",
        )));
        // 正文恢复 → 先封存 call1，再出句子。
        out.extend(d.push(&LlmEvent::TextDelta("完成！".into())));
        out.extend(d.flush());

        assert_eq!(sentence_texts(&out), ["好的。", "完成！"]);
        let tool_indexes: Vec<usize> = out
            .iter()
            .filter_map(|e| match e {
                DialogueEvent::ToolReady { call } => Some(call.index),
                _ => None,
            })
            .collect();
        assert_eq!(tool_indexes, [0, 1]);
        // 网络顺序：S(T0 前) → T0 → T1 → S(T1 后) → Done。
        assert!(matches!(
            out.last(),
            Some(DialogueEvent::Done {
                incomplete_tools: None
            })
        ));
        let seq: Vec<&str> = out
            .iter()
            .map(|e| match e {
                DialogueEvent::SentenceReady { .. } => "S",
                DialogueEvent::ToolReady { .. } => "T",
                DialogueEvent::Done { .. } => "D",
            })
            .collect();
        assert_eq!(seq, ["S", "T", "T", "S", "D"]);
    }

    #[test]
    fn dialogue_reports_incomplete_tools_at_done() {
        let mut d = DialogueAssembler::new(64);
        let mut out = d.push(&LlmEvent::TextDelta("点头".into()));
        out.extend(d.push(&tool_delta(0, Some("t0"), "{\"action\":\"nod\","))); // 故意截断
        out.extend(d.flush());
        assert_eq!(sentence_texts(&out), ["点头"]);
        match out.last() {
            Some(DialogueEvent::Done {
                incomplete_tools: Some(err),
            }) => {
                assert_eq!(err.calls[0].index, 0);
                assert!(matches!(
                    err.calls[0].reason,
                    IncompleteReason::InvalidArgumentsJson(_)
                ));
            }
            other => panic!("期望携带 Incomplete 的 Done，实际 {other:?}"),
        }
    }

    #[test]
    fn dialogue_flush_is_fallback_done_and_terminal() {
        let mut d = DialogueAssembler::new(64);
        d.push(&LlmEvent::TextDelta("还没说完".into()));
        let out = d.flush();
        assert_eq!(sentence_texts(&out), ["还没说完"]);
        assert!(matches!(
            out.last(),
            Some(DialogueEvent::Done {
                incomplete_tools: None
            })
        ));
        // 终态：flush/push 都不再产生事件。
        assert!(d.flush().is_empty());
        assert!(d.push(&LlmEvent::TextDelta("迟到的".into())).is_empty());
        assert!(d.push(&LlmEvent::Done).is_empty());
    }

    #[test]
    fn dialogue_explicit_done_matches_flush() {
        let mut d = DialogueAssembler::new(64);
        d.push(&tool_delta(0, None, "{\"action\":\"nod\",\"strength\":2}"));
        let out = d.push(&LlmEvent::Done);
        assert_eq!(out.len(), 2); // ToolReady + Done
        assert!(matches!(out[0], DialogueEvent::ToolReady { .. }));
        assert!(matches!(
            out[1],
            DialogueEvent::Done {
                incomplete_tools: None
            }
        ));
    }

    #[test]
    fn dialogue_tools_only_stream_keeps_order_without_text() {
        let mut d = DialogueAssembler::new(64);
        let mut out = Vec::new();
        out.extend(d.push(&tool_delta(
            0,
            Some("a"),
            "{\"action\":\"listen\",\"strength\":1}",
        )));
        // call_1 出现 → 封存 call_0。
        out.extend(d.push(&tool_delta(
            1,
            Some("b"),
            "{\"action\":\"surprise\",\"strength\":3}",
        )));
        out.extend(d.flush());
        let seq: Vec<&str> = out
            .iter()
            .map(|e| match e {
                DialogueEvent::ToolReady { .. } => "T",
                DialogueEvent::Done { .. } => "D",
                DialogueEvent::SentenceReady { .. } => "S",
            })
            .collect();
        assert_eq!(seq, ["T", "T", "D"]);
        assert!(
            out.iter()
                .take(2)
                .all(|e| matches!(e, DialogueEvent::ToolReady { .. }))
        );
    }

    #[test]
    fn dialogue_overflow_sentence_still_emits_without_punctuation() {
        let mut d = DialogueAssembler::new(4);
        let out = d.push(&LlmEvent::TextDelta("一二三四五六七".into()));
        assert_eq!(sentence_texts(&out), ["一二三四"]);
    }
}
