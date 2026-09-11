//! 对话流编排**纯逻辑**：把 [`LlmEvent`] 增量流整理成「完整句 / 结束」两类事件，
//! 供下游（TTS、字幕）直接消费。
//!
//! 职责边界：
//! - **不发网络、不做 TTS、无锁无线程**——全部是内存状态机，输入什么切割粒度都行；
//! - 动作能力 gate 与优先级仲裁归 `live2d-ai-core`，但**本模块不再解析任何工具
//!   调用**（2026-09-11 用户裁决：LLM 不暴露任何工具，只做对话）。
//!
//! # 组成
//!
//! - [`sentence::SentenceAssembler`]：正文增量 → 完整句（含原标点）。中英文
//!   `.?!。！？…\n` 均为句界；省略号与连续标点折叠为**一个**边界、不产生空句；
//!   `.` 采用保守规则（`3.14` 这类小数、`e.g.this` 这类后随小写的缩写不切）；
//!   超过最大缓冲字符数时按纯位置强制切分兜底；`flush` 收尾。任意 delta 切割
//!   ⇒ 相同输出（见测试）。
//! - [`orchestrator::DialogueAssembler`]：在句子组装之上输出
//!   [`orchestrator::DialogueEvent`]，并保证流结束时恰好一个 `Done`。
//!
//! # 用法
//!
//! ```
//! use live2d_ai_runtime::dialogue::{DialogueAssembler, DialogueEvent};
//! use live2d_ai_runtime::LlmEvent;
//!
//! let mut d = DialogueAssembler::new(64);
//! let mut out = Vec::new();
//! out.extend(d.push(&LlmEvent::TextDelta("你好呀！今".into())));
//! out.extend(d.push(&LlmEvent::TextDelta("天天气不错。".into())));
//! out.extend(d.flush());
//!
//! assert_eq!(
//!     out,
//!     vec![
//!         DialogueEvent::SentenceReady { text: "你好呀！".into() },
//!         DialogueEvent::SentenceReady { text: "今天天气不错。".into() },
//!         DialogueEvent::Done,
//!     ]
//! );
//! ```

pub mod orchestrator;
pub mod sentence;

pub use orchestrator::{DialogueAssembler, DialogueEvent};
pub use sentence::SentenceAssembler;

#[cfg(test)]
mod tests;
