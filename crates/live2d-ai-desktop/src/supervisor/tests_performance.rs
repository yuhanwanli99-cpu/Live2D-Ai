//! **表演层端到端冒烟（mock 表演层，2026-09-22；2026-10-08 起显式 opt-in）**：
//! 真实 `[performance]` 配置段（`enabled = true`）→ `build_performance_runtime`
//! 装配 → 引擎接线 → 送 TTS 的文本 == 表演层的 speak。
//!
//! **动作恒开（2026-10-17 rc4 返工裁决）**：表演层打开时也不再是 0 cue——
//! 阶段 1.5 对**每一段原文**用核心本地规则表（`performance::rules`，纯函数、
//! **不新增 LLM 调用**）产 cue，非空才在该段音频前发 ActionCue；表演层自己的
//! `resolution.cues` 仍**不投递**，也不为 cue 造无声锚句（单 cue 写者）。
//!
//! 缺省（`enabled = false`）时主链走**常驻二路按句清洗**，表演层整段 JSON 不参与
//! ——那条路的回归在 `live2d-ai-runtime/tests/conversation_engine_cleaning.rs`。
//!
//! 为什么单列：runtime 的内联测试证明了引擎语义，desktop 的装配单测证明了配置解析，
//! 但「写一份带 [performance] 的 live2d-ai.toml → supervisor 起来 → 真的发一次
//! /chat/completions → speak 进 TTS」这条链只有端到端跑得出来（无模型依赖，
//! 所以不需要 ignite 的 assets/models）。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use live2d_ai_core::ModelCapabilities;
use live2d_ai_mod_director::motion_bias::MotionBias;
use live2d_ai_runtime::{AppSettings, ConversationConfig, LlmConfig, OpenAiClient, TtsConfig};

use crate::app_event::{AppEvent, ConversationUiEvent, RootFact};
use crate::supervisor::{SupervisorConfig, spawn_supervisor};

use super::support::{
    Collector, spawn_llm_mock, spawn_llm_mock_status, spawn_performance_mock,
    spawn_tts_mock_recording, wait_for,
};

/// **二路清洗的装配回归**（2026-10-08）：`LlmConfig::new` 的便捷构造
/// （`clean_tts = false`）不装配；生产路径（`settings::resolve` 置 true）装配，
/// 且模型名与端点就是**一路那一份**（不再有第二个用户旋钮）。
#[test]
fn sentence_cleaner_is_wired_only_when_clean_tts_is_on() {
    let tts = || TtsConfig::new("http://127.0.0.1:9/v1", "alloy");
    let off = OpenAiClient::new(
        LlmConfig::new("http://127.0.0.1:9/v1", "deepseek-flash"),
        tts(),
    )
    .expect("client");
    // rc4：二路常驻、端点恒为主 LLM（导演 Mod 自己的二路端点已删除），
    // 这里测的就是 clean_tts 这一个开关；motion_bias 只影响提示词子句。
    let bias = MotionBias::default();
    assert!(
        super::reload::build_sentence_cleaner(&off, &bias).is_none(),
        "clean_tts=false（便捷构造）必须不跑二路，引擎走规则表"
    );

    let mut llm = LlmConfig::new("http://127.0.0.1:9/v1", "deepseek-flash");
    llm.clean_tts = true;
    let on = OpenAiClient::new(llm, tts()).expect("client");
    let cleaner =
        super::reload::build_sentence_cleaner(&on, &bias).expect("clean_tts=true 必须装配二路清洗");
    assert!(cleaner.enabled());
    assert_eq!(
        cleaner.model(),
        "deepseek-flash",
        "二路模型 = 一路模型（同一份，没有第二个下拉框）"
    );
}

fn canned_sse() -> String {
    "data: {\"choices\":[{\"delta\":{\"content\":\"（挥手）你好呀。\"}}]}\n\n\
     data: {\"choices\":[{\"finish_reason\":\"stop\"}],\"usage\":{\"total_tokens\":1}}\n\n\
     data: [DONE]\n\n"
        .to_string()
}

/// 两句正文（产品路径「每句一条 action_cue」用）。
///
/// 两句都带问号：规则表对**这一句原文**的判据是「？/ ?」，问号会补齐空槽
/// （手势 tilt_left + 表情 thinking），因此两句都必然产非空 cue。不用
/// 「再见。」—— farewell 缺省映射 `none`，规则表刻意不出动作（不做 > 做错），
/// 会让「每句一条」这条断言依赖映射表内容而不是单 cue 写者这条红线。
fn two_sentence_sse() -> String {
    "data: {\"choices\":[{\"delta\":{\"content\":\"（挥手）你好呀？今天开心吗？\"}}]}\n\n\
     data: {\"choices\":[{\"finish_reason\":\"stop\"}],\"usage\":{\"total_tokens\":1}}\n\n\
     data: [DONE]\n\n"
        .to_string()
}

/// 写一份测试 `live2d-ai.toml`（`[performance]` 由调用方决定）。
///
/// 2026-10-17（rc4）：整段表演层只读 `[performance]` 自己的端点，不再读
/// `mods.json` 的导演 staging 端点（已删除），所以本测试**不再写 mods.json**。
fn write_config(
    path: &std::path::Path,
    llm_base: &str,
    tts_base: &str,
    performance: live2d_ai_runtime::settings::PerformanceSettings,
) {
    let settings = AppSettings {
        llm: live2d_ai_runtime::settings::LlmSettings {
            base_url: llm_base.to_string(),
            model: "test-model".to_string(),
            ..Default::default()
        },
        tts: live2d_ai_runtime::settings::TtsSettings {
            mode: live2d_ai_runtime::settings::TtsMode::Cloud,
            base_url: tts_base.to_string(),
            voice: "alloy".to_string(),
            ..Default::default()
        },
        persona: live2d_ai_runtime::settings::PersonaSettings {
            system_prompt: "你是桌宠，只写剧情。".to_string(),
            max_history_pairs: 0,
        },
        performance,
        ..AppSettings::default()
    };
    std::fs::write(path, settings.to_toml_string()).expect("写测试配置");
}

/// `[performance]` 显式打开 + 自有端点（**不再**借导演 staging）。
fn write_performance_config(
    path: &std::path::Path,
    llm_base: &str,
    tts_base: &str,
    performance_base: &str,
) {
    write_config(
        path,
        llm_base,
        tts_base,
        live2d_ai_runtime::settings::PerformanceSettings {
            // 2026-10-08：整段表演层是**显式 opt-in**（`[performance].enabled`）。
            enabled: true,
            base_url: performance_base.to_string(),
            model: "perf-model".to_string(),
            ..Default::default()
        },
    );
}

/// 产品路径配置：`[performance]` **缺省关**。
fn write_plain_config(path: &std::path::Path, llm_base: &str, tts_base: &str) {
    write_config(
        path,
        llm_base,
        tts_base,
        live2d_ai_runtime::settings::PerformanceSettings::default(),
    );
}

/// 整段表演层成功路径：speak 进 TTS；json_schema 真发出去；**动作恒开**——
/// 每段原文过核心本地规则表：命中段（你好 → greeting/nod）恰 1 条 ActionCue，
/// 中性段（再见 → farewell 缺省 none）不发；表演层 `resolution.cues` 不投递
/// （无双发）；表演层与 LLM 调用次数都不增。
#[test]
fn performance_layer_end_to_end_drives_speak_with_local_rule_cues() {
    // 每个用例独占一个目录（并发跑，不能共用 temp_dir 根下的同名配置）。
    let dir = std::env::temp_dir().join(format!("live2d_ai_perf_e2e_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let tmp = dir.join("live2d-ai.toml");
    let llm_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let llm_bodies_for_assert = Arc::clone(&llm_bodies);
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

    // 同步到**本轮真正收尾**的确定性信号，然后才 quit/join 与断言。
    //
    // RootFact::TurnCompleted{outcome_completed:true} 由 supervisor Stage C
    // 发出，而 Stage C 只在 gen_fut 返回之后执行；engine.run_turn 返回前
    // **无条件 await TTS worker 排空**（engine.rs 阶段 2 worker.await，阶段 3
    // 才产出 TurnReport）。即：TurnCompleted 可见 ⇒ 两句 speak 的合成请求
    // 都已完成并被 mock 记账，整段表演层也已调用过。
    let turn_closed = wait_for(Duration::from_secs(15), || {
        collector.lock().expect("poison").iter().any(|e| {
            matches!(
                e,
                AppEvent::RootAudit(RootFact::TurnCompleted {
                    outcome_completed: true,
                    ..
                })
            )
        })
    });
    assert!(
        turn_closed,
        "本轮必须以 Completed 收口（TurnCompleted）；收尾之后 TTS 记账才完整"
    );
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

    // ③ rc4 返工裁决（2026-10-17）：表演层打开时动作**恒开**，但 cue 只由
    //    **每段原文 + 核心本地规则表**产出，表演层 JSON 里的 cues 不投递。
    //    speak 切出「你好呀。」「再见。」两段：
    //    - 段 1「你好呀。」→ greeting → nod（规则命中，恰 1 条 ActionCue）；
    //    - 段 2「再见。」→ farewell 缺省 none → **不发**（中性段不动）。
    //    `resolution.cues` 里也有一条 nod；若被双发，这里会看到 2 条。
    let cue_events: Vec<Vec<(u64, String)>> = collector
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
        .collect();
    assert_eq!(
        cue_events,
        vec![vec![(1, "nod".to_string())]],
        "命中段恰 1 条、中性段不发、表演层 cues 不得双发；实际 {cue_events:?}"
    );

    // ④ 规则表是纯本地函数：不得新增 LLM 调用（主模型仍恰 1 次）。
    assert_eq!(
        llm_bodies_for_assert.lock().expect("poison").len(),
        1,
        "本地规则表出 cue 不得新增 LLM 调用"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// **rc4 产品路径（`[performance]` 缺省关）**：二路按句清洗常驻，**每句一条**
/// ActionCue（规则表）；cue 的句号与该句一致。这条与
/// `performance_layer_end_to_end_drives_speak_with_local_rule_cues` 一起守住
/// 「动作恒开 + 每段/每句只有一条 ActionCue（规则表）」这条单 cue 写者红线。
#[test]
fn default_off_emits_one_action_cue_per_sentence() {
    let dir =
        std::env::temp_dir().join(format!("live2d_ai_perf_default_off_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let tmp = dir.join("live2d-ai.toml");
    let llm_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let llm_base = spawn_llm_mock(llm_bodies, two_sentence_sse());
    let tts_inputs: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let tts_base = spawn_tts_mock_recording(tts_inputs.clone());
    write_plain_config(&tmp, &llm_base, &tts_base);

    // `LlmConfig::new` ⇒ clean_tts=false ⇒ 二路 None ⇒ 引擎走
    // clean_for_tts + 规则表（每句非空 cues 发一条 ActionCue）。
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

    let turn_closed = wait_for(Duration::from_secs(15), || {
        collector.lock().expect("poison").iter().any(|e| {
            matches!(
                e,
                AppEvent::RootAudit(RootFact::TurnCompleted {
                    outcome_completed: true,
                    ..
                })
            )
        })
    });
    assert!(turn_closed, "本轮必须以 Completed 收口");
    handle.quit();
    handle.join();

    let cue_events: Vec<Vec<(u64, String)>> = collector
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
        .collect();
    assert_eq!(
        cue_events.len(),
        2,
        "缺省关 → 每句恰一条 action_cue（单 cue 写者），实际 {cue_events:?}"
    );
    for cues in &cue_events {
        assert!(!cues.is_empty(), "规则表给每句非空 cues: {cues:?}");
    }
    let seqs: Vec<u64> = cue_events.iter().map(|cues| cues[0].0).collect();
    assert_eq!(seqs, vec![1, 2], "cue 句号必须与该句一致");
    assert_eq!(
        tts_inputs.lock().expect("poison").len(),
        2,
        "产品路径一句一单元送 TTS"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// **P0-1b（2026-09-23）**：整段表演层（T8；rc4 起只读 `[performance]`）开着、但
/// **主模型失败**时，本轮必须直接以 failed 收口——既不调表演层（llm_failed 短路），
/// 也不干等一份永远来不了的 JSON。
///
/// 反向证据链：error 帧带码 + TurnCompleted{failed} + 表演层 mock 一个请求都没收到
/// + 没有 ActionCue。
#[test]
fn performance_layer_is_skipped_when_the_main_model_fails() {
    let dir = std::env::temp_dir().join(format!("live2d_ai_perf_llmfail_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let tmp = dir.join("live2d-ai.toml");
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

    let _ = std::fs::remove_dir_all(&dir);
}
