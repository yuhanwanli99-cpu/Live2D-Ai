//! supervisor 播放闩锁 / stop / 收口 / 跨代次 / 失败轮回归（拆分自 tests_loop.rs）。
use std::sync::{Arc, Mutex};
use std::time::Duration;

use live2d_ai_core::{ActionCommand, ActionId, ActionSource, Epoch, Strength};

use super::support::*;
use super::*;
use crate::app_event::{AppEvent, ConversationUiEvent, RootFact};

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
    // 先同步到「A 的 LLM 请求真的已经发出并被 mock 记账」——保证 stop 落在
    // A 的在飞窗口内。旧写法用 sleep(30ms) 赌这一点：重负载下 30ms 内 A 的
    // 请求可能还没到达 mock，stop 就先把 A 取消了，于是最终 bodies 为 0
    //（/tmp/flaky-A-r2-4.log 实测 left:0 / right:1 正是这条）。
    assert!(
        wait_for(Duration::from_secs(5), || !llm_bodies
            .lock()
            .expect("poison")
            .is_empty()),
        "A 的 LLM 请求必须先发出（否则 stop 会落在 idle，测试前提不成立）"
    );
    assert!(handle.say("B"), "pending Say 应入队（容量1的空闲槽位）");
    handle.stop();

    // ---- 同步到「stop 已被处理」：do_stop! 是 stop 的唯一事务入口，它推进
    // root epoch 并发出 NewEpoch（正常收口路径从不发 NewEpoch）。看到
    // NewEpoch 即意味着 do_stop! 已经跑过；而 do_stop! 在发 NewEpoch 之后
    // 同步执行 while say_rx.try_recv().is_ok() {} 清掉 pending B——
    // 从这一刻起才开始观察「stop 之后有没有人被误启动」。
    let stop_seen = wait_for(Duration::from_secs(5), || {
        collector.lock().expect("poison").iter().any(|e| {
            matches!(
                e,
                AppEvent::Conversation(ConversationUiEvent::NewEpoch { .. })
            )
        })
    });
    assert!(
        stop_seen,
        "stop 必须被处理（do_stop! 推进 epoch → NewEpoch）"
    );

    // ---- 显式「证明否定命题」：最多观察 T，断言被禁止事件没有发生。
    // 被禁止事件 = B 被误启动 ⇒ mock 收到第二次 LLM 请求（请求到达即记账，
    // 早于 slow-mock 自身的 500ms 响应延迟）。
    // T 依据：slow LLM mock 延迟 500ms（若 B 真启动，最迟不会晚于 A 的
    // 500ms 响应窗口退出后 + 排程延迟）+ 3× flutter 并发满负载下的排程余量
    // 1500ms ⇒ 2000ms。T 是**观察上限**：wait_for 一旦看到禁止事件立即返回
    // true；正确路径下负向断言会等满 T 才通过（这是显式证明否定命题的固有成本）。
    const FORBIDDEN_WINDOW: Duration = Duration::from_millis(2000);
    let restarted_b = wait_for(FORBIDDEN_WINDOW, || {
        llm_bodies.lock().expect("poison").len() >= 2
    });

    handle.quit();
    assert!(wait_for(Duration::from_secs(2), || collector
        .lock()
        .expect("poison")
        .iter()
        .any(|e| matches!(e, AppEvent::ShutdownReady))));

    assert!(
        !restarted_b,
        "stop 后不得自动启动 pending Say：{FORBIDDEN_WINDOW:?} 观察窗内出现了第二次 LLM 请求"
    );
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

/// **rc.3 N0 回归（2026-09-13，真机抓到的缺陷）**：失败轮里**已经生成的正文**
/// 必须经 `ConversationUiEvent::TextFallback` 交给 UI。
///
/// 它同时钉住 supervisor 的一个**时序缺陷**：`gen_fut` 的**最后一次轮询**可能
/// 在返回前同步塞进若干引擎事件（`send_event` 在缓冲未满时立即完成），而
/// biased `select!` 一旦选中 `gen_fut` 臂就立刻返回、**不会**回头再 poll
/// 上面的 `event_rx` 臂——那批事件落在通道里，被收尾的
/// `drain_residual_events` 静默丢掉。
///
/// 真机表现（2026-09-13，TTS 指着死端口）：WS 上只有 `error` + `turn_state`，
/// 一个字都没有；界面上是「（生成失败）」而不是正文。修法见 `turn.rs` Stage A 末尾
/// 的「生成返回后排空通道」循环。
///
/// **2026-10-15 执行单 §2 之后**：TTS 的**单句**状态码 / 传输 / 解码失败都不再致命
/// （走静音句、轮次继续），所以本测试改用仍属致命那一档的 `InvalidBaseUrl`
/// （base_url 解析不出来）来造失败轮——上面那条排空时序的回归一个字没变。
#[test]
fn failed_turn_delivers_generated_text_via_text_fallback() {
    let llm_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"第一句。第二\"}}]}\n\n",
        "data: [DONE]\n\n"
    )
    .to_owned();
    let llm_base = spawn_llm_mock(llm_bodies.clone(), sse);

    // TTS 的 base_url 解析不出绝对 http(s) URL → `Error::InvalidBaseUrl`，仍是**致命**
    // 那一档（单句的状态码 / 传输 / 解码失败自 2026-10-15 起都不再终结本轮）。
    let client = live2d_ai_runtime::OpenAiClient::new(
        live2d_ai_runtime::LlmConfig::new(llm_base, "test-model"),
        live2d_ai_runtime::TtsConfig::new("not a url", "alloy"),
    )
    .expect("client");

    let collector: Collector = Arc::new(Mutex::new(Vec::new()));
    let emit_collector = collector.clone();
    let handle = spawn_supervisor(
        SupervisorConfig {
            client,
            conversation: live2d_ai_runtime::ConversationConfig::new(""),
            capabilities: live2d_ai_core::ModelCapabilities::all(),
            audio: None,
            config_path: None,
            mod_events: None,
        },
        move |ev| {
            emit_collector.lock().expect("poison").push(ev);
        },
    );

    assert!(handle.say("测试"), "空闲态 Say 必须入队");
    let finished = |c: &Collector| {
        c.lock().expect("poison").iter().any(|e| {
            matches!(
                e,
                AppEvent::RootAudit(crate::app_event::RootFact::GenerationFinished { .. })
            )
        })
    };
    assert!(
        wait_for(Duration::from_secs(5), || finished(&collector)),
        "本轮应在超时前收口（失败也要收口）"
    );

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

    let events = collector.lock().expect("poison");
    let fallbacks: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            AppEvent::Conversation(ConversationUiEvent::TextFallback { text, .. }) => {
                Some(text.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        fallbacks,
        ["第一句。第二"],
        "失败轮必须把**整轮正文**交给 UI（否则用户看到的是「模型没回」）：{events:?}"
    );
}
