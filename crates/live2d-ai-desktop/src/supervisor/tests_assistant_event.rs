//! Wave 3（2026-09-21）：**轮末助手侧正文 → AssistantReplied**。
//!
//! 钉住三件事（记忆摘要含助手侧的唯一入口）：
//! 1. 投递时机在 TurnEnded 之前、且带**本轮会话 id**（记忆据此分桶）；
//! 2. payload 文本是**确定性清洗后**的上屏正文（与送 TTS 同源）；
//! 3. payload 是 JSON（turn / role / text / interrupted），不是裸正文——
//!    老主题 TurnPrompt 的裸正文口径不变。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::support::{Collector, spawn_llm_mock, spawn_tts_mock, wait_for};
use super::*;
use crate::app_event::AppEvent;

type Seen = Arc<Mutex<Vec<(ModEventTopic, String, Option<String>)>>>;

#[test]
fn turn_close_emits_cleaned_assistant_text_to_mod_events() {
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let seen_clone = seen.clone();

    let llm_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"你好呀（挥手）。*歪头* 再见。\"}}]}\n\n",
        "data: [DONE]\n\n"
    )
    .to_owned();
    let llm_base = spawn_llm_mock(llm_bodies, sse);
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
            conversation: live2d_ai_runtime::ConversationConfig::new("人设"),
            capabilities: live2d_ai_core::ModelCapabilities::all(),
            audio: None,
            config_path: None,
            mod_events: Some(Arc::new(move |t, p, s| {
                seen_clone
                    .lock()
                    .expect("poison")
                    .push((t, p.to_string(), s.map(str::to_string)));
            })),
        },
        move |ev| emit_collector.lock().expect("poison").push(ev),
    );

    assert!(handle.say_scoped("在吗", Some("sess-1".to_string())));
    let has = |topic: ModEventTopic| {
        seen.lock()
            .expect("poison")
            .iter()
            .any(|(t, _, _)| *t == topic)
    };
    assert!(
        wait_for(Duration::from_secs(3), || has(
            ModEventTopic::AssistantReplied
        )),
        "轮末必须投递 AssistantReplied"
    );
    assert!(
        wait_for(Duration::from_secs(3), || has(ModEventTopic::TurnEnded)),
        "TurnEnded 仍必须照常发出"
    );

    let events = seen.lock().expect("poison");
    let (_, payload, session) = events
        .iter()
        .find(|(t, _, _)| *t == ModEventTopic::AssistantReplied)
        .expect("AssistantReplied");
    assert_eq!(session.as_deref(), Some("sess-1"), "必须带本轮会话 id");
    let value: serde_json::Value = serde_json::from_str(payload).expect("payload 必须是 JSON");
    assert_eq!(value["role"], "assistant");
    assert_eq!(
        value["text"], "你好呀。再见。",
        "交给 Mod 的必须是**清洗后**的上屏正文：{payload}"
    );
    assert!(
        value["turn"].as_u64().unwrap_or(0) >= 1,
        "payload 必须带 turn id：{payload}"
    );
    assert_eq!(value["interrupted"], false, "正常轮不是 interrupted");
    // 顺序：先给正文，再收口本轮（记忆在 TurnEnded 只做结项）。
    let ar = events
        .iter()
        .position(|(t, _, _)| *t == ModEventTopic::AssistantReplied)
        .unwrap();
    let te = events
        .iter()
        .position(|(t, _, _)| *t == ModEventTopic::TurnEnded)
        .unwrap();
    assert!(ar < te, "AssistantReplied 应在 TurnEnded 之前: {events:?}");

    handle.stop();
    handle.quit();
    let _ = wait_for(Duration::from_secs(2), || {
        collector
            .lock()
            .expect("poison")
            .iter()
            .any(|e| matches!(e, AppEvent::ShutdownReady))
    });
    handle.join();
}
