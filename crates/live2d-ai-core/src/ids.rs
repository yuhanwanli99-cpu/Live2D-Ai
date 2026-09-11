//! 领域 id 新类型 + [`Epoch`] 单调代次计数器。
//!
//! 全部为 `Copy` 的零成本包装，外部使用既有的 `live2d_ai_core::{Epoch, TurnId,
//! SentenceId}` 路径不变。

use std::fmt;

macro_rules! id_newtype {
    ($(#[$doc:meta])* $name:ident, $label:literal) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            /// 底层数值。
            pub const fn get(self) -> u64 {
                self.0
            }
        }

        impl From<u64> for $name {
            fn from(value: u64) -> Self {
                Self(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!($label, "#{}"), self.0)
            }
        }
    };
}

id_newtype!(
    /// 会话代次：每次 `StopRequested` 递增。事件携带其所属代次，
    /// 代次不符的事件在 [`crate::apply`] 处被确定性丢弃。**唯一 owner 是根 [`State`]**。
    Epoch,
    "epoch"
);

id_newtype!(
    /// 一轮主对话（main turn）的唯一标识。v0 同时只允许一个活动 turn。
    ///
    /// 允许跨 [`Epoch`] 重用数值：同一 `TurnId` 在新旧两代里互不可见
    /// （迟到事件由 epoch 闸门拦截）。
    TurnId,
    "turn"
);

id_newtype!(
    /// turn 内一个句子的唯一标识（一个 turn 可含多句，顺序即计划顺序）。
    SentenceId,
    "sentence"
);

impl Epoch {
    /// 初始代次（0）。
    pub const ZERO: Self = Self(0);

    /// 递增（饱和加法，避免溢出回绕破坏单调性）。
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl Default for Epoch {
    fn default() -> Self {
        Self::ZERO
    }
}
