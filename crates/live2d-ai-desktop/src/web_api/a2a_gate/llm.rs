//! A2A 上游（0.3.9 rc.3 起住宿主门）：当前这一路 \`[llm]\` → OpenAiClient →
//! 正文收集 + 失败脱敏。
//!
//! 从 0.3.8 的 \`web_api/a2a_routes/a2a_llm.rs\` **逐字迁入**：协议核不读配置、
//! 不碰密钥，上游只能由宿主注入——这是「宿主只留门」的另一半。
//!
//! 边界（执行单 §1）：只读当前配置快照的 \`[llm]\` 段；**不**认识 supervisor / say /
//! 句子装配 / TTS / 口型 / cue。请求体由既有 chat_request_body 组装（固定
//! stream=true），测试断言它没有 tools / tool_choice / functions。
//!
//! 失败口径：只回**稳定错误码**（llm_upstream_401 / llm_transport 等），
//! 不把上游 body、更不把密钥原样回出去（日志同样只记错误码）。

use live2d_ai_runtime::{ApiSecret, ChatMessage, LlmConfig, OpenAiClient, TtsConfig};

use crate::web_api::ServerContext;

/// 生产上游：读当前配置快照 → \`[llm]\` → OpenAiClient → 流式收集正文。
pub(super) fn llm_chat_upstream(ctx: &ServerContext, text: &str) -> Result<String, String> {
    llm_chat_upstream_with(ctx, text, &live2d_ai_runtime::secrets::lookup)
}

/// \`llm_chat_upstream\` 的实现：密钥查找抽成注入点（测试不污染进程环境）。
pub(super) fn llm_chat_upstream_with(
    ctx: &ServerContext,
    text: &str,
    lookup: &dyn Fn(&str) -> Option<String>,
) -> Result<String, String> {
    let settings = ctx.status_ctx.settings_snapshot();
    let llm = llm_config_for(&settings, lookup)?;
    let client = OpenAiClient::new(llm, TtsConfig::new("", "unused"))
        .map_err(|_| "构建 LLM 客户端失败（错误码 llm_client_build）".to_string())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "构建异步运行时失败（错误码 llm_runtime_build）".to_string())?;
    let text = text.to_string();
    runtime.block_on(async move {
        let stream = client
            .chat_stream(&[ChatMessage::user(text)])
            .await
            .map_err(|e| sanitize_llm_error(&e))?;
        // 思考（ReasoningDelta）在收集器里被丢弃，不进 A2A 正文。
        live2d_ai_runtime::collect_chat_text(stream)
            .await
            .map_err(|e| sanitize_llm_error(&e))
    })
}

/// 从当前 settings 里构造**只有一路 LLM** 的 LlmConfig。
///
/// 不走 \`AppSettings::resolve_with\`：那条路会顺带校验 TTS（A2A 不碰 TTS），
/// 用户 TTS 配错不该让 A2A 端点失效。
fn llm_config_for(
    settings: &live2d_ai_runtime::AppSettings,
    lookup: &dyn Fn(&str) -> Option<String>,
) -> Result<LlmConfig, String> {
    let llm = &settings.llm;
    if llm.base_url.trim().is_empty() {
        return Err("LLM 未配置：[llm].base_url 为空（错误码 llm_unconfigured）".to_string());
    }
    let mut config = LlmConfig::new(llm.base_url.trim(), llm.model.trim());
    config.max_tokens = llm.effective_max_tokens();
    config.thinking = llm.effective_show_reasoning();
    if let Some(name) = llm
        .api_key_env
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        if !live2d_ai_runtime::secrets::is_valid_key(name) {
            return Err(
                "LLM 配置有误：[llm].api_key_env 不是合法环境变量名（错误码 llm_api_key_env_invalid）"
                    .to_string(),
            );
        }
        if let Some(value) = lookup(name) {
            config = config.with_api_key(ApiSecret::new(value));
        }
    }
    Ok(config)
}

/// 上游失败 → **已脱敏**的稳定描述：只带状态码 / 错误码，不带上游 body、不带 key。
fn sanitize_llm_error(error: &live2d_ai_runtime::Error) -> String {
    match error.upstream_status() {
        Some(status) => format!("上游对话失败（llm_upstream_{status}）"),
        None => format!("上游对话失败（llm_{}）", error.code_suffix()),
    }
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;

    use live2d_ai_runtime::Error;

    use super::*;

    /// 上游 body 可能带密钥 / 内部细节：错误描述只保留状态码。
    #[test]
    fn sanitize_llm_error_drops_upstream_body_and_key() {
        let error = Error::Status {
            status: StatusCode::UNAUTHORIZED,
            body: "invalid api key sk-super-secret".to_string(),
        };
        let message = sanitize_llm_error(&error);
        assert_eq!(message, "上游对话失败（llm_upstream_401）");
        assert!(!message.contains("sk-super-secret"));
        assert!(!message.contains("invalid api key"));
    }

    /// 非 HTTP 状态类失败也只给稳定错误码。
    #[test]
    fn sanitize_llm_error_maps_transport_failures_to_code_only() {
        let error = Error::Sse {
            message: "upstream said secret sk-super-secret".to_string(),
        };
        let message = sanitize_llm_error(&error);
        assert_eq!(message, "上游对话失败（llm_sse_parse）");
        assert!(!message.contains("sk-super-secret"));
    }
}
