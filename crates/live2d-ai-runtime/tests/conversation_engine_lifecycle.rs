//! `ConversationEngine` 生命周期集成测试：流水线重叠、
//! 取消与预取消、消费者离场。
//!
//! 2026-09-11 用户裁决：LLM 不暴露任何工具、只做对话——本文件里
//! 「完整工具调用 → ToolAction」「残缺工具调用 → IncompleteTools」两条
//! 用例已随工具整体删除。
//!
//! 历史提交契约见 `conversation_engine_history.rs`（独立文件，控制本文件体量）。

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use live2d_ai_runtime::{ConversationConfig, EngineEvent, TurnStatus};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use common::{
    Handler, STEP_TIMEOUT, assert_epoch, assert_single_terminal_last, close_sock, collect_audio,
    drain_events, engine, read_request, read_speech_input, spawn_multi_server, spawn_tts_mock,
    sse_content, sse_done, write_head, write_pieces,
};

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
    assert_single_terminal_last(&events, TurnStatus::Completed);
    assert!(
        events
            .iter()
            .all(|e| !matches!(e, EngineEvent::Error { .. })),
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

    // **一句一单元（2026-09-10）**：每句语音**完整合成**后恰好发一次
    // `SentenceVoiced`，携带该句完整文本；顺序必须是「先音频、后上屏」。
    let voiced: Vec<(u64, String)> = events
        .iter()
        .filter_map(|e| match e {
            EngineEvent::SentenceVoiced {
                sentence_seq, text, ..
            } => Some((*sentence_seq, text.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        voiced,
        [
            (1, "你好呀！".to_string()),
            (2, "今天天气不错。".to_string()),
            (3, "好".to_string()),
        ],
        "每句恰好一次 SentenceVoiced，且文本完整"
    );

    // 顺序契约：第 N 句的 SentenceVoiced 必须晚于该句的 final AudioChunk。
    for seq in 1..=3u64 {
        let last_audio = events
            .iter()
            .rposition(|e| {
                matches!(e, EngineEvent::AudioChunk { sentence_seq, final_chunk: true, .. }
                    if *sentence_seq == seq)
            })
            .unwrap_or_else(|| panic!("句 {seq} 缺少 final AudioChunk"));
        let voiced_pos = events
            .iter()
            .position(|e| {
                matches!(e, EngineEvent::SentenceVoiced { sentence_seq, .. } if *sentence_seq == seq)
            })
            .unwrap_or_else(|| panic!("句 {seq} 缺少 SentenceVoiced"));
        assert!(
            last_audio < voiced_pos,
            "句 {seq}：必须先发音频（{last_audio}）再上屏（{voiced_pos}）"
        );
    }

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

    // P0-4：run_turn **不自动**提交历史——supervisor 在真实播放排空后
    // 显式调用 commit_completed_turn；这里模拟该调用并验证 clear 生效。
    assert_eq!(eng.history_len(), 0);
    eng.commit_completed_turn("打个招呼", &report.assistant_text);
    assert_eq!(eng.history_len(), 1);
    eng.clear_history();
    assert_eq!(eng.history_len(), 0);
}

/// 中途取消：终态恰为一次 Terminal{Cancelled}，其后无任何事件
/// （发送端全部关闭即证）。
#[tokio::test]
async fn cancel_mid_turn_emits_single_cancelled_terminal() {
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
    // 步骤 7/D8 统一终态：取消恰一次 Terminal{Cancelled}，且必为最后事件。
    let terminals: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, EngineEvent::Terminal { .. }))
        .collect();
    assert_eq!(terminals.len(), 1, "恰好一次终态: {events:?}");
    assert!(
        matches!(
            terminals[0],
            EngineEvent::Terminal {
                status: TurnStatus::Cancelled,
                ..
            }
        ),
        "终态应为 Cancelled: {terminals:?}"
    );
    assert!(
        matches!(events.last(), Some(EngineEvent::Terminal { .. })),
        "Terminal 必须是最后一个事件: {events:?}"
    );
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
    assert!(matches!(
        events[0],
        EngineEvent::Terminal {
            epoch: 31,
            status: TurnStatus::Cancelled,
            ..
        }
    ));
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
