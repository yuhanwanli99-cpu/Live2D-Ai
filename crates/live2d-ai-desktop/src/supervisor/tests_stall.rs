//! 发布前专项测试（节点 B 遗留清单）：TTS 在飞 stop、pending PCM 泵期 stop、
//! 真实 stall 5s 致命收尾，外加从 `tests_fault.rs` 迁来的
//! `stop_during_drain_produces_no_commit_and_clean_next_round`（语义上
//! 也是「stop during drain-watch」，与本文件的「stop during X」主题一致）。
//! 全部走 `spawn_supervisor` 真实链路，复用 `support.rs` 中既有 helper。
//!
//! 文件豁免依据：用户硬约束 ≤500；本文件 500-1000 区间允许但需头注说明。
//!
//! 本文件涵盖四条 stop/stall 专项测试，每条必须携带独立的真实 LLM/TTS mock
//! 以及 collector 配置代码，无法在 ≤500 行内完成。仅 4 条测试平均每条
//! 120 余行；若强行拆到两个文件，断言所需的 helper（spawn_tts_mock_slow、
//! make_stall_producer、detached_audio 等）的可见性、stale 闭包与线程句柄
//! 将被迫在两处重复声明，并引入新的 borrow 复杂度——保留单文件才能保证
//! 「stop during X」主题测试集的语义零变化。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::support::{
    Collector, count_fact, detached_audio, facts, make_stall_producer, spawn_llm_mock,
    spawn_tts_mock, spawn_tts_mock_slow, wait_for,
};
use super::*;
use crate::app_event::{AppEvent, RootFact};

// ---- 1/3: stop_during_tts_in_flight ----
//
// LLM 已回 SSE、TTS 慢速响应（delay > stop 窗口）：在 TTS 在飞时
// `handle.stop()`。断言：stop 后无 TurnCompleted、无第二轮 LLM 请求、
// supervisor 有限时间内可继续受理下一轮且下一轮正常收口。
#[test]
fn stop_during_tts_in_flight_yields_no_completion_and_reopens() {
    let llm_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"好。。\"}}]}\n\n",
        "data: [DONE]\n\n"
    )
    .to_owned();
    let llm_base = spawn_llm_mock(llm_bodies.clone(), sse);
    // TTS 慢 1.2s：保证 stop 落在 TTS 在飞窗口内（请求已发出、响应未回）。
    let tts_base = spawn_tts_mock_slow(Duration::from_millis(1200));

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
            audio: None, // dry-run：聚焦控制面语义，不掺 PCM 路径,

            config_path: None,
            mod_events: None,
        },
        move |ev| {
            ec.lock().expect("poison").push(ev);
        },
    );

    assert!(handle.say("第一轮"));
    // 等 TTS 请求已被 mock 收到（在飞窗口已打开）。
    // LLM 响应快，TTS 延迟 1.2s ⇒ 数十毫秒后 TTS socket 必然已 accepted。
    std::thread::sleep(Duration::from_millis(400));
    handle.stop();

    // 关停句柄后等 stop 完全收口（drain_residual_events 跑完）。
    std::thread::sleep(Duration::from_millis(200));

    // 断言 (a)：零 TurnCompleted。
    assert_eq!(
        count_fact(&collector, |f| matches!(f, RootFact::TurnCompleted { .. })),
        0,
        "stop 整轮不得发 TurnCompleted"
    );
    // 断言 (a')：零 PlaybackDrained（dry-run 文本模式也不会出现）。
    assert_eq!(
        count_fact(&collector, |f| matches!(
            f,
            RootFact::PlaybackDrained { .. }
        )),
        0,
        "stop 整轮不得发 PlaybackDrained"
    );

    // 断言 (b)：supervisor 可继续受理下一轮。
    assert!(
        handle.say("第二轮"),
        "stop 后 supervisor 必须仍在空闲态可受理"
    );
    assert!(
        wait_for(Duration::from_secs(3), || count_fact(
            &collector,
            |f| matches!(f, RootFact::GenerationFinished { .. })
        ) >= 1),
        "第二轮必须正常收口（GenerationFinished 出现）"
    );

    // 关机握手。
    handle.quit();
    assert!(wait_for(Duration::from_secs(2), || collector
        .lock()
        .expect("poison")
        .iter()
        .any(|e| matches!(e, AppEvent::ShutdownReady))));
    handle.join();

    // 断言 (c)：恰好 1 个 LLM 请求（被停轮；后续第二轮因 TTS 慢/文本模式
    // 不必经 LLM 之外的真实代价，但 dry-run 仍要发——这里我们只断言 stop 的
    // 轮 + 新一轮的最小集：「停轮已 stop，不应被提交」由 bodies 数量作软证据）。
    let bodies = llm_bodies.lock().expect("poison");
    assert!(
        !bodies.is_empty(),
        "至少第一轮 LLM 请求必须发出: {bodies:?}"
    );
    // 第二轮 dry-run 必然再发一次 LLM。
    assert_eq!(
        bodies.len(),
        2,
        "stop 后 supervisor 受理的第二轮必须发新 LLM 请求（恰好两轮）: {bodies:?}"
    );
    // 第二轮请求体只含 system+user(第二轮)，证明被停轮历史未提交。
    let msgs2 = bodies[1]["messages"].as_array().expect("messages");
    assert_eq!(msgs2.len(), 2, "第二轮请求体不得带被停轮历史：{msgs2:?}");
    assert_eq!(msgs2[1]["role"], "user");
    assert_eq!(msgs2[1]["content"], "第二轮");
}

// ---- 2/3: stop_during_pending_pcm_pump ----
//
// detached ring cap=4 + 极慢 consumer：TTS chunk=8 源样本 ⇒ 32 设备样本，
// ring 必走 WouldBlock 多次；Stage B 泵会长期驻留。在窗口内 handle.stop()。
// 断言：stop 后根推进新 epoch、无 Drained/TurnCompleted、可再受理新轮。
#[test]
fn stop_during_pending_pcm_pump_advances_epoch_without_completion() {
    let (producer, mut render) = detached_audio(4);

    // consumer 极慢（每 100ms 取 16 设备样本）——保证 Stage B 泵长时间停留。
    let consumer_stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let cs = consumer_stop.clone();
    let consumer = std::thread::spawn(move || {
        while !cs.load(std::sync::atomic::Ordering::Relaxed) {
            let mut buf = [0f32; 16];
            render.render_f32(&mut buf);
            std::thread::sleep(Duration::from_millis(100));
        }
    });

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
    let ec = collector.clone();
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

    assert!(handle.say("hi"));
    // 等 PlaybackStarted 已出现（Stage B 泵工作已开始的硬证据）。
    assert!(
        wait_for(Duration::from_secs(3), || count_fact(
            &collector,
            |f| matches!(f, RootFact::PlaybackStarted { .. })
        ) >= 1),
        "Stage B 泵必须已开始（PlaybackStarted 必现）"
    );

    // 等待进入 Stage B 泵的「pending 驻留」窗口：GenFinished 已现 + Drained 未现
    // + 我们已经看到 PlaybackStarted ⇒ Stage B 处于 WouldBlock 等待。
    let in_pump_window = wait_for(Duration::from_secs(3), || {
        let fs = facts(&collector);
        fs.iter()
            .any(|f| matches!(f, RootFact::GenerationFinished { .. }))
            && !fs
                .iter()
                .any(|f| matches!(f, RootFact::PlaybackDrained { .. }))
            && fs
                .iter()
                .any(|f| matches!(f, RootFact::PlaybackStarted { .. }))
    });
    assert!(
        in_pump_window,
        "必须真实进入 Stage B 泵驻留窗口：Started+GenFinished 已现 + Drained 未现"
    );
    // 在窗口里 sleep 50ms 再打 stop。
    std::thread::sleep(Duration::from_millis(50));
    handle.stop();

    // 关停句柄后充分等待（确保无迟到 TurnCompleted / Drained）。
    std::thread::sleep(Duration::from_millis(200));

    let fs = facts(&collector);
    assert_eq!(
        count_fact(&collector, |f| matches!(f, RootFact::TurnCompleted { .. })),
        0,
        "Stage B 泵期 stop 整轮不得发 TurnCompleted：{fs:?}"
    );
    assert_eq!(
        count_fact(&collector, |f| matches!(
            f,
            RootFact::PlaybackDrained { .. }
        )),
        0,
        "Stage B 泵期 stop 整轮不得发 PlaybackDrained：{fs:?}"
    );
    // 根推进新 epoch：Collector 中应出现至少一个 NewEpoch{epoch:1}（ConversationUiEvent）。
    let new_epoch1_seen = collector.lock().expect("poison").iter().any(|e| {
        matches!(
            e,
            AppEvent::Conversation(crate::app_event::ConversationUiEvent::NewEpoch { epoch: 1 })
        )
    });
    assert!(new_epoch1_seen, "stop 后根必须推进到 epoch=1");

    // 关机握手。
    handle.quit();
    assert!(wait_for(Duration::from_secs(2), || collector
        .lock()
        .expect("poison")
        .iter()
        .any(|e| matches!(e, AppEvent::ShutdownReady))));
    handle.join();
    consumer_stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = consumer.join();
}

// ---- 3/3: stall_timeout_fatal_terminates_after_5s_zero_progress ----
//
// StallProducer：`try_enqueue_prepared` 恒返回 `WouldBlock{accepted:0}`，
// `healthy()` 恒 true，`is_drained()` 恒 false。Stage B 泵进入纯零进展等待。
// 断言：MAX_STALL(250 tick × 20ms = 5s) 后 supervisor 以 fatal 收尾：
//   - GenerationFinished{completed:false} 出现
//   - TurnCompleted{outcome_completed:false} 出现
//   - PlaybackCleared 出现**当且仅当**曾报过 PlaybackStarted；
//     StallProducer 永远 accepted:0 ⇒ 永不可能 Started ⇒ 无 PlaybackCleared
// 时间预算放 8s（5s stall + 余量）。
#[test]
fn stall_timeout_fatal_terminates_after_5s_zero_progress() {
    let (producer, _render) = make_stall_producer();
    // 不启动 consumer——确保 ring 永远满、StallProducer 永远 WouldBlock{0}。

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
    let ec = collector.clone();
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

    assert!(handle.say("hi"));

    // 8s 窗口内等待 GenerationFinished{completed:false}（最坏：5s stall + 收口）。
    assert!(
        wait_for(Duration::from_secs(8), || count_fact(
            &collector,
            |f| matches!(
                f,
                RootFact::GenerationFinished {
                    completed: false,
                    ..
                }
            )
        ) >= 1),
        "stall 5s 后 supervisor 必须以 GenerationFinished{{completed:false}} 致命收尾"
    );

    let fs = facts(&collector);
    // StallProducer 永远 accepted:0 ⇒ 永不可能 Started ⇒ 无 PlaybackCleared。
    assert_eq!(
        count_fact(&collector, |f| matches!(
            f,
            RootFact::PlaybackCleared { .. }
        )),
        0,
        "StallProducer 路径无 PlaybackStarted ⇒ 不得有 PlaybackCleared：{fs:?}"
    );
    // P0-2 归一化：TurnCompleted 必须 outcome_completed=false。
    assert!(
        fs.iter().any(|f| matches!(
            f,
            RootFact::TurnCompleted {
                outcome_completed: false,
                ..
            }
        )),
        "stall fatal 收尾必须发 TurnCompleted{{outcome_completed:false}}：{fs:?}"
    );
    // GenerationFinished 恰一次。
    assert_eq!(
        count_fact(&collector, |f| matches!(
            f,
            RootFact::GenerationFinished { .. }
        )),
        1,
        "GenerationFinished 必须恰一次：{fs:?}"
    );

    handle.quit();
    assert!(wait_for(Duration::from_secs(2), || collector
        .lock()
        .expect("poison")
        .iter()
        .any(|e| matches!(e, AppEvent::ShutdownReady))));
    handle.join();
}

// ---- P0-5: stop_during_drain_produces_no_commit_and_clean_next_round ----
//
// 真实进入 drain-watch：consumer 拖慢排空，collector 观察到
// 「GenerationFinished 已出现 && PlaybackDrained 未出现」窗口后 sleep 30ms
// 再 handle.stop()。断言：
//   (a) 全程零 TurnCompleted、零 PlaybackDrained；
//   (b) 新一轮请求体不含被停止轮 assistant 正文（证明未提交历史）；
//   (c) 第二轮正常收口。
//
// 2026-09-11 用户裁决：动作系统整体移除——原先的「控制组先证 `好的` ⇒
// Render(Perform)」以及断言「stop 后无 Perform」都已不可能成立
// （`rule_fallback` 触发源与 `RenderCommand` 投影一并拆除），故删除该部分；
// 本用例保留 stop-during-drain 的提交/收口语义，这部分与本裁决无关。
#[test]
fn stop_during_drain_produces_no_commit_and_clean_next_round() {
    // ---- 变体：drain-watch 中打 stop ----
    let (producer, mut render) = detached_audio(4);
    // consumer 极慢（每 80ms 取 16 设备样本）——保证 drain-watch 长时间停留。
    let consumer_stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let cs = consumer_stop.clone();
    let consumer = std::thread::spawn(move || {
        while !cs.load(std::sync::atomic::Ordering::Relaxed) {
            let mut buf = [0f32; 16];
            render.render_f32(&mut buf);
            std::thread::sleep(Duration::from_millis(80));
        }
    });

    let llm_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"好的\"}}]}\n\n",
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
    let ec = collector.clone();
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

    assert!(handle.say("hi"));
    // 等 GenerationFinished 出现。
    assert!(
        wait_for(Duration::from_secs(3), || count_fact(
            &collector,
            |f| matches!(f, RootFact::GenerationFinished { .. })
        ) >= 1),
        "stop 之前 GenerationFinished 必须先到"
    );
    // drain-watch 窗口：每 10ms 轮询 PlaybackDrained 是否出现。
    let in_drain_window = wait_for(Duration::from_secs(3), || {
        let fs = facts(&collector);
        fs.iter()
            .any(|f| matches!(f, RootFact::GenerationFinished { .. }))
            && !fs
                .iter()
                .any(|f| matches!(f, RootFact::PlaybackDrained { .. }))
    });
    assert!(
        in_drain_window,
        "必须真实进入 drain-watch：GenFinished 已现 + Drained 未现"
    );
    // 在窗口里 sleep 30ms 再打 stop。
    std::thread::sleep(Duration::from_millis(30));
    handle.stop();

    // 关停句柄后充分等待（确保无迟到 TurnCompleted / Drained）。
    std::thread::sleep(Duration::from_millis(200));

    // 断言 (a)：零 TurnCompleted、零 PlaybackDrained。
    let fs = facts(&collector);
    assert_eq!(
        count_fact(&collector, |f| matches!(f, RootFact::TurnCompleted { .. })),
        0,
        "stop 整轮不得发 TurnCompleted：{fs:?}"
    );
    assert_eq!(
        count_fact(&collector, |f| matches!(
            f,
            RootFact::PlaybackDrained { .. }
        )),
        0,
        "stop 整轮不得发 PlaybackDrained：{fs:?}"
    );

    // ---- 断言 (b)：stop 后 supervisor 仍可受理新轮，且**第二轮请求体**
    // 不含被停轮的 assistant 历史——这是 P0-4「未提交」的硬证据。
    // （复审口径修正：第一轮请求体本来就不含本轮 assistant 文本，
    //   不能证明未提交；必须观察 stop 之后的新一轮请求体。）
    assert!(
        handle.say("第二轮"),
        "stop 后 supervisor 必须仍在空闲态可受理"
    );
    assert!(
        wait_for(Duration::from_secs(3), || count_fact(
            &collector,
            |f| matches!(f, RootFact::GenerationFinished { .. })
        ) >= 2),
        "第二轮必须正常收口"
    );

    // 关机。
    handle.quit();
    assert!(wait_for(Duration::from_secs(2), || collector
        .lock()
        .expect("poison")
        .iter()
        .any(|e| matches!(e, AppEvent::ShutdownReady))));
    handle.join();
    consumer_stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = consumer.join();

    // 断言 (c)：第二轮 LLM 请求体**不含**被停轮的 assistant 正文。
    // 若被停轮历史被错误提交，第二轮请求体会带 system+user(hi)+assistant(好的)
    // +user(第二轮) 四段；正确实现只有 system+user(第二轮) 两段。
    let bodies = llm_bodies.lock().expect("poison");
    assert_eq!(
        bodies.len(),
        2,
        "恰好两轮 LLM 请求（stop 的轮 + 第二轮）: {bodies:?}"
    );
    let msgs2 = bodies[1]["messages"].as_array().expect("messages");
    assert_eq!(
        msgs2.len(),
        2,
        "第二轮请求体应只有 system+user，无被停轮历史：{msgs2:?}"
    );
    assert_eq!(msgs2[1]["role"], "user");
    assert_eq!(msgs2[1]["content"], "第二轮");
    let joined2 = serde_json::to_string(&bodies[1]).unwrap_or_default();
    assert!(
        !joined2.contains("好的"),
        "被停轮的 assistant 正文不得出现在第二轮请求体：{joined2}"
    );
}
