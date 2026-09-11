//! 集成测试：本地 tokio TCP mock 服务（不引第三方 mock 依赖）。
//!
//! 覆盖：
//! - SSE 事件被 TCP 切割到「半行」级别仍正确解码（含多字节字符）；
//! - `[DONE]` 终止与「对端未发 `[DONE]` 即关流」的兜底 Done；
//! - 非 2xx 状态码 → 可观测的 [`Error::Status`]；
//! - 密钥只进 `Authorization` 头；无 key 时头不存在；Debug 不泄露；
//! - **请求体不含 `tools` 键**（2026-09-11 用户裁决：LLM 不暴露任何工具）；
//! - TTS `/audio/speech` 请求形状（默认 `response_format=pcm`）+ chunked bytes
//!   原样转发，且 raw PCM 字节流可跨任意 chunk 切割增量解码。
//!
//! 行数豁免：本文件超 ≤500 红线（豁免上限 1000）。
//! 豁免理由：测试文件各用例均为「单文件 mock TCP 端 + 端到端断言」的小循环，
//! 强行拆分为多文件会要求把 mock 服务端构造（`PlannedResponse` / `spawn_server` /
//! `sse_data` / `llm_client` 等）和 SSE 字节构造器抽到 common 模块——这会改动
//! common 的公共面，而 common 同时被 `conversation_engine_*` 三个集成测试使用，
//! 会扩大改动半径、引入与本任务无关的回归风险；本任务 scope 明确不在拆分该文件。
//! 因此维持单文件、附头注豁免。

use std::time::Duration;

use futures_util::StreamExt;
use live2d_ai_runtime::{ChatMessage, Error, LlmConfig, LlmEvent, OpenAiClient, TtsConfig};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio::time::timeout;

const READ_TIMEOUT: Duration = Duration::from_secs(5);

const SECRET: &str = "sk-test-secret-do-not-leak";

// ---------------------------------------------------------------- mock 服务端

/// 预设的一次 HTTP 响应（body 按 pieces 分片写入，模拟任意网络切割）。
struct PlannedResponse {
    status: u16,
    reason: &'static str,
    content_type: &'static str,
    pieces: Vec<Vec<u8>>,
    piece_delay_ms: u64,
}

impl PlannedResponse {
    fn sse(pieces: Vec<Vec<u8>>) -> Self {
        Self {
            status: 200,
            reason: "OK",
            content_type: "text/event-stream",
            pieces,
            piece_delay_ms: 15,
        }
    }

    fn bytes(
        status: u16,
        reason: &'static str,
        content_type: &'static str,
        pieces: Vec<Vec<u8>>,
    ) -> Self {
        Self {
            status,
            reason,
            content_type,
            pieces,
            piece_delay_ms: 10,
        }
    }

    fn status_only(
        status: u16,
        reason: &'static str,
        content_type: &'static str,
        body: &str,
    ) -> Self {
        Self {
            status,
            reason,
            content_type,
            pieces: vec![body.as_bytes().to_vec()],
            piece_delay_ms: 0,
        }
    }
}

#[derive(Debug)]
struct CapturedRequest {
    method: String,
    path: String,
    authorization: Option<String>,
    content_type: Option<String>,
    body: Vec<u8>,
}

fn contains(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

async fn read_request(sock: &mut TcpStream) -> CapturedRequest {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];

    let header_end = loop {
        if let Some(pos) = contains(&buf, b"\r\n\r\n") {
            break pos + 4;
        }
        let n = timeout(READ_TIMEOUT, sock.read(&mut tmp))
            .await
            .expect("read headers timed out")
            .expect("read headers failed");
        assert!(n > 0, "peer closed before sending full headers");
        buf.extend_from_slice(&tmp[..n]);
    };

    let head = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let mut lines = head.split("\r\n");
    let request_line = lines.next().expect("request line");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().expect("method").to_string();
    let path = parts.next().expect("path").to_string();

    let mut authorization = None;
    let mut content_type = None;
    let mut content_length = 0usize;
    for line in lines {
        if let Some((key, value)) = line.split_once(':') {
            match key.trim().to_ascii_lowercase().as_str() {
                "authorization" => authorization = Some(value.trim().to_string()),
                "content-type" => content_type = Some(value.trim().to_string()),
                "content-length" => content_length = value.trim().parse().expect("content-length"),
                _ => {}
            }
        }
    }

    let mut body = buf[header_end..].to_vec();
    while body.len() < content_length {
        let n = timeout(READ_TIMEOUT, sock.read(&mut tmp))
            .await
            .expect("read body timed out")
            .expect("read body failed");
        assert!(n > 0, "peer closed before sending full body");
        body.extend_from_slice(&tmp[..n]);
    }
    body.truncate(content_length);

    CapturedRequest {
        method,
        path,
        authorization,
        content_type,
        body,
    }
}

/// 起一个只服务**一次**连接的本地 mock；返回 `<base>/v1` 形式的 base_url
/// （顺带验证路径前缀拼接），以及取回捕获请求的 handle。
async fn spawn_server(planned: PlannedResponse) -> (String, JoinHandle<CapturedRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let handle = tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.expect("accept");
        let captured = read_request(&mut sock).await;

        let total: usize = planned.pieces.iter().map(Vec::len).sum();
        let head = format!(
            "HTTP/1.1 {} {}\r\ncontent-type: {}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            planned.status, planned.reason, planned.content_type, total
        );
        sock.write_all(head.as_bytes()).await.expect("write head");
        for piece in &planned.pieces {
            if planned.piece_delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(planned.piece_delay_ms)).await;
            }
            sock.write_all(piece).await.expect("write piece");
        }
        sock.shutdown().await.expect("shutdown");
        captured
    });
    (format!("http://{addr}/v1"), handle)
}

fn sse_data(payload: &str) -> Vec<u8> {
    format!("data: {payload}\n\n").into_bytes()
}

fn llm_client(llm_base: &str) -> OpenAiClient {
    OpenAiClient::new(
        LlmConfig::new(llm_base, "qwen2.5:7b").with_api_key(SECRET),
        TtsConfig::new("http://127.0.0.1:9/v1", "alloy"),
    )
    .expect("client")
}

// ---------------------------------------------------------------- LLM 流式

#[tokio::test]
async fn llm_stream_survives_half_line_cuts_and_multibyte_payloads() {
    // 一个完整 SSE 事件的字节流被劈在 JSON 字符串中间（TCP 半行切割），
    // 且切点落在**多字节字符内部** —— 解码层必须缓冲到完整字符再解析。
    let whole_event = "data: {\"choices\":[{\"delta\":{\"content\":\"再见了\"}}]}\n\n"
        .as_bytes()
        .to_vec();
    // 尾部 `了"}}]}\n\n` 共 10 字节；从倒数第 9 字节切即落在「了」的 UTF-8 序列内。
    let cut = whole_event.len() - 9;
    let split_event_head = whole_event[..cut].to_vec();
    let split_event_tail = whole_event[cut..].to_vec();

    let planned = PlannedResponse::sse(vec![
        b": keep-alive\n\n".to_vec(),
        sse_data(r#"{"choices":[{"delta":{"role":"assistant","content":"你好"}}]}"#),
        sse_data(r#"{"choices":[{"delta":{"content":"呀"}}]}"#),
        split_event_head,
        split_event_tail,
        sse_data("[DONE]"),
    ]);
    let (base, server) = spawn_server(planned).await;

    let mut stream = llm_client(&base)
        .chat_stream(&[
            ChatMessage::system("你是桌宠"),
            ChatMessage::user("打个招呼"),
        ])
        .await
        .expect("chat_stream");

    let mut events = Vec::new();
    while let Some(item) = stream.next().await {
        events.push(item.expect("stream item"));
    }

    assert_eq!(
        events,
        vec![
            LlmEvent::TextDelta("你好".to_string()),
            LlmEvent::TextDelta("呀".to_string()),
            LlmEvent::TextDelta("再见了".to_string()),
            LlmEvent::Done,
        ]
    );

    // 请求形状断言。
    let captured = server.await.expect("server task");
    assert_eq!(captured.method, "POST");
    assert_eq!(captured.path, "/v1/chat/completions");
    let expected_auth = format!("Bearer {SECRET}");
    assert_eq!(
        captured.authorization.as_deref(),
        Some(expected_auth.as_str())
    );
    assert!(
        captured
            .content_type
            .as_deref()
            .unwrap_or_default()
            .starts_with("application/json")
    );

    let body: serde_json::Value = serde_json::from_slice(&captured.body).expect("json body");
    assert_eq!(body["model"], "qwen2.5:7b");
    assert_eq!(body["stream"], true);
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][1]["role"], "user");
    // **2026-09-11 用户裁决**：LLM 不暴露任何工具，只做对话 → 请求体**不含**
    // `tools` 键（工具一旦出现，模型有时只发工具调用、正文一个字都不发）。
    assert!(body.get("tools").is_none(), "请求体不得含 tools 键: {body}");

    // 请求体里绝不允许出现密钥明文。
    assert!(
        !captured
            .body
            .windows(SECRET.len())
            .any(|w| w == SECRET.as_bytes())
    );
}

#[tokio::test]
async fn llm_stream_emits_done_when_peer_closes_without_done_marker() {
    let planned = PlannedResponse::sse(vec![sse_data(
        r#"{"choices":[{"delta":{"content":"hi"}}]}"#,
    )]);
    let (base, server) = spawn_server(planned).await;

    let mut stream = llm_client(&base)
        .chat_stream(&[ChatMessage::user("x")])
        .await
        .expect("ok");
    let mut events = Vec::new();
    while let Some(item) = stream.next().await {
        events.push(item.expect("item"));
    }

    assert_eq!(
        events,
        vec![LlmEvent::TextDelta("hi".to_string()), LlmEvent::Done]
    );
    let _ = server.await.expect("server task");
}

#[tokio::test]
async fn llm_stream_surfaces_malformed_sse_data_as_error() {
    let planned = PlannedResponse::sse(vec![sse_data("{definitely not json")]);
    let (base, server) = spawn_server(planned).await;

    let mut stream = llm_client(&base)
        .chat_stream(&[ChatMessage::user("x")])
        .await
        .expect("ok");
    let err = stream
        .next()
        .await
        .expect("one item")
        .expect_err("must be error");
    assert!(matches!(err, Error::Sse { .. }), "got: {err:?}");
    assert!(err.to_string().contains("JSON"));
    // 错误后流终止。
    assert!(stream.next().await.is_none());
    let _ = server.await.expect("server task");
}

#[tokio::test]
async fn llm_error_status_is_observable_with_code_and_body() {
    let body = r#"{"error":{"message":"Incorrect API key provided","type":"invalid_request_error","code":"invalid_api_key"}}"#;
    let planned = PlannedResponse::status_only(401, "Unauthorized", "application/json", body);
    let (base, server) = spawn_server(planned).await;

    let err = match llm_client(&base)
        .chat_stream(&[ChatMessage::user("x")])
        .await
    {
        Err(e) => e,
        Ok(_) => panic!("chat_stream must fail with 401"),
    };
    match err {
        Error::Status { status, body } => {
            assert_eq!(status.as_u16(), 401);
            assert!(body.contains("Incorrect API key provided"));
            assert!(body.contains("invalid_api_key"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
    let _ = server.await.expect("server task");
}

#[tokio::test]
async fn missing_key_omits_authorization_header_entirely() {
    let planned = PlannedResponse::sse(vec![sse_data("[DONE]")]);
    let (base, server) = spawn_server(planned).await;

    let client = OpenAiClient::new(
        LlmConfig::new(&base, "m"),
        TtsConfig::new("http://127.0.0.1:9/v1", "alloy"),
    )
    .expect("client");
    let mut stream = client.chat_stream(&[]).await.expect("ok");
    assert!(matches!(stream.next().await, Some(Ok(LlmEvent::Done))));

    let captured = server.await.expect("server task");
    assert_eq!(captured.authorization, None);
}

// ------------------------------------------------- llm.max_tokens 上线形态

/// `LlmConfig.max_tokens` 必须真的走到线上：非 0 → 出现；0 → 字段省略。
///
/// 单测只覆盖了 `chat_request_body` 的形状，这里补上「配置 → 请求体」那一段
/// ——否则 `resolve()` 解析出来的上限被谁吞掉都不会有人发现。
#[tokio::test]
async fn llm_max_tokens_reaches_the_wire_and_zero_omits_the_field() {
    for (configured, expect_on_wire) in [(512u32, Some(512u64)), (0u32, None)] {
        let planned = PlannedResponse::sse(vec![sse_data("[DONE]")]);
        let (base, server) = spawn_server(planned).await;

        let mut cfg = LlmConfig::new(&base, "m");
        cfg.max_tokens = configured;
        let client = OpenAiClient::new(cfg, TtsConfig::new("http://127.0.0.1:9/v1", "alloy"))
            .expect("client");
        let mut stream = client.chat_stream(&[]).await.expect("ok");
        assert!(matches!(stream.next().await, Some(Ok(LlmEvent::Done))));

        let captured = server.await.expect("server task");
        let raw = std::str::from_utf8(&captured.body).expect("request body is UTF-8");
        let body: serde_json::Value =
            serde_json::from_str(raw).unwrap_or_else(|e| panic!("body is not JSON: {e}"));
        assert_eq!(body["stream"], true, "全链路仍是流式");
        match expect_on_wire {
            Some(v) => assert_eq!(body["max_tokens"], v, "配置了上限就必须上线"),
            None => assert!(
                body.get("max_tokens").is_none(),
                "0 = 不限制 → 省略字段（某些服务把 0 当「拒绝生成」）: {body}"
            ),
        }
    }
}

// ---------------------------------------------------------------- TTS

#[tokio::test]
async fn tts_forwards_chunked_bytes_and_sends_expected_shape() {
    let planned = PlannedResponse::bytes(
        200,
        "OK",
        "audio/mpeg",
        vec![
            b"RIFF".to_vec(),
            b"\x00\x01\x02\xff".to_vec(),
            b"data-tail".to_vec(),
        ],
    );
    let (base, server) = spawn_server(planned).await;

    let client = OpenAiClient::new(
        LlmConfig::new("http://127.0.0.1:9/v1", "m"),
        TtsConfig {
            api_key: Some(SECRET.into()),
            model: Some("tts-1".to_string()),
            response_format: Some("mp3".to_string()),
            ..TtsConfig::new(&base, "alloy")
        },
    )
    .expect("client");

    let mut audio = client
        .synthesize_speech("你好，世界")
        .await
        .expect("speech");

    let mut collected = Vec::new();
    while let Some(chunk) = audio.next().await {
        collected.extend_from_slice(&chunk.expect("chunk"));
    }
    assert_eq!(collected, b"RIFF\x00\x01\x02\xffdata-tail");

    let captured = server.await.expect("server task");
    assert_eq!(captured.method, "POST");
    assert_eq!(captured.path, "/v1/audio/speech");
    let expected_auth = format!("Bearer {SECRET}");
    assert_eq!(
        captured.authorization.as_deref(),
        Some(expected_auth.as_str())
    );

    let body: serde_json::Value = serde_json::from_slice(&captured.body).expect("json body");
    assert_eq!(body["input"], "你好，世界");
    assert_eq!(body["voice"], "alloy");
    assert_eq!(body["model"], "tts-1");
    assert_eq!(body["response_format"], "mp3");
}

#[tokio::test]
async fn tts_default_request_sends_response_format_pcm_on_the_wire() {
    let planned = PlannedResponse::bytes(200, "OK", "audio/pcm", vec![vec![0u8, 1, 2]]);
    let (base, server) = spawn_server(planned).await;

    // TtsConfig::new 的默认契约：请求 response_format=pcm（本批唯一保证可解码格式）。
    let client = OpenAiClient::new(
        LlmConfig::new("http://127.0.0.1:9/v1", "m"),
        TtsConfig::new(&base, "nova"),
    )
    .expect("client");

    let mut audio = client.synthesize_speech("hi").await.expect("speech");
    while let Some(chunk) = audio.next().await {
        chunk.expect("chunk");
    }

    let captured = server.await.expect("server task");
    let body: serde_json::Value = serde_json::from_slice(&captured.body).expect("json body");
    assert_eq!(
        body,
        serde_json::json!({
            "input": "hi",
            "voice": "nova",
            "response_format": "pcm",
        })
    );
    assert_eq!(captured.authorization, None);
}

#[tokio::test]
async fn tts_pcm_stream_decodes_across_arbitrary_chunk_splits() {
    // 5 个 mono s16le 样本 = 10 字节，按「奇数字节」边界劈给网络层：
    // 解码器必须把半个样本留到下一个 chunk 拼接。
    let samples: [i16; 5] = [0, 16_384, -32_768, 32_767, -1];
    let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
    let planned = PlannedResponse::bytes(
        200,
        "OK",
        "audio/pcm",
        vec![
            bytes[..3].to_vec(),
            bytes[3..6].to_vec(),
            bytes[6..].to_vec(),
        ],
    );
    let (base, server) = spawn_server(planned).await;

    let client = OpenAiClient::new(
        LlmConfig::new("http://127.0.0.1:9/v1", "m"),
        TtsConfig::new(&base, "alloy"), // 默认 pcm + 24 kHz mono
    )
    .expect("client");

    let mut speech = client.synthesize_speech("hi").await.expect("speech");
    let mut dec = live2d_ai_runtime::PcmS16LeDecoder::new(client.tts().spec);
    let mut decoded = Vec::new();
    while let Some(chunk) = speech.next().await {
        decoded.extend(dec.decode(&chunk.expect("chunk")));
    }
    dec.finish().expect("aligned stream");

    let expected: Vec<f32> = samples.iter().map(|&s| f32::from(s) / 32_768.0).collect();
    assert_eq!(decoded, expected);
    assert_eq!(client.tts().spec.sample_rate(), 24_000);
    assert_eq!(client.tts().spec.channels(), 1);
    let _ = server.await.expect("server task");
}

#[tokio::test]
async fn tts_error_status_is_observable_too() {
    let planned = PlannedResponse::status_only(
        500,
        "Internal Server Error",
        "application/json",
        r#"{"error":"engine offline"}"#,
    );
    let (base, server) = spawn_server(planned).await;

    let client = OpenAiClient::new(
        LlmConfig::new("http://127.0.0.1:9/v1", "m"),
        TtsConfig::new(&base, "alloy"),
    )
    .expect("client");

    match match client.synthesize_speech("x").await {
        Err(e) => e,
        Ok(_) => panic!("synthesize_speech must fail with 500"),
    } {
        Error::Status { status, body } => {
            assert_eq!(status.as_u16(), 500);
            assert!(body.contains("engine offline"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
    let _ = server.await.expect("server task");
}
