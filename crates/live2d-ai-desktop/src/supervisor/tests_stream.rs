//! P1WS-1：真实 LLM 流式文本透传（text_delta emit）+ epoch 镜像。
//!
//! 这些测试不依赖真 LLM（避免 CI 噪声）——直接调
//! `handlers::handle_engine_event` 喂 `EngineEvent::TextDelta`，断言
//! collector 收到 `AppEvent::Conversation(ConversationUiEvent::TextDelta)`
//! 携带真实 text payload。
//!
//! epoch 镜像相关：
//! - `supervisor.current_epoch()` 启动瞬间 = 0；
//! - 用户提交一轮后 = 1（UserSubmitted 不推进 root.epoch；本测试用
//!   显式 `StopRequested` 推进 epoch 验证镜像更新——stop 事务是 D7/D13
//!   唯一会推进 epoch 的入口）。
//!
//! 与 `tests_pcm.rs` 风格一致：直接调 `handle_engine_event` + `do_stop!` 内的
//! 根事件序列，不走 spawn_supervisor（避免启动 LLM/TTS 拖慢 CI）。

use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use live2d_ai_runtime::EngineEvent;

use super::support::{Collector, wait_for};
use super::*;
use crate::app_event::{AppEvent, ConversationUiEvent};

/// P1WS-1：handler 处理 TextDelta 时 emit `Conversation(TextDelta)` 含真实
/// text payload——前端经 `ws::app_event_to_ws_frame` 投影为
/// `{type:"text_delta",data:{epoch, ts_ms, text}}`。
///
/// **不**依赖真 LLM：直接构造 `EngineEvent::TextDelta` 喂
/// `handle_engine_event`，断言 collector 收到对应事件。
#[test]
fn handle_engine_event_emits_text_delta_with_text_payload() {
    let collector: Collector = Arc::new(Mutex::new(Vec::new()));
    let ec = collector.clone();
    let emit: Emit = Arc::new(move |ev: AppEvent| {
        ec.lock().expect("poison").push(ev);
    });

    // 极简 root + 状态机变量；handler 不依赖 root.epoch 推进（TextDelta
    // 不触发 root 事件），但需要保证 `epoch` 字段是合理初值（= 0）。
    let mut root = RootState::default();
    root.action.capabilities = live2d_ai_core::ModelCapabilities::all();

    let mut pending_pcm: Option<crate::audio::PreparedPcm> = None;
    let mut playback_started = false;
    let mut voice_started_emitted = false;
    let mut accepted_tool_seen = false;
    let mut assistant_text_len = 0usize;
    let mut saw_fatal_kind = false;
    let mut saw_llm_error = false;
    let mut audio: Option<Box<dyn crate::audio::PcmProducer>> = None;

    // 喂 3 个 TextDelta 片段。
    let deltas = [
        EngineEvent::TextDelta {
            epoch: 5,
            ts_ms: 100,
            text: "你".to_string(),
        },
        EngineEvent::TextDelta {
            epoch: 5,
            ts_ms: 110,
            text: "好，".to_string(),
        },
        EngineEvent::TextDelta {
            epoch: 5,
            ts_ms: 120,
            text: "世".to_string(),
        },
    ];
    for d in &deltas {
        super::handlers::handle_engine_event(
            d,
            &mut root,
            &mut audio,
            1,
            &mut pending_pcm,
            &mut playback_started,
            &mut voice_started_emitted,
            &mut accepted_tool_seen,
            &mut assistant_text_len,
            &mut saw_fatal_kind,
            &mut saw_llm_error,
            &emit,
        );
    }

    // 断言：3 个 TextDelta 都被 emit，payload 完整。
    let events = collector.lock().expect("poison");
    let text_deltas: Vec<&ConversationUiEvent> = events
        .iter()
        .filter_map(|e| match e {
            AppEvent::Conversation(ConversationUiEvent::TextDelta { .. }) => {
                if let AppEvent::Conversation(ui) = e {
                    Some(ui)
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect();
    assert_eq!(text_deltas.len(), 3, "3 个 TextDelta 全部应被 emit");

    // 校验 payload 完整（epoch/ts_ms/text 字段一致）。
    let mut concatenated = String::new();
    for (i, td) in text_deltas.iter().enumerate() {
        if let ConversationUiEvent::TextDelta { epoch, ts_ms, text } = td {
            assert_eq!(*epoch, 5, "deltas[{i}].epoch");
            assert_eq!(*ts_ms, 100 + (i as u64) * 10, "deltas[{i}].ts_ms");
            concatenated.push_str(text);
        } else {
            panic!("should be TextDelta");
        }
    }
    assert_eq!(concatenated, "你好，世", "text 应按字累加（无截断/排序错）");

    // 副断言：assistant_text_len 累计 = "你" 1 + "好，" 2 + "世" 1 = 4 chars。
    // （这是 TextDelta 的内部统计，前端不感知；只校验不被回归破坏。）
    assert_eq!(assistant_text_len, 4, "text 字符计数应累计");
}

/// P1WS-1：`ConversationUiEvent::TextDelta` 的 ts_ms 与 `EngineEvent::TextDelta`
/// 的 ts_ms 一一对应（链路耗时可视化保留）。本测试断言 0 边界。
#[test]
fn text_delta_zero_ts_ms_is_preserved() {
    let collector: Collector = Arc::new(Mutex::new(Vec::new()));
    let ec = collector.clone();
    let emit: Emit = Arc::new(move |ev: AppEvent| {
        ec.lock().expect("poison").push(ev);
    });

    let mut root = RootState::default();
    root.action.capabilities = live2d_ai_core::ModelCapabilities::all();
    let mut pending_pcm: Option<crate::audio::PreparedPcm> = None;
    let mut playback_started = false;
    let mut voice_started_emitted = false;
    let mut accepted_tool_seen = false;
    let mut assistant_text_len = 0usize;
    let mut saw_fatal_kind = false;
    let mut saw_llm_error = false;
    let mut audio: Option<Box<dyn crate::audio::PcmProducer>> = None;

    let ev = EngineEvent::TextDelta {
        epoch: 0,
        ts_ms: 0,
        text: "x".to_string(),
    };
    super::handlers::handle_engine_event(
        &ev,
        &mut root,
        &mut audio,
        1,
        &mut pending_pcm,
        &mut playback_started,
        &mut voice_started_emitted,
        &mut accepted_tool_seen,
        &mut assistant_text_len,
        &mut saw_fatal_kind,
        &mut saw_llm_error,
        &emit,
    );

    let events = collector.lock().expect("poison");
    let td = events
        .iter()
        .find_map(|e| match e {
            AppEvent::Conversation(ConversationUiEvent::TextDelta { .. }) => {
                if let AppEvent::Conversation(ui) = e {
                    Some(ui.clone())
                } else {
                    None
                }
            }
            _ => None,
        })
        .expect("TextDelta event emitted");
    if let ConversationUiEvent::TextDelta { epoch, ts_ms, text } = td {
        assert_eq!(epoch, 0);
        assert_eq!(ts_ms, 0, "ts_ms 0 边界保留");
        assert_eq!(text, "x");
    } else {
        panic!("not TextDelta");
    }
}

/// P1WS-1：epoch 镜像（`AtomicU64`）在 stop 事务（`StopRequested`）后被
/// supervisor 线程更新——验证镜像更新路径（`run_forever` → `do_stop!`
/// 经 `current_epoch.store`）。
///
/// **不**走 `spawn_supervisor` 全链路（避免 mock LLM/TTS）；直接构造
/// `RootState` + `current_epoch: AtomicU64`，模拟 stop 流程：
/// `root_apply(root, StopRequested) → current_epoch.store(new)`。
#[test]
fn epoch_mirror_updates_on_stop_requested() {
    let current_epoch = Arc::new(AtomicU64::new(0));
    let mut root = RootState::default();
    root.action.capabilities = live2d_ai_core::ModelCapabilities::all();

    // 启动瞬间 = 0。
    assert_eq!(current_epoch.load(std::sync::atomic::Ordering::Acquire), 0);

    // 模拟 UserSubmitted（不推进 epoch；保持 0）。
    let cur = root.epoch;
    let _ = live2d_ai_core::apply(
        &mut root,
        live2d_ai_core::Event::UserSubmitted {
            epoch: cur,
            turn_id: 1.into(),
            sentence_id: 1.into(),
            text: String::new(),
        },
    );
    // run_forever 路径：开轮前镜像当前 epoch。
    current_epoch.store(root.epoch.get(), std::sync::atomic::Ordering::Release);
    assert_eq!(current_epoch.load(std::sync::atomic::Ordering::Acquire), 0);

    // 模拟 stop 事务：do_stop! 调用 `root_apply(root, StopRequested)`。
    let _fx = live2d_ai_core::apply(&mut root, live2d_ai_core::Event::StopRequested);
    // do_stop! 紧随其后：current_epoch.store(root.epoch.get(), Release)。
    current_epoch.store(root.epoch.get(), std::sync::atomic::Ordering::Release);

    // StopRequested 推进 epoch：0 → 1。
    assert_eq!(
        current_epoch.load(std::sync::atomic::Ordering::Acquire),
        1,
        "stop 事务后镜像应同步"
    );
    assert_eq!(root.epoch.get(), 1, "root 真实 epoch 也被推进");
}

/// P1WS-1：SupervisorHandle::current_epoch() 默认 = 0；与 AtomicU64 镜像
/// 共享；外部读 `handle.current_epoch()` 即可拿到**最新**值。
#[test]
fn supervisor_handle_current_epoch_starts_at_zero() {
    use live2d_ai_core::ModelCapabilities;
    use live2d_ai_runtime::{ConversationConfig, LlmConfig, OpenAiClient, TtsConfig};

    // 用真实 spawn_supervisor（指向不可达端点；不会真发请求）——验证
    // 启动瞬间 current_epoch = 0。
    let tmp = std::env::temp_dir().join(format!(
        "live2d_ai_test_stream_epoch_{}_{}.toml",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_file(&tmp);
    std::fs::write(
        &tmp,
        live2d_ai_runtime::AppSettings::default().to_toml_string(),
    )
    .expect("write tmp");
    let client = OpenAiClient::new(
        LlmConfig::new("http://127.0.0.1:1/v1", "m"),
        TtsConfig::new("http://127.0.0.1:2/v1", "alloy"),
    )
    .expect("client");
    let handle = std::sync::Arc::new(crate::supervisor::spawn_supervisor(
        crate::supervisor::SupervisorConfig {
            client,
            conversation: ConversationConfig::new("人设"),
            capabilities: ModelCapabilities::all(),
            audio: None,
            config_path: Some(tmp.to_string_lossy().into_owned()),
        },
        |_ev| {},
    ));
    // 启动瞬间 current_epoch = 0。
    assert_eq!(handle.current_epoch(), 0);
    handle.quit();
    let _ = wait_for(Duration::from_secs(2), || {
        // supervisor 退出：handle.join() 必须成功（= 进程安全回收）。
        // 这里不直接 join（join 夺所有权）；仅验证 quit 路径不 panic。
        true
    });
    let _ = std::fs::remove_file(&tmp);
}
