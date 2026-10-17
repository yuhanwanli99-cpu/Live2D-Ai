//! supervisor 闭环 + 竞态测试：双轮历史提交、记忆同轮生效、Mod 事件桥。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::support::{Collector, spawn_llm_mock, spawn_tts_mock, wait_for};
use super::*;
use crate::app_event::{AppEvent, RootFact};
use live2d_ai_mod_system::ModEventTopic;

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

/// **记忆同轮生效**（2026-09-15）：Mod 在 `TurnPrompt` 里写进会话注入槽的东西，
/// 必须出现在**本轮**发给 LLM 的请求体里，而不是下一轮。
///
/// 这个测试同时钉住三件事：
/// 1. 时序：`TurnPrompt` 投递**先于** supervisor 决议本轮 system_prompt
///    （旧实现反了 → 本轮请求体只有旧快照，注入要等下一轮）；
/// 2. 会话绑定：A 会话的 MEMORY 槽不进 B 会话的请求体；
/// 3. 无会话注入时的降级：回落配置里的全局 `persona.system_prompt`。
///
/// 注意：这里用的是**同步** sink（测试线程直接写表），它模拟的是生产 sink 的
/// 可观察结果——生产侧由 `mod_event_sink` 用 `dispatch_event_and_flush` 保证
/// 「写完了才读」（回执语义见 `mod_registry::tests`）。
#[test]
fn memory_written_on_turn_prompt_lands_in_the_same_turn_request() {
    use live2d_ai_mod_system::{SESSION_PROMPT_OWNER_MEMORY, SessionPromptSink as _};

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

    // 晚绑定：sink 在 supervisor 线程上跑，而会话表句柄要等 `spawn_supervisor`
    // 返回后才拿得到（`handle.session_scopes()`）。
    let scopes: Arc<Mutex<Option<SessionScopeStore>>> = Arc::new(Mutex::new(None));
    let scopes_for_sink = scopes.clone();
    let collector: Collector = Arc::new(Mutex::new(Vec::new()));
    let emit_collector = collector.clone();
    let handle = spawn_supervisor(
        SupervisorConfig {
            client,
            conversation: live2d_ai_runtime::ConversationConfig::new("人设"),
            capabilities: live2d_ai_core::ModelCapabilities::all(),
            audio: None,
            config_path: None,
            mod_events: Some(Arc::new(move |t, p: &str, s: Option<&str>| {
                // 只对 session-a 注入，用来验证 A/B 不串。
                if t != ModEventTopic::TurnPrompt || s != Some("session-a") {
                    return;
                }
                let guard = scopes_for_sink.lock().expect("poison");
                if let Some(store) = guard.as_ref() {
                    store.set_owned(
                        SESSION_PROMPT_OWNER_MEMORY,
                        "session-a",
                        &format!("<!-- memory -->\n- 暗号是薄荷（本轮输入：{p}）"),
                    );
                }
            })),
        },
        move |ev| emit_collector.lock().expect("poison").push(ev),
    );
    *scopes.lock().expect("poison") = Some(handle.session_scopes().clone());

    let gen_finished = |c: &Collector| {
        c.lock()
            .expect("poison")
            .iter()
            .filter(|e| matches!(e, AppEvent::RootAudit(RootFact::GenerationFinished { .. })))
            .count()
    };

    assert!(handle.say_scoped("A 的暗号：薄荷", Some("session-a".into())));
    assert!(
        wait_for(Duration::from_secs(3), || gen_finished(&collector) >= 1),
        "A 会话第一轮应在超时前收口"
    );
    assert!(handle.say_scoped("B 在说话", Some("session-b".into())));
    assert!(
        wait_for(Duration::from_secs(3), || gen_finished(&collector) >= 2),
        "B 会话第一轮应在超时前收口"
    );

    let bodies = llm_bodies.lock().expect("poison");
    assert_eq!(bodies.len(), 2, "恰好两次 LLM 请求: {bodies:?}");
    let system_of = |i: usize| -> String {
        bodies[i]["messages"][0]["content"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    };
    let a = system_of(0);
    assert!(
        a.contains("暗号是薄荷"),
        "同轮：TurnPrompt 刚写的 MEMORY 块必须已在本轮请求体里，实际 system={a:?}"
    );
    assert!(
        a.contains("本轮输入：A 的暗号：薄荷"),
        "本轮 TurnPrompt 的正文必须已参与检索（不是上一轮的残留）: {a:?}"
    );
    let b = system_of(1);
    assert!(
        !b.contains("暗号是薄荷"),
        "会话 A 的记忆不得进会话 B: {b:?}"
    );
    assert_eq!(b, "人设", "本会话无注入 → 回落全局 persona.system_prompt");

    handle.quit();
    handle.join();
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
            // L1：第三个参数是会话 id（本测试不带会话 → None）。
            mod_events: Some(Arc::new(move |t, p, _s| {
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

    // **Wave 3**：turn 收口后必须发出 `TurnEnded`（payload = 同一 turn id）。
    // GenerationFinished 与 TurnEnded 之间有竞态，必须显式等到它到达再断言。
    assert!(
        wait_for(Duration::from_secs(3), || {
            mod_topics
                .lock()
                .expect("poison")
                .iter()
                .any(|(t, _)| *t == ModEventTopic::TurnEnded)
        }),
        "turn 收口后应发出 TurnEnded"
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
    // **Wave 2**：同一提交点还必须把**本轮输入正文**交给 Mod——
    // `TurnStarted` 只有序号，记忆 / 导演类 Mod 靠 `TurnPrompt` 拿正文。
    let tp_payload = topics
        .iter()
        .find(|(t, _)| *t == ModEventTopic::TurnPrompt)
        .map(|(_, p)| p.clone())
        .expect("应发出 TurnPrompt");
    assert_eq!(
        tp_payload, "触发一轮",
        "TurnPrompt 的 payload 是本轮输入正文"
    );
    // 顺序：TurnStarted 必须先于 TurnPrompt（后者依赖前者的语境）。
    let ts_idx = topics
        .iter()
        .position(|(t, _)| *t == ModEventTopic::TurnStarted)
        .unwrap();
    let tp_idx = topics
        .iter()
        .position(|(t, _)| *t == ModEventTopic::TurnPrompt)
        .unwrap();
    assert!(ts_idx < tp_idx, "TurnStarted 应在 TurnPrompt 之前");
    // **Wave 3**：TurnEnded 必须在 TurnPrompt 之后，且 payload 与 TurnStarted
    // 的 turn id 一致（同一个「第几轮」）。
    let te_idx = topics
        .iter()
        .position(|(t, _)| *t == ModEventTopic::TurnEnded)
        .expect("应发出 TurnEnded");
    assert!(tp_idx < te_idx, "TurnEnded 应在 TurnPrompt 之后");
    let te_payload = topics[te_idx].1.clone();
    assert_eq!(
        te_payload, ts_payload,
        "TurnEnded 的 payload 是同一 turn id"
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
