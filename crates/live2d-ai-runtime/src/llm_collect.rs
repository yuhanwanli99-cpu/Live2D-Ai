//! 把一路 LLM 增量流收集成一段**正文**（0.3.8 rc.1，A2A 上游）。
//!
//! A2A 的 `message/send` 是「文本进、文本出」：上游仍复用
//! [`crate::OpenAiClient::chat_stream`]（固定 `stream=true`），但外部 agent
//! 要的是一条完整回复，而不是增量帧。本模块只做这一件事：
//!
//! - 累加 [`LlmEvent::TextDelta`]；
//! - **丢弃** [`LlmEvent::ReasoningDelta`]（思考不是正文——推理模型的内心独白
//!   不得出现在 A2A 返回里，同「思考不进 TTS」同一条纪律）；
//! - 遇到 [`LlmEvent::Done`] 停止；流自然结束（无 Done）也算成功；
//! - 流里的错误原样上抛（`Err`），调用方把它映射成 A2A 的失败结果。
//!
//! 边界：本模块**不**碰句子装配器、TTS、口型、cue，也不认识 `supervisor`——
//! A2A 与皮套回合是两条独立路径（见执行单 §1「禁止」）。
//!
//! 为什么不把实现塞进 `llm.rs`：那个文件在 500 行棘轮上（599 行），
//! AGENTS.md 要求新逻辑放新文件。

use futures_util::StreamExt;

use crate::error::Result;
use crate::llm::LlmEvent;

/// 收集一路 `chat_stream` 的正文；思考（`ReasoningDelta`）被丢弃。
///
/// 参数接受任意 `Stream<Item = Result<LlmEvent>>`（生产路径是
/// `chat_stream` 返回的 `Pin<Box<dyn Stream…>>`），测试可直接喂
/// `futures_util::stream::iter` 构造的假流——不需要网络。
pub async fn collect_chat_text<S>(stream: S) -> Result<String>
where
    S: futures_util::Stream<Item = Result<LlmEvent>>,
{
    let mut stream = Box::pin(stream);
    let mut text = String::new();
    while let Some(item) = stream.next().await {
        match item? {
            LlmEvent::TextDelta(delta) => text.push_str(&delta),
            // 思考单列一类事件：不进正文（也不会进 TTS/口型/A2A 返回）。
            LlmEvent::ReasoningDelta(_) => {}
            LlmEvent::Done => break,
        }
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_stream(
        events: Vec<Result<LlmEvent>>,
    ) -> impl futures_util::Stream<Item = Result<LlmEvent>> {
        futures_util::stream::iter(events)
    }

    /// 正文按到达顺序拼接；与思考交错也不影响正文的字节。
    #[tokio::test]
    async fn text_deltas_are_concatenated_and_reasoning_is_dropped() {
        let text = collect_chat_text(fake_stream(vec![
            Ok(LlmEvent::ReasoningDelta("我先想一下。".into())),
            Ok(LlmEvent::TextDelta("你好".into())),
            Ok(LlmEvent::ReasoningDelta("再想一下。".into())),
            Ok(LlmEvent::TextDelta("，世界。".into())),
            Ok(LlmEvent::Done),
        ]))
        .await
        .expect("流不应报错");
        assert_eq!(text, "你好，世界。");
        assert!(
            !text.contains("想"),
            "思考不得出现在 A2A 返回正文里：{text}"
        );
    }

    /// Done 之后的事件不再消费（上游关流语义；也防止后续杂音混进正文）。
    #[tokio::test]
    async fn done_stops_collection() {
        let text = collect_chat_text(fake_stream(vec![
            Ok(LlmEvent::TextDelta("半句".into())),
            Ok(LlmEvent::Done),
            Ok(LlmEvent::TextDelta("尾巴".into())),
        ]))
        .await
        .expect("流不应报错");
        assert_eq!(text, "半句");
    }

    /// 空流（上游一行都没发就关）→ 空正文；不当失败。
    #[tokio::test]
    async fn empty_stream_yields_empty_text() {
        let text = collect_chat_text(fake_stream(Vec::new()))
            .await
            .expect("空流不是错误");
        assert!(text.is_empty());
    }

    /// 流里的错误原样上抛（调用方据此给 A2A 失败结果）。
    #[tokio::test]
    async fn errors_propagate() {
        let err = collect_chat_text(fake_stream(vec![
            Ok(LlmEvent::TextDelta("半".into())),
            Err(crate::error::Error::Sse {
                message: "upstream broke".into(),
            }),
        ]))
        .await
        .expect_err("错误必须上抛");
        assert!(err.to_string().contains("upstream broke"));
    }

    /// **A2A 上游请求体契约（0.3.8）**：绝不带 `tools` 键。
    ///
    /// 皮套回合与 A2A 回合是两条路径，但共用同一个 `chat_request_body`——
    /// 这里把断言放在**新文件**而不是 `llm.rs`（执行单 §3：不要把 `llm.rs`
    /// 再撑大）。A2A 的消息形态就是一条 `role=user` 的纯文本。
    #[test]
    fn a2a_upstream_request_has_no_tools_key() {
        use crate::llm::{ChatMessage, chat_request_body};
        let messages = [ChatMessage::user("你好")];
        let json =
            serde_json::to_value(chat_request_body("m", &messages, 0, false)).expect("序列化");
        assert!(
            json.get("tools").is_none(),
            "A2A 上游请求体不得含 tools 键：{json}"
        );
        assert!(json.get("tool_choice").is_none(), "got: {json}");
        assert!(json.get("functions").is_none(), "got: {json}");
        assert_eq!(json["stream"], true, "A2A 上游仍是流式请求：{json}");
        assert_eq!(json["messages"][0]["role"], "user");
    }
}
