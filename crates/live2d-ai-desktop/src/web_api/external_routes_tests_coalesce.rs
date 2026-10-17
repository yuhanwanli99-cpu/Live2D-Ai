//! 外部事件「忙时合并再注入」回归（3.0 队首第一片，2026-10-17）。
//!
//! 上一条在飞时连送三条 → 主链空出来 → **只注入一次**、正文按到达顺序合成；
//! 空闲时仍逐条注入；窗口满回退旧的 `busy`。注入点替身直接驱动
//! `external_coalesce::flush_once`（生产线程调的就是它），另有真 supervisor
//! 的端到端一条，覆盖「在飞判定 → 收下 → 轮末合并注入」整条链。

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use super::tests_support::*;
use super::*;
use crate::web_api::external_coalesce::{
    COALESCE_SEP, FlushOutcome, MAX_WINDOW_CHARS, flush_once, spawn_flusher,
};

/// 可切换「忙 / 空闲」、记录每次注入正文的替身注入点。
#[derive(Clone, Default)]
struct FakeInject {
    busy: Arc<AtomicBool>,
    calls: Arc<AtomicUsize>,
    injected: Arc<Mutex<Vec<String>>>,
}

impl FakeInject {
    fn busy() -> Self {
        let f = Self::default();
        f.busy.store(true, Ordering::SeqCst);
        f
    }

    fn free(&self) {
        self.busy.store(false, Ordering::SeqCst);
    }

    fn call(&self, _ctx: &ServerContext, text: String) -> Option<bool> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.busy.load(Ordering::SeqCst) {
            return Some(false);
        }
        self.injected.lock().unwrap().push(text);
        Some(true)
    }

    fn injections(&self) -> Vec<String> {
        self.injected.lock().unwrap().clone()
    }
}

/// 启用了 `external-input` 的上下文（端点门禁放行）。
fn ctx_enabled() -> ServerContext {
    let sec = crate::web_api::security::SecurityContext::new(18099, true);
    let ctx = ctx_with_manifest(
        sec,
        &serde_json::json!({"mods":{"external-input":{"enabled": true}}}),
    );
    ctx.mod_registry.lock().unwrap().start_all();
    ctx
}

/// 走 handler + 替身注入点发一条外部文本，返回响应体。
fn post(ctx: &ServerContext, inject: &FakeInject, text: &str) -> String {
    use std::io::Read as _;

    let method = method_post();
    let body = serde_json::json!({"text": text}).to_string();
    let call = |c: &ServerContext, t: String| inject.call(c, t);
    let resp = handle_external_chat_with(ctx, req(&method, &body), &call, &no_env_lookup)
        .expect("external chat 路由必须命中");
    let mut out = String::new();
    let _ = resp.into_reader().read_to_string(&mut out);
    out
}

/// 走生产 handler（真实 `supervisor_inject`）发一条外部文本，返回响应体。
fn post_real(ctx: &ServerContext, text: &str) -> String {
    use std::io::Read as _;

    let method = method_post();
    let body = serde_json::json!({"text": text}).to_string();
    let resp = call(
        ctx,
        &method,
        EXTERNAL_CHAT_PATH,
        &body,
        None,
        Some("application/json"),
    )
    .expect("external chat 路由必须命中");
    let mut out = String::new();
    let _ = resp.into_reader().read_to_string(&mut out);
    out
}

/// 忙时连送三条：只注入一次，正文是按到达顺序合成的那一条。
#[test]
fn busy_three_sends_coalesce_into_one_injection_in_order() {
    let _guard = counter_lock();
    let ctx = ctx_enabled();
    let fake = FakeInject::busy();
    let before = live2d_ai_mod_external_input::counters_snapshot();
    let base = |k: &str| before[k].as_u64().expect("计数为整数");

    for text in ["一", "二", "三"] {
        let body = post(&ctx, &fake, text);
        assert!(body.contains("\"ok\":true"), "忙时应收下而非丢弃：{body}");
        assert!(
            body.contains("\"coalesced\":true"),
            "忙时应标注 coalesced：{body}"
        );
    }

    // 忙时没有任何一次直送成功；三条都在窗口里。
    assert!(fake.injections().is_empty(), "忙时不应直送");
    assert_eq!(ctx.external_coalescer.pending(), 3, "三条都进合并窗口");

    // 主链空出来：一次注入，正文 = 三条按到达顺序拼成的一条。
    fake.free();
    let flushed = |c: &ServerContext, t: String| fake.call(c, t);
    assert_eq!(flush_once(&ctx, &flushed), FlushOutcome::Injected);
    assert_eq!(
        fake.injections(),
        vec![format!("一{COALESCE_SEP}二{COALESCE_SEP}三")],
        "整窗只注入一次，顺序 = 到达顺序"
    );
    // 窗口已清空：再 flush = 无注入。
    assert_eq!(flush_once(&ctx, &flushed), FlushOutcome::Empty);
    assert_eq!(fake.injections().len(), 1, "一次窗口只注入一次");

    let after = live2d_ai_mod_external_input::counters_snapshot();
    assert_eq!(after["accepts"], base("accepts") + 3, "端口收下 3 条");
    assert_eq!(after["busy"], base("busy") + 3, "3 条都是忙时到达");
}

/// 空闲窗口：仍一条一条注入（不进合并窗口）。
#[test]
fn idle_sends_still_inject_one_by_one() {
    let _guard = counter_lock();
    let ctx = ctx_enabled();
    let fake = FakeInject::default(); // 空闲
    let before = live2d_ai_mod_external_input::counters_snapshot();
    let base = |k: &str| before[k].as_u64().expect("计数为整数");

    for text in ["甲", "乙", "丙"] {
        let body = post(&ctx, &fake, text);
        assert!(body.contains("\"ok\":true"), "{body}");
        assert!(!body.contains("coalesced"), "空闲时不应合并：{body}");
    }
    assert_eq!(fake.injections(), vec!["甲", "乙", "丙"], "逐条注入");
    assert_eq!(ctx.external_coalescer.pending(), 0, "窗口始终为空");

    let after = live2d_ai_mod_external_input::counters_snapshot();
    assert_eq!(after["accepts"], base("accepts") + 3);
    assert_eq!(after["busy"], base("busy"), "空闲路径不记 busy");
}

/// 窗口满（≥ MAX_WINDOW_CHARS）：回退旧语义——200 + ok:false busy，本条丢弃。
#[test]
fn full_window_falls_back_to_busy_drop() {
    let _guard = counter_lock();
    let ctx = ctx_enabled();
    let fake = FakeInject::busy();
    let long = "x".repeat(MAX_WINDOW_CHARS);

    let first = post(&ctx, &fake, &long);
    assert!(first.contains("\"coalesced\":true"), "{first}");

    let overflow = post(&ctx, &fake, "y");
    assert!(
        overflow.contains("\"ok\":false"),
        "窗口满应回退 busy：{overflow}"
    );
    assert!(overflow.contains("\"busy\""), "{overflow}");
    assert_eq!(ctx.external_coalescer.pending(), 1, "只有第一条进窗口");

    fake.free();
    let flushed = |c: &ServerContext, t: String| fake.call(c, t);
    assert_eq!(flush_once(&ctx, &flushed), FlushOutcome::Injected);
    assert_eq!(fake.injections(), vec![long], "窗口里的那条照常注入");
}

/// 端到端验收：一轮在飞时经真实 handler 连送三条 → **只注入一次**，
/// 正文是三条合成的那一条（`user_line` 帧证明确实进了主链路）。
#[test]
fn in_flight_three_sends_coalesce_into_one_injection() {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    let ctx = ctx_enabled();
    let broadcaster = ctx.broadcaster.clone();
    let llm_base = spawn_slow_llm(Duration::from_millis(500));
    let client = live2d_ai_runtime::OpenAiClient::new(
        live2d_ai_runtime::LlmConfig::new(&llm_base, "m"),
        live2d_ai_runtime::TtsConfig::new("http://127.0.0.1:2/v1", "alloy"),
    )
    .expect("client");
    let emit_bc = broadcaster.clone();
    let handle = Arc::new(crate::supervisor::spawn_supervisor(
        crate::supervisor::SupervisorConfig {
            client,
            conversation: live2d_ai_runtime::ConversationConfig::new("人设"),
            capabilities: live2d_ai_core::ModelCapabilities::all(),
            audio: None,
            config_path: None,
            mod_events: None,
        },
        move |event| {
            if let Some(frame) = crate::web_api::ws::app_event_to_ws_frame(&event) {
                emit_bc.broadcast(&frame.to_string());
            }
        },
    ));
    ctx.supervisor_slot.set(Arc::clone(&handle));
    let (tx, rx) = mpsc::sync_channel::<String>(64);
    broadcaster.try_subscribe(tx).expect("subscribe");
    let flusher = spawn_flusher(ctx.clone());

    // 起一轮并等它真的在飞：从这一刻起外部文本应「收下、先不注入」。
    assert!(handle.say("第一条"));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !handle.turn_in_flight() {
        assert!(Instant::now() < deadline, "超时：第一轮没有开始");
        std::thread::sleep(Duration::from_millis(5));
    }

    for text in ["甲", "乙", "丙"] {
        let body = post_real(&ctx, text);
        assert!(body.contains("\"ok\":true"), "在飞时应收下：{body}");
        assert!(
            body.contains("\"coalesced\":true"),
            "在飞时应进合并窗口：{body}"
        );
    }
    assert_eq!(ctx.external_coalescer.pending(), 3, "三条都进合并窗口");

    // user_line 只应出现两条：第一条一轮 + 合并后的三条一轮。
    let mut lines = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while lines.len() < 2 {
        let left = deadline.saturating_duration_since(Instant::now());
        assert!(!left.is_zero(), "超时：合并文本没有被注入；已收 {lines:?}");
        let frame = rx.recv_timeout(left).expect("超时：没有收到 user_line");
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&frame)
            && v["type"] == "user_line"
        {
            lines.push(v["data"]["text"].as_str().unwrap_or_default().to_string());
        }
    }
    assert_eq!(
        lines,
        vec![
            "第一条".to_string(),
            format!("甲{COALESCE_SEP}乙{COALESCE_SEP}丙"),
        ],
        "忙时三条只注入一次，正文按到达顺序合成"
    );

    // 收尾：先停常驻线程，再回收 supervisor（quit 后 join）。
    ctx.external_coalescer.shutdown();
    flusher.join().expect("flusher join");
    ctx.supervisor_slot.take();
    handle.quit();
    if let Ok(handle) = Arc::try_unwrap(handle) {
        handle.join();
    }
}

/// 无积压时停机：常驻线程阻塞在 Condvar 上，shutdown 后立即退出。
#[test]
fn flusher_thread_stops_on_shutdown_without_backlog() {
    let ctx = ctx_enabled();
    let flusher = spawn_flusher(ctx.clone());
    ctx.external_coalescer.shutdown();
    flusher.join().expect("flusher join");
}

/// 慢速 LLM mock：读完请求头 → 睡 `delay` → 回一条最小 SSE，让一轮稳定「在飞」。
///
/// TTS 端点不可达 → 该轮最终失败；但 `user_line` 在轮内早已发出，不影响本测试
/// 要验证的「注入了几次 / 正文是什么」。
fn spawn_slow_llm(delay: std::time::Duration) -> String {
    use std::io::{Read as _, Write as _};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind llm");
    let addr = listener.local_addr().expect("addr");
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut sock) = conn else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let mut got = Vec::new();
                loop {
                    match sock.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            got.extend_from_slice(&buf[..n]);
                            if got.windows(4).any(|w| w == b"\r\n\r\n") {
                                break;
                            }
                        }
                    }
                }
                std::thread::sleep(delay);
                let sse =
                    "data: {\"choices\":[{\"delta\":{\"content\":\"好。\"}}]}\n\ndata: [DONE]\n\n";
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    sse.len()
                );
                let _ = sock.write_all(head.as_bytes());
                let _ = sock.write_all(sse.as_bytes());
                let _ = sock.flush();
            });
        }
    });
    format!("http://{addr}/v1")
}
