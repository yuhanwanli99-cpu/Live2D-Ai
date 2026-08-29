//! LLM 工具 wire 参数 → core [`SemanticAction`] 的映射与**接受判定**（D11 裁决）。
//!
//! 职责：把 `live2d_perform_action` 的已解析参数翻译成动作调度输入，并给出
//! 「这个工具调用是否算被接受」的**唯一权威判定**——supervisor 据此决定本轮
//! 是否抑制规则 fallback：
//!
//! - **强度严格 1..=3，非法值直接拒绝、绝不 clamp**（D11 原文）；
//!   协议外强度意味着上游模型行为异常，静默修正会掩盖问题；
//! - 动作名映射是纯枚举转换（wire [`PerformAction`] ↔ core [`ActionId`]，
//!   两边条目一一对应，编译期锁定）；
//! - capability gate / 优先级仲裁仍归 core reducer（本模块不做第二裁判）；
//!   「被接受」= 本模块通过 **且** root 对 Play 产出了 Start/Transition 效果。

use live2d_ai_core::{ActionId, SemanticAction, Strength};
use live2d_ai_runtime::tool::{Live2dPerformActionArgs, PerformAction};

/// wire 调用被拒绝的原因（可观测；调用方负责日志/计数）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolReject {
    /// 强度不在协议范围 1..=3（拒绝，不 clamp）。
    IllegalStrength(u8),
}

impl std::fmt::Display for ToolReject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IllegalStrength(v) => write!(f, "工具强度 {v} 超出协议范围 1..=3（拒绝执行）"),
        }
    }
}

/// 把 wire 参数映射为语义动作；非法强度返回拒绝原因。
///
/// 映射表（wire → core）：Nod→Nod、ShakeNo→ShakeNo、Tilt→Tilt、
/// LookAround→LookAround、Listen→Listen、Surprise→Surprise。
/// 来源固定为 [`ActionSource::LlmTool`](live2d_ai_core::ActionSource)：
/// 它的优先级高于 RuleFallback（D12 抢占规则的既有出处）。
pub fn wire_to_semantic(args: Live2dPerformActionArgs) -> Result<SemanticAction, ToolReject> {
    // D11：非法强度拒绝且不 clamp——静默修正会鼓励上游继续越界。
    let strength = match args.strength {
        1 => Strength::Low,
        2 => Strength::Medium,
        3 => Strength::High,
        other => return Err(ToolReject::IllegalStrength(other)),
    };
    let action = match args.action {
        PerformAction::Nod => ActionId::Nod,
        PerformAction::ShakeNo => ActionId::ShakeNo,
        PerformAction::Tilt => ActionId::Tilt,
        PerformAction::LookAround => ActionId::LookAround,
        PerformAction::Listen => ActionId::Listen,
        PerformAction::Surprise => ActionId::Surprise,
    };
    Ok(SemanticAction::new(
        action,
        strength,
        live2d_ai_core::ActionSource::LlmTool,
    ))
}
