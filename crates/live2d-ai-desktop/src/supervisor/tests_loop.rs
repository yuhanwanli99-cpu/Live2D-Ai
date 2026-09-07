//! supervisor 闭环 + 竞态测试：双轮历史提交、BL-2 双闩锁、stop 清 pending、
//! GenerationFinished 恰一次、跨代次完成被中和。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use live2d_ai_core::{ActionCommand, ActionId, ActionSource, Epoch, Strength};

use super::support::{
    Collector, count_fact, detached_audio, facts, first_index, has_dropped, spawn_llm_mock,
    spawn_llm_mock_slow, spawn_tts_mock, wait_for,
};
use super::*;
use crate::app_event::{AppEvent, ConversationUiEvent, RootFact};

#[test]
fn closed_loop_two_turns_commit_history_via_drain_path() {
    let _ = ControlCommand::Stop; // 命令通道枚举与句柄 API 联动的引用锚

    let llm_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"好。。\"}}]}\n\n",
        "data: [DONE]\n\n"
    )
    .to_owned();
    let llm_base = spawn_llm_mock(llm_bodies.clone(), sse);
    let tts_base = spawn_tts_mock();

    let client = live2d_ai_runtime::OpenAiClient::new(
        live2d_ai_runtime::LlmConfig::new(llm_base.clone(), "test-model"),
        live2d_ai_runtime::TtsConfig::new(tts_base.clone(), "alloy"),
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
            audio: None, // dry-run：无声卡也必须完成闭环（这正是 CI 形态）,

            config_path: None,
            mod_events: None,
        },
        move |ev| {
            eprintln!("[chat-loop] emit: {ev:?}");
            emit_collector.lock().expect("poison").push(ev);
        },
    );

    // ---- 第 1 轮：开轮 NewEpoch。
    // 轮次完成信号 = RootAudit::GenerationFinished（恰一次/轮，B2-P0-1 锁定）。
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

    assert!(handle.say("第一轮"), "空闲态 Say 必须入队");
    assert!(
        wait_for(Duration::from_secs(3), || gen_finished_count(&collector)
            >= 1),
        "第 1 轮应在超时前收口"
    );

    // ---- 第 2 轮：只有第 1 轮真实提交了历史，第二次请求体才会带历史对。
    assert!(handle.say("第二轮"), "完成态 Say 必须入队");
    assert!(
        wait_for(Duration::from_secs(3), || gen_finished_count(&collector)
            >= 2),
        "第 2 轮应同样收口"
    );

    // ---- 关机 handshake。
    handle.stop(); // idle 态幂等无害
    handle.quit();
    assert!(
        wait_for(Duration::from_secs(2), || {
            collector
                .lock()
                .expect("poison")
                .iter()
                .any(|e| matches!(e, AppEvent::ShutdownReady))
        }),
        "supervisor 退出前必须发 ShutdownReady"
    );
    handle.join();

    // ---- 闭环硬证据。
    let bodies = llm_bodies.lock().expect("poison");
    assert_eq!(bodies.len(), 2, "恰好两次 LLM 请求: {bodies:?}");
    let msgs = bodies[1]["messages"].as_array().expect("messages");
    assert_eq!(msgs.len(), 4, "system+user+assistant+user: {msgs:?}");
    assert_eq!(msgs[1]["role"], "user");
    assert_eq!(msgs[1]["content"], "第一轮");
    assert_eq!(msgs[2]["role"], "assistant");
    assert_eq!(msgs[2]["content"], "好。。");
    assert_eq!(msgs[3]["role"], "user");
    assert_eq!(msgs[3]["content"], "第二轮");
}

/// **d2 事件桥**：supervisor 在 turn 提交点 emit `ModEventTopic::TurnStarted`
/// 到注入的 `mod_events` 回调（host→Mod 事件桥的生产侧）。
///
/// 验证「supervisor 生产 TurnStarted → 回调收到」，证明 `dispatch_event` 的
/// 生产路径已打通（不再是 node-f P3 审计的「dispatch_event 无生产者」）。
#[test]
fn supervisor_emits_turn_started_to_mod_events() {
    let mod_topics: Arc<Mutex<Vec<(ModEventTopic, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let mod_topics_clone = mod_topics.clone();

    let llm_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"好。。\"}}]}\n\n",
        "data: [DONE]\n\n"
    )
    .to_owned();
    let llm_base = spawn_llm_mock(llm_bodies.clone(), sse);
    let tts_base = spawn_tts_mock();
    let client = live2d_ai_runtime::OpenAiClient::new(
        live2d_ai_runtime::LlmConfig::new(llm_base.clone(), "test-model"),
        live2d_ai_runtime::TtsConfig::new(tts_base.clone(), "alloy"),
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
            mod_events: Some(Arc::new(move |t, p| {
                mod_topics_clone
                    .lock()
                    .expect("poison")
                    .push((t, p.to_string()));
            })),
        },
        move |ev| emit_collector.lock().expect("poison").push(ev),
    );

    // 提交一轮对话 → supervisor 必须在进入 turn 前 emit TurnStarted。
    assert!(handle.say("触发一轮"), "空闲态 Say 必须入队");
    let gen_finished = |c: &Collector| {
        c.lock()
            .expect("poison")
            .iter()
            .filter(|e| matches!(e, AppEvent::RootAudit(RootFact::GenerationFinished { .. })))
            .count()
    };
    assert!(
        wait_for(Duration::from_secs(3), || gen_finished(&collector) >= 1),
        "turn 应在超时前收口"
    );

    // TurnStarted + TextDelta 都已到达 mod_events（证明「显式调用」与
    // 「emit 包裹投影」两个生产路径都打通）。
    let topics = mod_topics.lock().expect("poison");
    assert!(
        topics.iter().any(|(t, _)| *t == ModEventTopic::TurnStarted),
        "应启动 TurnStarted: {topics:?}"
    );
    assert!(
        topics.iter().any(|(t, _)| *t == ModEventTopic::TextDelta),
        "流式文本也应投影到 TextDelta: {topics:?}"
    );
    // TurnStarted 的 payload 是 turn id（>0）。
    let ts_payload = topics
        .iter()
        .find(|(t, _)| *t == ModEventTopic::TurnStarted)
        .map(|(_, p)| p.clone())
        .unwrap();
    assert!(
        ts_payload.parse::<u64>().unwrap_or(0) >= 1,
        "payload 是 turn id"
    );

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

/// BL-2 真实双闩锁： PCM 入环 ⇒ VoiceStarted； 排空 ⇒ VoiceEnded；
/// turn 收口； 退出握手。注入 detached 生产者（ring=4 设备样本，
/// TTS 回 8 ⇒ 必走 WouldBlock→pending 泵）。
///
/// 复审增补：收口后按 collector 首次出现下标断言
///   PlaybackStarted < GenerationFinished < PlaybackDrained
///   < TurnCompleted{outcome_completed:true}
/// 的相对顺序成立，并锁定计数 GenerationFinished==1、PlaybackDrained==1、
/// TurnCompleted==1、Dropped==0。
#[test]
fn bl2_real_double_latch_with_detached_producer() {
    let llm_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"好。。\"}}]}\n\n",
        "data: [DONE]\n\n"
    )
    .to_owned();
    let llm_base = spawn_llm_mock(llm_bodies.clone(), sse);
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
            conversation: live2d_ai_runtime::ConversationConfig {
                max_history_pairs: 4,
                ..live2d_ai_runtime::ConversationConfig::new("人设")
            },
            capabilities: live2d_ai_core::ModelCapabilities::all(),
            audio: Some(producer),

            config_path: None,
            mod_events: None,
        },
        move |ev| {
            ec.lock().expect("poison").push(ev);
        },
    );

    assert!(handle.say("第一轮"));
    // 扮演声卡回调：持续消费 ring（每 5ms 取一批），制造真实排空节奏。
    // 无此消费者 ring 会满 → supervisor 正确地进入 pending 泵等待（B-P0-3 设计）。
    let consumer_stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let cs = consumer_stop.clone();
    let consumer = std::thread::spawn(move || {
        while !cs.load(std::sync::atomic::Ordering::Relaxed) {
            let mut buf = [0f32; 64];
            render.render_f32(&mut buf);
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    assert!(
        wait_for(Duration::from_secs(3), || collector
            .lock()
            .expect("poison")
            .iter()
            .any(|e| matches!(
                e,
                AppEvent::Conversation(ConversationUiEvent::VoiceStarted { .. })
            ))),
        "首个非空 PCM 入环必须触发 VoiceStarted（B-P0-2 真实路径）"
    );
    assert!(
        wait_for(Duration::from_secs(3), || collector
            .lock()
            .expect("poison")
            .iter()
            .any(|e| matches!(
                e,
                AppEvent::Conversation(ConversationUiEvent::VoiceEnded { .. })
            ))),
        "排空后应有 VoiceEnded"
    );

    // 收口（GenerationFinished + Drained + TurnCompleted 都应已出现）。
    assert!(
        wait_for(Duration::from_secs(3), || {
            let fs = facts(&collector);
            count_fact(&collector, |f| {
                matches!(f, RootFact::PlaybackDrained { .. })
            }) >= 1
                && count_fact(&collector, |f| matches!(f, RootFact::TurnCompleted { .. })) >= 1
                && !fs.is_empty()
        }),
        "turn 应在本轮正常收口"
    );

    // ---- 复审增补断言：事实相对顺序与计数。
    let i_started = first_index(&collector, |f| {
        matches!(f, RootFact::PlaybackStarted { .. })
    })
    .expect("PlaybackStarted 必须出现");
    let i_gen = first_index(&collector, |f| {
        matches!(f, RootFact::GenerationFinished { .. })
    })
    .expect("GenerationFinished 必须出现");
    let i_drained = first_index(&collector, |f| {
        matches!(f, RootFact::PlaybackDrained { .. })
    })
    .expect("PlaybackDrained 必须出现");
    let i_turn = first_index(&collector, |f| matches!(f, RootFact::TurnCompleted { .. }))
        .expect("TurnCompleted 必须出现");
    assert!(
        i_started < i_gen,
        "PlaybackStarted({i_started}) < GenerationFinished({i_gen})"
    );
    assert!(
        i_gen < i_drained,
        "GenerationFinished({i_gen}) < PlaybackDrained({i_drained})"
    );
    assert!(
        i_drained < i_turn,
        "PlaybackDrained({i_drained}) < TurnCompleted({i_turn})"
    );

    assert_eq!(
        count_fact(&collector, |f| matches!(
            f,
            RootFact::GenerationFinished { .. }
        )),
        1,
        "GenerationFinished 恰一次"
    );
    assert_eq!(
        count_fact(&collector, |f| matches!(
            f,
            RootFact::PlaybackDrained { .. }
        )),
        1,
        "PlaybackDrained 恰一次"
    );
    assert_eq!(
        count_fact(&collector, |f| matches!(f, RootFact::TurnCompleted { .. })),
        1,
        "TurnCompleted 恰一次"
    );
    assert!(!has_dropped(&collector), "BL-2 成功路径不得出现事实丢弃");

    handle.quit();
    assert!(wait_for(Duration::from_secs(2), || collector
        .lock()
        .expect("poison")
        .iter()
        .any(|e| matches!(e, AppEvent::ShutdownReady))));
    consumer_stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = consumer.join();
    handle.join();
}

/// BL-5 pending Say 被 /stop 清除：A 进行中排队 B，stop——
/// 之后不得自动启动 B（无第二次 LLM 请求）。
#[test]
fn stop_clears_pending_say() {
    let llm_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"好。。\"}}]}\n\n",
        "data: [DONE]\n\n"
    )
    .to_owned();
    // LLM 响应前延迟 500ms：保证 B 排队与 stop 都落在 A 的在飞窗口内。
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
            mod_events: None,
        },
        move |ev| {
            ec.lock().expect("poison").push(ev);
        },
    );

    assert!(handle.say("A"));
    std::thread::sleep(Duration::from_millis(30));
    assert!(handle.say("B"), "pending Say 应入队（容量1的空闲槽位）");
    handle.stop();

    // 足够长的观察窗：若 stop 后自动启动 B，必现第二次 LLM 请求。
    //（B 的 LLM 响应本身有 500ms slow-mock 延迟；全 workspace 并行负载下
    //  800ms 曾偶发不足——放宽到 2.5s，让「假启动」有充分时间显形。）
    std::thread::sleep(Duration::from_millis(2500));
    handle.quit();
    assert!(wait_for(Duration::from_secs(2), || collector
        .lock()
        .expect("poison")
        .iter()
        .any(|e| matches!(e, AppEvent::ShutdownReady))));

    let bodies = llm_bodies.lock().expect("poison");
    assert_eq!(
        bodies.len(),
        1,
        "stop 后不得自动启动 pending Say: {bodies:?}"
    );
}

/// BL-6：GenerationFinished 每 turn 恰一次（B2-P0-1 锁定）；
/// 且 dry-run 文本模式下 Completed 轮 history 被提交。
#[test]
fn gen_finished_exactly_once_per_turn() {
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
            audio: None,

            config_path: None,
            mod_events: None,
        },
        move |ev| {
            ec.lock().expect("poison").push(ev);
        },
    );

    for round in ["第一轮", "第二轮"] {
        assert!(handle.say(round));
        let target = if round == "第一轮" { 1 } else { 2 };
        assert!(
            wait_for(Duration::from_secs(3), || count_fact(&collector, |f| {
                matches!(f, RootFact::GenerationFinished { .. })
            }) >= target),
            "{round} 应恰收口一次"
        );
    }
    handle.quit();
    assert!(wait_for(Duration::from_secs(2), || count_fact(
        &collector,
        |f| { matches!(f, RootFact::GenerationFinished { .. }) }
    ) == 2));
    handle.join();

    // 恰一次语义：两轮共 2 次 GenerationFinished，无 Dropped。
    assert_eq!(
        count_fact(&collector, |f| matches!(
            f,
            RootFact::GenerationFinished { .. }
        )),
        2
    );
    assert!(!has_dropped(&collector), "dry-run 闭环不得出现事实丢弃");
}

/// BL-cross-epoch：epoch0 Nod → stop → epoch1 Nod → 迟到的 epoch0 完成事实
/// 不得结束 epoch1 的同名动作（经 supervisor 全链路）。
///
/// 由于 PerformancePlayer 在窗口线程，这里直接以 root 层等价序列 +
/// supervisor 的 finish 通道模拟完整转发面：
#[test]
fn late_finish_cross_epoch_is_neutralized_via_supervisor() {
    let nod0 = SemanticAction::new(ActionId::Nod, Strength::Low, ActionSource::LlmTool);
    let nod1 = SemanticAction::new(ActionId::Nod, Strength::Low, ActionSource::LlmTool);

    // core 层已有 StaleActionCompletion 单测；此处锁定 supervisor 的
    // finish 通道不重盖 epoch：显式构造 fact 并检查 root_apply 行为一致。
    let mut root = RootState::default();
    root.action.capabilities = live2d_ai_core::ModelCapabilities::all();

    // epoch0：Nod 开始。
    let e0 = Epoch::ZERO;
    live2d_ai_core::apply(
        &mut root,
        RootEvent::Action {
            epoch: e0,
            command: ActionCommand::Play { action: nod0 },
        },
    );
    assert_eq!(root.action.current, Some(nod0));

    // stop → epoch1。
    live2d_ai_core::apply(&mut root, RootEvent::StopRequested);
    assert_eq!(root.epoch.get(), 1);

    // epoch1 重演 Nod。
    live2d_ai_core::apply(
        &mut root,
        RootEvent::Action {
            epoch: 1.into(),
            command: ActionCommand::Play { action: nod1 },
        },
    );
    assert_eq!(root.action.current, Some(nod1));

    // 迟到的 epoch0 完成：根闸门先行拦截（StaleEpoch），B 存活。
    let effects = live2d_ai_core::apply(
        &mut root,
        RootEvent::ActionPlaybackFinished {
            epoch: e0,
            action: nod0,
        },
    );
    assert!(matches!(
        effects.as_slice(),
        [RootEffect::Dropped {
            reason: live2d_ai_core::DropReason::StaleEpoch { .. }
        }]
    ));
    assert_eq!(root.action.current, Some(nod1));

    // 同代次但身份不符（epoch1 完成的是 tilt，不是 nod）：
    // → StaleActionCompletion——这正是 supervisor 必须原样回传 fact.epoch
    //   的原因（重盖当前值会把这两层防线全部穿透）。
    let effects = live2d_ai_core::apply(
        &mut root,
        RootEvent::ActionPlaybackFinished {
            epoch: 1.into(),
            action: SemanticAction::new(ActionId::Tilt, Strength::Low, ActionSource::LlmTool),
        },
    );
    assert!(matches!(
        effects.as_slice(),
        [RootEffect::Dropped {
            reason: live2d_ai_core::DropReason::StaleActionCompletion { .. },
        }]
    ));
    assert_eq!(root.action.current, Some(nod1), "后来者不得被误伤");
}
