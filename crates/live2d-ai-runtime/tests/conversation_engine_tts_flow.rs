//! `ConversationEngine` TTS/解码/背压集成测试：错误分类覆盖 Tts / Decode / Backpressure。

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use live2d_ai_runtime::{ConversationConfig, EngineEvent, ErrorKind, TurnStatus};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use common::{
    Handler, STEP_TIMEOUT, assert_epoch, assert_single_terminal_last, close_sock, drain_events,
    engine, find_error_kind, read_speech_input, respond_pieces, spawn_multi_server, spawn_tts_mock,
    sse_content, sse_done, write_head, write_pieces,
};

/// LLM 非 2xx：Error(Llm) → Terminal{Failed}，TTS 零请求。
#[tokio::test]
async fn llm_status_error_is_observable_then_failed_terminal() {
    let llm_base = spawn_multi_server(respond_pieces(
        503,
        "Service Unavailable",
        "application/json",
        vec![br#"{"error":"overloaded"}"#.to_vec()],
    ))
    .await;
    let (tts_base, tts_inputs) = spawn_tts_mock().await;

    let mut eng = engine(&llm_base, &tts_base, ConversationConfig::default());
    let (event_tx, event_rx) = mpsc::channel(64);
    let report = eng
        .run_turn(11, "hi", event_tx, CancellationToken::new())
        .await;
    assert_eq!(report.status, TurnStatus::Failed);
    assert_eq!(report.assistant_text, "");

    let events = drain_events(event_rx).await;
    assert_epoch(&events, 11);
    assert_single_terminal_last(&events, TurnStatus::Failed);
    assert_eq!(events.len(), 2, "{events:?}");
    match &events[0] {
        EngineEvent::Error { kind, .. } => match kind {
            ErrorKind::Llm(e) => assert!(e.to_string().contains("503"), "{e}"),
            other => panic!("期望 Llm 错误，实际 {other:?}"),
        },
        other => panic!("首事件必须是 Error，实际 {other:?}"),
    }
    assert!(tts_inputs.lock().unwrap().is_empty());
}

/// TTS 500：Error(Tts) 致命——丢弃剩余排队句子，Terminal{Failed} 收尾。
#[tokio::test]
async fn tts_error_stops_turn_with_failed_terminal() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("第一句。第二"), sse_done()],
    ))
    .await;

    // 第一笔 TTS 请求返回 500（后续本就不应被触达）。
    let inputs: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let handler_inputs = inputs.clone();
    let tts_base = spawn_multi_server(Handler::new(move |mut sock| {
        let inputs = handler_inputs.clone();
        Box::pin(async move {
            let input = read_speech_input(&mut sock).await;
            inputs.lock().unwrap().push(input);
            let body = br#"{"error":"engine offline"}"#.to_vec();
            write_head(
                &mut sock,
                500,
                "Internal Server Error",
                "application/json",
                body.len(),
            )
            .await;
            write_pieces(&mut sock, &[body], 0).await;
            close_sock(&mut sock).await;
        })
    }))
    .await;

    let mut eng = engine(&llm_base, &tts_base, ConversationConfig::default());
    let (event_tx, event_rx) = mpsc::channel(64);
    let report = eng
        .run_turn(12, "hi", event_tx, CancellationToken::new())
        .await;
    assert_eq!(report.status, TurnStatus::Failed);

    let events = drain_events(event_rx).await;
    assert_epoch(&events, 12);
    assert_single_terminal_last(&events, TurnStatus::Failed);
    // 第二句「第二」被丢弃：worker 在首个致命错误后退出。
    assert_eq!(*inputs.lock().unwrap(), ["第一句。"]);
    match find_error_kind(&events) {
        Some(ErrorKind::Tts(e)) => assert!(e.to_string().contains("engine offline"), "{e}"),
        other => panic!("期望 Tts 错误，实际 {other:?}"),
    }
}

/// PCM 总字节数为奇数 → Error(Decode)，Terminal{Failed} 收尾。
#[tokio::test]
async fn odd_total_pcm_bytes_yield_decode_failed_terminal() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("念一句。"), sse_done()],
    ))
    .await;
    let tts_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "audio/pcm",
        vec![vec![1u8, 2, 3, 4, 5]], // 5 字节：奇数 → TruncatedPcm
    ))
    .await;

    let mut eng = engine(&llm_base, &tts_base, ConversationConfig::default());
    let (event_tx, event_rx) = mpsc::channel(64);
    let report = eng
        .run_turn(13, "hi", event_tx, CancellationToken::new())
        .await;
    assert_eq!(report.status, TurnStatus::Failed);

    let events = drain_events(event_rx).await;
    assert_epoch(&events, 13);
    assert_single_terminal_last(&events, TurnStatus::Failed);
    assert!(matches!(
        find_error_kind(&events),
        Some(ErrorKind::Decode(_))
    ));
}

/// 有界队列背压：下游长期不消费且配置了推送超时
/// → Error(Backpressure) → Terminal{Failed}。
#[tokio::test]
async fn queue_push_timeout_yields_backpressure_error() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        // 一个 delta 直接产生 4 个句子：worker 卡在第 1 句时队列（容量 1）被第 2 句占满，
        // 第 3 句推送阻塞直到超时。
        vec![sse_content("甲。乙。丙。丁"), sse_done()],
    ))
    .await;
    // TTS 收到首笔请求后长期不响应（远超推送超时）。
    let tts_hits: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let handler_hits = tts_hits.clone();
    let tts_base = spawn_multi_server(Handler::new(move |mut sock| {
        let hits = handler_hits.clone();
        Box::pin(async move {
            let input = read_speech_input(&mut sock).await;
            hits.lock().unwrap().push(input);
            tokio::time::sleep(Duration::from_secs(60)).await; // 卡住 worker
            let bytes = vec![0u8, 0];
            write_head(&mut sock, 200, "OK", "audio/pcm", bytes.len()).await;
            write_pieces(&mut sock, &[bytes], 0).await;
            close_sock(&mut sock).await;
        })
    }))
    .await;

    let config = ConversationConfig {
        tts_queue_capacity: 1,
        queue_push_timeout: Some(Duration::from_millis(150)),
        ..ConversationConfig::default()
    };
    let mut eng = engine(&llm_base, &tts_base, config);
    let (event_tx, event_rx) = mpsc::channel(64);
    let started = std::time::Instant::now();
    let report = eng
        .run_turn(14, "hi", event_tx, CancellationToken::new())
        .await;
    // 引擎内部的 child 取消会让 worker 立刻放弃卡住的请求：整体必须快速收场。
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "致命错误后必须快速收场，实际 {:?}",
        started.elapsed()
    );
    assert_eq!(report.status, TurnStatus::Failed);

    let events = drain_events(event_rx).await;
    assert_epoch(&events, 14);
    assert_single_terminal_last(&events, TurnStatus::Failed);
    assert!(matches!(
        find_error_kind(&events),
        Some(ErrorKind::Backpressure { .. })
    ));
    // 只有第 1 句真正发起了 TTS 请求。
    assert_eq!(tts_hits.lock().unwrap().len(), 1);
    // 兜底：确保测试间用到的 STEP_TIMEOUT 常量未被误删。
    let _ = STEP_TIMEOUT;
}
