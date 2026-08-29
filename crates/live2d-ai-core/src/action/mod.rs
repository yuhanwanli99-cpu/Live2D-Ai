//! 语义动作核心：六个可播放动作 + 能力 gate + 单 active 调度子状态。
//!
//! 职责边界（收敛后）：
//! - **本模块不持有、不推进 [`crate::Epoch`]**：epoch 的唯一 owner 是根
//!   [`crate::State`]。表演命令经根 `Event::Action { epoch, command }` 进入，
//!   先过根 epoch 闸门，再由 [`apply_action`] 做同代次仲裁；
//! - [`ActionId`] 只含六个可播放动作；「释放/回中」是控制命令
//!   （[`ActionCommand::Release`]）与终止效果（[`ActionEffect::End`]），不是动作；
//! - 抢占是单一显式效果 [`ActionEffect::Transition { from, to }`]（shell 停旧起新，
//!   替代「停旧+回中+起新」三连）；真正的收尾/打断/停止才产生唯一的
//!   [`ActionEffect::End`]（shell 据此停下当前动作并回到中心姿态）；
//! - 确定性文本规则 fallback 在子模块 [`rules`](self::rules::rule_fallback)。
//!
//! v0 动作是离散表情/姿态指令，无连续参数曲线（留给渲染层）。

mod rules;

pub use rules::rule_fallback;

// ---------------------------------------------------------------- ActionId

/// v0 六个可播放动作（RFC D9）。数值即能力位集位索引（见 [`ModelCapabilities`]）。
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ActionId {
    /// 点头（同意/应和）。
    Nod = 0,
    /// 摇头（否定）。
    ShakeNo = 1,
    /// 歪头（疑惑/好奇）。
    Tilt = 2,
    /// 环视 / 视线游移。
    LookAround = 3,
    /// 倾听姿态。
    Listen = 4,
    /// 惊讶。
    Surprise = 5,
}

impl ActionId {
    /// 全部可播放动作（顺序即位索引）。
    pub const ALL: [Self; 6] = [
        Self::Nod,
        Self::ShakeNo,
        Self::Tilt,
        Self::LookAround,
        Self::Listen,
        Self::Surprise,
    ];

    /// 动作的稳定字符串名（协议名，与 RFC D9 表一致；不含 release——它不是动作）。
    pub const fn name(self) -> &'static str {
        match self {
            Self::Nod => "nod",
            Self::ShakeNo => "shake_no",
            Self::Tilt => "tilt",
            Self::LookAround => "look_around",
            Self::Listen => "listen",
            Self::Surprise => "surprise",
        }
    }

    /// 动作在能力位集中的位索引。
    const fn bit(self) -> u32 {
        // 枚举显式 repr(u8) 且判别式 0..=5，`as u32` 安全且确定。
        self as u32
    }
}

// ---------------------------------------------------------------- Strength

/// 动作强度三档，经 [`Strength::level`] 映射到协议数值 1..=3。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum Strength {
    /// 轻（1）：默认强度。
    #[default]
    Low,
    /// 中（2）。
    Medium,
    /// 强（3）。
    High,
}

impl Strength {
    /// 全部档位（低→高）。
    pub const ALL: [Self; 3] = [Self::Low, Self::Medium, Self::High];

    /// 映射到协议数值 1..=3（Low=1 / Medium=2 / High=3）。
    pub const fn level(self) -> u8 {
        match self {
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 3,
        }
    }

    /// 从协议数值 1..=3 解析；越界返回 `None`。
    pub const fn from_level(level: u8) -> Option<Self> {
        match level {
            1 => Some(Self::Low),
            2 => Some(Self::Medium),
            3 => Some(Self::High),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------- ActionSource

/// 动作指令的来源（决定调度优先级）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ActionSource {
    /// 用户直接命令（如手势/按钮/语音指令）：优先级 100，最高。
    UserCommand,
    /// 主 LLM 的 tool action 输出：优先级 90。
    LlmTool,
    /// 确定性规则 fallback：优先级 50，最低。
    RuleFallback,
}

impl ActionSource {
    /// 固定优先级：user=100 > tool=90 > rule=50。
    pub const fn priority(self) -> u32 {
        match self {
            Self::UserCommand => 100,
            Self::LlmTool => 90,
            Self::RuleFallback => 50,
        }
    }
}

// ---------------------------------------------------------------- SemanticAction

/// 一条完整的语义动作指令：做什么 + 多强 + 谁要求的。
///
/// 纯 Rust 输入结构（v0 不引 serde）：LLM tool / 用户命令 / 规则 fallback 都先构造
/// 本结构，再经根 `Event::Action` 进入调度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SemanticAction {
    /// 动作种类。
    pub action: ActionId,
    /// 强度。
    pub strength: Strength,
    /// 来源（决定优先级与可观测性）。
    pub source: ActionSource,
}

impl SemanticAction {
    /// 构造一条语义动作指令。
    pub const fn new(action: ActionId, strength: Strength, source: ActionSource) -> Self {
        Self {
            action,
            strength,
            source,
        }
    }
}

// ---------------------------------------------------------------- ModelCapabilities

/// 模型能力集合：当前模型支持哪些动作（u64 位集，v0 共 6 个动作）。
///
/// `Default` 为空集（**fail-closed**：未显式配置能力的模型任何动作都不允许）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ModelCapabilities {
    mask: u64,
}

impl ModelCapabilities {
    /// 空能力集（什么都不支持）。
    pub const EMPTY: Self = Self { mask: 0 };

    /// 支持全部 v0 动作。
    pub fn all() -> Self {
        let mut caps = Self::EMPTY;
        for action in ActionId::ALL {
            caps.add(action);
        }
        caps
    }

    /// 声明支持某个动作（就地修改）。
    pub fn add(&mut self, action: ActionId) {
        self.mask |= 1 << action.bit();
    }

    /// 返回添加了 `action` 的新集合（不改动 `self`，便于链式构建）。
    pub fn with(mut self, action: ActionId) -> Self {
        self.add(action);
        self
    }

    /// 模型是否支持该动作（capability gate 的判定本身）。
    pub const fn supports(self, action: ActionId) -> bool {
        self.mask & (1 << action.bit()) != 0
    }
}

// ---------------------------------------------------------------- 子状态 / 命令 / 效果

/// 动作子状态（挂在根 [`crate::State`] 下）。
///
/// 不变量：同一时刻至多一个动作在表演（单 active）；**不含 epoch**——
/// 代次校验由根 reducer 完成后才调用 [`apply_action`]。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActionState {
    /// 模型能力集合（fail-closed 默认空集；shell 初始化时应配置为实际能力）。
    pub capabilities: ModelCapabilities,
    /// 当前表演中的动作（无则 `None`）。
    pub current: Option<SemanticAction>,
}

/// 表演控制命令（作为根 `Event::Action { epoch, command }` 的载荷进入）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionCommand {
    /// 请求表演一个语义动作（经 capability gate 与优先级仲裁后可能被拒绝）。
    Play { action: SemanticAction },
    /// 强制打断当前动作（立刻停下并回中）。
    Interrupt,
    /// 正常收尾当前动作：停下并回中、回空闲。
    ///
    /// 仅用于**显式释放**意图。动作自然播完的收尾走根事实
    /// `Event::ActionPlaybackFinished { epoch, action }`——携带完成者身份，
    /// 防止被抢占后的迟到收尾误伤在演的后来者（P0-6；本命令无身份，不得复用）。
    Release,
}

/// 命令被丢弃的原因（供 shell 观测/日志）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionDropReason {
    /// 模型能力缺失：capability gate 拒绝（模型不支持该动作）。
    CapabilityMissing { action: SemanticAction },
    /// 当前动作的来源优先级更高，新指令被拒（如 rule fallback 不能抢断 LLM tool 动作）。
    LowerPriority {
        incoming: SemanticAction,
        active: SemanticAction,
    },
    /// 完全相同的语义动作已在表演中（幂等拒绝，状态不变）。
    AlreadyActive { action: SemanticAction },
}

/// 动作子系统输出给 shell 的渲染命令（以 [`crate::Effect::Action`] 从根冒泡）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionEffect {
    /// 空闲 → 表演：开始一个动作。
    Start { action: SemanticAction },
    /// 抢占换演：停掉 `from` 并立即开始 `to`（单一显式效果）。
    Transition {
        from: SemanticAction,
        to: SemanticAction,
    },
    /// 终止当前动作（正常收尾 / 打断 / 会话停止共用）：shell 停下该动作并回到中心姿态。
    End { action: SemanticAction },
    /// 命令被丢弃（附原因）。
    Dropped { reason: ActionDropReason },
}

// ---------------------------------------------------------------- 子 reducer

/// 动作子 reducer：把 `command` 应用到 `state`，返回渲染效果列表。
///
/// **前置条件（由根 reducer 保证）**：调用即表示命令所属 epoch 与根状态一致。
///
/// 判定顺序固定且确定：
/// 1. **capability gate**（仅 Play）：模型不支持 → `Dropped(CapabilityMissing)`；
/// 2. 完全相同指令在演 → `Dropped(AlreadyActive)`（幂等）；
/// 3. 当前来源优先级更高 → `Dropped(LowerPriority)`；
/// 4. 空闲 → [`ActionEffect::Start`]；否则抢占 → [`ActionEffect::Transition`]。
///
/// 不 panic、不 I/O；结果只依赖 `state` 与 `command` 自身。
pub fn apply_action(state: &mut ActionState, command: ActionCommand) -> Vec<ActionEffect> {
    match command {
        ActionCommand::Play { action } => play(state, action),
        // 收尾与打断的状态效果一致（都回到空闲+回中）；区分保留给 shell 观测/日志。
        ActionCommand::Interrupt | ActionCommand::Release => end_current(state),
    }
}

/// Play 分支：capability gate → 幂等 → 优先级仲裁 → 起播/抢占。
fn play(state: &mut ActionState, action: SemanticAction) -> Vec<ActionEffect> {
    // capability gate：模型不支持的动作直接拒绝（用户命令也不例外）。
    if !state.capabilities.supports(action.action) {
        return vec![ActionEffect::Dropped {
            reason: ActionDropReason::CapabilityMissing { action },
        }];
    }

    let Some(active) = state.current else {
        // 空闲：直接起播。
        state.current = Some(action);
        return vec![ActionEffect::Start { action }];
    };

    // 完全相同（动作+强度+来源）已在表演：幂等拒绝。
    if active == action {
        return vec![ActionEffect::Dropped {
            reason: ActionDropReason::AlreadyActive { action },
        }];
    }

    // 当前来源优先级严格更高：新指令被拒（rule 不能抢 tool，tool 不能抢 user）。
    if active.source.priority() > action.source.priority() {
        return vec![ActionEffect::Dropped {
            reason: ActionDropReason::LowerPriority {
                incoming: action,
                active,
            },
        }];
    }

    // 同优先级「后来者胜」/ 更高优先级「抢占」：单一显式换演效果。
    state.current = Some(action);
    vec![ActionEffect::Transition {
        from: active,
        to: action,
    }]
}

/// 收尾分支：有动作则终止（一个 End），空闲则无操作。
fn end_current(state: &mut ActionState) -> Vec<ActionEffect> {
    match state.current.take() {
        None => Vec::new(),
        Some(action) => vec![ActionEffect::End { action }],
    }
}

#[cfg(test)]
mod tests;
