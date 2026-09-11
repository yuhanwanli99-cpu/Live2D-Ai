//! [`ErrorKind`] 的**可观测投影**：稳定错误码 + 阶段 + 一句可执行提示。
//!
//! # 为什么单独一个文件（2026-09-11）
//!
//! 用户报「后端出错无具体错误代码，改动提示词就崩了，日志里也看不出来」。
//! 查下来失败本身是上游 `401 Unauthorized`（进程环境里没有 `DEEPSEEK_API_KEY`），
//! 但**三处都不可观测**：
//!
//! 1. `ErrorKind` 只有 `Display`（`LLM 阶段错误: 上游非成功状态 401 …`）——
//!    散文，不是码，前端/脚本无从分支；
//! 2. supervisor 用 `println!` 打它，只进 stdout，**不进 tracing 文件 sink**；
//! 3. WS 根本没有 `error` 帧（`AppEvent` 无 `Error` 变体）→ 前端**永远**收不到。
//!
//! 本模块解决第 1 条；第 2/3 条由 desktop 侧接线（`AppEvent::Error` +
//! `app_event_to_ws_frame` + `tracing::error!`）。
//!
//! # 契约
//!
//! [`ErrorKind::code`] 的返回值是**对外协议**（WS `error` 帧的 `code` 字段、
//! 日志的 `code=` 字段、前端文案）。形态 `<stage>_<suffix>`：
//!
//! | kind | code 示例 | fatal |
//! | --- | --- | --- |
//! | `Llm(Status 401)` | `llm_upstream_401` | 否（已生成语音照常播完） |
//! | `Llm(Http)` | `llm_transport` | 否 |
//! | `Tts(Status 400)` | `tts_upstream_400` | 是 |
//! | `Decode(_)` | `decode_pcm_unaligned` | 是 |
//! | `Backpressure{..}` | `tts_backpressure` | 是 |
//!
//! 2026-09-11 用户裁决后 `IncompleteTools(_)` / `tools_incomplete` 一行已随
//! 工具移除删除（LLM 不暴露任何工具，不再有「工具调用残缺」这条错误路径）。
//!
//! `fatal` 与 supervisor 的 `saw_fatal_kind` 判据一致（见
//! `supervisor::handlers::handle_engine_event`）——**唯一口径**，前端据此决定
//! 「还能不能继续等这一轮」。
//!
//! 本文件是纯函数（无 IO / 无时间 / 无随机），可直接单测。

use super::ErrorKind;
use crate::error::Error;

impl ErrorKind {
    /// 稳定的机器可读错误码（契约，见模块头注）。
    pub fn code(&self) -> String {
        match self {
            ErrorKind::Llm(e) => format!("llm_{}", e.code_suffix()),
            ErrorKind::Tts(e) => format!("tts_{}", e.code_suffix()),
            ErrorKind::Decode(e) => format!("decode_{}", e.code_suffix()),
            ErrorKind::Backpressure { .. } => "tts_backpressure".to_string(),
        }
    }

    /// 出错阶段（`llm` / `tts` / `decode`）。
    ///
    /// **与 [`Self::code`] 的前缀同源**：`code()` 总是以本值开头（回归测试锁定），
    /// 这样日志里既能按 `stage` 粗筛、也能按 `code` 精确定位。背压归 `tts`——
    /// 它就是 TTS 队列的背压，不是独立阶段。
    pub fn stage(&self) -> &'static str {
        match self {
            ErrorKind::Llm(_) => "llm",
            ErrorKind::Tts(_) | ErrorKind::Backpressure { .. } => "tts",
            ErrorKind::Decode(_) => "decode",
        }
    }

    /// 是否致命（丢弃剩余排队句子、立即结束本轮）。
    ///
    /// **口径唯一**：与 supervisor 的 `saw_fatal_kind` 完全一致——
    /// `Llm` 不致命（已生成的语音继续播完，见 D8 裁决）。
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            ErrorKind::Tts(_) | ErrorKind::Decode(_) | ErrorKind::Backpressure { .. }
        )
    }

    /// 一句**可执行**的排查提示（按错误码分流）。
    ///
    /// 生产实现**总是**给出提示（每种 `Error` 变体都有下一步）；返回 `Option`
    /// 只为 WS 帧的 `hint` 字段保留「无提示」这一形状。
    ///
    /// 存在意义：`llm_upstream_401` 的处置（「去后端进程环境里设 `api_key_env`
    /// 指向的变量」）不是常识——2026-09-11 那次排查里，人看到 401 的第一反应
    /// 是「改提示词改坏了」。提示必须由后端给出，前端只负责显示。
    pub fn hint(&self) -> Option<String> {
        match self {
            ErrorKind::Llm(e) => hint_for(e, "llm", "提示词内容与鉴权无关"),
            ErrorKind::Tts(e) => {
                hint_for(e, "tts", "检查 [tts] base_url / voice 是否与已部署服务一致")
            }
            ErrorKind::Decode(e) => match e {
                Error::TruncatedPcm { .. } => Some(
                    "上游 PCM 流长度非样本对齐：确认 [tts] response_format 与服务端实际返回一致"
                        .to_string(),
                ),
                _ => Some("检查 [tts] sample_rate / channels 与服务端实际输出一致".to_string()),
            },
            ErrorKind::Backpressure { .. } => {
                Some("下游播放长期不消费：确认前端音频播放未被暂停或浏览器标签页未休眠".to_string())
            }
        }
    }
}

/// `llm` / `tts` 段共用的凭证/端点提示表。
///
/// `stage` 用于文案里的段名前缀（`[llm]` / `[tts]`）；`non_auth_note` 只在
/// 401/403 时追加（它要直接反驳「是不是我改提示词改坏的」这个误判）。
///
/// **全变体覆盖**（无 `_` 兜底）：每个错误都配一句下一步——「不知道该怎么办」
/// 正是这次投诉的核心。新增 [`Error`] 变体时编译器会在这里点名。
fn hint_for(e: &Error, stage: &str, non_auth_note: &str) -> Option<String> {
    match e {
        Error::Status { status, .. } => {
            let s = status.as_u16();
            match s {
                401 | 403 => Some(format!(
                    "上游拒绝鉴权（HTTP {s}）：确认 [{stage}] api_key_env 指向的环境变量在「后端进程」的环境里已设置且非空；{non_auth_note}"
                )),
                404 => Some(format!(
                    "上游 404：确认 [{stage}] base_url 与 model 名在服务端存在（含尾部 /v1 的写法）"
                )),
                429 => Some(format!(
                    "上游限流（HTTP {s}）：降低并发或稍后重试（[{stage}]）"
                )),
                s if (500..600).contains(&s) => Some(format!(
                    "上游 {s}：服务端故障，重试或查看上游日志（[{stage}]）"
                )),
                _ => Some(format!(
                    "上游 HTTP {s} 拒绝了请求：核对 [{stage}] 配置，并看上文字段里的上游响应体"
                )),
            }
        }
        Error::Http(_) => Some(format!(
            "连不上 [{stage}] 端点：确认服务在跑、端口/地址可达（base_url 与网络）"
        )),
        Error::InvalidBaseUrl { .. } => Some(format!(
            "[{stage}] base_url 非法：需要 http(s):// 开头的绝对 URL"
        )),
        Error::Sse { .. } => Some(format!(
            "[{stage}] 流式响应不是合法 SSE：确认端点真的 OpenAI 兼容（非兼容端点会返回整包 JSON）"
        )),
        Error::Json(_) => Some(format!(
            "[{stage}] 响应 JSON 不符合契约：确认端点兼容 OpenAI 协议"
        )),
        Error::UnsupportedFormat { .. } => {
            Some("[tts] response_format 只支持 \"pcm\" / \"wav\"（本批次保证 pcm）".to_string())
        }
        Error::TruncatedPcm { .. } => Some(
            "[tts] 上游 PCM 流长度非样本对齐：确认 response_format 与服务端实际返回一致"
                .to_string(),
        ),
        Error::InvalidAudioConfig { .. } => {
            Some("[tts] 音频参数非法：检查 sample_rate / channels".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status_err(code: u16) -> Error {
        Error::status(
            reqwest::StatusCode::from_u16(code).unwrap(),
            "Authentication Fails (governor)",
        )
    }

    /// 契约：`code()` 以 `stage()` 开头（日志里两种筛法口径一致）。
    #[test]
    fn code_always_starts_with_stage() {
        let kinds = [
            ErrorKind::Llm(status_err(401)),
            ErrorKind::Tts(status_err(500)),
            ErrorKind::Decode(status_err(400)),
            ErrorKind::Backpressure {
                message: "队列满".into(),
            },
        ];
        for k in kinds {
            assert!(
                k.code().starts_with(k.stage()),
                "code={} stage={}",
                k.code(),
                k.stage()
            );
        }
    }

    /// 用户报的那一条：401 必须进码，而且提示要指向**环境变量**而不是提示词。
    #[test]
    fn llm_401_gets_specific_code_and_env_hint() {
        let k = ErrorKind::Llm(status_err(401));
        assert_eq!(k.code(), "llm_upstream_401");
        assert_eq!(k.stage(), "llm");
        assert!(!k.is_fatal(), "LLM 失败不致命：已生成语音仍要播完");
        let hint = k.hint().expect("401 必须有提示");
        assert!(hint.contains("api_key_env"), "hint: {hint}");
        assert!(hint.contains("后端进程"), "hint: {hint}");
        // 提示必须直接反驳「是不是我改提示词改坏的」这个误判，且**不能**带
        // Markdown 星号（文案直接上屏，裸 `Text` 不认 `**`）。
        assert!(hint.contains("提示词内容与鉴权无关"), "hint: {hint}");
        assert!(!hint.contains("**"), "上屏文案不许有 Markdown: {hint}");
    }

    #[test]
    fn llm_transport_and_base_url_codes() {
        assert_eq!(
            ErrorKind::Llm(Error::InvalidBaseUrl { url: "nope".into() }).code(),
            "llm_base_url_invalid"
        );
        assert_eq!(
            ErrorKind::Llm(Error::Sse {
                message: "bad data line".into()
            })
            .code(),
            "llm_sse_parse"
        );
    }

    #[test]
    fn tts_and_decode_are_fatal_with_own_codes() {
        let tts = ErrorKind::Tts(status_err(400));
        assert_eq!(tts.code(), "tts_upstream_400");
        assert!(tts.is_fatal());
        assert!(tts.hint().unwrap().contains("[tts]"));

        let dec = ErrorKind::Decode(Error::TruncatedPcm { trailing_bytes: 1 });
        assert_eq!(dec.code(), "decode_pcm_unaligned");
        assert!(dec.is_fatal());
    }

    #[test]
    fn backpressure_has_stable_code() {
        let bp = ErrorKind::Backpressure {
            message: "超时".into(),
        };
        assert_eq!(bp.code(), "tts_backpressure");
        assert!(bp.is_fatal());
    }

    /// 非 401/403 的上游状态不产生鉴权误导文案。
    #[test]
    fn hint_does_not_claim_auth_for_other_statuses() {
        let hint = ErrorKind::Llm(status_err(400)).hint().unwrap_or_default();
        assert!(!hint.contains("鉴权"), "400 不应说鉴权: {hint}");
    }
}
