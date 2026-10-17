//! 兼容层（2026-10-17，rc4）：规则表已迁入核心
//! `live2d_ai_runtime::performance::rules`。
//!
//! 本文件只做 `pub use`，**旧路径 `crate::presets::*` 逐项仍可用**
//! （director 自己的测试 / host 的旧装配点都靠它过渡）。真实现在
//! `crates/live2d-ai-runtime/src/performance/rules/presets.rs`——**不要**在这里
//! 再抄一份 id 列表或映射表。
//!
//! director Mod 已退休（退出注册表、不再投递 cue），这些名字保留只为兼容；
//! 主链按句第二路直接调 runtime 的 rules。

pub use live2d_ai_runtime::performance::rules::presets::*;

#[cfg(test)]
#[path = "presets_tests.rs"]
mod tests;
