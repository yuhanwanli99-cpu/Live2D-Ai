//! **规则表**（2026-10-17，rc4）：从 `live2d-ai-mod-director` 迁入核心。
//!
//! 它是主链按句第二路的**失败回退**单一真源（[rule_cues_for_sentence]），
//! 也是预设 id 契约（[PRESET_IDS]）与情绪 / 意图纯函数推导的唯一真源。
//! director Mod 只做 `pub use` 兼容层（crate 已退出 `AVAILABLE_MOD_FACTORIES`）。
//!
//! # 依赖方向
//!
//! 展开后就是 runtime 的普通子模块：只依赖 `serde_json` 与
//! `crate::performance`（`PerformanceCue` / 问句判据），**不**依赖任何 Mod
//! crate——这是「第二路并入核心」的关键一环（runtime 不能反向依赖 director）。
//!
//! # 文件划分（rc4）
//!
//! - `decision.rs`：情绪 / 意图 / 词表打分 / 清洗（纯函数）；
//! - `presets.rs`：预设 id allowlist、映射表、规则 cue；
//! - 本文件只做模块装配与 `pub use`，保持 `performance::rules::*` 的稳定路径。

pub mod decision;
pub mod presets;

pub use decision::*;
pub use presets::*;

#[cfg(test)]
mod tests;
