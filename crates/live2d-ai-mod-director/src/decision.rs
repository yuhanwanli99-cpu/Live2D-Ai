//! 兼容层（2026-10-17，rc4）：纯函数决策已迁入核心
//! `live2d_ai_runtime::performance::rules::decision`。
//!
//! 本文件只做 `pub use`，**旧路径 `crate::decision::*` 逐项仍可用**
//! （ledger 与既有测试都靠它）。真实现在
//! `crates/live2d-ai-runtime/src/performance/rules/decision.rs`——**不要**在这里
//! 再抄一份词表或推导逻辑。

pub use live2d_ai_runtime::performance::rules::decision::*;
