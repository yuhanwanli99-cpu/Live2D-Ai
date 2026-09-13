//! `live2d-ai-core` — 纯 reducer 状态机（零依赖、无 I/O、无 async）。
//!
//! 核心不变量（shell 可据此安全驱动真实世界）：
//! 1. **epoch 单一 owner**：只有根 [`State::epoch`]；只有 [`Event::StopRequested`]
//!    推进它（恰好 +1）。除 stop 外一切事件都携带产生它的 epoch，先过**根闸门**
//!    （不匹配当前代次 → [`DropReason::StaleEpoch`]，状态不变），再进各子规则——
//!    包括动作子系统（[`Event::Action`]）。这是防「ID 重用后迟到事件」的唯一权威闸门。
//! 2. **单主 turn / 多句**：至多一个活动 turn；首个句子经 [`Event::UserSubmitted`]
//!    开 turn，后续句子经 [`Event::SentenceAdded`] 追加进计划。
//! 3. **turn 终态双闩锁**（节点 A 裁决 P0-1/P0-3）：音频是 **turn 级事实**，
//!    不是逐块调度——不存在 `PlayAudio` 效果与 per-chunk 完成信号。turn 只有在
//!    「生成终态」与「播放终态」**同时**成立才结束：
//!    - 生成闩 [`GenerationOutcome`]：`Completed | Failed | Cancelled`
//!      （事实 [`Event::GenerationFinished`]，恰一次）；
//!    - 播放闩 [`PlaybackState`]：`NeverStarted → Playing → Drained`，
//!      或 `NeverStarted/Playing → Cleared`（事实 [`Event::PlaybackStarted`] /
//!      [`Event::PlaybackDrained`] / [`Event::PlaybackCleared`]）；
//!    - 双闩齐 → 恰好一个 [`Effect::TurnCompleted`] 后回 Idle。**生成结束 ≠ turn
//!      完成**：Completed 但仍在 Playing 时必须等 Drained，否则声卡尚未播完的
//!      内容会被截断。
//! 4. **单 active 动作**：动作子状态 [`ActionState`] 不持有 epoch；抢占是单一显式
//!    效果 [`ActionEffect::Transition`]，任何终止路径（release / interrupt / stop）
//!    恰产生一个 [`ActionEffect::End`]。文本规则 fallback 见 [`action::rule_fallback`]。
//! 5. **stop 原子性**：一次 stop 同时清空 turn 计划、动作，并推进 epoch 恰好一次；
//!    此后旧代次的迟到事件全部被闸门确定性隔离。播放侧清理由 shell 执行
//!    （[`Effect::StopPlayback`] 无条件发出——对空 ring 幂等）。
//! 6. **表演参数曲线**：[`performance`] 把六动作用纯函数采样为项目自有
//!    [`ParameterFrame`](performance::ParameterFrame)（零分配、确定性、限幅在
//!    Bai 常规安全范围），供未来 l2d adapter 做字段→参数 ID 映射。
//!
//! 本 crate 为纯逻辑；shell 负责把 [`Effect`] 翻译成真实播放/渲染动作。
//!
//! # 动作 / 表演子系统：**休眠**（2026-09-12，rc.2）
//!
//! [`action`] 与 [`performance`] **保留原样**（不变量 4 与 6 仍以它们为前提），
//! 但它们在产品路径上**没有任何驱动方**：
//!
//! - 桌面侧把 `RootEvent::Action` 送进 reducer 的**唯一**通道已整体删除
//!   （`SupervisorHandle::trigger_action` + supervisor 的 `action_rx` 分支）；
//! - 动作序列的唯一驱动方 `live2d-ai-mod-director` 已删除；
//! - `ModServices.action_tx` 仍在（Mod API 契约），但 host 注入的是**固定休眠
//!   sender**：请求只留一行 debug 日志并返回 `false`。
//!
//! 所以这两个模块是**惰性**的——没有东西会调用它们，也就不会误触发。
//!「谁休眠、为什么、谁能唤醒」的完整裁决见
//! `docs/architecture/core-chain-baseline.md` §3.2 与 `AGENTS.md`
//!「动作与表演的归属（休眠台账）」。
//!
//! 模块与再导出都标了 `#[doc(hidden)]`：**这不是遗漏，是归属声明**——它们不是
//! 对外能力面，而是等待被**显式**唤醒（或显式删除）的内部残余。rc.2 不做 feature
//! gate：那会动 Cargo feature 矩阵与 `cli/tests.rs` 的断言，归 rc.3。
//!
//! **待机生命体征与动作无关**：`IdleState` 在渲染面（`l2d-wasm-demo`），
//! 与 [performance] 的动作曲线是两套机制，删动作时绝不要连带删它。
//!
//! 许可：**AGPL-3.0-only**，以仓库根 `LICENSE` 为准。

#[doc(hidden)]
pub mod action;
#[doc(hidden)]
pub mod performance;

mod events;
mod ids;
mod reducer;
mod state;

#[cfg(test)]
mod core_tests;

#[doc(hidden)]
pub use action::{
    ActionCommand, ActionDropReason, ActionEffect, ActionId, ActionSource, ActionState,
    ModelCapabilities, SemanticAction, Strength, apply_action,
};
pub use events::{DropReason, Effect, Event, GenerationOutcome};
pub use ids::{Epoch, SentenceId, TurnId};
#[doc(hidden)]
pub use performance::{ParameterFrame, ParameterMask, PerformancePlayer, SampleStatus};
pub use state::{Phase, PlaybackState, State, Turn};

// 自由函数（reducer 入口）必须留在 lib.rs 的最外层以保住 `live2d_ai_core::apply` 路径。
pub use reducer::apply;
