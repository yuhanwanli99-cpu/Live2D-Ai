//! 对话流编排**纯逻辑**：把 [`LlmEvent`] 增量流整理成「完整句 / 完整工具调用 /
//! 结束」三类事件，供下游（TTS、字幕、动作分发）直接消费。
//!
//! 职责边界：
//! - **不发网络、不做 TTS、无锁无线程**——全部是内存状态机，输入什么切割粒度都行；
//! - 动作能力 gate 与优先级仲裁仍归 `live2d-ai-core`：本模块只校验工具名必须是
//!   `live2d_perform_action` 并把参数解析成 [`Live2dPerformActionArgs`]，
//!   **不判断**模型能不能做该动作、强度是否越界；
//! - 流结束时仍有残缺调用 → 类型化 [`tool::IncompleteToolCalls`]（列出每个坏调用的
//!   index/id 与原因），绝不静默丢弃。
//!
//! # 组成
//!
//! - [`sentence::SentenceAssembler`]：正文增量 → 完整句（含原标点）。中英文
//!   `.?!。！？…\n` 均为句界；省略号与连续标点折叠为**一个**边界、不产生空句；
//!   `.` 采用保守规则（`3.14` 这类小数、`e.g.this` 这类后随小写的缩写不切）；
//!   超过最大缓冲字符数时按纯位置强制切分兜底；`flush` 收尾。任意 delta 切割
//!   ⇒ 相同输出（见测试）。
//! - [`tool::ToolCallAssembler`]：按 `index` 分组组装同轮多个工具调用的
//!   name/arguments 分片；`finish`（对应 `[DONE]`/flush）全量结算，成功或
//!   类型化 Incomplete。
//! - [`orchestrator::DialogueAssembler`]：组合前两者，输出 [`orchestrator::DialogueEvent`]，
//!   并保证文本与工具的相对顺序 = 网络到达顺序。
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

pub mod orchestrator;
pub mod sentence;
pub mod tool;

pub use orchestrator::{DialogueAssembler, DialogueEvent};
pub use sentence::SentenceAssembler;
pub use tool::{
    IncompleteReason, IncompleteToolCall, IncompleteToolCalls, ParsedToolCall, ToolCallAssembler,
};

#[cfg(test)]
mod tests;
