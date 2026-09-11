//! 密钥包装 [`ApiSecret`]：Debug / Display 恒定脱敏，绝不泄露明文。
//!
//! 设计约束：
//! - **不派生 `Debug` 的明文输出**：`format!("{:?}")` 与 `{}` 一律输出占位符；
//! - **不实现 serde 序列化**：防止密钥被意外写进日志、快照或请求体；
//!   明文只能经 [`expose_secret`](ApiSecret::expose_secret) 显式取出，
//!   且在本 crate 内唯一用途是写入 `Authorization: Bearer …` 请求头；
//! - 不引入 zeroize 等额外依赖（保持依赖克制；密钥生命周期由调用方管理）。

use core::fmt;

/// API 密钥（如 OpenAI-compatible 服务的 `sk-…`）。
///
/// `Debug` 输出恒为 `ApiSecret(REDACTED)`，`Display` 输出恒为 `[REDACTED]`，
/// 因此把含密钥的配置结构体整体 `Debug` 打印也是安全的。
#[derive(Clone, Default, PartialEq, Eq)]
pub struct ApiSecret(String);

impl ApiSecret {
    /// 包装明文密钥。
    pub fn new(secret: impl Into<String>) -> Self {
        Self(secret.into())
    }

    /// 显式取出明文。仅用于设置 `Authorization` 头，不要打印或落盘。
    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    /// 是否为空串。
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for ApiSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 绝不输出 self.0：长度、前后缀都不泄露。
        f.write_str("ApiSecret(REDACTED)")
    }
}

impl fmt::Display for ApiSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl From<&str> for ApiSecret {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for ApiSecret {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "sk-live-super-secret-123";

    #[test]
    fn debug_and_display_never_leak_secret() {
        let key = ApiSecret::new(SECRET);
        let dbg = format!("{key:?}");
        let disp = format!("{key}");
        assert_eq!(dbg, "ApiSecret(REDACTED)");
        assert_eq!(disp, "[REDACTED]");
        assert!(!dbg.contains("sk-live"));
        assert!(!disp.contains("sk-live"));
        assert!(!dbg.contains(SECRET));
        assert!(!disp.contains(SECRET));
    }

    #[test]
    fn config_debug_does_not_leak_nested_secret() {
        let cfg = crate::config::LlmConfig {
            base_url: "http://127.0.0.1:11434/v1".to_string(),
            model: "qwen2.5:7b".to_string(),
            api_key: Some(ApiSecret::new(SECRET)),
            max_tokens: 0,
        };
        let dbg = format!("{cfg:?}");
        assert!(dbg.contains("ApiSecret(REDACTED)"), "got: {dbg}");
        assert!(!dbg.contains(SECRET));
    }

    #[test]
    fn expose_and_convert_roundtrip() {
        let key = ApiSecret::from(SECRET.to_string());
        assert_eq!(key.expose_secret(), SECRET);
        assert!(!key.is_empty());
        assert!(ApiSecret::default().is_empty());
        assert_eq!(ApiSecret::new("a"), ApiSecret::from("a"));
    }
}
