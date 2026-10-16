//! 单句失败**续走**与**碎片句**的纯判定（2026-10-15 执行单 §2 / §3）。
//!
//! 两件事都放纯函数里（无 IO / 无时间 / 无随机），`worker.rs` 只调用：
//!
//! - [`kind_is_recoverable`]：worker 手里这一句的失败该不该终结整轮。可续走的
//!   失败打一行 `tracing::error!`（`code` 沿用现成 `ErrorKind::code()`），然后
//!   走既有静音句（这一句还没发出过音频块）或补一个空尾块（已经发出过），
//!   worker 不退出、后面排队的句子照常合成；
//! - [`is_fragment_marker`]：`speech_for_tts` **之后**要送去合成的这份文本是不是
//!   「碎片句」，是则不发 HTTP、走静音句。屏幕上的字一个不动。
//!
//! [`super::ErrorKind::is_fatal`] **不变**：背压仍致命、取消仍是取消——这里判的是
//! 「这一句的合成失败」而不是「整轮的生成侧终态」。

use crate::Error;

use super::ErrorKind;

/// 这一句的合成 / 解码错误是否**只影响这一句**（轮次继续，worker 不退出）。
///
/// | 变体 | 轮次 |
/// | --- | --- |
/// | `Status`（任何状态码，含 500 与「input 为空」的 400） | 继续 |
/// | `Http`（传输，含连不上本地 TTS 端口） | 继续 |
/// | `TruncatedPcm` / `UnsupportedFormat` / `InvalidAudioConfig` | 继续 |
/// | `InvalidBaseUrl` / `Sse` / `Json` | 仍是致命（发 `Error`、worker 退出） |
///
/// 端点 / 协议本身就坏的（base_url 非法、不是 SSE、JSON 不合契约）后面每一句都会
/// 同样失败，继续只是把坏链路拉长；单句状态码 / 传输抖动 / 音频规格问题只属于这一句。
pub fn sentence_error_is_recoverable(err: &Error) -> bool {
    match err {
        Error::Status { .. }
        | Error::Http(_)
        | Error::TruncatedPcm { .. }
        | Error::UnsupportedFormat { .. }
        | Error::InvalidAudioConfig { .. } => true,
        Error::InvalidBaseUrl { .. } | Error::Sse { .. } | Error::Json(_) => false,
    }
}

/// [`ErrorKind`] 版本：只有 `Tts` / `Decode` 里的可续走变体继续；
/// `Llm` 与 `Backpressure` 不归这条路径（后者仍是致命）。
pub fn kind_is_recoverable(kind: &ErrorKind) -> bool {
    match kind {
        ErrorKind::Tts(e) | ErrorKind::Decode(e) => sentence_error_is_recoverable(e),
        ErrorKind::Llm(_) | ErrorKind::Backpressure { .. } => false,
    }
}

/// 送 TTS 的文本（**`speech_for_tts` 之后**那一份）是不是「碎片句」。
///
/// 判据（先去掉首尾空白）：
/// - 空，或只由 `*` `#` 反引号组成；
/// - 整段是「一位以上 ASCII 数字 + 一个英文句点」（`1.`、`  2.` 命中）。
///
/// `1. 你好`、`12.3`、`a_b` **不**命中：只有整段就是那个序号才不算可念文本。
pub fn is_fragment_marker(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return true;
    }
    if trimmed.chars().all(|c| matches!(c, '*' | '#' | '`')) {
        return true;
    }
    let bytes = trimmed.as_bytes();
    bytes.len() >= 2
        && bytes[bytes.len() - 1] == b'.'
        && bytes[..bytes.len() - 1].iter().all(u8::is_ascii_digit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;

    /// 造一个**真的** `reqwest::Error`：空 host 在发请求之前的 URL 解析就失败
    ///（`IntoUrl`），不需要网络、也不需要监听端口。
    fn transport_error() -> Error {
        Error::Http(
            reqwest::Client::new()
                .get("http://")
                .build()
                .expect_err("空 host 的 URL 解析必须失败"),
        )
    }

    /// 500 `Status` / `Http` / `TruncatedPcm` / `UnsupportedFormat` /
    /// `InvalidAudioConfig` → 这一句继续，轮次不以 `Failed` 收尾。
    #[test]
    fn speakable_recoverable_sentence_errors_continue_the_turn() {
        let cases = [
            ErrorKind::Tts(Error::status(
                StatusCode::INTERNAL_SERVER_ERROR,
                "{\"error\":\"engine offline\"}",
            )),
            ErrorKind::Tts(Error::status(
                StatusCode::BAD_REQUEST,
                "{\"detail\":\"input 为空\"}",
            )),
            ErrorKind::Tts(transport_error()),
            ErrorKind::Decode(Error::TruncatedPcm { trailing_bytes: 1 }),
            ErrorKind::Decode(Error::UnsupportedFormat {
                format: "mp3".into(),
            }),
            ErrorKind::Decode(Error::InvalidAudioConfig {
                message: "sample_rate=0".into(),
            }),
        ];
        for kind in cases {
            assert!(
                kind_is_recoverable(&kind),
                "{kind:?} 必须只影响这一句（轮次继续）"
            );
        }
    }

    /// `InvalidBaseUrl` / `Sse` / `Json` 仍是致命：发 `Error`、worker 退出。
    #[test]
    fn speakable_endpoint_and_protocol_errors_stay_fatal() {
        let json: Error = serde_json::from_str::<serde_json::Value>("{")
            .expect_err("坏 JSON 必须报错")
            .into();
        let fatal = [
            ErrorKind::Tts(Error::InvalidBaseUrl {
                url: "ftp://x".into(),
            }),
            ErrorKind::Tts(Error::Sse {
                message: "bad data line".into(),
            }),
            ErrorKind::Decode(json),
        ];
        for kind in fatal {
            assert!(!kind_is_recoverable(&kind), "{kind:?} 仍是致命");
        }
    }

    /// 纯空白 / 纯标记 / 纯「数字 + 句点」不进 TTS。
    #[test]
    fn speakable_fragment_markers_do_not_reach_tts() {
        for text in [
            "", "   ", "\n", "*", "**", "#", "###", "`", "```", "1.", "  2.", "12.",
        ] {
            assert!(is_fragment_marker(text), "{text:?} 应判碎片句");
        }
    }

    /// 有可念文字的一律照常合成（不做按字符位置硬切）。
    #[test]
    fn speakable_real_text_is_never_a_fragment() {
        for text in [
            "1. 你好",
            "12.3",
            "a_b",
            "你好。",
            "#测试",
            "**粗体**",
            "喵",
        ] {
            assert!(!is_fragment_marker(text), "{text:?} 不是碎片句");
        }
    }
}
