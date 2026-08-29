//! 工具调用组装器：按 `index` 分组组装同轮多个工具调用的 name/arguments 分片。
//!
//! - `index` 是分组键（wire 缺省 0）；首次出现的顺序即网络顺序；
//! - `id` 取该 index 收到的第一个非空值（OpenAI 只在首个分片携带）；
//! - `name` / `arguments` 分片跨事件顺序拼接（arguments 标准行为就是分块到达）；
//! - [`ToolCallAssembler::seal_parseable`] 在「能证明某调用不再有新分片」的时机
//!   （新 index 出现、正文恢复、DONE）把已完整的调用取走；
//! - [`ToolCallAssembler::finish`] 对应 `[DONE]`/flush：全量结算——要么全部成功，
//!   要么返回类型化 [`IncompleteToolCalls`]（全有或全无，绝不静默丢调用）。
//!
//! 只接受最终 name == `live2d_perform_action` 并解析
//! [`Live2dPerformActionArgs`]；其余名称记为 [`IncompleteReason::UnsupportedTool`]。

use std::fmt;

use crate::tool::{Live2dPerformActionArgs, PERFORM_ACTION_TOOL_NAME};

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
