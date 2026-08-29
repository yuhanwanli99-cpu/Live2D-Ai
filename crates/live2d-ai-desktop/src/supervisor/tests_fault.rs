//! 故障/止损/finish 通道专项：声卡 fatal 路径归一化为 Failed、stop-during-drain
//! 禁止 commit/fallback/后续 Perform、finish 事实穿越 root 闸门（自然完成 vs
//! 跨代次 StaleEpoch）。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use live2d_ai_core::{ActionId, ActionSource, Strength};

use super::support::{
    Collector, count_fact, facts, has_dropped, make_fault_producer, spawn_llm_mock,
    spawn_llm_mock_slow, spawn_text_only_collector, spawn_tts_mock, wait_for,
};
use super::*;
use crate::app_event::{AppEvent, ConversationUiEvent, RootFact};

/// B4-P0：运行中直接 drop SupervisorHandle（不调 stop/quit）⇒ control/finish
/// 通道全部关闭，supervisor 必须把通道关闭视作 Quit、cancel 当前轮、在有限
/// 时间内自然回收（发 ShutdownReady），且不产生第二个 LLM 请求。
///
/// 回归目标：此前 control_rx 关闭后 recv() 永久就绪 None，在 biased select
/// 中每轮必赢，gen_fut 永无轮询机会 → supervisor 线程忙循环无法回收。
#[test]
fn dropping_handle_during_generation_cancels_and_reaps_supervisor() {
    let llm_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"好。。\"}}]}\n\n",
        "data: [DONE]\n\n"
    )
    .to_owned();
    // 慢 LLM：保证 drop 落在生成窗口内（请求已发出、响应未回）。
    let llm_base = spawn_llm_mock_slow(llm_bodies.clone(), sse, Duration::from_millis(500));
    let tts_base = spawn_tts_mock();

    let collector: Collector = Arc::new(Mutex::new(Vec::new()));
    let ec = collector.clone();
    let client = live2d_ai_runtime::OpenAiClient::new(
        live2d_ai_runtime::LlmConfig::new(llm_base, "test-model"),
        live2d_ai_runtime::TtsConfig::new(tts_base, "alloy"),
    )
    .expect("client");

    let handle = spawn_supervisor(
        SupervisorConfig {
            client,
            conversation: live2d_ai_runtime::ConversationConfig::new("人设"),
            capabilities: live2d_ai_core::ModelCapabilities::all(),
            audio: None,

            config_path: None,
        },
        move |ev| {
            ec.lock().expect("poison").push(ev);
        },
    );

    assert!(handle.say("A"));
    // 等 mock 确认请求已开始（生成窗口打开）。
    assert!(
        wait_for(Duration::from_secs(2), || !llm_bodies
            .lock()
            .expect("poison")
            .is_empty()),
        "LLM 请求必须已发出"
    );

    // 直接 drop 句柄，不调 stop/quit：三个 sender 全部销毁。
    drop(handle);

    // 有限时间内 supervisor 必须自然回收（发 ShutdownReady），不得忙循环。
    assert!(
        wait_for(Duration::from_secs(3), || collector
            .lock()
            .expect("poison")
            .iter()
            .any(|e| matches!(e, AppEvent::ShutdownReady))),
        "drop 句柄后 supervisor 必须有限时间内发 ShutdownReady（不得忙循环）"
    );

    // 断开后不得继续发送第二个请求。
    std::thread::sleep(Duration::from_millis(300));
    let bodies = llm_bodies.lock().expect("poison");
    assert_eq!(bodies.len(), 1, "drop 句柄后不得再发 LLM 请求: {bodies:?}");
}

#[test]
fn fatal_after_started_reports_cleared() {
    let (producer, shared_fault, _render) = make_fault_producer();
    // LLM 正常回包（SSE 完成）；fault 由 VoiceStarted 事件回调即时注入。
    let llm_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"好。。\"}}]}\n\n",
        "data: [DONE]\n\n"
    )
    .to_owned();
    let llm_base = spawn_llm_mock(llm_bodies.clone(), sse);
    let tts_base = spawn_tts_mock();

    let collector: Collector = Arc::new(Mutex::new(Vec::new()));
    let ec = collector.clone();
    let fault_flag = shared_fault.clone();
    let client = live2d_ai_runtime::OpenAiClient::new(
        live2d_ai_runtime::LlmConfig::new(llm_base, "test-model"),
        live2d_ai_runtime::TtsConfig::new(tts_base, "alloy"),
    )
    .expect("client");

    let handle = spawn_supervisor(
        SupervisorConfig {
            client,
            conversation: live2d_ai_runtime::ConversationConfig::new("人设"),
            capabilities: live2d_ai_core::ModelCapabilities::all(),
            audio: Some(producer),

            config_path: None,
        },
        move |ev| {
            // VoiceStarted 到达即刻注错：复现「开始播放即失效」。
            if matches!(
                ev,
                AppEvent::Conversation(ConversationUiEvent::VoiceStarted { .. })
            ) {
                fault_flag.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            ec.lock().expect("poison").push(ev);
        },
    );

    assert!(handle.say("A"));
    assert!(
        wait_for(Duration::from_secs(3), || count_fact(
            &collector,
            |f| matches!(f, RootFact::PlaybackCleared { .. })
        ) >= 1),
        "fatal 且曾 Started ⇒ 必须报告 PlaybackCleared"
    );
    // GenerationFinished 恰一次（B2-P0-1）。
    assert_eq!(
        count_fact(&collector, |f| matches!(
            f,
            RootFact::GenerationFinished { .. }
        )),
        1
    );

    handle.quit();
    assert!(wait_for(Duration::from_secs(2), || collector
        .lock()
        .expect("poison")
        .iter()
        .any(|e| matches!(e, AppEvent::ShutdownReady))));
    handle.join();
}

// ---- P0-4: internal_audio_fault_yields_turn_completed_failed ----
//
// 类似 fatal_after_started_reports_cleared，但 P0-2 归一化口径：声卡 fault
// ⇒ outcome == Failed ⇒ TurnCompleted{outcome_completed:false}。这是 P0-2
// 归一化裁决的锁定证据。
#[test]
fn internal_audio_fault_yields_turn_completed_failed() {
    let (producer, shared_fault, _render) = make_fault_producer();
    let llm_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"好。。\"}}]}\n\n",
        "data: [DONE]\n\n"
    )
    .to_owned();
    let llm_base = spawn_llm_mock(llm_bodies.clone(), sse);
    let tts_base = spawn_tts_mock();

    let collector: Collector = Arc::new(Mutex::new(Vec::new()));
    let ec = collector.clone();
    let fault_flag = shared_fault.clone();
    let client = live2d_ai_runtime::OpenAiClient::new(
        live2d_ai_runtime::LlmConfig::new(llm_base, "test-model"),
        live2d_ai_runtime::TtsConfig::new(tts_base, "alloy"),
    )
    .expect("client");

    let handle = spawn_supervisor(
        SupervisorConfig {
            client,
            conversation: live2d_ai_runtime::ConversationConfig::new("人设"),
            capabilities: live2d_ai_core::ModelCapabilities::all(),
            audio: Some(producer),

            config_path: None,
        },
        move |ev| {
            if matches!(
                ev,
                AppEvent::Conversation(ConversationUiEvent::VoiceStarted { .. })
            ) {
                fault_flag.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            ec.lock().expect("poison").push(ev);
        },
    );

    assert!(handle.say("A"));
    // 等待 TurnCompleted 出现（确切的 P0-2 归一化收尾）。
    assert!(
        wait_for(Duration::from_secs(3), || count_fact(
            &collector,
            |f| matches!(f, RootFact::TurnCompleted { .. })
        ) >= 1),
        "fatal 收尾必须发出 TurnCompleted 事实"
    );
    // P0-2 归一化：outcome_completed == false（Failed）。
    let fs = facts(&collector);
    let turn_count = fs
        .iter()
        .filter(|f| matches!(f, RootFact::TurnCompleted { .. }))
        .count();
    let turn_completed_false = fs.iter().any(|f| {
        matches!(
            f,
            RootFact::TurnCompleted {
                outcome_completed: false,
                ..
            }
        )
    });
    assert_eq!(
        turn_count, 1,
        "TurnCompleted 恰一次（无 double-latch 漏报或重发）"
    );
    assert!(
        turn_completed_false,
        "TurnCompleted 必须 outcome_completed=false（Failed 归一化）"
    );
    // 兜底：PlaybackCleared 也出现（与 fatal_after_started 路径一致）。
    assert!(
        fs.iter()
            .any(|f| matches!(f, RootFact::PlaybackCleared { .. }))
    );

    handle.quit();
    assert!(wait_for(Duration::from_secs(2), || collector
        .lock()
        .expect("poison")
        .iter()
        .any(|e| matches!(e, AppEvent::ShutdownReady))));
    handle.join();
}

// ---- P0-6: action_finished_fact_travels_real_finish_channel ----
//
// 基线：spawn_supervisor 空闲态 handle.report_action_finished(0, action) ⇒
// collector 断言 RootFact::Dropped 出现（事实穿越真实 finish_rx 通道到达 root，
// 由 root 闸门判定 StaleEpoch ⇒ Dropped，forward_effects 把 Dropped 投影为
// RootFact::Dropped）。
//
// 进阶：让 epoch0 出现 Render(Perform)，handle.report_action_finished(0, action) ⇒
// 随后出现 Render(RenderCommand::CeasePerforming{epoch:0})，且全程无 Dropped
// （动作的"自然完成"以 root 闸门接受，root 触发 ActionEffect::End ⇒ 翻译为
// CeasePerforming，不走 Dropped 路径）。
#[test]
fn action_finished_fact_travels_real_finish_channel() {
    // ---- 基线：空闲态 finish ⇒ Dropped ----
    {
        let collector: Collector = Arc::new(Mutex::new(Vec::new()));
        let ec = collector.clone();
        let client = live2d_ai_runtime::OpenAiClient::new(
            live2d_ai_runtime::LlmConfig::new("http://127.0.0.1:1", "m"),
            live2d_ai_runtime::TtsConfig::new("http://127.0.0.1:1", "v"),
        )
        .expect("client");
        let handle = spawn_supervisor(
            SupervisorConfig {
                client,
                conversation: live2d_ai_runtime::ConversationConfig::new("人设"),
                capabilities: live2d_ai_core::ModelCapabilities::all(),
                audio: None,

                config_path: None,
            },
            move |ev| {
                ec.lock().expect("poison").push(ev);
            },
        );

        let action = SemanticAction::new(ActionId::Nod, Strength::Low, ActionSource::LlmTool);
        handle.report_action_finished(0, action);

        assert!(
            wait_for(Duration::from_secs(2), || has_dropped(&collector)),
            "事实必须经真实 finish_rx 通道到达 root 并被投影为 Dropped"
        );

        handle.quit();
        assert!(wait_for(Duration::from_secs(2), || collector
            .lock()
            .expect("poison")
            .iter()
            .any(|e| matches!(e, AppEvent::ShutdownReady))));
        handle.join();
    }

    // ---- 进阶：fallback "好的" ⇒ Perform{epoch:0}，随后 finish ⇒ CeasePerforming{epoch:0} ----
    let (handle, collector, _) = spawn_text_only_collector(
        "data: {\"choices\":[{\"delta\":{\"content\":\"好的\"}}]}\n\ndata: [DONE]\n\n".to_owned(),
    );
    assert!(handle.say("hi"));
    let action = SemanticAction::new(ActionId::Nod, Strength::Low, ActionSource::RuleFallback);
    // 等 Perform{epoch:0, action=...} 出现再回报完成。
    let action_for_match = action;
    assert!(
        wait_for(Duration::from_secs(3), || collector
            .lock()
            .expect("poison")
            .iter()
            .any(|e| matches!(
                e,
                AppEvent::Render(crate::app_event::RenderCommand::Perform { epoch: 0, action: a })
                    if *a == action_for_match
            ))),
        "「好的」必须经 fallback ⇒ Perform{{epoch:0,action}}"
    );
    handle.report_action_finished(0, action);
    assert!(
        wait_for(Duration::from_secs(3), || collector
            .lock()
            .expect("poison")
            .iter()
            .any(|e| matches!(
                e,
                AppEvent::Render(crate::app_event::RenderCommand::CeasePerforming { epoch: 0 })
            ))),
        "finish 经真实通道 ⇒ root ActionEffect::End ⇒ CeasePerforming{{epoch:0}}"
    );
    // 全程无 Dropped（动作完成被 root 接受，未被丢弃）。
    assert!(
        !has_dropped(&collector),
        "自然完成路径不得触发 Dropped 投影"
    );

    handle.quit();
    assert!(wait_for(Duration::from_secs(2), || collector
        .lock()
        .expect("poison")
        .iter()
        .any(|e| matches!(e, AppEvent::ShutdownReady))));
    handle.join();
}
