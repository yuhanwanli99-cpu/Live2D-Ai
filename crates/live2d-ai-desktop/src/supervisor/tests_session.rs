//! **L1 会话绑定**：会话级 system_prompt 覆盖的主链回归（2026-09-15）。
//!
//! 从 `tests_loop.rs` 拆出：那一份已经贴到「测试文件 ≤800 行」的上限，而这条
//! 用例自带完整的 mock 装配（LLM/TTS/server + 三轮对话），放回去会把两份互不
//! 相关的契约挤在一个文件里——拆开各自守一组不变量，也守住行数纪律。
//!
//! 这条测试守的是**主链唯一决议点**（`run_forever` 里每轮
//! `engine.set_system_prompt_override(scopes.prompt_for(session))`）：
//! 它坏了的表现不是报错，而是「B 会话用上了 A 的人设」——静默且最难查。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::support::{Collector, spawn_llm_mock, spawn_tts_mock, wait_for};
use super::*;
use crate::app_event::AppEvent;
// `SessionScopeStore::set / get` 是 trait 方法（固有方法只保留了 supervisor
// 热路径要用的 active/set_active），这里显式引入。
use live2d_ai_mod_system::SessionPromptSink as _;

/// 同一张会话表里，不同会话的 system_prompt 互不串；不带会话 → 回落全局。
#[test]
fn scoped_session_prompt_overrides_global_and_never_leaks_across_sessions() {
    let llm_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"收到。\"}}]}\n\n",
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
            conversation: live2d_ai_runtime::ConversationConfig::new("全局人设"),
            capabilities: live2d_ai_core::ModelCapabilities::all(),
            audio: None,
            config_path: None,
            mod_events: None,
        },
        move |ev| emit_collector.lock().expect("poison").push(ev),
    );

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

    // 会话 A 与 B 各有一张「卡」。
    handle.session_scopes().set("session-a", "A 的人设");
    handle.session_scopes().set("session-b", "B 的人设");

    assert!(
        handle.say_scoped("我是 A", Some("session-a".to_string())),
        "空闲态 Say 必须入队"
    );
    assert!(
        wait_for(Duration::from_secs(3), || gen_finished_count(&collector)
            >= 1),
        "第一轮应收口"
    );
    assert!(
        handle.say_scoped("我是 B", Some("session-b".to_string())),
        "第二轮必须入队"
    );
    assert!(
        wait_for(Duration::from_secs(3), || gen_finished_count(&collector)
            >= 2),
        "第二轮应收口"
    );
    // 第三轮**不带会话**：必须回落到全局人设（降级语义）。
    assert!(handle.say_scoped("没有会话", None), "第三轮必须入队");
    assert!(
        wait_for(Duration::from_secs(3), || gen_finished_count(&collector)
            >= 3),
        "第三轮应收口"
    );
    // 活动会话游标：`say_scoped(.., None)` 显式清掉（不带会话是一个事实）。
    // **必须在 join 之前读**——join 会消费掉 handle。
    assert_eq!(handle.session_scopes().active(), None);

    handle.quit();
    assert!(
        wait_for(Duration::from_secs(2), || {
            collector
                .lock()
                .expect("poison")
                .iter()
                .any(|e| matches!(e, AppEvent::ShutdownReady))
        }),
        "退出前必须发 ShutdownReady"
    );
    handle.join();

    let bodies = llm_bodies.lock().expect("poison");
    assert_eq!(bodies.len(), 3, "恰好三次 LLM 请求: {bodies:?}");
    let system_of = |i: usize| -> String {
        bodies[i]["messages"][0]["content"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    };
    assert_eq!(system_of(0), "A 的人设", "会话 A 必须用 A 的卡");
    assert_eq!(system_of(1), "B 的人设", "会话 B 必须用 B 的卡（不串卡）");
    assert_eq!(
        system_of(2),
        "全局人设",
        "不带会话 → 回落全局 persona.system_prompt（降级语义）"
    );
}
