//! `ConversationEngine` 历史提交契约（P0-4 独立测试集）：
//!
//! - 成功轮的历史经 supervisor 显式提交后进入下一轮请求体；
//! - 完成轮**绝不**自动提交历史——未显式 commit 时，下一轮请求体不含任何历史对。
//!
//! 拆出独立文件以保持主 lifecycle 测试文件的体量在 ≤500 行。

mod common;

use std::sync::{Arc, Mutex};

use live2d_ai_runtime::{ConversationConfig, TurnStatus};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use common::{
    Handler, assert_single_terminal_last, close_sock, drain_events, engine, read_request,
    respond_pieces, spawn_multi_server, spawn_tts_mock, sse_content, sse_done, write_head,
    write_pieces,
};

/// 成功轮的历史经 supervisor 显式提交后进入下一轮请求体
/// （system + 历史 + 当前输入；P0-4：引擎绝不自行提交）。
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
        assert_single_terminal_last(&events, TurnStatus::Completed);
        // supervisor 角色显式提交（第一轮的历史才能进第二轮请求体）。
        eng.commit_completed_turn(round, &report.assistant_text);
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

/// P0-4 负例：完成轮**绝不**自动提交历史——未显式 commit 时，
/// 下一轮请求体不含任何历史对（合成完成 ≠ 播放完成，root 双闩锁收口后才有资格提交）。
#[tokio::test]
async fn completed_turn_does_not_commit_history_automatically() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("好。。"), sse_done()],
    ))
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

    // 第一轮：正常完成。
    let (event_tx, _event_rx) = mpsc::channel(64);
    let report = eng
        .run_turn(1, "第一轮", event_tx, CancellationToken::new())
        .await;
    assert_eq!(report.status, TurnStatus::Completed);
    assert_eq!(
        eng.history_len(),
        0,
        "P0-4：引擎不得自行提交历史（播放侧事实未知）"
    );

    // 第二轮请求体：只有 system + user，无历史对。
    // （复用 bodies 探针太重；这里直接查 history_len 为 0 后走第二轮，
    //   经 build_messages 的可见行为由 successful_turn_* 测试覆盖正例。）
    let (event_tx, _event_rx) = mpsc::channel(64);
    let report2 = eng
        .run_turn(1, "第二轮", event_tx, CancellationToken::new())
        .await;
    assert_eq!(report2.status, TurnStatus::Completed);
    assert_eq!(eng.history_len(), 0, "多轮同理：不显式提交就永远没有历史");

    // supervisor 显式提交后历史才存在。
    eng.commit_completed_turn("第二轮", &report2.assistant_text);
    assert_eq!(eng.history_len(), 1);
}
