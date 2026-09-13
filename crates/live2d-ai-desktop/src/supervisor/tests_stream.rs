//! 文本上屏时机（**一句一单元，先完整合成再显示**）+ epoch 镜像。
//!
//! **2026-09-10 行为变更**：`EngineEvent::TextDelta`（LLM 原始增量）**不再**上屏，
//! 它只做控制台回显与字符计数；真正推给 UI/WS 的文字由
//! `EngineEvent::SentenceVoiced`（该句**语音已完整合成**）承载。
//! 因此本文件的断言从「TextDelta 触发 emit」改为「TextDelta 不 emit、
//! SentenceVoiced 才 emit」。
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

/// **回归（2026-09-10）**：原始 LLM 增量**不**上屏，只有「该句已合成」才上屏。
///
/// 这条锁的是「一句一单元：先完整合成、再显示」契约——若有人把 emit 挪回
/// `EngineEvent::TextDelta` 分支，本测试立刻变红（文字会重新跑在声音前面）。
///
/// 前端经 `ws::app_event_to_ws_frame` 把 `Conversation(TextDelta)` 投影为
/// `{type:"text_delta",data:{epoch, ts_ms, text}}`。
#[test]
fn sentence_voiced_emits_text_delta_but_raw_deltas_do_not() {
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
    let mut saw_fatal_kind = false;
    let mut saw_llm_error = false;
    let mut audio: Option<Box<dyn crate::audio::PcmProducer>> = None;

    // 先喂 3 个原始 LLM 增量（属于同一句，尚未合成）。
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
    let mut feed = |ev: &EngineEvent| {
        super::handlers::handle_engine_event(
            ev,
            &mut root,
            &mut audio,
            1,
            &mut pending_pcm,
            &mut playback_started,
            &mut voice_started_emitted,
            &mut saw_fatal_kind,
            &mut saw_llm_error,
            &emit,
        );
    };
    for d in &deltas {
        feed(d);
    }

    // 关键断言 1：原始增量**不得**上屏。
    assert!(
        collector.lock().expect("poison").is_empty(),
        "LLM 原始增量不得上屏（文字必须等该句合成完毕）"
    );

    // 再喂「本句已合成」事件（整句文本）。
    feed(&EngineEvent::SentenceVoiced {
        epoch: 5,
        ts_ms: 900,
        sentence_seq: 1,
        text: "你好，世".to_string(),
    });

    let events = collector.lock().expect("poison");
    let text_deltas: Vec<&ConversationUiEvent> = events
        .iter()
        .filter_map(|e| match e {
            AppEvent::Conversation(ui @ ConversationUiEvent::TextDelta { .. }) => Some(ui),
            _ => None,
        })
        .collect();
    assert_eq!(
        text_deltas.len(),
        1,
        "每句恰好一次上屏（一句一单元），实际 {}",
        text_deltas.len()
    );
    if let ConversationUiEvent::TextDelta { epoch, ts_ms, text } = text_deltas[0] {
        assert_eq!(*epoch, 5);
        // ts_ms 必须是**合成完成**时刻，不是 LLM 产生时刻。
        assert_eq!(*ts_ms, 900, "上屏时刻应取 SentenceVoiced 的 ts_ms");
        assert_eq!(text, "你好，世", "整句文本逐字无损");
    } else {
        panic!("should be TextDelta");
    }
}

/// 上屏事件的 ts_ms 与 `EngineEvent::SentenceVoiced` 一一对应
/// （链路耗时可视化保留）。本测试断言 0 边界。
#[test]
fn sentence_voiced_zero_ts_ms_is_preserved() {
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
    let mut saw_fatal_kind = false;
    let mut saw_llm_error = false;
    let mut audio: Option<Box<dyn crate::audio::PcmProducer>> = None;

    let ev = EngineEvent::SentenceVoiced {
        epoch: 0,
        ts_ms: 0,
        sentence_seq: 1,
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
            mod_events: None,
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
/// **rc.3 N0 回归（2026-09-13）**：失败轮的正文兜底必须投影到
/// `ConversationUiEvent::TextFallback`，供 WS 转成 `text_fallback` 帧。
///
/// 与上一条测试互补：上一条锁「健康路径靠 SentenceVoiced 上屏」，本条锁
/// 「失败路径有兜底出口」，且兜底**不走** `TextDelta`——否则前端会把兜底
/// 当增量追加、正文出现两遍。
#[test]
fn text_fallback_emits_its_own_ui_event() {
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
    let mut saw_fatal_kind = false;
    let mut saw_llm_error = false;
    let mut audio: Option<Box<dyn crate::audio::PcmProducer>> = None;

    super::handlers::handle_engine_event(
        &EngineEvent::TextFallback {
            epoch: 5,
            ts_ms: 777,
            text: "第一句。第二".to_string(),
        },
        &mut root,
        &mut audio,
        1,
        &mut pending_pcm,
        &mut playback_started,
        &mut voice_started_emitted,
        &mut saw_fatal_kind,
        &mut saw_llm_error,
        &emit,
    );

    let events = collector.lock().expect("poison");
    let fallbacks: Vec<&ConversationUiEvent> = events
        .iter()
        .filter_map(|e| match e {
            AppEvent::Conversation(ui @ ConversationUiEvent::TextFallback { .. }) => Some(ui),
            _ => None,
        })
        .collect();
    assert_eq!(fallbacks.len(), 1, "兜底必须恰好投影一次");
    if let ConversationUiEvent::TextFallback { epoch, ts_ms, text } = fallbacks[0] {
        assert_eq!(*epoch, 5);
        assert_eq!(*ts_ms, 777);
        assert_eq!(text, "第一句。第二", "整轮正文原样透传");
    } else {
        panic!("should be TextFallback");
    }
    assert!(
        !events.iter().any(|e| matches!(
            e,
            AppEvent::Conversation(ConversationUiEvent::TextDelta { .. })
        )),
        "兜底不得走 TextDelta 分支（那是健康路径的增量上屏）"
    );
}
