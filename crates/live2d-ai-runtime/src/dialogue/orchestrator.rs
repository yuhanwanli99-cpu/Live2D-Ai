//! 对话流编排器：在 [`SentenceAssembler`] 之上输出 [`DialogueEvent`]。
//!
//! 2026-09-11 用户裁决：LLM 不暴露任何工具、只做对话，因此编排器不再持有
//! 工具调用组装器，输出的对话事件只剩「完整句」与「结束」两类——
//! 文本相对顺序 = 到达顺序这一点不受影响（本就只有一个来源）。
//!
//! `Done` / [`DialogueAssembler::flush`] 之后进入终态，后续 `push` 一律忽略
//! （返回空）。

use crate::dialogue::sentence::SentenceAssembler;
use crate::llm::LlmEvent;

/// 编排输出的对话事件（纯逻辑，不含任何 IO）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogueEvent {
    /// 一句完整文本（保留原标点），可直接交给 TTS/字幕。
    SentenceReady {
        /// 句子全文（含标点）。
        text: String,
    },
    /// 流结束（收到 `Done` 或调用 `flush` 兜底），每次流恰好一次。
    Done,
}

/// 对话流编排器：[`SentenceAssembler`] 的状态机外壳。
///
/// 输入 [`LlmEvent`] 流，输出 [`DialogueEvent`] 流；
/// `Done` / [`Self::flush`] 收尾：先吐完残余句子，再发恰一次 `Done`。
#[derive(Debug)]
pub struct DialogueAssembler {
    sentences: SentenceAssembler,
    finished: bool,
}

impl DialogueAssembler {
    /// 创建编排器。`sentence_max_chars` 透传给 [`SentenceAssembler`].
    pub fn new(sentence_max_chars: usize) -> Self {
        Self {
            sentences: SentenceAssembler::new(sentence_max_chars),
            finished: false,
        }
    }

    /// 消费一个 LLM 事件，返回因此产生的对话事件（可能为空）。
    pub fn push(&mut self, event: &LlmEvent) -> Vec<DialogueEvent> {
        if self.finished {
            return Vec::new();
        }
        match event {
            LlmEvent::TextDelta(delta) => self
                .sentences
                .push(delta)
                .into_iter()
                .map(|text| DialogueEvent::SentenceReady { text })
                .collect(),
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

    /// 收尾序列：残余句子 → 恰一次 Done。
    fn end(&mut self) -> Vec<DialogueEvent> {
        self.finished = true;
        let mut out: Vec<DialogueEvent> = self
            .sentences
            .flush()
            .into_iter()
            .map(|text| DialogueEvent::SentenceReady { text })
            .collect();
        out.push(DialogueEvent::Done);
        out
    }
}
