//! 对话历史按会话分桶：请求体里只能看见本会话已经提交的轮次。
//!
//! 全局桶（不带 `session_id`）的连续两轮仍由
//! `tests_loop::closed_loop_two_turns_commit_history_via_drain_path` 锁住。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::support::{Collector, spawn_llm_mock, spawn_tts_mock, wait_for};
use super::*;
use crate::app_event::AppEvent;

fn user_contents(body: &serde_json::Value) -> Vec<String> {
    body["messages"]
        .as_array()
        .expect("messages")
        .iter()
        .filter(|m| m["role"] == "user")
        .map(|m| m["content"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[test]
fn session_history_returns_with_its_session_and_not_the_other() {
    let llm_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"好。。\"}}]}\n\n",
        "data: [DONE]\n\n"
    )
    .to_owned();
    let llm_base = spawn_llm_mock(llm_bodies.clone(), sse);
    let tts_base = spawn_tts_mock();
    let client = live2d_ai_runtime::OpenAiClient::new(
        live2d_ai_runtime::LlmConfig::new(llm_base, "test-model"),
        live2d_ai_runtime::TtsConfig::new(tts_base, "alloy"),
    )
    .expect("client");

    let collector: Collector = Arc::new(Mutex::new(Vec::new()));
    let emit_collector = collector.clone();
    let handle = spawn_supervisor(
        SupervisorConfig {
            client,
            conversation: live2d_ai_runtime::ConversationConfig {
                max_history_pairs: 4,
                ..live2d_ai_runtime::ConversationConfig::new("人设")
            },
            capabilities: live2d_ai_core::ModelCapabilities::all(),
            audio: None,
            config_path: None,
            mod_events: None,
        },
        move |ev| emit_collector.lock().expect("poison").push(ev),
    );

    let gen_finished_count = |c: &Collector| {
        c.lock()
            .expect("poison")
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    AppEvent::RootAudit(crate::app_event::RootFact::GenerationFinished { .. })
                )
            })
            .count()
    };

    assert!(handle.say_scoped("甲", Some("session-a".to_string())));
    assert!(wait_for(Duration::from_secs(3), || {
        gen_finished_count(&collector) >= 1
    }));
    assert!(handle.say_scoped("乙", Some("session-b".to_string())));
    assert!(wait_for(Duration::from_secs(3), || {
        gen_finished_count(&collector) >= 2
    }));
    assert!(handle.say_scoped("丙", Some("session-a".to_string())));
    assert!(wait_for(Duration::from_secs(3), || {
        gen_finished_count(&collector) >= 3
    }));

    handle.quit();
    assert!(wait_for(Duration::from_secs(2), || {
        collector
            .lock()
            .expect("poison")
            .iter()
            .any(|e| matches!(e, AppEvent::ShutdownReady))
    }));
    handle.join();

    let bodies = llm_bodies.lock().expect("poison");
    assert_eq!(bodies.len(), 3, "恰好三次 LLM 请求: {bodies:?}");
    let a_again = user_contents(&bodies[2]);
    assert!(
        a_again.contains(&"甲".to_string()),
        "回到 A 时应带上 A 的上一轮: {a_again:?}"
    );
    assert!(
        !a_again.iter().any(|t| t.contains('乙')),
        "A 的请求体不得带上 B 的正文: {a_again:?}"
    );
    let b = user_contents(&bodies[1]);
    assert!(
        !b.iter().any(|t| t.contains('甲')),
        "B 的第一轮不得看见 A: {b:?}"
    );
}
