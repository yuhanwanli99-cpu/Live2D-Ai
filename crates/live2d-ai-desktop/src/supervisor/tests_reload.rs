//! 热重载（Reload）端到端测试：
//!
//! 1. `reload_rebuilds_client_after_config_save` — PATCH 保存端点 B 后
//!    `handle.reload()` 立即生效，下一轮请求落到端点 B；
//! 2. `reload_falls_back_on_invalid_url` — 坏 URL 触发 reload 失败，**保留**
//!    旧 engine，下一轮请求仍到旧端点；
//! 3. `reload_during_inflight_turn_applies_to_next_turn` — 在飞 turn 中收到
//!    Reload：当前 turn 收尾，**下一轮**才用新配置。
//!
//! 与 W1/W2 的契约：reload 是「保存并应用」，由 PATCH/egui save 写盘成功后
//! 触发；本文件不写盘（写盘是 W2 职责），只验证 supervisor 收到 Reload 后
//! 的行为。

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use live2d_ai_core::ModelCapabilities;
use live2d_ai_runtime::{AppSettings, ConversationConfig, LlmConfig, OpenAiClient, TtsConfig};

use crate::supervisor::{SupervisorConfig, SupervisorHandle, spawn_supervisor};

use super::support::{Collector, spawn_llm_mock, spawn_tts_mock, wait_for};

/// 与现有 `spawn_text_only_collector` 同形态，但显式接收 config_path；
/// 复用相同 mock 基建。
fn spawn_text_only_collector_with_path(
    llm_sse: String,
    config_path: std::path::PathBuf,
) -> (
    SupervisorHandle,
    Collector,
    Arc<Mutex<Vec<serde_json::Value>>>,
) {
    let llm_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let llm_base = spawn_llm_mock(llm_bodies.clone(), llm_sse);
    let tts_base = spawn_tts_mock();
    let client = OpenAiClient::new(
        LlmConfig::new(llm_base, "test-model"),
        TtsConfig::new(tts_base, "alloy"),
    )
    .expect("client");
    let collector: Collector = Arc::new(Mutex::new(Vec::new()));
    let ec = collector.clone();
    let handle = spawn_supervisor(
        SupervisorConfig {
            client,
            conversation: ConversationConfig::new("人设"),
            capabilities: ModelCapabilities::all(),
            audio: None,
            config_path: Some(config_path.to_string_lossy().into_owned()),
            mod_events: None,
        },
        move |ev| {
            ec.lock().expect("poison").push(ev);
        },
    );
    (handle, collector, llm_bodies)
}

/// 一段固定 SSE 响应（足够让 LLM 流正常结束、Terminal 发出）。
fn canned_sse() -> String {
    "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n\
     data: {\"choices\":[{\"finish_reason\":\"stop\"}],\"usage\":{\"total_tokens\":1}}\n\n\
     data: [DONE]\n\n"
        .to_string()
}

/// 写一个最小 AppSettings 到 path（含有效 base_url）。
///
/// 测试需要 LLM + TTS 两段都填（reload 走 `resolve_with` 会校验两段 URL）；
/// 仅写 LLM 段会让 resolve 失败（tts base_url 缺省 = 空串 = url_invalid）。
/// TTS 段借用调用方传入的 tts_base（生产中 TTS 与 LLM 是独立服务，但测试
/// 只关心 reload 路径是否走通，复用同一 URL 不会改变断言）。
fn write_settings(path: &std::path::Path, llm_base_url: &str) {
    write_settings_with_tts(path, llm_base_url, llm_base_url);
}

/// 写一个最小 AppSettings 到 path（含有效 llm + tts base_url）。
fn write_settings_with_tts(path: &std::path::Path, llm_base_url: &str, tts_base_url: &str) {
    let s = AppSettings {
        llm: live2d_ai_runtime::settings::LlmSettings {
            base_url: llm_base_url.to_string(),
            model: "test-model".to_string(),
            api_key_env: None,
            max_tokens: None,
        },
        tts: live2d_ai_runtime::settings::TtsSettings {
            base_url: tts_base_url.to_string(),
            model: None,
            voice: "alloy".to_string(),
            response_format: "pcm".to_string(),
            api_key_env: None,
            sample_rate: 24_000,
            channels: 1,
        },
        ..AppSettings::default()
    };
    std::fs::write(path, s.to_toml_string()).expect("write settings");
}

// ---- 1. 主路径：reload 重建 client，下一轮请求落到新端点 ----

#[test]
fn reload_rebuilds_client_after_config_save() {
    // 阶段 1：起一个 mock LLM 端点 A。
    let sse = canned_sse();
    let llm_a_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let llm_a_base = spawn_llm_mock(llm_a_bodies.clone(), sse.clone());
    let tts_base = spawn_tts_mock();
    // 把初始 settings 写到临时文件，端点 A。
    let tmp = std::env::temp_dir().join(format!(
        "live2d_ai_test_reload_main_{}.toml",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&tmp);
    write_settings(&tmp, &llm_a_base);

    // 构造初始 client（指向 A），起 supervisor。
    let client = OpenAiClient::new(
        LlmConfig::new(llm_a_base.clone(), "test-model"),
        TtsConfig::new(tts_base, "alloy"),
    )
    .expect("client");
    let collector: Collector = Arc::new(Mutex::new(Vec::new()));
    let ec = collector.clone();
    let handle = spawn_supervisor(
        SupervisorConfig {
            client,
            conversation: ConversationConfig::new("人设"),
            capabilities: ModelCapabilities::all(),
            audio: None,
            config_path: Some(tmp.to_string_lossy().into_owned()),
            mod_events: None,
        },
        move |ev| {
            ec.lock().expect("poison").push(ev);
        },
    );

    // 阶段 2：先跑一轮，请求应该落到端点 A。
    assert!(handle.say("first"), "Say 容量 1，必须可入队");
    // **flaky fix**：高压复跑（--test-threads=8）下 8 个 LLM mock 同时
    // 冷启动，OS 调度 + Tokio runtime 初始化 + reqwest 客户端连接建立叠
    // 加，第一轮 LLM 请求偶发跑过 3s（实测 6s 仍偶发）。放宽到 10s 仅本
    // 场景用，其余 reload 等待保持 3s（热路径，warm cache）。tests_stall
    // 已用 8s 作先例，本场景为冷启动首轮，留足余量。
    assert!(
        wait_for(Duration::from_secs(10), || !llm_a_bodies
            .lock()
            .expect("poison")
            .is_empty()),
        "第一轮 LLM 必须已发出"
    );

    // 阶段 3：起端点 B（独立 mock），改写 settings 文件指向 B。
    let llm_b_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let llm_b_base = spawn_llm_mock(llm_b_bodies.clone(), sse.clone());
    write_settings(&tmp, &llm_b_base);

    // 阶段 4：触发 Reload（模拟 PATCH/egui 保存成功后的调用）。
    handle.reload();

    // 阶段 5：等 Reload 生效（idle 态同步处理，几毫秒内完成）。
    // 我们没有直接的"已 Reload"事件：靠下一轮请求落到 B 来间接验证。
    // 给一点时间让 idle select 跑一次。
    // **flaky fix**：固定 sleep(50) 在并发负载下偶发不足——supervisor
    // 可能在 turn drain 阶段尚未回到 select! 时 sleep 已结束，后续
    // say("second") 与控制通道里的 Reload 在 select! 中随机选臂，可能
    // 先抢到 say 而走旧 engine → 第二轮落到 A 断言失败。改为双闸门：
    //   (a) 等 GenerationFinished 出现（supervisor 至少过了 turn 收口），
    //   (b) 等 reload_pending 标志被清（apply_reload 已被调用，引擎同
    //       步替换完毕）——之后再触发 say 必然命中新 client。
    assert!(
        wait_for(Duration::from_secs(3), || !handle
            .reload_pending_for_test()
            .load(Ordering::SeqCst)),
        "reload 后 supervisor 必须已消费 reload_pending（apply_reload 同步替换 engine）"
    );

    // 阶段 6：跑第二轮，请求应落到端点 B（不再到 A）。
    assert!(handle.say("second"), "Say 容量 1，第二轮必须可入队");
    assert!(
        wait_for(Duration::from_secs(3), || !llm_b_bodies
            .lock()
            .expect("poison")
            .is_empty()),
        "第二轮 LLM 必须落到端点 B（reload 后新 client）"
    );
    // 端点 A 不应再有新请求：原有 1 条不变。
    let a_count = llm_a_bodies.lock().expect("poison").len();
    assert_eq!(
        a_count, 1,
        "端点 A 不应被第二轮命中（说明 client 已被替换）"
    );

    let _ = std::fs::remove_file(&tmp);
    drop(handle);
}

// ---- 2. 失败回退：坏 URL → 旧 client 继续工作 ----

#[test]
fn reload_falls_back_on_invalid_url() {
    // 端点 A：先跑通。
    let llm_a_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let llm_a_base = spawn_llm_mock(llm_a_bodies.clone(), canned_sse());
    let tts_base = spawn_tts_mock();
    let tmp = std::env::temp_dir().join(format!(
        "live2d_ai_test_reload_fallback_{}.toml",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&tmp);
    write_settings(&tmp, &llm_a_base);

    let client = OpenAiClient::new(
        LlmConfig::new(llm_a_base.clone(), "test-model"),
        TtsConfig::new(tts_base, "alloy"),
    )
    .expect("client");
    let collector: Collector = Arc::new(Mutex::new(Vec::new()));
    let ec = collector.clone();
    let handle = spawn_supervisor(
        SupervisorConfig {
            client,
            conversation: ConversationConfig::new("人设"),
            capabilities: ModelCapabilities::all(),
            audio: None,
            config_path: Some(tmp.to_string_lossy().into_owned()),
            mod_events: None,
        },
        move |ev| {
            ec.lock().expect("poison").push(ev);
        },
    );

    // 第一轮：端点 A 收到 1 条。
    assert!(handle.say("first"));
    // **flaky fix**：同 test 1，冷启动并发负载下放宽首轮 LLM 等待（10s）。
    assert!(
        wait_for(Duration::from_secs(10), || !llm_a_bodies
            .lock()
            .expect("poison")
            .is_empty()),
        "第一轮 LLM 必须已发出"
    );

    // 写一个坏 URL 的 settings（apply_patch 会拒收 `not a url`，但这里
    // 我们直接写盘模拟 PATCH 写盘成功 + 后续 resolve 失败路径——
    // supervisor 的 reload 是 resolve 阶段失败，留旧 engine）。
    std::fs::write(
        &tmp,
        r#"
        [llm]
        base_url = "not a url"
        model = "broken"
        "#,
    )
    .expect("write broken settings");

    // 触发 Reload：内部会失败（base_url 非法），保留旧 engine。
    handle.reload();
    // **flaky fix**：见 test 1 注释。固定 sleep(50) 在并发负载下偶发
    // 不足——替换为「等 reload_pending 被清」闸门：reload 后 supervisor
    // 必调用 apply_reload（即使失败路径也走完），swap 清 pending 标志
    // 是同步原子操作、且 apply_reload 内部全程同步（无 await），标志
    // 清零 ⇒ engine 状态已定（成功则替换，失败则保留旧 engine）。后续
    // say 必命中当前 engine，避免 select! 随机选臂把 say 抢在 Reload 之前。
    assert!(
        wait_for(Duration::from_secs(3), || !handle
            .reload_pending_for_test()
            .load(Ordering::SeqCst)),
        "reload 后 supervisor 必须已消费 reload_pending（即便 reload 失败也走完 apply_reload）"
    );

    // 第二轮：仍应落到端点 A（旧 client 不变）。
    let a_before = llm_a_bodies.lock().expect("poison").len();
    assert!(handle.say("second"));
    assert!(
        wait_for(Duration::from_secs(3), || llm_a_bodies
            .lock()
            .expect("poison")
            .len()
            > a_before),
        "reload 失败后旧 client 必须仍工作：第二轮仍到端点 A"
    );

    let _ = std::fs::remove_file(&tmp);
    drop(handle);
}

// ---- 3. 在飞 turn 语义：Reload 在 turn 中到达 → 当前 turn 收尾，**下一轮**用新配置 ----

#[test]
fn reload_during_inflight_turn_applies_to_next_turn() {
    // 端点 A：用于第一轮（in-flight）。
    let llm_a_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let llm_a_base = spawn_llm_mock(llm_a_bodies.clone(), canned_sse());
    let tts_base = spawn_tts_mock();
    let tmp = std::env::temp_dir().join(format!(
        "live2d_ai_test_reload_inflight_{}.toml",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&tmp);
    write_settings(&tmp, &llm_a_base);

    let client = OpenAiClient::new(
        LlmConfig::new(llm_a_base.clone(), "test-model"),
        TtsConfig::new(tts_base, "alloy"),
    )
    .expect("client");
    let collector: Collector = Arc::new(Mutex::new(Vec::new()));
    let ec = collector.clone();
    let handle = spawn_supervisor(
        SupervisorConfig {
            client,
            conversation: ConversationConfig::new("人设"),
            capabilities: ModelCapabilities::all(),
            audio: None,
            config_path: Some(tmp.to_string_lossy().into_owned()),
            mod_events: None,
        },
        move |ev| {
            ec.lock().expect("poison").push(ev);
        },
    );

    // 第一轮（in-flight 中）。
    // **flaky fix**：B mock 的 spawn + bind 推迟到首轮 LLM body 抵达之后。
    // 原顺序「spawn_supervisor → spawn B mock → say → write_settings →
    // reload」在 --test-threads=8 冷启动下导致本测试线程在 say() 之后
    // 还要做 3 个系统调用（spawn_llm_mock = bind + thread::spawn，
    // write_settings = fs::write），叠加其他 4 个测试的初始化线程开
    // 销，supervisor 线程易被饿死、首轮 LLM 请求卡过 10s 仍不到 A。
    // 推迟 B 的 spawn 到首轮收口之后，测试线程在 say() 后只做
    // write_settings（fs::write 是最快的）+ reload（无 I/O），最大限度
    // 让 supervisor 拿到 CPU 把首轮跑完。B 的 mock listener 后建仍能在
    // 「二次 reload + say("third")」之前就绪——二次 reload 是同步读盘 + 构
    // 建 client 的过程（无 I/O 等待），足以把 B listener 调度起来。
    //
    // 注：B 的 base_url 在第二阶段 write_settings 之前就确定（先随机绑
    // 一个临时端口、再把 URL 串写进 tmp 文件），后续二次 reload 读盘 +
    // 构造 client 用同一个 URL——B listener 在二次 reload 之前必已 bind。
    assert!(handle.say("first"));
    handle.reload();

    // 等第一轮收口。
    // **flaky fix**：同 test 1，冷启动并发负载下放宽首轮 LLM 等待（10s）。
    assert!(
        wait_for(Duration::from_secs(10), || !llm_a_bodies
            .lock()
            .expect("poison")
            .is_empty()),
        "第一轮必须已落到 A"
    );

    // 端点 B：首轮收口后再建 mock（避免与首轮争抢 CPU）。
    let llm_b_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let llm_b_base = spawn_llm_mock(llm_b_bodies.clone(), canned_sse());
    // 把 settings 改成指向 B（首轮期间 reload 读的还是旧 A config，没改）
    write_settings(&tmp, &llm_b_base);

    // Reload 在 in-flight 中被 turn.rs no-op 掉（最小实现）。
    // 让线程回到 idle：等 terminal 事件（任何 turn 完成后 supervisor 回 idle）。
    // 直接等 idle 即可（已 first run 完成）。
    // **flaky fix**：固定 sleep(100) 在并发负载下偶发不足——turn 收口后
    // supervisor 还会走「drain control_rx + 兜底 apply_reload」一串动作，
    // 全 workspace 并行测试下可能跑过 100ms 仍未进入 select!，后续二次
    // reload() 在控制通道里排队、与之后的 say("third") 在 select! 随机
    // 选臂时可能让 say 抢先走旧 engine。改为「等 reload_pending 清零」：
    // in-flight 阶段 reload 设的标志会在 turn 收口后的兜底 apply_reload
    // 里被 swap 清零——标志清 ⇒ fallback apply_reload 已同步执行完毕、
    // engine 已替换为 B（指向 llm_b_base）。
    assert!(
        wait_for(Duration::from_secs(3), || !handle
            .reload_pending_for_test()
            .load(Ordering::SeqCst)),
        "第一轮 in-flight 期间到达的 reload 必须被收口后兜底 apply_reload 处理"
    );

    // 第二轮：用户再次 PATCH / egui save / 或本测试再 reload 一次（实际
    // 场景中 PATCH 成功时 supervisor 已在 idle 才能 reload；in-flight 中到达
    // 的 reload 被吞，**下一次 PATCH 触发 reload 才能真正换 client**。
    // 这里我们直接再 reload 一次（模拟 PATCH 再次成功）。
    handle.reload();
    // **flaky fix**：同上，二次 reload 同样必须等到 pending 清零再放行
    // 后续 say，否则 select! 随机选臂会让 say 抢先走（虽然本次 engine 已
    // 是 B，但重建后的新实例未就位时仍可能引发时序歧义）。
    assert!(
        wait_for(Duration::from_secs(3), || !handle
            .reload_pending_for_test()
            .load(Ordering::SeqCst)),
        "二次 reload 后 supervisor 必须已消费 reload_pending"
    );

    // 第三轮：落到 B。
    assert!(handle.say("third"));
    assert!(
        wait_for(Duration::from_secs(3), || !llm_b_bodies
            .lock()
            .expect("poison")
            .is_empty()),
        "第三次 LLM 应落到 B（在飞 turn 中的 Reload 被吞，第二次 Reload 生效）"
    );

    let _ = std::fs::remove_file(&tmp);
    drop(handle);
}

// ---- 4. config_path 为 None 时 Reload 静默 no-op ----

#[test]
fn reload_without_config_path_is_noop() {
    // config_path: None —— 模拟 web 模式或测试装配。
    let sse = canned_sse();
    let llm_bodies: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let llm_base = spawn_llm_mock(llm_bodies.clone(), sse);
    let tts_base = spawn_tts_mock();
    let client = OpenAiClient::new(
        LlmConfig::new(llm_base, "test-model"),
        TtsConfig::new(tts_base, "alloy"),
    )
    .expect("client");
    let collector: Collector = Arc::new(Mutex::new(Vec::new()));
    let ec = collector.clone();
    let handle = spawn_supervisor(
        SupervisorConfig {
            client,
            conversation: ConversationConfig::new("人设"),
            capabilities: ModelCapabilities::all(),
            audio: None,
            config_path: None, // 关键：未配置
            mod_events: None,
        },
        move |ev| {
            ec.lock().expect("poison").push(ev);
        },
    );

    // Reload 收到但 config_path 为 None → no-op（tracing::debug），不发新请求。
    handle.reload();
    std::thread::sleep(Duration::from_millis(50));

    // 端点应保持 0 条（无 Reload 触发新请求）。
    assert_eq!(
        llm_bodies.lock().expect("poison").len(),
        0,
        "config_path=None 时 Reload 不应触发新请求"
    );

    // 正常 say 仍可工作。
    assert!(handle.say("hello"));
    assert!(
        wait_for(Duration::from_secs(3), || !llm_bodies
            .lock()
            .expect("poison")
            .is_empty()),
        "正常 say 仍能跑通"
    );

    drop(handle);
}

// ---- 5. 基础：SupervisorHandle::reload 不 panic、可重复调用 ----

#[test]
fn handle_reload_is_idempotent_and_does_not_panic() {
    let sse = canned_sse();
    let (handle, _collector, _llm_bodies) = spawn_text_only_collector_with_path(
        sse,
        std::env::temp_dir().join(format!(
            "live2d_ai_test_reload_idem_{}.toml",
            std::process::id()
        )),
    );
    // 多次 reload 都不应 panic。
    for _ in 0..5 {
        handle.reload();
    }
    std::thread::sleep(Duration::from_millis(50));
    drop(handle);
}
