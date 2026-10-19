//! LLM 端点形态二选一（`[llm].mode`，2026-10-18 0.3.7 T1）。
//!
//! 从 `settings.rs` 拆出（该文件已贴 1000 行硬上限）：`settings.rs` 仍持有
//! `LlmSettings` 字段与解析/写回/serde 语义，本文件只放 `LlmMode` 类型本身。
//! 语义一行未改。

use serde::{Deserialize, Serialize};

/// LLM 端点形态二选一（`[llm].mode`，2026-10-18 0.3.7 T1）。
///
/// **缺省 `Cloud`**：地址 / 模型 / 密钥由用户填（0.3.9 起不再有厂商快照）。
/// `Local`：端点在用户**自己起的**本机 / 局域网 OpenAI-compatible 服务上
/// （Ollama / llama.cpp / vLLM 等）。**transport 不变**——仍是同一条
/// `/chat/completions`；本项目不嵌模型、不下载权重、不加推理依赖、
/// 不预填厂商或 Key。模式只是配置标签：**不参与** `resolve`，也不改变鉴权
/// （是否带 `Authorization` 仍只由 `api_key_env` 决定）。
///
/// 切换走同一次「保存并重载」；失败就是这一轮没生效（错误码照旧），
/// **不自动改回/改去另一个模式**（local 连不上不会偷偷切 cloud）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LlmMode {
    /// 云端（缺省）。
    #[default]
    Cloud,
    /// 本地（用户自起的 OpenAI-compatible 端点）。
    Local,
}

impl LlmMode {
    /// 稳定值（写进 `[llm].mode`，也是 HTTP patch 的取值）。
    pub const CLOUD: &'static str = "cloud";
    /// 见 [Self::CLOUD]。
    pub const LOCAL: &'static str = "local";

    /// 稳定字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cloud => Self::CLOUD,
            Self::Local => Self::LOCAL,
        }
    }

    /// 是否缺省 `cloud`（`skip_serializing_if` 用：**缺省不落盘**，
    /// 旧 toml 无 `mode` 键因此逐字 round-trip）。
    ///
    /// 收 `&self`：serde 的 `skip_serializing_if` 传的是字段引用。
    #[must_use]
    pub fn is_cloud(&self) -> bool {
        *self == Self::Cloud
    }
}
