//! `peer_pi_parse` 单测（0.4.5 rc.1）：纯函数，录好的 stdout，不碰进程。
//!
//! 这些用例锁死「只取 assistant text 块 / 末条 assistant / error 与 bad JSON」
//! 三条契约；全链路（假二进制）用例在 `peer_pi_tests.rs`。

// =====================================================================
// 解析器（隔离的契约面）
// =====================================================================

#[test]
fn parser_keeps_only_text_blocks_and_joins_them() {
    let fixture = r#"{"type":"message_end","message":{"role":"assistant","stopReason":"stop","content":[{"type":"thinking","thinking":"内心"},{"type":"text","text":"第一句"},{"type":"toolCall","id":"t","name":"bash","arguments":{}},{"type":"text","text":"第二句"}]}}"#;
    assert_eq!(
        crate::web_api::peer_pi_parse::parse_pi_session_stdout(fixture).unwrap(),
        "第一句\n第二句"
    );
}

#[test]
fn parser_ignores_noise_and_takes_last_assistant_message() {
    // 噪声行（半截 JSON / 非 JSON）+ header + 第二条 assistant 胜出。
    let fixture = r#"garbage before
{"type":"session","version":3}
{"type":"message_end","message":{"role":"assistant","stopReason":"stop","content":[{"type":"text","text":"旧"}]}}
{"type":"message_end","message":{"role":"assistant","stopReason":"stop","content":[{"type":"text","text":"新"}]}}
{"truncated":
"#;
    assert_eq!(
        crate::web_api::peer_pi_parse::parse_pi_session_stdout(fixture).unwrap(),
        "新"
    );
}

#[test]
fn parser_rejects_no_assistant_message() {
    use crate::web_api::peer_pi_parse::{PiParseError, parse_pi_session_stdout};
    // 空 stdout / 只有 header / 只有 user / 坏 JSON → 同一个失败。
    for stdout in [
        "",
        "{\"type\":\"session\",\"version\":3}",
        "{\"type\":\"message_end\",\"message\":{\"role\":\"user\",\"content\":\"hi\"}}",
        "not json at all",
    ] {
        assert_eq!(
            parse_pi_session_stdout(stdout),
            Err(PiParseError::NoAssistantMessage),
            "stdout={stdout:?}"
        );
    }
    assert_eq!(
        PiParseError::NoAssistantMessage.reason(),
        "no_assistant_message"
    );
}

#[test]
fn parser_rejects_empty_text_and_error_stop() {
    use crate::web_api::peer_pi_parse::{PiParseError, parse_pi_session_stdout};
    // 只有思考 / 工具调用 → EmptyText。
    let only_thinking = r#"{"type":"message_end","message":{"role":"assistant","stopReason":"stop","content":[{"type":"thinking","thinking":"心里话"}]}}"#;
    assert_eq!(
        parse_pi_session_stdout(only_thinking),
        Err(PiParseError::EmptyText)
    );
    // stopReason error / aborted → AssistantError（json 模式退出码仍是 0）。
    for reason in ["error", "aborted"] {
        let stdout = format!(
            r#"{{"type":"message_end","message":{{"role":"assistant","stopReason":"{reason}","errorMessage":"boom","content":[{{"type":"text","text":"半句"}}]}}}}"#
        );
        assert_eq!(
            parse_pi_session_stdout(&stdout),
            Err(PiParseError::AssistantError),
            "reason={reason}"
        );
    }
    assert_eq!(PiParseError::EmptyText.reason(), "empty_assistant_text");
    assert_eq!(
        PiParseError::AssistantError.reason(),
        "assistant_error_stop"
    );
}

#[test]
fn parser_accepts_length_stop_reason_with_text() {
    // 明确取舍：`length`（截断）仍是**有正文**的合法 JSON，按「有正文就回」处理；
    // 只有 error / aborted 才是失败（执行单没把截断列为错误）。
    let stdout = r#"{"type":"message_end","message":{"role":"assistant","stopReason":"length","content":[{"type":"text","text":"写到一半"}]}}"#;
    assert_eq!(
        crate::web_api::peer_pi_parse::parse_pi_session_stdout(stdout).unwrap(),
        "写到一半"
    );
}
