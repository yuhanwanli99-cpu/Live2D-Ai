//! 集成测试：`ConversationEngine` 流水线（本地 tokio TCP mock，不引第三方依赖）。
//!
//! 覆盖：
//! - **流水线重叠**：LLM 还在流句子时，第一句的 `/audio/speech` 请求已经到达
//!   （用全局顺序日志证明：TTS 首请求先于 LLM 写出 `[DONE]`）；
//! - **TTS 严格串行**：多句的合成请求按句子顺序逐个发生；
//! - **PCM 奇数字节 chunk**：mock 按 3 字节切块，解码结果与整段一致；
//! - 工具调用 → `ToolAction`；残缺工具调用可见但**不阻断**已生成文本/TTS；
//! - 取消：终态恰一次 `Cancelled`、其后无任何迟到事件；预取消零业务事件；
//!   消费者提前离场视同取消；
//! - 错误分类 Llm / Tts / Decode / Backpressure 各自可观测；每轮 `Done` 恰一次、
//!   位于最后（LLM 结束且 TTS worker 完全排空之后）。

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use live2d_ai_runtime::{
    ConversationConfig, ConversationEngine, EngineEvent, ErrorKind, LlmConfig, OpenAiClient,
    PerformAction, TtsConfig, TurnStatus,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

const READ_TIMEOUT: Duration = Duration::from_secs(5);
/// 单步兜底超时：流水线若卡死，让测试失败而不是挂死。
const STEP_TIMEOUT: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------- mock 基建

/// 单个连接的处理 future。
type BoxFut = Pin<Box<dyn Future<Output = ()> + Send>>;
/// 共享的连接处理闭包。
type SharedHandler = Arc<dyn Fn(TcpStream) -> BoxFut + Send + Sync>;

/// 可克隆的连接处理器（每次 accept 一个连接）。
#[derive(Clone)]
struct Handler(SharedHandler);

impl Handler {
    fn new<F>(f: F) -> Self
    where
        F: Fn(TcpStream) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync + 'static,
    {
        Self(Arc::new(f))
    }
}

#[derive(Debug)]
struct CapturedRequest {
    body: Vec<u8>,
}

fn contains(haystack: &[u8], needle: &[u8; 4]) -> Option<usize> {
    haystack.windows(4).position(|w| w == needle)
}

async fn read_request(sock: &mut TcpStream) -> CapturedRequest {
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
async fn read_speech_input(sock: &mut TcpStream) -> String {
    let req = read_request(sock).await;
    let json: serde_json::Value = serde_json::from_slice(&req.body).expect("json body");
    json["input"].as_str().expect("input 字段").to_string()
}

/// 写响应头（content-length 由 pieces 总长决定）。
async fn write_head(
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
async fn write_pieces(sock: &mut TcpStream, pieces: &[Vec<u8>], piece_delay_ms: u64) {
    for piece in pieces {
        if piece_delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(piece_delay_ms)).await;
        }
        sock.write_all(piece).await.expect("write piece");
    }
}

/// 关闭写端（发 FIN）：响应体写完后调用。
async fn close_sock(sock: &mut TcpStream) {
    sock.shutdown().await.expect("shutdown");
}

/// 静态分片响应处理器：每个连接收到相同的预设响应。
fn respond_pieces(
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
async fn spawn_multi_server(handler: Handler) -> String {
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

fn sse_content(text: &str) -> Vec<u8> {
    format!("data: {{\"choices\":[{{\"delta\":{{\"content\":\"{text}\"}}}}]}}\n\n").into_bytes()
}

/// 工具调用增量事件。`arguments` 是**线上原样**的参数 JSON 文本片段
/// （serde_json 负责把它作为字符串值正确转义，避免手写大括号出错）。
fn sse_tool(index: usize, id: &str, name: &str, arguments: &str) -> Vec<u8> {
    let chunk = serde_json::json!({
        "choices": [{
            "delta": {
                "tool_calls": [{
                    "index": index,
                    "id": id,
                    "type": "function",
                    "function": { "name": name, "arguments": arguments },
                }],
            },
        }],
    });
    format!("data: {chunk}\n\n").into_bytes()
}

fn sse_done() -> Vec<u8> {
    b"data: [DONE]\n\n".to_vec()
}

// ---------------------------------------------------------------- 断言工具

/// 所有事件都必须携带本轮 epoch。
fn assert_epoch(events: &[EngineEvent], epoch: u64) {
    let got: Vec<u64> = events
        .iter()
        .map(|e| match e {
            EngineEvent::TextDelta { epoch, .. }
            | EngineEvent::ToolAction { epoch, .. }
            | EngineEvent::AudioChunk { epoch, .. }
            | EngineEvent::Error { epoch, .. }
            | EngineEvent::Done { epoch }
            | EngineEvent::Cancelled { epoch } => *epoch,
        })
        .collect();
    assert!(got.iter().all(|&e| e == epoch), "epoch 不一致: {got:?}");
}

/// Done 必须恰好一次且是最后一个事件。
fn assert_single_done_last(events: &[EngineEvent]) {
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, EngineEvent::Done { .. }))
            .count(),
        1,
        "Done 必须恰好一次: {events:?}"
    );
    assert!(
        matches!(events.last(), Some(EngineEvent::Done { .. })),
        "Done 必须是最后一个事件"
    );
}

/// 按 sentence_seq 聚合音频样本；断言 seq 连续、句间严格串行、
/// 每句恰好一个 final_chunk 且为该句最后一块。
fn collect_audio(events: &[EngineEvent]) -> Vec<(u64, Vec<f32>)> {
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

fn find_error_kind(events: &[EngineEvent]) -> Option<&ErrorKind> {
    events.iter().find_map(|e| match e {
        EngineEvent::Error { kind, .. } => Some(kind),
        _ => None,
    })
}

fn engine(llm_base: &str, tts_base: &str, config: ConversationConfig) -> ConversationEngine {
    let client = OpenAiClient::new(
        LlmConfig::new(llm_base, "test-model"),
        TtsConfig::new(tts_base, "alloy"),
    )
    .expect("client");
    ConversationEngine::new(client, config)
}

/// 通用 TTS mock：记录到达的 input 顺序；返回确定性 PCM（按 3 字节奇数切块）。
async fn spawn_tts_mock() -> (String, Arc<Mutex<Vec<String>>>) {
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
async fn drain_events(mut event_rx: mpsc::Receiver<EngineEvent>) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    while let Some(event) = event_rx.recv().await {
        events.push(event);
    }
    events
}

// ---------------------------------------------------------------- 用例

/// 流水线重叠 + TTS 严格串行 + 奇数字节 PCM 解码 + 终态顺序 + 历史提交。
#[tokio::test]
async fn pipeline_overlaps_llm_streaming_with_first_tts_request() {
    // 全局顺序日志：证明 [DONE] 写出前，第一句 TTS 请求已经到达。
    let log: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));

    // TTS 首请求信号：watch 便于跨任务共享（oneshot 不可克隆）。
    let (first_tts_tx, first_tts_rx) = tokio::sync::watch::channel(false);
    let first_tts_tx_slot = Mutex::new(Some(first_tts_tx));

    let llm_log = log.clone();
    let llm_base = spawn_multi_server(Handler::new(move |mut sock| {
        let log = llm_log.clone();
        let mut first_tts_rx = first_tts_rx.clone();
        Box::pin(async move {
            let _req = read_request(&mut sock).await;
            let pieces = [
                sse_content("你好呀！"),
                sse_content("今"), // 前瞻字符封口第一句
                sse_content("天天气不错。"),
                sse_content("好"), // 封口第二句
                sse_done(),
            ];
            let total: usize = pieces.iter().map(Vec::len).sum();
            write_head(&mut sock, 200, "OK", "text/event-stream", total).await;
            // 先只流前两片：此时引擎应已切出第一句并发起 TTS。
            write_pieces(&mut sock, &pieces[..2], 15).await;
            // 关键：等第一句 TTS 请求真正到达 mock 后才继续流 —— 否则测试超时失败，
            // 即「不等待 LLM 全部结束就开始 TTS」的结构性证明。
            timeout(STEP_TIMEOUT, first_tts_rx.changed())
                .await
                .expect("TTS 第一句请求未在 LLM 流结束前到达：无流水线重叠")
                .expect("watch closed");
            log.lock().unwrap().push("llm:tts-seen-before-done");
            write_pieces(&mut sock, &pieces[2..], 15).await;
            close_sock(&mut sock).await;
        })
    }))
    .await;

    let tts_log = log.clone();
    let tts_inputs: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let tts_handler_inputs = tts_inputs.clone();
    let slot = Arc::new(first_tts_tx_slot);
    let tts_base = spawn_multi_server(Handler::new(move |mut sock| {
        let inputs = tts_handler_inputs.clone();
        let log = tts_log.clone();
        let slot = slot.clone();
        Box::pin(async move {
            let input = read_speech_input(&mut sock).await;
            inputs.lock().unwrap().push(input.clone());
            log.lock().unwrap().push("tts:first-request-received");
            if let Some(tx) = slot.lock().unwrap().take() {
                let _ = tx.send(true);
            }

            let samples: Vec<i16> = match input.as_str() {
                "你好呀！" => vec![300, -301],
                "今天天气不错。" => vec![400, -401, 402],
                // flush 收尾的残余句（"好"）：确定性非空样本即可。
                "好" => vec![500, 501],
                other => panic!("意外的 TTS 输入: {other:?}"),
            };
            let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
            let pieces: Vec<Vec<u8>> = bytes.chunks(3).map(<[u8]>::to_vec).collect();
            let total: usize = pieces.iter().map(Vec::len).sum();
            write_head(&mut sock, 200, "OK", "audio/pcm", total).await;
            write_pieces(&mut sock, &pieces, 5).await;
            close_sock(&mut sock).await;
        })
    }))
    .await;

    let mut eng = engine(
        &llm_base,
        &tts_base,
        ConversationConfig {
            max_history_pairs: 2,
            ..ConversationConfig::new("你是桌宠")
        },
    );

    let (event_tx, event_rx) = mpsc::channel(64);
    let report = eng
        .run_turn(7, "打个招呼", event_tx, CancellationToken::new())
        .await;
    assert_eq!(report.status, TurnStatus::Completed);
    assert_eq!(report.assistant_text, "你好呀！今天天气不错。好");

    let events = drain_events(event_rx).await;
    assert_epoch(&events, 7);
    assert_single_done_last(&events);
    assert!(
        events
            .iter()
            .all(|e| !matches!(e, EngineEvent::Error { .. } | EngineEvent::Cancelled { .. })),
        "正常轮不允许 Error/Cancelled: {events:?}"
    );

    // 文本增量原样实时转发。
    assert_eq!(
        events
            .iter()
            .filter_map(|e| match e {
                EngineEvent::TextDelta { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        ["你好呀！", "今", "天天气不错。", "好"]
    );

    // TTS 严格按句子顺序串行（请求到达序 == 播放序）。
    assert_eq!(
        *tts_inputs.lock().unwrap(),
        ["你好呀！", "今天天气不错。", "好"]
    );

    // 音频：seq 从 1 连续递增；每句一个 final_chunk；奇数字节切块解码无损。
    let scale = 1.0f32 / 32_768.0;
    let audio = collect_audio(&events);
    assert_eq!(audio.len(), 3);
    assert_eq!(audio[0].0, 1);
    assert_eq!(audio[0].1, vec![300f32 * scale, -301.0 * scale]);
    assert_eq!(
        audio[1],
        (2, vec![400f32 * scale, -401.0 * scale, 402.0 * scale])
    );
    assert_eq!(audio[2].0, 3);
    assert!(!audio[2].1.is_empty());

    // 重叠的直接证据：全局日志里 TTS 首请求先于 LLM 继续（即先于 [DONE]）。
    let log = log.lock().unwrap().clone();
    let tts_pos = log
        .iter()
        .position(|e| *e == "tts:first-request-received")
        .expect("tts 日志缺失");
    let done_pos = log
        .iter()
        .position(|e| *e == "llm:tts-seen-before-done")
        .expect("llm 日志缺失");
    assert!(tts_pos < done_pos, "全局顺序非重叠: {log:?}");

    // 历史已提交（max_history_pairs=2）且 clear_history 生效。
    assert_eq!(eng.history_len(), 1);
    eng.clear_history();
    assert_eq!(eng.history_len(), 0);
}

/// 成功轮的历史会进入下一轮请求体（system + 历史 + 当前输入）。
#[tokio::test]
async fn successful_turn_commits_history_into_next_request() {
    let bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let handler_bodies = bodies.clone();
    let llm_base = spawn_multi_server(Handler::new(move |mut sock| {
        let bodies = handler_bodies.clone();
        Box::pin(async move {
            let req = read_request(&mut sock).await;
            let json: serde_json::Value = serde_json::from_slice(&req.body).expect("json");
            bodies.lock().unwrap().push(json);
            let pieces = vec![sse_content("好。。"), sse_done()];
            let total: usize = pieces.iter().map(Vec::len).sum();
            write_head(&mut sock, 200, "OK", "text/event-stream", total).await;
            write_pieces(&mut sock, &pieces, 5).await;
            close_sock(&mut sock).await;
        })
    }))
    .await;
    let (tts_base, _) = spawn_tts_mock().await;

    let mut eng = engine(
        &llm_base,
        &tts_base,
        ConversationConfig {
            max_history_pairs: 4,
            ..ConversationConfig::new("人设提示")
        },
    );
    for round in ["第一轮", "第二轮"] {
        let (event_tx, event_rx) = mpsc::channel(64);
        let report = eng
            .run_turn(1, round, event_tx, CancellationToken::new())
            .await;
        assert_eq!(report.status, TurnStatus::Completed);
        let events = drain_events(event_rx).await;
        assert_single_done_last(&events);
    }

    let bodies = bodies.lock().unwrap();
    assert_eq!(bodies.len(), 2);
    // 第一轮：system + user。
    let msgs = bodies[0]["messages"].as_array().expect("messages");
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0]["role"], "system");
    assert_eq!(msgs[0]["content"], "人设提示");
    assert_eq!(msgs[1]["role"], "user");
    assert_eq!(msgs[1]["content"], "第一轮");
    // 第二轮：system + 历史(1对) + 当前输入。
    let msgs = bodies[1]["messages"].as_array().expect("messages");
    assert_eq!(msgs.len(), 4);
    assert_eq!(msgs[1]["role"], "user");
    assert_eq!(msgs[1]["content"], "第一轮");
    assert_eq!(msgs[2]["role"], "assistant");
    assert_eq!(msgs[2]["content"], "好。。");
    assert_eq!(msgs[3]["content"], "第二轮");
}

/// 完整工具调用 → ToolAction 事件；正文照常切句/TTS 不受影响。
#[tokio::test]
async fn tool_call_becomes_tool_action_event() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![
            sse_content("点头吧。好"),
            sse_tool(
                0,
                "call_9",
                "live2d_perform_action",
                r#"{"action":"nod","strength":2}"#,
            ),
            sse_done(),
        ],
    ))
    .await;
    let (tts_base, tts_inputs) = spawn_tts_mock().await;

    let mut eng = engine(&llm_base, &tts_base, ConversationConfig::default());
    let (event_tx, event_rx) = mpsc::channel(64);
    let report = eng
        .run_turn(3, "同意一下", event_tx, CancellationToken::new())
        .await;
    assert_eq!(report.status, TurnStatus::Completed);

    let events = drain_events(event_rx).await;
    assert_epoch(&events, 3);
    assert_single_done_last(&events);

    let tools: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            EngineEvent::ToolAction {
                epoch,
                index,
                id,
                args,
            } => Some((*epoch, *index, id.clone(), args.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(tools.len(), 1, "{events:?}");
    assert_eq!(tools[0].0, 3);
    assert_eq!(tools[0].1, 0);
    assert_eq!(tools[0].2.as_deref(), Some("call_9"));
    assert_eq!(tools[0].3.action, PerformAction::Nod);
    assert_eq!(tools[0].3.strength, 2);

    // 文本没有被工具阻断：两句都完成 TTS。
    assert_eq!(*tts_inputs.lock().unwrap(), ["点头吧。", "好"]);
    assert_eq!(collect_audio(&events).len(), 2);
}

/// 残缺工具调用：Error 可见，但不阻断已生成文本/TTS；最终仍是 Completed+Done。
#[tokio::test]
async fn incomplete_tool_error_is_visible_without_blocking_text() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![
            sse_content("先说话。"),
            // 故意截断 arguments：流结束时结算为类型化 Incomplete。
            sse_tool(0, "c0", "live2d_perform_action", r#"{"action":"nod","#),
            sse_done(),
        ],
    ))
    .await;
    let (tts_base, tts_inputs) = spawn_tts_mock().await;

    let mut eng = engine(&llm_base, &tts_base, ConversationConfig::default());
    let (event_tx, event_rx) = mpsc::channel(64);
    let report = eng
        .run_turn(4, "嗯", event_tx, CancellationToken::new())
        .await;
    // 可恢复错误不改变 Completed 语义；正文完整。
    assert_eq!(report.status, TurnStatus::Completed);
    assert_eq!(report.assistant_text, "先说话。");

    let events = drain_events(event_rx).await;
    assert_epoch(&events, 4);
    assert_single_done_last(&events);
    // 已生成文本的音频完整交付。
    assert_eq!(*tts_inputs.lock().unwrap(), ["先说话。"]);
    assert_eq!(collect_audio(&events).len(), 1);
    // 没有任何 ToolAction（调用未完成），但有类型化 IncompleteTools 错误事件。
    assert!(
        events
            .iter()
            .all(|e| !matches!(e, EngineEvent::ToolAction { .. }))
    );
    match find_error_kind(&events) {
        Some(ErrorKind::IncompleteTools(err)) => {
            assert_eq!(err.calls.len(), 1);
            assert_eq!(err.calls[0].index, 0);
            assert_eq!(err.calls[0].id.as_deref(), Some("c0"));
        }
        other => panic!("期望 IncompleteTools 错误，实际 {other:?}"),
    }
    assert!(
        matches!(events.last(), Some(EngineEvent::Done { .. })),
        "Error 之后仍以 Done 收尾"
    );
}

/// LLM 非 2xx：Error(Llm) → Done，TTS 零请求。
#[tokio::test]
async fn llm_status_error_is_observable_then_done() {
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
    assert_single_done_last(&events);
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

/// TTS 500：Error(Tts) 致命——丢弃剩余排队句子，Done 收尾。
#[tokio::test]
async fn tts_error_stops_turn_with_error_then_done() {
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
    assert_single_done_last(&events);
    // 第二句「第二」被丢弃：worker 在首个致命错误后退出。
    assert_eq!(*inputs.lock().unwrap(), ["第一句。"]);
    match find_error_kind(&events) {
        Some(ErrorKind::Tts(e)) => assert!(e.to_string().contains("engine offline"), "{e}"),
        other => panic!("期望 Tts 错误，实际 {other:?}"),
    }
}

/// PCM 总字节数为奇数 → Error(Decode)，Done 收尾。
#[tokio::test]
async fn odd_total_pcm_bytes_yield_decode_error_then_done() {
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
    assert_single_done_last(&events);
    assert!(matches!(
        find_error_kind(&events),
        Some(ErrorKind::Decode(_))
    ));
}

/// 有界队列背压：下游长期不消费且配置了推送超时 → Error(Backpressure) → Done。
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
    assert_single_done_last(&events);
    assert!(matches!(
        find_error_kind(&events),
        Some(ErrorKind::Backpressure { .. })
    ));
    // 只有第 1 句真正发起了 TTS 请求。
    assert_eq!(tts_hits.lock().unwrap().len(), 1);
}

/// 中途取消：终态恰为一次 Cancelled，其后无任何事件（发送端全部关闭即证）。
#[tokio::test]
async fn cancel_mid_turn_emits_only_cancelled_with_no_late_events() {
    let llm_base = spawn_multi_server(Handler::new(|mut sock| {
        Box::pin(async move {
            let _req = read_request(&mut sock).await;
            let pieces = vec![sse_content("你好呀！"), sse_content("今")];
            let total: usize = pieces.iter().map(Vec::len).sum();
            write_head(&mut sock, 200, "OK", "text/event-stream", total).await;
            write_pieces(&mut sock, &pieces, 10).await;
            close_sock(&mut sock).await;
            // 之后长时间不再输出：给测试留出确定的取消窗口。
            tokio::time::sleep(Duration::from_secs(60)).await;
        })
    }))
    .await;
    let (tts_base, tts_inputs) = spawn_tts_mock().await;

    let mut eng = engine(&llm_base, &tts_base, ConversationConfig::default());
    let (event_tx, mut event_rx) = mpsc::channel(64);
    let cancel = CancellationToken::new();
    let turn_cancel = cancel.clone();
    let task = tokio::spawn(async move { eng.run_turn(21, "hi", event_tx, turn_cancel).await });

    // 收到第一个 TextDelta 后立即取消。
    let mut seen = Vec::new();
    loop {
        let event = timeout(STEP_TIMEOUT, event_rx.recv())
            .await
            .expect("等待事件超时")
            .expect("事件通道过早关闭");
        let was_text = matches!(event, EngineEvent::TextDelta { .. });
        seen.push(event);
        if was_text {
            cancel.cancel();
            break;
        }
    }

    // run_turn 必须及时返回（内部 worker 一并退出，无泄漏）。
    let report = timeout(STEP_TIMEOUT, task)
        .await
        .expect("run_turn 未在取消后及时返回")
        .expect("join ok");
    assert_eq!(report.status, TurnStatus::Cancelled);

    // 排干缓冲事件；recv 返回 None ⇒ 所有发送端已 drop 且没有任何迟到事件。
    let rest = drain_events(event_rx).await;
    let mut events = seen;
    events.extend(rest);

    assert_epoch(&events, 21);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, EngineEvent::Cancelled { .. }))
            .count(),
        1
    );
    assert!(
        matches!(events.last(), Some(EngineEvent::Cancelled { .. })),
        "Cancelled 必须是最后一个事件: {events:?}"
    );
    assert!(!events.iter().any(|e| matches!(e, EngineEvent::Done { .. })));
    // 取消窗口内至多一句进入过 TTS。
    assert!(tts_inputs.lock().unwrap().len() <= 1);
}

/// 预先取消的令牌：零网络请求、唯一事件就是 Cancelled。
#[tokio::test]
async fn pre_cancelled_token_skips_network_entirely() {
    let llm_hits: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
    let handler_hits = llm_hits.clone();
    let llm_base = spawn_multi_server(Handler::new(move |sock| {
        let hits = handler_hits.clone();
        Box::pin(async move {
            *hits.lock().unwrap() += 1;
            drop(sock);
        })
    }))
    .await;
    let (tts_base, tts_inputs) = spawn_tts_mock().await;

    let mut eng = engine(&llm_base, &tts_base, ConversationConfig::default());
    let (event_tx, event_rx) = mpsc::channel(64);
    let cancel = CancellationToken::new();
    cancel.cancel();
    let report = eng.run_turn(31, "hi", event_tx, cancel).await;
    assert_eq!(report.status, TurnStatus::Cancelled);
    assert_eq!(report.assistant_text, "");

    let events = drain_events(event_rx).await;
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], EngineEvent::Cancelled { epoch: 31 }));
    assert_eq!(*llm_hits.lock().unwrap(), 0);
    assert!(tts_inputs.lock().unwrap().is_empty());
}

/// 事件消费者提前离场（drop receiver）：视同取消，引擎安静收场。
#[tokio::test]
async fn consumer_drop_is_treated_as_cancellation() {
    let llm_base = spawn_multi_server(Handler::new(|mut sock| {
        Box::pin(async move {
            let _req = read_request(&mut sock).await;
            let piece = sse_content("你好呀！");
            write_head(&mut sock, 200, "OK", "text/event-stream", piece.len()).await;
            sock.write_all(&piece).await.expect("write");
            tokio::time::sleep(Duration::from_secs(60)).await;
        })
    }))
    .await;
    let (tts_base, _) = spawn_tts_mock().await;

    let mut eng = engine(&llm_base, &tts_base, ConversationConfig::default());
    let (event_tx, event_rx) = mpsc::channel(4);
    drop(event_rx); // 消费者直接消失。
    let report = timeout(
        STEP_TIMEOUT,
        eng.run_turn(41, "hi", event_tx, CancellationToken::new()),
    )
    .await
    .expect("必须在消费者离场后及时收场");
    assert_eq!(report.status, TurnStatus::Cancelled);
}
