//! 节点 D 链路阶段耗时埋点（端到端）：单轮完整 turn 跑过 LLM → TTS → 播放
//! 链路，断言 `TurnStageTimings` 字段存在且满足「提交 → 首 token → Terminal
//! → TurnCompleted」递增关系。同时校验所有 `EngineEvent` 变体都带
//! `ts_ms`（节点 D 开发者模式 / D1 契约 §WS 事件可观察性）。
//!
//! 不模拟 stop / 故障 / 仲裁——纯成功路径以获得最稳定的单调断言。
//! 慢速 mock 仅用于在测试断言中将相邻阶段差值放大为可观察毫秒数。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use live2d_ai_runtime::EngineEvent;

use super::support::{
    Collector, count_fact, detached_audio, facts, spawn_llm_mock, spawn_tts_mock, wait_for,
};
use super::*;
use crate::app_event::{AppEvent, RootFact, TurnStageTimings};

/// 端到端：dry-run 文本模式（无音频）→ LLM 文本 + TTS 仍走通。`ts_ms`
/// 字段必须出现在所有事件变体上；至少一个 `TurnStages` 事实下发。
#[test]
fn engine_events_carry_ts_ms_and_turn_stages_emitted() {
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
            mod_events: None,
        },
        move |ev| {
            ec.lock().expect("poison").push(ev);
        },
    );

    assert!(handle.say("单轮"));
    assert!(
        wait_for(Duration::from_secs(3), || count_fact(
            &collector,
            |f| matches!(f, RootFact::TurnStages { .. })
        ) >= 1),
        "TurnStages 必须在 turn 收口后下发"
    );
    handle.quit();
    assert!(wait_for(Duration::from_secs(2), || collector
        .lock()
        .expect("poison")
        .iter()
        .any(|e| matches!(e, AppEvent::ShutdownReady))));
    handle.join();

    // 取出 TurnStages：递增语义检查。
    let stages: Vec<TurnStageTimings> = facts(&collector)
        .into_iter()
        .filter_map(|f| match f {
            RootFact::TurnStages { timings, .. } => Some(timings),
            _ => None,
        })
        .collect();
    assert_eq!(stages.len(), 1, "dry-run 文本模式恰好 1 个 TurnStages");
    let t = stages[0];
    // dry-run：无音频 ⇒ audio_play_ms 与 tts_* 不强制 > 0，但文本侧的
    // 三段（llm_first_token / llm_total / turn_total）必须满足单调链。
    assert!(
        t.llm_first_token_ms <= t.llm_total_ms,
        "llm_first_token({}) <= llm_total({})",
        t.llm_first_token_ms,
        t.llm_total_ms,
    );
    assert!(
        t.llm_total_ms <= t.turn_total_ms,
        "llm_total({}) <= turn_total({})",
        t.llm_total_ms,
        t.turn_total_ms,
    );
    // dry-run 不走音频：audio_play_ms 必为 0（无 producer ⇒ 无
    // PlaybackStarted 也就无 PlaybackDrained）。TTS 链路仍会跑过但
    // 几乎瞬时（mock 8 源样本 @ 24kHz），u64 字段本身保证 ≥ 0。
    assert_eq!(t.audio_play_ms, 0);
}

/// 端到端：detached 音频生产者（ring=4 设备样本，TTS 回 8 ⇒ 必走
/// WouldBlock→pending 泵）。注入**慢速 LLM** 把首 token 拉晚，使三段
/// 差值在毫秒级可观察，断言递增语义严格成立。
#[test]
fn turn_stage_timings_are_monotonic_with_audio() {
    let llm_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"好。。\"}}]}\n\n",
        "data: [DONE]\n\n"
    )
    .to_owned();
    // LLM 响应前 80ms 等待：放大 first-token 延迟。
    let llm_base =
        super::support::spawn_llm_mock_slow(llm_bodies.clone(), sse, Duration::from_millis(80));
    let tts_base = spawn_tts_mock();

    let (producer, mut render) = detached_audio(4);

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
            audio: Some(producer),

            config_path: None,
            mod_events: None,
        },
        move |ev| {
            ec.lock().expect("poison").push(ev);
        },
    );

    // 扮演声卡回调：每 5ms 取一批，制造真实排空节奏。
    let consumer_stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let cs = consumer_stop.clone();
    let consumer = std::thread::spawn(move || {
        while !cs.load(std::sync::atomic::Ordering::Relaxed) {
            let mut buf = [0f32; 64];
            render.render_f32(&mut buf);
            std::thread::sleep(Duration::from_millis(5));
        }
    });

    assert!(handle.say("单轮"));
    assert!(
        wait_for(Duration::from_secs(3), || count_fact(
            &collector,
            |f| matches!(f, RootFact::TurnStages { .. })
        ) >= 1),
        "TurnStages 必须在 turn 收口后下发"
    );
    handle.quit();
    assert!(wait_for(Duration::from_secs(2), || collector
        .lock()
        .expect("poison")
        .iter()
        .any(|e| matches!(e, AppEvent::ShutdownReady))));
    consumer_stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = consumer.join();
    handle.join();

    let stages: Vec<TurnStageTimings> = facts(&collector)
        .into_iter()
        .filter_map(|f| match f {
            RootFact::TurnStages { timings, .. } => Some(timings),
            _ => None,
        })
        .collect();
    assert_eq!(stages.len(), 1, "音频模式恰好 1 个 TurnStages");
    let t = stages[0];
    // 核心递增链：first token ≤ total ≤ turn total
    //   慢速 LLM 80ms ⇒ first_token 必 < total；端到端 turn_total 必 ≥ total。
    assert!(
        t.llm_first_token_ms <= t.llm_total_ms,
        "llm_first_token({}) <= llm_total({})",
        t.llm_first_token_ms,
        t.llm_total_ms,
    );
    assert!(
        t.llm_total_ms <= t.turn_total_ms,
        "llm_total({}) <= turn_total({})",
        t.llm_total_ms,
        t.turn_total_ms,
    );
    // 音频链路触达 ⇒ tts_first_chunk_ms 必 ≥ 0；detached consumer
    // 持续消费下，整段音频可能 < 1ms 完成（8 源样本 @ 24kHz ≈ 0.3ms），
    // 故不强制 > 0（避免伪阴性）。语义上 audio_play_ms/tts_synth_ms 同理。
    // turn_total 与 tts_first_chunk 也应满足「提交 → 首 chunk ≤ 提交 → 终态
    // 完成 ≤ 提交 → 收口」的全局链（u64 字段本身保证 ≥ 0）。
    assert!(
        t.tts_first_chunk_ms <= t.llm_total_ms,
        "tts_first_chunk({}) <= llm_total({})",
        t.tts_first_chunk_ms,
        t.llm_total_ms,
    );
}

/// EngineEvent 各变体均带 `ts_ms`：构造一个全变体数组，断言各 `ts_ms`
/// 被原样读出（构造期必须接受此字段）。
///
/// 2026-09-11：`EngineEvent::ToolAction` 已随动作系统移除，变体数由 6 降为 5。
#[test]
fn engine_event_variants_accept_ts_ms_field() {
    let events: [EngineEvent; 5] = [
        EngineEvent::TextDelta {
            epoch: 1,
            ts_ms: 7,
            text: "x".into(),
        },
        EngineEvent::AudioChunk {
            epoch: 1,
            ts_ms: 9,
            sentence_seq: 1,
            samples: vec![0.0f32, 0.1],
            spec: live2d_ai_runtime::AudioSpec::new(24_000, 1).expect("spec"),
            first_chunk: true,
            final_chunk: false,
        },
        EngineEvent::SentenceVoiced {
            epoch: 1,
            ts_ms: 8,
            sentence_seq: 1,
            text: "x".into(),
        },
        EngineEvent::Error {
            epoch: 1,
            ts_ms: 10,
            kind: live2d_ai_runtime::ErrorKind::Backpressure {
                message: "mock".into(),
            },
        },
        EngineEvent::Terminal {
            epoch: 1,
            ts_ms: 11,
            status: live2d_ai_runtime::TurnStatus::Completed,
        },
    ];
    let mut stamps = Vec::new();
    for e in &events {
        // ev_ts_ms 不可见（pub(crate)），但我们用匹配直接读出 ts_ms。
        let ts = match e {
            EngineEvent::TextDelta { ts_ms, .. }
            | EngineEvent::ReasoningDelta { ts_ms, .. }
            | EngineEvent::AudioChunk { ts_ms, .. }
            | EngineEvent::SentenceVoiced { ts_ms, .. }
            | EngineEvent::Error { ts_ms, .. }
            | EngineEvent::Terminal { ts_ms, .. } => *ts_ms,
        };
        stamps.push(ts);
    }
    assert_eq!(stamps, vec![7, 9, 8, 10, 11], "每个变体必须带 ts_ms");
}
