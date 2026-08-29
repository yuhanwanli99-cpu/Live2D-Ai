//! 对话流编排器：组合 [`SentenceAssembler`] + [`ToolCallAssembler`]，
//! 输出 [`DialogueEvent`]，并保证文本与工具的相对顺序 = 网络到达顺序。
//!
//! - 正文恢复（`TextDelta`）或新工具 index 出现，说明之前挂起的调用不会再有
//!   新分片 → 先封存它们，再处理当前事件，`ToolReady` 因此插在正确位置；
//! - `Done` / [`DialogueAssembler::flush`] 收尾：先吐完残余句子与已完成调用，
//!   再发恰一次 `Done`（携带类型化的残缺调用报告，若有）。
//!
//! `Done`/`flush` 之后进入终态，后续 `push` 一律忽略（返回空）。

use crate::dialogue::sentence::SentenceAssembler;
use crate::dialogue::tool::ToolCallAssembler;
use crate::llm::LlmEvent;

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
        call: crate::dialogue::tool::ParsedToolCall,
    },
    /// 流结束（收到 `Done` 或调用 `flush` 兜底），每次流恰好一次；
    /// `incomplete_tools` 非 `None` 表示有调用残缺（类型化详情）。
    Done {
        /// 残缺调用的类型化错误；全部正常时为 `None`。
        incomplete_tools: Option<crate::dialogue::tool::IncompleteToolCalls>,
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
    /// 创建编排器。`sentence_max_chars` 透传给 [`SentenceAssembler`].
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
