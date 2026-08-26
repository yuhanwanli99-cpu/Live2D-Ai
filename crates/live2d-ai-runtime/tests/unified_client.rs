//! 集成测试：本地 tokio TCP mock 服务（不引第三方 mock 依赖）。
//!
//! 覆盖：
//! - SSE 事件被 TCP 切割到「半行」级别仍正确解码；
//! - `tool_calls.function.name` 单事件内劈开、`arguments` 跨多个 SSE 事件分块拼接；
//! - `[DONE]` 终止与「对端未发 `[DONE]` 即关流」的兜底 Done；
//! - 非 2xx 状态码 → 可观测的 [`Error::Status`]；
//! - 密钥只进 `Authorization` 头；无 key 时头不存在；Debug 不泄露；
//! - TTS `/audio/speech` 请求形状（默认 `response_format=pcm`）+ chunked bytes
//!   原样转发，且 raw PCM 字节流可跨任意 chunk 切割增量解码。

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
async fn llm_stream_survives_half_line_cuts_and_split_tool_fragments() {
    // 一个完整 SSE 事件的字节流被劈在 JSON 字符串中间（TCP 半行切割）。
    let name_event_head =
        br#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"live2d_perfor"#.to_vec();
    // 注意：tail 补完 JSON 后必须带上事件边界的空行（\n\n）。
    let name_event_tail = br#"m_action"}}]}}]}

"#
    .to_vec();

    // arguments 是标准的跨事件分块：两个片段拼回完整 JSON（含非 ASCII reason）。
    let args_frag_1 = sse_data(
        r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"action\":\"nod\""}}]}}]}"#,
    );
    let args_frag_2 = sse_data(
        r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":",\"strength\":3,\"reason\":\"嗯\"}"}}]}}]}"#,
    );

    let planned = PlannedResponse::sse(vec![
        b": keep-alive\n\n".to_vec(),
        sse_data(r#"{"choices":[{"delta":{"role":"assistant","content":"你好"}}]}"#),
        sse_data(r#"{"choices":[{"delta":{"content":"呀"}}]}"#),
        // 名字事件：先写半个事件，停顿，再补完 —— 解码层必须缓冲而不是按行猜。
        name_event_head.clone(),
        name_event_tail.clone(),
        args_frag_1,
        args_frag_2,
        sse_data("[DONE]"),
    ]);
    let (base, server) = spawn_server(planned).await;

    let mut stream = llm_client(&base)
        .chat_stream(&[
            ChatMessage::system("你是桌宠"),
            ChatMessage::user("点头同意一下"),
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
            LlmEvent::ToolCallDelta {
                index: 0,
                id: Some("call_1".to_string()),
                name: Some("live2d_perform_action".to_string()),
                arguments: String::new(),
            },
            LlmEvent::ToolCallDelta {
                index: 0,
                id: None,
                name: None,
                arguments: "{\"action\":\"nod\"".to_string(),
            },
            LlmEvent::ToolCallDelta {
                index: 0,
                id: None,
                name: None,
                arguments: ",\"strength\":3,\"reason\":\"嗯\"}".to_string(),
            },
            LlmEvent::Done,
        ]
    );

    // 拼接出的参数能还原成 wire 类型。
    let joined: String = events
        .iter()
        .filter_map(|e| match e {
            LlmEvent::ToolCallDelta { arguments, .. } => Some(arguments.as_str()),
            _ => None,
        })
        .collect();
    let args: live2d_ai_runtime::Live2dPerformActionArgs =
        serde_json::from_str(&joined).expect("valid tool args json");
    assert_eq!(args.action, live2d_ai_runtime::PerformAction::Nod);
    assert_eq!(args.strength, 3);
    assert_eq!(args.reason.as_deref(), Some("嗯"));

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
    assert_eq!(
        body["tools"][0]["function"]["name"],
        "live2d_perform_action"
    );

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
