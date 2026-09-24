//! **表演层端到端冒烟（mock 表演层，2026-09-22）**：
//! 真实 \`[performance]\` 配置段 → \`build_performance_runtime\` 装配 → 引擎接线 →
//! \`AppEvent::Conversation(ActionCue)\` + 送 TTS 的文本 == 表演层的 speak。
//!
//! 为什么单列：runtime 的内联测试证明了引擎语义，desktop 的装配单测证明了配置解析，
//! 但「写一份带 [performance] 的 live2d-ai.toml → supervisor 起来 → 真的发一次
//! /chat/completions → speak 进 TTS」这条链只有端到端跑得出来（无模型依赖，
//! 所以不需要 ignite 的 assets/models）。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use live2d_ai_core::ModelCapabilities;
use live2d_ai_runtime::{AppSettings, ConversationConfig, LlmConfig, OpenAiClient, TtsConfig};

use crate::app_event::{AppEvent, ConversationUiEvent, RootFact};
use crate::supervisor::{SupervisorConfig, spawn_supervisor};

use super::support::{
    Collector, spawn_llm_mock, spawn_llm_mock_status, spawn_performance_mock,
    spawn_tts_mock_recording, wait_for,
};

fn canned_sse() -> String {
    "data: {\"choices\":[{\"delta\":{\"content\":\"（挥手）你好呀。\"}}]}\n\n\
     data: {\"choices\":[{\"finish_reason\":\"stop\"}],\"usage\":{\"total_tokens\":1}}\n\n\
     data: [DONE]\n\n"
        .to_string()
}

fn write_performance_config(
    path: &std::path::Path,
    llm_base: &str,
    tts_base: &str,
    performance_base: &str,
) {
    let settings = AppSettings {
        llm: live2d_ai_runtime::settings::LlmSettings {
            base_url: llm_base.to_string(),
            model: "test-model".to_string(),
            ..Default::default()
        },
        tts: live2d_ai_runtime::settings::TtsSettings {
            base_url: tts_base.to_string(),
            voice: "alloy".to_string(),
            ..Default::default()
        },
        persona: live2d_ai_runtime::settings::PersonaSettings {
            system_prompt: "你是桌宠，只写剧情。".to_string(),
            max_history_pairs: 0,
        },
        performance: live2d_ai_runtime::settings::PerformanceSettings {
            enabled: true,
            base_url: performance_base.to_string(),
            model: "perf-model".to_string(),
            ..Default::default()
        },
        ..AppSettings::default()
    };
    std::fs::write(path, settings.to_toml_string()).expect("写测试配置");
}

/// 表演层成功路径：speak 进 TTS；cues 进 ActionCue；json_schema 真发出去。
#[test]
fn performance_layer_end_to_end_drives_speak_and_cues() {
    let tmp = std::env::temp_dir().join(format!("live2d_ai_perf_e2e_{}.toml", std::process::id()));
    let llm_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let llm_base = spawn_llm_mock(llm_bodies, canned_sse());
    let tts_inputs: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let tts_base = spawn_tts_mock_recording(tts_inputs.clone());
    let perf_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let perf_base = spawn_performance_mock(
        perf_bodies.clone(),
        r#"{"speak":"你好呀。再见。","cues":[{"sentence_seq":1,"preset_id":"nod","intensity":2,"ttl_ms":1500}]}"#
            .to_string(),
    );
    write_performance_config(&tmp, &llm_base, &tts_base, &perf_base);

    let client = OpenAiClient::new(
        LlmConfig::new(&llm_base, "test-model"),
        TtsConfig::new(&tts_base, "alloy"),
    )
    .expect("client");
    let collector: Collector = Arc::new(Mutex::new(Vec::new()));
    let ec = collector.clone();
    let handle = spawn_supervisor(
        SupervisorConfig {
            client,
            conversation: ConversationConfig::new("你是桌宠，只写剧情。"),
            capabilities: ModelCapabilities::all(),
            audio: None,
            config_path: Some(tmp.to_string_lossy().into_owned()),
            mod_events: None,
        },
        move |ev| ec.lock().expect("poison").push(ev),
    );
    assert!(handle.say("你好"));

    let ok = wait_for(Duration::from_secs(15), || {
        collector.lock().expect("poison").iter().any(|e| {
            matches!(
                e,
                AppEvent::Conversation(ConversationUiEvent::ActionCue { .. })
            )
        })
    });
    assert!(ok, "必须收到表演层的 ActionCue 事件");
    handle.quit();
    handle.join();

    // ① 表演层真的被调了一次，且带 structured output（json_schema strict）。
    let bodies = perf_bodies.lock().expect("poison").clone();
    assert_eq!(bodies.len(), 1, "每轮恰好一次表演层调用");
    assert_eq!(bodies[0]["response_format"]["type"], "json_schema");
    assert_eq!(bodies[0]["response_format"]["json_schema"]["strict"], true);
    assert_eq!(bodies[0]["stream"], false, "必须非流式");

    // ② 送 TTS 的是 speak 切出的句子（不是主模型原文）。
    let inputs = tts_inputs.lock().expect("poison").clone();
    assert_eq!(inputs, ["你好呀。", "再见。"]);

    // ③ cue 经 AppEvent 出去（preset / 句号 / 优先级）。
    let cues: Vec<(u64, String)> = collector
        .lock()
        .expect("poison")
        .iter()
        .filter_map(|e| match e {
            AppEvent::Conversation(ConversationUiEvent::ActionCue { cues, .. }) => Some(
                cues.iter()
                    .map(|c| (c.sentence_seq, c.preset_id.clone()))
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(cues, [(1, "nod".to_string())]);

    let _ = std::fs::remove_file(&tmp);
}

/// **P0-1b（2026-09-23）**：表演层开着、但**主模型失败**时，本轮必须直接以
/// failed 收口——既不调表演层（llm_failed 短路），也不干等一份永远来不了的 JSON。
///
/// 反向证据链：error 帧带码 + TurnCompleted{failed} + 表演层 mock 一个请求都没收到
/// + 没有 ActionCue。
#[test]
fn performance_layer_is_skipped_when_the_main_model_fails() {
    let tmp = std::env::temp_dir().join(format!(
        "live2d_ai_perf_llmfail_{}.toml",
        std::process::id()
    ));
    let llm_base = spawn_llm_mock_status(401, r#"{"error":{"message":"Authentication Fails"}}"#);
    let tts_inputs: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let tts_base = spawn_tts_mock_recording(tts_inputs.clone());
    let perf_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let perf_base = spawn_performance_mock(
        perf_bodies.clone(),
        r#"{"speak":"不该被用到。","cues":[]}"#.to_string(),
    );
    write_performance_config(&tmp, &llm_base, &tts_base, &perf_base);

    let client = OpenAiClient::new(
        LlmConfig::new(&llm_base, "test-model"),
        TtsConfig::new(&tts_base, "alloy"),
    )
    .expect("client");
    let collector: Collector = Arc::new(Mutex::new(Vec::new()));
    let ec = collector.clone();
    let handle = spawn_supervisor(
        SupervisorConfig {
            client,
            conversation: ConversationConfig::new("你是桌宠，只写剧情。"),
            capabilities: ModelCapabilities::all(),
            audio: None,
            config_path: Some(tmp.to_string_lossy().into_owned()),
            mod_events: None,
        },
        move |ev| ec.lock().expect("poison").push(ev),
    );
    assert!(handle.say("你好"));

    let settled = wait_for(Duration::from_secs(10), || {
        let c = collector.lock().expect("poison");
        c.iter()
            .any(|e| matches!(e, AppEvent::Error(err) if err.code == "llm_upstream_401"))
            && c.iter().any(|e| {
                matches!(
                    e,
                    AppEvent::RootAudit(RootFact::TurnCompleted {
                        outcome_completed: false,
                        ..
                    })
                )
            })
    });
    assert!(
        settled,
        "主模型 401 必须被看见并以 failed 收口，不得干等表演层"
    );

    handle.quit();
    handle.join();

    assert!(
        perf_bodies.lock().expect("poison").is_empty(),
        "主模型失败 → 表演层一个请求都不该收到"
    );
    assert!(
        tts_inputs.lock().expect("poison").is_empty(),
        "失败轮没有 speak，不得给 TTS 发空活"
    );
    assert!(
        !collector.lock().expect("poison").iter().any(|e| matches!(
            e,
            AppEvent::Conversation(ConversationUiEvent::ActionCue { .. })
        )),
        "失败轮不得发出任何 action_cue"
    );

    let _ = std::fs::remove_file(&tmp);
}
