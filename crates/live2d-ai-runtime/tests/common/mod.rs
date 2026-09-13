//! 集成测试公用基建：本地 tokio TCP mock、SSE 构造器、断言工具。
//!
//! 仅供 `tests/conversation_engine_*.rs` 引用——本目录是一个 test-only 子 crate
//! 入口（Rust 集成测试约定：`tests/common/mod.rs` 不被视作顶层集成测试二进制，
//! 但其他 `tests/*.rs` 可以 `mod common;` 复用其中的项）。

// 跨多个 `tests/conversation_engine_*.rs` 复用：单一测试文件可能只用其中一部分，
// 由此 `#[allow(dead_code)]` 是必须的——clippy 在每个集成测试二进制内独立 dead-code 校验。
#![allow(dead_code)]

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use live2d_ai_runtime::{
    ConversationConfig, ConversationEngine, EngineEvent, ErrorKind, LlmConfig, OpenAiClient,
    TtsConfig, TurnStatus,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::timeout;

pub const READ_TIMEOUT: Duration = Duration::from_secs(5);
/// 单步兜底超时：流水线若卡死，让测试失败而不是挂死。
#[allow(dead_code)] // 公共基建：仅部分测试二进制引用
pub const STEP_TIMEOUT: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------- mock 基建

/// 单个连接的处理 future。
pub type BoxFut = Pin<Box<dyn Future<Output = ()> + Send>>;
/// 共享的连接处理闭包。
pub type SharedHandler = Arc<dyn Fn(TcpStream) -> BoxFut + Send + Sync>;

/// 可克隆的连接处理器（每次 accept 一个连接）。
#[derive(Clone)]
pub struct Handler(pub SharedHandler);

impl Handler {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(TcpStream) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync + 'static,
    {
        Self(Arc::new(f))
    }
}

#[derive(Debug)]
pub struct CapturedRequest {
    pub body: Vec<u8>,
}

pub fn contains(haystack: &[u8], needle: &[u8; 4]) -> Option<usize> {
    haystack.windows(4).position(|w| w == needle)
}

pub async fn read_request(sock: &mut TcpStream) -> CapturedRequest {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 2048];

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
    let content_length = head
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.trim()
                .eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())?
        })
        .unwrap_or(0);

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

    CapturedRequest { body }
}

/// 从请求体解析 `input` 字段（/audio/speech 的 JSON）。
pub async fn read_speech_input(sock: &mut TcpStream) -> String {
    let req = read_request(sock).await;
    let json: serde_json::Value = serde_json::from_slice(&req.body).expect("json body");
    json["input"].as_str().expect("input 字段").to_string()
}

/// 写响应头（content-length 由 pieces 总长决定）。
pub async fn write_head(
    sock: &mut TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    total_len: usize,
) {
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\ncontent-type: {content_type}\r\ncontent-length: {total_len}\r\nconnection: close\r\n\r\n"
    );
    sock.write_all(head.as_bytes()).await.expect("write head");
}

/// 写完响应体（片间可选延时）。**不关闭写端**：需要结束连接时显式调用
/// [`close_sock`]，以便同一响应分段写入。
pub async fn write_pieces(sock: &mut TcpStream, pieces: &[Vec<u8>], piece_delay_ms: u64) {
    for piece in pieces {
        if piece_delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(piece_delay_ms)).await;
        }
        sock.write_all(piece).await.expect("write piece");
    }
}

/// 关闭写端（发 FIN）：响应体写完后调用。
pub async fn close_sock(sock: &mut TcpStream) {
    sock.shutdown().await.expect("shutdown");
}

/// 静态分片响应处理器：每个连接收到相同的预设响应。
pub fn respond_pieces(
    status: u16,
    reason: &'static str,
    content_type: &'static str,
    pieces: Vec<Vec<u8>>,
) -> Handler {
    Handler::new(move |mut sock| {
        let pieces = pieces.clone();
        Box::pin(async move {
            let total: usize = pieces.iter().map(Vec::len).sum();
            write_head(&mut sock, status, reason, content_type, total).await;
            write_pieces(&mut sock, &pieces, 10).await;
            close_sock(&mut sock).await;
        })
    })
}

/// 起「接受任意多次连接」的本地服务；返回 `<base>/v1` 形式的 base_url。
pub async fn spawn_multi_server(handler: Handler) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        while let Ok((sock, _)) = listener.accept().await {
            // 每个连接独立任务处理；必须显式 spawn（否则返回的 future
            // 会被直接丢弃，连接秒断）。
            tokio::spawn(handler.0(sock));
        }
    });
    format!("http://{addr}/v1")
}

// ---------------------------------------------------------------- SSE 构造

pub fn sse_content(text: &str) -> Vec<u8> {
    format!("data: {{\"choices\":[{{\"delta\":{{\"content\":\"{text}\"}}}}]}}\n\n").into_bytes()
}

pub fn sse_done() -> Vec<u8> {
    b"data: [DONE]\n\n".to_vec()
}

// ---------------------------------------------------------------- 断言工具

/// 所有事件都必须携带本轮 epoch。
#[allow(dead_code)] // 公共基建：仅部分测试二进制引用
pub fn assert_epoch(events: &[EngineEvent], epoch: u64) {
    let got: Vec<u64> = events
        .iter()
        .map(|e| match e {
            EngineEvent::TextDelta { epoch, .. }
            | EngineEvent::ReasoningDelta { epoch, .. }
            | EngineEvent::AudioChunk { epoch, .. }
            | EngineEvent::SentenceVoiced { epoch, .. }
            | EngineEvent::TextFallback { epoch, .. }
            | EngineEvent::Error { epoch, .. }
            | EngineEvent::Terminal { epoch, .. } => *epoch,
        })
        .collect();
    assert!(got.iter().all(|&e| e == epoch), "epoch 不一致: {got:?}");
}

/// Terminal 必须恰好一次且是最后一个事件（终态统一，步骤 7/D8）。
pub fn assert_single_terminal_last(events: &[EngineEvent], expected: TurnStatus) {
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, EngineEvent::Terminal { .. }))
            .count(),
        1,
        "Terminal 必须恰好一次: {events:?}"
    );
    match events.last() {
        Some(EngineEvent::Terminal { status, .. }) => assert_eq!(*status, expected),
        other => panic!("最后事件必须是 Terminal，得到 {other:?}"),
    }
}

/// 按 sentence_seq 聚合音频样本；断言 seq 连续、句间严格串行、
/// 每句恰好一个 final_chunk 且为该句最后一块。
#[allow(dead_code)] // 公共基建：仅 lifecycle 用，tts_flow 不引用
pub fn collect_audio(events: &[EngineEvent]) -> Vec<(u64, Vec<f32>)> {
    let mut group_order: Vec<u64> = Vec::new();
    let mut chunks: Vec<(u64, Vec<f32>, bool)> = Vec::new();
    for event in events {
        if let EngineEvent::AudioChunk {
            sentence_seq,
            samples,
            final_chunk,
            ..
        } = event
        {
            if !group_order.contains(sentence_seq) {
                group_order.push(*sentence_seq);
            }
            chunks.push((*sentence_seq, samples.clone(), *final_chunk));
        }
    }
    assert_eq!(
        group_order,
        (1..=group_order.len() as u64).collect::<Vec<_>>(),
        "sentence_seq 必须从 1 连续递增: {group_order:?}"
    );
    for seq in &group_order {
        let mine: Vec<&(u64, Vec<f32>, bool)> =
            chunks.iter().filter(|(s, _, _)| s == seq).collect();
        assert_eq!(
            mine.iter().filter(|chunk| chunk.2).count(),
            1,
            "句 {seq} 的 final_chunk 必须恰好一个"
        );
        assert!(
            mine.last().is_some_and(|(_, _, f)| *f),
            "句 {seq} 的最后一块必须是 final_chunk"
        );
    }
    group_order
        .iter()
        .map(|&seq| {
            let samples: Vec<f32> = chunks
                .iter()
                .filter(|(s, _, _)| *s == seq)
                .flat_map(|(_, samples, _)| samples.iter().copied())
                .collect();
            (seq, samples)
        })
        .collect()
}

#[allow(dead_code)] // 公共基建：仅部分测试二进制引用
pub fn find_error_kind(events: &[EngineEvent]) -> Option<&ErrorKind> {
    events.iter().find_map(|e| match e {
        EngineEvent::Error { kind, .. } => Some(kind),
        _ => None,
    })
}

pub fn engine(llm_base: &str, tts_base: &str, config: ConversationConfig) -> ConversationEngine {
    let client = OpenAiClient::new(
        LlmConfig::new(llm_base, "test-model"),
        TtsConfig::new(tts_base, "alloy"),
    )
    .expect("client");
    ConversationEngine::new(client, config)
}

/// 通用 TTS mock：记录到达的 input 顺序；返回确定性 PCM（按 3 字节奇数切块）。
pub async fn spawn_tts_mock() -> (String, Arc<Mutex<Vec<String>>>) {
    let inputs: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let handler_inputs = inputs.clone();
    let base = spawn_multi_server(Handler::new(move |mut sock| {
        let inputs = handler_inputs.clone();
        Box::pin(async move {
            let input = read_speech_input(&mut sock).await;
            let n = input.chars().count().max(2);
            inputs.lock().unwrap().push(input);
            let samples: Vec<i16> = (0..n).map(|k| ((k % 13) as i16 + 1) * 97).collect();
            let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
            let pieces: Vec<Vec<u8>> = bytes.chunks(3).map(<[u8]>::to_vec).collect();
            let total: usize = pieces.iter().map(Vec::len).sum();
            write_head(&mut sock, 200, "OK", "audio/pcm", total).await;
            write_pieces(&mut sock, &pieces, 5).await;
            close_sock(&mut sock).await;
        })
    }))
    .await;
    (base, inputs)
}

/// 跑完一轮并收集全部事件（run_turn 返回后发送端全部关闭，recv 到 None 即排空）。
pub async fn drain_events(mut event_rx: mpsc::Receiver<EngineEvent>) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    while let Some(event) = event_rx.recv().await {
        events.push(event);
    }
    events
}
