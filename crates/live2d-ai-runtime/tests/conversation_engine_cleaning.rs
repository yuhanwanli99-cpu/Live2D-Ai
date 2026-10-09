//! **二路按句清洗**的引擎集成回归（2026-10-08）。
//!
//! 计划口径（`docs/plans/PROMPT-tts-llm-clean-2026-10-08.md`）：
//! 用一份**假响应** `{"display":"你好😄","speech":"你好"}` 证明——
//!
//! 1. 送 TTS 的任务文本是 `你好`（可念文本）；
//! 2. 上屏文本是 `你好😄`（原文切片，emoji 留着）；
//! 3. **第一句的提交发生在第二句原文出现之前**（不等整段回复收齐）；
//! 4. 假响应不是原文切片时，TTS 任务文本 = 原文（整句作废，不猜）；
//! 5. 二路整体失败时两份文本都用原文（不新写正则去剥）。
//!
//! 二路本身用一个注入的假客户端（零网络）；真 HTTP 客户端的线上形态由
//! `performance::client` 的 wire 测试钉住（含 `thinking.type == disabled`）。

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use live2d_ai_runtime::cleaning::{CLEAN_TIMEOUT_MS, SentenceCleaner};
use live2d_ai_runtime::performance::client::{
    PerformanceClient, PerformanceFuture, PerformanceReply,
};
use live2d_ai_runtime::{ConversationConfig, EngineEvent, TurnStatus};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use common::{drain_events, engine, spawn_multi_server, spawn_tts_mock, sse_content, sse_done};

/// 固定回一份二路 JSON 的替身（`reply=None` = 请求失败）。
#[derive(Debug)]
struct StubCleaner {
    reply: Option<String>,
    /// 第几次调用（从 0 起）；**第 0 次会先等闸门**（用来证明「清洗期间不读下一条
    /// token」＝第一句的提交先于第二句原文）。
    calls: Arc<AtomicUsize>,
    gate: Arc<tokio::sync::Notify>,
}

impl PerformanceClient for StubCleaner {
    fn enabled(&self) -> bool {
        true
    }
    fn kind(&self) -> &'static str {
        "injected"
    }
    fn has_api_key(&self) -> bool {
        false
    }
    fn request<'a>(
        &'a self,
        _system: &'a str,
        _user: &'a str,
        _timeout_ms: u64,
    ) -> PerformanceFuture<'a, Option<PerformanceReply>> {
        let reply = self.reply.clone().map(|raw| PerformanceReply {
            raw,
            structured: false,
        });
        // 只有「会成功回一份 JSON」的那条才卡闸门；失败替身（reply=None）
        // 直接返回 None，否则第二句/失败用例会一直等一个永远不来的放行。
        let first = reply.is_some() && self.calls.fetch_add(1, Ordering::SeqCst) == 0;
        let gate = Arc::clone(&self.gate);
        Box::pin(async move {
            if first {
                gate.notified().await;
            }
            reply
        })
    }
}

fn cleaned_engine(
    llm_base: &str,
    tts_base: &str,
    reply: Option<&str>,
    gate: Arc<tokio::sync::Notify>,
) -> live2d_ai_runtime::ConversationEngine {
    let mut eng = engine(llm_base, tts_base, ConversationConfig::default());
    eng.set_cleaner(Some(Arc::new(SentenceCleaner::new(
        Box::new(StubCleaner {
            reply: reply.map(str::to_string),
            calls: Arc::new(AtomicUsize::new(0)),
            gate,
        }),
        CLEAN_TIMEOUT_MS,
    ))));
    eng
}

/// 按 seq 收集**上屏**文本（`SentenceVoiced`）。
fn voiced_by_seq(events: &[EngineEvent]) -> Vec<(u64, String)> {
    events
        .iter()
        .filter_map(|e| match e {
            EngineEvent::SentenceVoiced {
                sentence_seq, text, ..
            } => Some((*sentence_seq, text.clone())),
            _ => None,
        })
        .collect()
}

/// 按 seq 收集 `SentenceReady` 文本（送 TTS 之前的锚点；其 push_job 紧随其后）。
fn ready_by_seq(events: &[EngineEvent]) -> Vec<(u64, String)> {
    events
        .iter()
        .filter_map(|e| match e {
            EngineEvent::SentenceReady {
                sentence_seq, text, ..
            } => Some((*sentence_seq, text.clone())),
            _ => None,
        })
        .collect()
}

fn first_text_delta_text(events: &[EngineEvent]) -> String {
    events
        .iter()
        .find_map(|e| match e {
            EngineEvent::TextDelta { text, .. } => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// 轮询等待一个条件（不睡死；超时即判失败）。
async fn wait_until(timeout: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        if cond() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    cond()
}

/// **计划验收的那条**：一份假响应 → 两份文本 + 第一句先提交（**不成批**）。
///
/// 顺序怎么证明（不靠时间戳猜）：二路替身对**第一句**卡在闸门上，测试据此断言
/// 「第一句的清洗还没回来时，第二句**没有被清洗、也没有被提交**」；放行后才出现
/// `SentenceReady(1)`（紧随其后就是 `push_job`）→ 第二句才被清洗/提交。
///
/// 边界（如实标注）：**第二句的 `text_delta` 可以先于第一句的 `SentenceReady`**，
/// 因为既有分句器要前瞻一个字符才能把句末的终止符 run 封口（`SentenceSplitter`
/// 的「任意 delta 切割结果一致」不变量）——那是分句器的既有性质，不是本轮改动，
/// 也不代表成批：引擎在清洗期间不会开始第二句的清洗。
#[tokio::test]
async fn first_sentence_is_submitted_before_the_second_sentence_arrives() {
    let llm_base = spawn_multi_server(common::respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("你好😄。"), sse_content("再见。"), sse_done()],
    ))
    .await;
    let (tts_base, tts_inputs) = spawn_tts_mock().await;
    let gate = Arc::new(tokio::sync::Notify::new());
    let calls = Arc::new(AtomicUsize::new(0));
    let mut eng = engine(&llm_base, &tts_base, ConversationConfig::default());
    eng.set_cleaner(Some(Arc::new(SentenceCleaner::new(
        Box::new(StubCleaner {
            reply: Some(r#"{"display":"你好😄","speech":"你好"}"#.to_string()),
            calls: Arc::clone(&calls),
            gate: Arc::clone(&gate),
        }),
        CLEAN_TIMEOUT_MS,
    ))));

    let (tx, mut rx) = mpsc::channel(64);
    let turn = tokio::spawn(async move {
        eng.run_turn(301, "打个招呼", tx, CancellationToken::new())
            .await
    });

    // ① 第一句原文到达（分句器的前瞻性质决定了第二个 delta 也会先到，收下即可）。
    let mut events: Vec<EngineEvent> = Vec::new();
    let first = tokio::time::timeout(Duration::from_secs(10), rx.recv())
        .await
        .expect("必须收到第一个事件")
        .expect("通道未关");
    assert!(
        matches!(&first, EngineEvent::TextDelta { text, .. } if text == "你好😄。"),
        "第一个事件应是第一句原文的 text_delta：{first:?}"
    );
    events.push(first);

    // ② 第一句的二路清洗已经开始并卡在闸门上。
    assert!(
        wait_until(Duration::from_secs(5), || calls.load(Ordering::SeqCst) >= 1).await,
        "第一句原文一到就必须立刻交给二路"
    );

    // ③ 闸门没放行之前：第二句**不得**被清洗、不得上屏、不得提交 TTS。
    let deadline = tokio::time::Instant::now() + Duration::from_millis(300);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout_at(deadline, rx.recv()).await {
            Ok(Some(ev)) => events.push(ev),
            _ => break,
        }
    }
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "第一句没交回之前不得开始第二句的清洗（= 不成批）"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, EngineEvent::SentenceReady { .. })),
        "第一句没交回之前不得有任何 SentenceReady：{events:?}"
    );
    assert!(
        tts_inputs.lock().expect("poison").is_empty(),
        "第一句没交回之前不得给 TTS 发任何请求"
    );

    // 放行第一句的二路。
    gate.notify_one();

    let report = turn.await.expect("turn 任务");
    assert_eq!(report.status, TurnStatus::Completed);
    // 回灌历史的仍是主模型**原文**（二路产物不进模型上下文）。
    assert_eq!(report.assistant_text, "你好😄。再见。");

    events.extend(drain_events(rx).await);

    // ③ 送 TTS 的第一句是**可念文本**；第二句的假响应不是它的原文切片 → 改送原文。
    let inputs = tts_inputs.lock().expect("poison").clone();
    assert_eq!(
        inputs,
        ["你好", "再见。"],
        "第一句送可念文本，第二句不是切片就送原文：{inputs:?}"
    );

    // ④ 上屏用的是 **display**（保留 emoji），不是送 TTS 的那一份。
    assert_eq!(
        ready_by_seq(&events),
        [(1, "你好😄".to_string()), (2, "再见。".to_string())],
        "SentenceReady 是上屏文本（display）"
    );
    assert_eq!(
        voiced_by_seq(&events),
        [(1, "你好😄".to_string()), (2, "再见。".to_string())],
        "气泡上屏 == display（不是 speech）"
    );

    // ⑤ 顺序：第一句的 SentenceReady（紧随其后就是 push_job）先于第二句的。
    let ready1 = events
        .iter()
        .position(|e| {
            matches!(
                e,
                EngineEvent::SentenceReady {
                    sentence_seq: 1,
                    ..
                }
            )
        })
        .expect("必须有第 1 句的 SentenceReady");
    let ready2 = events
        .iter()
        .position(|e| {
            matches!(
                e,
                EngineEvent::SentenceReady {
                    sentence_seq: 2,
                    ..
                }
            )
        })
        .expect("必须有第 2 句的 SentenceReady");
    assert!(
        ready1 < ready2,
        "第一句必须先提交：ready1={ready1} ready2={ready2}"
    );
    assert_eq!(first_text_delta_text(&events), "你好😄。");
    assert_eq!(calls.load(Ordering::SeqCst), 2, "两句各清洗一次");
}

/// 二路**整体失败**（请求 None）→ 两份文本都是原文，整轮照常出声。
#[tokio::test]
async fn failed_cleaner_falls_back_to_the_raw_sentence() {
    let llm_base = spawn_multi_server(common::respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("（挥手）你好呀。"), sse_done()],
    ))
    .await;
    let (tts_base, tts_inputs) = spawn_tts_mock().await;
    let mut eng = cleaned_engine(
        &llm_base,
        &tts_base,
        None,
        Arc::new(tokio::sync::Notify::new()),
    );
    let (tx, rx) = mpsc::channel(64);
    let report = eng
        .run_turn(302, "你好", tx, CancellationToken::new())
        .await;
    assert_eq!(report.status, TurnStatus::Completed);
    assert_eq!(
        *tts_inputs.lock().expect("poison"),
        ["（挥手）你好呀。"],
        "二路失败 = 改送原文（不新写正则去剥）"
    );
    let events = drain_events(rx).await;
    assert_eq!(
        voiced_by_seq(&events),
        [(1, "（挥手）你好呀。".to_string())],
        "失败时原文照上屏（括号里的字都留着）"
    );
}
