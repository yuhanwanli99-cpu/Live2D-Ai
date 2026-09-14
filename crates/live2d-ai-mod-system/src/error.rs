//! Mod 错误类型。

use std::fmt;

/// Mod 系统错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModError {
    /// Mod 声明了不兼容的 API 版本。
    IncompatibleApi {
        mod_id: String,
        got: u32,
        expected: u32,
    },
    /// Mod 初始化/注册失败。
    Init { mod_id: String, message: String },
    /// Mod 事件处理失败。
    Event { mod_id: String, message: String },
    /// 设置 schema 非法。
    InvalidSettings { mod_id: String, message: String },
    /// Mod 不认识 host 发来的一次性命令（产品级加强波次新增）。
    ///
    /// 与 [`ModError::Other`] 分开是为了让 host 能回一个**可处置**的码：
    /// `unsupported_command`（409）而不是笼统的「Mod 错误」。
    UnsupportedCommand { command: String },
    /// 其它。
    Other(String),
}

impl fmt::Display for ModError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModError::IncompatibleApi {
                mod_id,
                got,
                expected,
            } => {
                write!(
                    f,
                    "Mod {mod_id} API 版本不兼容（got {got}, expected {expected}）"
                )
            }
            ModError::Init { mod_id, message } => {
                write!(f, "Mod {mod_id} 初始化失败: {message}")
            }
            ModError::Event { mod_id, message } => {
                write!(f, "Mod {mod_id} 事件处理失败: {message}")
            }
            ModError::InvalidSettings { mod_id, message } => {
                write!(f, "Mod {mod_id} 设置非法: {message}")
            }
            ModError::UnsupportedCommand { command } => {
                write!(f, "Mod 不支持命令: {command}")
            }
            ModError::Other(s) => write!(f, "Mod 错误: {s}"),
        }
    }
}

impl std::error::Error for ModError {}

impl From<String> for ModError {
    fn from(s: String) -> Self {
        ModError::Other(s)
    }
}
