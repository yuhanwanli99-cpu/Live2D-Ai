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

/// **纯空白句不得发 TTS 请求，且不得把整轮判失败**（2026-09-11 修，实测缺陷）。
///
/// 来历：分句器把**换行**也算句读（`dialogue::sentence::is_terminator` 含 `'\n'`），
/// 所以模型吐出一个前导换行（或句间换行后紧跟正文）时，会切出一个**只含空白**的
/// 「句子」。它 `is_empty() == false` 因而躲过分句器的非空守卫，却会被 TTS 上游
/// trim 成空串，上游直接回 `400 {"detail":"input 为空"}`——而 TTS 错误是 **fatal**，
/// 于是一句纯空白把**整轮**判失败。真机日志原文（2026-09-11 13:28:02）：
///
/// ```text
/// ERROR TTS 阶段错误: 上游非成功状态 400 Bad Request: {"detail":"input 为空"}
/// code=tts_upstream_400 stage=tts fatal=true
/// ```
///
/// 修法：`synthesize_sentence` 对 `text.trim().is_empty()` 的句子**不发 HTTP**，
/// 与「TTS 未配置」走同一条静音句路径（发空 `final_chunk` + `SentenceVoiced`）——
/// 文本不丢（分句器的「逐字不丢」不变量不动），链路不断。
#[tokio::test]
async fn whitespace_only_sentence_never_reaches_tts_and_turn_completes() {
    let llm_base = spawn_multi_server(Handler::new(|mut sock| {
        Box::pin(async move {
            let _req = common::read_request(&mut sock).await;
            // 前导换行 → 切出纯空白句 `"\n"`；随后是正常句。
            let pieces = [
                // 注意：要给 **JSON 转义**后的 `\n`（真实换行会让 SSE JSON 非法）。
                sse_content("\\n"),
                sse_content("你好。"),
                sse_done(),
            ];
            let total: usize = pieces.iter().map(Vec::len).sum();
            write_head(&mut sock, 200, "OK", "text/event-stream", total).await;
            write_pieces(&mut sock, &pieces, 5).await;
            close_sock(&mut sock).await;
        })
    }))
    .await;
    // 自定义 TTS mock：**复刻真实上游**——空 input 回 400（CosyVoice 实测行为）。
    // 用 spawn_tts_mock 不行：它对本任何 input 都回 200，那样「空白句有没有被发出去」
    // 只能靠请求计数发现，整轮失败的后果反而测不到。
    let tts_inputs: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let inputs_h = tts_inputs.clone();
    let tts_base = spawn_multi_server(Handler::new(move |mut sock| {
        let inputs = inputs_h.clone();
        Box::pin(async move {
            let input = read_speech_input(&mut sock).await;
            inputs.lock().unwrap().push(input.clone());
            if input.trim().is_empty() {
                let body: Vec<u8> = "{\"detail\":\"input 为空\"}".as_bytes().to_vec();
                let total = body.len();
                write_head(&mut sock, 400, "Bad Request", "application/json", total).await;
                write_pieces(&mut sock, &[body], 5).await;
                close_sock(&mut sock).await;
                return;
            }
            let n = input.chars().count().max(2);
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

    let mut eng = engine(&llm_base, &tts_base, ConversationConfig::default());
    let (event_tx, event_rx) = mpsc::channel(64);
    let report = eng
        .run_turn(21, "hi", event_tx, CancellationToken::new())
        .await;

    // ① 关键回归：整轮必须**完成**，不能被一句空白判失败。
    assert_eq!(
        report.status,
        TurnStatus::Completed,
        "纯空白句把整轮判失败了（历史缺陷：TTS 400 input 为空 → fatal）"
    );

    // ② 空白句**从未**发起 TTS 请求；只有正常句到达。
    let inputs = tts_inputs.lock().unwrap().clone();
    assert_eq!(inputs.len(), 1, "只应有一句正常请求，实际 {inputs:?}");
    assert_eq!(inputs[0].trim(), "你好。");
    assert!(
        inputs.iter().all(|i| !i.trim().is_empty()),
        "任何空白句都不该发到 TTS 上游：{inputs:?}"
    );

    // ③ 文本侧仍按「一句一单元」上屏：空白句也发过 SentenceVoiced（逐字不丢）。
    let events = drain_events(event_rx).await;
    assert_epoch(&events, 21);
    assert_single_terminal_last(&events, TurnStatus::Completed);
    let voiced: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            EngineEvent::SentenceVoiced { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        voiced.iter().any(|t| t.trim().is_empty()),
        "空白句的文本不该被丢掉（分句器保证逐字不丢）：{voiced:?}"
    );
    assert!(voiced.iter().any(|t| t.contains("你好")), "{voiced:?}");
}

/// **rc.3 N0 正文兜底（2026-09-13）**：TTS 致命失败时，已经生成的正文必须经
/// `TextFallback` 交给 UI——否则用户看到的是「模型没有返回」，而模型其实写完了。
///
/// 三条断言缺一不可：
/// ① 失败轮**恰好一次** `TextFallback`，且 `text` 是**整轮正文**（不是残余）；
/// ② 它在 `Terminal` **之前**（终态仍是最后一个事件）；
/// ③ 同一轮的 `Error(Tts)` 照旧上报（兜底不掩盖错误）。
#[tokio::test]
async fn tts_failure_falls_back_to_the_generated_text() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("第一句。第二"), sse_done()],
    ))
    .await;

    // TTS 首笔即 500（致命）；第二句「第二」本就不会被送出。
    let tts_base = spawn_multi_server(respond_pieces(
        500,
        "Internal Server Error",
        "application/json",
        vec![br#"{"error":"engine offline"}"#.to_vec()],
    ))
    .await;

    let mut eng = engine(&llm_base, &tts_base, ConversationConfig::default());
    let (event_tx, event_rx) = mpsc::channel(64);
    let report = eng
        .run_turn(31, "hi", event_tx, CancellationToken::new())
        .await;
    assert_eq!(report.status, TurnStatus::Failed);

    let events = drain_events(event_rx).await;
    assert_epoch(&events, 31);
    assert_single_terminal_last(&events, TurnStatus::Failed);

    let fallbacks: Vec<(usize, &str)> = events
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match e {
            EngineEvent::TextFallback { text, .. } => Some((i, text.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(
        fallbacks.len(),
        1,
        "失败轮必须恰好兜底一次，实际 {}：{events:?}",
        fallbacks.len()
    );
    assert_eq!(
        fallbacks[0].1, "第一句。第二",
        "兜底必须是**整轮正文**（含未成句残余），不是残余单独一段"
    );

    let terminal_at = events
        .iter()
        .position(|e| matches!(e, EngineEvent::Terminal { .. }))
        .expect("有终态");
    assert!(
        fallbacks[0].0 < terminal_at,
        "兜底必须在 Terminal 之前（终态是最后一个事件）"
    );
    assert!(matches!(find_error_kind(&events), Some(ErrorKind::Tts(_))));
}

/// 健康轮**绝不**发 `TextFallback`：它只在失败时兜底，一旦健康路径也发，
/// 文字就会重新跑到声音前面（那正是 2026-09-10 用户裁决要修的事）。
#[tokio::test]
async fn completed_turn_never_emits_text_fallback() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("你好呀。"), sse_done()],
    ))
    .await;
    let (tts_base, _tts_inputs) = spawn_tts_mock().await;

    let mut eng = engine(&llm_base, &tts_base, ConversationConfig::default());
    let (event_tx, event_rx) = mpsc::channel(64);
    let report = eng
        .run_turn(32, "hi", event_tx, CancellationToken::new())
        .await;
    assert_eq!(report.status, TurnStatus::Completed);

    let events = drain_events(event_rx).await;
    assert_epoch(&events, 32);
    assert_single_terminal_last(&events, TurnStatus::Completed);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, EngineEvent::TextFallback { .. })),
        "健康轮不得发正文兜底：{events:?}"
    );
}

/// **rc.3 N0 探针：TTS 传输层失败（连不上）也必须有正文兜底**（2026-09-13）。
///
/// 为什么单列：真机验收时把 `[tts] base_url` 指到一个死端口，浏览器上看到的
/// 仍是「（生成失败）」而不是正文——而上面那条 500 的用例是通过的。传输失败与
/// 上游 5xx 走的是**不同的**错误构造路径（`Error::Transport` vs `Error::Status`），
/// 所以必须单独钉住，不能靠「都是 Tts」推断。
#[tokio::test]
async fn tts_transport_failure_also_falls_back_to_the_generated_text() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("探针句一。探针句二"), sse_done()],
    ))
    .await;

    // 127.0.0.1:9 = discard 端口，本机不监听 → 连接被拒（传输层失败）。
    let mut eng = engine(
        &llm_base,
        "http://127.0.0.1:9/v1",
        ConversationConfig::default(),
    );
    let (event_tx, event_rx) = mpsc::channel(64);
    let report = eng
        .run_turn(41, "hi", event_tx, CancellationToken::new())
        .await;
    assert_eq!(report.status, TurnStatus::Failed);
    assert_eq!(report.assistant_text, "探针句一。探针句二");

    let events = drain_events(event_rx).await;
    assert_epoch(&events, 41);
    assert_single_terminal_last(&events, TurnStatus::Failed);
    let fallbacks: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            EngineEvent::TextFallback { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        fallbacks,
        ["探针句一。探针句二"],
        "TTS 传输失败同样必须兜底整轮正文：{events:?}"
    );
    assert!(matches!(find_error_kind(&events), Some(ErrorKind::Tts(_))));
}
