//! **M4 轮后「记」**的判据：后台、不阻塞、失败不致命、加固与摘要有界。
//!
//! 计划 §6 M4 的三条要求逐条钉住：
//! 1. 挂在 `commit_completed_turn` **之后**、另起任务；说的 HTTP 响应不等它；
//! 2. 抽失败 → 记 episode 原文（截断 200 字），**不调用第二次**；
//! 3. 近期超过 `max_history_pairs` 的部分折成 ≤300 字摘要，替换上一条。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use live2d_ai_runtime::hotmem::{FactKind, Recollection};

use crate::hot_recall::{Recalled, recall_once};
use crate::hot_slot::HotSlot;

/// 造一个只回应固定文本的 LLM mock（用来喂「抽取」）。
fn spawn_extraction_mock(body: String, hits: Arc<AtomicUsize>) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(mut sock) = conn else { continue };
            let body = body.clone();
            let hits = hits.clone();
            std::thread::spawn(move || {
                // 读掉请求（到 \r\n\r\n 之后按 Content-Length 读完），再回 SSE。
                let mut buf = Vec::new();
                let mut tmp = [0u8; 4096];
                use std::io::{Read, Write};
                loop {
                    match sock.read(&mut tmp) {
                        Ok(0) => break,
                        Ok(n) => {
                            buf.extend_from_slice(&tmp[..n]);
                            if let Some(pos) = find_headers_end(&buf) {
                                let head = String::from_utf8_lossy(&buf[..pos]).to_string();
                                let len = content_length(&head);
                                if buf.len() >= pos + 4 + len {
                                    break;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
                hits.fetch_add(1, Ordering::SeqCst);
                let sse = format!(
                    "data: {}\n\ndata: [DONE]\n\n",
                    serde_json::json!({"choices":[{"delta":{"content": body}}]})
                );
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    sse.len(),
                    sse
                );
                let _ = sock.write_all(resp.as_bytes());
            });
        }
    });
    format!("http://{addr}/v1")
}

fn find_headers_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn content_length(head: &str) -> usize {
    for line in head.lines() {
        if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            return v.trim().parse().unwrap_or(0);
        }
    }
    0
}

fn client_for(base_url: &str) -> live2d_ai_runtime::OpenAiClient {
    live2d_ai_runtime::OpenAiClient::new(
        live2d_ai_runtime::LlmConfig::new(base_url, "test-model"),
        live2d_ai_runtime::TtsConfig::new("http://127.0.0.1:1/v1", "alloy"),
    )
    .expect("client")
}

/// **抽取成功**：模型回一个 JSON 数组 → 事实进库，durable 变钉住候选。
#[tokio::test]
async fn recall_success_writes_facts_into_the_slot() {
    let hits = Arc::new(AtomicUsize::new(0));
    let base = spawn_extraction_mock(
        r#"[{"text":"用户叫星梦","kind":"durable"},{"text":"今天聊了天气","kind":"episode"}]"#
            .into(),
        hits.clone(),
    );
    let client = client_for(&base);
    let slot = HotSlot::new();
    let recalled = recall_once(&client, "我叫星梦", "记住了。").await;
    let Recalled::Items(items) = recalled else {
        panic!("抽取应成功: {recalled:?}");
    };
    assert_eq!(items.len(), 2);
    assert_eq!(hits.load(Ordering::SeqCst), 1, "只调用一次，不重试");
    assert_eq!(slot.record(items), 2);
    assert_eq!(slot.snapshot().facts, 2);
    assert_eq!(slot.snapshot().durable, 1);
    assert_eq!(slot.pinned_items().len(), 1, "durable 那条应进钉住候选");
}

/// **抽取失败**：模型回一段没有数组的文本 → Failed，调用方记 episode 原文，
/// 且**不发生第二次请求**。
#[tokio::test]
async fn recall_failure_does_not_retry_and_falls_back_to_episode() {
    let hits = Arc::new(AtomicUsize::new(0));
    let base = spawn_extraction_mock("我不太确定该记什么。".into(), hits.clone());
    let client = client_for(&base);
    let slot = HotSlot::new();
    let recalled = recall_once(&client, "今天聊了天气", "是啊。").await;
    assert!(matches!(recalled, Recalled::Failed(_)), "{recalled:?}");
    assert_eq!(hits.load(Ordering::SeqCst), 1, "失败后不得再调用第二次");

    // 兜底：把这一轮原文记成 episode（截断到 200 字）。
    let written = slot.record_fallback_episode(&"字".repeat(500));
    assert_eq!(written, 1);
    let facts = slot.facts_for_test();
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].kind, FactKind::Episode);
    assert_eq!(
        facts[0].text.chars().count(),
        200,
        "兜底 episode 截断到 200 字"
    );
}

/// **抽取超时**也走 Failed（不重试）。
#[tokio::test]
async fn recall_timeout_is_a_failure_not_a_retry() {
    // 一个只接受连接、永不回包的 server。
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_server = hits.clone();
    std::thread::spawn(move || {
        let mut held = Vec::new();
        for conn in listener.incoming() {
            let Ok(sock) = conn else { continue };
            hits_server.fetch_add(1, Ordering::SeqCst);
            held.push(sock); // 不读不写，让客户端超时
        }
    });
    let client = client_for(&format!("http://{addr}/v1"));
    let started = std::time::Instant::now();
    // 这条测试要**真等满**抽取超时（10 s），是整套测试里最慢的一条；
    // 它守的是「上游接受连接但不回包 → 后台任务不会无限期挂着」。
    let recalled = recall_once(&client, "你好", "你好").await;
    assert!(matches!(recalled, Recalled::Failed(_)), "{recalled:?}");
    assert!(started.elapsed() < std::time::Duration::from_secs(30));
    assert_eq!(hits.load(Ordering::SeqCst), 1, "超时后不得重试");
}

/// **加固**：同一句原文再被记一次 → 不新增条目，强度 ×1.6/1.8。
#[test]
fn recording_the_same_text_again_reinforces_instead_of_duplicating() {
    let slot = HotSlot::new();
    slot.record(vec![Recollection::new("用户喜欢薄荷", FactKind::Durable)]);
    let before = slot.facts_for_test()[0].s_hours;
    slot.record(vec![Recollection::new("用户喜欢薄荷", FactKind::Durable)]);
    let facts = slot.facts_for_test();
    assert_eq!(facts.len(), 1, "同一原文不得产生第二条");
    assert!(
        facts[0].s_hours > before,
        "再被提到应加固：{} → {}",
        before,
        facts[0].s_hours
    );
}

/// **一次一轮有上限**：抽取回来一堆也只收 8 条（不让库被一次撑大）。
#[test]
fn recording_is_bounded_per_turn() {
    let slot = HotSlot::new();
    let items: Vec<Recollection> = (0..50)
        .map(|i| Recollection::new(format!("第{i}条"), FactKind::Episode))
        .collect();
    let written = slot.record(items);
    assert_eq!(written, crate::hot_slot::MAX_RECALLED_PER_TURN);
    assert_eq!(
        slot.snapshot().facts,
        crate::hot_slot::MAX_RECALLED_PER_TURN
    );
}

/// **摘要**：替换上一条，长度有界（≥3 轮仍 ≤300 字）。
#[test]
fn summary_is_replaced_and_bounded() {
    let slot = HotSlot::new();
    for round in 0..20 {
        let dropped: Vec<(String, String)> =
            vec![(format!("第{round}轮用户"), format!("第{round}轮助手"))];
        let previous = slot.summary(Some("s1"));
        let folded = crate::hot_recall::fold(&previous, &dropped);
        slot.set_summary(Some("s1"), &folded);
    }
    let summary = slot.summary(Some("s1"));
    assert!(!summary.is_empty());
    assert!(
        summary.chars().count() <= live2d_ai_runtime::hotmem::SUMMARY_MAX_CHARS,
        "摘要长度 {} 超限",
        summary.chars().count()
    );
    assert!(summary.contains("19"), "最新一轮必须还在：{summary}");
}

/// 兜底 episode 的截断长度常量是契约（计划 §3「原文对应一次，截断到 200 字」）。
#[test]
fn fallback_episode_truncation_is_the_documented_value() {
    assert_eq!(crate::hot_slot::FALLBACK_EPISODE_CHARS, 200);
}

/// 「查」的最长等待常量是契约（计划 §3「最多等 1500 ms」）。
#[test]
fn retrieve_wait_is_the_documented_value() {
    assert_eq!(crate::hot_slot::RETRIEVE_WAIT_MS, 1_500);
}
